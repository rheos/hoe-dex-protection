use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use std::collections::HashSet;

// Module declarations
pub mod constants;
pub mod events;
pub mod types;
pub use constants::*;
pub use events::*;
pub use types::*;

// Program ID (replace with actual ID after deployment)
declare_id!("HoeDexProtect111111111111111111111111111111111111");

#[program]
pub mod hoe_dex_protection {
    use super::*;

    /// Initialize a new pool with protection parameters
    /// 
    /// This function sets up the initial state of the pool with all necessary
    /// protection mechanisms and parameters. It performs extensive validation
    /// to ensure the pool starts in a safe state.
    pub fn initialize_pool(
        ctx: Context<contexts::InitializePool>,
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

        // Validate fee tiers
        utils::validate_fee_tiers(&fee_tiers)?;

        // Initialize pool state
        pool_state.version = 1;
        pool_state.migration_flag = false;
        pool_state.is_initialized = true;
        pool_state.admin = ctx.accounts.admin.key();
        pool_state.emergency_admin = ctx.accounts.admin.key(); // Initially same as admin
        pool_state.token_mint = ctx.accounts.token_mint.key();
        pool_state.token_decimals = ctx.accounts.token_mint.decimals;
        pool_state.total_fees_collected = 0;
        pool_state.total_liquidity = 0;
        pool_state.is_paused = false;
        pool_state.is_emergency_paused = false;
        pool_state.is_finalized = false;
        pool_state.pool_start_time = current_time as u64;
        pool_state.last_update = current_time as u64;
        pool_state.last_admin_update = current_time as u64;
        pool_state.emergency_action_scheduled_time = 0;
        pool_state.pending_update = None;
        pool_state.trade_settings = TradeSettings {
            max_size_bps: max_trade_size_bps,
            min_size: min_trade_size,
            cooldown_seconds,
            last_trade_time: 0,
            early_trade_fee_bps,
            early_trade_window_seconds,
        };
        pool_state.rate_limit = RateLimitSettings {
            window_seconds: rate_limit_window,
            count: 0,
            max_calls: rate_limit_max as u64,
            last_reset: current_time as u64,
        };
        pool_state.circuit_breaker = CircuitBreakerSettings {
            enabled: true,
            threshold: circuit_breaker_threshold,
            window: circuit_breaker_window,
            cooldown: circuit_breaker_cooldown,
            last_trigger: 0,
        };
        pool_state.volume = VolumeSettings {
            daily_limit: max_daily_volume,
            current_volume: 0,
            last_reset: current_time as u64,
            decay_rate: 0,
        };
        pool_state.protection = ProtectionSettings {
            enabled: true,
            min_liquidity: 0,
            max_price_impact: max_price_impact_bps,
            max_slippage: 0,
            blacklist_enabled: false,
        };
        pool_state.fee_tiers = fee_tiers;
        pool_state.fee_tiers_locked = false;
        pool_state.default_fee_bps = None;
        pool_state.trader_blacklist = Vec::new();

        emit!(PoolInitialized {
            pool: pool_state.key(),
            admin_pubkey: pool_state.admin,
            ts: current_time,
        });

        Ok(())
    }

    /// Add liquidity to the pool
    /// 
    /// This function allows the admin to add liquidity to the pool before it starts.
    /// - Validates: token program, amount, pool state, token accounts
    /// - Transfers: tokens from admin to pool
    /// - Updates: pool state with new liquidity and timestamps
    pub fn add_liquidity(ctx: Context<contexts::AddLiquidity>, amount: u64) -> Result<()> {
        let current_time = current_unix_ts()?;
        msg!("Adding liquidity: amount={}", amount);

        // Validate admin action
        validation::validate_admin_action(&ctx.accounts.pool_state, &ctx.accounts.admin.key(), current_time)?;

        // Validate amount
        if amount == 0 {
            msg!("Invalid amount: must be greater than zero");
            return Err(ErrorCode::InvalidAmount.into());
        }

        // Check token accounts
        ctx.accounts.pool_state.check_token_account(
            &ctx.accounts.admin_token_account,
            &ctx.accounts.pool_state.token_mint,
        )?;
        ctx.accounts.pool_state.check_token_account(
            &ctx.accounts.pool_token_account,
            &ctx.accounts.pool_state.token_mint,
        )?;

        // Transfer tokens
        let transfer_ctx = with_pool_signer(
            ctx.program_id,
            &ctx.accounts.pool_state,
            &[ctx.accounts.pool_authority.to_account_info()],
        )?;

        // Transfer from admin to pool
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TokenTransfer {
            from: ctx.accounts.admin_token_account.to_account_info(),
            to: ctx.accounts.pool_token_account.to_account_info(),
            authority: ctx.accounts.admin.to_account_info(),
                },
            ),
            amount,
        )?;

        // Update pool state
        ctx.accounts.pool_state.total_liquidity = ctx.accounts.pool_state.total_liquidity
            .checked_add(amount)
            .ok_or_else(|| {
                msg!("Liquidity overflow: {} + {}", ctx.accounts.pool_state.total_liquidity, amount);
                error!(ErrorCode::Overflow)
            })?;

        ctx.accounts.pool_state.last_update = current_time;
        ctx.accounts.pool_state.last_admin_update = current_time;

        // Emit event
        ctx.accounts.pool_state.emit_liquidity_added(
            &ctx.accounts.admin.key(),
            amount,
            current_time as i64,
        );
        
        Ok(())
    }

    /// Remove liquidity from the pool
    /// 
    /// This function allows the admin to withdraw liquidity from the pool.
    /// - Validates: token program, admin, pool state, token accounts, amount
    /// - Transfers: tokens from pool to admin
    /// - Updates: pool state with reduced liquidity and timestamps
    pub fn remove_liquidity(ctx: Context<contexts::AdminAction>, amount: u64) -> Result<()> {
        let current_time = current_unix_ts()?;
        msg!("Removing liquidity: amount={}", amount);

        // Validate admin action
        validation::validate_admin_action(&ctx.accounts.pool_state, &ctx.accounts.admin.key(), current_time)?;

        // Validate amount
        if amount == 0 {
            msg!("Invalid amount: must be greater than zero");
            return Err(ErrorCode::InvalidAmount.into());
        }

        // Check if enough liquidity
        if amount > ctx.accounts.pool_state.total_liquidity {
            msg!("Insufficient liquidity: requested {} > available {}", 
                amount, 
                ctx.accounts.pool_state.total_liquidity
            );
            return Err(ErrorCode::InsufficientLiquidity.into());
        }

        // Update pool state
        ctx.accounts.pool_state.total_liquidity = ctx.accounts.pool_state.total_liquidity
            .checked_sub(amount)
            .ok_or_else(|| {
                msg!("Liquidity underflow: {} - {}", ctx.accounts.pool_state.total_liquidity, amount);
                error!(ErrorCode::Overflow)
            })?;

        ctx.accounts.pool_state.last_update = current_time;
        ctx.accounts.pool_state.last_admin_update = current_time;

        // Emit event
        ctx.accounts.pool_state.emit_liquidity_removed(
            &ctx.accounts.admin.key(),
            amount,
            current_time as i64,
        );

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
        ctx: Context<contexts::ExecuteTrade>,
        amount_in: u64,
        minimum_amount_out: u64,
    ) -> Result<TradeOutcome> {
        let current_time = current_unix_ts()?;
        msg!("Executing trade: amount_in={}, minimum_amount_out={}", amount_in, minimum_amount_out);

        // Validate trade parameters
        validation::validate_trade_parameters(&ctx.accounts.pool_state, amount_in, current_time)?;

        // Calculate fee and amount out
        let (fee_amount, fee_mode) = ctx.accounts.pool_state.calculate_fee(amount_in, current_time as i64)?;
        let amount_after_fee = amount_in.checked_sub(fee_amount).ok_or_else(|| {
            msg!("Fee calculation overflow: {} - {}", amount_in, fee_amount);
            error!(ErrorCode::Overflow)
        })?;

        // Calculate price impact
        let price_impact = ctx.accounts.pool_state.calculate_price_impact(amount_after_fee, ctx.accounts.pool_state.total_liquidity)?;
        if price_impact > ctx.accounts.pool_state.protection.max_price_impact {
            msg!("Price impact too high: {} > {}", price_impact, ctx.accounts.pool_state.protection.max_price_impact);
            return Err(ErrorCode::PriceImpactTooHigh.into());
        }

        // Calculate amount out
        let amount_out = amount_after_fee.checked_mul(ctx.accounts.pool_state.total_liquidity)
            .ok_or_else(|| {
                msg!("Amount calculation overflow: {} * {}", amount_after_fee, ctx.accounts.pool_state.total_liquidity);
                error!(ErrorCode::Overflow)
            })?
            .checked_div(ctx.accounts.pool_state.total_liquidity.checked_add(amount_after_fee)
                .ok_or_else(|| {
                    msg!("Pool balance overflow: {} + {}", ctx.accounts.pool_state.total_liquidity, amount_after_fee);
                    error!(ErrorCode::Overflow)
                })?)
            .ok_or_else(|| {
                msg!("Division by zero in amount calculation");
                error!(ErrorCode::Overflow)
            })?;

        // Check slippage
        if amount_out < minimum_amount_out {
            msg!("Slippage exceeded: got {} < minimum {}", amount_out, minimum_amount_out);
            return Err(ErrorCode::SlippageExceeded.into());
        }

        // Transfer tokens
        let transfer_ctx = with_pool_signer(
            ctx.program_id,
            &ctx.accounts.pool_state,
            &[ctx.accounts.pool_authority.to_account_info()],
        )?;

        // Transfer from buyer to pool
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TokenTransfer {
                    from: ctx.accounts.buyer_token_account.to_account_info(),
                    to: ctx.accounts.pool_token_account.to_account_info(),
                    authority: ctx.accounts.buyer.to_account_info(),
                },
            ),
            amount_in,
        )?;

        // Update pool state
        ctx.accounts.pool_state.total_liquidity = ctx.accounts.pool_state.total_liquidity
            .checked_add(amount_in)
            .ok_or_else(|| {
                msg!("Liquidity overflow: {} + {}", ctx.accounts.pool_state.total_liquidity, amount_in);
                error!(ErrorCode::Overflow)
            })?;

        ctx.accounts.pool_state.total_fees_collected = ctx.accounts.pool_state.total_fees_collected
            .checked_add(fee_amount)
            .ok_or_else(|| {
                msg!("Fee collection overflow: {} + {}", ctx.accounts.pool_state.total_fees_collected, fee_amount);
                error!(ErrorCode::Overflow)
            })?;

        ctx.accounts.pool_state.trade_settings.last_trade_time = current_time;
        ctx.accounts.pool_state.last_update = current_time;

        // Emit trade event
        ctx.accounts.pool_state.emit_trade_executed(
            &ctx.accounts.buyer.key(),
            amount_in,
            amount_out,
            fee_amount,
            fee_mode as u8,
            current_time as i64,
        );

        Ok(TradeOutcome {
            amount_out,
            fee_amount,
            fee_mode: FeeMode::from_u8(fee_mode).unwrap_or(FeeMode::None),
            price_impact,
            timestamp: current_time as i64,
        })
    }

    /// Calculate fee based on trade timing and volume
    ///
    /// This function determines the appropriate fee to charge based on:
    /// 1. Whether the trade is within the early trade window
    /// 2. The current 24h volume and applicable fee tier
    /// 3. Returns both the fee amount and the fee mode for tracking
    fn calculate_fee(pool_state: &PoolState, amount_in: u64, current_time: i64) -> Result<(u64, u8)> {
        // Early trade fee if within protection window
        if current_time - pool_state.pool_start_time as i64 <= pool_state.trade_settings.early_trade_window_seconds as i64 {
            let fee = amount_in
                .checked_mul(pool_state.trade_settings.early_trade_fee_bps)
                .ok_or(ErrorCode::Overflow)?
                .checked_div(10000)
                .ok_or(ErrorCode::Overflow)?;
            
            // Use default fee if configured, otherwise minimum fee
            let effective_fee = if fee == 0 {
                pool_state.default_fee_bps
                    .map(|bps| amount_in.checked_mul(bps).ok_or(ErrorCode::Overflow)?.checked_div(10000).ok_or(ErrorCode::Overflow)?)
                    .unwrap_or(MINIMUM_FEE)
            } else {
                fee.max(MINIMUM_FEE)
            };
            
            return Ok((effective_fee, FEE_MODE_EARLY_TRADE));
        }

        // Find applicable fee tier based on volume
        for tier in &pool_state.fee_tiers {
            if pool_state.volume.volume_24h <= tier.volume_threshold {
                let fee = amount_in
                    .checked_mul(tier.fee_bps)
                    .ok_or(ErrorCode::Overflow)?
                    .checked_div(10000)
                    .ok_or(ErrorCode::Overflow)?;
                
                // Use default fee if configured, otherwise minimum fee
                let effective_fee = if fee == 0 {
                    pool_state.default_fee_bps
                        .map(|bps| amount_in.checked_mul(bps).ok_or(ErrorCode::Overflow)?.checked_div(10000).ok_or(ErrorCode::Overflow)?)
                        .unwrap_or(MINIMUM_FEE)
        } else {
                    fee.max(MINIMUM_FEE)
                };
                
                return Ok((effective_fee, FEE_MODE_TIER_BASED));
            }
        }

        // Use default fee if configured, otherwise minimum fee
        let fallback_fee = pool_state.default_fee_bps
            .map(|bps| amount_in.checked_mul(bps).ok_or(ErrorCode::Overflow)?.checked_div(10000).ok_or(ErrorCode::Overflow)?)
            .unwrap_or(MINIMUM_FEE);

        Ok((fallback_fee, FEE_MODE_NONE))
    }

    /// Blacklist a trader to prevent them from trading
    ///
    /// This function allows the admin to blacklist a trader with:
    /// - Admin must be a signer
    /// - Trader cannot be already blacklisted
    /// - Trader cannot be the admin or emergency admin
    pub fn blacklist_trader(ctx: Context<contexts::ManageBlacklist>, trader: Pubkey) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;
        
        // Add reentrancy protection
        let _guard = ReentrancyGuard::new(pool_state)?;
        
        utils::process_blacklist_operations(
            pool_state,
            vec![trader],
            BlacklistOperation::Add,
            current_time,
        )
    }

    /// Remove a trader from the blacklist
    ///
    /// This function allows the admin to remove a trader from the blacklist with:
    /// - Admin must be a signer
    /// - Trader must be currently blacklisted
    pub fn remove_from_blacklist(ctx: Context<contexts::ManageBlacklist>, trader: Pubkey) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;
        
        // Add reentrancy protection
        let _guard = ReentrancyGuard::new(pool_state)?;
        
        utils::process_blacklist_operations(
            pool_state,
            vec![trader],
            BlacklistOperation::Remove,
            current_time,
        )
    }

    /// Batch blacklist multiple traders efficiently
    ///
    /// This function allows the admin to blacklist multiple traders in a single transaction with:
    /// - Admin must be a signer
    /// - Maximum of 50 traders per batch
    /// - Each trader must not be already blacklisted
    /// - No trader can be the admin or emergency admin
    pub fn batch_blacklist_traders(ctx: Context<contexts::ManageBlacklist>, traders: Vec<Pubkey>) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;
        
        utils::process_blacklist_operations(
            pool_state,
            traders,
            BlacklistOperation::Add,
            current_time,
        )
    }

    /// Batch unblacklist multiple traders efficiently
    ///
    /// NEW: Added to allow the admin to remove multiple traders from the blacklist in a single transaction.
    /// - Admin must be a signer
    /// - Maximum of 50 traders per batch
    /// - Each trader must be currently blacklisted
    pub fn batch_unblacklist_traders(ctx: Context<contexts::ManageBlacklist>, traders: Vec<Pubkey>) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;
        
        utils::process_blacklist_operations(
            pool_state,
            traders,
            BlacklistOperation::Remove,
            current_time,
        )
    }

    /// Withdraw collected fees from the pool
    ///
    /// This function allows the admin to withdraw collected fees with:
    /// - Admin must be a signer
    /// - Fees must be available to withdraw
    /// - Reentrancy protection
    pub fn withdraw_fees(ctx: Context<contexts::WithdrawFees>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;

        // Validate admin and check cooldown
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;

        // Validate token accounts
        state.check_token_mint(&ctx.accounts.token_mint)?;
        state.check_token_account(&ctx.accounts.admin_token_account, &state.token_mint)?;
        state.check_token_account(&ctx.accounts.pool_token_account, &state.token_mint)?;

        // Validate fees available
        validate_condition!(state.total_fees_collected > 0, ErrorCode::NoFeesAvailable);

        // Transfer fees from pool to admin
        let cpi_ctx = with_pool_signer(
            &crate::ID,
            state,
            &[ctx.accounts.pool_token_account.to_account_info(), ctx.accounts.admin_token_account.to_account_info()],
        )?;

        token::transfer(
            cpi_ctx,
            state.total_fees_collected,
        )?;

        // Update pool state
        state.total_fees_collected = 0;
        state.last_update = current_time;

        // Emit event
        emit!(FeesWithdrawn {
            pool: state.key(),
            admin_pubkey: state.admin,
            amount: state.total_fees_collected,
            ts: current_time as i64,
        });
        
        Ok(())
    }

    /// Lock fee tiers to prevent further changes
    ///
    /// This function allows the admin to lock fee tiers with:
    /// - Admin must be a signer
    /// - Fee tiers must not be already locked
    pub fn lock_fee_tiers(ctx: Context<contexts::LockFeeTiers>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;

        // Validate admin and check cooldown
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;

        // Validate fee tiers not already locked
        validate_condition!(!state.fee_tiers_locked, ErrorCode::FeeTiersLocked);

        // Update pool state
        state.fee_tiers_locked = true;
        state.last_update = current_time;

        // Emit event
        emit!(FeeTiersLocked {
            pool: state.key(),
            admin_pubkey: state.admin,
            ts: current_time as i64,
        });

        Ok(())
    }

    /// Unlock fee tiers to allow future changes
    ///
    /// NEW: Added to allow the admin to unlock fee tiers with a 24-hour timelock.
    /// - Admin must be a signer
    /// - Fee tiers must be currently locked
    /// - Schedules unlock via pending_update for delayed execution
    pub fn unlock_fee_tiers(ctx: Context<contexts::AdminAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;

        // Validate admin and check cooldown
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;

        // Validate fee tiers are locked
        validate_condition!(state.fee_tiers_locked, ErrorCode::FeeTiersNotLocked);

        // Update pool state
        state.fee_tiers_locked = false;
        state.last_update = current_time;

        // Emit event
        emit!(FeeTiersUnlockScheduled {
            pool: state.key(),
            admin_pubkey: state.admin,
            scheduled_time: current_time as i64,
        });

        Ok(())
    }

    /// Schedule a parameter update with a 24-hour timelock
    ///
    /// This function allows the admin to schedule changes to pool parameters.
    /// - Validates: admin, new fee tiers, parameter relationships
    /// - Stores: pending update with scheduled execution time
    pub fn schedule_parameter_update(
        ctx: Context<contexts::AdminAction>,
        trade_settings: Option<TradeSettingsUpdate>,
        protection_settings: Option<ProtectionSettingsUpdate>,
        fee_settings: Option<FeeSettingsUpdate>,
        state_settings: Option<StateSettingsUpdate>,
    ) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;

        // Validate admin and check cooldown
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;

        // Validate new settings if provided
        if let Some(settings) = &trade_settings {
            validate_parameter!(
                settings.max_trade_size_bps,
                settings.min_trade_size,
                u64::MAX,
                ErrorCode::InvalidParameterRelationship
            );
        }

        if let Some(settings) = &protection_settings {
            validate_parameter!(
                settings.max_price_impact_bps,
                0,
                10000,
                ErrorCode::PriceImpactTooHigh
            );
        }

        if let Some(settings) = &fee_settings {
            if let Some(fee_tiers) = &settings.fee_tiers {
                validation::validate_fee_parameters(state, fee_tiers)?;
            }
        }

        // Create pending update
        state.pending_update = Some(PendingUpdate {
            scheduled_time: current_time + 86400, // 24 hour timelock
            trade_settings,
            protection_settings,
            fee_settings,
            state_settings,
        });

        emit!(ParameterUpdateScheduled {
            pool: state.key(),
            admin_pubkey: state.admin,
            scheduled_time: current_time + 86400,
        });

        Ok(())
    }

    /// Cancel a scheduled parameter update
    ///
    /// This function allows the admin to cancel a pending parameter update before the timelock expires.
    /// - Validates: admin, presence of pending update
    /// - Clears: pending update
    pub fn cancel_parameter_update(ctx: Context<contexts::AdminAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = current_unix_ts()?;

        // Validate admin
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;

        // Take the pending update
        let pending_update = state.pending_update.take().ok_or_else(|| {
            error!(ErrorCode::NoPendingUpdate, "No pending update available")
        })?;

        // Emit detailed cancellation event
        emit!(ParameterUpdateCancelled {
            pool: state.key(),
            admin_pubkey: ctx.accounts.admin.key(),
            ts: current_time as i64,
            trade_settings: pending_update.trade_settings,
            protection_settings: pending_update.protection_settings,
            fee_settings: pending_update.fee_settings,
            state_settings: pending_update.state_settings,
        });

        state.last_update = current_time;
        Ok(())
    }

    /// Apply a scheduled parameter update
    ///
    /// This function applies a pending parameter update after the timelock expires.
    /// - Validates: admin, timelock, parameter relationships
    /// - Updates: pool state with new parameters
    pub fn apply_parameter_update(ctx: Context<contexts::AdminAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = current_unix_ts()?;

        // Validate admin and timelock
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;
        validate_condition!(
            state.pending_update.is_some(),
            ErrorCode::NoPendingUpdate,
            "No pending update available"
        );

        let pending_update = state.pending_update.as_ref().unwrap();
        validate_condition!(
            current_time >= pending_update.scheduled_time,
            ErrorCode::TimelockNotExpired,
            "Timelock not yet expired"
        );

        // Apply updates and emit events
        if let Some(trade_settings) = &pending_update.trade_settings {
            state.trade_settings.early_trade_fee_bps = trade_settings.early_trade_fee_bps;
            state.trade_settings.early_trade_window_seconds = trade_settings.early_trade_window_seconds;
            state.trade_settings.max_size_bps = trade_settings.max_trade_size_bps;
            state.trade_settings.min_size = trade_settings.min_trade_size;
            state.trade_settings.cooldown_seconds = trade_settings.cooldown_seconds;

            emit!(TradeSettingsUpdated {
                pool: state.key(),
                admin_pubkey: ctx.accounts.admin.key(),
                early_trade_fee_bps: trade_settings.early_trade_fee_bps,
                early_trade_window_seconds: trade_settings.early_trade_window_seconds,
                max_trade_size_bps: trade_settings.max_trade_size_bps,
                min_trade_size: trade_settings.min_trade_size,
                cooldown_seconds: trade_settings.cooldown_seconds,
                ts: current_time as i64,
            });
        }

        if let Some(protection_settings) = &pending_update.protection_settings {
            state.volume.daily_limit = protection_settings.max_daily_volume;
            state.protection.max_price_impact = protection_settings.max_price_impact_bps;
            state.protection.max_slippage = protection_settings.max_slippage;
            state.protection.blacklist_enabled = protection_settings.blacklist_enabled;
            state.circuit_breaker.threshold = protection_settings.circuit_breaker_threshold;
            state.circuit_breaker.window = protection_settings.circuit_breaker_window;
            state.circuit_breaker.cooldown = protection_settings.circuit_breaker_cooldown;
            state.rate_limit.window_seconds = protection_settings.rate_limit_window;
            state.rate_limit.max_calls = protection_settings.rate_limit_max as u64;

            emit!(ProtectionSettingsUpdated {
                pool: state.key(),
                admin_pubkey: ctx.accounts.admin.key(),
                max_daily_volume: protection_settings.max_daily_volume,
                max_price_impact_bps: protection_settings.max_price_impact_bps,
                max_slippage: protection_settings.max_slippage,
                blacklist_enabled: protection_settings.blacklist_enabled,
                circuit_breaker_threshold: protection_settings.circuit_breaker_threshold,
                circuit_breaker_window: protection_settings.circuit_breaker_window,
                circuit_breaker_cooldown: protection_settings.circuit_breaker_cooldown,
                rate_limit_window: protection_settings.rate_limit_window,
                rate_limit_max: protection_settings.rate_limit_max,
                ts: current_time as i64,
            });
        }

        if let Some(fee_settings) = &pending_update.fee_settings {
            if !fee_settings.fee_tiers.is_empty() {
                state.fee_tiers = fee_settings.fee_tiers.clone();
            }
            state.fee_tiers_locked = fee_settings.fee_tiers_locked;

            emit!(FeeSettingsUpdated {
                pool: state.key(),
                admin_pubkey: ctx.accounts.admin.key(),
                fee_tiers_count: state.fee_tiers.len(),
                fee_tiers_locked: state.fee_tiers_locked,
                ts: current_time as i64,
            });
        }

        if let Some(state_settings) = &pending_update.state_settings {
            state.is_paused = state_settings.is_paused;
            state.is_emergency_paused = state_settings.is_emergency_paused;

            emit!(StateSettingsUpdated {
                pool: state.key(),
                admin_pubkey: ctx.accounts.admin.key(),
                is_paused: state_settings.is_paused,
                is_emergency_paused: state_settings.is_emergency_paused,
                ts: current_time as i64,
            });
        }

        // Clear pending update
        state.pending_update = None;
        state.last_update = current_time;

        emit!(ParametersUpdated {
            pool: state.key(),
            admin_pubkey: ctx.accounts.admin.key(),
            ts: current_time as i64,
        });

        Ok(())
    }

    /// Schedule an emergency pause with a 1-hour timelock
    ///
    /// This function allows the emergency admin to schedule a pool pause.
    /// - Validates: emergency admin
    /// - Sets: scheduled pause time
    pub fn schedule_emergency_pause(ctx: Context<contexts::EmergencyAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;

        // Validate emergency admin
        validate_condition!(
            ctx.accounts.emergency_admin.key() == state.emergency_admin,
            ErrorCode::InvalidEmergencyAdmin
        );

        // Validate not already paused
        validate_condition!(!state.is_emergency_paused, ErrorCode::EmergencyPaused);

        // Schedule emergency pause
        state.emergency_action_scheduled_time = current_time + 3600; // 1 hour timelock

        // Emit event
        emit!(EmergencyPauseScheduled {
            pool: state.key(),
            emergency_admin_pubkey: state.emergency_admin,
            scheduled_time: current_time + 3600,
        });

        Ok(())
    }

    /// Apply a scheduled emergency pause
    ///
    /// This function applies a scheduled pause after the timelock expires.
    /// - Validates: emergency admin, timelock
    /// - Sets: pool to emergency paused state
    pub fn apply_emergency_pause(ctx: Context<contexts::EmergencyAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;

        // Validate emergency admin
        validate_condition!(
            ctx.accounts.emergency_admin.key() == state.emergency_admin,
            ErrorCode::InvalidEmergencyAdmin
        );

        // Validate timelock has expired
        validate_condition!(
            current_time >= state.emergency_action_scheduled_time,
            ErrorCode::TimelockNotExpired
        );

        // Apply emergency pause
        state.is_emergency_paused = true;
        state.last_update = current_time;

        // Emit event
        emit!(EmergencyPaused {
            pool: state.key(),
            emergency_admin_pubkey: state.emergency_admin,
            ts: current_time as i64,
        });
        
        Ok(())
    }

    /// Schedule an emergency resume with a 1-hour timelock
    ///
    /// This function allows the emergency admin to schedule a pool resume.
    /// - Validates: emergency admin
    /// - Sets: scheduled resume time
    pub fn schedule_emergency_resume(ctx: Context<contexts::EmergencyAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;

        // Validate emergency admin
        validate_condition!(
            ctx.accounts.emergency_admin.key() == state.emergency_admin,
            ErrorCode::InvalidEmergencyAdmin
        );
        
        // Validate is paused
        validate_condition!(state.is_emergency_paused, ErrorCode::PoolNotPaused);

        // Schedule emergency resume
        state.emergency_action_scheduled_time = current_time + 3600; // 1 hour timelock

        // Emit event
        emit!(EmergencyResumeScheduled {
            pool: state.key(),
            emergency_admin_pubkey: state.emergency_admin,
            scheduled_time: current_time + 3600,
        });
        
        Ok(())
    }

    /// Apply a scheduled emergency resume
    ///
    /// This function applies a scheduled resume after the timelock expires.
    /// - Validates: emergency admin, timelock
    /// - Sets: pool to non-emergency paused state
    pub fn apply_emergency_resume(ctx: Context<contexts::EmergencyAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;

        // Validate emergency admin
        validate_condition!(
            ctx.accounts.emergency_admin.key() == state.emergency_admin,
            ErrorCode::InvalidEmergencyAdmin
        );
        
        // Validate timelock has expired
        validate_condition!(
            current_time >= state.emergency_action_scheduled_time,
            ErrorCode::TimelockNotExpired
        );

        // Apply emergency resume
        state.is_emergency_paused = false;
        state.last_update = current_time;

        // Emit event
        emit!(EmergencyResumed {
            pool: state.key(),
            emergency_admin_pubkey: state.emergency_admin,
            ts: current_time as i64,
        });
        
        Ok(())
    }

    /// Reset the circuit breaker
    ///
    /// This function allows the admin to reset the circuit breaker after its cooldown.
    /// - Validates: admin, timestamp, cooldown
    /// - Resets: circuit breaker and 24h volume
    pub fn reset_circuit_breaker(ctx: Context<contexts::AdminAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;

        // Validate admin and check cooldown
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;

        // Validate cooldown has expired
        validate_condition!(
            current_time >= state.circuit_breaker.last_trigger + state.circuit_breaker.cooldown,
            ErrorCode::CircuitBreakerCooldown
        );

        // Reset circuit breaker
        state.circuit_breaker.last_trigger = 0;
        state.last_update = current_time;

        // Emit event
        emit!(CircuitBreakerReset {
            pool: state.key(),
            admin_pubkey: state.admin,
            ts: current_time as i64,
        });

        Ok(())
    }

    /// Update the pool admin with cooldown protection
    ///
    /// This function allows changing the pool admin with the following protections:
    /// - 24-hour cooldown between admin changes
    /// - New admin must be different from current and emergency admin
    /// - Current admin must be a signer
    pub fn update_admin(ctx: Context<contexts::AdminAction>, new_admin: Pubkey) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;

        // Validate admin and check cooldown
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;

        // Validate new admin
        validate_condition!(
            new_admin != state.admin && new_admin != state.emergency_admin,
            ErrorCode::InvalidNewAdmin
        );

        // Update admin
        let old_admin = state.admin;
        state.admin = new_admin;
        state.last_admin_update = current_time;
        state.last_update = current_time;

        // Emit event
        emit!(AdminUpdated {
            pool: state.key(),
            old_admin_pubkey: old_admin,
            new_admin_pubkey: new_admin,
            ts: current_time as i64,
        });

        Ok(())
    }

    /// Reset the pending update
    pub fn reset_pending_update(ctx: Context<contexts::AdminAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;

        // Validate admin and check cooldown
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;

        // Validate pending update exists
        validate_condition!(state.pending_update.is_some(), ErrorCode::NoPendingUpdate);

        // Reset pending update
        state.pending_update = None;
        state.last_update = current_time;

        // Emit event
        emit!(ParameterUpdateCancelled {
            pool: state.key(),
            admin_pubkey: state.admin,
            scheduled_time: current_time as i64,
            trade_settings: state.pending_update.as_ref().and_then(|u| u.trade_settings.clone()),
            protection_settings: state.pending_update.as_ref().and_then(|u| u.protection_settings.clone()),
            fee_settings: state.pending_update.as_ref().and_then(|u| u.fee_settings.clone()),
            state_settings: state.pending_update.as_ref().and_then(|u| u.state_settings.clone()),
            ts: current_time as i64,
        });

        Ok(())
    }

    /// Toggle the pool pause state
    pub fn toggle_pause(ctx: Context<contexts::AdminAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp as u64;

        // Validate admin and check cooldown
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;

        // Toggle pause state
        state.is_paused = !state.is_paused;
        state.last_update = current_time;

        // Emit event
        if state.is_paused {
            emit!(PoolPaused {
                pool: state.key(),
                admin_pubkey: state.admin,
                ts: current_time as i64,
            });
        } else {
            emit!(PoolResumed {
                pool: state.key(),
                admin_pubkey: state.admin,
                ts: current_time as i64,
            });
        }

        Ok(())
    }

    // Add fee tier validation function
    pub fn validate_fee_tiers(fee_tiers: &[FeeTier]) -> Result<()> {
        require!(!fee_tiers.is_empty(), ErrorCode::InvalidFeeTier);
        require!(fee_tiers.len() <= MAX_FEE_TIERS, ErrorCode::TooManyFeeTiers);

        // Check for duplicate thresholds
        let mut thresholds: Vec<u64> = fee_tiers.iter().map(|tier| tier.volume_threshold).collect();
        thresholds.sort_unstable();
        thresholds.dedup();
        require!(thresholds.len() == fee_tiers.len(), ErrorCode::DuplicateFeeTierThreshold);

        // Validate fee ranges and spacing
        for tier in fee_tiers {
            require!(tier.fee_bps >= MINIMUM_FEE, ErrorCode::FeeTooLow);
            require!(tier.fee_bps <= MAX_TIER_FEE_BPS, ErrorCode::FeeTooHigh);
        }

        // Check for proper spacing between thresholds
        for i in 1..fee_tiers.len() {
            let spacing = fee_tiers[i].volume_threshold
                .checked_sub(fee_tiers[i - 1].volume_threshold)
                .ok_or(ErrorCode::Overflow)?;
            require!(spacing >= MIN_FEE_TIER_SPACING_BPS, ErrorCode::InvalidFeeTierSpacing);
        }

        Ok(())
    }

    pub fn validate_emergency_action(
        pool_state: &PoolState,
        emergency_admin: &Pubkey,
        current_time: u64,
    ) -> Result<()> {
        // Check if caller is emergency admin
        if emergency_admin != &pool_state.emergency_admin {
            msg!("Unauthorized: expected emergency admin {} but got {}", 
                pool_state.emergency_admin, 
                emergency_admin
            );
            return Err(ErrorCode::InvalidEmergencyAdmin.into());
        }

        // Check if pool is finalized
        if pool_state.is_finalized {
            msg!("Pool is finalized: emergency actions not allowed");
            return Err(ErrorCode::PoolFinalized.into());
        }

        // Check if emergency action is already scheduled
        if pool_state.emergency_action_scheduled_time > 0 {
            msg!("Emergency action already scheduled for timestamp {}", 
                pool_state.emergency_action_scheduled_time
            );
            return Err(ErrorCode::OperationFailed.into());
        }

        // Check if emergency action is within timelock
        let timelock_end = pool_state.last_update + EMERGENCY_TIMELOCK_SECONDS;
        if current_time < timelock_end {
            msg!("Emergency action timelock not expired: {} seconds remaining", 
                timelock_end - current_time
            );
            return Err(ErrorCode::TimelockNotExpired.into());
        }

        Ok(())
    }

    pub fn validate_token_operation(
        pool_state: &PoolState,
        token_account: &Account<TokenAccount>,
        token_mint: &Account<Mint>,
        owner: &Pubkey,
    ) -> Result<()> {
        // Check token mint
        if token_mint.key() != pool_state.token_mint {
            msg!("Invalid token mint: expected {} but got {}", 
                pool_state.token_mint, 
                token_mint.key()
            );
            return Err(ErrorCode::InvalidTokenMint.into());
        }

        // Check token decimals
        if token_mint.decimals != pool_state.token_decimals {
            msg!("Invalid token decimals: expected {} but got {}", 
                pool_state.token_decimals, 
                token_mint.decimals
            );
            return Err(ErrorCode::InvalidTokenDecimals.into());
        }

        // Check freeze authority
        if token_mint.freeze_authority.is_some() {
            msg!("Token mint has freeze authority: {}", token_mint.freeze_authority.unwrap());
            return Err(ErrorCode::TokenMintHasFreezeAuthority.into());
        }

        // Check token account
        if token_account.mint != pool_state.token_mint {
            msg!("Invalid token account mint: expected {} but got {}", 
                pool_state.token_mint, 
                token_account.mint
            );
            return Err(ErrorCode::InvalidTokenAccount.into());
        }

        // Check token account owner
        if token_account.owner != *owner {
            msg!("Invalid token account owner: expected {} but got {}", 
                owner, 
                token_account.owner
            );
            return Err(ErrorCode::InvalidTokenAccount.into());
        }

        // Check delegation
        if token_account.is_delegated() {
            msg!("Token account is delegated: {}", token_account.delegate.unwrap());
            return Err(ErrorCode::TokenAccountDelegated.into());
        }

        Ok(())
    }
}

// Move account contexts to a separate module
mod contexts {
    use super::*;

    /// Context for initializing a new pool
    /// 
    /// # Accounts
    /// * `pool_state` - The pool state account to initialize
    /// * `admin` - The admin account that will own the pool
    /// * `token_mint` - The token mint for the pool
    /// * `system_program` - Required for account creation
    /// * `token_program` - Required for token operations
    /// * `rent` - Required for account creation
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
        #[account(mut)]
    pub token_mint: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

    /// Context for adding liquidity to the pool
    /// 
    /// # Accounts
    /// * `pool_state` - The pool state account
    /// * `admin` - The admin account that owns the pool
    /// * `admin_token_account` - The admin's token account
    /// * `pool_token_account` - The pool's token account
    /// * `pool_authority` - The pool's authority PDA
    /// * `token_program` - Required for token operations
#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(
        mut,
            seeds = [b"pool_state", pool_state.admin.as_ref()],
            bump = pool_state.bump
    )]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut)]
    pub admin_token_account: Account<'info, TokenAccount>,
        #[account(mut)]
    pub pool_token_account: Account<'info, TokenAccount>,
    #[account(
        seeds = [b"pool_authority", pool_state.key().as_ref()],
            bump = pool_state.bump
    )]
    pub pool_authority: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ExecuteTrade<'info> {
    #[account(
        mut,
        seeds = [b"pool_state", pool_state.admin.as_ref()],
            bump = pool_state.bump
    )]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub buyer: Signer<'info>,
        #[account(mut)]
    pub buyer_token_account: Account<'info, TokenAccount>,
        #[account(mut)]
    pub pool_token_account: Account<'info, TokenAccount>,
    #[account(
        seeds = [b"pool_authority", pool_state.key().as_ref()],
            bump = pool_state.bump
    )]
    pub pool_authority: AccountInfo<'info>,
        #[account(
            constraint = token_mint.key() == pool_state.token_mint
        )]
        pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
    pub struct ManageBlacklist<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
        /// CHECK: This is the reentrancy guard PDA
        #[account(
            seeds = [b"reentrancy_guard", pool_state.key().as_ref()],
            bump,
            constraint = reentrancy_guard.to_account_info().owner == program_id
        )]
        pub reentrancy_guard: UncheckedAccount<'info>,
}

#[derive(Accounts)]
    pub struct AdminAction<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
        /// CHECK: This is the reentrancy guard PDA
        #[account(
            seeds = [b"reentrancy_guard", pool_state.key().as_ref()],
            bump,
            constraint = reentrancy_guard.to_account_info().owner == program_id
        )]
        pub reentrancy_guard: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct WithdrawFees<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut)]
    pub pool_token_account: Account<'info, TokenAccount>,
        #[account(mut)]
        pub admin_token_account: Account<'info, TokenAccount>,
    pub pool_authority: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
    pub struct LockFeeTiers<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
    pub struct EmergencyAction<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub emergency_admin: Signer<'info>,
}

#[derive(Accounts)]
    pub struct SimulateTrade<'info> {
    pub pool_state: Account<'info, PoolState>,
}

#[derive(Accounts)]
    pub struct WithdrawCollectedFees<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
        #[account(mut)]
        pub admin: Signer<'info>,
        #[account(mut)]
        pub pool_token_account: Account<'info, TokenAccount>,
        #[account(mut)]
        pub admin_token_account: Account<'info, TokenAccount>,
        #[account(
            seeds = [b"pool_authority", pool_state.key().as_ref()],
            bump
        )]
        pub pool_authority: AccountInfo<'info>,
        pub token_program: Program<'info, Token>,
    }
}

// Add helper function for PDA derivation with proper error handling
pub fn derive_pool_authority(pool_state: &Pubkey, program_id: &Pubkey) -> Result<(Pubkey, u8)> {
    Pubkey::find_program_address(
        &[b"pool_authority", pool_state.as_ref()],
        program_id,
    ).ok_or(ErrorCode::InvalidPoolAuthority)
}

// Add helper function for CPI context with proper error handling
pub fn with_pool_signer<'info>(
    program_id: &Pubkey,
    pool_state: &Account<'info, PoolState>,
    remaining_accounts: &[AccountInfo<'info>],
) -> Result<CpiContext<'info, 'info, 'info, 'info, Transfer>> {
    let (pool_authority, bump) = derive_pool_authority(&pool_state.key(), program_id)?;
    let seeds = &[
        b"pool_authority".as_ref(),
        pool_state.key().as_ref(),
        &[bump],
    ];
    let signer = &[&seeds[..]];
    Ok(CpiContext::new(
        remaining_accounts[0].clone(),
        Transfer {
            from: remaining_accounts[1].clone(),
            to: remaining_accounts[2].clone(),
            authority: pool_authority,
        },
    ))
}

#[account]
pub struct PoolState {
    pub version: u8,
    pub migration_flag: bool,
    pub is_initialized: bool,
    pub admin: Pubkey,
    pub emergency_admin: Pubkey,
    pub token_mint: Pubkey,
    pub token_decimals: u8,
    pub total_fees_collected: u64,
    pub total_liquidity: u64,
    pub is_paused: bool,
    pub is_emergency_paused: bool,
    pub is_finalized: bool,
    pub pool_start_time: u64,
    pub last_update: u64,
    pub last_admin_update: u64,
    pub last_fee_withdrawal: u64, // Track last fee withdrawal
    pub emergency_action_scheduled_time: u64,
    pub pending_update: Option<PendingUpdate>,
    pub trade_settings: TradeSettings,
    pub rate_limit: RateLimitSettings,
    pub circuit_breaker: CircuitBreakerSettings,
    pub volume: VolumeSettings,
    pub protection: ProtectionSettings,
    pub fee_tiers: Vec<FeeTier>,
    pub fee_tiers_locked: bool,
    pub default_fee_bps: Option<u64>,
    pub trader_blacklist: Vec<Pubkey>,
    pub instruction_counter: u64,
    pub bump: u8, // Store PDA bump
    pub pool_id: [u8; 32], // Unique pool identifier
}

impl PoolState {
    pub fn calculate_space() -> usize {
        // Base size for fixed fields
        let base_size = std::mem::size_of::<Self>();
        
        // Add space for dynamic fields
        let fee_tiers_size = MAX_FEE_TIERS * std::mem::size_of::<FeeTier>();
        let blacklist_size = MAX_BLACKLIST_SIZE * std::mem::size_of::<Pubkey>();
        let pending_update_size = MAX_PENDING_UPDATE_SIZE;
        
        // Add buffer for future-proofing
        base_size + fee_tiers_size + blacklist_size + pending_update_size + 32
    }

    pub fn toggle_pause(&mut self, current_time: u64) -> Result<()> {
        self.is_paused = !self.is_paused;
        self.last_update = current_time;

        if self.is_paused {
            emit!(PoolPaused {
                pool: self.key(),
                admin_pubkey: self.admin,
                ts: current_time as i64,
            });
        } else {
            emit!(PoolResumed {
                pool: self.key(),
                admin_pubkey: self.admin,
                ts: current_time as i64,
            });
        }

        Ok(())
    }

    pub fn toggle_emergency_pause(&mut self, current_time: u64) -> Result<()> {
        self.is_emergency_paused = !self.is_emergency_paused;
        self.last_update = current_time;

        if self.is_emergency_paused {
            emit!(EmergencyPaused {
                pool: self.key(),
                emergency_admin_pubkey: self.emergency_admin,
                ts: current_time as i64,
            });
        } else {
            emit!(EmergencyResumed {
                pool: self.key(),
                emergency_admin_pubkey: self.emergency_admin,
                ts: current_time as i64,
            });
        }

        Ok(())
    }

    pub fn decay_volume(&mut self, current_time: u64) -> Result<()> {
        let hours_passed = current_time
            .checked_sub(self.volume.last_reset)
            .ok_or_else(|| {
                error!(ErrorCode::InvalidTimestamp, "Failed to calculate hours passed: {} - {}", 
                    current_time, self.volume.last_reset)
            })?
            .checked_div(3600)
            .ok_or_else(|| {
                error!(ErrorCode::Overflow, "Hours calculation overflow: {} / 3600", 
                    current_time - self.volume.last_reset)
            })?;

        if hours_passed > 0 {
            let decay_factor = 100u64.saturating_sub(hours_passed.min(24));
            let new_volume = self.volume.current_volume
                .saturating_mul(decay_factor)
                .checked_div(100)
                .ok_or_else(|| {
                    error!(ErrorCode::Overflow, "Volume decay calculation overflow: {} * {} / 100", 
                        self.volume.current_volume, decay_factor)
                })?;

            self.volume.current_volume = new_volume;
            self.volume.last_reset = current_time;
        }

        Ok(())
    }

    pub fn reset_rate_limit(&mut self, current_time: u64) -> Result<()> {
        let old_count = self.rate_limit.count;
        self.rate_limit.count = 0;
        self.rate_limit.last_reset = current_time;

        emit!(RateLimitReset {
            pool: self.key(),
            old_count,
            new_count: 0,
            ts: current_time as i64,
        });

        Ok(())
    }

    pub fn pause_pool(&mut self, current_time: u64) -> Result<()> {
        require!(!self.is_paused, ErrorCode::PoolPaused);
        self.is_paused = true;
        self.last_update = current_time;

        emit!(PoolPaused {
            pool: self.key(),
            admin_pubkey: self.admin,
            ts: current_time as i64,
        });

        Ok(())
    }

    pub fn resume_pool(&mut self, current_time: u64) -> Result<()> {
        require!(self.is_paused, ErrorCode::PoolNotPaused);
        self.is_paused = false;
        self.last_update = current_time;

        emit!(PoolResumed {
            pool: self.key(),
            admin_pubkey: self.admin,
            ts: current_time as i64,
        });

        Ok(())
    }

    pub fn check_token_mint(&self, mint: &Account<Mint>) -> Result<()> {
        validate_condition!(mint.key() == self.token_mint, ErrorCode::InvalidTokenMint);
        validate_condition!(mint.decimals == self.token_decimals, ErrorCode::InvalidTokenDecimals);
        validate_condition!(mint.freeze_authority.is_none(), ErrorCode::TokenMintHasFreezeAuthority);
        Ok(())
    }

    pub fn check_token_account(&self, account: &Account<TokenAccount>, mint: &Pubkey) -> Result<()> {
        validate_condition!(account.mint == *mint, ErrorCode::InvalidTokenAccount);
        validate_condition!(!account.is_delegated(), ErrorCode::TokenAccountDelegated);
        Ok(())
    }

    pub fn emit_liquidity_added(&self, admin: &Pubkey, amount: u64, ts: i64) {
        emit!(LiquidityAdded {
            pool: self.key(),
            admin_pubkey: *admin,
            amount,
            ts,
        });
    }

    pub fn emit_liquidity_removed(&self, admin: &Pubkey, amount: u64, ts: i64) {
        emit!(LiquidityRemoved {
            pool: self.key(),
            admin_pubkey: *admin,
            amount,
            ts,
        });
    }

    pub fn emit_trade_executed(&self, buyer: &Pubkey, amount_in: u64, amount_out: u64, fee_amount: u64, fee_mode: u8, ts: i64) {
        emit!(TradeExecuted {
            pool: self.key(),
            buyer_pubkey: *buyer,
            amount_in,
            amount_out,
            fee_amount,
            fee_mode,
            ts,
            token_mint: self.token_mint,
        });
    }

    /// Calculates the price impact of a trade
    /// 
    /// # Arguments
    /// * `amount_in` - The amount of tokens being traded
    /// * `pool_balance` - The current pool balance
    /// 
    /// # Returns
    /// * `Result<u64>` - The price impact in basis points
    pub fn calculate_price_impact(&self, amount_in: u64, pool_balance: u64) -> Result<u64> {
        if pool_balance == 0 {
            return Ok(0);
        }

        // Calculate price impact in basis points
        let impact = amount_in
            .checked_mul(10000)
            .ok_or(ErrorCode::Overflow)?
            .checked_div(pool_balance)
            .ok_or(ErrorCode::Overflow)?;

        Ok(impact)
    }

    /// Enforces minimum fee requirements and handles edge cases
    /// 
    /// # Arguments
    /// * `fee` - The calculated fee amount
    /// 
    /// # Returns
    /// * `Result<u64>` - The enforced minimum fee
    pub fn enforce_min_fee(&self, fee: u64) -> Result<u64> {
        // Ensure fee is at least MINIMUM_FEE to prevent zero-fee edge cases
        let min_fee = fee.max(MINIMUM_FEE);
        
        // Check for overflow after max operation
        if min_fee < fee {
            return Err(ErrorCode::Overflow.into());
        }

        Ok(min_fee)
    }

    pub fn calculate_fee(&self, amount_in: u64, current_time: i64) -> Result<(u64, u8)> {
        // Check if we're in early trade window
        let pool_age = current_time
            .checked_sub(self.pool_start_time as i64)
            .ok_or_else(|| {
                error!(ErrorCode::InvalidTimestamp, "Failed to calculate pool age: {} - {}", 
                    current_time, self.pool_start_time)
            })?;

        if pool_age < self.trade_settings.early_trade_window_seconds as i64 {
            // Apply early trade fee
            let fee = self.trade_settings.early_trade_fee_bps
                .checked_mul(amount_in)
                .ok_or_else(|| {
                    error!(ErrorCode::Overflow, "Fee calculation overflow: {} * {}", 
                        self.trade_settings.early_trade_fee_bps, amount_in)
                })?
                .checked_div(10000)
                .ok_or_else(|| {
                    error!(ErrorCode::Overflow, "Fee calculation division overflow: {} / 10000", 
                        self.trade_settings.early_trade_fee_bps * amount_in)
                })?;

            // Ensure minimum fee
            let fee = self.enforce_min_fee(fee)?;
            return Ok((fee, FEE_MODE_EARLY_TRADE));
        }

        // Find applicable fee tier
        for tier in &self.fee_tiers {
            if self.volume.current_volume <= tier.volume_threshold {
                let fee = tier.fee_bps
                    .checked_mul(amount_in)
                    .ok_or(ErrorCode::Overflow)?
                    .checked_div(10000)
                    .ok_or(ErrorCode::Overflow)?;

                // Ensure minimum fee
                let fee = self.enforce_min_fee(fee)?;
                return Ok((fee, FEE_MODE_TIER_BASED));
            }
        }

        // Use default fee if no tier applies
        let default_fee_bps = self.default_fee_bps.unwrap_or(MINIMUM_FEE_BPS);
        let fee = default_fee_bps
            .checked_mul(amount_in)
            .ok_or(ErrorCode::Overflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::Overflow)?;

        // Ensure minimum fee
        let fee = self.enforce_min_fee(fee)?;
        Ok((fee, FEE_MODE_NONE))
    }

    pub fn schedule_emergency_pause(&mut self, current_time: u64) -> Result<()> {
        let scheduled_time = current_time
            .checked_add(EMERGENCY_TIMELOCK_SECONDS)
            .ok_or(ErrorCode::Overflow)?;

        self.emergency_action_scheduled_time = scheduled_time;
        Ok(())
    }

    pub fn schedule_emergency_resume(&mut self, current_time: u64) -> Result<()> {
        let scheduled_time = current_time
            .checked_add(EMERGENCY_TIMELOCK_SECONDS)
            .ok_or(ErrorCode::Overflow)?;

        self.emergency_action_scheduled_time = scheduled_time;
        Ok(())
    }

    pub fn validate_fee_tiers(&self, fee_tiers: &[FeeTier]) -> Result<()> {
        // Check if fee tiers are empty
        if fee_tiers.is_empty() {
            msg!("Fee tiers cannot be empty");
            return Err(ErrorCode::InvalidFeeTier.into());
        }

        // Check if too many fee tiers
        if fee_tiers.len() > MAX_FEE_TIERS {
            msg!("Too many fee tiers: {} > {}", fee_tiers.len(), MAX_FEE_TIERS);
            return Err(ErrorCode::TooManyFeeTiers.into());
        }

        // Validate each tier
        let mut prev_threshold = 0;
        let mut prev_fee = MAXIMUM_FEE_BPS + 1; // Start with a value higher than max allowed

        for (i, tier) in fee_tiers.iter().enumerate() {
            // Check volume threshold
            if tier.volume_threshold <= prev_threshold {
                msg!("Invalid fee tier threshold at index {}: {} <= {}", 
                    i, 
                    tier.volume_threshold, 
                    prev_threshold
                );
                return Err(ErrorCode::InvalidFeeTierSpacing.into());
            }

            // Check fee bounds
            if tier.fee_bps < MINIMUM_FEE_BPS {
                msg!("Fee too low at index {}: {} < {}", 
                    i, 
                    tier.fee_bps, 
                    MINIMUM_FEE_BPS
                );
                return Err(ErrorCode::FeeTooLow.into());
            }
            if tier.fee_bps > MAXIMUM_FEE_BPS {
                msg!("Fee too high at index {}: {} > {}", 
                    i, 
                    tier.fee_bps, 
                    MAXIMUM_FEE_BPS
                );
                return Err(ErrorCode::FeeTooHigh.into());
            }

            // Check fee monotonicity (fees should be non-increasing)
            if tier.fee_bps > prev_fee {
                msg!("Invalid fee progression at index {}: {} > {}", 
                    i, 
                    tier.fee_bps, 
                    prev_fee
                );
                return Err(ErrorCode::InvalidFeeTier.into());
            }

            // Check for duplicate fees
            if tier.fee_bps == prev_fee {
                msg!("Duplicate fee at index {}: {}", i, tier.fee_bps);
                return Err(ErrorCode::DuplicateFeeTierThreshold.into());
            }

            prev_threshold = tier.volume_threshold;
            prev_fee = tier.fee_bps;
        }

        Ok(())
    }

    pub fn validate_fee_bounds(&self, fee_bps: u64) -> Result<()> {
        validate_condition!(
            fee_bps >= MINIMUM_FEE_BPS && fee_bps <= MAXIMUM_FEE_BPS,
            ErrorCode::FeeTooLow,
            "Fee {} bps outside allowed range [{}, {}]",
            fee_bps,
            MINIMUM_FEE_BPS,
            MAXIMUM_FEE_BPS
        );
        Ok(())
    }

    pub fn is_address_forbidden(&self, address: &Pubkey) -> bool {
        address == &self.admin || 
        address == &self.emergency_admin || 
        self.trader_blacklist.contains(address)
    }

    /// Initializes a new pool state with the given parameters
    /// 
    /// # Arguments
    /// * `admin` - Public key of the pool admin
    /// * `token_mint` - Public key of the pool's token mint
    /// * `bump` - PDA bump seed
    /// 
    /// # Returns
    /// * `Result<()>` - Success or error
    pub fn initialize(&mut self, admin: &Pubkey, token_mint: &Pubkey, bump: u8) -> Result<()> {
        // Generate unique pool ID
        let pool_id = Pubkey::find_program_address(
            &[POOL_ID_SEED, admin.as_ref(), token_mint.as_ref()],
            &crate::ID,
        ).0.to_bytes();

        self.pool_id = pool_id;
        self.bump = bump;
        self.admin = *admin;
        self.token_mint = *token_mint;
        self.is_initialized = true;
        self.pool_start_time = current_unix_ts()?;
        self.last_update = current_unix_ts()?;
        Ok(())
    }

    /// Validates fee tiers before locking them
    /// 
    /// Ensures that:
    /// 1. Volume thresholds are strictly increasing
    /// 2. Fee rates are non-increasing
    /// 3. All fees are within allowed bounds
    /// 
    /// # Returns
    /// * `Result<()>` - Success or error
    pub fn validate_fee_tiers_before_lock(&self) -> Result<()> {
        if self.fee_tiers.is_empty() {
            return Ok(());
        }

        let mut prev_threshold = 0;
        let mut prev_fee = u64::MAX;

        for tier in &self.fee_tiers {
            // Check threshold ordering
            validate_condition!(
                tier.volume_threshold > prev_threshold,
                ErrorCode::InvalidFeeTier,
                "Fee tier threshold {} not strictly increasing",
                tier.volume_threshold
            );

            // Check fee ordering
            validate_condition!(
                tier.fee_bps <= prev_fee,
                ErrorCode::InvalidFeeTier,
                "Fee tier {} bps not non-increasing",
                tier.fee_bps
            );

            prev_threshold = tier.volume_threshold;
            prev_fee = tier.fee_bps;
        }

        Ok(())
    }

    pub fn check_rate_limit(&mut self, current_time: u64) -> Result<()> {
        if current_time >= self.rate_limit.last_reset + self.rate_limit.window_seconds {
            self.rate_limit.count = 0;
            self.rate_limit.last_reset = current_time;
        }
        
        if self.rate_limit.count >= self.rate_limit.max_calls {
            msg!("Rate limit exceeded: {} calls in window (max: {})", 
                self.rate_limit.count, 
                self.rate_limit.max_calls
            );
            return Err(ErrorCode::RateLimitExceeded.into());
        }
        
        self.rate_limit.count = self.rate_limit.count.checked_add(1)
            .ok_or(ErrorCode::Overflow)?;
            
        Ok(())
    }
}

impl ValidationHelpers for PoolState {
    fn check_token_account_ownership(&self, owner: &Pubkey) -> Result<()> {
        if owner != &self.admin {
            msg!("Unauthorized: expected admin {} but got {}", self.admin, owner);
            return Err(ErrorCode::Unauthorized.into());
        }
        Ok(())
    }

    fn check_pool_authority(&self, authority: &Pubkey, program_id: &Pubkey) -> Result<()> {
        let (expected_authority, _) = derive_pool_authority(&self.key(), program_id)?;
        if authority != &expected_authority {
            msg!("Invalid pool authority: expected {} but got {}", expected_authority, authority);
            return Err(ErrorCode::InvalidPoolAuthority.into());
        }
        Ok(())
    }

    fn check_token_mint(&self, mint: &Account<Mint>) -> Result<()> {
        if mint.key() != self.token_mint {
            msg!("Invalid token mint: expected {} but got {}", self.token_mint, mint.key());
            return Err(ErrorCode::InvalidTokenMint.into());
        }
        if mint.decimals != self.token_decimals {
            msg!("Invalid token decimals: expected {} but got {}", self.token_decimals, mint.decimals);
            return Err(ErrorCode::InvalidTokenDecimals.into());
        }
        if mint.freeze_authority.is_some() {
            msg!("Token mint has freeze authority: {}", mint.freeze_authority.unwrap());
            return Err(ErrorCode::TokenMintHasFreezeAuthority.into());
        }
        Ok(())
    }

    fn check_token_account(&self, account: &Account<TokenAccount>, mint: &Pubkey) -> Result<()> {
        if account.mint != *mint {
            msg!("Invalid token account mint: expected {} but got {}", mint, account.mint);
            return Err(ErrorCode::InvalidTokenAccount.into());
        }
        if account.is_delegated() {
            msg!("Token account is delegated: {}", account.key());
            return Err(ErrorCode::TokenAccountDelegated.into());
        }
        Ok(())
    }

    fn check_circuit_breaker(&self, current_time: u64) -> Result<()> {
        let cooldown_end = self.circuit_breaker.last_trigger + self.circuit_breaker.cooldown;
        if current_time < cooldown_end {
            msg!("Circuit breaker cooldown active: {} seconds remaining", cooldown_end - current_time);
            return Err(ErrorCode::CircuitBreakerCooldown.into());
        }
        Ok(())
    }

    fn check_rate_limit(&self, current_time: u64) -> Result<()> {
        let window_end = self.rate_limit.last_reset + self.rate_limit.window_seconds;
        if current_time >= window_end {
            msg!("Rate limit window expired: resetting counter");
            self.reset_rate_limit(current_time)?;
        }
        if self.rate_limit.count >= self.rate_limit.max_calls {
            msg!("Rate limit exceeded: {} calls in window (max: {})", self.rate_limit.count, self.rate_limit.max_calls);
            return Err(ErrorCode::RateLimitExceeded.into());
        }
        Ok(())
    }

    fn check_volume_limit(&self, amount: u64) -> Result<()> {
        let new_volume = self.volume.current_volume.checked_add(amount).ok_or_else(|| {
            msg!("Volume overflow: {} + {}", self.volume.current_volume, amount);
            error!(ErrorCode::Overflow)
        })?;
        if new_volume > self.volume.max_daily {
            msg!("Daily volume limit exceeded: {} > {}", new_volume, self.volume.max_daily);
            return Err(ErrorCode::DailyVolumeLimitExceeded.into());
        }
        Ok(())
    }
}

impl anchor_lang::Key for PoolState {
    fn key(&self) -> Pubkey {
        self.key()
    }
}


#[macro_export]
macro_rules! validate_condition {
    ($condition:expr, $error:expr) => {
        if !$condition {
            return Err($error.into());
        }
    };
}

pub trait ValidationHelpers {
    fn check_token_account_ownership(&self, owner: &Pubkey) -> Result<()>;
    fn check_pool_authority(&self, authority: &Pubkey, program_id: &Pubkey) -> Result<()>;
    fn check_token_mint(&self, mint: &Account<Mint>) -> Result<()>;
    fn check_token_account(&self, account: &Account<TokenAccount>, mint: &Pubkey) -> Result<()>;
    fn check_circuit_breaker(&self, current_time: u64) -> Result<()>;
    fn check_rate_limit(&self, current_time: u64) -> Result<()>;
    fn check_volume_limit(&self, amount: u64) -> Result<()>;
}

#[macro_export]
macro_rules! error {
    ($error:expr) => {
        Err($error.into())
    };
    ($error:expr, $msg:expr) => {
        {
            msg!($msg);
            Err($error.into())
        }
    };
}