use anchor_lang::prelude::*;
use crate::types::*;
use crate::errors::ErrorCode;

pub fn validate_admin_action(pool_state: &Account<PoolState>, admin: &Pubkey, current_time: i64) -> Result<()> {
    require!(pool_state.admin == *admin, ErrorCode::Unauthorized);
    require!(!pool_state.is_emergency_paused, ErrorCode::EmergencyPaused);
    require!(current_time > 0, ErrorCode::InvalidTimestamp);
    Ok(())
}

pub fn validate_trade_parameters(pool_state: &Account<PoolState>, amount: u64, current_time: i64) -> Result<()> {
    require!(!pool_state.is_paused, ErrorCode::PoolPaused);
    require!(!pool_state.is_emergency_paused, ErrorCode::EmergencyPaused);
    require!(amount > 0, ErrorCode::InvalidAmount);
    require!(current_time > 0, ErrorCode::InvalidTimestamp);
    pool_state.check_volume_limit(current_time)?;
    pool_state.check_rate_limit(amount, current_time)?;
    pool_state.check_circuit_breaker(amount, current_time)?;
    if pool_state.protection.blacklist_enabled {
        require!(
            !pool_state.blacklist.contains(&ctx.accounts.user.key()),
            ErrorCode::Unauthorized
        );
    }
    Ok(())
}

pub fn validate_fee_parameters(pool_state: &Account<PoolState>, fee_tiers: &[FeeTier]) -> Result<()> {
    pool_state.validate_fee_parameters(fee_tiers)
}