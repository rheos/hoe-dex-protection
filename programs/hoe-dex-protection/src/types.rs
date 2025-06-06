use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct TradeSettings {
    pub max_size_bps: u64,
    pub min_size: u64,
    pub cooldown_seconds: u64,
    pub last_trade_time: u64,
    pub early_trade_fee_bps: u64,
    pub early_trade_window_seconds: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ProtectionSettings {
    pub snipe_protection_seconds: u64,
    pub max_price_impact_bps: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct TradeSettingsUpdate {
    pub early_trade_fee_bps: u64,
    pub early_trade_window_seconds: u64,
    pub max_trade_size_bps: u64,
    pub min_trade_size: u64,
    pub cooldown_seconds: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ProtectionSettingsUpdate {
    pub max_daily_volume: u64,
    pub max_price_impact_bps: u64,
    pub circuit_breaker_threshold: u64,
    pub circuit_breaker_window: u64,
    pub circuit_breaker_cooldown: u64,
    pub rate_limit_window: u64,
    pub rate_limit_max: u32,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct FeeSettingsUpdate {
    pub fee_tiers: Vec<FeeTier>,
    pub fee_tiers_locked: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct StateSettingsUpdate {
    pub is_paused: bool,
    pub is_emergency_paused: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct FeeTier {
    pub volume_threshold: u64,
    pub fee_bps: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct RateLimitSettings {
    pub window_seconds: u64,
    pub count: u64,
    pub max_calls: u64,
    pub last_reset: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CircuitBreakerSettings {
    pub threshold: u64,
    pub window: u64,
    pub cooldown: u64,
    pub last_trigger: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct VolumeSettings {
    pub volume_24h: u64,
    pub last_update: u64,
    pub last_decay: u64,
    pub max_daily: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct PendingUpdate {
    /// When the update should be applied
    pub scheduled_time: u64,
    /// Updates to trade-related parameters
    pub trade_settings: Option<TradeSettingsUpdate>,
    /// Updates to protection mechanisms
    pub protection_settings: Option<ProtectionSettingsUpdate>,
    /// Updates to fee structure
    pub fee_settings: Option<FeeSettingsUpdate>,
    /// Updates to pool state
    pub state_settings: Option<StateSettingsUpdate>,
}

