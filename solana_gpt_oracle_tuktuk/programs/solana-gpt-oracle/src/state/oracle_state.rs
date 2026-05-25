use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct OracleState {
    pub bump: u8,
    pub treasury_bump: u8,
    pub requests: u64,
    pub llm_context: Pubkey,
    pub task_queue_authority: Pubkey,
    #[max_len(280)]
    pub default_prompt: String,
    #[max_len(512)]
    pub last_response: String,
}
