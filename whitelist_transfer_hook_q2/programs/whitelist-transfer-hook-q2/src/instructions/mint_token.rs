use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::{self, MintTo},
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::state::{Vault, MINT_SEED, VAULT_SEED};

#[derive(Accounts)]
pub struct MintTokens<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        seeds = [VAULT_SEED],
        bump = vault.bump,
        has_one = mint,
        has_one = admin,
    )]
    pub vault: Account<'info, Vault>,
    #[account(
        mut,
        seeds = [MINT_SEED],
        bump = vault.mint_bump,
    )]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = recipient,
    )]
    pub recipient_token_account: InterfaceAccount<'info, TokenAccount>,
    /// CHECK: Token account authority; can be a wallet or PDA.
    pub recipient: UncheckedAccount<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> MintTokens<'info> {
    pub fn mint_tokens(&self, amount: u64) -> Result<()> {
        let vault_seeds: &[&[u8]] = &[VAULT_SEED, &[self.vault.bump]];
        let signer_seeds: &[&[&[u8]]] = &[vault_seeds];

        let cpi_accounts = MintTo {
            mint: self.mint.to_account_info(),
            to: self.recipient_token_account.to_account_info(),
            authority: self.vault.to_account_info(),
        };
        let cpi_context =
            CpiContext::new_with_signer(self.token_program.key(), cpi_accounts, signer_seeds);

        token_2022::mint_to(cpi_context, amount)
    }
}
