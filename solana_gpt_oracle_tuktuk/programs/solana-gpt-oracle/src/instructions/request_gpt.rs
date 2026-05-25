use anchor_lang::{prelude::*, InstructionData};

use crate::state::OracleState;

#[derive(Accounts)]
pub struct RequestGpt<'info> {
    #[account(
        mut,
        seeds = [b"oracle_state"],
        bump = oracle_state.bump,
    )]
    pub oracle_state: Account<'info, OracleState>,
    #[account(
        mut,
        seeds = [b"treasury"],
        bump = oracle_state.treasury_bump,
    )]
    /// CHECK: system-owned PDA signer used to pay oracle interaction rent.
    pub treasury: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [
            solana_gpt_oracle::Interaction::seed(),
            treasury.key().as_ref(),
            llm_context.key().as_ref(),
        ],
        bump,
        seeds::program = oracle_program.key(),
    )]
    /// CHECK: initialized/reallocated inside the oracle program CPI.
    pub interaction: UncheckedAccount<'info>,
    #[account(
        mut,
        address = oracle_state.llm_context,
    )]
    pub llm_context: Account<'info, solana_gpt_oracle::ContextAccount>,
    #[account(address = oracle_state.task_queue_authority)]
    pub task_queue_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub oracle_program: Program<'info, solana_gpt_oracle::program::SolanaGptOracle>,
}

impl<'info> RequestGpt<'info> {
    pub fn request_gpt(&mut self) -> Result<()> {
        require!(
            self.oracle_state.llm_context != Pubkey::default(),
            crate::SolanaGptOracleError::ContextNotInitialized
        );

        let callback_accounts = vec![solana_gpt_oracle::AccountMeta {
            pubkey: self.oracle_state.key(),
            is_signer: false,
            is_writable: true,
        }];

        let callback_data = crate::instruction::ProcessOracleCallback {
            response: String::new(),
        }
        .data();
        let mut callback_discriminator = [0u8; 8];
        callback_discriminator.copy_from_slice(&callback_data[..8]);

        solana_gpt_oracle::cpi::interact_with_llm(
            CpiContext::new_with_signer(
                self.oracle_program.to_account_info(),
                solana_gpt_oracle::cpi::accounts::InteractWithLlm {
                    payer: self.treasury.to_account_info(),
                    interaction: self.interaction.to_account_info(),
                    context_account: self.llm_context.to_account_info(),
                    system_program: self.system_program.to_account_info(),
                },
                &[&[b"treasury", &[self.oracle_state.treasury_bump]]],
            ),
            self.oracle_state.default_prompt.clone(),
            crate::ID,
            callback_discriminator,
            Some(callback_accounts),
        )?;

        self.oracle_state.requests = self.oracle_state.requests.saturating_add(1);

        Ok(())
    }
}
