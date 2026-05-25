use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::{prelude::*, InstructionData};
use tuktuk_program::{
    compile_transaction,
    tuktuk::{
        cpi::{accounts::QueueTaskV0, queue_task_v0},
        program::Tuktuk,
        types::TriggerV0,
    },
    types::{QueueTaskArgsV0, TransactionSourceV0},
};

use crate::state::OracleState;

#[derive(Accounts)]
pub struct Schedule<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        mut,
        seeds = [b"oracle_state"],
        bump = oracle_state.bump,
    )]
    pub oracle_state: Account<'info, OracleState>,
    #[account(
        seeds = [b"treasury"],
        bump = oracle_state.treasury_bump,
    )]
    /// CHECK: system-owned oracle payer PDA referenced by the compiled task.
    pub treasury: UncheckedAccount<'info>,
    #[account(address = oracle_state.llm_context)]
    pub llm_context: Account<'info, solana_gpt_oracle::ContextAccount>,
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
    /// CHECK: created by the oracle program during the queued request.
    pub interaction: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: TukTuk task queue, validated by the TukTuk CPI.
    pub task_queue: UncheckedAccount<'info>,
    #[account(address = oracle_state.task_queue_authority)]
    /// CHECK: configured task queue authority used by the TukTuk CPI.
    pub task_queue_authority: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: initialized by the TukTuk CPI.
    pub task: UncheckedAccount<'info>,
    #[account(
        seeds = [b"queue_authority"],
        bump
    )]
    /// CHECK: program PDA signer for the queue_task_v0 CPI.
    pub queue_authority: UncheckedAccount<'info>,
    pub oracle_program: Program<'info, solana_gpt_oracle::program::SolanaGptOracle>,
    pub system_program: Program<'info, System>,
    pub tuktuk_program: Program<'info, Tuktuk>,
}

impl<'info> Schedule<'info> {
    pub fn schedule(&mut self, task_id: u16, bumps: ScheduleBumps) -> Result<()> {
        require!(
            self.oracle_state.llm_context != Pubkey::default(),
            crate::SolanaGptOracleError::ContextNotInitialized
        );

        let (compiled_tx, _) = compile_transaction(
            vec![Instruction {
                program_id: crate::ID,
                accounts: crate::__cpi_client_accounts_request_gpt::RequestGpt {
                    oracle_state: self.oracle_state.to_account_info(),
                    treasury: self.treasury.to_account_info(),
                    interaction: self.interaction.to_account_info(),
                    llm_context: self.llm_context.to_account_info(),
                    task_queue_authority: self.task_queue_authority.to_account_info(),
                    system_program: self.system_program.to_account_info(),
                    oracle_program: self.oracle_program.to_account_info(),
                }
                .to_account_metas(None)
                .to_vec(),
                data: crate::instruction::RequestGpt {}.data(),
            }],
            vec![],
        )
        .map_err(|_| error!(crate::SolanaGptOracleError::TaskCompilationFailed))?;

        queue_task_v0(
            CpiContext::new_with_signer(
                self.tuktuk_program.to_account_info(),
                QueueTaskV0 {
                    payer: self.payer.to_account_info(),
                    queue_authority: self.queue_authority.to_account_info(),
                    task_queue: self.task_queue.to_account_info(),
                    task_queue_authority: self.task_queue_authority.to_account_info(),
                    task: self.task.to_account_info(),
                    system_program: self.system_program.to_account_info(),
                },
                &[&[b"queue_authority", &[bumps.queue_authority]]],
            ),
            QueueTaskArgsV0 {
                trigger: TriggerV0::Now,
                transaction: TransactionSourceV0::CompiledV0(compiled_tx),
                crank_reward: Some(1_000_001),
                free_tasks: 1,
                id: task_id,
                description: "solana-gpt-oracle request".to_string(),
            },
        )?;

        Ok(())
    }
}
