use anchor_lang::prelude::*;

declare_id!("EPKa5m6izBgWC3eJBL7zYYybNDgdB29wzpyeeGZNM6cV");

#[program]
pub mod hoe_dex_protection {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
