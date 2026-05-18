#![allow(unexpected_cfgs)]
#![allow(deprecated)]

use anchor_lang::prelude::*;
use ephemeral_rollups_sdk::anchor::ephemeral;

mod instructions;
mod state;

use instructions::*;

declare_id!("9ChqoFDgVmmvD6Hcajv2JppVZ7S1qPDozrTw4V7q2yLP");

#[ephemeral]
#[program]
pub mod er_state_account {

    use super::*;

    pub fn initialize(ctx: Context<InitUser>) -> Result<()> {
        ctx.accounts.initialize(&ctx.bumps)?;

        Ok(())
    }

    pub fn update(ctx: Context<UpdateUser>, new_data: u64) -> Result<()> {
        ctx.accounts.update(new_data)?;

        Ok(())
    }

    pub fn update_commit(ctx: Context<UpdateCommit>, new_data: u64) -> Result<()> {
        ctx.accounts.update_commit(new_data)?;

        Ok(())
    }

    pub fn request_random_update(ctx: Context<RequestRandomUpdate>, client_seed: u8) -> Result<()> {
        ctx.accounts.request_random_update(client_seed)?;

        Ok(())
    }

    pub fn request_random_update_er(
        ctx: Context<RequestRandomUpdateEr>,
        client_seed: u8,
    ) -> Result<()> {
        ctx.accounts.request_random_update_er(client_seed)?;

        Ok(())
    }

    pub fn consume_random_update(
        ctx: Context<ConsumeRandomUpdate>,
        randomness: [u8; 32],
    ) -> Result<()> {
        ctx.accounts.consume_random_update(randomness)?;

        Ok(())
    }

    pub fn scheduled_update(ctx: Context<ScheduledUpdate>) -> Result<()> {
        ctx.accounts.scheduled_update()?;

        Ok(())
    }

    pub fn schedule_tuktuk_update(
        ctx: Context<ScheduleTuktukUpdate>,
        task_id: u16,
    ) -> Result<()> {
        ctx.accounts.schedule_tuktuk_update(task_id, ctx.bumps)?;

        Ok(())
    }

    pub fn delegate(ctx: Context<Delegate>) -> Result<()> {
        ctx.accounts.delegate()?;

        Ok(())
    }

    pub fn undelegate(ctx: Context<Undelegate>) -> Result<()> {
        ctx.accounts.undelegate()?;

        Ok(())
    }

    pub fn close(ctx: Context<CloseUser>) -> Result<()> {
        ctx.accounts.close()?;

        Ok(())
    }
}
