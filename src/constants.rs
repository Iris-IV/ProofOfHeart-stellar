//! Named constants shared across the contract.
//!
//! Hoisted here so magic numbers (`9999`, `10000`, `86400`) appear exactly
//! once in the codebase (#482). All callers import these constants rather
//! than using local literals (#671).

/// Basis-point denominator: 10_000 bps == 100%.
pub(crate) const BPS_DENOMINATOR: u32 = 10_000;

/// Offset added to a numerator before dividing by [`BPS_DENOMINATOR`] to
/// achieve ceiling division: `ceil(a / b) == (a + b - 1) / b`.
///
/// Consumed in `campaigns/withdraw.rs` for the platform-fee and reserve
/// ceiling-division computations.
pub(crate) const BPS_CEIL_OFFSET: i128 = BPS_DENOMINATOR as i128 - 1;

/// Number of seconds in one day.
pub(crate) const SECONDS_PER_DAY: u64 = 86_400;

/// Default delay before a proposed token update can be accepted (7 days).
///
/// This is only the fallback used until the admin sets an explicit override
/// via `set_token_update_delay_secs` (#650); the value actually enforced by
/// `propose_token_update` is read from storage and falls back to this
/// constant, so platforms that want a longer or shorter timelock no longer
/// need a code change and redeploy.
pub(crate) const TOKEN_UPDATE_DELAY_SECS: u64 = 7 * SECONDS_PER_DAY;

/// Upper bound accepted by `set_token_update_delay_secs` (365 days), so the
/// admin-configurable range stays sane while still covering any realistic
/// timelock policy (#650).
pub(crate) const MAX_TOKEN_UPDATE_DELAY_SECS: u64 = 365 * SECONDS_PER_DAY;
