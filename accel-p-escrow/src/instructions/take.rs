use pinocchio::{
    AccountView, ProgramResult,
    cpi::{Seed, Signer},
    error::ProgramError,
};
use pinocchio_pubkey::derive_address;

use crate::state::Escrow;

pub fn process_take_instruction(accounts: &mut [AccountView], _data: &[u8]) -> ProgramResult {
    let [
        taker,
        maker,
        mint_a,
        mint_b,
        escrow_account,
        taker_ata_a,
        taker_ata_b,
        maker_ata_b,
        escrow_ata,
        token_program,
        ..,
    ] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !taker.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if !escrow_account.owned_by(&crate::ID) {
        return Err(ProgramError::IllegalOwner);
    }

    let escrow = *Escrow::from_account_info(escrow_account)?;
    if escrow.maker() != maker.address()
        || escrow.mint_a() != mint_a.address()
        || escrow.mint_b() != mint_b.address()
    {
        return Err(ProgramError::InvalidAccountData);
    }

    verify_escrow_pda(escrow_account, maker, escrow.bump)?;

    {
        let taker_ata_a_state = pinocchio_token::state::Account::from_account_view(taker_ata_a)?;
        if taker_ata_a_state.owner() != taker.address()
            || taker_ata_a_state.mint() != mint_a.address()
        {
            return Err(ProgramError::InvalidAccountData);
        }
    }
    {
        let taker_ata_b_state = pinocchio_token::state::Account::from_account_view(taker_ata_b)?;
        if taker_ata_b_state.owner() != taker.address()
            || taker_ata_b_state.mint() != mint_b.address()
        {
            return Err(ProgramError::InvalidAccountData);
        }
    }
    {
        let maker_ata_b_state = pinocchio_token::state::Account::from_account_view(maker_ata_b)?;
        if maker_ata_b_state.owner() != maker.address()
            || maker_ata_b_state.mint() != mint_b.address()
        {
            return Err(ProgramError::InvalidAccountData);
        }
    }
    {
        let escrow_ata_state = pinocchio_token::state::Account::from_account_view(escrow_ata)?;
        if escrow_ata_state.owner() != escrow_account.address()
            || escrow_ata_state.mint() != mint_a.address()
        {
            return Err(ProgramError::InvalidAccountData);
        }
    }

    pinocchio_token::instructions::Transfer::new(
        taker_ata_b,
        maker_ata_b,
        taker,
        escrow.amount_to_receive(),
    )
    .invoke()?;

    let bump = [escrow.bump];
    let signer_seeds = [
        Seed::from(b"escrow"),
        Seed::from(maker.address().as_array()),
        Seed::from(bump.as_ref()),
    ];
    let signers = [Signer::from(&signer_seeds)];

    pinocchio_token::instructions::Transfer::new(
        escrow_ata,
        taker_ata_a,
        escrow_account,
        escrow.amount_to_give(),
    )
    .invoke_signed(&signers)?;

    pinocchio_token::instructions::CloseAccount::new(escrow_ata, maker, escrow_account)
        .invoke_signed(&signers)?;

    close_escrow_account(escrow_account, maker)?;

    let _ = token_program;
    Ok(())
}

fn verify_escrow_pda(escrow_account: &AccountView, maker: &AccountView, bump: u8) -> ProgramResult {
    let seed = [b"escrow".as_ref(), maker.address().as_ref(), &[bump]];
    let pda = derive_address(&seed, None, &crate::ID.to_bytes());
    if pda != *escrow_account.address().as_array() {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(())
}

fn close_escrow_account(
    escrow_account: &mut AccountView,
    destination: &mut AccountView,
) -> ProgramResult {
    let lamports = escrow_account.lamports();
    destination.set_lamports(
        destination
            .lamports()
            .checked_add(lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?,
    );
    escrow_account.set_lamports(0);
    escrow_account.close()
}
