use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use spl_tlv_account_resolution::state::ExtraAccountMetaList;

use crate::state::{Vault, DECIMALS, MINT_SEED, VAULT_SEED};
use crate::InitializeExtraAccountMetaList;

#[derive(Accounts)]
pub struct InitializeVault<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(
        init,
        payer = admin,
        space = 8 + Vault::LEN,
        seeds = [VAULT_SEED],
        bump
    )]
    pub vault: Account<'info, Vault>,
    #[account(
        init,
        payer = admin,
        mint::decimals = DECIMALS,
        mint::authority = vault,
        mint::token_program = token_program,
        extensions::transfer_hook::authority = vault,
        extensions::transfer_hook::program_id = crate::ID,
        extensions::permanent_delegate::delegate = admin,
        seeds = [MINT_SEED],
        bump
    )]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        init,
        payer = admin,
        associated_token::mint = mint,
        associated_token::authority = vault,
        associated_token::token_program = token_program,
    )]
    pub vault_token_account: InterfaceAccount<'info, TokenAccount>,
    /// CHECK: ExtraAccountMetaList account for the Token-2022 transfer hook.
    #[account(
        init,
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump,
        space = ExtraAccountMetaList::size_of(
            InitializeExtraAccountMetaList::extra_account_metas()?.len()
        ).unwrap(),
        payer = admin
    )]
    pub extra_account_meta_list: AccountInfo<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> InitializeVault<'info> {
    pub fn initialize_vault(&mut self, bumps: InitializeVaultBumps) -> Result<()> {
        self.vault.set_inner(Vault {
            admin: self.admin.key(),
            mint: self.mint.key(),
            vault_token_account: self.vault_token_account.key(),
            whitelist: Vec::new(),
            bump: bumps.vault,
            mint_bump: bumps.mint,
            extra_account_meta_bump: bumps.extra_account_meta_list,
        });

        ExtraAccountMetaList::init::<spl_transfer_hook_interface::instruction::ExecuteInstruction>(
            &mut self.extra_account_meta_list.try_borrow_mut_data()?,
            &InitializeExtraAccountMetaList::extra_account_metas()?,
        )?;

        Ok(())
    }
}
