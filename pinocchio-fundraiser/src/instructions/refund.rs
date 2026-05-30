use pinocchio::{
    AccountView, ProgramResult,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{Sysvar, clock::Clock},
};

use crate::{
    error::{FUNDRAISER_NOT_ENDED, TARGET_MET, custom},
    instructions::utils::{
        close_program_account, has_ended, token_amount, validate_token_account,
        verify_contributor_pda, verify_fundraiser_pda,
    },
    state::{Contributor, Fundraiser},
};

pub fn process_refund(accounts: &mut [AccountView], _data: &[u8]) -> ProgramResult {
    let [
        contributor,
        maker,
        mint_to_raise,
        fundraiser,
        contributor_account,
        contributor_ata,
        vault,
        token_program,
        _system_program @ ..,
    ] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !contributor.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if !fundraiser.owned_by(&crate::ID) || !contributor_account.owned_by(&crate::ID) {
        return Err(ProgramError::IllegalOwner);
    }

    let fundraiser_state = *Fundraiser::from_account_info(fundraiser)?;
    if fundraiser_state.maker() != maker.address()
        || fundraiser_state.mint_to_raise() != mint_to_raise.address()
    {
        return Err(ProgramError::InvalidAccountData);
    }
    verify_fundraiser_pda(fundraiser, maker, fundraiser_state.bump)?;

    let contributor_state = *Contributor::from_account_info(contributor_account)?;
    verify_contributor_pda(
        contributor_account,
        fundraiser,
        contributor,
        contributor_state.bump,
    )?;

    validate_token_account(
        contributor_ata,
        contributor.address(),
        mint_to_raise.address(),
    )?;
    validate_token_account(vault, fundraiser.address(), mint_to_raise.address())?;

    let now = Clock::get()?.unix_timestamp;
    if !has_ended(&fundraiser_state, now) {
        return Err(custom(FUNDRAISER_NOT_ENDED));
    }

    if token_amount(vault)? >= fundraiser_state.amount_to_raise() {
        return Err(custom(TARGET_MET));
    }

    let bump = [fundraiser_state.bump];
    let signer_seeds = [
        Seed::from(b"fundraiser"),
        Seed::from(maker.address().as_array()),
        Seed::from(bump.as_ref()),
    ];
    let signers = [Signer::from(&signer_seeds)];

    pinocchio_token::instructions::Transfer::new(
        vault,
        contributor_ata,
        fundraiser,
        contributor_state.amount(),
    )
    .invoke_signed(&signers)?;

    let new_current_amount = fundraiser_state
        .current_amount()
        .checked_sub(contributor_state.amount())
        .ok_or(ProgramError::ArithmeticOverflow)?;
    Fundraiser::from_account_info(fundraiser)?.set_current_amount(new_current_amount);

    close_program_account(contributor_account, contributor)?;

    let _ = token_program;
    Ok(())
}
