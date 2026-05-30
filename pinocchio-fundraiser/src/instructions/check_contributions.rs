use pinocchio::{
    AccountView, ProgramResult,
    cpi::{Seed, Signer},
    error::ProgramError,
};

use crate::{
    error::{TARGET_NOT_MET, custom},
    instructions::utils::{
        close_program_account, token_amount, validate_token_account, verify_fundraiser_pda,
    },
    state::Fundraiser,
};

pub fn process_check_contributions(accounts: &mut [AccountView], _data: &[u8]) -> ProgramResult {
    let [
        maker,
        mint_to_raise,
        fundraiser,
        vault,
        maker_ata,
        token_program,
        system_program,
        _associated_token_program @ ..,
    ] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !maker.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if !fundraiser.owned_by(&crate::ID) {
        return Err(ProgramError::IllegalOwner);
    }

    let fundraiser_state = *Fundraiser::from_account_info(fundraiser)?;
    if fundraiser_state.maker() != maker.address()
        || fundraiser_state.mint_to_raise() != mint_to_raise.address()
    {
        return Err(ProgramError::InvalidAccountData);
    }
    verify_fundraiser_pda(fundraiser, maker, fundraiser_state.bump)?;
    validate_token_account(vault, fundraiser.address(), mint_to_raise.address())?;

    if !maker_ata.owned_by(&pinocchio_token::ID) {
        pinocchio_associated_token_account::instructions::Create {
            funding_account: maker,
            account: maker_ata,
            wallet: maker,
            mint: mint_to_raise,
            token_program,
            system_program,
        }
        .invoke()?;
    } else {
        validate_token_account(maker_ata, maker.address(), mint_to_raise.address())?;
    }

    let amount = token_amount(vault)?;
    if amount < fundraiser_state.amount_to_raise() {
        return Err(custom(TARGET_NOT_MET));
    }

    let bump = [fundraiser_state.bump];
    let signer_seeds = [
        Seed::from(b"fundraiser"),
        Seed::from(maker.address().as_array()),
        Seed::from(bump.as_ref()),
    ];
    let signers = [Signer::from(&signer_seeds)];

    pinocchio_token::instructions::Transfer::new(vault, maker_ata, fundraiser, amount)
        .invoke_signed(&signers)?;

    pinocchio_token::instructions::CloseAccount::new(vault, maker, fundraiser)
        .invoke_signed(&signers)?;

    close_program_account(fundraiser, maker)?;

    Ok(())
}
