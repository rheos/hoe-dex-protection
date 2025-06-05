use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use anchor_spl::token::{self, Mint, Token, TokenAccount};

declare_id!("YourProgramIdHere"); // Replace with your program ID after deployment

/// HOE DEX Protection Program
/// This program implements a DEX protection mechanism with the following features:
/// - Snipe protection with configurable duration
/// - Early trade fees with basis points precision
/// - Trade size limits and cooldown periods
/// - Admin-controlled parameter updates
/// - Fee collection and withdrawal
#[program]
pub mod hoe_dex_protection {
    use super::*;

    /// Initialize a new pool protection instance
    /// 
    /// # Arguments
    /// * `snipe_protection_seconds` - Duration of snipe protection in seconds
    /// * `early_trade_fee_bps` - Fee for early trades in basis points (1-1000)
    /// * `max_trade_size_bps` - Maximum trade size as percentage of pool (1-1000)
    /// * `min_trade_size` - Minimum trade size in token's smallest unit
    /// * `cooldown_seconds` - Cooldown period between trades in seconds
    pub fn initialize_pool_protection(
        ctx: Context<InitializePoolProtection>,
        snipe_protection_seconds: u64,
        early_trade_fee_bps: u64,
        max_trade_size_bps: u64,
        min_trade_size: u64,
        cooldown_seconds: u64,
    ) -> Result<()> {
        // Validate system program
        require!(
            ctx.accounts.system_program.key() == system_program::ID,
            ErrorCode::InvalidSystemProgram
        );

        // Validate rent
        let rent = &ctx.accounts.rent;
        let space = 8 + 32 + 8 + 8 + 8 + 8 + 8 + 1 + 8 + 8 + 8 + 8 + 32 + 8;
        let rent_lamports = rent.minimum_balance(space);
        require!(
            ctx.accounts.admin.lamports() >= rent_lamports,
            ErrorCode::InsufficientFunds
        );

        // Validate token program
        require!(
            ctx.accounts.token_program.key() == token::ID,
            ErrorCode::InvalidTokenProgram
        );

        // Validate parameters
        require!(early_trade_fee_bps <= 1000, ErrorCode::FeeTooHigh); // Cap at 10%
        require!(max_trade_size_bps <= 1000, ErrorCode::TradeTooLarge); // Max 10% of pool
        require!(cooldown_seconds <= 3600, ErrorCode::InvalidAmount); // Max 1 hour cooldown
        require!(min_trade_size > 0, ErrorCode::InvalidAmount);
        require!(snipe_protection_seconds > 0, ErrorCode::InvalidAmount);

        // Validate parameter relationships
        require!(
            min_trade_size <= max_trade_size_bps.checked_mul(1_000_000).unwrap().checked_div(10000).unwrap(),
            ErrorCode::InvalidParameterRelationship
        );
        require!(
            cooldown_seconds <= snipe_protection_seconds,
            ErrorCode::InvalidParameterRelationship
        );

        // Validate token mint
        let token_mint = &ctx.accounts.token_mint;
        require!(token_mint.mint_authority.is_some(), ErrorCode::InvalidTokenMint);
        require!(token_mint.decimals <= 9, ErrorCode::InvalidTokenDecimals);

        // Get current time for initialization
        let current_time = Clock::get()?.unix_timestamp;

        // Initialize pool state
        let pool_state = &mut ctx.accounts.pool_state;
        pool_state.admin = *ctx.accounts.admin.key;
        pool_state.token_mint = token_mint.key();
        pool_state.token_decimals = token_mint.decimals;
        pool_state.snipe_protection_seconds = snipe_protection_seconds;
        pool_state.early_trade_fee_bps = early_trade_fee_bps;
        pool_state.pool_start_time = current_time;
        pool_state.total_fees_collected = 0;
        pool_state.total_liquidity = 0;
        pool_state.is_paused = false;
        pool_state.max_trade_size_bps = max_trade_size_bps;
        pool_state.min_trade_size = min_trade_size;
        pool_state.cooldown_seconds = cooldown_seconds;
        pool_state.last_trade_time = 0;
        pool_state.version = 1;
        pool_state.last_update = current_time;

        // Emit initialization event
        emit!(PoolInitialized {
            admin: pool_state.admin,
            token_mint: pool_state.token_mint,
            snipe_protection_seconds,
            early_trade_fee_bps,
            max_trade_size_bps,
            min_trade_size,
            cooldown_seconds,
            timestamp: current_time,
        });

        Ok(())
    }

    /// Add liquidity to the pool
    /// 
    /// # Arguments
    /// * `amount` - Amount of tokens to add to the pool
    pub fn add_liquidity(ctx: Context<AddLiquidity>, amount: u64) -> Result<()> {
        // Validate token program
        require!(
            ctx.accounts.token_program.key() == token::ID,
            ErrorCode::InvalidTokenProgram
        );

        // Validate amount
        require!(amount > 0, ErrorCode::InvalidAmount);
        
        let pool_state = &mut ctx.accounts.pool_state;
        
        // Validate pool state
        require!(pool_state.pool_start_time == 0, ErrorCode::PoolAlreadyStarted);
        require!(!pool_state.is_paused, ErrorCode::PoolPaused);
        
        // Validate token accounts
        require!(
            ctx.accounts.admin_token_account.owner == ctx.accounts.admin.key(),
            ErrorCode::InvalidTokenAccount
        );
        require!(
            ctx.accounts.pool_token_account.owner == ctx.accounts.pool_authority.key(),
            ErrorCode::InvalidTokenAccount
        );
        
        // Validate pool authority
        let (pool_authority, _) = Pubkey::find_program_address(
            &[b"pool_authority", pool_state.key().as_ref()],
            program_id
        );
        require!(
            pool_authority == ctx.accounts.pool_authority.key(),
            ErrorCode::InvalidPoolAuthority
        );
        
        // Get current time
        let current_time = Clock::get()?.unix_timestamp;
        
        // Update pool state
        pool_state.pool_start_time = current_time;
        pool_state.total_liquidity = amount;
        pool_state.last_update = current_time;
        
        // Transfer tokens to pool
        let cpi_accounts = token::Transfer {
            from: ctx.accounts.admin_token_account.to_account_info(),
            to: ctx.accounts.pool_token_account.to_account_info(),
            authority: ctx.accounts.admin.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(CpiContext::new(cpi_program, cpi_accounts), amount)?;
        
        // Emit liquidity added event
        emit!(LiquidityAdded {
            pool: pool_state.key(),
            admin: pool_state.admin,
            amount,
            timestamp: current_time,
        });
        
        Ok(())
    }

    /// Execute a trade in the pool
    /// 
    /// # Arguments
    /// * `amount_in` - Amount of tokens to trade
    /// * `minimum_amount_out` - Minimum amount of tokens to receive
    pub fn execute_trade(
        ctx: Context<ExecuteTrade>,
        amount_in: u64,
        minimum_amount_out: u64,
    ) -> Result<()> {
        // Validate reentrancy
        let pool_state = &mut ctx.accounts.pool_state;
        require!(!pool_state.is_locked, ErrorCode::ReentrancyDetected);
        pool_state.is_locked = true;

        // Validate token program
        require!(
            ctx.accounts.token_program.key() == token::ID,
            ErrorCode::InvalidTokenProgram
        );

        // Validate pool state
        require!(!pool_state.is_paused, ErrorCode::PoolPaused);
        require!(pool_state.pool_start_time > 0, ErrorCode::InvalidStateTransition);
        require!(pool_state.total_liquidity > 0, ErrorCode::InvalidStateTransition);
        
        // Validate trade size
        require!(amount_in >= pool_state.min_trade_size, ErrorCode::TradeTooSmall);
        require!(
            amount_in <= pool_state.total_liquidity
                .checked_mul(pool_state.max_trade_size_bps)
                .unwrap()
                .checked_div(10000)
                .unwrap(),
            ErrorCode::TradeTooLarge
        );
        
        // Validate token accounts
        let buyer_token_account = &ctx.accounts.buyer_token_account;
        let pool_token_account = &ctx.accounts.pool_token_account;
        
        require!(
            buyer_token_account.owner == ctx.accounts.buyer.key(),
            ErrorCode::InvalidTokenAccount
        );
        require!(
            pool_token_account.owner == ctx.accounts.pool_authority.key(),
            ErrorCode::InvalidTokenAccount
        );
        require!(
            buyer_token_account.mint == pool_state.token_mint,
            ErrorCode::InvalidTokenMint
        );
        require!(
            pool_token_account.mint == pool_state.token_mint,
            ErrorCode::InvalidTokenMint
        );
        
        // Validate pool authority
        let (pool_authority, bump) = Pubkey::find_program_address(
            &[b"pool_authority", pool_state.key().as_ref()],
            program_id
        );
        require!(
            pool_authority == ctx.accounts.pool_authority.key(),
            ErrorCode::InvalidPoolAuthority
        );
        require!(
            bump == *ctx.bumps.get("pool_authority").unwrap(),
            ErrorCode::InvalidPoolAuthority
        );
        
        // Validate token decimals
        require!(
            ctx.accounts.token_mint.decimals == pool_state.token_decimals,
            ErrorCode::InvalidTokenDecimals
        );
        
        // Validate balances
        require!(
            buyer_token_account.amount >= amount_in,
            ErrorCode::InsufficientBalance
        );
        require!(
            pool_token_account.amount >= minimum_amount_out,
            ErrorCode::InsufficientPoolBalance
        );
        
        // Check cooldown period
        let current_time = ctx.accounts.clock.unix_timestamp as u64;
        require!(
            current_time >= pool_state.last_trade_time + pool_state.cooldown_seconds,
            ErrorCode::SnipeProtectionActive
        );
        
        // Calculate fees with higher precision
        let mut fee_amount = 0;
        if current_time < pool_state.pool_start_time + pool_state.snipe_protection_seconds {
            // Use higher precision for fee calculation
            let fee_numerator = amount_in.checked_mul(pool_state.early_trade_fee_bps).unwrap();
            fee_amount = fee_numerator.checked_div(10000).unwrap();
            if fee_amount == 0 && fee_numerator > 0 {
                fee_amount = 1; // Minimum fee
            }
        }
        
        // Calculate amount after fee
        let amount_after_fee = amount_in.checked_sub(fee_amount).unwrap();
        require!(amount_after_fee >= minimum_amount_out, ErrorCode::SlippageExceeded);
        
        // Update state first (checks-effects-interactions pattern)
        pool_state.last_trade_time = current_time;
        pool_state.last_update = current_time;
        if fee_amount > 0 {
            pool_state.total_fees_collected = pool_state.total_fees_collected
                .checked_add(fee_amount)
                .unwrap();
        }
        
        // Transfer tokens from buyer to pool
        let transfer_in_accounts = token::Transfer {
            from: buyer_token_account.to_account_info(),
            to: pool_token_account.to_account_info(),
            authority: ctx.accounts.buyer.to_account_info(),
        };
        let transfer_in_program = ctx.accounts.token_program.to_account_info();
        token::transfer(
            CpiContext::new(transfer_in_program, transfer_in_accounts),
            amount_in,
        )?;
        
        // Transfer tokens from pool to buyer
        let transfer_out_accounts = token::Transfer {
            from: pool_token_account.to_account_info(),
            to: buyer_token_account.to_account_info(),
            authority: ctx.accounts.pool_authority.to_account_info(),
        };
        let transfer_out_program = ctx.accounts.token_program.to_account_info();
        token::transfer(
            CpiContext::new_with_signer(
                transfer_out_program,
                transfer_out_accounts,
                &[&[
                    b"pool_authority",
                    pool_state.key().as_ref(),
                    &[*ctx.bumps.get("pool_authority").unwrap()],
                ]],
            ),
            amount_after_fee,
        )?;
        
        // Validate balances after transfer
        require!(
            pool_token_account.amount == pool_token_account.amount.checked_add(amount_in).unwrap().checked_sub(amount_after_fee).unwrap(),
            ErrorCode::InvalidBalance
        );
        require!(
            buyer_token_account.amount == buyer_token_account.amount.checked_sub(amount_in).unwrap().checked_add(amount_after_fee).unwrap(),
            ErrorCode::InvalidBalance
        );
        
        // Emit trade executed event
        emit!(TradeExecuted {
            pool: pool_state.key(),
            buyer: ctx.accounts.buyer.key(),
            amount_in,
            amount_out: amount_after_fee,
            fee_amount,
            timestamp: current_time,
        });
        
        // Release reentrancy lock
        pool_state.is_locked = false;

        Ok(())
    }

    pub fn update_pool_parameters(
        ctx: Context<UpdatePoolParameters>,
        early_trade_fee_bps: Option<u64>,
        max_trade_size_bps: Option<u64>,
        min_trade_size: Option<u64>,
        cooldown_seconds: Option<u64>,
        is_paused: Option<bool>,
    ) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        require!(*ctx.accounts.admin.key == pool_state.admin, ErrorCode::Unauthorized);

        if let Some(fee) = early_trade_fee_bps {
            require!(fee <= 1000, ErrorCode::FeeTooHigh);
            pool_state.early_trade_fee_bps = fee;
        }

        if let Some(size) = max_trade_size_bps {
            require!(size <= 1000, ErrorCode::TradeTooLarge);
            pool_state.max_trade_size_bps = size;
        }

        if let Some(size) = min_trade_size {
            pool_state.min_trade_size = size;
        }

        if let Some(cooldown) = cooldown_seconds {
            require!(cooldown <= 3600, ErrorCode::InvalidAmount);
            pool_state.cooldown_seconds = cooldown;
        }

        if let Some(paused) = is_paused {
            pool_state.is_paused = paused;
        }

        Ok(())
    }

    /// Withdraw collected fees from the pool
    pub fn withdraw_fees(ctx: Context<WithdrawFees>) -> Result<()> {
        // Validate admin
        let pool_state = &mut ctx.accounts.pool_state;
        require!(*ctx.accounts.admin.key == pool_state.admin, ErrorCode::Unauthorized);
        
        // Validate fee amount
        require!(pool_state.total_fees_collected > 0, ErrorCode::NoFeesToWithdraw);
        
        // Validate token accounts
        require!(
            ctx.accounts.fee_destination.owner == ctx.accounts.admin.key(),
            ErrorCode::InvalidTokenAccount
        );
        require!(
            ctx.accounts.pool_token_account.owner == ctx.accounts.pool_authority.key(),
            ErrorCode::InvalidTokenAccount
        );
        
        // Get current time
        let current_time = Clock::get()?.unix_timestamp;
        
        // Store fee amount and reset state
        let fee_amount = pool_state.total_fees_collected;
        pool_state.total_fees_collected = 0;
        
        // Transfer fees
        let cpi_accounts = token::Transfer {
            from: ctx.accounts.pool_token_account.to_account_info(),
            to: ctx.accounts.fee_destination.to_account_info(),
            authority: ctx.accounts.admin.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(CpiContext::new(cpi_program, cpi_accounts), fee_amount)?;
        
        // Emit fee withdrawal event
        emit!(FeesWithdrawn {
            pool: pool_state.key(),
            admin: pool_state.admin,
            amount: fee_amount,
            timestamp: current_time,
        });
        
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializePoolProtection<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + 32 + 8 + 8 + 8 + 8 + 8 + 1 + 8 + 8 + 8 + 8 + 32 + 8,
        seeds = [b"pool_state", admin.key().as_ref()],
        bump
    )]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
    /// CHECK: Validated in instruction
    pub token_mint: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(
        mut,
        seeds = [b"pool_state", admin.key().as_ref()],
        bump,
        has_one = admin,
        has_one = token_mint @ ErrorCode::InvalidTokenMint
    )]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        mut,
        has_one = mint,
        constraint = admin_token_account.owner == admin.key() @ ErrorCode::InvalidTokenAccount
    )]
    pub admin_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        has_one = mint,
        constraint = pool_token_account.owner == pool_authority.key() @ ErrorCode::InvalidTokenAccount
    )]
    pub pool_token_account: Account<'info, TokenAccount>,
    pub token_mint: Account<'info, Mint>,
    /// CHECK: Validated in instruction
    #[account(
        seeds = [b"pool_authority", pool_state.key().as_ref()],
        bump,
        constraint = pool_authority.key() == Pubkey::find_program_address(
            &[b"pool_authority", pool_state.key().as_ref()],
            program_id
        ).0 @ ErrorCode::InvalidPoolAuthority
    )]
    pub pool_authority: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct ExecuteTrade<'info> {
    #[account(
        mut,
        seeds = [b"pool_state", pool_state.admin.as_ref()],
        bump,
        has_one = token_mint @ ErrorCode::InvalidTokenMint
    )]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub buyer: Signer<'info>,
    #[account(
        mut,
        has_one = mint,
        constraint = buyer_token_account.owner == buyer.key() @ ErrorCode::InvalidTokenAccount,
        constraint = buyer_token_account.mint == pool_state.token_mint @ ErrorCode::InvalidTokenMint
    )]
    pub buyer_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        has_one = mint,
        constraint = pool_token_account.owner == pool_authority.key() @ ErrorCode::InvalidTokenAccount,
        constraint = pool_token_account.mint == pool_state.token_mint @ ErrorCode::InvalidTokenMint
    )]
    pub pool_token_account: Account<'info, TokenAccount>,
    pub token_mint: Account<'info, Mint>,
    /// CHECK: Validated in instruction
    #[account(
        seeds = [b"pool_authority", pool_state.key().as_ref()],
        bump,
        constraint = pool_authority.key() == Pubkey::find_program_address(
            &[b"pool_authority", pool_state.key().as_ref()],
            program_id
        ).0 @ ErrorCode::InvalidPoolAuthority
    )]
    pub pool_authority: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct UpdatePoolParameters<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct WithdrawFees<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut)]
    pub fee_destination: Account<'info, TokenAccount>,
    #[account(mut)]
    pub pool_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[account]
pub struct PoolState {
    pub admin: Pubkey,
    pub token_mint: Pubkey,
    pub token_decimals: u8,        // Added token decimals
    pub snipe_protection_seconds: u64,
    pub early_trade_fee_bps: u64,
    pub pool_start_time: i64,
    pub total_fees_collected: u64,
    pub total_liquidity: u64,
    pub is_paused: bool,
    pub max_trade_size_bps: u64,
    pub min_trade_size: u64,
    pub cooldown_seconds: u64,
    pub last_trade_time: i64,
    pub version: u8,
    pub last_update: i64,
    pub is_locked: bool,           // Added reentrancy guard
}

// Add events for important state changes
#[event]
pub struct PoolInitialized {
    pub admin: Pubkey,
    pub token_mint: Pubkey,
    pub snipe_protection_seconds: u64,
    pub early_trade_fee_bps: u64,
    pub max_trade_size_bps: u64,
    pub min_trade_size: u64,
    pub cooldown_seconds: u64,
    pub timestamp: i64,
}

#[event]
pub struct LiquidityAdded {
    pub pool: Pubkey,
    pub admin: Pubkey,
    pub amount: u64,
}

#[event]
pub struct TradeExecuted {
    pub pool: Pubkey,
    pub buyer: Pubkey,
    pub amount_in: u64,
    pub amount_out: u64,
    pub fee_amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct FeesWithdrawn {
    pub pool: Pubkey,
    pub admin: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Snipe protection is still active")]
    SnipeProtectionActive,
    #[msg("Pool has already started")]
    PoolAlreadyStarted,
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Fee too high")]
    FeeTooHigh,
    #[msg("Trade too large")]
    TradeTooLarge,
    #[msg("Trade too small")]
    TradeTooSmall,
    #[msg("Insufficient pool balance")]
    InsufficientPoolBalance,
    #[msg("Invalid pool authority")]
    InvalidPoolAuthority,
    #[msg("Invalid token account")]
    InvalidTokenAccount,
    #[msg("Pool is paused")]
    PoolPaused,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Slippage exceeded")]
    SlippageExceeded,
    #[msg("No fees to withdraw")]
    NoFeesToWithdraw,
    #[msg("Invalid token mint")]
    InvalidTokenMint,
    #[msg("Token account balance too low")]
    InsufficientBalance,
    #[msg("Invalid state transition")]
    InvalidStateTransition,
    #[msg("Invalid token program")]
    InvalidTokenProgram,
    #[msg("Invalid parameter relationship")]
    InvalidParameterRelationship,
    #[msg("Invalid system program")]
    InvalidSystemProgram,
    #[msg("Insufficient funds for rent")]
    InsufficientFunds,
    #[msg("Invalid token decimals")]
    InvalidTokenDecimals,
    #[msg("Invalid balance after transfer")]
    InvalidBalance,
    #[msg("Reentrancy detected")]
    ReentrancyDetected,
}