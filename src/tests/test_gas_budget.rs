//! Gas-budget regression gate (#1331).
//!
//! Each test measures one hot-path entrypoint with the Soroban host budget
//! (CPU instructions and memory bytes) and fails if either exceeds its
//! threshold in [`thresholds`]. CI runs exactly these tests in the
//! `gas-budget` job via `cargo test --features testutils -- gas_budget`, so
//! every test here must keep `gas_budget` in its name.
//!
//! See docs/gas.md ("Gas-budget CI gate") for how to re-measure and update
//! the thresholds.
extern crate std;

use super::helpers::*;
use soroban_sdk::{testutils::Ledger, Env};

/// The single source of truth for the CI gas-budget thresholds.
///
/// Each limit is the cost measured when it was last set (noted beside it,
/// soroban-sdk 22.0.11) plus roughly 20% headroom, rounded up. A limit is an upper bound: going over it fails CI.
/// Raise one only alongside the change that justifies the extra cost, and
/// state the new measurement in the PR.
pub(crate) mod thresholds {
    pub const CREATE_CAMPAIGN_CPU: u64 = 750_000; // measured 625_009
    pub const CREATE_CAMPAIGN_MEM: u64 = 119_000; // measured 98_725
    pub const CONTRIBUTE_CPU: u64 = 930_000; // measured 770_385
    pub const CONTRIBUTE_MEM: u64 = 182_000; // measured 151_157
    pub const WITHDRAW_FUNDS_CPU: u64 = 880_000; // measured 733_479
    pub const WITHDRAW_FUNDS_MEM: u64 = 162_000; // measured 134_796
    pub const CLAIM_REFUND_CPU: u64 = 520_000; // measured 428_877
    pub const CLAIM_REFUND_MEM: u64 = 92_000; // measured 76_298
}

/// Starts a fresh measurement window. Setup done before this call is not
/// counted.
fn start_measuring(env: &Env) {
    env.cost_estimate().budget().reset_default();
}

/// Asserts the CPU and memory consumed since [`start_measuring`] stay within
/// the given limits, and prints the measurement so limits can be re-tuned
/// (`cargo test --features testutils -- gas_budget --nocapture`).
fn assert_within_budget(env: &Env, entrypoint: &str, cpu_limit: u64, mem_limit: u64) {
    let budget = env.cost_estimate().budget();
    let cpu = budget.cpu_instruction_cost();
    let mem = budget.memory_bytes_cost();
    std::println!(
        "gas_budget {entrypoint}: cpu={cpu} (limit {cpu_limit}), mem={mem} (limit {mem_limit})"
    );
    assert!(
        cpu <= cpu_limit,
        "{entrypoint} used {cpu} CPU instructions, budget is {cpu_limit}"
    );
    assert!(
        mem <= mem_limit,
        "{entrypoint} used {mem} memory bytes, budget is {mem_limit}"
    );
}

fn campaign(env: &Env, creator: &soroban_sdk::Address, title: &str) -> CreateCampaignParams {
    make_params(
        creator.clone(),
        String::from_str(env, title),
        String::from_str(env, "Gas budget regression test"),
        1_000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    )
}

#[test]
fn gas_budget_create_campaign() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    start_measuring(&env);
    let id = client.create_campaign(&campaign(&env, &creator, "Created"));

    assert_within_budget(
        &env,
        "create_campaign",
        thresholds::CREATE_CAMPAIGN_CPU,
        thresholds::CREATE_CAMPAIGN_MEM,
    );
    assert_eq!(id, 1);
}

#[test]
fn gas_budget_contribute() {
    let (env, _admin, creator, contributor, _c2, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &10_000);
    let id = client.create_campaign(&campaign(&env, &creator, "Contributed"));
    client.verify_campaign(&id);

    start_measuring(&env);
    client.contribute(&id, &contributor, &500);

    assert_within_budget(
        &env,
        "contribute",
        thresholds::CONTRIBUTE_CPU,
        thresholds::CONTRIBUTE_MEM,
    );
    assert_eq!(token.balance(&client.address), 500);
}

#[test]
fn gas_budget_withdraw_funds() {
    let (env, _admin, creator, contributor, _c2, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &10_000);
    let id = client.create_campaign(&campaign(&env, &creator, "Withdrawn"));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor, &1_000);
    // `withdraw_funds` is only allowed once the funding window has closed.
    let deadline = client.get_campaign(&id).deadline;
    env.ledger().with_mut(|l| l.timestamp = deadline + 1);

    start_measuring(&env);
    client.withdraw_funds(&id);

    assert_within_budget(
        &env,
        "withdraw_funds",
        thresholds::WITHDRAW_FUNDS_CPU,
        thresholds::WITHDRAW_FUNDS_MEM,
    );
    assert!(token.balance(&creator) > 0);
}

#[test]
fn gas_budget_claim_refund() {
    let (env, _admin, creator, contributor, _c2, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor, &10_000);
    let id = client.create_campaign(&campaign(&env, &creator, "Refunded"));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor, &400);
    client.cancel_campaign(&id);
    let before = token.balance(&contributor);

    start_measuring(&env);
    client.claim_refund(&id, &contributor);

    assert_within_budget(
        &env,
        "claim_refund",
        thresholds::CLAIM_REFUND_CPU,
        thresholds::CLAIM_REFUND_MEM,
    );
    assert_eq!(token.balance(&contributor) - before, 400);
}
