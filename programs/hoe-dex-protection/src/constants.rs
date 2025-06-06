// Fee-related constants
pub const MINIMUM_FEE_BPS: u64 = 1; // 0.01%
pub const MINIMUM_FEE: u64 = 1; // Minimum fee in lamports
pub const MAX_EARLY_TRADE_FEE_BPS: u64 = 1000; // 10% maximum fee for early trades

// Cooldowns and timelocks
pub const EMERGENCY_TIMELOCK_SECONDS: u64 = 3600; // 1 hour emergency action delay
pub const PARAMETER_UPDATE_TIMELOCK: u64 = 86400; // 24 hours
pub const ADMIN_UPDATE_COOLDOWN: u64 = 86400; // 24 hours

// Pool state seeds
pub const POOL_ID_SEED: &[u8] = b"pool_authority";
pub const REENTRANCY_GUARD_SEED: &[u8] = b"reentrancy_guard";
pub const PENDING_UPDATE_SEED: &[u8] = b"pending_update";

// Fee mode constants for tracking fee application
pub const FEE_MODE_NONE: u8 = 0;
pub const FEE_MODE_EARLY_TRADE: u8 = 1;
pub const FEE_MODE_VOLUME_BASED: u8 = 2;
pub const FEE_MODE_CIRCUIT_BREAKER: u8 = 3;

// --- Limits ---
pub const MAX_FEE_TIERS: usize = 100;
pub const MAX_BLACKLIST_SIZE: usize = 1000;
pub const MAX_PENDING_UPDATE_SIZE: usize = 100;
pub const BATCH_BLACKLIST_MAX_SIZE: usize = 50;
pub const MIN_FEE_TIER_SPACING_BPS: u64 = 10; // 0.1%

// --- Circuit Breaker Settings ---
pub const MAX_PRICE_IMPACT_BPS: u64 = 1000; // 10% maximum price impact
pub const MAX_DAILY_VOLUME_BPS: u64 = 10000; // 100% of max_daily_volume
pub const MAX_TRADE_SIZE_BPS: u64 = 1000; // 10% of max_daily_volume
pub const MAX_HOURLY_TRADES: u32 = 100; // Maximum trades per hour
pub const MAX_HOURLY_VOLUME_BPS: u64 = 1000; // 10% of max_daily_volume

// --- Volume and Trade Limits ---
pub const MAX_TRADE_SIZE: u64 = 1000000; // Maximum trade size in token units
pub const MAX_HOURLY_VOLUME: u64 = 10000000; // Maximum hourly volume in token units
pub const MAX_DAILY_VOLUME_LIMIT: u64 = 100000000; // Maximum daily volume in token units

// --- Cooldown Periods ---
pub const MAX_HOURLY_TRADES_COOLDOWN: u64 = 3600; // 1 hour cooldown for hourly trade limit
pub const MAX_HOURLY_VOLUME_COOLDOWN: u64 = 3600; // 1 hour cooldown for hourly volume limit
pub const MAX_TRADE_SIZE_COOLDOWN: u64 = 3600; // 1 hour cooldown for trade size limit
pub const MAX_PRICE_IMPACT_COOLDOWN: u64 = 3600; // 1 hour cooldown for price impact limit
pub const MAX_DAILY_VOLUME_COOLDOWN: u64 = 86400; // 24 hour cooldown for daily volume limit

// --- Time Windows ---
pub const MAX_TRADE_SIZE_WINDOW: u64 = 3600; // 1 hour window for trade size limit
pub const MAX_HOURLY_TRADES_WINDOW: u64 = 3600; // 1 hour window for hourly trades limit
pub const MAX_HOURLY_VOLUME_WINDOW: u64 = 3600; // 1 hour window for hourly volume limit
pub const MAX_DAILY_VOLUME_WINDOW: u64 = 86400; // 24 hour window for daily volume limit
pub const MAX_PRICE_IMPACT_WINDOW: u64 = 3600; // 1 hour window for price impact limit

// --- Reset Periods ---
pub const MAX_TRADE_SIZE_RESET: u64 = 3600; // 1 hour reset for trade size limit
pub const MAX_HOURLY_TRADES_RESET: u64 = 3600; // 1 hour reset for hourly trades limit
pub const MAX_HOURLY_VOLUME_RESET: u64 = 3600; // 1 hour reset for hourly volume limit
pub const MAX_DAILY_VOLUME_RESET: u64 = 86400; // 24 hour reset for daily volume limit
pub const MAX_PRICE_IMPACT_RESET: u64 = 3600; // 1 hour reset for price impact limit

// --- Decay Settings ---
pub const MAX_TRADE_SIZE_DECAY: u64 = 100; // 1% decay per hour
pub const MAX_HOURLY_TRADES_DECAY: u64 = 100; // 1% decay per hour
pub const MAX_HOURLY_VOLUME_DECAY: u64 = 100; // 1% decay per hour
pub const MAX_DAILY_VOLUME_DECAY: u64 = 100; // 1% decay per hour
pub const MAX_PRICE_IMPACT_DECAY: u64 = 100; // 1% decay per hour

// --- Decay Windows ---
pub const MAX_TRADE_SIZE_DECAY_WINDOW: u64 = 3600; // 1 hour decay window
pub const MAX_HOURLY_TRADES_DECAY_WINDOW: u64 = 3600; // 1 hour decay window
pub const MAX_HOURLY_VOLUME_DECAY_WINDOW: u64 = 3600; // 1 hour decay window
pub const MAX_DAILY_VOLUME_DECAY_WINDOW: u64 = 86400; // 24 hour decay window
pub const MAX_PRICE_IMPACT_DECAY_WINDOW: u64 = 3600; // 1 hour decay window

// --- Decay Reset Periods ---
pub const MAX_TRADE_SIZE_DECAY_RESET: u64 = 3600; // 1 hour decay reset
pub const MAX_HOURLY_TRADES_DECAY_RESET: u64 = 3600; // 1 hour decay reset
pub const MAX_HOURLY_VOLUME_DECAY_RESET: u64 = 3600; // 1 hour decay reset
pub const MAX_DAILY_VOLUME_DECAY_RESET: u64 = 86400; // 24 hour decay reset
pub const MAX_PRICE_IMPACT_DECAY_RESET: u64 = 3600; // 1 hour decay reset 