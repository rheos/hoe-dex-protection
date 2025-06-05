use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};

// Program ID (replace with actual ID after deployment)
declare_id!("YourProgramIdHere");

// Constants for fee and protection limits
const MAX_EARLY_TRADE_FEE_BPS: u64 = 1000; // 10% maximum early trade fee
const MAX_TIER_FEE_BPS: u64 = 1000; // 10% maximum tier-based fee
const MAX_TRADE_SIZE_BPS: u64 = 1000; // 10% of pool size
const MAX_PRICE_IMPACT_BPS: u64 = 1000; // 10% maximum price impact
const MAX_COOLDOWN_SECONDS: u64 = 3600; // 1 hour maximum cooldown
const MIN_CIRCUIT_BREAKER_COOLDOWN: u64 = 60; // 1 minute minimum cooldown
const MAX_BLACKLIST_SIZE: usize = 100; // Maximum number of blacklisted traders
const MAX_FEE_TIERS: usize = 10; // Maximum number of fee tiers
const MINIMUM_FEE: u64 = 1; // Minimum fee in token units
const MAX_TOKEN_DECIMALS: u8 = 9; // Maximum token decimals supported
const MIN_FEE_TIER_SPACING_BPS: u64 = 100; // 1% of max_daily_volume minimum spacing
const EMERGENCY_TIMELOCK_SECONDS: u64 = 3600; // 1 hour emergency action delay
const ADMIN_UPDATE_COOLDOWN: u64 = 86400; // 24 hours between admin updates
const BATCH_BLACKLIST_MAX_SIZE: usize = 50; // Maximum traders per batch blacklist

// Fee mode constants for tracking fee application
const FEE_MODE_NONE: u8 = 0; // No fee applied
const FEE_MODE_EARLY_TRADE: u8 = 1; // Early trade fee applied
const FEE_MODE_TIER_BASED: u8 = 2; // Volume-based tier fee applied

#[program]
pub mod hoe_dex_protection {
    use super::*;

    /// Initialize a new pool with protection parameters
    /// 
    /// This function sets up the initial state of the pool with all necessary
    /// protection mechanisms and parameters. It performs extensive validation
    /// to ensure the pool starts in a safe state.
    pub fn initialize_pool(
        ctx: Context<InitializePool>,
        early_trade_fee_bps: u64,
        early_trade_window_seconds: u64,
        max_trade_size_bps: u64,
        min_trade_size: u64,
        cooldown_seconds: u64,
        max_daily_volume: u64,
        max_price_impact_bps: u64,
        circuit_breaker_threshold: u64,
        circuit_breaker_window: u64,
        circuit_breaker_cooldown: u64,
        rate_limit_window: u64,
        rate_limit_max: u32,
        fee_tiers: Vec<FeeTier>,
        snipe_protection_seconds: u64,
    ) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp;

        // Validate admin authority
        require!(
            ctx.accounts.admin.key() == pool_state.admin,
            ErrorCode::Unauthorized
        );

        // Ensure token mint cannot be frozen (security measure)
        require!(
            ctx.accounts.token_mint.freeze_authority.is_none(),
            ErrorCode::TokenMintHasFreezeAuthority
        );

        // Validate all protection parameters are within safe limits
        require!(snipe_protection_seconds > 0, ErrorCode::InvalidAmount);
        require!(early_trade_fee_bps <= MAX_EARLY_TRADE_FEE_BPS, ErrorCode::FeeTooHigh);
        require!(max_trade_size_bps <= MAX_TRADE_SIZE_BPS, ErrorCode::TradeTooLarge);
        require!(max_price_impact_bps <= MAX_PRICE_IMPACT_BPS, ErrorCode::PriceImpactTooHigh);
        require!(cooldown_seconds <= MAX_COOLDOWN_SECONDS, ErrorCode::InvalidAmount);
        require!(circuit_breaker_cooldown >= MIN_CIRCUIT_BREAKER_COOLDOWN, ErrorCode::InvalidAmount);
        require!(max_daily_volume > 0, ErrorCode::InvalidAmount);
        require!(circuit_breaker_threshold > 0, ErrorCode::InvalidAmount);
        require!(circuit_breaker_window > 0, ErrorCode::InvalidAmount);
        require!(rate_limit_window > 0, ErrorCode::InvalidRateLimit);
        require!(rate_limit_max > 0, ErrorCode::InvalidRateLimit);

        // Ensure early trade window is within snipe protection period
        require!(
            early_trade_window_seconds <= snipe_protection_seconds,
            ErrorCode::InvalidParameterRelationship
        );

        // Validate fee tier configuration
        require!(!fee_tiers.is_empty(), ErrorCode::InvalidFeeTier);
        require!(fee_tiers.len() <= MAX_FEE_TIERS, ErrorCode::TooManyFeeTiers);

        // Calculate minimum spacing between fee tiers based on max daily volume
        let min_spacing = max_daily_volume
            .checked_mul(MIN_FEE_TIER_SPACING_BPS)
            .ok_or(ErrorCode::Overflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::Overflow)?;

        // Validate fee tier ordering and spacing
        for (i, tier) in fee_tiers.iter().enumerate() {
            require!(tier.fee_bps <= MAX_TIER_FEE_BPS, ErrorCode::FeeTooHigh);
            require!(tier.volume_threshold > 0, ErrorCode::InvalidFeeTier);
            
            if i > 0 {
                let prev_tier = &fee_tiers[i - 1];
                // Ensure tiers are properly ordered by volume threshold
                require!(
                    tier.volume_threshold > prev_tier.volume_threshold,
                    ErrorCode::DuplicateFeeTierThreshold
                );
                // Ensure sufficient spacing between tiers
                require!(
                    tier.volume_threshold - prev_tier.volume_threshold >= min_spacing,
                    ErrorCode::InvalidFeeTierSpacing
                );
                // Ensure fees decrease as volume increases
                require!(
                    tier.fee_bps <= prev_tier.fee_bps,
                    ErrorCode::InvalidFeeTier
                );
            }
        }

        // Set initial version
        pool_state.version = 1;

        // Initialize pool state with validated parameters
        pool_state.snipe_protection_seconds = snipe_protection_seconds;
        pool_state.early_trade_fee_bps = early_trade_fee_bps;
        pool_state.early_trade_window_seconds = early_trade_window_seconds;
        pool_state.max_trade_size_bps = max_trade_size_bps;
        pool_state.min_trade_size = min_trade_size;
        pool_state.cooldown_seconds = cooldown_seconds;
        pool_state.max_daily_volume = max_daily_volume;
        pool_state.max_price_impact_bps = max_price_impact_bps;
        pool_state.circuit_breaker_threshold = circuit_breaker_threshold;
        pool_state.circuit_breaker_window = circuit_breaker_window;
        pool_state.circuit_breaker_cooldown = circuit_breaker_cooldown;
        pool_state.rate_limit_window = rate_limit_window;
        pool_state.rate_limit_max = rate_limit_max;
        pool_state.fee_tiers = fee_tiers;
        pool_state.pool_start_time = current_time as u64;
        pool_state.last_admin_update = current_time as u64;
        pool_state.last_rate_limit_reset = current_time as u64;
        pool_state.rate_limit_count = 0;
        pool_state.total_fees_collected = 0;
        pool_state.last_trade_time = 0;
        pool_state.last_circuit_breaker = 0;
        pool_state.volume_24h = 0;
        pool_state.last_volume_update = current_time as u64;
        pool_state.is_paused = false;
        pool_state.is_emergency_paused = false;
        pool_state.pending_update = None;
        pool_state.emergency_action_scheduled_time = 0;

        // Emit initialization event
        emit!(PoolInitialized {
            pool: pool_state.key(),
            admin: ctx.accounts.admin.key(),
            timestamp: current_time,
        });

        Ok(())
    }

    pub fn add_liquidity(ctx: Context<AddLiquidity>, amount: u64) -> Result<()> {
        // Unchanged from original, included for completeness
        require!(
            ctx.accounts.token_program.key() == token::ID,
            ErrorCode::InvalidTokenProgram
        );
        require!(amount > 0, ErrorCode::InvalidAmount);
        let pool_state = &mut ctx.accounts.pool_state;
        require!(pool_state.pool_start_time == 0, ErrorCode::PoolAlreadyStarted);
        require!(!pool_state.is_paused, ErrorCode::PoolPaused);
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
        let (pool_authority, _) = Pubkey::find_program_address(
            &[b"pool_authority", pool_state.key().as_ref()],
            program_id
        );
        require!(
            pool_authority == ctx.accounts.pool_authority.key(),
            ErrorCode::InvalidPoolAuthority
        );
        let current_time = Clock::get()?.unix_timestamp;
        require!(current_time >= 0, ErrorCode::InvalidTimestamp);
        let initial_admin_balance = ctx.accounts.admin_token_account.amount;
        let initial_pool_balance = ctx.accounts.pool_token_account.amount;
        pool_state.pool_start_time = current_time as u64;
        pool_state.total_liquidity = amount;
        pool_state.last_update = current_time as u64;
        let cpi_accounts = token::Transfer {
            from: ctx.accounts.admin_token_account.to_account_info(),
            to: ctx.accounts.pool_token_account.to_account_info(),
            authority: ctx.accounts.admin.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(CpiContext::new(cpi_program, cpi_accounts), amount)?;
        require!(
            ctx.accounts.admin_token_account.amount == initial_admin_balance.checked_sub(amount).ok_or(ErrorCode::Overflow)?,
            ErrorCode::InvalidBalance
        );
        require!(
            ctx.accounts.pool_token_account.amount == initial_pool_balance.checked_add(amount).ok_or(ErrorCode::Overflow)?,
            ErrorCode::InvalidBalance
        );
        require!(
            ctx.accounts.pool_token_account.amount >= amount,
            ErrorCode::InsufficientPoolBalance
        );
        emit!(LiquidityAdded {
            pool: pool_state.key(),
            admin: pool_state.admin,
            amount,
            timestamp: current_time as i64,
        });
        Ok(())
    }

    pub fn remove_liquidity(ctx: Context<RemoveLiquidity>, amount: u64) -> Result<()> {
        // Unchanged from original
        require!(
            ctx.accounts.token_program.key() == token::ID,
            ErrorCode::InvalidTokenProgram
        );
        let pool_state = &mut ctx.accounts.pool_state;
        require!(*ctx.accounts.admin.key == pool_state.admin, ErrorCode::Unauthorized);
        require!(!pool_state.is_paused, ErrorCode::PoolPaused);
        require!(!pool_state.is_emergency_paused, ErrorCode::EmergencyPaused);
        require!(pool_state.total_liquidity >= amount, ErrorCode::InsufficientLiquidity);
        require!(amount > 0, ErrorCode::InvalidAmount);
        require!(
            ctx.accounts.admin_token_account.owner == ctx.accounts.admin.key(),
            ErrorCode::InvalidTokenAccount
        );
        require!(
            ctx.accounts.pool_token_account.owner == ctx.accounts.pool_token_account.owner(),
            ErrorCode::InvalidTokenAccount
        );
        require!(ctx.accounts.admin_token_account.delegate.is_none(), ErrorCode::TokenAccountDelegated);
        require!(ctx.accounts.pool_token_account.delegate.is_none(), ErrorCode::TokenAccountDelegated);
        let (pool) = Pubkey::find_program_address(
            &[b"pool_authority", pool_state.key().as_ref()],
            program_id,
        );
        require!(
            pool_state == ctx.accounts.pool().key(),
            ErrorCode::InvalidPoolAuthority
        );
        let current_time = Clock::get()?.unix_timestamp() as u64;
        require!(current_time >= 0, ErrorCode::InvalidTimestamp);
        let initial_pool_balance = ctx.accounts.pool_token_account.amount;
        let initial_admin_amount = ctx.accounts.admin_token_account.amount;
        pool_state.total_liquidity = pool_state.total_liquidity.checked_sub(amount).unwrap();
        pool_state.last_update = current_time as i64;
        let cpi_accounts = token::Transfer {
            from: ctx.accounts.pool_token_account.to_account(),
            to: ctx.accounts.admin_token_account.to_account(),
            authority: ctx.accounts.pool_authority.to_account(),
        };
        let cpi_program = ctx.accounts.token_program.to_account();
        token::transfer(
            CpiContext::new_with_signer(
                cpi_program,
                cpi_accounts,
                &[&[
                    b"pool_state",
                    pool_state.key().as_ref(),
                    &[*ctx.accounts().get("pool").unwrap()],
                ]],
            ),
            amount,
        )?;
        require!(
            ctx.accounts.pool_token_amount == initial_pool.balance.checked_sub(amount).unwrap_or(ErrorCode::Overflow)?,
            ErrorCode::InvalidBalance
        );
        require!(
            ctx.accounts.admin_token_amount == initial_admin_amount.balance.checked_add(amount).unwrap_or_else(ErrorCode::Overflow)?,
            ErrorCode::InvalidBalance
        );
        if amount != pool_state.total_liquidity {
            require!(
                ctx.accounts.pool_token_amount.amount >= pool_state.min_trade_size,
                ErrorCode::InsufficientPoolBalance
            );
        } else {
            require!(
                ctx.accounts.pool_token_amount.amount >= amount,
                ErrorCode::InsufficientPoolBalance
            );
        }
        emit!(LiquidityRemoved {
            pool: pool_state.key(),
            admin: pool_state.pool_admin,
            amount,
            timestamp: current_time as i64,
        });
        Ok(())
    }

    /// Execute a trade in the pool with all protection mechanisms active
    /// 
    /// This function implements the core trading logic with multiple layers of protection:
    /// - Snipe protection
    /// - Rate limiting
    /// - Circuit breaker
    /// - Price impact checks
    /// - Fee calculation
    /// - Volume tracking
    pub fn execute_trade(
        ctx: Context<ExecuteTrade>,
        amount_in: u64,
        minimum_amount_out: u64,
    ) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;

        // Reentrancy protection using guard pattern
        let _guard = ReentrancyGuard::new(pool_state)?;

        // Check if trade is within snipe protection window
        require!(
            current_time >= pool_state.pool_start_time + pool_state.snipe_protection_seconds,
            ErrorCode::SnipeProtectionActive
        );

        // Check if trader is blacklisted
        require!(
            !pool_state.trader_blacklist.contains(&ctx.accounts.buyer.key()),
            ErrorCode::TraderBlacklisted
        );

        // Validate pool state and trade parameters
        require!(!pool_state.is_paused, ErrorCode::PoolPaused);
        require!(!pool_state.is_emergency_paused, ErrorCode::EmergencyPaused);
        require!(amount_in > 0, ErrorCode::InvalidAmount);
        require!(amount_in >= pool_state.min_trade_size, ErrorCode::TradeTooSmall);

        // Rate limiting with independent reset tracking
        if current_time - pool_state.last_rate_limit_reset >= pool_state.rate_limit_window {
            pool_state.rate_limit_count = 0;
            pool_state.last_rate_limit_reset = current_time;
            emit!(RateLimitReset {
                pool: pool_state.key(),
                timestamp: current_time as i64,
            });
        }
        require!(
            pool_state.rate_limit_count < pool_state.rate_limit_max,
            ErrorCode::RateLimitExceeded
        );
        pool_state.rate_limit_count += 1;

        // Circuit breaker check with volume tracking
        let volume_24h = pool_state.volume_24h.checked_add(amount_in).ok_or(ErrorCode::Overflow)?;
        let is_circuit_breaker_active = current_time - pool_state.last_circuit_breaker < pool_state.circuit_breaker_cooldown;
        
        // Trigger circuit breaker if volume threshold exceeded
        if is_circuit_breaker_active && volume_24h > pool_state.circuit_breaker_threshold {
            return Err(ErrorCode::CircuitBreakerCooldown.into());
        }
        if volume_24h > pool_state.circuit_breaker_threshold {
            pool_state.last_circuit_breaker = current_time;
            emit!(CircuitBreakerTriggered {
                pool: pool_state.key(),
                timestamp: current_time as i64,
            });
        }

        // Check daily volume limit
        require!(
            volume_24h <= pool_state.max_daily_volume,
            ErrorCode::DailyVolumeLimitExceeded
        );

        // Calculate and validate price impact
        let price_impact = calculate_price_impact(
            amount_in,
            ctx.accounts.pool_token_account.amount,
            pool_state.token_decimals,
        )?;
        require!(
            price_impact <= pool_state.max_price_impact_bps,
            ErrorCode::PriceImpactTooHigh
        );

        // Calculate fee with mode tracking
        let (fee_amount, fee_mode) = calculate_fee(pool_state, amount_in, current_time as i64)?;
        require!(fee_amount >= MINIMUM_FEE, ErrorCode::FeeTooLow);
        require!(fee_amount < amount_in, ErrorCode::FeeTooHigh);

        // Calculate and validate output amount
        let amount_out = amount_in.checked_sub(fee_amount).ok_or(ErrorCode::Overflow)?;
        require!(amount_out >= minimum_amount_out, ErrorCode::SlippageExceeded);

        // Update pool state
        pool_state.volume_24h = volume_24h;
        pool_state.last_trade_time = current_time;
        pool_state.total_fees_collected = pool_state.total_fees_collected.checked_add(fee_amount).ok_or(ErrorCode::Overflow)?;

        // Execute token transfers
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.buyer_token_account.to_account_info(),
                    to: ctx.accounts.pool_token_account.to_account_info(),
                    authority: ctx.accounts.buyer.to_account_info(),
                },
            ),
            amount_in,
        )?;

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.pool_token_account.to_account_info(),
                    to: ctx.accounts.buyer_token_account.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                &[&[
                    b"pool_authority",
                    pool_state.key().as_ref(),
                    &[*ctx.bumps.get("pool_authority").unwrap()],
                ]],
            ),
            amount_out,
        )?;

        // Emit trade event with fee mode
        emit!(TradeExecuted {
            pool: pool_state.key(),
            buyer: ctx.accounts.buyer.key(),
            amount_in,
            amount_out,
            fee_amount,
            fee_mode,
            timestamp: current_time as i64,
            token_mint: pool_state.token_mint,
        });

        Ok(())
    }

    /// Calculate fee based on trade timing and volume
    /// 
    /// This function determines the appropriate fee to charge based on:
    /// 1. Whether the trade is within the early trade window
    /// 2. The current 24h volume and applicable fee tier
    /// 3. Returns both the fee amount and the fee mode for tracking
    fn calculate_fee(pool_state: &PoolState, amount_in: u64, current_time: i64) -> Result<(u64, u8)> {
        // Early trade fee if within protection window
        if current_time - pool_state.pool_start_time as i64 <= pool_state.early_trade_window_seconds as i64 {
            let fee = amount_in
                .checked_mul(pool_state.early_trade_fee_bps)
                .ok_or(ErrorCode::Overflow)?
                .checked_div(10000)
                .ok_or(ErrorCode::Overflow)?;
            return Ok((fee, FEE_MODE_EARLY_TRADE));
        }

        // Fee tier based on current 24h volume
        for tier in &pool_state.fee_tiers {
            if pool_state.volume_24h <= tier.volume_threshold {
                let fee = amount_in
                    .checked_mul(tier.fee_bps)
                    .ok_or(ErrorCode::Overflow)?
                    .checked_div(10000)
                    .ok_or(ErrorCode::Overflow)?;
                return Ok((fee, FEE_MODE_TIER_BASED));
            }
        }

        // No fee if no tier applies
        Ok((0, FEE_MODE_NONE))
    }

    /// Calculate price impact of a trade
    /// 
    /// This function calculates the price impact of a trade in basis points,
    /// taking into account token decimals for accurate calculation.
    fn calculate_price_impact(amount_in: u64, pool_balance: u64, token_decimals: u8) -> Result<u64> {
        // Scale amounts to same decimal precision
        let amount_in_scaled = amount_in
            .checked_mul(10u64.pow(token_decimals as u32))
            .ok_or(ErrorCode::Overflow)?;
        let pool_balance_scaled = pool_balance
            .checked_mul(10u64.pow(token_decimals as u32))
            .ok_or(ErrorCode::Overflow)?;
        
        // Calculate impact in basis points
        let impact = amount_in_scaled
            .checked_mul(10000)
            .ok_or(ErrorCode::Overflow)?
            .checked_div(pool_balance_scaled)
            .ok_or(ErrorCode::Overflow)?;
        
        // Handle edge case of very small trades
        if impact == 0 && amount_in > 0 {
            return Ok(1);
        }
        Ok(impact)
    }

    /// Blacklist a trader to prevent them from trading
    /// 
    /// This function allows the admin to blacklist a trader with:
    /// - Admin must be a signer
    /// - Trader cannot be already blacklisted
    /// - Trader cannot be the admin or emergency admin
    pub fn blacklist_trader(ctx: Context<BlacklistTrader>, trader: Pubkey) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;

        // Validate admin
        require!(
            ctx.accounts.admin.key() == pool_state.admin,
            ErrorCode::InvalidAdmin
        );

        // Validate trader
        require!(!pool_state.trader_blacklist.contains(&trader), ErrorCode::TraderAlreadyBlacklisted);
        require!(trader != pool_state.admin, ErrorCode::InvalidTrader);
        require!(trader != pool_state.emergency_admin, ErrorCode::InvalidTrader);

        // Add trader to blacklist
        pool_state.trader_blacklist.push(trader);

        // Emit blacklist event
        emit!(TraderBlacklisted {
            pool: pool_state.key(),
            trader,
            timestamp: current_time as i64,
        });

        Ok(())
    }

    /// Remove a trader from the blacklist
    /// 
    /// This function allows the admin to remove a trader from the blacklist with:
    /// - Admin must be a signer
    /// - Trader must be currently blacklisted
    pub fn remove_from_blacklist(ctx: Context<RemoveFromBlacklist>, trader: Pubkey) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;

        // Validate admin
        require!(
            ctx.accounts.admin.key() == pool_state.admin,
            ErrorCode::InvalidAdmin
        );

        // Find and remove trader from blacklist
        let index = pool_state.trader_blacklist
            .iter()
            .position(|&x| x == trader)
            .ok_or(ErrorCode::TraderNotBlacklisted)?;
        pool_state.trader_blacklist.remove(index);

        // Emit removal event
        emit!(TraderRemovedFromBlacklist {
            pool: pool_state.key(),
            trader,
            timestamp: current_time as i64,
        });

        Ok(())
    }

    /// Withdraw collected fees from the pool
    /// 
    /// This function allows the admin to withdraw collected fees with:
    /// - Admin must be a signer
    /// - Fees must be available to withdraw
    /// - Reentrancy protection
    pub fn withdraw_fees(ctx: Context<WithdrawFees>) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;

        // Reentrancy protection
        let _guard = ReentrancyGuard::new(pool_state)?;

        // Validate admin
        require!(
            ctx.accounts.admin.key() == pool_state.admin,
            ErrorCode::InvalidAdmin
        );

        // Validate token accounts
        require!(
            ctx.accounts.pool_token_account.mint == pool_state.token_mint,
            ErrorCode::InvalidTokenAccount
        );
        require!(
            ctx.accounts.admin_token_account.mint == pool_state.token_mint,
            ErrorCode::InvalidTokenAccount
        );

        // Check if fees are available
        let fees_to_withdraw = pool_state.total_fees_collected;
        require!(fees_to_withdraw > 0, ErrorCode::NoFeesAvailable);

        // Reset fees before transfer to prevent reentrancy
        pool_state.total_fees_collected = 0;

        // Transfer fees to admin
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.pool_token_account.to_account_info(),
                    to: ctx.accounts.admin_token_account.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                &[&[
                    b"pool_authority",
                    pool_state.key().as_ref(),
                    &[*ctx.bumps.get("pool_authority").unwrap()],
                ]],
            ),
            fees_to_withdraw,
        )?;

        // Emit withdrawal event
        emit!(FeesWithdrawn {
            pool: pool_state.key(),
            admin: ctx.accounts.admin.key(),
            amount: fees_to_withdraw,
            timestamp: current_time as i64,
        });

        Ok(())
    }

    /// Lock fee tiers to prevent further changes
    /// 
    /// This function allows the admin to permanently lock fee tiers with:
    /// - Admin must be a signer
    /// - Fee tiers must not be already locked
    pub fn lock_fee_tiers(ctx: Context<LockFeeTiers>) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;

        // Validate admin
        require!(
            ctx.accounts.admin.key() == pool_state.admin,
            ErrorCode::InvalidAdmin
        );

        // Check if fee tiers are already locked
        require!(!pool_state.fee_tiers_locked, ErrorCode::FeeTiersLocked);

        // Lock fee tiers
        pool_state.fee_tiers_locked = true;

        // Emit locking event
        emit!(FeeTiersLocked {
            pool: pool_state.key(),
            admin: ctx.accounts.admin.key(),
            timestamp: current_time as i64,
        });

        Ok(())
    }

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
        let current_time = Clock::get()?.unix_timestamp;
        require!(
            ctx.accounts.admin.key() == pool_state.admin,
            ErrorCode::Unauthorized
        );
        if let Some(new_fee_tiers) = &fee_tiers {
            require!(!new_fee_tiers.is_empty(), ErrorCode::InvalidFeeTier);
            require!(new_fee_tiers.len() <= MAX_FEE_TIERS, ErrorCode::TooManyFeeTiers);
            let min_spacing = pool_state.max_daily_volume
                .checked_mul(MIN_FEE_TIER_SPACING_BPS)
                .ok_or(ErrorCode::Overflow)?
                .checked_div(10000)?;
            for (i, tier) in new_fee_tiers.iter().enumerate() {
                require!(tier.fee_bps <= MAX_TIER_FEE_BPS, ErrorCode::FeeTooHigh);
                require!(tier.volume_threshold > 0, ErrorCode::InvalidFeeTier);
                if i > 0 {
                    let prev = &new_fee_tiers[i - 1];
                    require!(
                        tier.volume_threshold > prev.volume_threshold,
                        ErrorCode::DuplicateFeeTierThreshold
                    );
                    require!(
                        tier.volume_threshold - prev.volume_threshold >= min_spacing,
                        ErrorCode::InvalidFeeTierSpacing
                    );
                    require!(
                        tier.fee_bps <= prev.fee_bps,
                        ErrorCode::InvalidFeeTier
                    );
                }
            }
        }
        let pending_update = PendingUpdate {
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
            scheduled_time: current_time + 86400,
        };
        pool_state.pending_update = Some(pending_update);
        emit!(ParameterUpdateScheduled {
            pool: pool_state.key(),
            admin: ctx.accounts.admin.key(),
            scheduled_time: current_time + 86400,
        });
        Ok(())
    }

    pub fn cancel_parameter_update(ctx: Context<ScheduleParameterUpdate>) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp;
        require!(
            ctx.accounts.admin.key() == pool_state.admin,
            ErrorCode::Unauthorized
        );
        require!(pool_state.pending_update.is_some(), ErrorCode::NoPendingUpdate);
        pool_state.pending_update = None;
        emit!(ParameterUpdateCancelled {
            pool: pool_state.key(),
            admin: ctx.accounts.admin.key(),
            timestamp: current_time,
        });
        Ok(())
    }

    pub fn apply_parameter_update(ctx: Context<ApplyParameterUpdate>) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;
        require!(
            ctx.accounts.admin.key() == pool_state.admin,
            ErrorCode::Unauthorized
        );
        let pending_update = pool_state.pending_update.take().ok_or(ErrorCode::NoPendingUpdate)?;
        require!(
            current_time >= pending_update.scheduled_time as u64,
            ErrorCode::TimelockNotExpired
        );
        if let Some(fee) = pending_update.early_trade_fee_bps {
            pool_state.early_trade_fee_bps = fee;
        }
        if let Some(window) = pending_update.early_trade_window_seconds {
            require!(
                window <= pool_state.snipe_protection_seconds,
                ErrorCode::InvalidParameterRelationship
            );
            pool_state.early_trade_window_seconds = window;
        }
        if let Some(size) = pending_update.max_trade_size_bps {
            pool_state.max_trade_size_bps = size;
        }
        if let Some(size) = pending_update.min_trade_size {
            require!(
                size <= pool_state.max_trade_size_bps.checked_mul(1000_000).unwrap().checked_div(10000)?,
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

    pub fn schedule_emergency_pause(ctx: Context<EmergencyPause>) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;
        require!(
            ctx.accounts.emergency_admin.key() == pool_state.emergency_admin,
            ErrorCode::InvalidEmergencyAdmin
        );
        pool_state.emergency_action_scheduled_time = current_time + EMERGENCY_TIMELOCK_SECONDS;
        emit!(EmergencyPauseScheduled {
            pool: pool_state.key(),
            emergency_admin: pool_state.emergency_admin,
            scheduled_time: current_time + EMERGENCY_TIMELOCK_SECONDS,
        });
        Ok(())
    }

    pub fn apply_emergency_pause(ctx: Context<EmergencyPause>) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;
        require!(
            ctx.accounts.emergency_admin.key() == pool_state.emergency_admin,
            ErrorCode::InvalidEmergencyAdmin
        );
        require!(
            current_time >= pool_state.emergency_action_scheduled_time,
            ErrorCode::TimelockNotExpired
        );
        pool_state.is_emergency_paused = true;
        pool_state.last_update = current_time;
        pool_state.emergency_action_scheduled_time = 0;
        emit!(EmergencyPaused {
            pool: pool_state.key(),
            emergency_admin: pool_state.emergency_admin,
            timestamp: current_time as i64,
        });
        Ok(())
    }

    pub fn schedule_emergency_resume(ctx: Context<EmergencyPause>) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;
        require!(
            ctx.accounts.emergency_admin.key() == pool_state.emergency_admin,
            ErrorCode::InvalidEmergencyAdmin
        );
        pool_state.emergency_action_scheduled_time = current_time + EMERGENCY_TIMELOCK_SECONDS;
        emit!(EmergencyResumeScheduled {
            pool: pool_state.key(),
            emergency_admin: pool_state.emergency_admin,
            scheduled_time: current_time + EMERGENCY_TIMELOCK_SECONDS,
        });
        Ok(())
    }

    pub fn apply_emergency_resume(ctx: Context<EmergencyPause>) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;
        require!(
            ctx.accounts.emergency_admin.key() == pool_state.emergency_admin,
            ErrorCode::InvalidEmergencyAdmin
        );
        require!(
            current_time >= pool_state.emergency_action_scheduled_time,
            ErrorCode::TimelockNotExpired
        );
        pool_state.is_emergency_paused = false;
        pool_state.last_update = current_time;
        pool_state.emergency_action_scheduled_time = 0;
        emit!(EmergencyResumed {
            pool: pool_state.key(),
            emergency_admin: pool_state.emergency_admin,
            timestamp: current_time as i64,
        });
        Ok(())
    }

    pub fn reset_circuit_breaker(ctx: Context<ResetCircuitBreaker>) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;
        require!(ctx.accounts.admin.key() == pool_state.admin, ErrorCode::Unauthorized);
        require!(current_time >= 0, ErrorCode::InvalidTimestamp);
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

    /// Update the pool admin with cooldown protection
    /// 
    /// This function allows changing the pool admin with the following protections:
    /// - 24-hour cooldown between admin changes
    /// - New admin must be different from current and emergency admin
    /// - Current admin must be a signer
    pub fn update_admin(ctx: Context<UpdateAdmin>, new_admin: Pubkey) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;

        // Validate current admin
        require!(
            ctx.accounts.admin.key() == pool_state.admin,
            ErrorCode::InvalidAdmin
        );

        // Check admin change cooldown
        require!(
            current_time - pool_state.last_admin_update >= ADMIN_UPDATE_COOLDOWN,
            ErrorCode::AdminUpdateTooFrequent
        );

        // Validate new admin
        require!(new_admin != pool_state.admin, ErrorCode::InvalidNewAdmin);
        require!(new_admin != pool_state.emergency_admin, ErrorCode::InvalidNewAdmin);

        // Update admin and timestamp
        let old_admin = pool_state.admin;
        pool_state.admin = new_admin;
        pool_state.last_admin_update = current_time;

        // Emit admin update event
        emit!(AdminUpdated {
            pool: pool_state.key(),
            old_admin,
            new_admin,
            timestamp: current_time as i64,
        });

        Ok(())
    }

    /// Batch blacklist multiple traders efficiently
    /// 
    /// This function allows the admin to blacklist multiple traders in a single transaction with:
    /// - Admin must be a signer
    /// - Maximum of 50 traders per batch
    /// - Each trader must not be already blacklisted
    /// - No trader can be the admin or emergency admin
    pub fn batch_blacklist_traders(ctx: Context<BlacklistTrader>, traders: Vec<Pubkey>) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;

        // Validate admin
        require!(
            ctx.accounts.admin.key() == pool_state.admin,
            ErrorCode::InvalidAdmin
        );

        // Check batch size
        require!(traders.len() <= BATCH_BLACKLIST_MAX_SIZE, ErrorCode::TooManyTraders);
        require!(
            pool_state.trader_blacklist.len() + traders.len() <= MAX_BLACKLIST_SIZE,
            ErrorCode::BlacklistFull
        );

        // Process each trader
        for trader in traders {
            // Validate trader
            require!(!pool_state.trader_blacklist.contains(&trader), ErrorCode::TraderAlreadyBlacklisted);
            require!(trader != pool_state.admin, ErrorCode::InvalidTrader);
            require!(trader != pool_state.emergency_admin, ErrorCode::InvalidTrader);

            // Add trader to blacklist
            pool_state.trader_blacklist.push(trader);

            // Emit individual blacklist event
            emit!(TraderBlacklisted {
                pool: pool_state.key(),
                trader,
                timestamp: current_time as i64,
            });
        }

        // Emit batch completion event
        emit!(BatchBlacklistCompleted {
            pool: pool_state.key(),
            admin: ctx.accounts.admin.key(),
            count: traders.len() as u64,
            timestamp: current_time as i64,
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializePool<'info> {
    #[account(
        init,
        payer = admin,
        space = PoolState::calculate_space(),
        seeds = [b"pool_state", admin.key().as_ref()],
        bump
    )]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub token_mint: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

// Other account structs unchanged for brevity

#[account]
pub struct PoolState {
    pub version: u8, // Added for upgrade tracking
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
    pub last_rate_limit_reset: u64,
    pub trader_blacklist: Vec<Pubkey>,
    pub last_admin_update: u64,
    pub emergency_action_scheduled_time: u64, // Added for emergency timelock
}

impl PoolState {
    pub fn calculate_space() -> usize {
        8 + // discriminator
        1 + // version
        32 + // admin
        32 + // token_mint
        1 + // token_decimals
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
        8 + // last_update
        1 + // is_locked
        48 + // pending_update
        8 + // volume_24h
        8 + // last_volume_update
        32 + // emergency_admin
        (4 + MAX_FEE_TIERS * 16) + // fee_tiers
        8 + // max_daily_volume
        8 + // max_price_impact_bps
        8 + // circuit_breaker_threshold
        8 + // circuit_breaker_window
        8 + // circuit_breaker_cooldown
        8 + // last_circuit_breaker
        8 + // rate_limit_window
        4 + // rate_limit_count
        4 + // rate_limit_max
        8 + // last_rate_limit_reset
        (4 + MAX_BLACKLIST_SIZE * 32) + // trader_blacklist
        8 + // last_admin_update
        8 // emergency_action_scheduled_time
    }
}

// Other structs and events unchanged for brevity

#[event]
pub struct ParameterUpdateCancelled {
    pub pool: Pubkey,
    pub admin: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct EmergencyPauseScheduled {
    pub pool: Pubkey,
    pub emergency_admin: Pubkey,
    pub scheduled_time: i64,
}

#[event]
pub struct EmergencyResumeScheduled {
    pub pool: Pubkey,
    pub emergency_admin: Pubkey,
    pub scheduled_time: i64,
}

#[event]
pub struct BatchBlacklistCompleted {
    pub pool: Pubkey,
    pub admin: Pubkey,
    pub count: u64,
    pub timestamp: i64,
}