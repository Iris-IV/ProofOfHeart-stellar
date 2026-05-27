// Tests for issue #217: benchmark tests asserting per-call instruction budgets.
// Uses env.budget() to assert contribute/withdraw_funds/claim_revenue
// stay below documented CPU instruction thresholds.
use super::*;
use crate::test::setup_env;
use soroban_sdk::String;

// Conservative thresholds (native Rust underestimates vs WASM; real limits are 100M per tx).
// These catch regressions while remaining well below mainnet limits.
const CONTRIBUTE_CPU_LIMIT: u64 = 5_000_000;
const WITHDRAW_CPU_LIMIT: u64 = 5_000_000;
const CLAIM_REVENUE_CPU_LIMIT: u64 = 5_000_000;
const GET_CAMPAIGNS_BY_CATEGORY_CPU_LIMIT: u64 = 10_000_000;

fn make_revenue_campaign(
    env: &soroban_sdk::Env,
    creator: soroban_sdk::Address,
) -> CreateCampaignParams {
    CreateCampaignParams {
        creator,
        title: String::from_str(env, "Benchmark Campaign"),
        description: String::from_str(env, "Budget regression test"),
        funding_goal: 1_000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage: 1000,
        max_contribution_per_user: 0,
    }
}

/// contribute() must stay below the CPU instruction threshold.
#[test]
fn test_contribute_instruction_budget() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &10_000);

    let id = client.create_campaign(&make_revenue_campaign(&env, creator.clone()));
    client.verify_campaign(&id);

    // Reset budget immediately before the call under test
    env.budget().reset_default();
    client.contribute(&id, &contributor1, &500);

    let cpu = env.budget().cpu_instruction_cost();
    assert!(
        cpu < CONTRIBUTE_CPU_LIMIT,
        "contribute() used {} CPU instructions, limit is {}",
        cpu,
        CONTRIBUTE_CPU_LIMIT
    );
}

/// withdraw_funds() must stay below the CPU instruction threshold.
#[test]
fn test_withdraw_funds_instruction_budget() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &10_000);

    let id = client.create_campaign(&make_revenue_campaign(&env, creator.clone()));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &1_000);

    env.budget().reset_default();
    client.withdraw_funds(&id);

    let cpu = env.budget().cpu_instruction_cost();
    assert!(
        cpu < WITHDRAW_CPU_LIMIT,
        "withdraw_funds() used {} CPU instructions, limit is {}",
        cpu,
        WITHDRAW_CPU_LIMIT
    );
}

/// claim_revenue() must stay below the CPU instruction threshold.
#[test]
fn test_claim_revenue_instruction_budget() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &10_000);
    token_admin.mint(&creator, &5_000);

    let id = client.create_campaign(&make_revenue_campaign(&env, creator.clone()));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &1_000);
    client.deposit_revenue(&id, &2_000);

    env.budget().reset_default();
    client.claim_revenue(&id, &contributor1);

    let cpu = env.budget().cpu_instruction_cost();
    assert!(
        cpu < CLAIM_REVENUE_CPU_LIMIT,
        "claim_revenue() used {} CPU instructions, limit is {}",
        cpu,
        CLAIM_REVENUE_CPU_LIMIT
    );
}

#[test]
fn test_get_campaigns_by_category_bucketed_pagination_budget() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    for i in 0..601u32 {
        let params = CreateCampaignParams {
            creator: creator.clone(),
            title: String::from_str(&env, "Campaign"),
            description: String::from_str(&env, "Benchmark campaign"),
            funding_goal: 1_000,
            duration_days: 30,
            category: Category::Learner,
            has_revenue_sharing: false,
            revenue_share_percentage: 0,
            max_contribution_per_user: 0,
        };
        client.create_campaign(&params);
    }

    env.budget().reset_default();
    let campaigns = client.get_campaigns_by_category(&Category::Learner, &498, &10);

    let cpu = env.budget().cpu_instruction_cost();
    assert!(
        cpu < GET_CAMPAIGNS_BY_CATEGORY_CPU_LIMIT,
        "get_campaigns_by_category() used {} CPU instructions, limit is {}",
        cpu,
        GET_CAMPAIGNS_BY_CATEGORY_CPU_LIMIT
    );

    assert_eq!(campaigns.len(), 10);
    assert_eq!(campaigns.get(0).unwrap().id, 499);
    assert_eq!(campaigns.get(9).unwrap().id, 508);
}
