use anchor_lang::prelude::*;
use anchor_lang::system_program;

use crate::state::OracleState;

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        init,
        payer = payer,
        seeds = [b"oracle_state"],
        bump,
        space = 8 + OracleState::INIT_SPACE,
    )]
    pub oracle_state: Account<'info, OracleState>,
    #[account(
        init,
        payer = payer,
        seeds = [b"treasury"],
        bump,
        space = 0,
        owner = system_program::ID
    )]
    /// CHECK: program treasury PDA used as system-owned CPI payer for the oracle interaction account.
    pub treasury: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> Initialize<'info> {
    pub fn initialize(
        &mut self,
        default_prompt: String,
        task_queue_authority: Pubkey,
        bumps: &InitializeBumps,
    ) -> Result<()> {
        require!(
            default_prompt.as_bytes().len() <= 280,
            crate::SolanaGptOracleError::PromptTooLong
        );

        self.oracle_state.bump = bumps.oracle_state;
        self.oracle_state.treasury_bump = bumps.treasury;
        self.oracle_state.requests = 0;
        self.oracle_state.llm_context = Pubkey::default();
        self.oracle_state.task_queue_authority = task_queue_authority;
        self.oracle_state.default_prompt = default_prompt;
        self.oracle_state.last_response = String::new();

        Ok(())
    }
}
