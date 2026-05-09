use anchor_lang::prelude::*;

use crate::state::WhitelistConfig;

#[derive(Accounts)]
pub struct InitializeWhitelist<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        init,
        payer = admin,
        space = 8 + 32 + 1,
        seeds = [b"whitelist"],
        bump
    )]
    pub whitelist: Account<'info, WhitelistConfig>,
    pub system_program: Program<'info, System>,
}

impl<'info> InitializeWhitelist<'info> {
    pub fn initialize_whitelist(&mut self, bumps: InitializeWhitelistBumps) -> Result<()> {
        self.whitelist.set_inner(WhitelistConfig {
            admin: self.admin.key(),
            bump: bumps.whitelist,
        });

        Ok(())
    }
}
