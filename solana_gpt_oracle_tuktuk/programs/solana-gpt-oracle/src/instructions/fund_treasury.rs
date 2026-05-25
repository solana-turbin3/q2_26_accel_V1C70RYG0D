use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct FundTreasury<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        mut,
        seeds = [b"treasury"],
        bump,
    )]
    /// CHECK: system-owned treasury PDA used by the oracle CPI to pay interaction rent.
    pub treasury: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> FundTreasury<'info> {
    pub fn fund_treasury(&mut self, lamports: u64) -> Result<()> {
        require!(
            lamports > 0,
            crate::SolanaGptOracleError::InvalidFundingAmount
        );

        anchor_lang::system_program::transfer(
            CpiContext::new(
                self.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: self.payer.to_account_info(),
                    to: self.treasury.to_account_info(),
                },
            ),
            lamports,
        )?;

        Ok(())
    }
}
