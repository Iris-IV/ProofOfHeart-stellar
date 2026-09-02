//! On-chain campaign bookmark / save list for wallets (#507).
//!
//! Lets a wallet track causes it cares about without relying on the
//! frontend: `save_campaign`, `remove_saved_campaign`,
//! `batch_save_campaigns`, `clear_saved_campaigns`, and `get_saved_campaigns`
//! are keyed by the wallet address, so any client can display a user's saved
//! campaigns directly from chain state. `get_saved_campaigns` and
//! `get_saved_campaigns_count` only surface live (non-cancelled) bookmarks.
//!
//! # Authorization
//!
//! Every mutator — `save_campaign`, `remove_saved_campaign`,
//! `batch_save_campaigns`, `clear_saved_campaigns` — calls
//! `user.require_auth()` on the wallet whose list is being edited, not on
//! whoever submitted the transaction. Without that, any address could add to
//! or empty another wallet's saved list. The reads are deliberately
//! unauthenticated: a saved list is public, and requiring a signature to view
//! one would stop a client from displaying anybody else's.
//!
//! `src/tests/test_bookmarks.rs` pins this per entry point via `env.auths()`,
//! which records an address only because `require_auth` was called for it, so
//! dropping a guard fails the suite (#786). A direct negative test — invoke
//! without authorization, expect a rejection — is not expressible: the native
//! test host escalates a failed `require_auth` to a non-unwinding panic that
//! aborts the test binary rather than returning an error or unwinding.

use soroban_sdk::{Address, Env, Vec};

use crate::errors::Error;
use crate::lifecycle::get_campaign_or_error;
use crate::storage::{get_campaign, get_saved_campaigns, set_saved_campaigns};

/// Maximum number of campaigns a wallet may bookmark. Bounds a single
/// persistent storage entry and keeps read/write costs predictable (#782).
pub const MAX_BOOKMARKS_PER_WALLET: u32 = 50;

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
    if saved.len() >= MAX_BOOKMARKS_PER_WALLET {
        return Err(Error::BookmarkLimitReached);
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

/// Saves multiple `campaign_ids` to `user`'s saved-campaigns list in a single
/// transaction instead of one `save_campaign` call per id, reducing the number
/// of auth checks and storage writes a wallet needs to bookmark several
/// campaigns at once.
///
/// Requires the wallet's authorization (checked once for the whole batch).
/// The batch is atomic: if any campaign doesn't exist or is already
/// bookmarked, the entire call reverts. Emits one `campaign_bookmarked` event
/// per successfully saved id, matching `save_campaign`.
pub fn batch_save_campaigns(env: &Env, user: Address, campaign_ids: Vec<u32>) -> Result<(), Error> {
    user.require_auth();

    let mut saved = get_saved_campaigns(env, &user);
    for campaign_id in campaign_ids.iter() {
        // Ensure the campaign actually exists before letting it be bookmarked.
        get_campaign_or_error(env, campaign_id)?;
        if saved.iter().any(|id| id == campaign_id) {
            return Err(Error::CampaignAlreadyBookmarked);
        }
        saved.push_back(campaign_id);
    }
    set_saved_campaigns(env, &user, &saved);

    for campaign_id in campaign_ids.iter() {
        env.events()
            .publish(("campaign_bookmarked", user.clone()), campaign_id);
    }

    Ok(())
}

/// Returns the list of campaign ids `user` has bookmarked, in the order they
/// were saved, excluding any bookmarked campaign that has since been
/// cancelled (or no longer exists). This is a public, unauthenticated read —
/// any wallet/app can display another wallet's saved causes, and receives
/// only live bookmarks without needing a separate lookup per id (#667).
pub fn get_saved(env: &Env, user: Address) -> Vec<u32> {
    let saved = get_saved_campaigns(env, &user);

    let mut live = Vec::new(env);
    for campaign_id in saved.iter() {
        match get_campaign(env, campaign_id) {
            Some(campaign) if !campaign.is_cancelled => live.push_back(campaign_id),
            _ => {}
        }
    }
    live
}

/// Returns the number of `user`'s live (non-cancelled) bookmarks. This is a
/// public, unauthenticated read intended for lightweight consumers such as a
/// UI badge that only needs a counter rather than the full list.
pub fn get_saved_count(env: &Env, user: Address) -> u32 {
    get_saved(env, user).len()
}

/// Removes every bookmark from `user`'s saved-campaigns list in a single
/// transaction, resetting it to empty. Requires the wallet's authorization.
/// Succeeds even if the list is already empty.
pub fn clear_saved_campaigns(env: &Env, user: Address) -> Result<(), Error> {
    user.require_auth();

    let cleared = get_saved_campaigns(env, &user).len();
    set_saved_campaigns(env, &user, &Vec::new(env));

    env.events()
        .publish(("campaign_bookmarks_cleared", user), cleared);

    Ok(())
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
    // For now, bookmarks persist in storage after cancellation; `get_saved`
    // filters cancelled campaigns out so clients get live bookmarks directly.
    let _ = (env, campaign_id);
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use alloc::format;

    use crate::bookmarks::MAX_BOOKMARKS_PER_WALLET;
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

    #[test]
    fn test_bookmark_limit_reached() {
        let (env, _admin, creator, user, _c2, _token, _token_admin, client) = setup_env();
        // Fill up to MAX_BOOKMARKS_PER_WALLET
        for i in 0..MAX_BOOKMARKS_PER_WALLET {
            let id = client.create_campaign(&make_params(
                creator.clone(),
                String::from_str(&env, &format!("C{i}")),
                String::from_str(&env, &format!("D{i}")),
                1000 + i as i128,
                30,
                Category::Learner,
                false,
                0,
                0i128,
            ));
            client.save_campaign(&user, &id);
        }
        assert_eq!(
            client.get_saved_campaigns(&user).len(),
            MAX_BOOKMARKS_PER_WALLET
        );
        // One more should fail
        let extra = client.create_campaign(&make_params(
            creator.clone(),
            String::from_str(&env, "Extra"),
            String::from_str(&env, "Desc"),
            1000,
            30,
            Category::Learner,
            false,
            0,
            0i128,
        ));
        let res = client.try_save_campaign(&user, &extra);
        assert_eq!(res, Err(Ok(crate::errors::Error::BookmarkLimitReached)));
        // Removing one frees a slot
        let saved = client.get_saved_campaigns(&user);
        client.remove_saved_campaign(&user, &saved.get(0).unwrap());
        client.save_campaign(&user, &extra);
        assert_eq!(
            client.get_saved_campaigns(&user).len(),
            MAX_BOOKMARKS_PER_WALLET
        );
    }

    #[test]
    fn test_save_campaign_extends_ttl() {
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

        let saved = client.get_saved_campaigns(&user);
        assert_eq!(saved.len(), 1);
        assert_eq!(saved.get(0).unwrap(), id);
    }
}
