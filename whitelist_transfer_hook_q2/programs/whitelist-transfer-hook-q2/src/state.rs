use anchor_lang::prelude::*;

pub const VAULT_SEED: &[u8] = b"vault";
pub const MINT_SEED: &[u8] = b"mint";
pub const DECIMALS: u8 = 9;
pub const MAX_WHITELIST_ENTRIES: usize = 16;

#[account]
pub struct Vault {
    pub admin: Pubkey,
    pub mint: Pubkey,
    pub vault_token_account: Pubkey,
    pub whitelist: Vec<WhitelistEntry>,
    pub bump: u8,
    pub mint_bump: u8,
    pub extra_account_meta_bump: u8,
}

impl Vault {
    pub const LEN: usize = 32 + 32 + 32 + 4 + (MAX_WHITELIST_ENTRIES * WhitelistEntry::LEN) + 3;

    pub fn assert_allowed(&self, user: Pubkey, amount: u64) -> Result<()> {
        require!(amount > 0, VaultError::InvalidAmount);

        let entry = self
            .whitelist
            .iter()
            .find(|entry| entry.user == user)
            .ok_or(VaultError::NotWhitelisted)?;

        require!(amount <= entry.amount, VaultError::AmountExceedsWhitelist);

        Ok(())
    }

    pub fn upsert_whitelist_entry(&mut self, user: Pubkey, amount: u64) -> Result<()> {
        require!(amount > 0, VaultError::InvalidAmount);

        if let Some(entry) = self.whitelist.iter_mut().find(|entry| entry.user == user) {
            entry.amount = amount;
            return Ok(());
        }

        require!(
            self.whitelist.len() < MAX_WHITELIST_ENTRIES,
            VaultError::WhitelistFull
        );

        self.whitelist.push(WhitelistEntry { user, amount });

        Ok(())
    }

    pub fn remove_whitelist_entry(&mut self, user: Pubkey) -> Result<()> {
        let position = self
            .whitelist
            .iter()
            .position(|entry| entry.user == user)
            .ok_or(VaultError::NotWhitelisted)?;

        self.whitelist.remove(position);

        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct WhitelistEntry {
    pub user: Pubkey,
    pub amount: u64,
}

impl WhitelistEntry {
    pub const LEN: usize = 32 + 8;
}

#[error_code]
pub enum VaultError {
    #[msg("The amount must be greater than zero.")]
    InvalidAmount,
    #[msg("This user is not whitelisted for vault interactions.")]
    NotWhitelisted,
    #[msg("The requested amount exceeds this user's whitelist allowance.")]
    AmountExceedsWhitelist,
    #[msg("The vault whitelist is full.")]
    WhitelistFull,
    #[msg("Transfers for this mint must deposit into or withdraw from the vault token account.")]
    InvalidVaultInteraction,
}
