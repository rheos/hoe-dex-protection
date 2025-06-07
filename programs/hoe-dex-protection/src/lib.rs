use anchor_lang::prelude::*;
use anchor_spl::token::{self, TokenAccount, Transfer};
use std::collections::HashSet;
use crate::context::*;
use crate::errors::ErrorCode;
use crate::types::*;
use crate::validation::*;
use crate::utils::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod hoe_dex_protection {
    use super::*;

    // Initialize a new pool with the given admin and token mint
    pub fn initialize(ctx: Context<InitializePool>, admin: Pubkey, token_mint: Pubkey, bump: u8) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        pool_state.admin = admin;
        pool_state.emergency_admin = admin;
        pool_state.token_mint = token_mint;
        pool_state.token_decimals = ctx.accounts.token_mint.decimals;
        pool_state.total_liquidity = 0;
        pool_state.total_fees_collected = 0;
        pool_state.is_paused = false;
        pool_state.is_emergency_paused = false;
        pool_state.fee_tiers_locked = false;
        pool_state.pending_update = None;
        pool_state.rate_limit = RateLimitSettings {
            max_calls: 100,
            window_size: 3600,
            current_window: 0,
            max_per_window: 1000,
        };
        pool_state.circuit_breaker = CircuitBreakerSettings {
            max_amount: 1_000_000,
            cooldown_period: 3600,
            current_amount: 0,
        };
        pool_state.volume = VolumeSettings {
            max_daily: 1_000_000_000,
            volume_24h: 0,
            last_update: 0,
            last_decay: 0,
            current_volume: 0,
            decay_period: 86_400,
        };
        pool_state.protection = ProtectionSettings {
            max_price_impact_bps: 500,
            max_slippage_bps: 200,
            blacklist_enabled: true,
            circuit_breaker_threshold: 1_000_000,
            circuit_breaker_window: 3600,
            circuit_breaker_cooldown: 3600,
        };
        pool_state.fee_tiers = vec![FeeTier {
            threshold: 0,
            fee_bps: 30,
        }];
        Ok(())
    }

    // Add liquidity to the pool
    pub fn add_liquidity(ctx: Context<AddLiquidity>, amount: u64) -> Result<()> {
        validation::validate_admin_action(&ctx.accounts.pool_state, &ctx.accounts.admin.key(), Clock::get()?.unix_timestamp)?;
        if amount == 0 {
            return Err(ErrorCode::InvalidAmount.into());
        }
        let pool_state = &mut ctx.accounts.pool_state;
        pool_state.total_liquidity = pool_state.total_liquidity.checked_add(amount).ok_or(ErrorCode::Overflow)?;
        let transfer_instruction = TokenTransfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.pool_token_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
            amount,
        };
        token::transfer(transfer_instruction.into(), amount)?;
        pool_state.last_update = Clock::get()?.unix_timestamp;
        Ok(())
    }

    // Remove liquidity from the pool
    pub fn remove_liquidity(ctx: Context<AdminAction>, amount: u64) -> Result<()> {
        validation::validate_admin_action(&ctx.accounts.pool_state, &ctx.accounts.admin.key(), Clock::get()?.unix_timestamp)?;
        if amount == 0 {
            return Err(ErrorCode::InvalidAmount.into());
        }
        let pool_state = &mut ctx.accounts.pool_state;
        if amount > pool_state.total_liquidity {
            return Err(ErrorCode::InsufficientLiquidity.into());
        }
        pool_state.total_liquidity = pool_state.total_liquidity.checked_sub(amount).ok_or(ErrorCode::Overflow)?;
        let transfer_instruction = TokenTransfer {
            from: ctx.accounts.pool_token_account.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.pool_authority.to_account_info(),
            amount,
        };
        let cpi_context = utils::create_cpi_context(&ctx.accounts.pool_state, &ctx, ctx.accounts.program_id)?;
        token::transfer(cpi_context, amount)?;
        pool_state.last_update = Clock::get()?.unix_timestamp;
        Ok(())
    }

    // Execute a trade with the given parameters
    pub fn execute_trade<'info>(
        ctx: Context<ExecuteTrade>,
        amount_in: u64,
        min_amount_out: u64,
        price_impact: u64,
        max_slippage: u64,
    ) -> Result<TradeOutcome> {
        validation::validate_trade_parameters(&ctx.accounts.pool_state, amount_in, Clock::get()?.unix_timestamp)?;
        let pool_state = &mut ctx.accounts.pool_state;
        let (fee_amount, fee_mode) = PoolState::calculate_fee(&pool_state, amount_in, Clock::get()?.unix_timestamp)?;
        let amount_out = amount_in.checked_sub(fee_amount).ok_or(ErrorCode::Overflow)?;
        if price_impact > pool_state.protection.max_price_impact_bps {
            msg!("Price impact too high: {} > {}", price_impact, pool_state.protection.max_price_impact_bps);
            return Err(ErrorCode::PriceImpactTooHigh.into());
        }
        let slippage = amount_in.checked_sub(amount_out).ok_or(ErrorCode::Overflow)?
            .checked_mul(10000).ok_or(ErrorCode::Overflow)?
            .checked_div(amount_in).ok_or(ErrorCode::Overflow)?;
        if slippage > max_slippage {
            return Err(ErrorCode::SlippageExceeded.into());
        }
        let transfer_in = TokenTransfer {
            from: ctx.accounts.user_token_account_in.to_account_info(),
            to: ctx.accounts.pool_token_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
            amount: amount_in,
        };
        let transfer_out = TokenTransfer {
            from: ctx.accounts.pool_token_account.to_account_info(),
            to: ctx.accounts.user_token_account_out.to_account_info(),
            authority: ctx.accounts.pool_authority.to_account_info(),
            amount: amount_out,
        };
        let cpi_context = utils::create_cpi_context(&ctx.accounts.pool_state, &ctx, ctx.program_id)?;
        token::transfer(transfer_in.into(), amount_in)?;
        token::transfer(cpi_context, amount_out)?;
        pool_state.total_fees_collected = pool_state.total_fees_collected.checked_add(fee_amount).ok_or(ErrorCode::Overflow)?;
        Ok(TradeOutcome {
            amount_out,
            fee_amount,
            fee_mode: FeeMode::from_u8(fee_mode).unwrap_or(FeeMode::None),
        })
    }

    // Blacklist a trader
    pub fn blacklist_trader(ctx: Context<ManageBlacklist>, trader: Pubkey) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let _guard = ReentrancyGuard::new(pool_state)?;
        utils::process_blacklist_operations(
            pool_state,
            vec![trader],
            BlacklistOperation::Add,
        )?;
        Ok(())
    }

    // Remove a trader from the blacklist
    pub fn remove_from_blacklist(ctx: Context<ManageBlacklist>, trader: Pubkey) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        let _guard = ReentrancyGuard::new(pool_state)?;
        utils::process_blacklist_operations(
            pool_state,
            vec![trader],
            BlacklistOperation::Remove,
        )?;
        Ok(())
    }

    // Batch blacklist traders
    pub fn batch_blacklist_traders(ctx: Context<ManageBlacklist>, traders: Vec<Pubkey>) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        utils::process_blacklist_operations(
            pool_state,
            traders,
            BlacklistOperation::Add,
        )?;
        Ok(())
    }

    // Batch unblacklist traders
    pub fn batch_unblacklist_traders(ctx: Context<ManageBlacklist>, traders: Vec<Pubkey>) -> Result<()> {
        let pool_state = &mut ctx.accounts.pool_state;
        utils::process_blacklist_operations(
            pool_state,
            traders,
            BlacklistOperation::Remove,
        )?;
        Ok(())
    }

    // Withdraw accumulated fees
    pub fn withdraw_fees(ctx: Context<WithdrawFees>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp;
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;
        validate_condition!(state.total_fees_collected > 0, ErrorCode::NoFeesAvailable);
        let amount = state.total_fees_collected;
        state.total_fees_collected = 0;
        let cpi_context = utils::create_cpi_context(state, &ctx, ctx.program_id)?;
        token::transfer(cpi_context, amount)?;
        Ok(())
    }

    // Lock fee tiers
    pub fn lock_fee_tiers(ctx: Context<LockFeeTiers>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp;
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;
        validate_condition!(!state.fee_tiers_locked, ErrorCode::FeeTiersLocked);
        state.fee_tiers_locked = true;
        Ok(())
    }

    // Unlock fee tiers
    pub fn unlock_fee_tiers(ctx: Context<AdminAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp;
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;
        validate_condition!(state.fee_tiers_locked, ErrorCode::FeeTiersNotLocked);
        state.fee_tiers_locked = false;
        Ok(())
    }

    // Schedule a parameter update
    pub fn schedule_parameter_update(
        ctx: Context<AdminAction>,
        settings: ParameterUpdate,
    ) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp;
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;
        if let Some(fee_tiers) = &settings.fee_tiers {
            validation::validate_fee_parameters(state, fee_tiers)?;
            state.fee_tiers = fee_tiers.clone();
        }
        state.pending_update = Some(ParameterUpdateScheduled {
            update: settings.clone(),
            scheduled_time: current_time + 86400,
        });
        Ok(())
    }

    // Cancel a pending parameter update
    pub fn cancel_parameter_update(ctx: Context<AdminAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp;
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;
        if state.pending_update.is_none() {
            error!(ErrorCode::NoPendingUpdate, "No pending update available");
        }
        state.pending_update = None;
        emit!(ParameterUpdateCancelled {
            update_type: "all".to_string(),
            scheduled_time: current_time,
        });
        Ok(())
    }

    // Apply a pending parameter update
    pub fn apply_parameter_update(ctx: Context<AdminAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp;
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;
        let pending_update = state.pending_update.take().ok_or(ErrorCode::NoPendingUpdate)?;
        if current_time < pending_update.scheduled_time {
            return Err(ErrorCode::TimelockNotExpired.into());
        }
        match pending_update.update {
            ParameterUpdate::Trade(trade_settings) => {
                state.fee_tiers = trade_settings.fee_tiers.unwrap_or(state.fee_tiers.clone());
                emit!(TradeSettingsUpdate {
                    fee_tiers: state.fee_tiers.clone(),
                });
            }
            ParameterUpdate::Protection(protection_settings) => {
                state.volume.max_daily = protection_settings.max_daily_volume;
                state.protection.max_price_impact_bps = protection_settings.max_price_impact_bps;
                state.protection.max_slippage_bps = protection_settings.max_slippage_bps;
                state.protection.blacklist_enabled = protection_settings.blacklist_enabled;
                state.rate_limit.max_calls = protection_settings.rate_limit_max;
                state.rate_limit.window_size = protection_settings.rate_limit_window;
                state.circuit_breaker.max_amount = protection_settings.circuit_breaker_threshold;
                state.circuit_breaker.cooldown_period = protection_settings.circuit_breaker_cooldown;
                emit!(ProtectionSettingsUpdate {
                    max_daily_volume: protection_settings.max_daily_volume,
                    max_price_impact_bps: protection_settings.max_price_impact_bps,
                    max_slippage_bps: protection_settings.max_slippage_bps,
                    blacklist_enabled: protection_settings.blacklist_enabled,
                    rate_limit_max: protection_settings.rate_limit_max,
                    rate_limit_window: protection_settings.rate_limit_window,
                    circuit_breaker_threshold: protection_settings.circuit_breaker_threshold,
                    circuit_breaker_cooldown: protection_settings.circuit_breaker_cooldown,
                });
            }
            ParameterUpdate::Fee(fee_settings) => {
                state.fee_tiers = fee_settings.fee_tiers.unwrap_or(state.fee_tiers.clone());
                emit!(FeeSettingsUpdate {
                    fee_tiers: state.fee_tiers.clone(),
                });
            }
            ParameterUpdate::State(state_settings) => {
                state.is_paused = state_settings.is_paused;
                emit!(StateSettingsUpdate {
                    is_paused: state_settings.is_paused,
                });
            }
        }
        Ok(())
    }

    // Schedule an emergency pause
    pub fn schedule_emergency_pause(ctx: Context<EmergencyAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp;
        if ctx.accounts.emergency_admin.key() != state.emergency_admin {
            return Err(ErrorCode::InvalidEmergencyAdmin.into());
        }
        validate_condition!(!state.is_emergency_paused, ErrorCode::EmergencyPaused);
        state.pending_update = Some(ParameterUpdateScheduled {
            update: ParameterUpdate::State(StateSettingsUpdate {
                is_paused: true,
            }),
            scheduled_time: current_time + 3600,
        });
        Ok(())
    }

    // Apply an emergency pause
    pub fn apply_emergency_pause(ctx: Context<EmergencyAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp;
        if ctx.accounts.emergency_admin.key() != state.emergency_admin {
            return Err(ErrorCode::InvalidEmergencyAdmin.into());
        }
        let pending_update = state.pending_update.take().ok_or(ErrorCode::NoPendingUpdate)?;
        if current_time < pending_update.scheduled_time {
            return Err(ErrorCode::TimelockNotExpired.into());
        }
        state.is_emergency_paused = true;
        Ok(())
    }

    // Schedule an emergency resume
    pub fn schedule_emergency_resume(ctx: Context<EmergencyAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp;
        if ctx.accounts.emergency_admin.key() != state.emergency_admin {
            return Err(ErrorCode::InvalidEmergencyAdmin.into());
        }
        validate_condition!(state.is_emergency_paused, ErrorCode::PoolNotPaused);
        state.pending_update = Some(ParameterUpdateScheduled {
            update: ParameterUpdate::State(StateSettingsUpdate {
                is_paused: false,
            }),
            scheduled_time: current_time + 3600,
        });
        Ok(())
    }

    // Apply an emergency resume
    pub fn apply_emergency_resume(ctx: Context<EmergencyAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp;
        if ctx.accounts.emergency_admin.key() != state.emergency_admin {
            return Err(ErrorCode::InvalidEmergencyAdmin.into());
        }
        let pending_update = state.pending_update.take().ok_or(ErrorCode::NoPendingUpdate)?;
        if current_time < pending_update.scheduled_time {
            return Err(ErrorCode::TimelockNotExpired.into());
        }
        state.is_emergency_paused = false;
        Ok(())
    }

    // Reset circuit breaker
    pub fn reset_circuit_breaker(ctx: Context<AdminAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp;
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;
        if current_time < state.circuit_breaker.cooldown_period + state.circuit_breaker.current_amount {
            return Err(ErrorCode::CircuitBreakerCooldown.into());
        }
        state.circuit_breaker.current_amount = 0;
        Ok(())
    }

    // Update admin
    pub fn update_admin(ctx: Context<AdminAction>, new_admin: Pubkey) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp;
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;
        if new_admin == Pubkey::default() {
            return Err(ErrorCode::InvalidNewAdmin.into());
        }
        state.admin = new_admin;
        state.last_update = current_time;
        Ok(())
    }

    // Reset pending update
    pub fn reset_pending_update(ctx: Context<AdminAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp;
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;
        validate_condition!(state.pending_update.is_some(), ErrorCode::NoPendingUpdate);
        state.pending_update = None;
        emit!(ParameterUpdateCancelled {
            update_type: "all".to_string(),
            scheduled_time: current_time,
        });
        Ok(())
    }

    // Toggle pause
    pub fn toggle_pause(ctx: Context<AdminAction>) -> Result<()> {
        let state = &mut ctx.accounts.pool_state;
        let current_time = Clock::get()?.unix_timestamp;
        validation::validate_admin_action(state, &ctx.accounts.admin.key(), current_time)?;
        state.is_paused = !state.is_paused;
        emit!(StateSettingsUpdate {
            is_paused: state.is_paused,
        });
        Ok(())
    }
}

impl PoolState {
    // Initialize default settings
    pub fn initialize_default(&mut self) -> Result<()> {
        self.rate_limit = RateLimitSettings {
            max_calls: 100,
            window_size: 3600,
            current_window: 0,
            max_per_window: 1000,
        };
        self.circuit_breaker = CircuitBreakerSettings {
            max_amount: 1_000_000,
            cooldown_period: 3600,
            current_amount: 0,
        };
        self.volume = VolumeSettings {
            max_daily: 1_000_000_000,
            volume_24h: 0,
            last_update: 0,
            last_decay: 0,
            current_volume: 0,
            decay_period: 86_400,
        };
        self.protection = ProtectionSettings {
            max_price_impact_bps: 500,
            max_slippage_bps: 200,
            blacklist_enabled: true,
            circuit_breaker_threshold: 1_000_000,
            circuit_breaker_window: 3600,
            circuit_breaker_cooldown: 3600,
        };
        self.fee_tiers = vec![FeeTier {
            threshold: 0,
            fee_bps: 30,
        }];
        Ok(())
    }

    // Toggle emergency pause state
    pub fn toggle_emergency_pause(&mut self, current_time: i64) -> Result<()> {
        if current_time <= 0 {
            return Err(ErrorCode::InvalidTimestamp.into());
        }
        self.is_emergency_paused = !self.is_emergency_paused;
        emit!(StateSettingsUpdate {
            is_paused: self.is_emergency_paused,
        });
        Ok(())
    }

    // Decay volume over time
    pub fn decay_volume(&mut self, current_time: i64) -> Result<()> {
        if current_time <= 0 {
            return Err(ErrorCode::InvalidTimestamp.into());
        }
        if self.volume.last_decay == 0 || current_time > self.volume.last_decay + self.volume.decay_period as i64 {
            self.volume.volume_24h = 0;
            self.volume.last_decay = current_time;
        }
        Ok(())
    }

    // Update volume tracking
    pub fn update_volume(&mut self, amount: u64, current_time: i64) -> Result<()> {
        if current_time <= 0 {
            return Err(ErrorCode::InvalidTimestamp.into());
        }
        let new_volume = self.volume.volume_24h.checked_add(amount).ok_or(ErrorCode::Overflow)?;
        if new_volume > self.volume.max_daily {
            msg!("Daily volume limit exceeded: {} > {}", new_volume, self.volume.max_daily);
            return Err(ErrorCode::DailyVolumeLimitExceeded.into());
        }
        self.volume.volume_24h = new_volume;
        self.volume.last_update = current_time;
        self.volume.current_volume = new_volume;
        Ok(())
    }

    // Check volume limit
    pub fn check_volume_limit(&self, current_time: i64) -> Result<()> {
        if current_time <= 0 {
            return Err(ErrorCode::InvalidTimestamp.into());
        }
        if self.volume.volume_24h > self.volume.max_daily {
            return Err(ErrorCode::VolumeLimitExceeded.into());
        }
        Ok(())
    }

    // Check rate limit
    pub fn check_rate_limit(&self, amount: u64, current_time: i64) -> Result<()> {
        if current_time <= 0 {
            return Err(ErrorCode::InvalidTimestamp.into());
        }
        if self.rate_limit.current_window + self.rate_limit.window_size as i64 <= current_time {
            return Ok(());
        }
        if self.rate_limit.max_calls >= self.rate_limit.max_per_window {
            return Err(ErrorCode::RateLimitExceeded.into());
        }
        Ok(())
    }

    // Update rate limit
    pub fn update_rate_limit(&mut self, amount: u64, current_time: i64) -> Result<()> {
        if current_time <= 0 {
            return Err(ErrorCode::InvalidTimestamp.into());
        }
        if self.rate_limit.current_window + self.rate_limit.window_size as i64 <= current_time {
            self.rate_limit.max_calls = 0;
            self.rate_limit.current_window = current_time;
        }
        self.rate_limit.max_calls = self.rate_limit.max_calls.checked_add(1).ok_or(ErrorCode::Overflow)?;
        if self.rate_limit.max_calls > self.rate_limit.max_per_window {
            return Err(ErrorCode::RateLimitExceeded.into());
        }
        Ok(())
    }

    // Check circuit breaker
    pub fn check_circuit_breaker(&self, amount: u64, current_time: i64) -> Result<()> {
        if current_time <= 0 {
            return Err(ErrorCode::InvalidTimestamp.into());
        }
        let cooldown_end = self.circuit_breaker.current_amount + self.circuit_breaker.cooldown_period as i64;
        if current_time < cooldown_end {
            msg!("Circuit breaker cooldown active: {} seconds remaining", cooldown_end - current_time);
            return Err(ErrorCode::CircuitBreakerCooldown.into());
        }
        if self.circuit_breaker.current_amount + amount > self.circuit_breaker.max_amount {
            return Err(ErrorCode::CircuitBreakerTriggered.into());
        }
        Ok(())
    }

    // Update circuit breaker
    pub fn update_circuit_breaker(&mut self, amount: u64, current_time: i64) -> Result<()> {
        if current_time <= 0 {
            return Err(ErrorCode::InvalidTimestamp.into());
        }
        self.circuit_breaker.current_amount = self.circuit_breaker.current_amount.checked_add(amount).ok_or(ErrorCode::Overflow)?;
        if self.circuit_breaker.current_amount > self.circuit_breaker.max_amount {
            return Err(ErrorCode::CircuitBreakerTriggered.into());
        }
        Ok(())
    }

    // Calculate fee for a trade
    pub fn calculate_fee(pool_state: &PoolState, amount_in: u64, current_time: i64) -> Result<(u64, u8)> {
        let fee_bps = pool_state
            .fee_tiers
            .iter()
            .find(|tier| pool_state.total_liquidity >= tier.threshold)
            .map(|tier| tier.fee_bps)
            .ok_or(ErrorCode::InvalidFeeTier)?;
        let fee_amount = amount_in
            .checked_mul(fee_bps as u64)
            .ok_or(ErrorCode::Overflow)?
            .checked_div(10000)
            .ok_or(ErrorCode::Overflow)?;
        Ok((fee_amount, 0))
    }

    // Validate fee parameters
    pub fn validate_fee_parameters(&self, fee_tiers: &[FeeTier]) -> Result<()> {
        if fee_tiers.is_empty() {
            return Err(ErrorCode::InvalidFeeTier.into());
        }
        if fee_tiers.len() > 10 {
            return Err(ErrorCode::TooManyFeeTiers.into());
        }
        let mut last_threshold = 0;
        for tier in fee_tiers.iter() {
            if tier.threshold < last_threshold {
                return Err(ErrorCode::InvalidFeeTierSpacing.into());
            }
            if tier.fee_bps < 10 {
                return Err(ErrorCode::FeeTooLow.into());
            }
            if tier.fee_bps > 1000 {
                return Err(ErrorCode::FeeTooHigh.into());
            }
            if tier.threshold == last_threshold && last_threshold != 0 {
                return Err(ErrorCode::DuplicateFeeTierThreshold.into());
            }
            last_threshold = tier.threshold;
        }
        Ok(())
    }

    // Reset rate limit
    pub fn reset_rate_limit(&mut self, current_time: i64) -> Result<()> {
        self.rate_limit.max_calls = 0;
        self.rate_limit.current_window = current_time;
        Ok(())
    }

    // Check if pool is paused
    pub fn check_pause(&self) -> Result<()> {
        require!(!self.is_paused, ErrorCode::PoolPaused);
        Ok(())
    }

    // Check if pool is not paused
    pub fn check_not_paused(&self) -> Result<()> {
        require!(self.is_paused, ErrorCode::PoolNotPaused);
        Ok(())
    }

    // Validate token mint
    pub fn check_token_mint(&self, mint: &AccountInfo) -> Result<()> {
        validate_condition!(mint.key() == self.token_mint, ErrorCode::InvalidTokenMint);
        let mint_account: Account<Mint> = Account::try_from(mint)?;
        validate_condition!(mint_account.decimals == self.token_decimals, ErrorCode::InvalidTokenDecimals);
        validate_condition!(mint_account.freeze_authority.is_none(), ErrorCode::TokenMintHasFreezeAuthority);
        Ok(())
    }

    // Validate token account
    pub fn check_token_account(&self, account: &Account<TokenAccount>) -> Result<()> {
        if account.mint != self.token_mint {
            return Err(ErrorCode::InvalidTokenAccount.into());
        }
        if account.delegate.is_some() {
            return Err(ErrorCode::TokenAccountDelegated.into());
        }
        Ok(())
    }

    // Get pool authority
    pub fn get_pool_authority(&self, program_id: &Pubkey) -> Result<(Pubkey, u8)> {
        Pubkey::find_program_address(&[b"pool_authority", self.to_account_info().key.as_ref()], program_id)
            .ok_or(ErrorCode::InvalidPoolAuthority)
    }
}

// Reentrancy guard
pub struct ReentrancyGuard<'info> {
    pool_state: &'info mut Account<'info, PoolState>,
}

impl<'info> ReentrancyGuard<'info> {
    pub fn new(pool_state: &'info mut Account<'info, PoolState>) -> Result<Self> {
        if pool_state.is_paused {
            return Err(ErrorCode::PoolPaused.into());
        }
        Ok(ReentrancyGuard { pool_state })
    }
}

impl<'info> Drop for ReentrancyGuard<'info> {
    fn drop(&mut self) {}
}