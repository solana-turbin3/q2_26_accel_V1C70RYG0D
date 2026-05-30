use std::path::PathBuf;

use litesvm::LiteSVM;
use litesvm_token::{
    CreateAssociatedTokenAccount, CreateMint, MintTo,
    spl_token::{self},
};
use solana_clock::Clock;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::Message;
use solana_native_token::LAMPORTS_PER_SOL;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;

use crate::{constants::SECONDS_TO_DAYS, state::Contributor};

const TOKEN_PROGRAM_ID: Pubkey = spl_token::ID;

fn program_id() -> Pubkey {
    Pubkey::from(crate::ID)
}

fn so_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for subdir in &["sbpf-solana-solana", "sbf-solana-solana"] {
        let p = manifest_dir
            .join("target")
            .join(subdir)
            .join("release/pinocchio_fundraiser.so");
        if p.exists() {
            return p;
        }
    }
    manifest_dir.join("target/deploy/pinocchio_fundraiser.so")
}

fn setup() -> (LiteSVM, Keypair) {
    let mut svm = LiteSVM::new();
    let maker = Keypair::new();
    svm.airdrop(&maker.pubkey(), 10 * LAMPORTS_PER_SOL)
        .expect("maker airdrop failed");

    let program_data = std::fs::read(so_path())
        .expect("failed to read pinocchio_fundraiser.so; run `cargo build-sbf` first");
    svm.add_program(program_id(), &program_data)
        .expect("failed to add program");

    (svm, maker)
}

fn system_program() -> Pubkey {
    solana_sdk_ids::system_program::ID
}

fn ata_program() -> Pubkey {
    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
        .parse()
        .unwrap()
}

fn fundraiser_pda(maker: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"fundraiser", maker.as_ref()], &program_id())
}

fn contributor_pda(fundraiser: &Pubkey, contributor: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"contributor", fundraiser.as_ref(), contributor.as_ref()],
        &program_id(),
    )
}

fn read_token_balance(svm: &LiteSVM, ata: &Pubkey) -> u64 {
    let account = svm.get_account(ata).expect("token account not found");
    let bytes: [u8; 8] = account.data[64..72].try_into().unwrap();
    u64::from_le_bytes(bytes)
}

fn read_contributor_amount(svm: &LiteSVM, contributor_account: &Pubkey) -> u64 {
    let account = svm
        .get_account(contributor_account)
        .expect("contributor account not found");
    assert_eq!(account.data.len(), Contributor::LEN);
    let bytes: [u8; 8] = account.data[0..8].try_into().unwrap();
    u64::from_le_bytes(bytes)
}

fn assert_closed(svm: &LiteSVM, address: &Pubkey) {
    if let Some(account) = svm.get_account(address) {
        assert_eq!(account.lamports, 0);
        assert_eq!(account.data.len(), 0);
    }
}

struct InitializedFundraiser {
    svm: LiteSVM,
    maker: Keypair,
    mint: Pubkey,
    fundraiser: Pubkey,
    vault: Pubkey,
    target: u64,
}

fn initialize(target: u64, duration: u8) -> InitializedFundraiser {
    let (mut svm, maker) = setup();
    let mint = CreateMint::new(&mut svm, &maker)
        .decimals(6)
        .authority(&maker.pubkey())
        .send()
        .unwrap();

    let (fundraiser, bump) = fundraiser_pda(&maker.pubkey());
    let vault = spl_associated_token_account::get_associated_token_address(&fundraiser, &mint);

    let data = [vec![0, bump], target.to_le_bytes().to_vec(), vec![duration]].concat();
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(maker.pubkey(), true),
            AccountMeta::new(mint, false),
            AccountMeta::new(fundraiser, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(system_program(), false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(ata_program(), false),
        ],
        data,
    };
    let msg = Message::new(&[ix], Some(&maker.pubkey()));
    let blockhash = svm.latest_blockhash();
    let tx = Transaction::new(&[&maker], msg, blockhash);
    let meta = svm.send_transaction(tx).expect("initialize failed");
    println!("Initialize CU: {}", meta.compute_units_consumed);

    InitializedFundraiser {
        svm,
        maker,
        mint,
        fundraiser,
        vault,
        target,
    }
}

fn funded_contributor(
    svm: &mut LiteSVM,
    maker: &Keypair,
    mint: &Pubkey,
    amount: u64,
) -> (Keypair, Pubkey) {
    let contributor = Keypair::new();
    svm.airdrop(&contributor.pubkey(), LAMPORTS_PER_SOL)
        .expect("contributor airdrop failed");
    let contributor_ata = CreateAssociatedTokenAccount::new(svm, maker, mint)
        .owner(&contributor.pubkey())
        .send()
        .unwrap();
    MintTo::new(svm, maker, mint, &contributor_ata, amount)
        .send()
        .unwrap();
    (contributor, contributor_ata)
}

fn contribute(
    svm: &mut LiteSVM,
    contributor: &Keypair,
    mint: Pubkey,
    fundraiser: Pubkey,
    contributor_ata: Pubkey,
    vault: Pubkey,
    amount: u64,
) -> Pubkey {
    let (contributor_account, contributor_bump) =
        contributor_pda(&fundraiser, &contributor.pubkey());
    let data = [vec![1, contributor_bump], amount.to_le_bytes().to_vec()].concat();
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(contributor.pubkey(), true),
            AccountMeta::new(mint, false),
            AccountMeta::new(fundraiser, false),
            AccountMeta::new(contributor_account, false),
            AccountMeta::new(contributor_ata, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(system_program(), false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        ],
        data,
    };
    let msg = Message::new(&[ix], Some(&contributor.pubkey()));
    let blockhash = svm.latest_blockhash();
    let tx = Transaction::new(&[contributor], msg, blockhash);
    let meta = svm.send_transaction(tx).expect("contribute failed");
    println!("Contribute CU: {}", meta.compute_units_consumed);
    contributor_account
}

#[test]
fn initialize_and_contribute() {
    let mut s = initialize(10_000_000, 1);
    let (contributor, contributor_ata) =
        funded_contributor(&mut s.svm, &s.maker, &s.mint, 2_000_000);

    let contributor_account = contribute(
        &mut s.svm,
        &contributor,
        s.mint,
        s.fundraiser,
        contributor_ata,
        s.vault,
        1_000_000,
    );

    assert_eq!(read_token_balance(&s.svm, &s.vault), 1_000_000);
    assert_eq!(read_token_balance(&s.svm, &contributor_ata), 1_000_000);
    assert_eq!(
        read_contributor_amount(&s.svm, &contributor_account),
        1_000_000
    );
}

#[test]
fn check_contributions_claims_when_target_met() {
    let mut s = initialize(10_000_000, 1);
    MintTo::new(&mut s.svm, &s.maker, &s.mint, &s.vault, s.target)
        .send()
        .unwrap();

    let maker_ata =
        spl_associated_token_account::get_associated_token_address(&s.maker.pubkey(), &s.mint);
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(s.maker.pubkey(), true),
            AccountMeta::new(s.mint, false),
            AccountMeta::new(s.fundraiser, false),
            AccountMeta::new(s.vault, false),
            AccountMeta::new(maker_ata, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(system_program(), false),
            AccountMeta::new_readonly(ata_program(), false),
        ],
        data: vec![2],
    };
    let msg = Message::new(&[ix], Some(&s.maker.pubkey()));
    let blockhash = s.svm.latest_blockhash();
    let tx = Transaction::new(&[&s.maker], msg, blockhash);
    let meta = s
        .svm
        .send_transaction(tx)
        .expect("check contributions failed");
    println!("Check contributions CU: {}", meta.compute_units_consumed);

    assert_eq!(read_token_balance(&s.svm, &maker_ata), s.target);
    assert_closed(&s.svm, &s.vault);
    assert_closed(&s.svm, &s.fundraiser);
}

#[test]
fn refund_after_duration_when_target_not_met() {
    let mut s = initialize(10_000_000, 1);
    let (contributor, contributor_ata) =
        funded_contributor(&mut s.svm, &s.maker, &s.mint, 2_000_000);
    let contributor_account = contribute(
        &mut s.svm,
        &contributor,
        s.mint,
        s.fundraiser,
        contributor_ata,
        s.vault,
        1_000_000,
    );

    let mut clock = s.svm.get_sysvar::<Clock>();
    clock.unix_timestamp += SECONDS_TO_DAYS * 2;
    s.svm.set_sysvar(&clock);

    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(contributor.pubkey(), true),
            AccountMeta::new(s.maker.pubkey(), false),
            AccountMeta::new(s.mint, false),
            AccountMeta::new(s.fundraiser, false),
            AccountMeta::new(contributor_account, false),
            AccountMeta::new(contributor_ata, false),
            AccountMeta::new(s.vault, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(system_program(), false),
        ],
        data: vec![3],
    };
    let msg = Message::new(&[ix], Some(&contributor.pubkey()));
    let blockhash = s.svm.latest_blockhash();
    let tx = Transaction::new(&[&contributor], msg, blockhash);
    let meta = s.svm.send_transaction(tx).expect("refund failed");
    println!("Refund CU: {}", meta.compute_units_consumed);

    assert_eq!(read_token_balance(&s.svm, &contributor_ata), 2_000_000);
    assert_eq!(read_token_balance(&s.svm, &s.vault), 0);
    assert_closed(&s.svm, &contributor_account);
}
