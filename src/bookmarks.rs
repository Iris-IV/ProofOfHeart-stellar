//! On-chain campaign bookmark / save list for wallets (#507).
//!
//! Lets a wallet track causes it cares about without relying on the
//! frontend: `save_campaign`, `remove_saved_campaign`, and
//! `get_saved_campaigns` are plain ledger reads/writes keyed by the wallet
//! address, so any client can display a user's saved campaigns directly from
//! chain state.

use soroban_sdk::{Address, Env, Vec};

use crate::errors::Error;
use crate::lifecycle::get_campaign_or_error;
use crate::storage::{get_saved_campaigns, set_saved_campaigns};

/// Adds `campaign_id` to `user`'s saved-campaigns list.
///
/// Requires the wallet's authorization. Fails if the campaign doesn't exist
/// or is already bookmarked.
pub fn save_campaign(env: &Env, user: Address, campaign_id: u32) -> Result<(), Error> {
    user.require_auth();

    // Ensure the campaign actually exists before letting it be bookmarked.
    get_campaign_or_error(env, campaign_id)?;

    let mut saved = get_saved_campaigns(env, &user);
    if saved.iter().any(|id| id == campaign_id) {
        return Err(Error::CampaignAlreadyBookmarked);
    }

    saved.push_back(campaign_id);
    set_saved_campaigns(env, &user, &saved);

    env.events()
        .publish(("campaign_bookmarked", user), campaign_id);

    Ok(())
}

/// Removes `campaign_id` from `user`'s saved-campaigns list.
///
/// Requires the wallet's authorization. Fails if the campaign isn't
/// currently bookmarked.
pub fn remove_saved_campaign(env: &Env, user: Address, campaign_id: u32) -> Result<(), Error> {
    user.require_auth();

    let saved = get_saved_campaigns(env, &user);
    let position = saved.iter().position(|id| id == campaign_id);

    match position {
        Some(idx) => {
            let mut updated = saved;
            updated.remove(idx as u32);
            set_saved_campaigns(env, &user, &updated);

            env.events()
                .publish(("campaign_unbookmarked", user), campaign_id);

            Ok(())
        }
        None => Err(Error::CampaignNotBookmarked),
    }
}

/// Returns the list of campaign ids `user` has bookmarked, in the order they
/// were saved. This is a public, unauthenticated read — any wallet/app can
/// display another wallet's saved causes.
pub fn get_saved(env: &Env, user: Address) -> Vec<u32> {
    get_saved_campaigns(env, &user)
}
