use anchor_lang::prelude::*;
use mpl_core::{
    ID as MPL_CORE_ID,
    accounts::{BaseAssetV1, BaseCollectionV1},
    instructions::{AddPluginV1CpiBuilder, UpdatePluginV1CpiBuilder},
    types::{UpdateAuthority, Attribute, Attributes, Plugin, PluginAuthority, PluginType, FreezeDelegate},
    fetch_plugin,
};
use crate::constants::{KEY_LAST_CLAIM, KEY_STAKED, KEY_STAKED_AT};
use crate::state::Config;
use crate::error::ErrorCode;
use crate::utils::update_collection_staked_count;

#[derive(Accounts)]
pub struct Stake<'info> {
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
        has_one = update_authority @ ErrorCode::InvalidUpdateAuthority,
    )]
    pub collection: Account<'info, BaseCollectionV1>,
    /// CHECK: This account is not initialized and is being used for signing purposes only, we verify that derives from the correct seeds
    #[account(
        seeds = [b"update_authority", collection.key().as_ref()],
        bump,
    )]
    pub update_authority: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: This is the ID of the MPL Core Program
    #[account(address = MPL_CORE_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,
}
pub fn handler(ctx: Context<Stake>) -> Result<()> {

    // We start by fetching the existing attributes (if they exist)
    let attributes_fetched: Option<Attributes> = fetch_plugin::<BaseAssetV1, Attributes>(
        &ctx.accounts.asset.to_account_info(),
        PluginType::Attributes,
    )
    .ok()
    .map(|(_,attrs,_)| attrs);


    // Prepare the Attributes list to add or update based on the existing attributes
    let mut attributes_list: Vec<Attribute> = Vec::new();

    // Loop over all attributes and keep only the ones that are not staking attributes
    // ("staked", "staked_at", "last_claim") — these are (re)written fresh below.
    // If we find the "staked" attribute already present, we make sure the asset is not already staked.
    if let Some(attributes) = &attributes_fetched {
        for attribute in &attributes.attribute_list {
            match attribute.key.as_str() {
                KEY_STAKED => require!(attribute.value == "false", ErrorCode::AlreadyStaked),
                KEY_STAKED_AT | KEY_LAST_CLAIM => {}
                _ => attributes_list.push(attribute.clone()),
            }
        }
    }

    // Add the staking attributes. `staked_at` drives the freeze period; `last_claim`
    // drives reward accrual and starts equal to `staked_at`.
    let now = Clock::get()?.unix_timestamp;
    attributes_list.push(Attribute {
        key: KEY_STAKED.to_string(),
        value: "true".to_string(),
    });
    attributes_list.push(Attribute {
        key: KEY_STAKED_AT.to_string(),
        value: now.to_string(),
    });
    attributes_list.push(Attribute {
        key: KEY_LAST_CLAIM.to_string(),
        value: now.to_string(),
    });

    // Now that we have the complete list of Attributes we either add the Plugin or Update the existing one
    // The Attributes Plugin is an Authority-Managed Plugin, so it needs to be signed by the update authority (PDA of the program)

    // Prepare signing seeds for the update authority
    let collection_key = ctx.accounts.collection.key();
    let signer_seeds = &[
        b"update_authority",
        collection_key.as_ref(),
        &[ctx.bumps.update_authority],
    ];

    // If the Attributes Plugin does not exist, we add it
    if attributes_fetched.is_none() {
        AddPluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .payer(&ctx.accounts.owner.to_account_info())
        .authority(Some(&ctx.accounts.update_authority.to_account_info()))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .plugin(Plugin::Attributes(Attributes { attribute_list: attributes_list }))
        .init_authority(PluginAuthority::UpdateAuthority)
        .invoke_signed(&[signer_seeds])?;
    }
    // If the Attributes Plugin exists, we update it
    else {
        UpdatePluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .payer(&ctx.accounts.owner.to_account_info())
        .authority(Some(&ctx.accounts.update_authority.to_account_info()))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .plugin(Plugin::Attributes(Attributes { attribute_list: attributes_list }))
        .invoke_signed(&[signer_seeds])?;
    }

    // Freeze the asset with the FreezeDelegate Plugin.
    // On the FIRST stake the plugin does not exist yet, so we ADD it: the owner (a real signer)
    // authorizes adding this owner-managed plugin, and we delegate its authority to the
    // update-authority PDA so the program can thaw it later in `unstake`.
    // On a RE-STAKE the plugin still exists (left thawed by the previous unstake), so we UPDATE
    // it back to frozen instead — signed by the update-authority PDA that now owns the plugin.
    let freeze_delegate_exists = fetch_plugin::<BaseAssetV1, FreezeDelegate>(
        &ctx.accounts.asset.to_account_info(),
        PluginType::FreezeDelegate,
    )
    .is_ok();

    if !freeze_delegate_exists {
        AddPluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .payer(&ctx.accounts.owner.to_account_info())
        .authority(Some(&ctx.accounts.owner.to_account_info()))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: true }))
        .init_authority(PluginAuthority::UpdateAuthority)
        .invoke()?;
    } else {
        UpdatePluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .payer(&ctx.accounts.owner.to_account_info())
        .authority(Some(&ctx.accounts.update_authority.to_account_info()))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: true }))
        .invoke_signed(&[signer_seeds])?;
    }

    // Challenge 2: bump the collection-level staked counter (+1).
    update_collection_staked_count(
        &ctx.accounts.mpl_core_program.to_account_info(),
        &ctx.accounts.collection.to_account_info(),
        &ctx.accounts.owner.to_account_info(),
        &ctx.accounts.update_authority.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &[signer_seeds],
        1,
    )?;

    Ok(())
}
