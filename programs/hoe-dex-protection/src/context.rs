use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, TokenAccount};

#[derive(Accounts)]
pub struct InitializePool<'info> {
    #[account(init, payer = admin, space = 8 + 512)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub token_mint: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub pool_token_account: Account<'info, TokenAccount>,
    pub user: Signer<'info>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
}

#[derive(Accounts)]
pub struct AdminAction<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut)]
    pub pool_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut, constraint = pool_authority.key() == pool_state.get_pool_authority(&program_id)?.0)]
    pub pool_authority: AccountInfo<'info>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
    pub program_id: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct ExecuteTrade<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub user_token_account_in: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_token_account_out: Account<'info, TokenAccount>,
    #[account(mut)]
    pub pool_token_account: Account<'info, TokenAccount>,
    pub user: Signer<'info>,
    #[account(mut, constraint = pool_authority.key() == pool_state.get_pool_authority(&program_id)?.0)]
    pub pool_authority: AccountInfo<'info>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
    pub program_id: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct ManageBlacklist<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct WithdrawFees<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut)]
    pub pool_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub admin_token_account: Account<'info, TokenAccount>,
    #[account(mut, constraint = pool_authority.key() == pool_state.get_pool_authority(&program_id)?.0)]
    pub pool_authority: AccountInfo<'info>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
    pub program_id: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct LockFeeTiers<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct EmergencyAction<'info> {
    #[account(mut)]
    pub pool_state: Account<'info, PoolState>,
    #[account(mut)]
    pub emergency_admin: Signer<'info>,
}