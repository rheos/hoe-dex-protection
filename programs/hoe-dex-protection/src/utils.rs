use anchor_lang::prelude::*;
use crate::constants::*;
use crate::errors::ErrorCode;
use crate::types::*;

pub fn process_blacklist_operations(
    pool_state: &mut Account<PoolState>,
    traders: Vec<Pubkey>,
    operation: BlacklistOperation,
) -> Result<()> {
    require!(
        pool_state.protection.blacklist_enabled,
        ErrorCode::Unauthorized
    );
    require!(
        traders.len() <= BATCH_BLACKLIST_MAX_SIZE,
        ErrorCode::TooManyFeeTiers
    );
    match operation {
        BlacklistOperation::Add => {
            require!(
                pool_state.blacklist.len() + traders.len() <= MAX_BLACKLIST_SIZE,
                ErrorCode::TooManyFeeTiers
            );
            for trader in traders {
                pool_state.blacklist.insert(trader);
            }
        }
        BlacklistOperation::Remove => {
            for trader in traders {
                pool_state.blacklist.remove(&trader);
            }
        }
    }
    Ok(())
}

pub fn create_cpi_context<'a, 'b, 'c, 'info>(
    pool_state: &Account<PoolState>,
    ctx: &Context<'a, 'b, 'c, 'info, impl Accounts>,
    program_id: &Pubkey,
) -> Result<CpiContext<'a, 'b, 'c, 'info, Transfer<'info>>> {
    let (pool_authority, bump) = pool_state.get_pool_authority(program_id)?;
    let seeds = &[
        POOL_ID_SEED.as_ref(),
        pool_state.to_account_info().key.as_ref(),
        &[bump],
    ];
    let signer_seeds = &[&seeds[..]];
    Ok(CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        Transfer {
            from: ctx.accounts.pool_token_account.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.pool_authority.to_account_info(),
        },
        signer_seeds,
    ))
}