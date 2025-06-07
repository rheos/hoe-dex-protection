use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid amount provided")]
    InvalidAmount,
    #[msg("Arithmetic overflow")]
    Overflow,
    #[msg("Insufficient liquidity")]
    InsufficientLiquidity,
    #[msg("Price impact too high")]
    PriceImpactTooHigh,
    #[msg("Slippage exceeded")]
    SlippageExceeded,
    #[msg("No fees available")]
    NoFeesAvailable,
    #[msg("Fee tiers are locked")]
    FeeTiersLocked,
    #[msg("Fee tiers are not locked")]
    FeeTiersNotLocked,
    #[msg("No pending update available")]
    NoPendingUpdate,
    #[msg("Timelock has not expired")]
    TimelockNotExpired,
    #[msg("Invalid emergency admin")]
    InvalidEmergencyAdmin,
    #[msg("Pool is emergency paused")]
    EmergencyPaused,
    #[msg("Pool is not paused")]
    PoolNotPaused,
    #[msg("Circuit breaker cooldown active")]
    CircuitBreakerCooldown,
    #[msg("Invalid new admin")]
    InvalidNewAdmin,
    #[msg("Volume limit exceeded")]
    VolumeLimitExceeded,
    #[msg("Rate limit exceeded")]
    RateLimitExceeded,
    #[msg("Circuit breaker triggered")]
    CircuitBreakerTriggered,
    #[msg("Invalid token mint")]
    InvalidTokenMint,
    #[msg("Invalid token decimals")]
    InvalidTokenDecimals,
    #[msg("Token mint has freeze authority")]
    TokenMintHasFreezeAuthority,
    #[msg("Invalid token account")]
    InvalidTokenAccount,
    #[msg("Token account is delegated")]
    TokenAccountDelegated,
    #[msg("Invalid fee tier")]
    InvalidFeeTier,
    #[msg("Too many fee tiers")]
    TooManyFeeTiers,
    #[msg("Invalid fee tier spacing")]
    InvalidFeeTierSpacing,
    #[msg("Fee too low")]
    FeeTooLow,
    #[msg("Fee too high")]
    FeeTooHigh,
    #[msg("Duplicate fee tier threshold")]
    DuplicateFeeTierThreshold,
    #[msg("Daily volume limit exceeded")]
    DailyVolumeLimitExceeded,
    #[msg("Unauthorized action")]
    Unauthorized,
    #[msg("Invalid pool authority")]
    InvalidPoolAuthority,
    #[msg("Invalid timestamp")]
    InvalidTimestamp,
}