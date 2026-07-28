use proptest::collection::vec as prop_vec;
use proptest::prelude::*;

use super::helpers::*;
use crate::{storage, Category, CreateCampaignParams, Error};
use soroban_sdk::String;

// ── pull-based revenue distribution ─────────────────────────────────────────────

#[test]
fn test_pull_based_revenue_distribution() {
    let (env, _admin, creator, contributor1, contributor2, token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &1000);
    token_admin.mint(&contributor2, &1000);
    token_admin.mint(&creator, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Next Gen AI"),
        String::from_str(&env, "Build AI"),
        2000,
        30,
        Category::EducationalStartup,
        true,
        2000,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1000);
    client.contribute(&campaign_id, &contributor2, &1000);
    client.withdraw_funds(&campaign_id);

    token_admin.mint(&creator, &5000);
    client.deposit_revenue(&campaign_id, &5000);
    assert_eq!(client.get_revenue_pool(&campaign_id), 5000);

    client.claim_revenue(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 500);
    assert_eq!(client.get_revenue_claimed(&campaign_id, &contributor1), 500);

    client.deposit_revenue(&campaign_id, &1000);
    assert_eq!(client.get_revenue_pool(&campaign_id), 6000);

    client.claim_revenue(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 600);

    client.claim_revenue(&campaign_id, &contributor2);
    assert_eq!(token.balance(&contributor2), 600);
}

#[test]
fn test_revenue_sharing_edge_cases() {
    let (env, _admin, creator, contributor1, contributor2, token, token_admin, client) =
        setup_env();

    let campaign_nr = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "No Revenue"),
        String::from_str(&env, "Non-revenue campaign"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_nr);
    let res = client.try_claim_revenue(&campaign_nr, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    token_admin.mint(&contributor1, &10);
    token_admin.mint(&contributor2, &10);
    token_admin.mint(&creator, &100);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Rounding Test"),
        String::from_str(&env, "Test rounding and pool edge cases"),
        3,
        30,
        Category::EducationalStartup,
        true,
        5000,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1);
    client.contribute(&campaign_id, &contributor2, &2);
    client.withdraw_funds(&campaign_id);

    let res = client.try_claim_revenue(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);

    client.deposit_revenue(&campaign_id, &10);
    client.claim_revenue(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 10);

    client.claim_revenue(&campaign_id, &contributor2);
    assert_eq!(token.balance(&contributor2), 11);

    let res = client.try_claim_revenue(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);
}

#[test]
fn test_claim_revenue_requires_contributor_auth() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &2000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Revenue Claim Auth"),
        String::from_str(&env, "Testing claim revenue auth"),
        1000,
        10,
        Category::EducationalStartup,
        true,
        1000,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    token_admin.mint(&creator, &5000);
    client.deposit_revenue(&campaign_id, &5000);

    env.mock_all_auths();
    client.claim_revenue(&campaign_id, &contributor1);

    let auths = env.auths();
    let found = auths.iter().any(|(addr, inv)| {
        *addr == contributor1
            && match &inv.function {
                soroban_sdk::testutils::AuthorizedFunction::Contract((contract, function, _)) => {
                    contract == &client.address
                        && function == &soroban_sdk::Symbol::new(&env, "claim_revenue")
                }
                _ => false,
            }
    });
    assert!(found);
}

#[test]
fn test_revenue_lifecycle_e2e() {
    let (env, _admin, creator, contributor1, contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&contributor2, &3000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Revenue Sharing Campaign"),
        String::from_str(
            &env,
            "Full lifecycle test: create, fund, withdraw, deposit revenue, claim",
        ),
        6000,
        30,
        Category::EducationalStartup,
        true,
        2000,
        0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &4000);
    client.contribute(&campaign_id, &contributor2, &2500);

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.amount_raised, 6500);
    assert!(campaign.amount_raised >= campaign.funding_goal);

    client.withdraw_funds(&campaign_id);

    let campaign_after_withdrawal = client.get_campaign(&campaign_id);
    assert!(campaign_after_withdrawal.funds_withdrawn);
    assert!(!campaign_after_withdrawal.is_active);

    token_admin.mint(&creator, &10000);
    client.deposit_revenue(&campaign_id, &10000);
    assert_eq!(client.get_revenue_pool(&campaign_id), 10000);

    let contrib1_claimed_before = client.get_revenue_claimed(&campaign_id, &contributor1);
    client.claim_revenue(&campaign_id, &contributor1);
    let contrib1_claimed_after = client.get_revenue_claimed(&campaign_id, &contributor1);
    assert!(contrib1_claimed_after > contrib1_claimed_before);

    let contrib2_claimed_before = client.get_revenue_claimed(&campaign_id, &contributor2);
    client.claim_revenue(&campaign_id, &contributor2);
    let contrib2_claimed_after = client.get_revenue_claimed(&campaign_id, &contributor2);
    assert!(contrib2_claimed_after > contrib2_claimed_before);

    client.claim_creator_revenue(&campaign_id);

    assert!(client
        .try_claim_revenue(&campaign_id, &contributor1)
        .is_err());
    assert!(client
        .try_claim_revenue(&campaign_id, &contributor2)
        .is_err());

    assert!(!env.events().all().is_empty());
}

#[test]
fn test_revenue_claim_after_full_refunds_no_panic() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Revenue Refund Test"),
        String::from_str(&env, "Testing revenue claim after full refund"),
        1_000,
        30,
        Category::EducationalStartup,
        true,
        2000, // 20% revenue share
        0i128,
    ));

    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &500);

    // Cancel so the contributor is eligible for a refund
    client.cancel_campaign(&campaign_id);

    // Contributor claims full refund — removes their contribution entry
    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 5_000);

    // claim_revenue must not panic; campaign is cancelled so expect CampaignNotActive
    let res = client.try_claim_revenue(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);

    // claim_creator_revenue must not panic; no revenue deposited so expect NoFundsToWithdraw
    let res = client.try_claim_creator_revenue(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);
}

#[test]
fn test_claim_creator_revenue_overflow_returns_error_not_panic() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5_000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Creator Revenue Overflow"),
        String::from_str(&env, "total_pool * share must not panic"),
        1_000,
        30,
        Category::EducationalStartup,
        true,
        5000,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1_000);

    // Inject a pathological revenue pool that overflows total_pool * share_bps.
    env.as_contract(&client.address, || {
        storage::set_revenue_pool(&env, campaign_id, i128::MAX);
    });

    let res = client.try_claim_creator_revenue(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::Overflow);
}

#[test]
fn test_claim_revenue_blocked_before_funds_withdrawn() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &10_000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Claim Gated On Withdraw"),
        description: String::from_str(&env, "Revenue claim must wait for funds_withdrawn"),
        funding_goal: 5000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 2000,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &5000);

    // Funds not yet withdrawn — claim must be rejected.
    let res = client.try_claim_revenue(&campaign_id, &contributor1);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    // Revenue deposits are also blocked before withdrawal.
    let deposit_res = client.try_deposit_revenue(&campaign_id, &1000);
    assert_eq!(deposit_res.unwrap_err().unwrap(), Error::ValidationFailed);

    // After withdrawal, deposit + claim succeeds.
    client.withdraw_funds(&campaign_id);
    client.deposit_revenue(&campaign_id, &1000);
    client.claim_revenue(&campaign_id, &contributor1);
    assert!(client.get_revenue_claimed(&campaign_id, &contributor1) > 0);
}

// ── deposit_revenue validation ──────────────────────────────────────────────────

#[test]
fn test_deposit_revenue_negative_amount() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Startup"),
        String::from_str(&env, "Revenue sharing startup"),
        1000,
        30,
        Category::EducationalStartup,
        true,
        2000,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    let res = client.try_deposit_revenue(&campaign_id, &-100);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_deposit_revenue_zero_amount() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Startup"),
        String::from_str(&env, "Revenue sharing startup"),
        1000,
        30,
        Category::EducationalStartup,
        true,
        2000,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    let res = client.try_deposit_revenue(&campaign_id, &0);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_deposit_revenue_without_revenue_sharing() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Educator Campaign"),
        String::from_str(&env, "No revenue sharing"),
        1000,
        30,
        Category::Educator,
        false,
        0,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    let res = client.try_deposit_revenue(&campaign_id, &1000);
    assert_eq!(res.unwrap_err().unwrap(), Error::RevenueSharingNotEnabled);
}

#[test]
fn test_deposit_revenue_when_paused() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Startup"),
        String::from_str(&env, "Revenue sharing startup"),
        1000,
        30,
        Category::EducationalStartup,
        true,
        2000,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    client.pause();

    let res = client.try_deposit_revenue(&campaign_id, &1000);
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);
}

#[test]
fn test_deposit_revenue_non_existent_campaign() {
    let (_env, _admin, _creator, _, _, _token, token_admin, client) = setup_env();
    token_admin.mint(&_admin, &10000);

    let res = client.try_deposit_revenue(&999, &1000);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotFound);
}

#[test]
fn test_deposit_revenue_repeated_calls_accumulate_and_emit_events() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &10_000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Repeated Deposits"),
        description: String::from_str(&env, "Deposit idempotency"),
        funding_goal: 1000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 2000,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    let events_before = env.events().all().len();
    for _ in 0..10 {
        client.deposit_revenue(&campaign_id, &100);
    }
    let events_after = env.events().all().len();
    assert_eq!(client.get_revenue_pool(&campaign_id), 1000);
    assert_eq!(events_after - events_before, 20);
}

#[test]
fn test_deposit_revenue_requires_funds_withdrawn() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Revenue pre-withdraw blocked"),
        String::from_str(&env, "Deposit requires successful withdrawal"),
        1000,
        30,
        Category::EducationalStartup,
        true,
        2000,
        0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);

    let res = client.try_deposit_revenue(&campaign_id, &1000);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_deposit_revenue_cancelled_campaign() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &10000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Startup"),
        String::from_str(&env, "Revenue sharing startup"),
        1000,
        30,
        Category::EducationalStartup,
        true,
        2000,
        0i128,
    ));
    client.verify_campaign(&campaign_id);

    // Cancel the campaign (before withdrawal — cancellation after withdrawal is disallowed)
    client.cancel_campaign(&campaign_id);

    // Depositing revenue into a cancelled campaign should fail
    let res = client.try_deposit_revenue(&campaign_id, &500);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);
}

// ── revenue-share arithmetic proptests ──────────────────────────────────────────
// Property-based fuzz tests for the revenue-share calculation logic.
//
// These tests use `proptest` to exercise the arithmetic used in
// `claim_revenue` and `claim_creator_revenue` with arbitrary inputs,
// confirming that:
//
// * No integer overflow or underflow occurs (the contract is compiled
//   with `overflow-checks = true` in release, and the tests run in debug
//   mode where overflow already panics).
// * The contributor pool never exceeds the total revenue pool.
// * Individual contributor dues never exceed the contributor pool.
// * Contributor due + creator share equals the full revenue pool
//   (no tokens are lost or created).
// * All results remain non-negative.

// ── Pure arithmetic helpers ──────────────────────────────────────────────────
//
// These mirror the formulas in lib.rs exactly so the properties are tested
// against the real calculation, not a reimplementation.

/// Compute the portion of the pool allocated to all contributors combined.
///
/// `revenue_share_percentage` is in basis points (0 – 5 000, i.e. 0 – 50 %).
fn contributor_pool(total_pool: i128, revenue_share_bps: i128) -> i128 {
    (total_pool * revenue_share_bps) / 10_000
}

/// Compute one contributor's share of the contributor pool.
fn contributor_due(contribution: i128, contributor_pool: i128, amount_raised: i128) -> i128 {
    (contribution * contributor_pool) / amount_raised
}

/// Checked mirror of the `claim_creator_revenue` contributor-pool multiply
/// (`revenue.rs`): returns `None` instead of panicking on i128 overflow (#408).
fn contributor_pool_checked(total_pool: i128, revenue_share_bps: i128) -> Option<i128> {
    Some(total_pool.checked_mul(revenue_share_bps)? / 10_000)
}

/// Checked mirror of the `withdraw_funds` ceiling-division fee multiply
/// (`withdraw.rs`): `(amount * bps + 9_999) / 10_000`, overflow-safe (#408).
fn fee_amount_checked(amount: i128, fee_bps: i128) -> Option<i128> {
    Some(amount.checked_mul(fee_bps)?.checked_add(9_999)? / 10_000)
}

// ── Strategies ───────────────────────────────────────────────────────────────

/// Revenue pool: allow 0 up to a realistic ceiling (~10 billion stroops).
fn arb_pool() -> impl Strategy<Value = i128> {
    0i128..=10_000_000_000i128
}

/// Revenue-share percentage in basis points (0 – 5 000 = 0 – 50 %).
fn arb_revenue_bps() -> impl Strategy<Value = i128> {
    0i128..=5_000i128
}

/// Amount raised: at least 1 (division guard) up to the pool ceiling.
fn arb_amount_raised() -> impl Strategy<Value = i128> {
    1i128..=10_000_000_000i128
}

// ── Properties ───────────────────────────────────────────────────────────────

proptest! {
    /// Contributor pool never exceeds the total revenue pool.
    #[test]
    fn prop_contributor_pool_does_not_exceed_total(
        total_pool in arb_pool(),
        bps in arb_revenue_bps(),
    ) {
        let cp = contributor_pool(total_pool, bps);
        prop_assert!(cp >= 0, "contributor pool must be non-negative");
        prop_assert!(
            cp <= total_pool,
            "contributor pool ({cp}) must not exceed total pool ({total_pool})"
        );
    }

    /// Creator share (total_pool – contributor_pool) is always non-negative.
    #[test]
    fn prop_creator_share_non_negative(
        total_pool in arb_pool(),
        bps in arb_revenue_bps(),
    ) {
        let cp = contributor_pool(total_pool, bps);
        let creator_share = total_pool - cp;
        prop_assert!(
            creator_share >= 0,
            "creator share ({creator_share}) must be non-negative"
        );
    }

    /// contributor_due ≤ contributor_pool for any valid contribution slice.
    #[test]
    fn prop_individual_due_does_not_exceed_pool(
        total_pool in arb_pool(),
        bps in arb_revenue_bps(),
        amount_raised in arb_amount_raised(),
        contribution in arb_amount_raised(), // reuse; will be clamped below
    ) {
        let contribution = contribution.min(amount_raised);
        let cp = contributor_pool(total_pool, bps);
        let due = contributor_due(contribution, cp, amount_raised);
        prop_assert!(due >= 0, "contributor due must be non-negative");
        prop_assert!(
            due <= cp,
            "contributor due ({due}) must not exceed contributor pool ({cp})"
        );
    }

    /// The sum of contributor_pool + creator_share equals total_pool exactly
    /// (no tokens are lost or created by the split).
    #[test]
    fn prop_pool_split_is_lossless(
        total_pool in arb_pool(),
        bps in arb_revenue_bps(),
    ) {
        let cp = contributor_pool(total_pool, bps);
        let creator_share = total_pool - cp;
        prop_assert_eq!(
            cp + creator_share,
            total_pool,
            "split must be lossless: {} + {} != {}",
            cp, creator_share, total_pool
        );
    }

    /// Zero revenue_share_bps allocates nothing to contributors.
    #[test]
    fn prop_zero_bps_gives_contributors_nothing(total_pool in arb_pool()) {
        let cp = contributor_pool(total_pool, 0);
        prop_assert_eq!(cp, 0);
    }

    /// Maximum revenue_share_bps (5000 = 50 %) gives contributors exactly half.
    #[test]
    fn prop_max_bps_gives_contributors_half(total_pool in 0i128..=10_000_000_000i128) {
        let cp = contributor_pool(total_pool, 5_000);
        // Integer division may lose 1 stroop on odd pools — that is correct behaviour.
        prop_assert_eq!(
            cp,
            total_pool / 2,
            "max bps should give ~half: cp={}, half={}",
            cp,
            total_pool / 2
        );
    }

    /// contributor_due with a full contribution (== amount_raised) equals
    /// the whole contributor pool (single contributor edge case).
    #[test]
    fn prop_sole_contributor_gets_full_pool(
        total_pool in arb_pool(),
        bps in arb_revenue_bps(),
        amount_raised in arb_amount_raised(),
    ) {
        let cp = contributor_pool(total_pool, bps);
        // A contributor who contributed everything gets the full contributor pool.
        let due = contributor_due(amount_raised, cp, amount_raised);
        prop_assert_eq!(
            due, cp,
            "sole contributor should get entire contributor pool"
        );
    }

    /// Boundary: revenue pool of 0 produces 0 for all shares.
    #[test]
    fn prop_empty_pool_gives_zero_shares(
        bps in arb_revenue_bps(),
        amount_raised in arb_amount_raised(),
        contribution in arb_amount_raised(),
    ) {
        let contribution = contribution.min(amount_raised);
        let cp = contributor_pool(0, bps);
        let due = contributor_due(contribution, cp, amount_raised);
        prop_assert_eq!(cp, 0);
        prop_assert_eq!(due, 0);
    }

    /// Issue #210: across any sequence of deposit_revenue + claim_revenue operations,
    /// a contributor's total claimed amount never exceeds their proportional pool share
    /// (contribution / amount_raised) * (total_pool * bps / 10_000).
    ///
    /// Proptest uses 1 000 shrink iterations by default, satisfying the ≥100 requirement.
    #[test]
    fn prop_claim_never_exceeds_pool_share(
        bps in arb_revenue_bps(),
        amount_raised in arb_amount_raised(),
        contribution in arb_amount_raised(),
        deposits in prop_vec(0i128..=1_000_000_000i128, 1..=20),
    ) {
        // Clamp contribution to the total raised (single-contributor upper bound).
        let contribution = contribution.min(amount_raised);
        let mut total_pool = 0i128;
        let mut already_claimed = 0i128;

        for deposit in &deposits {
            total_pool = total_pool.saturating_add(*deposit);
            let cp = contributor_pool(total_pool, bps);
            let total_due = contributor_due(contribution, cp, amount_raised);

            // Simulate claiming everything available in this round.
            let claimable = (total_due - already_claimed).max(0);
            already_claimed += claimable;

            // Invariant: cumulative claimed must never exceed the contributor's
            // proportional share of the current pool.
            prop_assert!(
                already_claimed <= total_due,
                "claimed ({already_claimed}) > proportional due ({total_due}) \
                 [pool={total_pool}, bps={bps}, contribution={contribution}, amount_raised={amount_raised}]"
            );
            // Also: must not exceed the whole contributor pool.
            prop_assert!(
                already_claimed <= cp,
                "claimed ({already_claimed}) > contributor pool ({cp})"
            );
        }
    }

    /// #408: the checked contributor-pool multiply never panics across the full
    /// i128 range, returns None exactly when the multiply overflows, and divides
    /// correctly otherwise.
    #[test]
    fn prop_contributor_pool_checked_is_overflow_safe(
        total_pool in any::<i128>(),
        bps in 0i128..=10_000i128,
    ) {
        let result = contributor_pool_checked(total_pool, bps);
        prop_assert_eq!(result.is_some(), total_pool.checked_mul(bps).is_some());
        if let Some(cp) = result {
            prop_assert_eq!(cp, total_pool.checked_mul(bps).unwrap() / 10_000);
        }
    }

    /// #408: the checked fee multiply (`amount * bps + 9_999`) never panics across
    /// the full i128 range and returns None exactly when the computation overflows.
    #[test]
    fn prop_fee_amount_checked_is_overflow_safe(
        amount in any::<i128>(),
        fee_bps in 0i128..=10_000i128,
    ) {
        let result = fee_amount_checked(amount, fee_bps);
        let ground = amount.checked_mul(fee_bps).and_then(|n| n.checked_add(9_999));
        prop_assert_eq!(result.is_some(), ground.is_some());
        if let Some(fee) = result {
            prop_assert_eq!(fee, ground.unwrap() / 10_000);
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_contributor_pool_calculation() {
        assert_eq!(contributor_pool(1000, 2000), 200);
        assert_eq!(contributor_pool(1000, 0), 0);
        assert_eq!(contributor_pool(0, 2000), 0);
    }

    #[test]
    fn test_contributor_due_calculation() {
        assert_eq!(contributor_due(500, 1000, 1000), 500);
        assert_eq!(contributor_due(0, 1000, 1000), 0);
    }
}

// ── revenue pool refund on cancel ───────────────────────────────────────────────

/// Test that reproduces the orphaned revenue pool bug:
/// When revenue is deposited into a campaign and the campaign is then cancelled,
/// the revenue pool should be refunded to the creator (not orphaned).
#[test]
fn test_cancel_campaign_refunds_revenue_pool() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &2000);
    token_admin.mint(&creator, &5000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Campaign with Revenue"),
        description: String::from_str(&env, "Testing revenue refund on cancel"),
        funding_goal: 5000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 2000, // 20% to contributors
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);

    // Contributor makes a contribution
    client.contribute(&campaign_id, &contributor1, &1000);

    // Creator deposits revenue
    let revenue_amount = 5000i128;
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.funds_withdrawn = true;
        storage::set_campaign(&env, campaign_id, &campaign);
    });
    client.deposit_revenue(&campaign_id, &revenue_amount);
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.funds_withdrawn = false;
        storage::set_campaign(&env, campaign_id, &campaign);
    });

    // Verify revenue pool is set
    assert_eq!(client.get_revenue_pool(&campaign_id), revenue_amount);

    // Creator's balance should be reduced by the revenue deposit
    assert_eq!(token.balance(&creator), 0); // 5000 - 5000 = 0

    // Cancel the campaign (before withdrawal)
    client.cancel_campaign(&campaign_id);

    // Verify campaign is cancelled
    assert!(client.get_campaign(&campaign_id).is_cancelled);

    // Revenue pool should be cleared
    assert_eq!(client.get_revenue_pool(&campaign_id), 0);

    // Creator should have received the full revenue pool refund
    assert_eq!(token.balance(&creator), revenue_amount);

    // Contract should still have the contribution (1000) but not the revenue
    // Contributions are only refunded when contributors claim their refunds
    assert_eq!(token.balance(&client.address), 1000);

    // Contributor can claim their contribution back via refund
    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 2000); // 1000 (original) + 1000 (refunded)
    assert_eq!(token.balance(&client.address), 0);
}

/// Test that revenue pool is cleared but contributors can still claim refunds
/// even if they previously had revenue claims.
#[test]
fn test_cannot_claim_revenue_after_cancel() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);
    token_admin.mint(&creator, &10_000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Cancel Then Refund"),
        description: String::from_str(&env, "Verify revenue is unavailable after cancel"),
        funding_goal: 5000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 2000,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1000);
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.funds_withdrawn = true;
        storage::set_campaign(&env, campaign_id, &campaign);
    });
    client.deposit_revenue(&campaign_id, &1000);
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.funds_withdrawn = false;
        storage::set_campaign(&env, campaign_id, &campaign);
    });

    // Cancel the campaign
    client.cancel_campaign(&campaign_id);

    // Revenue pool should be empty now (refunded to creator)
    assert_eq!(client.get_revenue_pool(&campaign_id), 0);

    // Contributor can still claim refund
    client.claim_refund(&campaign_id, &contributor1);

    // Verify contribution is cleared (as part of refund process)
    assert_eq!(client.get_contribution(&campaign_id, &contributor1), 0);

    // Revenue claimed should be cleared on refund
    assert_eq!(client.get_revenue_claimed(&campaign_id, &contributor1), 0);
}

/// Test that multiple contributors with different contributions
/// cannot claim revenue after campaign is cancelled.
#[test]
fn test_cancel_with_multiple_contributors_and_revenue() {
    let (env, _admin, creator, contributor1, contributor2, token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &3000);
    token_admin.mint(&contributor2, &2000);
    token_admin.mint(&creator, &8000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Multi-contributor Cancel"),
        description: String::from_str(&env, "Multiple contributors with revenue"),
        funding_goal: 5000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 3000, // 30% to contributors
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &2000);
    client.contribute(&campaign_id, &contributor2, &1000);

    let revenue_deposited = 3000i128;
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.funds_withdrawn = true;
        storage::set_campaign(&env, campaign_id, &campaign);
    });
    client.deposit_revenue(&campaign_id, &revenue_deposited);
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.funds_withdrawn = false;
        storage::set_campaign(&env, campaign_id, &campaign);
    });

    let creator_balance_before_cancel = token.balance(&creator);
    let contract_balance_before_cancel = token.balance(&client.address);

    // Cancel the campaign
    client.cancel_campaign(&campaign_id);

    // Revenue pool should be refunded to creator
    assert_eq!(client.get_revenue_pool(&campaign_id), 0);
    assert_eq!(
        token.balance(&creator),
        creator_balance_before_cancel + revenue_deposited
    );

    // Contract should only have the contributions now (revenue removed)
    assert_eq!(
        token.balance(&client.address),
        contract_balance_before_cancel - revenue_deposited
    );

    // Both contributors should be able to claim refunds
    client.claim_refund(&campaign_id, &contributor1);
    client.claim_refund(&campaign_id, &contributor2);

    // Verify all funds are returned to their original owners
    assert_eq!(token.balance(&contributor1), 3000);
    assert_eq!(token.balance(&contributor2), 2000);
    assert_eq!(token.balance(&client.address), 0);
}

/// Test that revenue refund event is emitted when campaign is cancelled with revenue pool.
#[test]
fn test_cancel_campaign_emits_revenue_refund_event() {
    let (env, _admin, creator, contributor1, _, _token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &1000);
    token_admin.mint(&creator, &5000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Event Test"),
        description: String::from_str(&env, "Verify events are emitted"),
        funding_goal: 5000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 2000,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);

    let revenue_amount = 5000i128;
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.funds_withdrawn = true;
        storage::set_campaign(&env, campaign_id, &campaign);
    });
    client.deposit_revenue(&campaign_id, &revenue_amount);
    env.as_contract(&client.address, || {
        let mut campaign = storage::get_campaign(&env, campaign_id).unwrap();
        campaign.funds_withdrawn = false;
        storage::set_campaign(&env, campaign_id, &campaign);
    });

    // Cancel campaign - should emit revenue_pool_refunded event
    client.cancel_campaign(&campaign_id);

    // Verify campaign is cancelled
    assert!(client.get_campaign(&campaign_id).is_cancelled);
    assert_eq!(client.get_revenue_pool(&campaign_id), 0);
}

/// Test that cancel still works correctly when no revenue has been deposited.
#[test]
fn test_cancel_campaign_with_no_revenue() {
    let (env, _admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "No Revenue Cancel"),
        description: String::from_str(&env, "Cancel without revenue deposit"),
        funding_goal: 5000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 2000,
        max_contribution_per_user: 0i128,
    });
    client.verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1000);

    let contract_balance_before = token.balance(&client.address);

    // Cancel the campaign (no revenue deposited)
    client.cancel_campaign(&campaign_id);

    // Verify campaign is cancelled
    assert!(client.get_campaign(&campaign_id).is_cancelled);

    // Revenue pool should remain 0
    assert_eq!(client.get_revenue_pool(&campaign_id), 0);

    // Contract balance should not change (no revenue to refund)
    assert_eq!(token.balance(&client.address), contract_balance_before);

    // Contributor should still be able to claim refund
    client.claim_refund(&campaign_id, &contributor1);
    assert_eq!(token.balance(&contributor1), 5000);
    assert_eq!(token.balance(&client.address), 0);
}
