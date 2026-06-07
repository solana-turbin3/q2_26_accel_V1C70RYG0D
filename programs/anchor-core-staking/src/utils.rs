use anchor_lang::prelude::*;
use mpl_core::{
    fetch_collection_plugin,
    instructions::{AddCollectionPluginV1CpiBuilder, UpdateCollectionPluginV1CpiBuilder},
    types::{Attribute, Attributes, Plugin, PluginAuthority, PluginType},
};

use crate::constants::{KEY_TOTAL_STAKED, KEY_TOTAL_STAKED_LIFETIME};
use crate::error::ErrorCode;

/// Challenge 2: track staking statistics at the collection level using the
/// Attributes plugin on the collection itself.
///
/// Maintains two counters as collection attributes:
/// - `total_staked`          – assets *currently* staked (incremented on stake, decremented on unstake).
/// - `total_staked_lifetime` – cumulative number of stake events (only ever incremented).
///
/// The Attributes plugin is created lazily on the first stake. It is an
/// authority-managed plugin owned by the collection's update authority (our PDA),
/// so every add/update is signed with the update-authority seeds.
///
/// `delta` is `+1` when an asset is staked and `-1` when it is unstaked. The
/// current counter is clamped at zero so it can never underflow.
pub fn update_collection_staked_count<'info>(
    mpl_core_program: &AccountInfo<'info>,
    collection: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    update_authority: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    signer_seeds: &[&[&[u8]]],
    delta: i64,
) -> Result<()> {
    // Fetch the existing collection Attributes plugin, if any.
    let existing: Option<Attributes> =
        fetch_collection_plugin::<Attributes>(collection, PluginType::Attributes)
            .ok()
            .map(|(_, attrs, _)| attrs);

    // Carry over every attribute that is not one of our counters; parse the
    // counters so we can update them.
    let mut total_staked: i64 = 0;
    let mut total_staked_lifetime: i64 = 0;
    let mut attributes_list: Vec<Attribute> = Vec::new();

    if let Some(attributes) = &existing {
        for attribute in &attributes.attribute_list {
            match attribute.key.as_str() {
                KEY_TOTAL_STAKED => {
                    total_staked = attribute.value.parse::<i64>().unwrap_or(0);
                }
                KEY_TOTAL_STAKED_LIFETIME => {
                    total_staked_lifetime = attribute.value.parse::<i64>().unwrap_or(0);
                }
                _ => attributes_list.push(attribute.clone()),
            }
        }
    }

    // Apply the delta. Current count is clamped at zero; lifetime only grows.
    total_staked = total_staked
        .checked_add(delta)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .max(0);
    if delta > 0 {
        total_staked_lifetime = total_staked_lifetime
            .checked_add(delta)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
    }

    attributes_list.push(Attribute {
        key: KEY_TOTAL_STAKED.to_string(),
        value: total_staked.to_string(),
    });
    attributes_list.push(Attribute {
        key: KEY_TOTAL_STAKED_LIFETIME.to_string(),
        value: total_staked_lifetime.to_string(),
    });

    let plugin = Plugin::Attributes(Attributes {
        attribute_list: attributes_list,
    });

    if existing.is_none() {
        // First stake in this collection: create the Attributes plugin.
        AddCollectionPluginV1CpiBuilder::new(mpl_core_program)
            .collection(collection)
            .payer(payer)
            .authority(Some(update_authority))
            .system_program(system_program)
            .plugin(plugin)
            .init_authority(PluginAuthority::UpdateAuthority)
            .invoke_signed(signer_seeds)?;
    } else {
        // Plugin already exists: update the counters.
        UpdateCollectionPluginV1CpiBuilder::new(mpl_core_program)
            .collection(collection)
            .payer(payer)
            .authority(Some(update_authority))
            .system_program(system_program)
            .plugin(plugin)
            .invoke_signed(signer_seeds)?;
    }

    Ok(())
}
