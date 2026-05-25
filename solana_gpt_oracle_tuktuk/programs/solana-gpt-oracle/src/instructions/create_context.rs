use anchor_lang::prelude::*;

use crate::state::OracleState;

#[derive(Accounts)]
#[instruction(agent_description: String)]
pub struct CreateContext<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        mut,
        seeds = [b"oracle_state"],
        bump = oracle_state.bump,
    )]
    pub oracle_state: Account<'info, OracleState>,
    #[account(
        mut,
        seeds = [b"counter"],
        bump,
        seeds::program = oracle_program.key(),
    )]
    pub oracle_counter: Account<'info, solana_gpt_oracle::Counter>,
    #[account(
        mut,
        seeds = [
            solana_gpt_oracle::ContextAccount::seed(),
            oracle_counter.count.to_le_bytes().as_ref(),
        ],
        bump,
        seeds::program = oracle_program.key(),
    )]
    /// CHECK: PDA fully constrained by the oracle program seeds and initialized via CPI.
    pub llm_context: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    pub oracle_program: Program<'info, solana_gpt_oracle::program::SolanaGptOracle>,
}

impl<'info> CreateContext<'info> {
    pub fn create_context(&mut self, agent_description: String) -> Result<()> {
        require!(
            agent_description.as_bytes().len() <= 280,
            crate::SolanaGptOracleError::PromptTooLong
        );

        solana_gpt_oracle::cpi::create_llm_context(
            CpiContext::new(
                self.oracle_program.to_account_info(),
                solana_gpt_oracle::cpi::accounts::CreateLlmContext {
                    payer: self.payer.to_account_info(),
                    context_account: self.llm_context.to_account_info(),
                    counter: self.oracle_counter.to_account_info(),
                    system_program: self.system_program.to_account_info(),
                },
            ),
            agent_description.clone(),
        )?;

        self.oracle_state.llm_context = self.llm_context.key();
        self.oracle_state.default_prompt = agent_description;

        Ok(())
    }
}
