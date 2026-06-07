use anchor_lang::prelude::*;

#[constant]
pub const SEED: &str = "anchor";

/// Number of seconds in a day, used to convert staked time into reward-bearing days.
pub const SECONDS_PER_DAY: i64 = 86400;

// ----- Asset-level Attributes plugin keys -----
/// Whether the asset is currently staked ("true" / "false").
pub const KEY_STAKED: &str = "staked";
/// Unix timestamp when the asset was staked. Drives the freeze-period check and is
/// NEVER reset by `claim_rewards`, so claiming has no impact on when the user can unstake.
pub const KEY_STAKED_AT: &str = "staked_at";
/// Unix timestamp up to which rewards have already been paid out. Advanced by
/// `claim_rewards` and `unstake` so rewards are never double-counted.
pub const KEY_LAST_CLAIM: &str = "last_claim";

// ----- Collection-level Attributes plugin keys (Challenge 2) -----
/// Number of assets currently staked in the collection.
pub const KEY_TOTAL_STAKED: &str = "total_staked";
/// Cumulative number of stake events ever recorded for the collection.
pub const KEY_TOTAL_STAKED_LIFETIME: &str = "total_staked_lifetime";
