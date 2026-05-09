use anchor_lang::prelude::*;

use crate::state::{Whitelist, WhitelistConfig};

#[derive(Accounts)]
#[instruction(user: Pubkey)]
pub struct AddToWhitelist<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        seeds = [b"whitelist"],
        bump = whitelist_config.bump,
        has_one = admin,
    )]
    pub whitelist_config: Account<'info, WhitelistConfig>,
    #[account(
        init,
        payer = admin,
        space = 8 + 32 + 1,
        seeds = [b"whitelist", user.as_ref()],
        bump,
    )]
    pub whitelist: Account<'info, Whitelist>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(user: Pubkey)]
pub struct RemoveFromWhitelist<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        seeds = [b"whitelist"],
        bump = whitelist_config.bump,
        has_one = admin,
    )]
    pub whitelist_config: Account<'info, WhitelistConfig>,
    #[account(
        mut,
        close = admin,
        seeds = [b"whitelist", user.as_ref()],
        bump = whitelist.bump,
        constraint = whitelist.address == user,
    )]
    pub whitelist: Account<'info, Whitelist>,
}

impl<'info> AddToWhitelist<'info> {
    pub fn add_to_whitelist(&mut self, address: Pubkey, bumps: AddToWhitelistBumps) -> Result<()> {
        self.whitelist.set_inner(Whitelist {
            address,
            bump: bumps.whitelist,
        });

        Ok(())
    }
}

impl<'info> RemoveFromWhitelist<'info> {
    pub fn remove_from_whitelist(&mut self) -> Result<()> {
        Ok(())
    }
}
