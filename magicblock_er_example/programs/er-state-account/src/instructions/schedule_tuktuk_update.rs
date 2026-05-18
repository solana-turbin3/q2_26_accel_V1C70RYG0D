use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::{prelude::*, InstructionData};
use tuktuk_program::{
    compile_transaction,
    tuktuk::{
        cpi::{accounts::QueueTaskV0, queue_task_v0},
        program::Tuktuk,
        types::TriggerV0,
    },
    types::QueueTaskArgsV0,
    TransactionSourceV0,
};

use crate::state::UserAccount;

#[derive(Accounts)]
pub struct ScheduleTuktukUpdate<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        mut,
        seeds = [b"user", user_account.user.as_ref()],
        bump = user_account.bump,
    )]
    pub user_account: Account<'info, UserAccount>,
    /// CHECK: Passed through to TukTuk CPI.
    #[account(mut)]
    pub task_queue: UncheckedAccount<'info>,
    /// CHECK: Passed through to TukTuk CPI.
    pub task_queue_authority: UncheckedAccount<'info>,
    /// CHECK: Initialized by TukTuk CPI.
    #[account(mut)]
    pub task: UncheckedAccount<'info>,
    /// CHECK: Program PDA that is authorized on the TukTuk task queue.
    #[account(
        mut,
        seeds = [b"queue_authority"],
        bump
    )]
    pub queue_authority: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    pub tuktuk_program: Program<'info, Tuktuk>,
}

impl<'info> ScheduleTuktukUpdate<'info> {
    pub fn schedule_tuktuk_update(
        &mut self,
        task_id: u16,
        bumps: ScheduleTuktukUpdateBumps,
    ) -> Result<()> {
        let (compiled_tx, _) = compile_transaction(
            vec![Instruction {
                program_id: crate::ID,
                accounts: crate::__cpi_client_accounts_scheduled_update::ScheduledUpdate {
                    user_account: self.user_account.to_account_info(),
                }
                .to_account_metas(None)
                .to_vec(),
                data: crate::instruction::ScheduledUpdate.data(),
            }],
            vec![],
        )
        .unwrap();

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
                free_tasks: 0,
                id: task_id,
                description: "magicblock-user-data-update".to_string(),
            },
        )?;

        Ok(())
    }
}