use anchor_lang::prelude::*;

// Fee tier for dynamic fee calculation
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct FeeTier {
    pub threshold: u64,
    pub fee_bps: u16,
}

// Settings for rate limiting
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct RateLimitSettings {
    pub max_calls: u32,
    pub window_size: u64,
    pub current_window: i64,
    pub max_per_window: u32,
}

// Settings for circuit breaker
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct CircuitBreakerSettings {
    pub max_amount: u64,
    pub cooldown_period: u64,
    pub current_amount: u64,
}

// Settings for volume tracking
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct VolumeSettings {
    pub max_daily: u64,
    pub volume_24h: u64,
    pub last_update: i64,
    pub last_decay: i64,
    pub current_volume: u64,
    pub decay_period: u64,
}

// Protection settings for the pool
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct ProtectionSettings {
    pub max_price_impact_bps: u64,
    pub max_slippage_bps: u64,
    pub blacklist_enabled: bool,
    pub circuit_breaker_threshold: u64,
    pub circuit_breaker_window: u64,
    pub circuit_breaker_cooldown: u64,
}

// Trade settings update
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct TradeSettingsUpdate {
    pub fee_tiers: Option<Vec<FeeTier>>,
}

// Protection settings update
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ProtectionSettingsUpdate {
    pub max_daily_volume: u64,
    pub max_price_impact_bps: u64,
    pub max_slippage_bps: u64,
    pub blacklist_enabled: bool,
    pub rate_limit_max: u32,
    pub rate_limit_window: u64,
    pub circuit_breaker_threshold: u64,
    pub circuit_breaker_cooldown: u64,
}

// Fee settings update
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct FeeSettingsUpdate {
    pub fee_tiers: Option<Vec<FeeTier>>,
}

// State settings update
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct StateSettingsUpdate {
    pub is_paused: bool,
}

// Parameter update types
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub enum ParameterUpdate {
    Trade(TradeSettingsUpdate),
    Protection(ProtectionSettingsUpdate),
    Fee(FeeSettingsUpdate),
    State(StateSettingsUpdate),
}

// Scheduled parameter update
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ParameterUpdateScheduled {
    pub update: ParameterUpdate,
    pub scheduled_time: i64,
}

// Pool state
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct PoolState {
    pub admin: Pubkey,
    pub emergency_admin: Pubkey,
    pub token_mint: Pubkey,
    pub token_decimals: u8,
    pub total_liquidity: u64,
    pub total_fees_collected: u64,
    pub is_paused: bool,
    pub is_emergency_paused: bool,
    pub fee_tiers_locked: bool,
    pub pending_update: Option<ParameterUpdateScheduled>,
    pub rate_limit: RateLimitSettings,
    pub circuit_breaker: CircuitBreakerSettings,
    pub volume: VolumeSettings,
    pub protection: ProtectionSettings,
    pub fee_tiers: Vec<FeeTier>,
    pub last_update: i64,
}

impl Default for PoolState {
    fn default() -> Self {
        PoolState {
            admin: Pubkey::default(),
            emergency_admin: Pubkey::default(),
            token_mint: Pubkey::default(),
            token_decimals: 0,
            total_liquidity: 0,
            total_fees_collected: 0,
            is_paused: false,
            is_emergency_paused: false,
            fee_tiers_locked: false,
            pending_update: None,
            rate_limit: RateLimitSettings::default(),
            circuit_breaker: CircuitBreakerSettings::default(),
            volume: VolumeSettings::default(),
            protection: ProtectionSettings::default(),
            fee_tiers: vec![],
            last_update: 0,
        }
    }
}

// Token transfer instruction
#[derive(Clone, Debug)]
pub struct TokenTransfer<'info> {
    pub from: AccountInfo<'info>,
    pub to: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
    pub amount: u64,
}

impl<'info> From<TokenTransfer<'info>> for CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
    fn from(transfer: TokenTransfer<'info>) -> Self {
        CpiContext::new(
            transfer.from,
            Transfer {
                from: transfer.from,
                to: transfer.to,
                authority: transfer.authority,
            },
        )
    }
}

// Trade outcome
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct TradeOutcome {
    pub amount_out: u64,
    pub fee_amount: u64,
    pub fee_mode: FeeMode,
}

// Fee mode
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub enum FeeMode {
    None,
    Fixed,
    Dynamic,
}

impl FeeMode {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(FeeMode::None),
            1 => Some(FeeMode::Fixed),
            2 => Some(FeeMode::Dynamic),
            _ => None,
        }
    }
}

// Blacklist operation
#[derive(Clone, Debug)]
pub enum BlacklistOperation {
    Add,
    Remove,
}