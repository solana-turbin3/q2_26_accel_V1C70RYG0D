use anchor_lang::prelude::*;
use anchor_spl::{associated_token::AssociatedToken, token_interface::{Mint, TokenAccount, TokenInterface, mint_to_checked, MintToChecked}};
use mpl_core::{
    ID as MPL_CORE_ID,
    accounts::{BaseAssetV1, BaseCollectionV1},
    types::{UpdateAuthority, Attribute, Attributes, Plugin, PluginType, FreezeDelegate},
    instructions::{UpdatePluginV1CpiBuilder},
    fetch_plugin,
};
use crate::constants::{KEY_LAST_CLAIM, KEY_STAKED, KEY_STAKED_AT, SECONDS_PER_DAY};
use crate::Config;
use crate::error::ErrorCode;
use crate::utils::update_collection_staked_count;

#[derive(Accounts)]
pub struct Unstake<'info> {
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
    #[account(
        mut,
        has_one = update_authority @ ErrorCode::InvalidUpdateAuthority
    )]
    pub collection: Account<'info, BaseCollectionV1>,
    /// CHECK: This account data is not used, we only verify the address
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
    /// CHECK: This is the MPL Core program
    #[account(address = Pubkey::from(MPL_CORE_ID.to_bytes()))]
    pub mpl_core_program: UncheckedAccount<'info>,
}
pub fn handler(ctx: Context<Unstake>) -> Result<()> {

    // We start by fetching the existing attributes
    let attributes_fetched: Option<Attributes> = fetch_plugin::<BaseAssetV1, Attributes>(
        &ctx.accounts.asset.to_account_info(),
        PluginType::Attributes,
    )
    .ok()
    .map(|(_, attrs, _)| attrs);

    // If the attributes don't exist, we return an error
    require!(attributes_fetched.is_some(), ErrorCode::AssetNotStaked);

    let attributes = attributes_fetched.unwrap();

    // Prepare the Attributes list to update based on the existing attributes
    let mut attributes_list: Vec<Attribute> = Vec::with_capacity(attributes.attribute_list.len());

    // Auxiliary variables. We separate the freeze clock (`staked_at`) from the
    // reward clock (`last_claim`): the freeze period is measured against
    // `staked_at`, while pending rewards are measured against `last_claim`.
    let current_timestamp = Clock::get()?.unix_timestamp;
    let mut is_staked = false;
    let mut staked_at: i64 = 0;
    let mut last_claim: Option<i64> = None;

    for attribute in &attributes.attribute_list {
        match attribute.key.as_str() {
            KEY_STAKED => {
                is_staked = attribute.value == "true";
            }
            KEY_STAKED_AT => {
                staked_at = attribute
                    .value
                    .parse::<i64>()
                    .map_err(|_| ErrorCode::InvalidTimestamp)?;
            }
            KEY_LAST_CLAIM => {
                last_claim = Some(
                    attribute
                        .value
                        .parse::<i64>()
                        .map_err(|_| ErrorCode::InvalidTimestamp)?,
                );
            }
            // Carry over any non-staking attributes unchanged.
            _ => attributes_list.push(attribute.clone()),
        }
    }

    // The asset must be currently staked.
    require!(is_staked, ErrorCode::AssetNotStaked);

    // Enforce the freeze period against `staked_at` (NOT against `last_claim`),
    // so claiming rewards right before unstaking does not affect eligibility.
    let staked_days = current_timestamp
        .checked_sub(staked_at)
        .ok_or(ErrorCode::InvalidTimestamp)?
        .checked_div(SECONDS_PER_DAY)
        .ok_or(ErrorCode::InvalidTimestamp)?;
    require!(
        staked_days >= ctx.accounts.config.freeze_period as i64,
        ErrorCode::FreezePeriodNotElapsed
    );

    // Pending rewards accrue from the last claim (defaults to `staked_at` for
    // assets staked before `last_claim` existed / when never claimed).
    let last_claim = last_claim.unwrap_or(staked_at);
    let reward_days = current_timestamp
        .checked_sub(last_claim)
        .ok_or(ErrorCode::InvalidTimestamp)?
        .checked_div(SECONDS_PER_DAY)
        .ok_or(ErrorCode::InvalidTimestamp)?;

    // Prepare signing seeds for the update authority
    let collection_key = ctx.accounts.collection.key();
    let signer_seeds = &[
        b"update_authority",
        collection_key.as_ref(),
        &[ctx.bumps.update_authority],
    ];

    // Now we update the asset Attributes Plugin (with the existing attributes, including the staking attributes with reset values)

    // Add the staking attributes back with reset values.
    attributes_list.push(Attribute {
        key: KEY_STAKED.to_string(),
        value: "false".to_string(),
    });
    attributes_list.push(Attribute {
        key: KEY_STAKED_AT.to_string(),
        value: "0".to_string(),
    });
    attributes_list.push(Attribute {
        key: KEY_LAST_CLAIM.to_string(),
        value: "0".to_string(),
    });

    UpdatePluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
    .asset(&ctx.accounts.asset.to_account_info())
    .collection(Some(&ctx.accounts.collection.to_account_info()))
    .payer(&ctx.accounts.owner.to_account_info())
    .authority(Some(&ctx.accounts.update_authority.to_account_info()))
    .system_program(&ctx.accounts.system_program.to_account_info())
    .plugin(Plugin::Attributes(Attributes { attribute_list: attributes_list }))
    .invoke_signed(&[signer_seeds])?;

    // And we Thaw the asset (update the FreezeDelegate Plugin to false)
    UpdatePluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
    .asset(&ctx.accounts.asset.to_account_info())
    .collection(Some(&ctx.accounts.collection.to_account_info()))
    .payer(&ctx.accounts.owner.to_account_info())
    .authority(Some(&ctx.accounts.update_authority.to_account_info()))
    .system_program(&ctx.accounts.system_program.to_account_info())
    .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: false }))
    .invoke_signed(&[signer_seeds])?;

    // Challenge 2: decrement the collection-level staked counter (-1).
    update_collection_staked_count(
        &ctx.accounts.mpl_core_program.to_account_info(),
        &ctx.accounts.collection.to_account_info(),
        &ctx.accounts.owner.to_account_info(),
        &ctx.accounts.update_authority.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &[signer_seeds],
        -1,
    )?;

    // Finally, we want to mint any rewards accrued since the last claim to the user

    // Calculate the amount
    let amount = (reward_days as u64)
        .checked_mul(ctx.accounts.config.rewards_bps as u64)
        .ok_or(ErrorCode::InvalidRewardsBps)?
        .checked_mul(10u64.pow(ctx.accounts.rewards_mint.decimals as u32))
        .ok_or(ErrorCode::InvalidRewardsBps)?
        .checked_div(10000u64)
        .ok_or(ErrorCode::InvalidRewardsBps)?;

    // Prepare signer seeds for config PDA
    let config_seeds = &[
        b"config",
        collection_key.as_ref(),
        &[ctx.accounts.config.bump],
    ];
    let config_signer_seeds = &[&config_seeds[..]];

    if amount > 0 {
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
