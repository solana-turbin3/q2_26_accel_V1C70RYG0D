use anchor_lang::prelude::*;

#[account]
pub struct WhitelistConfig {
    pub admin: Pubkey,
    pub bump: u8,
}

#[account]
pub struct Whitelist {
    pub address: Pubkey,
    pub bump: u8,
}
