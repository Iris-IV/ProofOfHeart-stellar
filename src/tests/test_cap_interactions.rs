//! Property-based tests for the interaction between `max_contribution_per_user`
//! (the campaign-wide, lifetime, creator-set cap) and `personal_cap` (an
//! optional per-contributor override on top of it) (#505).
//!
//! Two edge cases were previously uncovered: a personal cap set above the
//! campaign's max, and cap state after a partial refund. On the refund
//! case: `contribute()` unconditionally rejects further contributions to a
//! campaign that's cancelled (`CampaignNotActive`) or past its deadline
//! (`DeadlinePassed`) — the same two conditions that make a refund
//! claimable in the first place. So a contributor can never actually
//! re-contribute to the same campaign after claiming a refund; the
//! meaningful property is state consistency across the refund, not
//! re-contribution: `current` and the `personal_cap` record both reset,
//! while `lifetime_contribution` — the value the campaign-wide cap checks
//! against — is deliberately preserved.

use proptest::prelude::*;

use super::helpers::*;
use crate::{Category, CreateCampaignParams, Error};
use soroban_sdk::{
    testutils::{AuthorizedFunction, AuthorizedInvocation, Ledger},
    IntoVal, String, Symbol, TryFromVal,
};

// ── Pure logic: mirrors `set_personal_cap_fn`'s bound check and the dual
//    cap check inside `contribute()` (src/contributions.rs) ────────────────

/// Mirrors `set_personal_cap_fn`'s validation: negative amounts are always
/// rejected; when the campaign has a cap (`campaign_max > 0`), the personal
/// cap may not exceed it. `campaign_max == 0` is the "unlimited" sentinel.
fn personal_cap_settable(campaign_max: i128, requested: i128) -> bool {
    requested >= 0 && (campaign_max == 0 || requested <= campaign_max)
}

/// Mirrors the combined cap check inside `contribute()`: a contribution is
/// bounded by whichever cap leaves less headroom — the campaign-wide cap
/// (checked against ever-increasing `lifetime`) and the optional personal
/// cap (checked against `current`, which can reset via refund).
fn max_contribution_allowed(
    campaign_max: i128,
    lifetime: i128,
    personal_cap: Option<i128>,
    current: i128,
) -> i128 {
    let campaign_remaining = if campaign_max == 0 {
        i128::MAX
    } else {
        campaign_max - lifetime
    };
    let personal_remaining = match personal_cap {
        Some(p) => p - current,
        None => i128::MAX,
    };
    campaign_remaining.min(personal_remaining)
}

proptest! {
    /// Any requested personal cap strictly above a nonzero campaign max must
    /// be rejected, for any campaign max / overage pair.
    #[test]
    fn prop_personal_cap_above_max_rejected_when_max_nonzero(
        campaign_max in 1i128..=1_000_000_000i128,
        overage in 1i128..=1_000_000_000i128,
    ) {
        let requested = campaign_max.saturating_add(overage);
        prop_assert!(!personal_cap_settable(campaign_max, requested));
    }

    /// Any requested personal cap at or below a nonzero campaign max must be
    /// settable.
    #[test]
    fn prop_personal_cap_at_or_below_max_settable(
        campaign_max in 1i128..=1_000_000_000i128,
        deficit in 0i128..=1_000_000_000i128,
    ) {
        let requested = (campaign_max - deficit).max(0);
        prop_assert!(personal_cap_settable(campaign_max, requested));
    }

    /// `campaign_max == 0` is the explicit "no cap" sentinel (#530) — any
    /// non-negative personal cap must be settable regardless of its size.
    #[test]
    fn prop_personal_cap_any_nonnegative_allowed_when_max_unlimited(
        requested in 0i128..=i128::MAX,
    ) {
        prop_assert!(personal_cap_settable(0, requested));
    }

    /// When both caps are active and currently satisfied (`lifetime <=
    /// campaign_max`, `current <= personal_cap`), the amount a contributor
    /// may still add is exactly the smaller of the two caps' remaining
    /// headroom — never determined by only one of the two.
    #[test]
    fn prop_dual_cap_binding_constraint_is_the_tighter_cap(
        campaign_max in 1i128..=1_000_000i128,
        lifetime in 0i128..=1_000_000i128,
        current in 0i128..=1_000_000i128,
        personal_cap in 0i128..=1_000_000i128,
    ) {
        prop_assume!(lifetime <= campaign_max);
        prop_assume!(current <= personal_cap);

        let allowed = max_contribution_allowed(campaign_max, lifetime, Some(personal_cap), current);
        let campaign_remaining = campaign_max - lifetime;
        let personal_remaining = personal_cap - current;

        prop_assert_eq!(allowed, campaign_remaining.min(personal_remaining));
        prop_assert!(allowed >= 0);
    }
}

// ── Stateful: drive the real contract through `ProofOfHeartClient` ─────────
// Bounded to a small case count since each case pays a fresh Env + token +
// contract registration, the same setup cost every ordinary `#[test]` in
// this suite already pays once.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// `set_personal_cap` must reject any value above the campaign's
    /// `max_contribution_per_user`, across generated cap/overage pairs —
    /// generalizes the fixed-value regression test for #355.
    #[test]
    fn prop_personal_cap_set_rejected_above_campaign_max(
        campaign_max in 1i128..=1_000_000i128,
        overage in 1i128..=1_000_000i128,
    ) {
        let (env, _admin, creator, contributor1, _, _token, _token_admin, client) = setup_env();
        let requested = campaign_max.saturating_add(overage);

        let campaign_id = client.create_campaign(&CreateCampaignParams {
            creator: creator.clone(),
            title: String::from_str(&env, "Cap ceiling"),
            description: String::from_str(&env, "personal cap above campaign max"),
            funding_goal: 10_000_000,
            duration_days: 30,
            category: Category::Learner,
            has_revenue_sharing: false,
            revenue_share_percentage: 0,
            max_contribution_per_user: campaign_max,
        });

        let res = client.try_set_personal_cap(&campaign_id, &contributor1, &requested);
        prop_assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
    }

    /// After a creator-cancel refund: `current` resets to 0 and the
    /// `personal_cap` record is cleared (both keyed off the live/settable
    /// state), while `lifetime_contribution` — what the campaign-wide cap
    /// checks against — is preserved unchanged. Generalizes the fixed-value
    /// `test_claim_refund_preserves_lifetime_contribution`.
    #[test]
    fn prop_cancel_refund_preserves_lifetime_clears_current_and_personal_cap(
        campaign_max in 1i128..=2_000i128,
        personal_cap in 1i128..=2_000i128,
        amount in 1i128..=2_000i128,
    ) {
        prop_assume!(personal_cap <= campaign_max);
        prop_assume!(amount <= personal_cap);

        let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
        token_admin.mint(&contributor1, &1_000_000);

        let campaign_id = client.create_campaign(&CreateCampaignParams {
            creator: creator.clone(),
            title: String::from_str(&env, "Cancel refund cap"),
            description: String::from_str(&env, "state after refund"),
            funding_goal: 1_000_000, // never met, so cancel is always allowed
            duration_days: 30,
            category: Category::Learner,
            has_revenue_sharing: false,
            revenue_share_percentage: 0,
            max_contribution_per_user: campaign_max,
        });
        client.verify_campaign(&campaign_id);
        client.set_personal_cap(&campaign_id, &contributor1, &personal_cap);
        client.contribute(&campaign_id, &contributor1, &amount);

        client.cancel_campaign(&campaign_id);
        client.claim_refund(&campaign_id, &contributor1);

        prop_assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);
        prop_assert_eq!(
            client.get_lifetime_contribution(&campaign_id, &contributor1),
            amount
        );
        prop_assert_eq!(client.get_personal_cap(&campaign_id, &contributor1), 0);
    }

    /// Same invariant as above, via the other refund trigger: deadline
    /// passed with the funding goal unmet. Neither existing example test
    /// nor the property above exercises this path.
    #[test]
    fn prop_deadline_refund_preserves_lifetime_contribution(
        funding_goal in 5_000i128..=20_000i128,
        campaign_max in 1i128..=2_000i128,
        amount in 1i128..=2_000i128,
    ) {
        prop_assume!(amount <= campaign_max);
        prop_assume!(amount < funding_goal);

        let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
        token_admin.mint(&contributor1, &1_000_000);

        let campaign_id = client.create_campaign(&CreateCampaignParams {
            creator: creator.clone(),
            title: String::from_str(&env, "Deadline refund cap"),
            description: String::from_str(&env, "state after deadline-triggered refund"),
            funding_goal,
            duration_days: 1,
            category: Category::Learner,
            has_revenue_sharing: false,
            revenue_share_percentage: 0,
            max_contribution_per_user: campaign_max,
        });
        client.verify_campaign(&campaign_id);
        client.contribute(&campaign_id, &contributor1, &amount);

        env.ledger().with_mut(|l| {
            l.timestamp += 2 * crate::SECONDS_PER_DAY;
        });

        client.claim_refund(&campaign_id, &contributor1);

        prop_assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);
        prop_assert_eq!(
            client.get_lifetime_contribution(&campaign_id, &contributor1),
            amount
        );
    }
}

// ── remove_personal_cap (#503) ───────────────────────────────────────────────

/// Setting a cap, then removing it, fully restores the "no personal cap"
/// state: `get_personal_cap` reads 0 again and a `personal_cap_removed`
/// event is emitted.
#[test]
fn test_remove_personal_cap_restores_no_cap_state() {
    let (env, _admin, creator, contributor1, _, _token, _token_admin, client) = setup_env();

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Remove cap"),
        description: String::from_str(&env, "remove_personal_cap flow"),
        funding_goal: 10_000_000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });

    assert_eq!(client.get_personal_cap(&campaign_id, &contributor1), 0);

    client.set_personal_cap(&campaign_id, &contributor1, &500);
    assert_eq!(client.get_personal_cap(&campaign_id, &contributor1), 500);

    client.remove_personal_cap(&campaign_id, &contributor1);
    assert_eq!(client.get_personal_cap(&campaign_id, &contributor1), 0);

    // The removal event is published with (symbol, campaign_id, contributor)
    // as topics.
    let events = env.events().all();
    let last = events.last().unwrap();
    let topics = &last.1;
    assert_eq!(topics.len(), 3);
    let topic_symbol: soroban_sdk::String =
        soroban_sdk::String::try_from_val(&env, &topics.get(0).unwrap()).unwrap();
    assert_eq!(
        topic_symbol,
        soroban_sdk::String::from_str(&env, "personal_cap_removed")
    );
    let topic_campaign: u32 = soroban_sdk::FromVal::from_val(&env, &topics.get(1).unwrap());
    assert_eq!(topic_campaign, campaign_id);
}

/// Removing a cap on a cancelled/inactive campaign fails with
/// `CampaignNotActive`, mirroring `set_personal_cap_fn`'s guard.
#[test]
fn test_remove_personal_cap_inactive_campaign_returns_error() {
    let (env, _admin, creator, contributor1, _, _token, _token_admin, client) = setup_env();

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Remove cap on cancelled"),
        description: String::from_str(&env, "remove cap on inactive campaign"),
        funding_goal: 10_000_000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });
    client.set_personal_cap(&campaign_id, &contributor1, &500);

    client.cancel_campaign(&campaign_id);

    let res = client.try_remove_personal_cap(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);
}

/// Removing a cap that was never set must fail with `PersonalCapNotFound`
/// rather than silently succeeding, so indexers can trust the event.
#[test]
fn test_remove_personal_cap_nonexistent_returns_error() {
    let (env, _admin, creator, contributor1, _, _token, _token_admin, client) = setup_env();

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Remove missing cap"),
        description: String::from_str(&env, "remove cap that does not exist"),
        funding_goal: 10_000_000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });

    let res = client.try_remove_personal_cap(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::PersonalCapNotFound);
}

/// Removing a cap on a non-existent campaign fails with `CampaignNotFound`.
#[test]
fn test_remove_personal_cap_unknown_campaign_returns_error() {
    let (_env, _admin, _creator, contributor1, _, _token, _token_admin, client) = setup_env();

    let res = client.try_remove_personal_cap(&999, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotFound);
}

/// `remove_personal_cap` must be authorized by the contributor whose cap is
/// being removed: the recorded auth entries must contain exactly one
/// authorization, for that contributor, for the `remove_personal_cap` call
/// (mirrors the auth verification in `test_claim_refund_requires_contributor_auth`).
#[test]
fn test_remove_personal_cap_requires_contributor_auth() {
    let (env, _admin, creator, contributor1, _, _token, _token_admin, client) = setup_env();

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Remove cap auth"),
        description: String::from_str(&env, "remove_personal_cap auth check"),
        funding_goal: 10_000_000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });
    client.set_personal_cap(&campaign_id, &contributor1, &500);

    client.remove_personal_cap(&campaign_id, &contributor1);

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    let (auth_addr, invocation) = &auths[0];
    assert_eq!(auth_addr, &contributor1);
    assert_eq!(
        invocation,
        &AuthorizedInvocation {
            function: AuthorizedFunction::Contract((
                client.address.clone(),
                Symbol::new(&env, "remove_personal_cap"),
                (campaign_id, contributor1.clone()).into_val(&env),
            )),
            sub_invocations: Default::default(),
        }
    );
}

/// #876: repeated set/remove churn on a personal cap must leave no stale
/// `PersonalCap` ledger entry behind — each `remove_personal_cap` call has
/// to fully delete the entry (value and TTL) rather than just clearing the
/// value client-side reads as 0 for.
#[test]
fn test_remove_personal_cap_leaves_no_stale_storage_after_churn() {
    let (env, _admin, creator, contributor1, _, _token, _token_admin, client) = setup_env();

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Cap churn"),
        description: String::from_str(&env, "repeated set/remove personal cap churn"),
        funding_goal: 10_000_000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });

    let key = crate::storage::ContributionKey::PersonalCap(campaign_id, contributor1.clone());

    for i in 0..5 {
        client.set_personal_cap(&campaign_id, &contributor1, &(500 + i));
        env.as_contract(&client.address, || {
            assert!(env.storage().persistent().has(&key));
        });

        client.remove_personal_cap(&campaign_id, &contributor1);
        env.as_contract(&client.address, || {
            assert!(!env.storage().persistent().has(&key));
        });
    }

    assert_eq!(client.get_personal_cap(&campaign_id, &contributor1), 0);
}
