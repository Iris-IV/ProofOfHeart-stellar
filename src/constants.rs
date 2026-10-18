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
pub(crate) const MAX_TOKEN_UPDATE_DELAY_SECS: u64 = 365 * SECONDS_PER_DAY;

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
///
/// Named rather than left as a literal so a future edit has to state that it
/// is changing a security bound.
pub(crate) const MAX_EXTENSION_DAYS: u64 = 30;

/// How long a pending campaign-ownership nomination remains acceptable
/// before it expires (7 days) (#869).
///
/// Without an expiry, nominating an address that never calls
/// `accept_campaign_transfer` — a lost key, an abandoned wallet, a typo —
/// would leave `pending_creator` valid forever: acceptable at any point in
/// the future, however unexpected, and (previously) unable to be replaced
/// with a new nomination without an explicit `cancel_campaign_transfer`
/// first. Once this window elapses, `accept_campaign_transfer` rejects the
/// stale nomination and `initiate_campaign_transfer` is free to overwrite
/// it outright; `admin_cancel_campaign_transfer` remains available as an
/// immediate recovery path that does not require waiting out the window.
pub(crate) const TRANSFER_EXPIRY_SECS: u64 = 7 * SECONDS_PER_DAY;

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

/// Maximum number of campaigns folded into the aggregate queries
/// (`get_platform_report`, `get_creator_stats`) per call (#1237).
///
/// Those queries do one or two persistent reads per campaign, and a Soroban
/// transaction may touch at most 100 ledger entries, so the far larger
/// [`MAX_SCAN_WINDOW`] would overrun the CPU/read budget long before it was
/// reached. Keeping the bound below the entry limit leaves room for the fixed
/// reads (counters, buckets, token, fee) each query also performs.
pub(crate) const MAX_STATS_SCAN: u32 = 40;

/// Category-specific platform fee scaling multipliers in basis points out of
/// [`BPS_DENOMINATOR`] (#1019).
///
/// Non-commercial educational causes (`Learner`, `Educator`, `Publisher`)
/// receive a discounted share of the global platform fee, whereas commercial
/// `EducationalStartup` campaigns pay the full base rate unless overridden.
pub(crate) const CATEGORY_FEE_MULTIPLIER_LEARNER_BPS: u32 = 5_000;
pub(crate) const CATEGORY_FEE_MULTIPLIER_EDUCATOR_BPS: u32 = 7_500;
pub(crate) const CATEGORY_FEE_MULTIPLIER_PUBLISHER_BPS: u32 = 9_000;
pub(crate) const CATEGORY_FEE_MULTIPLIER_STARTUP_BPS: u32 = 10_000;

/// Returns the fee scaling multiplier (in basis points out of `BPS_DENOMINATOR`)
/// for a campaign's [`crate::types::Category`] (#1019).
pub(crate) const fn category_fee_multiplier_bps(category: crate::types::Category) -> u32 {
    match category {
        crate::types::Category::Learner => CATEGORY_FEE_MULTIPLIER_LEARNER_BPS,
        crate::types::Category::Educator => CATEGORY_FEE_MULTIPLIER_EDUCATOR_BPS,
        crate::types::Category::Publisher => CATEGORY_FEE_MULTIPLIER_PUBLISHER_BPS,
        crate::types::Category::EducationalStartup => CATEGORY_FEE_MULTIPLIER_STARTUP_BPS,
    }
}

/// Computes the effective platform fee in basis points for a campaign given
/// the global `base_fee_bps`, its cause `category`, and an optional per-campaign
/// `fee_override` (#1019).
pub(crate) const fn calculate_category_platform_fee_bps(
    base_fee_bps: u32,
    category: crate::types::Category,
    fee_override: Option<u32>,
) -> Result<u32, crate::errors::Error> {
    if base_fee_bps > crate::PLATFORM_FEE_MAX_BPS {
        return Err(crate::errors::Error::InvalidPlatformFee);
    }
    if let Some(override_bps) = fee_override {
        if override_bps > crate::PLATFORM_FEE_MAX_BPS {
            return Err(crate::errors::Error::InvalidPlatformFee);
        }
        return Ok(override_bps);
    }

    let multiplier = category_fee_multiplier_bps(category) as u64;
    let effective = (base_fee_bps as u64 * multiplier) / (BPS_DENOMINATOR as u64);
    Ok(effective as u32)
}

/// Computes the token fee amount from `gross_amount` and `effective_fee_bps`
/// using ceiling division consistent with withdrawal accounting (#1019).
pub(crate) const fn calculate_category_platform_fee_amount(
    gross_amount: i128,
    effective_fee_bps: u32,
) -> Result<i128, crate::errors::Error> {
    if gross_amount < 0 {
        return Err(crate::errors::Error::ValidationFailed);
    }
    if effective_fee_bps > crate::PLATFORM_FEE_MAX_BPS {
        return Err(crate::errors::Error::InvalidPlatformFee);
    }
    if gross_amount == 0 || effective_fee_bps == 0 {
        return Ok(0);
    }
    let scaled = match gross_amount.checked_mul(effective_fee_bps as i128) {
        Some(v) => v,
        None => return Err(crate::errors::Error::Overflow),
    };
    let with_ceil = match scaled.checked_add(BPS_CEIL_OFFSET) {
        Some(v) => v,
        None => return Err(crate::errors::Error::Overflow),
    };
    Ok(with_ceil / (BPS_DENOMINATOR as i128))
}

const _: () = {
    assert!(
        calculate_category_platform_fee_bps(500, crate::types::Category::Learner, None).is_ok()
    );
    assert!(calculate_category_platform_fee_amount(10_000, 250).is_ok());
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::Error;
    use crate::types::Category;

    #[test]
    fn dynamic_category_platform_fee_bps_scales_by_cause_and_respects_override() {
        assert_eq!(
            calculate_category_platform_fee_bps(400, Category::Learner, None),
            Ok(200)
        );
        assert_eq!(
            calculate_category_platform_fee_bps(400, Category::Educator, None),
            Ok(300)
        );
        assert_eq!(
            calculate_category_platform_fee_bps(400, Category::Publisher, None),
            Ok(360)
        );
        assert_eq!(
            calculate_category_platform_fee_bps(400, Category::EducationalStartup, None),
            Ok(400)
        );
        assert_eq!(
            calculate_category_platform_fee_bps(400, Category::Learner, Some(150)),
            Ok(150)
        );
        assert_eq!(
            calculate_category_platform_fee_bps(
                crate::PLATFORM_FEE_MAX_BPS + 1,
                Category::Learner,
                None
            ),
            Err(Error::InvalidPlatformFee)
        );
        assert_eq!(
            calculate_category_platform_fee_bps(
                400,
                Category::Learner,
                Some(crate::PLATFORM_FEE_MAX_BPS + 1)
            ),
            Err(Error::InvalidPlatformFee)
        );
    }

    #[test]
    fn dynamic_category_platform_fee_amount_uses_ceiling_and_guards_bounds() {
        assert_eq!(calculate_category_platform_fee_amount(10_000, 200), Ok(200));
        assert_eq!(calculate_category_platform_fee_amount(101, 200), Ok(3));
        assert_eq!(calculate_category_platform_fee_amount(10_000, 0), Ok(0));
        assert_eq!(
            calculate_category_platform_fee_amount(-1, 200),
            Err(Error::ValidationFailed)
        );
        assert_eq!(
            calculate_category_platform_fee_amount(i128::MAX, 500),
            Err(Error::Overflow)
        );
    }
}
