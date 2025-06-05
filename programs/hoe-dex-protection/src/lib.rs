use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use anchor_spl::token::{self, Mint, Token, TokenAccount};

declare_id!("YourProgramIdHere"); // Replace with your program ID after deployment

#[program]
pub mod hoe_coin {
    use super::*;

    pub fn initialize_pool_protection(
        ctx: Context<InitializePoolProtection>,
        snipe_protection_seconds: u64,
        early_trade_fee_bps: u64,
    ) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        pool_state.admin = *ctx.accounts.admin.key;
        pool_state.snipe_protection_seconds = snipe_protection_seconds; // Delay before trading opens
        pool_state.early_trade_fee_bps = early_trade_fee_bps; // Fee for early trades (in basis points)
        pool_state.pool_start_time = 0; // Will be set on first liquidity add
        pool_state.total_fees_collected = 0;
        Ok(())
    }

    pub fn add_liquidity(ctx: Context<AddLiquidity>, amount: u64) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        require!(pool_state.pool_start_time == 0, ErrorCode::PoolAlreadyStarted);

        // Set pool start time on first liquidity addition
        let current_time = Clock::get()?.unix_timestamp;
        pool_state.pool_start_time = current_time;

        // Transfer tokens to the pool's token account
        let cpi_accounts = token::Transfer {
            from: ctx.accounts.admin_token_account.to_account_info(),
            to: ctx.accounts.pool_token_account.to_account_info(),
            authority: ctx.accounts.admin.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(CpiContext::new(cpi_program, cpi_accounts), amount)?;

        Ok(())
    }

    pub fn trade(ctx: Context<Trade>, amount: u64) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp;
        let time_since_start = current_time - pool_state.pool_start_time;

        // Enforce snipe protection
        require!(
            time_since_start >= pool_state.snipe_protection_seconds as i64,
            ErrorCode::SnipeProtectionActive
        );

        // Apply early trade fee if within the early window (e.g., first 5 minutes)
        let mut fee = 0;
        if time_since_start < 300 { // 5 minutes in seconds
            fee = (amount * pool_state.early_trade_fee_bps) / 10_000; // Calculate fee in basis points
            pool_state.total_fees_collected += fee;
        }

        let amount_after_fee = amount - fee;

        // Simulate a trade (transfer tokens to buyer, simplified for this example)
        let cpi_accounts = token::Transfer {
            from: ctx.accounts.pool_token_account.to_account_info(),
            to: ctx.accounts.buyer_token_account.to_account_info(),
            authority: ctx.accounts.pool_authority.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(CpiContext::new(cpi_program, cpi_accounts), amount_after_fee)?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializePoolProtection<'info> {
    #[account(init, payer = admin, space = 8 + 32 + 8 + 8 + 8 + 8)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut)]
    pub admin_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub pool_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct Trade<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub buyer: Signer<'info>,
    #[account(mut)]
    pub pool_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub buyer_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    #[account(mut)]
    pub pool_authority: AccountInfo<'info>,
    pub clock: Sysvar<'info, Clock>,
}

#[account]
pub struct PoolState {
    pub admin: Pubkey,
    pub snipe_protection_seconds: u64,
    pub early_trade_fee_bps: u64,
    pub pool_start_time: i64,
    pub total_fees_collected: u64,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Snipe protection is still active")]
    SnipeProtectionActive,
    #[msg("Pool has already started")]
    PoolAlreadyStarted,
}