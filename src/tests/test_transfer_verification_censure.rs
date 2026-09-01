//! Tests for #790 (self-transfer), #796 (verification revoked on description
//! edit) and #797 (on-chain comment censure).

use super::helpers::*;
use crate::{storage, Category, Error, MaybePendingCreator};
use soroban_sdk::{testutils::Ledger, Address, BytesN, String, TryFromVal};

fn make_campaign(env: &soroban_sdk::Env, creator: &Address, client: &ProofOfHeartClient) -> u32 {
    client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(env, "Campaign Title"),
        String::from_str(env, "Campaign Description"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ))
}

// ── #790: a campaign cannot be transferred to its own creator ────────────────

/// The nomination step rejects the current creator outright.
#[test]
fn test_initiate_transfer_to_self_is_rejected() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &creator, &client);

    let res = client.try_initiate_campaign_transfer(&campaign_id, &creator);
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidNewOwner);

    // No pending transfer was recorded, so the campaign is not left in a state
    // where `cancel_campaign_transfer` is needed to clean up.
    assert!(!client.has_pending_campaign_transfer(&campaign_id));
}

/// A rejected self-nomination does not block a subsequent real one.
///
/// Worth pinning because the guard sits before the `TransferAlreadyPending`
/// check: if it were placed after a write, a failed self-transfer could leave
/// the campaign permanently un-transferable.
#[test]
fn test_rejected_self_transfer_does_not_block_a_real_transfer() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &creator, &client);

    assert!(client
        .try_initiate_campaign_transfer(&campaign_id, &creator)
        .is_err());

    let new_creator = Address::generate(&env);
    client.initiate_campaign_transfer(&campaign_id, &new_creator);
    assert!(client.has_pending_campaign_transfer(&campaign_id));

    client.accept_campaign_transfer(&campaign_id);
    assert_eq!(client.get_campaign(&campaign_id).creator, new_creator);
}

/// The accept step refuses a nomination equal to the current creator.
///
/// Unreachable through the public API — `initiate_campaign_transfer` already
/// rejects it and nothing else writes `creator` — so the state is planted
/// directly. The guard exists because the bucket rewrite in
/// `accept_campaign_transfer` is not idempotent: removing the campaign from a
/// creator's bucket and re-adding it to the same bucket would corrupt the
/// index rather than no-op.
#[test]
fn test_accept_transfer_to_self_is_rejected() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &creator, &client);

    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.pending_creator = MaybePendingCreator::from(creator.clone());
        storage::set_campaign(&env, campaign_id, &campaign);
    });

    let res = client.try_accept_campaign_transfer(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidNewOwner);

    // The creator index is untouched: the campaign still appears exactly once.
    let owned = client.get_creator_campaigns(&creator, &0, &10);
    assert_eq!(owned.0.len(), 1);
    assert_eq!(owned.0.get(0).unwrap().id, campaign_id);
}

// ── #796: editing the description revokes verification ───────────────────────

/// A verified campaign loses its badge when the description is rewritten.
#[test]
fn test_update_description_revokes_verification() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &creator, &client);

    client.verify_campaign(&campaign_id);
    assert!(client.get_campaign(&campaign_id).is_verified);
    assert_eq!(client.get_platform_stats().verified_campaigns, 1);

    let new_desc = String::from_str(&env, "A materially different pitch");
    client.update_campaign_description(&campaign_id, &new_desc);

    let campaign = client.get_campaign(&campaign_id);
    assert!(
        !campaign.is_verified,
        "verification must not survive a description edit"
    );
    assert_eq!(campaign.description, new_desc);

    // The counter follows, or platform stats would drift with every edit.
    assert_eq!(client.get_platform_stats().verified_campaigns, 0);
}

/// The revocation is announced, so anyone watching the chain sees the badge
/// drop rather than only observing it missing later.
#[test]
fn test_update_description_emits_revocation_event() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &creator, &client);
    client.verify_campaign(&campaign_id);

    client.update_campaign_description(
        &campaign_id,
        &String::from_str(&env, "Rewritten description"),
    );

    let expected = String::from_str(&env, "campaign_verification_revoked");
    assert!(
        env.events().all().iter().any(|(_, topics, _)| {
            topics
                .get(0)
                .and_then(|v| String::try_from_val(&env, &v).ok())
                .map(|s| s == expected)
                .unwrap_or(false)
        }),
        "campaign_verification_revoked event missing"
    );
}

/// An unverified campaign is unaffected: no spurious event, and the counter
/// does not underflow into billions.
#[test]
fn test_update_description_on_unverified_campaign_is_inert() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &creator, &client);

    assert_eq!(client.get_platform_stats().verified_campaigns, 0);
    client.update_campaign_description(&campaign_id, &String::from_str(&env, "Edited copy"));

    assert!(!client.get_campaign(&campaign_id).is_verified);
    assert_eq!(client.get_platform_stats().verified_campaigns, 0);

    let unexpected = String::from_str(&env, "campaign_verification_revoked");
    assert!(
        !env.events().all().iter().any(|(_, topics, _)| {
            topics
                .get(0)
                .and_then(|v| String::try_from_val(&env, &v).ok())
                .map(|s| s == unexpected)
                .unwrap_or(false)
        }),
        "revocation event emitted for a campaign that was never verified"
    );
}

/// Re-verification after an edit restores both the flag and the counter, so
/// revocation is a round trip rather than a one-way door.
#[test]
fn test_campaign_can_be_reverified_after_description_edit() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &creator, &client);

    client.verify_campaign(&campaign_id);
    client.update_campaign_description(&campaign_id, &String::from_str(&env, "Second draft"));
    assert_eq!(client.get_platform_stats().verified_campaigns, 0);

    client.verify_campaign(&campaign_id);
    assert!(client.get_campaign(&campaign_id).is_verified);
    assert_eq!(client.get_platform_stats().verified_campaigns, 1);
}

/// Repeated edits do not decrement the counter more than once — the second
/// edit finds an already-unverified campaign.
#[test]
fn test_repeated_description_edits_decrement_counter_once() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let a = make_campaign(&env, &creator, &client);
    let b = make_campaign(&env, &creator, &client);

    client.verify_campaign(&a);
    client.verify_campaign(&b);
    assert_eq!(client.get_platform_stats().verified_campaigns, 2);

    client.update_campaign_description(&a, &String::from_str(&env, "Edit one"));
    client.update_campaign_description(&a, &String::from_str(&env, "Edit two"));
    client.update_campaign_description(&a, &String::from_str(&env, "Edit three"));

    // Only campaign `a` lost its badge; `b` is untouched.
    assert_eq!(client.get_platform_stats().verified_campaigns, 1);
    assert!(client.get_campaign(&b).is_verified);
}

// ── #797: on-chain censure record for off-chain comments ─────────────────────

fn hash(env: &soroban_sdk::Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

/// The basic loop: an admin censures a comment, and the flag plus its record
/// become readable.
#[test]
fn test_censure_comment_records_flag_and_reason() {
    let (env, admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &creator, &client);
    let comment = hash(&env, 0xAB);

    assert!(!client.is_comment_censured(&campaign_id, &comment));

    env.ledger().with_mut(|l| l.timestamp = 1_700_000_000);
    let reason = String::from_str(&env, "Doxxing another contributor");
    client.censure_comment(&campaign_id, &comment, &reason);

    assert!(client.is_comment_censured(&campaign_id, &comment));

    let record = client.get_comment_censure(&campaign_id, &comment).unwrap();
    assert_eq!(record.reason, reason);
    assert_eq!(record.censured_at, 1_700_000_000);
    assert_eq!(record.admin, admin);

    assert_eq!(client.get_censured_comment_count(&campaign_id), 1);
}

/// The censure is announced on-chain. This is the whole point of the feature:
/// storage answers "is this hidden", the event answers "what was hidden", which
/// an observer can collect without being told the hashes in advance.
#[test]
fn test_censure_comment_emits_event() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &creator, &client);

    client.censure_comment(
        &campaign_id,
        &hash(&env, 0x01),
        &String::from_str(&env, "Spam"),
    );

    let expected = String::from_str(&env, "comment_censured");
    assert!(
        env.events().all().iter().any(|(_, topics, _)| {
            topics
                .get(0)
                .and_then(|v| String::try_from_val(&env, &v).ok())
                .map(|s| s == expected)
                .unwrap_or(false)
        }),
        "comment_censured event missing"
    );
}

/// Censuring twice is a no-op, so a moderation tool retrying after a dropped
/// response cannot inflate the suppression count.
#[test]
fn test_censure_comment_is_idempotent() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &creator, &client);
    let comment = hash(&env, 0x02);

    let first = String::from_str(&env, "Harassment");
    client.censure_comment(&campaign_id, &comment, &first);
    client.censure_comment(&campaign_id, &comment, &String::from_str(&env, "Different"));

    assert_eq!(client.get_censured_comment_count(&campaign_id), 1);

    // The original reason stands: a repeat call must not quietly rewrite the
    // recorded justification.
    let record = client.get_comment_censure(&campaign_id, &comment).unwrap();
    assert_eq!(record.reason, first);
}

/// Censure is per (campaign, comment): the same hash on another campaign is
/// unaffected, and counts are tracked separately.
#[test]
fn test_censure_is_scoped_to_a_campaign() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let a = make_campaign(&env, &creator, &client);
    let b = make_campaign(&env, &creator, &client);
    let comment = hash(&env, 0x03);

    client.censure_comment(&a, &comment, &String::from_str(&env, "Off topic"));

    assert!(client.is_comment_censured(&a, &comment));
    assert!(!client.is_comment_censured(&b, &comment));
    assert_eq!(client.get_censured_comment_count(&a), 1);
    assert_eq!(client.get_censured_comment_count(&b), 0);
}

/// A censure can be lifted, and lifting it is itself recorded.
#[test]
fn test_uncensure_comment_restores_and_records() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &creator, &client);
    let comment = hash(&env, 0x04);

    client.censure_comment(&campaign_id, &comment, &String::from_str(&env, "Mistake"));
    client.uncensure_comment(&campaign_id, &comment);

    assert!(!client.is_comment_censured(&campaign_id, &comment));
    assert!(client.get_comment_censure(&campaign_id, &comment).is_none());
    assert_eq!(client.get_censured_comment_count(&campaign_id), 0);

    let expected = String::from_str(&env, "comment_uncensured");
    assert!(
        env.events().all().iter().any(|(_, topics, _)| {
            topics
                .get(0)
                .and_then(|v| String::try_from_val(&env, &v).ok())
                .map(|s| s == expected)
                .unwrap_or(false)
        }),
        "comment_uncensured event missing"
    );
}

/// Lifting a censure that was never applied is a no-op rather than an
/// underflow of the per-campaign counter.
#[test]
fn test_uncensure_untouched_comment_is_a_noop() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &creator, &client);

    client.uncensure_comment(&campaign_id, &hash(&env, 0x05));
    assert_eq!(client.get_censured_comment_count(&campaign_id), 0);
}

/// An empty reason is refused. An unexplained censure is only marginally
/// better than a silent deletion, which is the thing this feature exists to
/// prevent.
#[test]
fn test_censure_requires_a_reason() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &creator, &client);
    let comment = hash(&env, 0x06);

    let res = client.try_censure_comment(&campaign_id, &comment, &String::from_str(&env, ""));
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
    assert!(!client.is_comment_censured(&campaign_id, &comment));
}

/// Censuring against a campaign that does not exist is refused, so the record
/// cannot be padded with unverifiable entries.
#[test]
fn test_censure_requires_an_existing_campaign() {
    let (env, _admin, _creator, _, _, _, _, client) = setup_env();

    let res = client.try_censure_comment(
        &999,
        &hash(&env, 0x07),
        &String::from_str(&env, "Nonexistent"),
    );
    assert!(res.is_err());
}

/// Censure is unavailable while the platform is paused, matching every other
/// state-changing admin action.
#[test]
fn test_censure_is_blocked_while_paused() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &creator, &client);
    let comment = hash(&env, 0x08);

    client.pause();
    let res = client.try_censure_comment(&campaign_id, &comment, &String::from_str(&env, "Spam"));
    assert!(res.is_err());
    assert!(!client.is_comment_censured(&campaign_id, &comment));

    client.unpause();
    client.censure_comment(&campaign_id, &comment, &String::from_str(&env, "Spam"));
    assert!(client.is_comment_censured(&campaign_id, &comment));
}

/// The read side is total: an unknown hash simply reads as not censured, so a
/// frontend can query any comment without a prior existence check.
#[test]
fn test_unknown_comment_reads_as_uncensured() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &creator, &client);

    assert!(!client.is_comment_censured(&campaign_id, &hash(&env, 0xFF)));
    assert!(client
        .get_comment_censure(&campaign_id, &hash(&env, 0xFF))
        .is_none());
    // Also for a campaign that has never been moderated at all.
    assert_eq!(client.get_censured_comment_count(&campaign_id), 0);
}

/// Several comments on one campaign accumulate independently.
#[test]
fn test_multiple_censures_accumulate_and_lift_independently() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let campaign_id = make_campaign(&env, &creator, &client);
    let (a, b, c) = (hash(&env, 0x10), hash(&env, 0x11), hash(&env, 0x12));

    client.censure_comment(&campaign_id, &a, &String::from_str(&env, "Spam"));
    client.censure_comment(&campaign_id, &b, &String::from_str(&env, "Abuse"));
    client.censure_comment(&campaign_id, &c, &String::from_str(&env, "Off topic"));
    assert_eq!(client.get_censured_comment_count(&campaign_id), 3);

    client.uncensure_comment(&campaign_id, &b);
    assert_eq!(client.get_censured_comment_count(&campaign_id), 2);
    assert!(client.is_comment_censured(&campaign_id, &a));
    assert!(!client.is_comment_censured(&campaign_id, &b));
    assert!(client.is_comment_censured(&campaign_id, &c));
}
