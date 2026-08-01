//! Named constants shared across the contract.
//!
//! Hoisted here so magic numbers (`9999`, `10000`, `86400`) appear exactly
//! once in the codebase (#482).

/// Basis-point denominator: 10_000 bps == 100%.
pub(crate) const BPS_DENOMINATOR: u32 = 10_000;

/// Offset added to a numerator before dividing by [`BPS_DENOMINATOR`] to
/// achieve ceiling division: `ceil(a / b) == (a + b - 1) / b`.
pub(crate) const BPS_CEIL_OFFSET: i128 = BPS_DENOMINATOR as i128 - 1;

/// Number of seconds in one day.
pub(crate) const SECONDS_PER_DAY: u64 = 86_400;

/// Delay before a proposed token update can be accepted (7 days).
pub(crate) const TOKEN_UPDATE_DELAY_SECS: u64 = 7 * SECONDS_PER_DAY;
