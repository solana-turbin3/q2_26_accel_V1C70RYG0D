use anchor_lang::prelude::*;

use crate::state::UserAccount;

#[derive(Accounts)]
pub struct ConsumeRandomUpdate<'info> {
    #[account(address = ephemeral_vrf_sdk::consts::VRF_PROGRAM_IDENTITY)]
    pub vrf_program_identity: Signer<'info>,
    #[account(mut)]
    pub user_account: Account<'info, UserAccount>,
}

impl<'info> ConsumeRandomUpdate<'info> {
    pub fn consume_random_update(&mut self, randomness: [u8; 32]) -> Result<()> {
        let mut random_bytes = [0u8; 8];
        random_bytes.copy_from_slice(&randomness[..8]);
        let random_data = u64::from_le_bytes(random_bytes);

        self.user_account.data = random_data;

        msg!("User state data updated from VRF: {}", random_data);

        Ok(())
    }
}
