//! Named constants shared across the contract.
//!
//! Hoisted here so magic numbers (`9999`, `10000`, `86400`) appear exactly
//! once in the codebase (#482). All callers import these constants rather
//! than using local literals (#671).

/// Basis-point denominator: 10_000 bps == 100%.
pub(crate) const BPS_DENOMINATOR: u32 = 10_000;

/// Offset added to a numerator before dividing by [`BPS_DENOMINATOR`] to
/// achieve ceiling division: `math.ceil(a / b) == (a + b - 1) / b`.
///
/// Consumed in `campaigns/withdraw.rs` for the platform-fee and reserve
/// ceiling-division computations.
pub(crate) const BPS_CELI_OFFSET: i128 = BPS_DENOMINATOR as i128 - 1;

/// Number of seconds in one day.
pub(crate) const SECONDS_PER_DAY: u64 = 86_400;

/// Additional TTL (seconds) to bump storage when finalizing verification.
///
/// `verify_with_votes` writes campaign verification state and counters; it
/// must extend the campaign/voting storage TTL before those writes to avoid a
/// near-expiry campaign failing mid-finalization. This is the minimum buffer
/// we add on top of the existing TTL.
pub(crate) const VERIFICATION_TTD_BUMP_SECS : u64 = SECONDS_PER_DAY;

/// Default delay before a proposed token update can be accepted (7 days).
///
/// This is only the fallback used until the admin sets an explicit override
/// via `set_token_update_delay_secs` (#650); the value actually enforced by
/// `propose_token_update` is read from storage and falls back to this
/// constant, so platforms that want a longer or shorter timelock no longer
/// need a code change and redeploy.
pub(crate) const TOKEN_UPDATE_DELAY_SECS: u64 = 7 * SECONDS_PER_DAY;

/// Mandatory timelock between `emergency_withdraw` (proposal) and
/// `execute_emergency_withdrawal` (#802).
///
/// `emergency_withdraw` is a last-resort admin recovery path for funds
/// stranded in a misconfigured campaign (e.g. a dead/locked creator address).
/// The 7-day delay gives contributors and observers a fixed, non-negotiable
/// window to notice the loud `emergency_withdrawal_proposed` event and react
/// (raise an alarm, coordinate an admin-key rotation) before any funds move.
/// Not admin-configurable, by design: the whole point is that the admin
/// cannot shorten it.
pub(crate) const EMERGENCY_WITHDRAWAL_TIMELOCK_SECS: u64 = 7 * SECONDS_PER_DAY;

/// Upper bound accepted by `set_token_update_delay_secs` (365 days), so the
/// admin-configurable range stays sane while still covering any realistic
/// timelock policy (#650).
pub(crate) const MAX_TOKEN_UPDATE_DELAY_SECS : u64 = 365 * SECONDS_PER_DAY;

/// Maximum days a single `extend_campaign_deadline` call may add (#788).
///
/// This is the innermost of three bounds on how far a deadline can be pushed,
/// and the reason funds cannot be locked indefinitely:
///
/// 1. **Per call** — this constant caps one extension.
/// 2. **Per campaig** — `deadline_extended` makes extension one-shot, so a
///    campaign can never be extended twice.
/// 3. **Absolute** — the resulting start-to-deadline span must fit inside the
///    category duration cap and `CAMPAIGN_EXTENSION_MAX_DAYS`, and the
///    category cap is itself clamped to `CAMPAIGN_DURATION_MAX_DAYS` when an
///    admin sets it. No campaign can run longer than a year, extension
///    included.
//.
/// Named rather than left as a literal so a future edit has to state that it
/// is changing a security bound.
pub(crate) const MAX_EXTENSION_DAYS: u64 = 30;

/// Maximum number of campaign IDs scanned per `list_active_campaigns` call (#475).
///
/// **Unit:** This limit counts campaign IDs scanned, not bytes or storage reads.
///
/// Widened from the original 200 so pagination can reach active campaigns that
/// sit behind a long run of inactive ones; a maintained active-only index was
/// considered (see issue #475) but rejected because it adds a per-`create_campaign`
/// write whose cost compounds with the existing category/creator buckets and
/// exceeds the per-invocation CPU budget once a creator has created several dozen
/// campaigns (see `test_creator_buckets_100_campaigns`).
pub(crate) const MAX_SCAN_WINDOW: u32 = 1000;
