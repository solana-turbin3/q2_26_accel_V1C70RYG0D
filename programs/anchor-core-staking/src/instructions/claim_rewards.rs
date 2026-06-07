use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{mint_to_checked, Mint, MintToChecked, TokenAccount, TokenInterface},
};
use mpl_core::{
    accounts::{BaseAssetV1, BaseCollectionV1},
    fetch_plugin,
    instructions::UpdatePluginV1CpiBuilder,
    types::{Attribute, Attributes, Plugin, PluginType, UpdateAuthority},
    ID as MPL_CORE_ID,
};
use crate::constants::{KEY_LAST_CLAIM, KEY_STAKED, KEY_STAKED_AT, SECONDS_PER_DAY};
use crate::error::ErrorCode;
use crate::Config;

/// Challenge 1: claim accumulated staking rewards WITHOUT unstaking the asset.
///
/// The asset stays frozen/staked. Rewards are paid for the time elapsed since the
/// last claim and the `last_claim` clock is advanced — but only by the number of
/// *whole rewarded days*, so any sub-day remainder keeps accruing.
///
/// Crucially, `staked_at` (the freeze clock) is never touched here, so a user can
/// claim and then immediately unstake once the original freeze period has elapsed.
#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        seeds = [b"config", collection.key().as_ref()],
        bump = config.bump,
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        has_one = owner @ ErrorCode::InvalidOwner,
        constraint = asset.update_authority == UpdateAuthority::Collection(collection.key()) @ ErrorCode::InvalidUpdateAuthority,
    )]
    pub asset: Account<'info, BaseAssetV1>,
    // Writable because UpdatePluginV1 marks the collection account as writable.
    #[account(
        mut,
        has_one = update_authority @ ErrorCode::InvalidUpdateAuthority,
    )]
    pub collection: Account<'info, BaseCollectionV1>,
    /// CHECK: This account is not initialized and is being used for signing purposes only, we verify that derives from the correct seeds
    #[account(
        seeds = [b"update_authority", collection.key().as_ref()],
        bump,
    )]
    pub update_authority: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [b"rewards_mint", config.key().as_ref()],
        bump = config.rewards_bump,
    )]
    pub rewards_mint: InterfaceAccount<'info, Mint>,
    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = rewards_mint,
        associated_token::authority = owner,
    )]
    pub user_rewards_ata: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    /// CHECK: This is the ID of the MPL Core Program
    #[account(address = MPL_CORE_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<ClaimRewards>) -> Result<()> {
    // Fetch the existing asset attributes.
    let attributes_fetched: Option<Attributes> = fetch_plugin::<BaseAssetV1, Attributes>(
        &ctx.accounts.asset.to_account_info(),
        PluginType::Attributes,
    )
    .ok()
    .map(|(_, attrs, _)| attrs);

    require!(attributes_fetched.is_some(), ErrorCode::AssetNotStaked);
    let attributes = attributes_fetched.unwrap();

    let current_timestamp = Clock::get()?.unix_timestamp;

    // Rebuild the attribute list, keeping `staked` and `staked_at` untouched and
    // capturing `last_claim` so we can advance it. Everything else is carried over.
    let mut attributes_list: Vec<Attribute> = Vec::with_capacity(attributes.attribute_list.len());
    let mut is_staked = false;
    let mut staked_at: i64 = 0;
    let mut last_claim: Option<i64> = None;

    for attribute in &attributes.attribute_list {
        match attribute.key.as_str() {
            KEY_STAKED => {
                is_staked = attribute.value == "true";
                // Keep the staked flag unchanged (we are not unstaking).
                attributes_list.push(attribute.clone());
            }
            KEY_STAKED_AT => {
                staked_at = attribute
                    .value
                    .parse::<i64>()
                    .map_err(|_| ErrorCode::InvalidTimestamp)?;
                // Keep the freeze clock unchanged.
                attributes_list.push(attribute.clone());
            }
            KEY_LAST_CLAIM => {
                last_claim = Some(
                    attribute
                        .value
                        .parse::<i64>()
                        .map_err(|_| ErrorCode::InvalidTimestamp)?,
                );
                // Re-added below with the advanced value.
            }
            _ => attributes_list.push(attribute.clone()),
        }
    }

    // Asset must be currently staked to claim.
    require!(is_staked, ErrorCode::AssetNotStaked);

    // Default `last_claim` to `staked_at` (covers assets staked before this clock existed).
    let last_claim = last_claim.unwrap_or(staked_at);

    // Whole reward-bearing days since the last claim.
    let reward_days = current_timestamp
        .checked_sub(last_claim)
        .ok_or(ErrorCode::InvalidTimestamp)?
        .checked_div(SECONDS_PER_DAY)
        .ok_or(ErrorCode::InvalidTimestamp)?;

    // Nothing to claim yet (less than a full day since the last claim): no-op, so we avoid a
    // wasted attribute-update CPI and let the sub-day remainder keep accruing.
    if reward_days == 0 {
        return Ok(());
    }

    // Advance `last_claim` by exactly the rewarded days so the sub-day remainder
    // is preserved and not lost on frequent claims.
    let new_last_claim = last_claim
        .checked_add(
            reward_days
                .checked_mul(SECONDS_PER_DAY)
                .ok_or(ErrorCode::ArithmeticOverflow)?,
        )
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    attributes_list.push(Attribute {
        key: KEY_LAST_CLAIM.to_string(),
        value: new_last_claim.to_string(),
    });

    // Prepare signing seeds for the update authority (PDA).
    let collection_key = ctx.accounts.collection.key();
    let signer_seeds = &[
        b"update_authority",
        collection_key.as_ref(),
        &[ctx.bumps.update_authority],
    ];

    // Update the asset Attributes plugin with the advanced `last_claim`.
    UpdatePluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .payer(&ctx.accounts.owner.to_account_info())
        .authority(Some(&ctx.accounts.update_authority.to_account_info()))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .plugin(Plugin::Attributes(Attributes {
            attribute_list: attributes_list,
        }))
        .invoke_signed(&[signer_seeds])?;

    // Mint the accrued rewards to the user (signed by the config PDA, the mint authority).
    let amount = (reward_days as u64)
        .checked_mul(ctx.accounts.config.rewards_bps as u64)
        .ok_or(ErrorCode::InvalidRewardsBps)?
        .checked_mul(10u64.pow(ctx.accounts.rewards_mint.decimals as u32))
        .ok_or(ErrorCode::InvalidRewardsBps)?
        .checked_div(10000u64)
        .ok_or(ErrorCode::InvalidRewardsBps)?;

    if amount > 0 {
        let config_seeds = &[
            b"config",
            collection_key.as_ref(),
            &[ctx.accounts.config.bump],
        ];
        let config_signer_seeds = &[&config_seeds[..]];

        mint_to_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintToChecked {
                    mint: ctx.accounts.rewards_mint.to_account_info(),
                    to: ctx.accounts.user_rewards_ata.to_account_info(),
                    authority: ctx.accounts.config.to_account_info(),
                },
                config_signer_seeds,
            ),
            amount,
            ctx.accounts.rewards_mint.decimals,
        )?;
    }

    Ok(())
}
