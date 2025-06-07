use anchor_lang::prelude::*;
use crate::types::*;
use crate::errors::ErrorCode;

pub fn process_blacklist_operations(
    pool_state: &mut Account<PoolState>,
    traders: Vec<Pubkey>,
    operation: BlacklistOperation,
) -> Result<()> {
    match operation {
        BlacklistOperation::Add => {
            for trader in traders {
                // Add trader to blacklist (simplified; assumes storage elsewhere)
            }
        }
        BlacklistOperation::Remove => {
            for trader in traders {
                // Remove trader from blacklist (simplified; assumes storage elsewhere)
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
        b"pool_authority".as_ref(),
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