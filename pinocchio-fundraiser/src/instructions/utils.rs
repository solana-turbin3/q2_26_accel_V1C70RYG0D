use pinocchio::{AccountView, Address, ProgramResult, error::ProgramError};
use pinocchio_pubkey::derive_address;
use pinocchio_token::state::Account;

use crate::{constants::SECONDS_TO_DAYS, state::Fundraiser};

pub fn read_u64(data: &[u8], offset: usize) -> Result<u64, ProgramError> {
    let bytes = data
        .get(offset..offset + 8)
        .ok_or(ProgramError::InvalidInstructionData)?;
    Ok(u64::from_le_bytes(
        bytes
            .try_into()
            .map_err(|_| ProgramError::InvalidInstructionData)?,
    ))
}

pub fn pow10(decimals: u8) -> Result<u64, ProgramError> {
    10_u64
        .checked_pow(decimals as u32)
        .ok_or(ProgramError::ArithmeticOverflow)
}

pub fn has_ended(fundraiser: &Fundraiser, now: i64) -> bool {
    fundraiser.duration() != 0
        && now.saturating_sub(fundraiser.time_started()) / SECONDS_TO_DAYS
            >= fundraiser.duration() as i64
}

pub fn verify_fundraiser_pda(
    fundraiser: &AccountView,
    maker: &AccountView,
    bump: u8,
) -> ProgramResult {
    verify_fundraiser_pda_with_maker(fundraiser, maker.address(), bump)
}

pub fn verify_fundraiser_pda_with_maker(
    fundraiser: &AccountView,
    maker: &Address,
    bump: u8,
) -> ProgramResult {
    let seeds = [b"fundraiser".as_ref(), maker.as_ref(), &[bump]];
    let pda = derive_address(&seeds, None, &crate::ID.to_bytes());
    if pda != *fundraiser.address().as_array() {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(())
}

pub fn verify_contributor_pda(
    contributor_account: &AccountView,
    fundraiser: &AccountView,
    contributor: &AccountView,
    bump: u8,
) -> ProgramResult {
    let seeds = [
        b"contributor".as_ref(),
        fundraiser.address().as_ref(),
        contributor.address().as_ref(),
        &[bump],
    ];
    let pda = derive_address(&seeds, None, &crate::ID.to_bytes());
    if pda != *contributor_account.address().as_array() {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(())
}

pub fn validate_token_account(
    account: &AccountView,
    owner: &Address,
    mint: &Address,
) -> ProgramResult {
    let token_account = Account::from_account_view(account)?;
    if token_account.owner() != owner || token_account.mint() != mint {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(())
}

pub fn token_amount(account: &AccountView) -> Result<u64, ProgramError> {
    Ok(Account::from_account_view(account)?.amount())
}

pub fn close_program_account(
    account: &mut AccountView,
    destination: &mut AccountView,
) -> ProgramResult {
    let lamports = account.lamports();
    destination.set_lamports(
        destination
            .lamports()
            .checked_add(lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?,
    );
    account.set_lamports(0);
    account.close()
}
