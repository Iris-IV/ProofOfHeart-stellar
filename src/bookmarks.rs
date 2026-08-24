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
            // Vec::remove shifts all subsequent elements to the left.
            // Removing the first element causes the largest shift, while removing
            // the last element requires no shifting.
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

/// Removes all bookmarks for a cancelled campaign across all users.
///
/// Called internally by `cancel_campaign` to ensure bookmark lists don't
/// reference campaigns that will never become active again.
pub(crate) fn prune_bookmarks_for_campaign(env: &Env, campaign_id: u32) {
    // Note: This is a cleanup helper. In practice, iterating all users is not
    // feasible on-chain. The current implementation documents the gap (#667)
    // without a full solution. A future enhancement could maintain a reverse
    // index (campaign_id -> list of bookmarkers) to make this O(bookmarkers)
    // instead of O(all_users), but that adds write overhead to save_campaign.
    // For now, bookmarks persist after cancellation and clients should filter
    // cancelled campaigns in their UI.
    let _ = (env, campaign_id);
}

#[cfg(test)]
mod tests {
    use crate::tests::helpers::*;
    use crate::Category;
    use soroban_sdk::{Address, FromVal, String};

    #[test]
    fn test_save_campaign_emits_campaign_bookmarked_event() {
        let (env, _admin, creator, user, _c2, _token, _token_admin, client) = setup_env();

        let id = client.create_campaign(&make_params(
            creator.clone(),
            String::from_str(&env, "Campaign"),
            String::from_str(&env, "Desc"),
            1000,
            30,
            Category::Learner,
            false,
            0,
            0i128,
        ));

        let events_before = env.events().all().len();
        client.save_campaign(&user, &id);
        let events_after = env.events().all().len();

        // Exactly one event should have been emitted
        assert_eq!(
            events_after - events_before,
            1,
            "save_campaign must emit exactly 1 event"
        );

        let events = env.events().all();
        let last_event = events.last().unwrap();
        let topics = &last_event.1;
        let data = &last_event.2;

        // Topic 0: event name symbol
        let topic0: String = FromVal::from_val(&env, &topics.get(0).unwrap());
        assert_eq!(topic0, String::from_str(&env, "campaign_bookmarked"));

        // Topic 1: user address
        assert_eq!(topics.len(), 2);
        let topic1: Address = FromVal::from_val(&env, &topics.get(1).unwrap());
        assert_eq!(topic1, user);

        // Data: campaign_id as u32
        let payload: u32 = FromVal::from_val(&env, data);
        assert_eq!(payload, id);
    }

    #[test]
    fn test_remove_saved_campaign_emits_campaign_unbookmarked_event() {
        let (env, _admin, creator, user, _c2, _token, _token_admin, client) = setup_env();

        let id = client.create_campaign(&make_params(
            creator.clone(),
            String::from_str(&env, "Campaign"),
            String::from_str(&env, "Desc"),
            1000,
            30,
            Category::Learner,
            false,
            0,
            0i128,
        ));

        client.save_campaign(&user, &id);

        let events_before = env.events().all().len();
        client.remove_saved_campaign(&user, &id);
        let events_after = env.events().all().len();

        // Exactly one event should have been emitted
        assert_eq!(
            events_after - events_before,
            1,
            "remove_saved_campaign must emit exactly 1 event"
        );

        let events = env.events().all();
        let last_event = events.last().unwrap();
        let topics = &last_event.1;
        let data = &last_event.2;

        // Topic 0: event name symbol
        let topic0: String = FromVal::from_val(&env, &topics.get(0).unwrap());
        assert_eq!(topic0, String::from_str(&env, "campaign_unbookmarked"));

        // Topic 1: user address
        assert_eq!(topics.len(), 2);
        let topic1: Address = FromVal::from_val(&env, &topics.get(1).unwrap());
        assert_eq!(topic1, user);

        // Data: campaign_id as u32
        let payload: u32 = FromVal::from_val(&env, data);
        assert_eq!(payload, id);
    }
}
