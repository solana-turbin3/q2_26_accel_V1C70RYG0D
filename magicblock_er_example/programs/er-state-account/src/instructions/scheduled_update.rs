use anchor_lang::prelude::*;

use crate::state::UserAccount;

#[derive(Accounts)]
pub struct ScheduledUpdate<'info> {
    #[account(
        mut,
        seeds = [b"user", user_account.user.as_ref()],
        bump = user_account.bump,
    )]
    pub user_account: Account<'info, UserAccount>,
}

impl<'info> ScheduledUpdate<'info> {
    pub fn scheduled_update(&mut self) -> Result<()> {
        self.user_account.data = self.user_account.data.saturating_add(1);
        msg!("TukTuk scheduled update data: {}", self.user_account.data);

        Ok(())
    }
}