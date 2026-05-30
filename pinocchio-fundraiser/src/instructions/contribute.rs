use pinocchio::{
    AccountView, ProgramResult,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{Sysvar, clock::Clock, rent::Rent},
};
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::state::Mint;

use crate::{
    constants::{MAX_CONTRIBUTION_PERCENTAGE, PERCENTAGE_SCALER},
    error::{
        CONTRIBUTION_TOO_BIG, CONTRIBUTION_TOO_SMALL, FUNDRAISER_ENDED,
        MAXIMUM_CONTRIBUTIONS_REACHED, custom,
    },
    instructions::utils::{
        has_ended, pow10, read_u64, validate_token_account, verify_contributor_pda,
        verify_fundraiser_pda_with_maker,
    },
    state::{Contributor, Fundraiser},
};

pub fn process_contribute(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [
        contributor,
        mint_to_raise,
        fundraiser,
        contributor_account,
        contributor_ata,
        vault,
        system_program,
        token_program,
        ..,
    ] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if data.len() != 9 {
        return Err(ProgramError::InvalidInstructionData);
    }
    if !contributor.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if !fundraiser.owned_by(&crate::ID) {
        return Err(ProgramError::IllegalOwner);
    }

    let contributor_bump = data[0];
    let amount = read_u64(data, 1)?;
    let fundraiser_state = *Fundraiser::from_account_info(fundraiser)?;

    if fundraiser_state.mint_to_raise() != mint_to_raise.address() {
        return Err(ProgramError::InvalidAccountData);
    }
    verify_fundraiser_pda_with_maker(fundraiser, fundraiser_state.maker(), fundraiser_state.bump)?;

    create_contributor_if_needed(
        contributor,
        fundraiser,
        contributor_account,
        system_program,
        contributor_bump,
    )?;

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
    if has_ended(&fundraiser_state, now) {
        return Err(custom(FUNDRAISER_ENDED));
    }

    {
        let mint = Mint::from_account_view(mint_to_raise)?;
        if amount < pow10(mint.decimals())? {
            return Err(custom(CONTRIBUTION_TOO_SMALL));
        }
    }

    let max_contribution = fundraiser_state
        .amount_to_raise()
        .checked_mul(MAX_CONTRIBUTION_PERCENTAGE)
        .ok_or(ProgramError::ArithmeticOverflow)?
        / PERCENTAGE_SCALER;

    if amount > max_contribution {
        return Err(custom(CONTRIBUTION_TOO_BIG));
    }

    let new_contributor_amount = contributor_state
        .amount()
        .checked_add(amount)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    if new_contributor_amount > max_contribution {
        return Err(custom(MAXIMUM_CONTRIBUTIONS_REACHED));
    }

    let new_current_amount = fundraiser_state
        .current_amount()
        .checked_add(amount)
        .ok_or(ProgramError::ArithmeticOverflow)?;

    pinocchio_token::instructions::Transfer::new(contributor_ata, vault, contributor, amount)
        .invoke()?;

    Fundraiser::from_account_info(fundraiser)?.set_current_amount(new_current_amount);
    Contributor::from_account_info(contributor_account)?.set_amount(new_contributor_amount);

    let _ = token_program;
    Ok(())
}

fn create_contributor_if_needed(
    contributor: &AccountView,
    fundraiser: &AccountView,
    contributor_account: &mut AccountView,
    system_program: &AccountView,
    bump: u8,
) -> ProgramResult {
    if contributor_account.owned_by(&crate::ID) {
        return Ok(());
    }

    verify_contributor_pda(contributor_account, fundraiser, contributor, bump)?;

    let bump_bytes = [bump];
    let signer_seeds = [
        Seed::from(b"contributor"),
        Seed::from(fundraiser.address().as_array()),
        Seed::from(contributor.address().as_array()),
        Seed::from(bump_bytes.as_ref()),
    ];
    let signers = [Signer::from(&signer_seeds)];

    CreateAccount {
        from: contributor,
        to: contributor_account,
        lamports: Rent::get()?.try_minimum_balance(Contributor::LEN)?,
        space: Contributor::LEN as u64,
        owner: &crate::ID,
    }
    .invoke_signed(&signers)?;

    let state = Contributor::from_account_info(contributor_account)?;
    state.set_amount(0);
    state.bump = bump;

    let _ = system_program;
    Ok(())
}
