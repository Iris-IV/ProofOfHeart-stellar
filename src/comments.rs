//! On-chain censure record for off-chain campaign comments (#797).
//!
//! Comments live off-chain. That is the right place for them — text is
//! expensive to store on a ledger and nobody needs consensus on prose — but it
//! leaves moderation entirely unaccountable: a fraudulent campaign operator
//! who controls the comment backend can delete criticism and leave no trace
//! that it ever existed.
//!
//! This module does not move comments on-chain. It records the *act of
//! removal*: an admin marks a comment hash as censured, and that mark is
//! immutable and public. The frontend checks the flag before displaying a
//! comment, and — more importantly — anyone can enumerate the censure events
//! and see what was suppressed and when.
//!
//! What this does and does not give you:
//!
//! - It **does** make suppression evidence-producing. Removing a comment now
//!   leaves a permanent on-chain record naming the hash and the reason.
//! - It **does not** prevent suppression. An operator who controls the
//!   frontend can still refuse to render a comment without censuring it. The
//!   defence is that a comment absent from the UI but *not* on the censure
//!   list is detectable by anyone holding the off-chain corpus.
//! - It **does not** authenticate comment authorship. The hash commits to
//!   content, not to who wrote it.
//!
//! The hash is supplied by the caller and never interpreted here. Treating it
//! as an opaque 32-byte identifier keeps the contract agnostic about how the
//! off-chain system canonicalises a comment.

use soroban_sdk::{Address, BytesN, Env, String};

use crate::errors::Error;
use crate::lifecycle::{assert_admin, get_campaign_or_error, require_not_paused};
use crate::storage::{
    bump_instance_ttl, get_campaign_censured_count, get_comment_censure, is_comment_censured,
    set_campaign_censured_count, set_comment_censure, CommentCensure,
};

/// Longest accepted censure reason, in bytes.
///
/// A reason is mandatory — an unexplained censure is only marginally better
/// than a silent deletion — but it is a label, not an essay, and the ledger
/// pays for every byte.
pub const CENSURE_REASON_MAX_LEN: u32 = 200;

/// Mark an off-chain comment as censured. Admin only.
///
/// Idempotent: re-censuring an already-censured comment succeeds and changes
/// nothing. A moderation tool retrying after a dropped response must not
/// produce a second event, or the audit trail would over-count suppressions.
pub(crate) fn censure_comment(
    env: &Env,
    admin: Address,
    campaign_id: u32,
    comment_hash: BytesN<32>,
    reason: String,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    require_not_paused(env)?;

    // The campaign must exist. A censure against a non-existent campaign is
    // either a bug or an attempt to pollute the record with unverifiable
    // entries.
    get_campaign_or_error(env, campaign_id)?;

    let reason_len = reason.len();
    if reason_len == 0 || reason_len > CENSURE_REASON_MAX_LEN {
        return Err(Error::ValidationFailed);
    }

    if is_comment_censured(env, campaign_id, &comment_hash) {
        return Ok(());
    }

    bump_instance_ttl(env);

    let record = CommentCensure {
        reason: reason.clone(),
        censured_at: env.ledger().timestamp(),
        admin: admin.clone(),
    };
    set_comment_censure(env, campaign_id, &comment_hash, &record);

    let count = get_campaign_censured_count(env, campaign_id);
    set_campaign_censured_count(env, campaign_id, count.saturating_add(1));

    // The event is the point of the whole mechanism: the storage entry can be
    // read by anyone who knows the hash, but the event stream lets an observer
    // discover suppressions they were never told about.
    env.events().publish(
        ("comment_censured", campaign_id, comment_hash),
        (admin, reason, record.censured_at),
    );

    Ok(())
}

/// Lift a censure. Admin only.
///
/// Reversing a moderation decision must itself be recorded — an admin who
/// could quietly un-censure could launder a suppression by censuring, waiting,
/// and reverting. The counter decrements but the event history keeps both
/// acts.
pub(crate) fn uncensure_comment(
    env: &Env,
    admin: Address,
    campaign_id: u32,
    comment_hash: BytesN<32>,
) -> Result<(), Error> {
    assert_admin(env, &admin)?;
    require_not_paused(env)?;
    get_campaign_or_error(env, campaign_id)?;

    if !is_comment_censured(env, campaign_id, &comment_hash) {
        return Ok(());
    }

    bump_instance_ttl(env);
    crate::storage::remove_comment_censure(env, campaign_id, &comment_hash);

    let count = get_campaign_censured_count(env, campaign_id);
    set_campaign_censured_count(env, campaign_id, count.saturating_sub(1));

    env.events().publish(
        ("comment_uncensured", campaign_id, comment_hash),
        (admin, env.ledger().timestamp()),
    );

    Ok(())
}

/// Whether a comment is currently censured.
///
/// The read the frontend performs before rendering. Cheap and total: an
/// unknown hash is simply not censured, so a caller can ask about any comment
/// without a prior existence check.
pub(crate) fn comment_is_censured(env: &Env, campaign_id: u32, comment_hash: BytesN<32>) -> bool {
    is_comment_censured(env, campaign_id, &comment_hash)
}

/// The full censure record, or `None` when the comment is not censured.
pub(crate) fn comment_censure_record(
    env: &Env,
    campaign_id: u32,
    comment_hash: BytesN<32>,
) -> Option<CommentCensure> {
    get_comment_censure(env, campaign_id, &comment_hash)
}

/// How many comments have been censured on a campaign.
///
/// Lets a reader spot a campaign with an unusual amount of moderation without
/// enumerating hashes they may not know.
pub(crate) fn campaign_censured_comment_count(env: &Env, campaign_id: u32) -> u32 {
    get_campaign_censured_count(env, campaign_id)
}
