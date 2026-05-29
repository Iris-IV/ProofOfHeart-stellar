// Tests for issue #214: storage cleanup on refund/cancel/withdraw
// Probes storage after terminal actions to assert absence of orphan entries.
use super::*;
use crate::test::setup_env;
use soroban_sdk::{testutils::Address as _, testutils::Ledger, Address, Env, String};

fn make_params_local(
    creator: Address,
    env: &Env,
    goal: i128,
    has_revenue: bool,
) -> CreateCampaignParams {
    CreateCampaignParams {
        creator,
        title: String::from_str(env, "Test Campaign"),
        description: String::from_str(env, "Description"),
        funding_goal: goal,
        duration_days: 30,
        category: if has_revenue {
            Category::EducationalStartup
        } else {
            Category::Learner
        },
        has_revenue_sharing: has_revenue,
        revenue_share_percentage: if has_revenue { 1000 } else { 0 },
        max_contribution_per_user: 0,
    }
}

fn has_persistent_key(env: &Env, client: &ProofOfHeartClient<'_>, key: DataKey) -> bool {
    env.as_contract(&client.address, || env.storage().persistent().has(&key))
}

/// After claim_refund, Contribution and RevenueClaimed keys must be absent.
#[test]
fn test_storage_cleaned_after_claim_refund_on_cancel() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &5000);

    let id = client.create_campaign(&make_params_local(creator.clone(), &env, 10_000, false));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &1000);

    client.cancel_campaign(&id);
    client.claim_refund(&id, &contributor1);

    // Contribution key must be gone
    assert!(
        !has_persistent_key(
            &env,
            &client,
            DataKey::Contribution(id, contributor1.clone())
        ),
        "Contribution key must be removed after refund"
    );
    // RevenueClaimed key must be gone (was never set, but remove is idempotent)
    assert!(
        !has_persistent_key(
            &env,
            &client,
            DataKey::RevenueClaimed(id, contributor1.clone())
        ),
        "RevenueClaimed key must not exist after refund"
    );
}

/// After claim_refund on a failed campaign (goal not met), same cleanup applies.
#[test]
fn test_storage_cleaned_after_claim_refund_on_failed_campaign() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &5000);

    let id = client.create_campaign(&make_params_local(creator.clone(), &env, 10_000, false));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &500);

    // Advance time past deadline so campaign fails
    env.ledger().with_mut(|li| {
        li.timestamp += 31 * 86400;
    });

    client.claim_refund(&id, &contributor1);

    assert!(
        !has_persistent_key(
            &env,
            &client,
            DataKey::Contribution(id, contributor1.clone())
        ),
        "Contribution key must be removed after refund on failed campaign"
    );
}

/// After withdraw_funds, campaign is inactive and funds_withdrawn is true.
/// Contribution keys for contributors remain (they are not cleaned on withdraw).
#[test]
fn test_storage_state_after_withdraw_funds() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &10_000);

    let id = client.create_campaign(&make_params_local(creator.clone(), &env, 1_000, false));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &1_000);
    client.withdraw_funds(&id);

    let campaign = client.get_campaign(&id);
    assert!(campaign.funds_withdrawn, "funds_withdrawn must be true");
    assert!(
        !campaign.is_active,
        "campaign must be inactive after withdraw"
    );
}

/// After cancel_campaign, voting state keys are absent.
#[test]
fn test_voting_keys_absent_after_cancel() {
    let (env, _admin, creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();

    let id = client.create_campaign(&make_params_local(creator.clone(), &env, 10_000, false));
    client.cancel_campaign(&id);

    // Aggregate voting keys should not exist for a campaign that was never voted on
    assert!(
        !has_persistent_key(&env, &client, DataKey::ApproveVotes(id)),
        "ApproveVotes must not exist"
    );
    assert!(
        !has_persistent_key(&env, &client, DataKey::RejectVotes(id)),
        "RejectVotes must not exist"
    );
}

/// Issue #380: after cancel_campaign, aggregate voting keys are purged even when votes were cast.
#[test]
fn test_voting_keys_purged_after_cancel_with_prior_votes() {
    let (env, _admin, creator, _contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    let voter = Address::generate(&env);
    token_admin.mint(&voter, &500);

    let id = client.create_campaign(&make_params_local(creator.clone(), &env, 10_000, false));

    // Cast a vote so the aggregate storage keys are written
    client.vote_on_campaign(&id, &voter, &true);

    assert!(
        has_persistent_key(&env, &client, DataKey::ApproveVotes(id)),
        "ApproveVotes must exist before cancel"
    );

    client.cancel_campaign(&id);

    assert!(
        !has_persistent_key(&env, &client, DataKey::ApproveVotes(id)),
        "ApproveVotes must be purged after cancel"
    );
    assert!(
        !has_persistent_key(&env, &client, DataKey::RejectVotes(id)),
        "RejectVotes must be purged after cancel"
    );
    assert!(
        !has_persistent_key(&env, &client, DataKey::ApproveWeight(id)),
        "ApproveWeight must be purged after cancel"
    );
    assert!(
        !has_persistent_key(&env, &client, DataKey::RejectWeight(id)),
        "RejectWeight must be purged after cancel"
    );
}

// Issue #341: claim_revenue is gated on funds_withdrawn, and cancel is gated
// on !funds_withdrawn, so the prior "claim → cancel → refund cleans
// RevenueClaimed" path is no longer reachable. The defensive cleanup remains
// in claim_refund; behavioural coverage of the new guard lives in
// test::test_claim_revenue_blocked_before_funds_withdrawn.
