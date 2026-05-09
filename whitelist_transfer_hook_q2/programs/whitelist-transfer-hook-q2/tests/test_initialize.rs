use {
    anchor_lang::{
        solana_program::{
            self,
            instruction::{AccountMeta, Instruction},
            pubkey::Pubkey,
            system_instruction,
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
        instruction::create_associated_token_account,
    },
    spl_token_2022_interface::{
        extension::{transfer_hook::instruction::initialize as init_transfer_hook, ExtensionType},
        instruction::{initialize_mint2, mint_to, transfer_checked},
        state::Mint,
        ID as TOKEN_2022_ID,
    },
    whitelist_transfer_hook_q2 as program,
};

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

#[test]
fn test_full_flow() {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    let recipient = Keypair::new();

    let program_id = program::id();
    let bytes = include_bytes!("../../../target/deploy/whitelist_transfer_hook_q2.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let (whitelist_config_pda, _) = Pubkey::find_program_address(&[b"whitelist"], &program_id);
    let (payer_whitelist_pda, _) =
        Pubkey::find_program_address(&[b"whitelist", payer.pubkey().as_ref()], &program_id);
    let system_program_id = solana_program::system_program::id();

    // Step 1: Initialize whitelist
    let ix = Instruction::new_with_bytes(
        program_id,
        &program::instruction::InitializeWhitelist {}.data(),
        program::accounts::InitializeWhitelist {
            admin: payer.pubkey(),
            whitelist: whitelist_config_pda,
            system_program: system_program_id,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &[ix], &payer, &[&payer]).expect("initialize_whitelist failed");

    // Step 2: Add user (payer) to whitelist by creating the payer-specific PDA
    let ix = Instruction::new_with_bytes(
        program_id,
        &program::instruction::AddToWhitelist {
            user: payer.pubkey(),
        }
        .data(),
        program::accounts::AddToWhitelist {
            admin: payer.pubkey(),
            whitelist_config: whitelist_config_pda,
            whitelist: payer_whitelist_pda,
            system_program: system_program_id,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &[ix], &payer, &[&payer]).expect("add_to_whitelist failed");
    assert!(
        svm.get_account(&payer_whitelist_pda).is_some(),
        "add_to_whitelist should create the payer whitelist PDA"
    );

    // Step 3: Remove user from whitelist by closing the payer-specific PDA
    let ix = Instruction::new_with_bytes(
        program_id,
        &program::instruction::RemoveFromWhitelist {
            user: payer.pubkey(),
        }
        .data(),
        program::accounts::RemoveFromWhitelist {
            admin: payer.pubkey(),
            whitelist_config: whitelist_config_pda,
            whitelist: payer_whitelist_pda,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &[ix], &payer, &[&payer]).expect("remove_from_whitelist failed");
    assert!(
        svm.get_account(&payer_whitelist_pda).is_none(),
        "remove_from_whitelist should close the payer whitelist PDA"
    );

    // Step 4: Create mint with TransferHook extension
    let mint = Keypair::new();
    let mint_size =
        ExtensionType::try_calculate_account_len::<Mint>(&[ExtensionType::TransferHook]).unwrap();
    let mint_rent = svm.minimum_balance_for_rent_exemption(mint_size);

    let create_mint_acct = system_instruction::create_account(
        &payer.pubkey(),
        &mint.pubkey(),
        mint_rent,
        mint_size as u64,
        &TOKEN_2022_ID,
    );
    let init_hook = init_transfer_hook(
        &TOKEN_2022_ID,
        &mint.pubkey(),
        Some(payer.pubkey()),
        Some(program_id),
    )
    .unwrap();
    let init_mint =
        initialize_mint2(&TOKEN_2022_ID, &mint.pubkey(), &payer.pubkey(), None, 9).unwrap();

    send(
        &mut svm,
        &[create_mint_acct, init_hook, init_mint],
        &payer,
        &[&payer, &mint],
    )
    .expect("create mint with transfer hook failed");

    // Step 5: Create source/destination ATAs and mint 100 tokens to source
    let source_ata = get_associated_token_address_with_program_id(
        &payer.pubkey(),
        &mint.pubkey(),
        &TOKEN_2022_ID,
    );
    let dest_ata = get_associated_token_address_with_program_id(
        &recipient.pubkey(),
        &mint.pubkey(),
        &TOKEN_2022_ID,
    );

    let create_source_ata = create_associated_token_account(
        &payer.pubkey(),
        &payer.pubkey(),
        &mint.pubkey(),
        &TOKEN_2022_ID,
    );
    let create_dest_ata = create_associated_token_account(
        &payer.pubkey(),
        &recipient.pubkey(),
        &mint.pubkey(),
        &TOKEN_2022_ID,
    );
    let mint_amount = 100u64 * 10u64.pow(9);
    let mint_to_ix = mint_to(
        &TOKEN_2022_ID,
        &mint.pubkey(),
        &source_ata,
        &payer.pubkey(),
        &[],
        mint_amount,
    )
    .unwrap();

    send(
        &mut svm,
        &[create_source_ata, create_dest_ata, mint_to_ix],
        &payer,
        &[&payer],
    )
    .expect("create ATAs and mint_to failed");

    // Step 6: Initialize ExtraAccountMetaList for the transfer hook
    let (extra_meta_pda, _) = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        &program_id,
    );

    let ix = Instruction::new_with_bytes(
        program_id,
        &program::instruction::InitializeTransferHook {}.data(),
        program::accounts::InitializeExtraAccountMetaList {
            payer: payer.pubkey(),
            extra_account_meta_list: extra_meta_pda,
            mint: mint.pubkey(),
            system_program: system_program_id,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &[ix], &payer, &[&payer]).expect("initialize_transfer_hook failed");

    let transfer_amount = 1u64 * 10u64.pow(9);
    let build_transfer_ix = |source: &Keypair, mint_kp: &Keypair, src: Pubkey, dst: Pubkey| {
        let mut ix = transfer_checked(
            &TOKEN_2022_ID,
            &src,
            &mint_kp.pubkey(),
            &dst,
            &source.pubkey(),
            &[],
            transfer_amount,
            9,
        )
        .unwrap();
        // Order: extra_account_meta_list, then TLV-registered extras (payer whitelist PDA), then hook program ID.
        ix.accounts
            .push(AccountMeta::new_readonly(extra_meta_pda, false));
        ix.accounts
            .push(AccountMeta::new_readonly(payer_whitelist_pda, false));
        ix.accounts
            .push(AccountMeta::new_readonly(program_id, false));
        ix
    };

    // Step 7a: Transfer should fail — payer was removed from the whitelist
    let transfer_fail_ix = build_transfer_ix(&payer, &mint, source_ata, dest_ata);
    let res = send(&mut svm, &[transfer_fail_ix], &payer, &[&payer]);
    assert!(
        res.is_err(),
        "transfer should fail — payer is not whitelisted"
    );

    // Step 7b: Re-add payer to whitelist, then the transfer should succeed
    let ix = Instruction::new_with_bytes(
        program_id,
        &program::instruction::AddToWhitelist {
            user: payer.pubkey(),
        }
        .data(),
        program::accounts::AddToWhitelist {
            admin: payer.pubkey(),
            whitelist_config: whitelist_config_pda,
            whitelist: payer_whitelist_pda,
            system_program: system_program_id,
        }
        .to_account_metas(None),
    );
    send(&mut svm, &[ix], &payer, &[&payer]).expect("re-add_to_whitelist failed");

    let transfer_ok_ix = build_transfer_ix(&payer, &mint, source_ata, dest_ata);
    send(&mut svm, &[transfer_ok_ix], &payer, &[&payer])
        .expect("transfer should succeed — payer re-added to whitelist");
}
