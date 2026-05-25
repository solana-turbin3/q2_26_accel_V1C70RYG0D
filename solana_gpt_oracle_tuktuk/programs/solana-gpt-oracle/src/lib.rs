#![allow(unexpected_cfgs)]
#![allow(deprecated)]

use anchor_lang::prelude::*;

pub mod instructions;
pub mod state;

pub use instructions::*;

declare_id!("HtxZqtUjDbMtkcs3e3BcBroJqXSaVKcyMjPS3QMJP8bd");

#[program]
pub mod solana_gpt_oracle_scheduler {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        default_prompt: String,
        task_queue_authority: Pubkey,
    ) -> Result<()> {
        ctx.accounts
            .initialize(default_prompt, task_queue_authority, &ctx.bumps)
    }

    pub fn create_context(ctx: Context<CreateContext>, agent_description: String) -> Result<()> {
        ctx.accounts.create_context(agent_description)
    }

    pub fn fund_treasury(ctx: Context<FundTreasury>, lamports: u64) -> Result<()> {
        ctx.accounts.fund_treasury(lamports)
    }

    pub fn request_gpt(ctx: Context<RequestGpt>) -> Result<()> {
        ctx.accounts.request_gpt()
    }

    pub fn process_oracle_callback(
        ctx: Context<ProcessOracleCallback>,
        response: String,
    ) -> Result<()> {
        ctx.accounts.process_oracle_callback(response)
    }

    pub fn schedule(ctx: Context<Schedule>, task_id: u16) -> Result<()> {
        ctx.accounts.schedule(task_id, ctx.bumps)
    }
}

#[error_code]
pub enum SolanaGptOracleError {
    #[msg("Prompt exceeds maximum size")]
    PromptTooLong,
    #[msg("Response exceeds maximum size")]
    ResponseTooLong,
    #[msg("Oracle context has not been initialized")]
    ContextNotInitialized,
    #[msg("Unable to compile TukTuk task transaction")]
    TaskCompilationFailed,
    #[msg("Funding amount must be greater than zero")]
    InvalidFundingAmount,
}
