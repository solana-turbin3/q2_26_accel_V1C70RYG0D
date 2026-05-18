use anchor_lang::prelude::*;

use crate::state::{Vault, VAULT_SEED};

#[derive(Accounts)]
#[instruction(user: Pubkey)]
pub struct AddToWhitelist<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        mut,
        seeds = [VAULT_SEED],
        bump = vault.bump,
        has_one = admin,
    )]
    pub vault: Account<'info, Vault>,
}

#[derive(Accounts)]
#[instruction(user: Pubkey)]
pub struct RemoveFromWhitelist<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        mut,
        seeds = [VAULT_SEED],
        bump = vault.bump,
        has_one = admin,
    )]
    pub vault: Account<'info, Vault>,
}

impl<'info> AddToWhitelist<'info> {
    pub fn add_to_whitelist(&mut self, user: Pubkey, amount: u64) -> Result<()> {
        self.vault.upsert_whitelist_entry(user, amount)
    }
}

impl<'info> RemoveFromWhitelist<'info> {
    pub fn remove_from_whitelist(&mut self, user: Pubkey) -> Result<()> {
        self.vault.remove_whitelist_entry(user)
    }
}
