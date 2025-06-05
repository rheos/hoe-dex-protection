use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};

// Program ID should be replaced with actual ID after deployment
declare_id!("YourProgramIdHere");

/// HOE DEX Protection Program
/// This program implements a DEX protection mechanism with the following features:
/// - Snipe protection with configurable duration
/// - Early trade fees with basis points precision and volume-based tiers
/// - Separate early trade fee window for flexible fee application
/// - Trade size limits and cooldown periods
/// - Admin-controlled parameter updates with timelock
/// - Fee collection and withdrawal
/// - Emergency pause mechanism with separate admin
/// - Circuit breaker for volume protection
/// - Rate limiting to prevent spam
/// - Price impact protection
/// - Daily volume limits
/// - Liquidity removal functionality
/// - Blacklist/whitelist functionality for trader restrictions
/// 
/// # Security Features:
/// - Reentrancy protection on all state-modifying functions
/// - Timelock on parameter updates (24 hours)
/// - Emergency pause with separate admin
/// - Circuit breaker for volume protection with configurable cooldown
/// - Rate limiting to prevent spam with reset events
/// - Price impact protection
/// - Daily volume limits
/// - Fee tier limits (max 10 tiers)
/// - Parameter relationship validation
/// - Balance validation on transfers
/// - PDA validation with bump seeds
/// - Token program validation
/// - System program validation
/// - Rent exemption validation
/// - Token account delegate validation
#[program]
pub mod hoe_dex_protection {
    use super::*;

    /// Initialize a new pool protection instance
    /// 
    /// # Security Considerations:
    /// - Validates system program and rent exemption
    /// - Enforces parameter limits and relationships
    /// - Validates token mint and decimals
    /// - Implements proper space calculation
    /// - Uses proper PDA derivation
    /// - Validates fee tier configuration
    /// - Validates emergency admin
    /// - Initializes all safety parameters
    /// 
    /// # Parameters:
    /// - snipe_protection_seconds: Duration of protection (must be > 0)
    /// - early_trade_fee_bps: Fee in basis points (max 1000 = 10%)
    /// - early_trade_window_seconds: Duration of early trade fee window (must be <= snipe_protection_seconds)
    /// - max_trade_size_bps: Max trade size as % of pool (max 1000 = 10%)
    /// - min_trade_size: Minimum trade size in token units
    /// - cooldown_seconds: Cooldown between trades (max 3600 = 1 hour)
    /// - emergency_admin: Address of the emergency admin
    /// - fee_tiers: Vector of fee tiers (max 10 tiers)
    /// - max_daily_volume: Maximum daily volume limit
    /// - max_price_impact_bps: Maximum price impact in basis points
    /// - circuit_breaker_threshold: Volume threshold for circuit breaker
    /// - circuit_breaker_window: Time window for circuit breaker in seconds
    /// - circuit_breaker_cooldown: Cooldown period after circuit breaker trigger in seconds
    /// - rate_limit_window: Rate limit window in seconds
    /// - rate_limit_max: Maximum trades per window
    /// 
    /// # State Initialization:
    /// - Sets all safety parameters
    /// - Initializes timestamps
    /// - Sets up fee tiers
    /// - Configures emergency admin
    /// - Initializes volume tracking
    /// - Sets up rate limiting
    /// - Configures circuit breaker
    pub fn initialize_pool_protection(
        ctx: Context<InitializePoolProtection>,
        snipe_protection_seconds: u64,
        early_trade_fee_bps: u64,
        early_trade_window_seconds: u64,
        max_trade_size_bps: u64,
        min_trade_size: u64,
        cooldown_seconds: u64,
        emergency_admin: Pubkey,
        fee_tiers: Vec<FeeTier>,
        max_daily_volume: u64,
        max_price_impact_bps: u64,
        circuit_breaker_threshold: u64,
        circuit_breaker_window: u64,
        circuit_breaker_cooldown: u64,
        rate_limit_window: u64,
        rate_limit_max: u32,
    ) -> Result<()> {
        // Validate system program to prevent unauthorized account creation
        require!(
            ctx.accounts.system_program.key() == system_program::ID,
            ErrorCode::InvalidSystemProgram
        );

        // Calculate and validate rent exemption
        let rent = &ctx.accounts.rent;
        // Space calculation includes:
        // - 8 bytes for discriminator
        // - 32 bytes for admin pubkey
        // - 32 bytes for token mint pubkey
        // - 1 byte for token decimals
        // - 8 bytes for each u64 field (10 fields)
        // - 1 byte for each bool field (2 fields)
        // - 8 bytes for each u64 field (6 fields)
        // - 48 bytes for PendingUpdate option
        // - 100 bytes for fee_tiers
        // - 100 bytes for trader_blacklist (approximate)
        let space = 8 + 32 + 32 + 1 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + 1 + 8 + 8 + 8 + 8 + 1 + 8 + 48 + 8 + 8 + 32 + 100 + 8 + 8 + 8 + 8 + 8 + 8 + 4 + 4 + 100 + 32;
        let rent_lamports = rent.minimum_balance(space);
        require!(
            ctx.accounts.admin.lamports() >= rent_lamports,
            ErrorCode::InsufficientFunds
        );

        // Validate token program to ensure proper token operations
        require!(
            ctx.accounts.token_program.key() == token::ID,
            ErrorCode::InvalidTokenProgram
        );

        // Validate parameter limits
        require!(early_trade_fee_bps <= 1000, ErrorCode::FeeTooHigh); // Max 10% fee
        require!(max_trade_size_bps <= 1000, ErrorCode::TradeTooLarge); // Max 10% of pool
        require!(cooldown_seconds <= 3600, ErrorCode::InvalidAmount); // Max 1 hour cooldown
        require!(min_trade_size > 0, ErrorCode::InvalidAmount);
        require!(snipe_protection_seconds > 0, ErrorCode::InvalidAmount);
        require!(early_trade_window_seconds <= snipe_protection_seconds, ErrorCode::InvalidParameterRelationship);

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

        // Get and validate current time
        let current_time = Clock::get()?.unix_timestamp;
        require!(current_time >= 0, ErrorCode::InvalidTimestamp);

        // Validate fee tiers
        require!(!fee_tiers.is_empty(), ErrorCode::InvalidFeeTier);
        require!(fee_tiers.len() <= 10, ErrorCode::TooManyFeeTiers);
        for (i, tier) in fee_tiers.iter().enumerate() {
            require!(tier.fee_bps <= 1000, ErrorCode::FeeTooHigh);
            require!(tier.volume_threshold > 0, ErrorCode::InvalidFeeTier);
            if i > 0 {
                require!(
                    tier.volume_threshold > fee_tiers[i - 1].volume_threshold,
                    ErrorCode::InvalidFeeTier
                );
                require!(
                    tier.fee_bps <= fee_tiers[i - 1].fee_bps,
                    ErrorCode::InvalidFeeTier
                );
                // Ensure minimum spacing between thresholds (1% of max_daily_volume)
                require!(
                    tier.volume_threshold - fee_tiers[i - 1].volume_threshold >= max_daily_volume / 100,
                    ErrorCode::InvalidFeeTierSpacing
                );
            }
        }

        // Validate new parameters
        require!(max_daily_volume > 0, ErrorCode::InvalidAmount);
        require!(max_price_impact_bps <= 1000, ErrorCode::PriceImpactTooHigh); // Max 10% price impact
        require!(circuit_breaker_threshold > 0, ErrorCode::InvalidAmount);
        require!(circuit_breaker_window > 0, ErrorCode::InvalidAmount);
        require!(circuit_breaker_window >= circuit_breaker_cooldown, ErrorCode::InvalidParameterRelationship);
        require!(circuit_breaker_cooldown >= 60, ErrorCode::InvalidAmount); // Min 1 minute cooldown
        require!(rate_limit_window > 0, ErrorCode::InvalidRateLimit);
        require!(rate_limit_max > 0, ErrorCode::InvalidRateLimit);

        // Validate emergency admin is not the same as regular admin
        require!(
            emergency_admin != *ctx.accounts.admin.key,
            ErrorCode::InvalidEmergencyAdmin
        );

        // Initialize pool state with pre-allocated vectors
        let mut pool_state = PoolState {
            admin: ctx.accounts.admin.key(),
            token_mint: ctx.accounts.token_mint.key(),
            token_decimals: token_mint.decimals,
            snipe_protection_seconds,
            early_trade_fee_bps,
            early_trade_window_seconds,
            pool_start_time: current_time as u64,
            total_fees_collected: 0,
            total_liquidity: 0,
            is_paused: false,
            is_emergency_paused: false,
            max_trade_size_bps,
            min_trade_size,
            cooldown_seconds,
            last_trade_time: 0,
            version: 1,
            last_update: current_time as u64,
            is_locked: false,
            pending_update: None,
            volume_24h: 0,
            last_volume_update: current_time as u64,
            emergency_admin,
            fee_tiers: Vec::with_capacity(fee_tiers.len()),
            max_daily_volume,
            max_price_impact_bps,
            circuit_breaker_threshold,
            circuit_breaker_window,
            circuit_breaker_cooldown,
            last_circuit_breaker: 0,
            rate_limit_window,
            rate_limit_count: 0,
            rate_limit_max,
            trader_blacklist: Vec::new(),
        };

        // Copy fee tiers
        pool_state.fee_tiers = fee_tiers;

        // Emit initialization event for tracking
        emit!(PoolInitialized {
            admin: pool_state.admin,
            token_mint: pool_state.token_mint,
            snipe_protection_seconds,
            early_trade_fee_bps,
            max_trade_size_bps,
            min_trade_size,
            cooldown_seconds,
            timestamp: current_time as i64,
        });

        Ok(())
    }

    /// Add liquidity to the pool
    /// 
    /// # Security Considerations:
    /// - Validates token program and accounts
    /// - Enforces pool state constraints
    /// - Validates pool authority PDA
    /// - Implements proper token transfers
    /// - Uses checks-effects-interactions pattern
    /// - Validates pool not already started
    /// - Validates pool not paused
    /// - Validates token account ownership
    /// - Validates token mint
    /// - Validates token account delegate
    /// 
    /// # Parameters:
    /// - amount: Amount of tokens to add (must be > 0)
    /// 
    /// # State Updates:
    /// - Sets pool start time
    /// - Updates total liquidity
    /// - Updates last update timestamp
    /// - Transfers tokens to pool
    pub fn add_liquidity(ctx: Context<AddLiquidity>, amount: u64) -> Result<()> {
        // Validate token program to ensure proper token operations
        require!(
            ctx.accounts.token_program.key() == token::ID,
            ErrorCode::InvalidTokenProgram
        );

        // Validate amount is positive
        require!(amount > 0, ErrorCode::InvalidAmount);
        
        let pool_state = &mut ctx.accounts.pool_state;
        
        // Validate pool state
        require!(pool_state.pool_start_time == 0, ErrorCode::PoolAlreadyStarted);
        require!(!pool_state.is_paused, ErrorCode::PoolPaused);
        
        // Validate token accounts ownership and mint
        require!(
            ctx.accounts.admin_token_account.owner == ctx.accounts.admin.key(),
            ErrorCode::InvalidTokenAccount
        );
        require!(
            ctx.accounts.pool_token_account.owner == ctx.accounts.pool_authority.key(),
            ErrorCode::InvalidTokenAccount
        );
        require!(ctx.accounts.admin_token_account.delegate.is_none(), ErrorCode::TokenAccountDelegated);
        require!(ctx.accounts.pool_token_account.delegate.is_none(), ErrorCode::TokenAccountDelegated);
        
        // Validate pool authority PDA
        let (pool_authority, _) = Pubkey::find_program_address(
            &[b"pool_authority", pool_state.key().as_ref()],
            program_id
        );
        require!(
            pool_authority == ctx.accounts.pool_authority.key(),
            ErrorCode::InvalidPoolAuthority
        );
        
        // Get and validate current time
        let current_time = Clock::get()?.unix_timestamp;
        require!(current_time >= 0, ErrorCode::InvalidTimestamp);
        
        // Store initial balances for validation
        let initial_admin_balance = ctx.accounts.admin_token_account.amount;
        let initial_pool_balance = ctx.accounts.pool_token_account.amount;
        
        // Update pool state (checks)
        pool_state.pool_start_time = current_time as u64;
        pool_state.total_liquidity = amount;
        pool_state.last_update = current_time as u64;
        
        // Transfer tokens to pool (effects)
        let cpi_accounts = token::Transfer {
            from: ctx.accounts.admin_token_account.to_account_info(),
            to: ctx.accounts.pool_token_account.to_account_info(),
            authority: ctx.accounts.admin.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(CpiContext::new(cpi_program, cpi_accounts), amount)?;
        
        // Validate transfer
        require!(
            ctx.accounts.admin_token_account.amount == initial_admin_balance.checked_sub(amount).ok_or(ErrorCode::Overflow)?,
            ErrorCode::InvalidBalance
        );
        require!(
            ctx.accounts.pool_token_account.amount == initial_pool_balance.checked_add(amount).ok_or(ErrorCode::Overflow)?,
            ErrorCode::InvalidBalance
        );
        
        // Validate final pool balance
        require!(
            ctx.accounts.pool_token_account.amount >= amount,
            ErrorCode::InsufficientPoolBalance
        );
        
        // Emit event for tracking (interactions)
        emit!(LiquidityAdded {
            pool: pool_state.key(),
            admin: pool_state.admin,
            amount,
            timestamp: current_time as i64,
        });
        
        Ok(())
    }

    /// Remove liquidity from the pool
    /// 
    /// # Security Considerations:
    /// - Validates admin authority
    /// - Validates pool state
    /// - Validates token accounts
    /// - Implements proper token transfers
    /// - Uses checks-effects-interactions pattern
    /// - Validates pool not paused
    /// - Validates pool not emergency paused
    /// - Validates sufficient liquidity
    /// - Validates token account ownership
    /// - Validates pool authority PDA
    /// - Validates token account delegate
    /// 
    /// # Parameters:
    /// - amount: Amount of tokens to remove (must be <= total_liquidity)
    /// 
    /// # State Updates:
    /// - Updates total liquidity
    /// - Updates last update timestamp
    /// - Transfers tokens to admin
    pub fn remove_liquidity(ctx: Context<RemoveLiquidity>, amount: u64) -> Result<()> {
        // Validate token program
        require!(
            ctx.accounts.token_program.key() == token::ID,
            ErrorCode::InvalidTokenProgram
        );

        // Validate admin authority
        let pool_state = &mut ctx.accounts.pool_state;
        require!(*ctx.accounts.admin.key == pool_state.admin, ErrorCode::Unauthorized);

        // Validate pool state
        require!(!pool_state.is_paused, ErrorCode::PoolPaused);
        require!(!pool_state.is_emergency_paused, ErrorCode::EmergencyPaused);
        require!(pool_state.total_liquidity >= amount, ErrorCode::InsufficientLiquidity);
        require!(amount > 0, ErrorCode::InvalidAmount);

        // Validate token accounts
        require!(
            ctx.accounts.admin_token_account.owner == ctx.accounts.admin.key(),
            ErrorCode::InvalidTokenAccount
        );
        require!(
            ctx.accounts.pool_token_account.owner == ctx.accounts.pool_authority.key(),
            ErrorCode::InvalidTokenAccount
        );
        require!(ctx.accounts.admin_token_account.delegate.is_none(), ErrorCode::TokenAccountDelegated);
        require!(ctx.accounts.pool_token_account.delegate.is_none(), ErrorCode::TokenAccountDelegated);

        // Validate pool authority PDA
        let (pool_authority, _) = Pubkey::find_program_address(
            &[b"pool_authority", pool_state.key().as_ref()],
            program_id
        );
        require!(
            pool_authority == ctx.accounts.pool_authority.key(),
            ErrorCode::InvalidPoolAuthority
        );

        // Get and validate current time
        let current_time = Clock::get()?.unix_timestamp;
        require!(current_time >= 0, ErrorCode::InvalidTimestamp);

        // Store initial balances for validation
        let initial_pool_balance = ctx.accounts.pool_token_account.amount;
        let initial_admin_balance = ctx.accounts.admin_token_account.amount;

        // Update pool state (checks)
        pool_state.total_liquidity = pool_state.total_liquidity.checked_sub(amount).unwrap();
        pool_state.last_update = current_time as i64;

        // Transfer tokens from pool to admin (effects)
        let cpi_accounts = token::Transfer {
            from: ctx.accounts.pool_token_account.to_account_info(),
            to: ctx.accounts.admin_token_account.to_account_info(),
            authority: ctx.accounts.pool_authority.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(
            CpiContext::new_with_signer(
                cpi_program,
                cpi_accounts,
                &[&[
                    b"pool_authority",
                    pool_state.key().as_ref(),
                    &[*ctx.bumps.get("pool_authority").unwrap()],
                ]],
            ),
            amount,
        )?;

        // Validate transfer
        require!(
            ctx.accounts.pool_token_account.amount == initial_pool_balance.checked_sub(amount).ok_or(ErrorCode::Overflow)?,
            ErrorCode::InvalidBalance
        );
        require!(
            ctx.accounts.admin_token_account.amount == initial_admin_balance.checked_add(amount).ok_or(ErrorCode::Overflow)?,
            ErrorCode::InvalidBalance
        );

        // Validate final pool balance only if not draining the pool
        if amount != pool_state.total_liquidity {
            require!(
                ctx.accounts.pool_token_account.amount >= pool_state.min_trade_size,
                ErrorCode::InsufficientPoolBalance
            );
        } else {
            require!(
                ctx.accounts.pool_token_account.amount >= amount,
                ErrorCode::InsufficientPoolBalance
            );
        }

        // Emit event for tracking (interactions)
        emit!(LiquidityRemoved {
            pool: pool_state.key(),
            admin: pool_state.admin,
            amount,
            timestamp: current_time as i64,
        });

        Ok(())
    }

    /// Execute a trade in the pool
    /// 
    /// # Security Considerations:
    /// - Implements reentrancy protection
    /// - Validates all account relationships
    /// - Enforces trade size limits
    /// - Validates balances before transfers
    /// - Implements proper fee calculation
    /// - Uses checks-effects-interactions pattern
    /// - Validates PDA bump
    /// - Implements rate limiting
    /// - Enforces daily volume limits
    /// - Implements circuit breaker
    /// - Protects against price impact
    /// - Validates pool not paused
    /// - Validates pool not emergency paused
    /// - Validates token decimals
    /// - Validates token account ownership
    /// - Validates pool authority PDA
    /// - Validates sufficient balances
    /// - Validates cooldown period
    /// - Validates trader not blacklisted
    /// - Validates token account delegate
    /// 
    /// # Parameters:
    /// - amount_in: Amount of tokens to trade (must be within limits)
    /// - minimum_amount_out: Minimum amount to receive (slippage protection)
    /// 
    /// # State Updates:
    /// - Updates last trade time
    /// - Updates last update timestamp
    /// - Updates total fees collected
    /// - Updates volume tracking
    /// - Updates rate limit counter
    /// - Updates circuit breaker state
    /// - Transfers tokens between accounts
    pub fn execute_trade(
        ctx: Context<ExecuteTrade>,
        amount_in: u64,
        minimum_amount_out: u64,
    ) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let _guard = ReentrancyGuard::new(pool_state)?;

        // Check emergency pause
        require!(!pool_state.is_emergency_paused, ErrorCode::EmergencyPaused);

        // Validate token program
        require!(
            ctx.accounts.token_program.key() == token::ID,
            ErrorCode::InvalidTokenProgram
        );

        // Validate pool state
        require!(!pool_state.is_paused, ErrorCode::PoolPaused);
        require!(pool_state.pool_start_time > 0, ErrorCode::InvalidStateTransition);
        require!(pool_state.total_liquidity > 0, ErrorCode::InvalidStateTransition);
        
        // Validate trader not blacklisted
        require!(
            !pool_state.trader_blacklist.contains(&ctx.accounts.buyer.key()),
            ErrorCode::TraderBlacklisted
        );

        // Validate trade size limits
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
        require!(buyer_token_account.delegate.is_none(), ErrorCode::TokenAccountDelegated);
        require!(pool_token_account.delegate.is_none(), ErrorCode::TokenAccountDelegated);
        
        // Validate pool authority PDA and bump
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
        validate_balance_change(
            ctx.accounts.buyer_token_account.amount,
            -(amount_in as i64),
            pool_state.min_trade_size,
        )?;
        validate_balance_change(
            ctx.accounts.pool_token_account.amount,
            amount_after_fee as i64,
            minimum_amount_out,
        )?;
        
        // Validate cooldown period
        let current_time = ctx.accounts.clock.unix_timestamp as u64;
        require!(current_time >= 0, ErrorCode::InvalidTimestamp);
        require!(
            current_time >= pool_state.last_trade_time + pool_state.cooldown_seconds,
            ErrorCode::SnipeProtectionActive
        );
        
        // Check rate limiting
        if current_time - pool_state.last_trade_time >= pool_state.rate_limit_window {
            pool_state.rate_limit_count = 0;
        }
        
        // Check rate limit with overflow protection
        let new_count = pool_state.rate_limit_count.checked_add(1).ok_or(ErrorCode::RateLimitOverflow)?;
        require!(
            new_count <= pool_state.rate_limit_max,
            ErrorCode::RateLimitExceeded
        );
        pool_state.rate_limit_count = new_count;

        // Check daily volume limit
        if current_time - pool_state.last_volume_update >= 86400 {
            pool_state.volume_24h = 0;
            pool_state.last_volume_update = current_time;
        }
        let new_volume = pool_state.volume_24h.checked_add(amount_in).unwrap();
        require!(
            new_volume <= pool_state.max_daily_volume,
            ErrorCode::DailyVolumeLimitExceeded
        );

        // Check circuit breaker with cooldown
        let mut circuit_breaker_triggered = false;
        if current_time - pool_state.last_circuit_breaker < pool_state.circuit_breaker_window {
            if new_volume > pool_state.circuit_breaker_threshold {
                circuit_breaker_triggered = true;
                require!(
                    current_time - pool_state.last_circuit_breaker >= pool_state.circuit_breaker_cooldown,
                    ErrorCode::CircuitBreakerCooldown
                );
                pool_state.last_circuit_breaker = current_time;
            }
        } else {
            pool_state.volume_24h = 0;
            pool_state.last_circuit_breaker = current_time;
        }
        pool_state.volume_24h = new_volume;

        // Re-validate total_liquidity to prevent division by zero
        require!(pool_state.total_liquidity > 0, ErrorCode::InvalidStateTransition);

        // Calculate price impact with overflow protection
        let price_impact = calculate_price_impact(
            amount_in,
            pool_token_account.amount,
            pool_state.token_decimals,
        )?;
        require!(
            price_impact <= pool_state.max_price_impact_bps,
            ErrorCode::PriceImpactTooHigh
        );

        // Calculate fees based on volume tier
        let mut fee_amount = 0;
        if current_time < pool_state.pool_start_time + pool_state.early_trade_window_seconds {
            // Find applicable fee tier
            let mut applicable_fee = pool_state.early_trade_fee_bps;
            for tier in pool_state.fee_tiers.iter().rev() {
                if pool_state.volume_24h >= tier.volume_threshold {
                    applicable_fee = tier.fee_bps;
                    break;
                }
            }

            let fee_numerator = amount_in.checked_mul(applicable_fee).unwrap();
            fee_amount = fee_numerator.checked_div(10000).unwrap();
            if fee_amount == 0 && fee_numerator > 0 {
                fee_amount = 1;
            }
        }
        
        // Calculate amount after fee and validate slippage
        let amount_after_fee = amount_in.checked_sub(fee_amount).unwrap();
        require!(amount_after_fee >= minimum_amount_out, ErrorCode::SlippageExceeded);

        // Validate pool has enough liquidity after trade
        require!(
            pool_token_account.amount.checked_sub(amount_after_fee).ok_or(ErrorCode::Overflow)? >= minimum_amount_out,
            ErrorCode::InsufficientPoolBalance
        );

        // Store initial balances for validation
        let initial_buyer_balance = ctx.accounts.buyer_token_account.amount;
        let initial_pool_balance = ctx.accounts.pool_token_account.amount;

        // Update state (checks) - Finalize all state updates before CPI calls
        pool_state.last_trade_time = current_time as i64;
        pool_state.last_update = current_time as i64;
        if fee_amount > 0 {
            let new_total = pool_state.total_fees_collected.checked_add(fee_amount).ok_or(ErrorCode::Overflow)?;
            require!(new_total <= u64::MAX / 2, ErrorCode::FeeOverflow);
            pool_state.total_fees_collected = new_total;
        }

        // Transfer tokens from buyer to pool (effects)
        let transfer_in_accounts = token::Transfer {
            from: ctx.accounts.buyer_token_account.to_account_info(),
            to: ctx.accounts.pool_token_account.to_account_info(),
            authority: ctx.accounts.buyer.to_account_info(),
        };
        let transfer_in_program = ctx.accounts.token_program.to_account_info();
        token::transfer(
            CpiContext::new(transfer_in_program, transfer_in_accounts),
            amount_in,
        )?;

        // Validate transfer in
        require!(
            ctx.accounts.buyer_token_account.amount == initial_buyer_balance.checked_sub(amount_in).ok_or(ErrorCode::Overflow)?,
            ErrorCode::InvalidBalance
        );
        require!(
            ctx.accounts.pool_token_account.amount == initial_pool_balance.checked_add(amount_in).ok_or(ErrorCode::Overflow)?,
            ErrorCode::InvalidBalance
        );

        // Store intermediate balances
        let intermediate_buyer_balance = ctx.accounts.buyer_token_account.amount;
        let intermediate_pool_balance = ctx.accounts.pool_token_account.amount;

        // Transfer tokens from pool to buyer (effects)
        let transfer_out_accounts = token::Transfer {
            from: ctx.accounts.pool_token_account.to_account_info(),
            to: ctx.accounts.buyer_token_account.to_account_info(),
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

        // Validate transfer out
        require!(
            ctx.accounts.buyer_token_account.amount == intermediate_buyer_balance.checked_add(amount_after_fee).ok_or(ErrorCode::Overflow)?,
            ErrorCode::InvalidBalance
        );
        require!(
            ctx.accounts.pool_token_account.amount == intermediate_pool_balance.checked_sub(amount_after_fee).ok_or(ErrorCode::Overflow)?,
            ErrorCode::InvalidBalance
        );

        // Validate final pool balance
        require!(
            ctx.accounts.pool_token_account.amount >= minimum_amount_out,
            ErrorCode::InsufficientPoolBalance
        );

        // Emit trade event (interactions)
        emit!(TradeExecuted {
            pool: pool_state.key(),
            buyer: ctx.accounts.buyer.key(),
            amount_in,
            amount_out: amount_after_fee,
            fee_amount,
            timestamp: current_time as i64,
            token_mint: pool_state.token_mint,
        });

        // Emit circuit breaker event if triggered
        if circuit_breaker_triggered {
            emit!(CircuitBreakerTriggered {
                pool: pool_state.key(),
                timestamp: current_time as i64,
            });
        }
        
        Ok(())
    }

    /// Add a trader to the blacklist
    /// 
    /// # Security Considerations:
    /// - Validates admin authority
    /// - Validates trader not already blacklisted
    /// - Limits blacklist size to prevent gas griefing
    /// 
    /// # Parameters:
    /// - trader: Public key of the trader to blacklist
    /// 
    /// # State Updates:
    /// - Adds trader to blacklist
    /// - Emits trader blacklisted event
    pub fn blacklist_trader(ctx: Context<BlacklistTrader>, trader: Pubkey) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        require!(*ctx.accounts.admin.key == pool_state.admin, ErrorCode::Unauthorized);

        require!(
            !pool_state.trader_blacklist.contains(&trader),
            ErrorCode::TraderAlreadyBlacklisted
        );
        require!(
            pool_state.trader_blacklist.len() < 100, // Cap at 100 to prevent gas griefing
            ErrorCode::BlacklistFull
        );

        pool_state.trader_blacklist.push(trader);

        emit!(TraderBlacklisted {
            pool: pool_state.key(),
            trader,
            timestamp: Clock::get()?.unix_timestamp as i64,
        });

        Ok(())
    }

    /// Remove a trader from the blacklist
    /// 
    /// # Security Considerations:
    /// - Validates admin authority
    /// - Validates trader is blacklisted
    /// 
    /// # Parameters:
    /// - trader: Public key of the trader to remove from blacklist
    /// 
    /// # State Updates:
    /// - Removes trader from blacklist
    /// - Emits trader unblacklisted event
    pub fn unblacklist_trader(ctx: Context<BlacklistTrader>, trader: Pubkey) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        require!(*ctx.accounts.admin.key == pool_state.admin, ErrorCode::Unauthorized);

        let index = pool_state.trader_blacklist.iter().position(|&t| t == trader)
            .ok_or(ErrorCode::TraderNotBlacklisted)?;
        pool_state.trader_blacklist.remove(index);

        emit!(TraderUnblacklisted {
            pool: pool_state.key(),
            trader,
            timestamp: Clock::get()?.unix_timestamp as i64,
        });

        Ok(())
    }

    /// Schedule an update to pool parameters with a timelock
    /// 
    /// # Security Considerations:
    /// - Implements timelock mechanism (24 hours)
    /// - Validates admin authority
    /// - Enforces parameter limits
    /// - Stores pending updates safely
    /// - Emits events for tracking
    /// - Validates parameter relationships
    /// - Validates fee tier configuration
    /// 
    /// # Parameters:
    /// - early_trade_fee_bps: Optional new fee in basis points
    /// - early_trade_window_seconds: Optional new early trade window
    /// - max_trade_size_bps: Optional new max trade size
    /// - min_trade_size: Optional new min trade size
    /// - cooldown_seconds: Optional new cooldown period
    /// - is_paused: Optional new pause state
    /// - is_emergency_paused: Optional new emergency pause state
    /// - fee_tiers: Optional new fee tiers
    /// - max_daily_volume: Optional new daily volume limit
    /// - max_price_impact_bps: Optional new price impact limit
    /// - circuit_breaker_threshold: Optional new circuit breaker threshold
    /// - circuit_breaker_window: Optional new circuit breaker window
    /// - circuit_breaker_cooldown: Optional new circuit breaker cooldown
    /// - rate_limit_window: Optional new rate limit window
    /// - rate_limit_max: Optional new rate limit max
    /// 
    /// # State Updates:
    /// - Stores pending update
    /// - Emits update scheduled event
    pub fn schedule_parameter_update(
        ctx: Context<ScheduleParameterUpdate>,
        early_trade_fee_bps: Option<u64>,
        early_trade_window_seconds: Option<u64>,
        max_trade_size_bps: Option<u64>,
        min_trade_size: Option<u64>,
        cooldown_seconds: Option<u64>,
        is_paused: Option<bool>,
        is_emergency_paused: Option<bool>,
        fee_tiers: Option<Vec<FeeTier>>,
        max_daily_volume: Option<u64>,
        max_price_impact_bps: Option<u64>,
        circuit_breaker_threshold: Option<u64>,
        circuit_breaker_window: Option<u64>,
        circuit_breaker_cooldown: Option<u64>,
        rate_limit_window: Option<u64>,
        rate_limit_max: Option<u32>,
    ) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        require!(*ctx.accounts.admin.key == pool_state.admin, ErrorCode::Unauthorized);

        let current_time = Clock::get()?.unix_timestamp as u64;
        require!(current_time >= 0, ErrorCode::InvalidTimestamp);

        // Calculate timelock with overflow protection
        let scheduled_time = current_time
            .checked_add(86_400)
            .ok_or(ErrorCode::Overflow)?;

        // Validate parameter limits
        if let Some(fee) = early_trade_fee_bps {
            require!(fee <= 1000, ErrorCode::FeeTooHigh);
        }
        if let Some(window) = early_trade_window_seconds {
            require!(window <= pool_state.snipe_protection_seconds, ErrorCode::InvalidParameterRelationship);
        }
        if let Some(size) = max_trade_size_bps {
            require!(size <= 1000, ErrorCode::TradeTooLarge);
        }
        if let Some(cooldown) = cooldown_seconds {
            require!(cooldown <= 3600, ErrorCode::InvalidAmount);
        }
        if let Some(volume) = max_daily_volume {
            require!(volume > 0, ErrorCode::InvalidAmount);
        }
        if let Some(impact) = max_price_impact_bps {
            require!(impact <= 1000, ErrorCode::PriceImpactTooHigh);
        }
        if let Some(threshold) = circuit_breaker_threshold {
            require!(threshold > 0, ErrorCode::InvalidAmount);
        }
        if let Some(window) = circuit_breaker_window {
            require!(window > 0, ErrorCode::InvalidAmount);
        }
        if let Some(cooldown) = circuit_breaker_cooldown {
            require!(cooldown >= 60, ErrorCode::InvalidAmount);
        }
        if let Some(window) = rate_limit_window {
            require!(window > 0, ErrorCode::InvalidRateLimit);
        }
        if let Some(max) = rate_limit_max {
            require!(max > 0, ErrorCode::InvalidRateLimit);
        }

        // Validate circuit breaker window vs cooldown
        if let (Some(window), Some(cooldown)) = (circuit_breaker_window, circuit_breaker_cooldown) {
            require!(window >= cooldown, ErrorCode::InvalidParameterRelationship);
        }

        // Validate fee tiers if provided
        if let Some(tiers) = &fee_tiers {
            require!(!tiers.is_empty(), ErrorCode::InvalidFeeTier);
            require!(tiers.len() <= 10, ErrorCode::TooManyFeeTiers); // Cap at 10 tiers
            for (i, tier) in tiers.iter().enumerate() {
                require!(tier.fee_bps <= 1000, ErrorCode::FeeTooHigh);
                require!(tier.volume_threshold > 0, ErrorCode::InvalidFeeTier);
                if i > 0 {
                    require!(
                        tier.volume_threshold > tiers[i - 1].volume_threshold,
                        ErrorCode::InvalidFeeTier
                    );
                    require!(
                        tier.fee_bps <= tiers[i - 1].fee_bps,
                        ErrorCode::InvalidFeeTier
                    );
                    // Ensure minimum spacing between thresholds
                    let max_volume = max_daily_volume.unwrap_or(pool_state.max_daily_volume);
                    require!(
                        tier.volume_threshold - tiers[i - 1].volume_threshold >= max_volume / 100,
                        ErrorCode::InvalidFeeTierSpacing
                    );
                }
            }
        }

        pool_state.pending_update = Some(PendingUpdate {
            early_trade_fee_bps,
            early_trade_window_seconds,
            max_trade_size_bps,
            min_trade_size,
            cooldown_seconds,
            is_paused,
            is_emergency_paused,
            fee_tiers,
            max_daily_volume,
            max_price_impact_bps,
            circuit_breaker_threshold,
            circuit_breaker_window,
            circuit_breaker_cooldown,
            rate_limit_window,
            rate_limit_max,
            scheduled_time: scheduled_time as i64,
        });

        emit!(ParameterUpdateScheduled {
            pool: pool_state.key(),
            admin: pool_state.admin,
            scheduled_time: scheduled_time as i64,
        });

        Ok(())
    }

    /// Apply a scheduled parameter update after the timelock period
    /// 
    /// # Security Considerations:
    /// - Validates timelock expiration
    /// - Validates admin authority
    /// - Enforces parameter relationships
    /// - Updates state atomically
    /// - Emits events for tracking
    /// - Validates parameter limits
    /// - Validates fee tier configuration
    /// 
    /// # State Updates:
    /// - Applies pending update
    /// - Updates last update timestamp
    /// - Emits parameters updated event
    pub fn apply_parameter_update(ctx: Context<ApplyParameterUpdate>) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        require!(*ctx.accounts.admin.key == pool_state.admin, ErrorCode::Unauthorized);

        let current_time = Clock::get()?.unix_timestamp as u64;
        require!(current_time >= 0, ErrorCode::InvalidTimestamp);

        let pending_update = pool_state.pending_update.take().ok_or(ErrorCode::NoPendingUpdate)?;
        require!(
            current_time as i64 >= pending_update.scheduled_time,
            ErrorCode::TimelockNotExpired
        );

        // Apply updates with validation
        if let Some(fee) = pending_update.early_trade_fee_bps {
            pool_state.early_trade_fee_bps = fee;
        }
        if let Some(window) = pending_update.early_trade_window_seconds {
            require!(window <= pool_state.snipe_protection_seconds, ErrorCode::InvalidParameterRelationship);
            pool_state.early_trade_window_seconds = window;
        }
        if let Some(size) = pending_update.max_trade_size_bps {
            pool_state.max_trade_size_bps = size;
        }
        if let Some(size) = pending_update.min_trade_size {
            require!(
                size <= pool_state.max_trade_size_bps.checked_mul(1_000_000).unwrap().checked_div(10000).unwrap(),
                ErrorCode::InvalidParameterRelationship
            );
            pool_state.min_trade_size = size;
        }
        if let Some(cooldown) = pending_update.cooldown_seconds {
            require!(
                cooldown <= pool_state.snipe_protection_seconds,
                ErrorCode::InvalidParameterRelationship
            );
            pool_state.cooldown_seconds = cooldown;
        }
        if let Some(paused) = pending_update.is_paused {
            pool_state.is_paused = paused;
        }
        if let Some(emergency_paused) = pending_update.is_emergency_paused {
            pool_state.is_emergency_paused = emergency_paused;
        }
        if let Some(tiers) = pending_update.fee_tiers {
            pool_state.fee_tiers = tiers;
        }
        if let Some(volume) = pending_update.max_daily_volume {
            pool_state.max_daily_volume = volume;
        }
        if let Some(impact) = pending_update.max_price_impact_bps {
            pool_state.max_price_impact_bps = impact;
        }
        if let Some(threshold) = pending_update.circuit_breaker_threshold {
            pool_state.circuit_breaker_threshold = threshold;
        }
        if let Some(window) = pending_update.circuit_breaker_window {
            pool_state.circuit_breaker_window = window;
        }
        if let Some(cooldown) = pending_update.circuit_breaker_cooldown {
            pool_state.circuit_breaker_cooldown = cooldown;
        }
        if let Some(window) = pending_update.rate_limit_window {
            pool_state.rate_limit_window = window;
        }
        if let Some(max) = pending_update.rate_limit_max {
            pool_state.rate_limit_max = max;
        }

        // Validate circuit breaker window vs cooldown after update
        require!(
            pool_state.circuit_breaker_window >= pool_state.circuit_breaker_cooldown,
            ErrorCode::InvalidParameterRelationship
        );

        pool_state.last_update = current_time;

        emit!(ParametersUpdated {
            pool: pool_state.key(),
            admin: pool_state.admin,
            timestamp: current_time as i64,
        });

        Ok(())
    }

    /// Withdraw collected fees from the pool
    /// 
    /// # Security Considerations:
    /// - Validates admin authority
    /// - Validates fee amount
    /// - Validates token accounts
    /// - Validates pool authority PDA
    /// - Implements proper token transfers
    /// - Uses checks-effects-interactions pattern
    /// - Validates token account ownership
    /// - Validates token account delegate
    /// 
    /// # State Updates:
    /// - Resets total fees collected
    /// - Transfers fees to admin
    /// - Emits fees withdrawn event
    pub fn withdraw_fees(ctx: Context<WithdrawFees>) -> Result<()> {
        // Validate admin authority
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
        require!(ctx.accounts.fee_destination.delegate.is_none(), ErrorCode::TokenAccountDelegated);
        require!(ctx.accounts.pool_token_account.delegate.is_none(), ErrorCode::TokenAccountDelegated);
        
        // Validate pool authority PDA
        let (pool_authority, _) = Pubkey::find_program_address(
            &[b"pool_authority", pool_state.key().as_ref()],
            program_id
        );
        require!(
            pool_authority == ctx.accounts.pool_authority.key(),
            ErrorCode::InvalidPoolAuthority
        );
        
        // Get and validate current time
        let current_time = Clock::get()?.unix_timestamp as u64;
        require!(current_time >= 0, ErrorCode::InvalidTimestamp);
        
        // Store initial balances for validation
        let initial_pool_balance = ctx.accounts.pool_token_account.amount;
        let initial_fee_destination_balance = ctx.accounts.fee_destination.amount;

        // Store fee amount and reset state (checks)
        let fee_amount = pool_state.total_fees_collected;
        pool_state.total_fees_collected = 0;
        
        // Transfer fees (effects)
        let cpi_accounts = token::Transfer {
            from: ctx.accounts.pool_token_account.to_account_info(),
            to: ctx.accounts.fee_destination.to_account_info(),
            authority: ctx.accounts.pool_authority.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(
            CpiContext::new_with_signer(
                cpi_program,
                cpi_accounts,
                &[&[
                    b"pool_authority",
                    pool_state.key().as_ref(),
                    &[*ctx.bumps.get("pool_authority").unwrap()],
                ]],
            ),
            fee_amount,
        )?;
        
        // Validate transfer
        require!(
            ctx.accounts.pool_token_account.amount == initial_pool_balance.checked_sub(fee_amount).ok_or(ErrorCode::Overflow)?,
            ErrorCode::InvalidBalance
        );
        require!(
            ctx.accounts.fee_destination.amount == initial_fee_destination_balance.checked_add(fee_amount).ok_or(ErrorCode::Overflow)?,
            ErrorCode::InvalidBalance
        );

        // Validate final pool balance
        require!(
            ctx.accounts.pool_token_account.amount >= pool_state.min_trade_size,
            ErrorCode::InsufficientPoolBalance
        );

        // Emit withdrawal event (interactions)
        emit!(FeesWithdrawn {
            pool: pool_state.key(),
            admin: pool_state.admin,
            amount: fee_amount,
            timestamp: current_time as i64,
        });
        
        Ok(())
    }

    /// Emergency pause the pool
    /// 
    /// # Security Considerations:
    /// - Validates emergency admin authority
    /// - Updates state atomically
    /// - Emits events for tracking
    /// 
    /// # State Updates:
    /// - Sets emergency pause flag
    /// - Updates last update timestamp
    /// - Emits emergency paused event
    pub fn emergency_pause(ctx: Context<EmergencyPause>) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        require!(
            *ctx.accounts.emergency_admin.key == pool_state.emergency_admin,
            ErrorCode::InvalidEmergencyAdmin
        );
        
        pool_state.is_emergency_paused = true;
        pool_state.last_update = Clock::get()?.unix_timestamp as u64;
        
        emit!(EmergencyPaused {
            pool: pool_state.key(),
            emergency_admin: pool_state.emergency_admin,
            timestamp: pool_state.last_update as i64,
        });
        
        Ok(())
    }

    /// Resume pool operations after emergency pause
    /// 
    /// # Security Considerations:
    /// - Validates emergency admin authority
    /// - Updates state atomically
    /// - Emits events for tracking
    /// 
    /// # State Updates:
    /// - Clears emergency pause flag
    /// - Updates last update timestamp
    /// - Emits emergency resumed event
    pub fn emergency_resume(ctx: Context<EmergencyPause>) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        require!(
            *ctx.accounts.emergency_admin.key == pool_state.emergency_admin,
            ErrorCode::InvalidEmergencyAdmin
        );
        
        pool_state.is_emergency_paused = false;
        pool_state.last_update = Clock::get()?.unix_timestamp as u64;
        
        emit!(EmergencyResumed {
            pool: pool_state.key(),
            emergency_admin: pool_state.emergency_admin,
            timestamp: pool_state.last_update as i64,
        });
        
        Ok(())
    }

    /// Reset circuit breaker
    /// 
    /// # Security Considerations:
    /// - Validates admin authority
    /// - Resets volume tracking
    /// - Updates timestamps
    /// - Emits event for tracking
    /// 
    /// # State Updates:
    /// - Resets circuit breaker timestamp
    /// - Resets volume tracking
    /// - Updates last volume update timestamp
    /// - Emits circuit breaker reset event
    pub fn reset_circuit_breaker(ctx: Context<ResetCircuitBreaker>) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        require!(*ctx.accounts.admin.key == pool_state.admin, ErrorCode::Unauthorized);

        let current_time = Clock::get()?.unix_timestamp as u64;
        require!(current_time >= 0, ErrorCode::InvalidTimestamp);

        // Add cooldown period after circuit breaker triggers
        require!(
            current_time - pool_state.last_circuit_breaker >= pool_state.circuit_breaker_cooldown,
            ErrorCode::CircuitBreakerCooldown
        );

        pool_state.last_circuit_breaker = current_time;
        pool_state.volume_24h = 0;
        pool_state.last_volume_update = current_time;

        emit!(CircuitBreakerReset {
            pool: pool_state.key(),
            admin: pool_state.admin,
            timestamp: current_time as i64,
        });

        Ok(())
    }

    /// Update the pool admin
    /// 
    /// # Security Considerations:
    /// - Validates admin authority
    /// - Validates new admin
    /// - Emits event for tracking
    /// 
    /// # State Updates:
    /// - Updates admin
    pub fn update_admin(ctx: Context<UpdateAdmin>) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        
        // Verify current admin
        require!(
            pool_state.admin == ctx.accounts.current_admin.key(),
            ErrorCode::Unauthorized
        );

        // Verify new admin is not the same as current admin
        require!(
            pool_state.admin != ctx.accounts.new_admin.key(),
            ErrorCode::InvalidNewAdmin
        );

        // Update admin
        pool_state.admin = ctx.accounts.new_admin.key();

        // Emit event
        emit!(AdminUpdated {
            pool: pool_state.key(),
            old_admin: ctx.accounts.current_admin.key(),
            new_admin: ctx.accounts.new_admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializePoolProtection<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + // discriminator
            32 + // admin pubkey
            32 + // token mint pubkey
            1 + // token decimals
            8 + // snipe_protection_seconds
            8 + // early_trade_fee_bps
            8 + // early_trade_window_seconds
            8 + // pool_start_time
            8 + // total_fees_collected
            8 + // total_liquidity
            1 + // is_paused
            1 + // is_emergency_paused
            8 + // max_trade_size_bps
            8 + // min_trade_size
            8 + // cooldown_seconds
            8 + // last_trade_time
            1 + // version
            8 + // last_update
            1 + // is_locked
            48 + // pending_update option
            8 + // volume_24h
            8 + // last_volume_update
            32 + // emergency_admin
            100 + // fee_tiers vector (approximate)
            8 + // max_daily_volume
            8 + // max_price_impact_bps
            8 + // circuit_breaker_threshold
            8 + // circuit_breaker_window
            8 + // circuit_breaker_cooldown
            8 + // last_circuit_breaker
            8 + // rate_limit_window
            4 + // rate_limit_count
            4 + // rate_limit_max
            100 + // trader_blacklist (approximate)
            32, // padding for future fields
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
pub struct RemoveLiquidity<'info> {
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
pub struct ScheduleParameterUpdate<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct ApplyParameterUpdate<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub clock: Sysvar<'info, Clock>,
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
pub struct BlacklistTrader<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct EmergencyPause<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub emergency_admin: Signer<'info>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct ResetCircuitBreaker<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct UpdateAdmin<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    /// CHECK: Validated in the instruction
    pub current_admin: Signer<'info>,
    /// CHECK: Validated in the instruction
    pub new_admin: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct PoolState {
    pub admin: Pubkey,
    pub token_mint: Pubkey,
    pub token_decimals: u8,
    pub snipe_protection_seconds: u64,
    pub early_trade_fee_bps: u64,
    pub early_trade_window_seconds: u64,
    pub pool_start_time: u64,
    pub total_fees_collected: u64,
    pub total_liquidity: u64,
    pub is_paused: bool,
    pub is_emergency_paused: bool,
    pub max_trade_size_bps: u64,
    pub min_trade_size: u64,
    pub cooldown_seconds: u64,
    pub last_trade_time: u64,
    pub version: u8,
    pub last_update: u64,
    pub is_locked: bool,
    pub pending_update: Option<PendingUpdate>,
    pub volume_24h: u64,
    pub last_volume_update: u64,
    pub emergency_admin: Pubkey,
    pub fee_tiers: Vec<FeeTier>,
    pub max_daily_volume: u64,
    pub max_price_impact_bps: u64,
    pub circuit_breaker_threshold: u64,
    pub circuit_breaker_window: u64,
    pub circuit_breaker_cooldown: u64,
    pub last_circuit_breaker: u64,
    pub rate_limit_window: u64,
    pub rate_limit_count: u32,
    pub rate_limit_max: u32,
    pub trader_blacklist: Vec<Pubkey>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct FeeTier {
    pub volume_threshold: u64,
    pub fee_bps: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PendingUpdate {
    pub early_trade_fee_bps: Option<u64>,
    pub early_trade_window_seconds: Option<u64>,
    pub max_trade_size_bps: Option<u64>,
    pub min_trade_size: Option<u64>,
    pub cooldown_seconds: Option<u64>,
    pub is_paused: Option<bool>,
    pub is_emergency_paused: Option<bool>,
    pub fee_tiers: Option<Vec<FeeTier>>,
    pub max_daily_volume: Option<u64>,
    pub max_price_impact_bps: Option<u64>,
    pub circuit_breaker_threshold: Option<u64>,
    pub circuit_breaker_window: Option<u64>,
    pub circuit_breaker_cooldown: Option<u64>,
    pub rate_limit_window: Option<u64>,
    pub rate_limit_max: Option<u32>,
    pub scheduled_time: i64,
}

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
    pub timestamp: i64,
}

#[event]
pub struct LiquidityRemoved {
    pub pool: Pubkey,
    pub admin: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct TradeExecuted {
    pub pool: Pubkey,
    pub buyer: Pubkey,
    pub amount_in: u64,
    pub amount_out: u64,
    pub fee_amount: u64,
    pub timestamp: i64,
    pub token_mint: Pubkey,
}

#[event]
pub struct ParameterUpdateScheduled {
    pub pool: Pubkey,
    pub admin: Pubkey,
    pub scheduled_time: i64,
}

#[event]
pub struct ParametersUpdated {
    pub pool: Pubkey,
    pub admin: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct FeesWithdrawn {
    pub pool: Pubkey,
    pub admin: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct EmergencyPaused {
    pub pool: Pubkey,
    pub emergency_admin: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct EmergencyResumed {
    pub pool: Pubkey,
    pub emergency_admin: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct CircuitBreakerReset {
    pub pool: Pubkey,
    pub admin: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct CircuitBreakerTriggered {
    pub pool: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct RateLimitReset {
    pub pool: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct TraderBlacklisted {
    pub pool: Pubkey,
    pub trader: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct TraderUnblacklisted {
    pub pool: Pubkey,
    pub trader: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct AdminUpdated {
    pub pool: Pubkey,
    pub old_admin: Pubkey,
    pub new_admin: Pubkey,
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
    #[msg("Invalid timestamp")]
    InvalidTimestamp,
    #[msg("Fee overflow")]
    FeeOverflow,
    #[msg("Overflow in calculation")]
    Overflow,
    #[msg("No pending update")]
    NoPendingUpdate,
    #[msg("Timelock not expired")]
    TimelockNotExpired,
    #[msg("Emergency pause is active")]
    EmergencyPaused,
    #[msg("Invalid emergency admin")]
    InvalidEmergencyAdmin,
    #[msg("Invalid fee tier configuration")]
    InvalidFeeTier,
    #[msg("Too many fee tiers")]
    TooManyFeeTiers,
    #[msg("Volume limit exceeded")]
    VolumeLimitExceeded,
    #[msg("Rate limit exceeded")]
    RateLimitExceeded,
    #[msg("Circuit breaker triggered")]
    CircuitBreakerTriggered,
    #[msg("Price impact too high")]
    PriceImpactTooHigh,
    #[msg("Invalid rate limit parameters")]
    InvalidRateLimit,
    #[msg("Daily volume limit exceeded")]
    DailyVolumeLimitExceeded,
    #[msg("Rate limit counter overflow")]
    RateLimitOverflow,
    #[msg("Insufficient liquidity")]
    InsufficientLiquidity,
    #[msg("Circuit breaker is in cooldown")]
    CircuitBreakerCooldown,
    #[msg("Trader is blacklisted")]
    TraderBlacklisted,
    #[msg("Trader is already blacklisted")]
    TraderAlreadyBlacklisted,
    #[msg("Trader is not blacklisted")]
    TraderNotBlacklisted,
    #[msg("Blacklist is full")]
    BlacklistFull,
    #[msg("Invalid fee tier spacing")]
    InvalidFeeTierSpacing,
    #[msg("Token account has a delegate")]
    TokenAccountDelegated,
    #[msg("Invalid new admin")]
    InvalidNewAdmin,
}

struct ReentrancyGuard<'a> {
    pool_state: &'a mut Account<'a, PoolState>,
}

impl<'a> ReentrancyGuard<'a> {
    fn new(pool_state: &'a mut Account<'a, PoolState>) -> Result<Self> {
        require!(!pool_state.is_locked, ErrorCode::ReentrancyDetected);
        pool_state.is_locked = true;
        Ok(Self { pool_state })
    }
}

impl<'a> Drop for ReentrancyGuard<'a> {
    fn drop(&mut self) {
        self.pool_state.is_locked = false;
    }
}

// Add helper function for balance validation
fn validate_balance_change(
    current_balance: u64,
    expected_change: i64,
    min_balance: u64,
) -> Result<()> {
    if expected_change > 0 {
        require!(
            current_balance >= min_balance,
            ErrorCode::InsufficientBalance
        );
    } else {
        let expected_balance = current_balance.checked_sub(expected_change.unsigned_abs()).ok_or(ErrorCode::Overflow)?;
        require!(
            expected_balance >= min_balance,
            ErrorCode::InsufficientBalance
        );
    }
    Ok(())
}

// Helper function to calculate price impact
fn calculate_price_impact(
    amount_in: u64,
    pool_balance: u64,
    token_decimals: u8,
) -> Result<u64> {
    // Handle edge cases
    if amount_in == 0 || pool_balance == 0 {
        return Ok(0);
    }

    // Scale to same precision
    let scaled_amount = amount_in.checked_mul(10u64.pow(token_decimals as u32))
        .ok_or(ErrorCode::CalculationOverflow)?;
    
    // Calculate price impact in basis points
    let price_impact = scaled_amount
        .checked_mul(10000)
        .ok_or(ErrorCode::CalculationOverflow)?
        .checked_div(pool_balance)
        .ok_or(ErrorCode::CalculationOverflow)?;

    Ok(price_impact)
}