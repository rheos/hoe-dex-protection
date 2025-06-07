use anchor_lang::prelude::*;
use crate::*;

pub fn validate_admin_action(state: &PoolState, admin: &Pubkey, current_time: i64) -> Result<()> {
    validate_condition!(
        admin == &state.admin || admin == &state.emergency_admin,
        crate::ErrorCode::Unauthorized
    );
    Ok(())
}

pub fn validate_fee_parameters(state: &PoolState, fee_tiers: &[FeeTier]) -> Result<()> {
    state.validate_fee_tiers(fee_tiers)?;
    Ok(())
} 