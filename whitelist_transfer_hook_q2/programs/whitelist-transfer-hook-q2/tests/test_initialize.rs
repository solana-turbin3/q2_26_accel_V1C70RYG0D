use {
    anchor_lang::{
        solana_program::{
            self,
            instruction::{AccountMeta, Instruction},
            pubkey::Pubkey,
        },
        InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    spl_associated_token_account_interface::{
        address::get_associated_token_address_with_program_id,
        instruction::create_associated_token_account, program::ID as ASSOCIATED_TOKEN_PROGRAM_ID,
    },
    spl_token_2022_interface::{
        extension::{
            permanent_delegate::get_permanent_delegate, BaseStateWithExtensions, ExtensionType,
            StateWithExtensions,
        },
        instruction::transfer_checked,
        state::{Account as TokenAccountState, Mint},
        ID as TOKEN_2022_ID,
    },
    whitelist_transfer_hook_q2 as program,
};

const DECIMALS: u8 = 9;
const TOKEN: u64 = 1_000_000_000;

fn send(
    svm: &mut LiteSVM,
    ixs: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
) -> litesvm::types::TransactionResult {
    svm.expire_blockhash();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

fn token_amount(svm: &LiteSVM, token_account: &Pubkey) -> u64 {
    let account = svm
        .get_account(token_account)
        .expect("token account exists");
    StateWithExtensions::<TokenAccountState>::unpack(&account.data)
        .unwrap()
        .base
        .amount
}

fn build_transfer_ix(
    source: &Pubkey,
    mint: &Pubkey,
    destination: &Pubkey,
    authority: &Pubkey,
    amount: u64,
    extra_account_meta_list: &Pubkey,
    vault: &Pubkey,
    hook_program: &Pubkey,
) -> Instruction {
    let mut ix = transfer_checked(
        &TOKEN_2022_ID,
        source,
        mint,
        destination,
        authority,
        &[],
        amount,
        DECIMALS,
    )
    .unwrap();

    ix.accounts
        .push(AccountMeta::new_readonly(*extra_account_meta_list, false));
    ix.accounts.push(AccountMeta::new_readonly(*vault, false));
    ix.accounts
        .push(AccountMeta::new_readonly(*hook_program, false));

    ix
}

#[test]
fn test_full_flow() {
    let mut svm = LiteSVM::new();
    let admin = Keypair::new();
    let user = Keypair::new();
    let blocked_user = Keypair::new();

    let program_id = program::id();
    let bytes = include_bytes!("../../../target/deploy/whitelist_transfer_hook_q2.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&admin.pubkey(), 10_000_000_000).unwrap();

    let (vault_pda, _) = Pubkey::find_program_address(&[b"vault"], &program_id);
    let (mint_pda, _) = Pubkey::find_program_address(&[b"mint"], &program_id);
    let (extra_meta_pda, _) =
        Pubkey::find_program_address(&[b"extra-account-metas", mint_pda.as_ref()], &program_id);
    let vault_ata =
        get_associated_token_address_with_program_id(&vault_pda, &mint_pda, &TOKEN_2022_ID);
    let user_ata =
        get_associated_token_address_with_program_id(&user.pubkey(), &mint_pda, &TOKEN_2022_ID);
    let blocked_user_ata = get_associated_token_address_with_program_id(
        &blocked_user.pubkey(),
        &mint_pda,
        &TOKEN_2022_ID,
    );
    let system_program_id = solana_program::system_program::id();

    let ix = Instruction::new_with_bytes(
        program_id,
        &program::instruction::InitializeVault {}.data(),
        program::accounts::InitializeVault {
            admin: admin.pubkey(),
            vault: vault_pda,
            mint: mint_pda,
            vault_token_account: vault_ata,
            extra_account_meta_list: extra_meta_pda,
            token_program: TOKEN_2022_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            system_program: system_program_id,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &[ix], &admin, &[&admin]).expect("initialize_vault failed");

    let mint_account = svm.get_account(&mint_pda).expect("mint exists");
    let mint_state = StateWithExtensions::<Mint>::unpack(&mint_account.data).unwrap();
    let extension_types = mint_state.get_extension_types().unwrap();
    assert!(extension_types.contains(&ExtensionType::TransferHook));
    assert!(extension_types.contains(&ExtensionType::PermanentDelegate));
    assert_eq!(get_permanent_delegate(&mint_state), Some(admin.pubkey()));

    let create_user_ata =
        create_associated_token_account(&admin.pubkey(), &user.pubkey(), &mint_pda, &TOKEN_2022_ID);
    let create_blocked_user_ata = create_associated_token_account(
        &admin.pubkey(),
        &blocked_user.pubkey(),
        &mint_pda,
        &TOKEN_2022_ID,
    );
    send(
        &mut svm,
        &[create_user_ata, create_blocked_user_ata],
        &admin,
        &[&admin],
    )
    .expect("create user token accounts failed");

    let initial_amount = 100 * TOKEN;
    let mint_to_user = Instruction::new_with_bytes(
        program_id,
        &program::instruction::MintTokens {
            amount: initial_amount,
        }
        .data(),
        program::accounts::MintTokens {
            admin: admin.pubkey(),
            vault: vault_pda,
            mint: mint_pda,
            recipient_token_account: user_ata,
            recipient: user.pubkey(),
            token_program: TOKEN_2022_ID,
        }
        .to_account_metas(None),
    );
    let mint_to_blocked_user = Instruction::new_with_bytes(
        program_id,
        &program::instruction::MintTokens {
            amount: initial_amount,
        }
        .data(),
        program::accounts::MintTokens {
            admin: admin.pubkey(),
            vault: vault_pda,
            mint: mint_pda,
            recipient_token_account: blocked_user_ata,
            recipient: blocked_user.pubkey(),
            token_program: TOKEN_2022_ID,
        }
        .to_account_metas(None),
    );
    send(
        &mut svm,
        &[mint_to_user, mint_to_blocked_user],
        &admin,
        &[&admin],
    )
    .expect("program mint failed");
    assert_eq!(token_amount(&svm, &user_ata), initial_amount);
    assert_eq!(token_amount(&svm, &blocked_user_ata), initial_amount);

    let blocked_deposit = build_transfer_ix(
        &blocked_user_ata,
        &mint_pda,
        &vault_ata,
        &blocked_user.pubkey(),
        TOKEN,
        &extra_meta_pda,
        &vault_pda,
        &program_id,
    );
    assert!(
        send(
            &mut svm,
            &[blocked_deposit],
            &admin,
            &[&admin, &blocked_user]
        )
        .is_err(),
        "non-whitelisted deposits must fail"
    );

    let allowance = 50 * TOKEN;
    let ix = Instruction::new_with_bytes(
        program_id,
        &program::instruction::AddToWhitelist {
            user: user.pubkey(),
            amount: allowance,
        }
        .data(),
        program::accounts::AddToWhitelist {
            admin: admin.pubkey(),
            vault: vault_pda,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &[ix], &admin, &[&admin]).expect("add_to_whitelist failed");

    let too_large_deposit = build_transfer_ix(
        &user_ata,
        &mint_pda,
        &vault_ata,
        &user.pubkey(),
        allowance + TOKEN,
        &extra_meta_pda,
        &vault_pda,
        &program_id,
    );
    assert!(
        send(&mut svm, &[too_large_deposit], &admin, &[&admin, &user]).is_err(),
        "deposits above the whitelist amount must fail"
    );

    let deposit_amount = 40 * TOKEN;
    let deposit = build_transfer_ix(
        &user_ata,
        &mint_pda,
        &vault_ata,
        &user.pubkey(),
        deposit_amount,
        &extra_meta_pda,
        &vault_pda,
        &program_id,
    );
    send(&mut svm, &[deposit], &admin, &[&admin, &user]).expect("deposit failed");
    assert_eq!(
        token_amount(&svm, &user_ata),
        initial_amount - deposit_amount
    );
    assert_eq!(token_amount(&svm, &vault_ata), deposit_amount);

    let ix = Instruction::new_with_bytes(
        program_id,
        &program::instruction::RemoveFromWhitelist {
            user: user.pubkey(),
        }
        .data(),
        program::accounts::RemoveFromWhitelist {
            admin: admin.pubkey(),
            vault: vault_pda,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &[ix], &admin, &[&admin]).expect("remove_from_whitelist failed");

    let withdraw_amount = 10 * TOKEN;
    let withdraw_while_removed = build_transfer_ix(
        &vault_ata,
        &mint_pda,
        &user_ata,
        &admin.pubkey(),
        withdraw_amount,
        &extra_meta_pda,
        &vault_pda,
        &program_id,
    );
    assert!(
        send(&mut svm, &[withdraw_while_removed], &admin, &[&admin]).is_err(),
        "withdrawals after whitelist removal must fail"
    );

    let ix = Instruction::new_with_bytes(
        program_id,
        &program::instruction::AddToWhitelist {
            user: user.pubkey(),
            amount: allowance,
        }
        .data(),
        program::accounts::AddToWhitelist {
            admin: admin.pubkey(),
            vault: vault_pda,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &[ix], &admin, &[&admin]).expect("re-add_to_whitelist failed");

    let withdraw = build_transfer_ix(
        &vault_ata,
        &mint_pda,
        &user_ata,
        &admin.pubkey(),
        withdraw_amount,
        &extra_meta_pda,
        &vault_pda,
        &program_id,
    );
    send(&mut svm, &[withdraw], &admin, &[&admin]).expect("withdraw failed");
    assert_eq!(
        token_amount(&svm, &user_ata),
        initial_amount - deposit_amount + withdraw_amount
    );
    assert_eq!(
        token_amount(&svm, &vault_ata),
        deposit_amount - withdraw_amount
    );
}
