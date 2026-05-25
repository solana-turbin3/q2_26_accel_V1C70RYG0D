use anchor_lang::prelude::*;

use crate::state::OracleState;

#[derive(Accounts)]
pub struct ProcessOracleCallback<'info> {
    #[account(
        seeds = [b"identity"],
        bump,
        seeds::program = oracle_program.key(),
    )]
    pub identity: Signer<'info>,
    #[account(
        mut,
        seeds = [b"oracle_state"],
        bump = oracle_state.bump,
    )]
    pub oracle_state: Account<'info, OracleState>,
    pub oracle_program: Program<'info, solana_gpt_oracle::program::SolanaGptOracle>,
}

impl<'info> ProcessOracleCallback<'info> {
    pub fn process_oracle_callback(&mut self, response: String) -> Result<()> {
        require!(
            response.as_bytes().len() <= 512,
            crate::SolanaGptOracleError::ResponseTooLong
        );

        msg!("GPT oracle response: {}", response);
        self.oracle_state.last_response = response;

        Ok(())
    }
}
