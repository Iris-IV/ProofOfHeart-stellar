//! Deadline extension bounds (#788) and verification revocation (#789).

use super::helpers::*;
use crate::{Category, Error};
use soroban_sdk::{testutils::Ledger, Address, String};

fn campaign(
    env: &soroban_sdk::Env,
    creator: &Address,
    client: &ProofOfHeartClient,
    duration_days: u64,
) -> u32 {
    client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(env, "Campaign Title"),
        String::from_str(env, "Campaign Description"),
        1000,
        duration_days,
        Category::Educator,
        false,
        0,
        0i128,
    ))
}

// ── #788: a deadline cannot be pushed arbitrarily far into the future ────────
//
// The bound is layered, and each layer is pinned below:
//
//   1. `MAX_EXTENSION_DAYS` caps a single extension at 30 days.
//   2. `deadline_extended` makes extension one-shot per campaign.
//   3. The resulting start-to-deadline span must fit inside the category
//      duration cap and `CAMPAIGN_EXTENSION_MAX_DAYS` (365), and an admin
//      setting a category cap is themselves clamped to
//      `CAMPAIGN_DURATION_MAX_DAYS`.
//
// Together these mean no campaign can run more than a year including its
// extension. The tests exist so a future edit cannot quietly remove a layer:
// dropping any one of them individually still leaves the others passing, which
// is exactly why each is asserted separately.

/// A single extension is capped at `MAX_EXTENSION_DAYS`.
#[test]
fn test_extension_is_capped_per_call() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = campaign(&env, &creator, &client, 30);

    let over = crate::MAX_EXTENSION_DAYS + 1;
    let res = client.try_extend_campaign_deadline(&id, &over);
    assert_eq!(res.unwrap_err().unwrap(), Error::ExtensionTooLong);

    // The boundary value itself is accepted.
    client.extend_campaign_deadline(&id, &crate::MAX_EXTENSION_DAYS);
    assert!(client.get_campaign(&id).deadline_extended);
}

/// A zero-day extension is rejected rather than silently doing nothing.
#[test]
fn test_zero_day_extension_is_rejected() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = campaign(&env, &creator, &client, 30);

    let res = client.try_extend_campaign_deadline(&id, &0);
    assert_eq!(res.unwrap_err().unwrap(), Error::ExtensionTooLong);
    assert!(!client.get_campaign(&id).deadline_extended);
}

/// Extension is one-shot: a creator cannot walk the deadline forward by
/// repeating small extensions.
#[test]
fn test_extension_cannot_be_repeated() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = campaign(&env, &creator, &client, 30);

    client.extend_campaign_deadline(&id, &10);

    let res = client.try_extend_campaign_deadline(&id, &10);
    assert_eq!(res.unwrap_err().unwrap(), Error::DeadlineAlreadyExtended);
}

/// An extension that would push the total campaign span past the absolute
/// maximum is refused, even though it is within the per-call cap.
///
/// This is the bound the issue is really about: without it a campaign created
/// at the maximum duration could still be extended past a year.
#[test]
fn test_extension_cannot_push_total_span_past_the_absolute_maximum() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    // Start at the longest permitted duration.
    let id = campaign(&env, &creator, &client, crate::CAMPAIGN_DURATION_MAX_DAYS);

    // Well within MAX_EXTENSION_DAYS, but the total would exceed the cap.
    let res = client.try_extend_campaign_deadline(&id, &1);
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidDuration);

    let campaign_after = client.get_campaign(&id);
    assert!(!campaign_after.deadline_extended);
}

/// The deadline actually moves by the number of days requested — the cap is a
/// bound, not a silent clamp.
#[test]
fn test_extension_moves_the_deadline_by_exactly_the_requested_days() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = campaign(&env, &creator, &client, 30);

    let before = client.get_campaign(&id).deadline;
    client.extend_campaign_deadline(&id, &7);
    let after = client.get_campaign(&id).deadline;

    assert_eq!(after - before, 7 * crate::SECONDS_PER_DAY);
}

/// An admin cannot raise a category duration cap above the absolute maximum,
/// which is what keeps layer 3 from being configurable away.
#[test]
fn test_category_duration_cap_cannot_exceed_the_absolute_maximum() {
    let (env, admin, _creator, _, _, _, _, client) = setup_env();

    let too_long = crate::CAMPAIGN_DURATION_MAX_DAYS + 1;
    let res = client.try_set_category_duration_cap(&admin, &Category::Educator, &too_long);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    let _ = env;
}

/// A deadline that has already passed cannot be extended, so an expired
/// campaign cannot be revived to keep holding funds.
#[test]
fn test_expired_campaign_cannot_be_extended() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = campaign(&env, &creator, &client, 30);

    env.ledger()
        .with_mut(|l| l.timestamp += 31 * crate::SECONDS_PER_DAY);

    let res = client.try_extend_campaign_deadline(&id, &5);
    assert_eq!(res.unwrap_err().unwrap(), Error::DeadlinePassed);
}

/// A tighter category cap binds before the absolute one.
#[test]
fn test_category_duration_cap_bounds_extensions() {
    let (env, admin, creator, _, _, _, _, client) = setup_env();

    client.set_category_duration_cap(&admin, &Category::Educator, &40);
    let id = campaign(&env, &creator, &client, 30);

    // 30 + 20 = 50 days total, past the 40-day category cap.
    let res = client.try_extend_campaign_deadline(&id, &20);
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidDuration);

    // 30 + 10 = 40 fits exactly.
    client.extend_campaign_deadline(&id, &10);
    assert!(client.get_campaign(&id).deadline_extended);
}

// ── #789 + freeze policy: description edits on verified campaigns are rejected ──────────

/// The core behaviour: a verified campaign's description is frozen — editing
/// it is rejected with CampaignAlreadyVerified.
#[test]
fn test_description_edit_blocked_on_verified_campaign() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = campaign(&env, &creator, &client, 30);

    client.verify_campaign(&id);
    assert!(client.get_campaign(&id).is_verified);

    let res = client.try_update_campaign_description(
        &id,
        &String::from_str(&env, "A different pitch entirely"),
    );
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignAlreadyVerified);

    // Badge and counter are untouched.
    assert!(client.get_campaign(&id).is_verified);
    assert_eq!(client.get_platform_stats().verified_campaigns, 1);
}

/// Because the edit is blocked, stale votes are never a concern — the
/// description that was voted on cannot change.
#[test]
fn test_votes_are_intact_because_description_edit_is_blocked() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();
    let id = campaign(&env, &creator, &client, 30);

    token_admin.mint(&contributor1, &1_000_000);
    token_admin.mint(&contributor2, &1_000_000);

    client.vote_on_campaign(&id, &contributor1, &true);
    client.vote_on_campaign(&id, &contributor2, &true);
    assert_eq!(client.get_approve_votes(&id), 2);

    client.verify_campaign(&id);

    // Edit is rejected; votes remain.
    let _ = client.try_update_campaign_description(
        &id,
        &String::from_str(&env, "Attempted rewrite after approval"),
    );
    assert_eq!(client.get_approve_votes(&id), 2);
}

/// A blocked edit does not allow a bait-and-switch: community re-verification
/// on stale votes is impossible because the description never changed.
#[test]
fn test_bait_and_switch_is_prevented_by_freeze() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();
    let id = campaign(&env, &creator, &client, 30);

    token_admin.mint(&contributor1, &1_000_000);
    token_admin.mint(&contributor2, &1_000_000);
    client.vote_on_campaign(&id, &contributor1, &true);
    client.vote_on_campaign(&id, &contributor2, &true);

    client.verify_campaign(&id);

    // Creator cannot rewrite the description after verification.
    let res = client.try_update_campaign_description(
        &id,
        &String::from_str(&env, "Bait and switch"),
    );
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignAlreadyVerified);
    assert!(client.get_campaign(&id).is_verified);
}

/// Editing an unverified campaign still works, and does not disturb its votes.
#[test]
fn test_description_edit_on_unverified_campaign_keeps_votes() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    let id = campaign(&env, &creator, &client, 30);

    token_admin.mint(&contributor1, &1_000_000);
    client.vote_on_campaign(&id, &contributor1, &true);
    assert_eq!(client.get_approve_votes(&id), 1);

    client.update_campaign_description(&id, &String::from_str(&env, "Still gathering votes"));

    assert_eq!(client.get_approve_votes(&id), 1);
    assert!(!client.get_campaign(&id).is_verified);
}

/// `update_campaign` also rejects edits after verification — consistent
/// freeze policy across both entry points (#416).
#[test]
fn test_update_campaign_still_rejects_edits_after_verification() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = campaign(&env, &creator, &client, 30);

    client.verify_campaign(&id);

    let res = client.try_update_campaign(
        &id,
        &String::from_str(&env, "New Title"),
        &String::from_str(&env, "New Description"),
    );
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignAlreadyVerified);
    assert!(client.get_campaign(&id).is_verified);
}
