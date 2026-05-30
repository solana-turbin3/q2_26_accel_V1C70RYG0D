use pinocchio::{
    AccountView, ProgramResult,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{Sysvar, clock::Clock, rent::Rent},
};
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::state::Mint;

use crate::{
    constants::MIN_AMOUNT_TO_RAISE,
    error::{INVALID_AMOUNT, custom},
    instructions::utils::{pow10, read_u64, verify_fundraiser_pda},
    state::Fundraiser,
};

pub fn process_initialize(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [
        maker,
        mint_to_raise,
        fundraiser,
        vault,
        system_program,
        token_program,
        _associated_token_program @ ..,
    ] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if data.len() != 10 {
        return Err(ProgramError::InvalidInstructionData);
    }
    if !maker.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if fundraiser.owned_by(&crate::ID) {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    let bump = data[0];
    let amount = read_u64(data, 1)?;
    let duration = data[9];

    verify_fundraiser_pda(fundraiser, maker, bump)?;

    {
        let mint = Mint::from_account_view(mint_to_raise)?;
        let minimum = MIN_AMOUNT_TO_RAISE
            .checked_mul(pow10(mint.decimals())?)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        if amount <= minimum {
            return Err(custom(INVALID_AMOUNT));
        }
    }

    let bump_bytes = [bump];
    let signer_seeds = [
        Seed::from(b"fundraiser"),
        Seed::from(maker.address().as_array()),
        Seed::from(bump_bytes.as_ref()),
    ];
    let signers = [Signer::from(&signer_seeds)];

    CreateAccount {
        from: maker,
        to: fundraiser,
        lamports: Rent::get()?.try_minimum_balance(Fundraiser::LEN)?,
        space: Fundraiser::LEN as u64,
        owner: &crate::ID,
    }
    .invoke_signed(&signers)?;

    pinocchio_associated_token_account::instructions::Create {
        funding_account: maker,
        account: vault,
        wallet: fundraiser,
        mint: mint_to_raise,
        token_program,
        system_program,
    }
    .invoke()?;

    let state = Fundraiser::from_account_info(fundraiser)?;
    state.set_maker(maker.address());
    state.set_mint_to_raise(mint_to_raise.address());
    state.set_amount_to_raise(amount);
    state.set_current_amount(0);
    state.set_time_started(Clock::get()?.unix_timestamp);
    state.set_duration(duration);
    state.bump = bump;

    Ok(())
}
