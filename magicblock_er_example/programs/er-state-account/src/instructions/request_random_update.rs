use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use ephemeral_vrf_sdk::anchor::vrf;
use ephemeral_vrf_sdk::instructions::{create_request_randomness_ix, RequestRandomnessParams};
use ephemeral_vrf_sdk::types::SerializableAccountMeta;

use crate::state::UserAccount;
use crate::{instruction, ID};

#[vrf]
#[derive(Accounts)]
pub struct RequestRandomUpdate<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        seeds = [b"user", user.key().as_ref()],
        bump = user_account.bump,
        has_one = user,
    )]
    pub user_account: Account<'info, UserAccount>,
    /// CHECK: Validated against the default base-layer VRF queue.
    #[account(mut, address = ephemeral_vrf_sdk::consts::DEFAULT_QUEUE)]
    pub oracle_queue: AccountInfo<'info>,
}

#[vrf]
#[derive(Accounts)]
pub struct RequestRandomUpdateEr<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        seeds = [b"user", user.key().as_ref()],
        bump = user_account.bump,
        has_one = user,
    )]
    pub user_account: Account<'info, UserAccount>,
    /// CHECK: Validated against the default Ephemeral Rollup VRF queue.
    #[account(mut, address = ephemeral_vrf_sdk::consts::DEFAULT_EPHEMERAL_QUEUE)]
    pub oracle_queue: AccountInfo<'info>,
}

impl<'info> RequestRandomUpdate<'info> {
    pub fn request_random_update(&self, client_seed: u8) -> Result<()> {
        msg!("Requesting base-layer VRF update for user state data...");
        let request_ix = build_request_randomness_ix(
            self.user.key(),
            self.user_account.key(),
            self.oracle_queue.key(),
            client_seed,
        );

        self.invoke_signed_vrf(&self.user.to_account_info(), &request_ix)?;

        Ok(())
    }
}

impl<'info> RequestRandomUpdateEr<'info> {
    pub fn request_random_update_er(&self, client_seed: u8) -> Result<()> {
        msg!("Requesting Ephemeral Rollup VRF update for user state data...");
        let request_ix = build_request_randomness_ix(
            self.user.key(),
            self.user_account.key(),
            self.oracle_queue.key(),
            client_seed,
        );

        self.invoke_signed_vrf(&self.user.to_account_info(), &request_ix)?;

        Ok(())
    }
}

fn build_request_randomness_ix(
    payer: Pubkey,
    user_account: Pubkey,
    oracle_queue: Pubkey,
    client_seed: u8,
) -> Instruction {
    create_request_randomness_ix(RequestRandomnessParams {
        payer,
        oracle_queue,
        callback_program_id: ID,
        callback_discriminator: instruction::ConsumeRandomUpdate::DISCRIMINATOR.to_vec(),
        caller_seed: [client_seed; 32],
        accounts_metas: Some(vec![SerializableAccountMeta {
            pubkey: user_account,
            is_signer: false,
            is_writable: true,
        }]),
        ..Default::default()
    })
}
