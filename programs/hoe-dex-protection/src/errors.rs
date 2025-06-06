use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Unauthorized access: caller is not the admin or emergency admin")]
    Unauthorized,
    #[msg("Invalid amount: must be greater than zero")]
    InvalidAmount,
    #[msg("Invalid token account: owner mismatch or invalid mint")]
    InvalidTokenAccount,
    #[msg("Invalid token mint: mint mismatch or invalid decimals")]
    InvalidTokenMint,
    #[msg("Invalid pool authority: derived authority mismatch")]
    InvalidPoolAuthority,
    #[msg("Invalid fee tier: tier not found or invalid parameters")]
    InvalidFeeTier,
    #[msg("Invalid timestamp: timestamp is in the future or invalid")]
    InvalidTimestamp,
    #[msg("Invalid trade: trade amount too small")]
    TradeTooSmall,
    #[msg("Invalid trade: trade amount too large")]
    TradeTooLarge,
    #[msg("Rate limit exceeded: too many operations in time window")]
    RateLimitExceeded,
    #[msg("Circuit breaker triggered: operation blocked by circuit breaker")]
    CircuitBreakerTriggered,
    #[msg("Volume limit exceeded: daily volume limit reached")]
    VolumeLimitExceeded,
    #[msg("Arithmetic overflow: operation would overflow")]
    Overflow,
    #[msg("Arithmetic underflow: operation would underflow")]
    Underflow,
    #[msg("Invalid pool state: pool not initialized or invalid state")]
    InvalidPoolState,
    #[msg("Invalid protection settings: invalid parameters")]
    InvalidProtectionSettings,
    #[msg("Invalid fee settings: invalid parameters")]
    InvalidFeeSettings,
    #[msg("Invalid rate limit settings: invalid parameters")]
    InvalidRateLimitSettings,
    #[msg("Invalid circuit breaker settings: invalid parameters")]
    InvalidCircuitBreakerSettings,
    #[msg("Invalid volume settings: invalid parameters")]
    InvalidVolumeSettings,
    #[msg("Invalid trade settings: invalid parameters")]
    InvalidTradeSettings,
    #[msg("Pool is paused")]
    PoolPaused,
    #[msg("Pool is emergency paused")]
    EmergencyPaused,
    #[msg("Pool is finalized")]
    PoolFinalized,
    #[msg("Fee tiers are locked")]
    FeeTiersLocked,
    #[msg("Too many fee tiers")]
    TooManyFeeTiers,
    #[msg("Invalid fee tier spacing")]
    InvalidFeeTierSpacing,
    #[msg("Fee too high")]
    FeeTooHigh,
    #[msg("Admin update too frequent")]
    AdminUpdateTooFrequent,
    #[msg("Invalid emergency admin")]
    InvalidEmergencyAdmin,
    #[msg("Operation failed")]
    OperationFailed,
    #[msg("Timelock not expired")]
    TimelockNotExpired,
    #[msg("Daily volume limit exceeded")]
    DailyVolumeLimitExceeded,
    #[msg("Price impact too high")]
    PriceImpactTooHigh,
    #[msg("Pool is not paused")]
    PoolNotPaused,
    #[msg("Invalid token decimals")]
    InvalidTokenDecimals,
    #[msg("Token mint has freeze authority")]
    TokenMintHasFreezeAuthority,
    #[msg("Token account is delegated")]
    TokenAccountDelegated,
    #[msg("Fee too low")]
    FeeTooLow,
    #[msg("Duplicate fee tier threshold")]
    DuplicateFeeTierThreshold,
    #[msg("Circuit breaker cooldown")]
    CircuitBreakerCooldown,
} 