# Transfer Hook Vault

This project implements a single Token-2022 vault guarded by a transfer hook. The vault uses one `Vault` PDA for configuration, one program-derived mint, and one vault token account. Only users present in the vault whitelist can move tokens into or out of the vault, and each whitelist entry carries a maximum allowed interaction amount.

## Design

The main state account is `Vault`:

```rust
#[account]
pub struct Vault {
    pub admin: Pubkey,
    pub mint: Pubkey,
    pub vault_token_account: Pubkey,
    pub whitelist: Vec<WhitelistEntry>,
    pub bump: u8,
    pub mint_bump: u8,
    pub extra_account_meta_bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct WhitelistEntry {
    pub user: Pubkey,
    pub amount: u64,
}
```

The whitelist is intentionally stored as a bounded `Vec<WhitelistEntry>` for the exercise. A production-sized whitelist could move each user into a PDA such as `[b"whitelist", vault, user]` to avoid reallocating or bounding a single vector account.

## Token Extensions

`initialize_vault` creates the Token-2022 mint inside the program with:

- `TransferHook`, pointing at this program.
- `PermanentDelegate`, set to the vault admin. This gives the admin an explicit Token-2022 withdrawal authority while the transfer hook still checks the destination user against the vault whitelist.

The mint authority is the `Vault` PDA, so token issuance goes through the program via `mint_tokens`.

## Vault Flow

Deposits and withdrawals are ordinary Token-2022 `transfer_checked` instructions that include the transfer hook extra accounts:

1. Deposit: user token account to the vault token account, signed by the user.
2. Withdraw: vault token account to the user token account, signed by the admin as the permanent delegate.

The transfer hook receives the source, mint, destination, authority, `ExtraAccountMetaList`, and `Vault` PDA. It only allows transfers where either the source or destination is the vault token account, then checks the interacting user against `Vault.whitelist` and the transfer amount.

This direct-transfer shape avoids same-program reentrancy: a vault instruction in this same program cannot CPI into Token-2022 and then be called again as the hook during the same instruction.

## Tests

The LiteSVM test covers:

- Program-side vault, mint, vault ATA, and extra account meta initialization.
- Program-side minting.
- Verification that the mint has both `TransferHook` and `PermanentDelegate` extensions.
- Failed deposit for a non-whitelisted user.
- Failed deposit above a whitelisted user's amount.
- Successful whitelisted deposit.
- Failed withdrawal after whitelist removal.
- Successful withdrawal after re-adding the user.

Run:

```sh
anchor build
cargo test
```