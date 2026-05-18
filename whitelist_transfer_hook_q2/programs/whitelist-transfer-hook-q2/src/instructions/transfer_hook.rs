use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::spl_token_2022::{
        extension::{
            transfer_hook::TransferHookAccount, BaseStateWithExtensions, PodStateWithExtensions,
        },
        pod::PodAccount,
    },
    token_interface::{Mint, TokenAccount},
};

use crate::state::{Vault, VaultError, VAULT_SEED};

#[derive(Accounts)]
pub struct TransferHook<'info> {
    #[account(
        token::mint = mint,
    )]
    pub source_token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        token::mint = mint,
    )]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,
    /// CHECK: source token account owner, can be SystemAccount or PDA owned by another program
    pub owner: UncheckedAccount<'info>,
    /// CHECK: ExtraAccountMetaList Account,
    #[account(
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump
    )]
    pub extra_account_meta_list: UncheckedAccount<'info>,
    #[account(
        seeds = [VAULT_SEED],
        bump = vault.bump,
        has_one = mint,
        constraint = vault.vault_token_account == source_token.key()
            || vault.vault_token_account == destination_token.key()
            @ VaultError::InvalidVaultInteraction,
    )]
    pub vault: Account<'info, Vault>,
}

impl<'info> TransferHook<'info> {
    /// This function is called when the transfer hook is executed.
    pub fn transfer_hook(&mut self, amount: u64) -> Result<()> {
        self.check_is_transferring()?;

        msg!("Source token owner: {}", self.source_token.owner);
        msg!("Destination token owner: {}", self.destination_token.owner);

        let vault_token_account = self.vault.vault_token_account;
        let interacting_user = if self.destination_token.key() == vault_token_account {
            self.source_token.owner
        } else if self.source_token.key() == vault_token_account {
            self.destination_token.owner
        } else {
            return err!(VaultError::InvalidVaultInteraction);
        };

        self.vault.assert_allowed(interacting_user, amount)?;

        msg!("Transfer allowed: {} is whitelisted", interacting_user);

        Ok(())
    }

    /// Checks if the transfer hook is being executed during a transfer operation.
    fn check_is_transferring(&mut self) -> Result<()> {
        // Ensure that the source token account has the transfer hook extension enabled

        // Get the account info of the source token account
        let source_token_info = self.source_token.to_account_info();
        let account_data_ref = source_token_info.try_borrow_data()?;

        // Unpack the account data as a PodStateWithExtensions
        // This will allow us to access the extensions of the token account
        // We use PodStateWithExtensions because TokenAccount is a POD (Plain Old Data) type
        let account = PodStateWithExtensions::<PodAccount>::unpack(&account_data_ref)?;
        // Get the TransferHookAccount extension
        // Search for the TransferHookAccount extension in the token account
        // The returning struct has a `transferring` field that indicates if the account is in the middle of a transfer operation
        let account_extension = account.get_extension::<TransferHookAccount>()?;

        // Check if the account is in the middle of a transfer operation
        if !bool::from(account_extension.transferring) {
            return err!(VaultError::InvalidVaultInteraction);
        }

        Ok(())
    }
}
