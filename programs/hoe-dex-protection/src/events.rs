use anchor_lang::prelude::*;
use crate::types::{TradeSettingsUpdate, ProtectionSettingsUpdate, FeeSettingsUpdate, StateSettingsUpdate};

#[event]
pub struct PoolInitialized {
    pub pool: Pubkey,
    pub admin_pubkey: Pubkey,
    pub ts: i64,
}

#[event]
pub struct LiquidityAdded {
    pub pool: Pubkey,
    pub admin_pubkey: Pubkey,
    pub amount: u64,
    pub ts: i64,
}

#[event]
pub struct LiquidityRemoved {
    pub pool: Pubkey,
    pub admin_pubkey: Pubkey,
    pub amount: u64,
    pub ts: i64,
}

#[event]
pub struct TradeExecuted {
    pub pool: Pubkey,
    pub buyer_pubkey: Pubkey,
    pub amount_in: u64,
    pub amount_out: u64,
    pub fee_amount: u64,
    pub fee_mode: u8,
    pub ts: i64,
    pub token_mint: Pubkey,
}

#[event]
pub struct RateLimitReset {
    pub pool: Pubkey,
    pub old_count: u32,
    pub new_count: u32,
    pub ts: i64,
}

#[event]
pub struct CircuitBreakerTriggered {
    pub pool: Pubkey,
    pub volume_24h: u64,
    pub threshold: u64,
    pub ts: i64,
}

#[event]
pub struct TraderBlacklisted {
    pub pool: Pubkey,
    pub trader_pubkey: Pubkey,
    pub ts: i64,
}

#[event]
pub struct TraderRemovedFromBlacklist {
    pub pool: Pubkey,
    pub trader_pubkey: Pubkey,
    pub ts: i64,
}

#[event]
pub struct BatchBlacklistCompleted {
    pub pool: Pubkey,
    pub admin_pubkey: Pubkey,
    pub count: u64,
    pub ts: i64,
}

#[event]
pub struct FeesWithdrawn {
    pub pool: Pubkey,
    pub admin_pubkey: Pubkey,
    pub amount: u64,
    pub ts: i64,
}

#[event]
pub struct FeeTiersLocked {
    pub pool: Pubkey,
    pub admin_pubkey: Pubkey,
    pub ts: i64,
}

#[event]
pub struct FeeTiersUnlockScheduled {
    pub pool: Pubkey,
    pub admin_pubkey: Pubkey,
    pub scheduled_time: i64,
}

#[event]
pub struct ParameterUpdateScheduled {
    pub pool: Pubkey,
    pub admin_pubkey: Pubkey,
    pub scheduled_time: i64,
}

#[event]
pub struct ParameterUpdateCancelled {
    pub pool: Pubkey,
    pub admin_pubkey: Pubkey,
    pub ts: i64,
    pub trade_settings: Option<TradeSettingsUpdate>,
    pub protection_settings: Option<ProtectionSettingsUpdate>,
    pub fee_settings: Option<FeeSettingsUpdate>,
    pub state_settings: Option<StateSettingsUpdate>,
}

#[event]
pub struct ParametersUpdated {
    pub pool: Pubkey,
    pub admin_pubkey: Pubkey,
    pub ts: i64,
}

#[event]
pub struct EmergencyPauseScheduled {
    pub pool: Pubkey,
    pub emergency_admin_pubkey: Pubkey,
    pub scheduled_time: i64,
}

#[event]
pub struct EmergencyPaused {
    pub pool: Pubkey,
    pub emergency_admin_pubkey: Pubkey,
    pub ts: i64,
}

#[event]
pub struct EmergencyResumeScheduled {
    pub pool: Pubkey,
    pub emergency_admin_pubkey: Pubkey,
    pub scheduled_time: i64,
}

#[event]
pub struct EmergencyResumed {
    pub pool: Pubkey,
    pub emergency_admin_pubkey: Pubkey,
    pub ts: i64,
}

#[event]
pub struct CircuitBreakerReset {
    pub pool: Pubkey,
    pub admin_pubkey: Pubkey,
    pub ts: i64,
}

#[event]
pub struct AdminUpdated {
    pub pool: Pubkey,
    pub old_admin_pubkey: Pubkey,
    pub new_admin_pubkey: Pubkey,
    pub ts: i64,
}

#[event]
pub struct OperationFailed {
    pub pool: Pubkey,
    pub operation: String,
    pub reason: String,
    pub ts: i64,
}

#[event]
pub struct FreezeAuthorityWarning {
    pub pool: Pubkey,
    pub token_mint: Pubkey,
    pub ts: i64,
}

#[event]
pub struct PoolPaused {
    pub pool: Pubkey,
    pub admin_pubkey: Pubkey,
    pub ts: i64,
}

#[event]
pub struct PoolResumed {
    pub pool: Pubkey,
    pub admin_pubkey: Pubkey,
    pub ts: i64,
}

#[event]
pub struct VolumeDecayed {
    pub pool: Pubkey,
    pub old_volume: u64,
    pub new_volume: u64,
    pub hours_passed: u64,
    pub ts: i64,
}

#[event]
pub struct PriceImpactRejected {
    pub pool: Pubkey,
    pub amount_in: u64,
    pub price_impact: u64,
    pub max_allowed: u64,
    pub ts: i64,
}

#[event]
pub struct TradeExecutionFailed {
    pub pool: Pubkey,
    pub buyer: Pubkey,
    pub amount_in: u64,
    pub reason: String,
    pub ts: i64,
}

#[event]
pub struct LiquidityOperationFailed {
    pub pool: Pubkey,
    pub admin: Pubkey,
    pub operation: String,
    pub amount: u64,
    pub reason: String,
    pub ts: i64,
}

#[event]
pub struct AdminOperationFailed {
    pub pool: Pubkey,
    pub admin: Pubkey,
    pub operation: String,
    pub reason: String,
    pub ts: i64,
} 