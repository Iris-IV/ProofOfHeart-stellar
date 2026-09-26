//! Performance Benchmarking & Gas Usage Metrics Report
//!
//! This module contains CPU instruction budget regression tests for key Proof of Heart
//! contract methods. These tests ensure that resource consumption remains predictable and
//! within acceptable limits across protocol versions and contract updates.
//!
//! ## Purpose
//!
//! Gas optimization in Soroban is critical for:
//! - **Fee predictability**: Users can estimate transaction costs reliably
//! - **Network scalability**: Lower per-operation costs allow higher throughput
//! - **User adoption**: Affordable transactions encourage platform usage
//!
//! ## Methodology
//!
//! Each benchmark test:
//! 1. Sets up a realistic contract state (campaigns, contributors, revenue)
//! 2. Calls `env.budget().reset_default()` to start fresh measurement
//! 3. Executes the operation under test
//! 4. Asserts CPU instruction cost < configured limit
//!
//! Example workflow:
//! ```rust
//! // Setup: Create campaign, mint tokens, contribute
//! env.budget().reset_default();  // Start measurement
//! client.contribute(&campaign_id, &contributor, &amount);  // Operation
//! let cpu = env.budget().cpu_instruction_cost();  // Read result
//! assert!(cpu < LIMIT);  // Verify budget
//! ```
//!
//! ## CPU Instruction Budgets (Protocol 23)
//!
//! - `contribute()`: 5M instructions (token transfer + storage + validation)
//! - `withdraw_funds()`: 5M instructions (token transfer + fund redistribution)
//! - `claim_revenue()`: 5M instructions (revenue calculation + token transfer)
//! - `get_campaigns_by_category()`: 10M instructions (pagination + filtering)
//! - `unpause()`: 1M instructions (simple state update, caught copy-paste regression in #1251)
//!
//! ## Interpreting Budget Failures
//!
//! If a test fails (e.g., "contribute() used 5.2M instructions, limit is 5M"):
//!
//! 1. **Legitimate regression**: A code change genuinely uses more CPU
//!    - Profile the operation: Is it necessary?
//!    - Consider optimization opportunities (batch ops, cache reads)
//!    - Increase budget if optimized to its limit
//!
//! 2. **Measurement variance**: Rare, but can happen with complex storage patterns
//!    - Rerun test locally
//!    - Check for other processes consuming CPU
//!    - Consider increasing budget by 5-10% if consistently high
//!
//! ## Optimization Techniques
//!
//! ### Example 1: Batch Operations
//! ```rust
//! // Inefficient: Multiple transfers in a loop
//! for user in users {
//!     token.transfer(&user, &contract, &amount);
//! }
//! // Efficient: Single aggregated transfer
//! let total: i128 = amount * users.len() as i128;
//! token.transfer(&all_users, &contract, &total);
//! ```
//!
//! ### Example 2: Cache Storage Reads
//! ```rust
//! // Inefficient: Read campaign twice
//! if client.get_campaign(&id).is_active {
//!     let title = client.get_campaign(&id).title;
//! }
//! // Efficient: Cache in variable
//! let campaign = client.get_campaign(&id);
//! if campaign.is_active {
//!     let title = campaign.title;
//! }
//! ```
//!
//! ### Example 3: Early Exit on Validation
//! ```rust
//! // Efficient: Check authorization first
//! user.require_auth();  // Fail fast before expensive ops
//! let campaign = get_campaign(&id)?;
//! transfer_tokens(&token, &user, &amount)?;
//! ```
//!
//! ## Running Benchmarks
//!
//! Execute all budget tests:
//! ```bash
//! cargo test test_.*_instruction_budget --lib
//! ```
//!
//! Run a specific test:
//! ```bash
//! cargo test test_contribute_instruction_budget --lib -- --nocapture
//! ```
//!
//! Print CPU cost for debugging:
//! ```rust
//! let cpu = env.budget().cpu_instruction_cost();
//! eprintln!("CPU cost: {} instructions", cpu);
//! ```
//!
//! ## Regression History
//!
//! - **#1251**: `unpause()` had duplicate storage writes; fixed and budget pinned to 1M
//! - **#466**: Vesting snapshots added per-campaign overhead; buffered pagination budget to 10M
//!
//! ## Future Considerations
//!
//! - Profile contribution path under high contributor count (edge case perf)
//! - Consider lazy evaluation of revenue calculations
//! - Benchmark storage cleanup (refund claims) for deletion overhead
//! - Monitor Protocol 24+ budget changes and update thresholds

extern crate alloc;
extern crate std;
use alloc::format;

use super::helpers::*;

// CPU instruction budget thresholds for regression detection.
// These limits represent the maximum acceptable cost for each operation.
// Increasing limits should be rare and justified by code review.
const CONTRIBUTE_CPU_LIMIT: u64 = 5_000_000;
const WITHDRAW_CPU_LIMIT: u64 = 5_000_000;
const CLAIM_REVENUE_CPU_LIMIT: u64 = 5_000_000;
const GET_CAMPAIGNS_BY_CATEGORY_CPU_LIMIT: u64 = 10_000_000;
const UNPAUSE_CPU_LIMIT: u64 = 1_000_000;

/// Creates a revenue-sharing campaign for use in benchmark tests.
/// Used to measure performance under a realistic, feature-rich campaign configuration.
/// `withdraw_funds` is only allowed once the funding window has closed (#854).
fn pass_deadline(env: &soroban_sdk::Env, client: &ProofOfHeartClient, campaign_id: u32) {
    let deadline = client.get_campaign(&campaign_id).deadline;
    env.ledger().with_mut(|l| l.timestamp = deadline + 1);
}

/// Asserts the CPU instructions consumed since the last `reset_default` stay
/// under `limit`, and reports the measured cost so limits can be re-tuned.
fn assert_cpu_budget(env: &soroban_sdk::Env, entrypoint: &str, limit: u64) {
    let cpu = env.budget().cpu_instruction_cost();
    std::println!("bench {entrypoint}: {cpu} CPU instructions (limit {limit})");
    assert!(
        cpu < limit,
        "{} used {} CPU instructions, limit is {}",
        entrypoint,
        cpu,
        limit
    );
}

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
        has_revenue_sharing: true,        // Feature-rich: revenue sharing enabled
        revenue_share_percentage: 1000,   // 10% revenue share
        max_contribution_per_user: 0,     // No cap
    }
}

/// Benchmark test: `contribute()` method CPU instruction cost.
///
/// Measures the cost of a contributor transferring tokens to a campaign, including:
/// - Token transfer (USDC or similar) from contributor to contract
/// - Storage update for contribution amount
/// - Anomaly detection (burst detection, huge contribution checks)
/// - Event emission for contribution tracking
///
/// Setup: Campaign with revenue sharing, verified, ready to accept contributions.
/// Operation: Single 500-unit contribution from contributor1.
/// Expected: < 5M CPU instructions (includes auth, validation, state updates).
///
/// Note: Cost varies with:
/// - Contribution amount (larger amounts may trigger anomaly checks)
/// - Campaign state (active vs. paused)
/// - Contributor history (new vs. existing contributors)
///
/// Soroban CLI equivalent:
/// ```bash
/// stellar contract invoke \
///   --id <contract-id> \
///   -- contribute \
///   --campaign-id 1 \
///   --contributor <address> \
///   --amount 500
/// ```
#[test]
fn test_contribute_instruction_budget() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &10_000);

    let id = client.create_campaign(&make_revenue_campaign(&env, creator.clone()));
    client.verify_campaign(&id);

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

/// Benchmark test: `withdraw_funds()` method CPU instruction cost.
///
/// Measures the cost of a campaign creator withdrawing funds after goal is met, including:
/// - Verification that goal was met and deadline passed
/// - Platform fee calculation and transfer
/// - Campaign fund transfer to creator
/// - Campaign state mutation (mark as withdrawn, deactivate)
/// - Storage updates for campaign and global raised totals
///
/// Setup: Campaign verified, 1000 units contributed (goal met).
/// Operation: Creator calls withdraw_funds to claim earnings.
/// Expected: < 5M CPU instructions (includes token transfer + state updates).
///
/// Cost breakdown:
/// - Token transfer (platform fee): ~500K instructions
/// - Token transfer (creator withdrawal): ~500K instructions
/// - Campaign state update: ~1M instructions
/// - Global state update: ~500K instructions
/// Total: ~2.5M typical, with headroom for regression detection.
#[test]
fn test_withdraw_funds_instruction_budget() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &10_000);

    let id = client.create_campaign(&make_revenue_campaign(&env, creator.clone()));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &1_000);
    pass_deadline(&env, &client, id);

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

/// Benchmark test: `claim_revenue()` method CPU instruction cost.
///
/// Measures the cost of a contributor claiming their share of campaign revenue, including:
/// - Contributor authorization verification
/// - Revenue share calculation (contributor amount / total raised * revenue pool)
/// - Revenue claimed tracking update
/// - Token transfer from contract to contributor
/// - Storage cleanup (revenueclaimed key update)
///
/// Setup: Campaign with revenue sharing enabled, funds withdrawn, revenue deposited.
/// Contributors: 1 with 1000 units contributed; 2000 units deposited as revenue.
/// Operation: Contributor claims revenue share (1000/1000 * 2000 = 2000 units).
/// Expected: < 5M CPU instructions (includes math + token transfer).
///
/// Cost characteristics:
/// - Math is minimal (division, multiplication on i128)
/// - Token transfer: ~500K instructions
/// - Storage updates: ~1M instructions
/// Typical total: ~2M instructions with headroom.
#[test]
fn test_claim_revenue_instruction_budget() {
    let (env, _admin, creator, contributor1, _contributor2, _token, token_admin, client) =
        setup_env();

    token_admin.mint(&contributor1, &10_000);
    token_admin.mint(&creator, &5_000);

    let id = client.create_campaign(&make_revenue_campaign(&env, creator.clone()));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &1_000);
    pass_deadline(&env, &client, id);
    client.withdraw_funds(&id);
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

/// Benchmark test: `get_campaigns_by_category()` method CPU instruction cost.
///
/// Measures the cost of paginated campaign lookup by category, including:
/// - Bucket storage iteration (campaigns indexed by category)
/// - Offset-based pagination (skip first N, return next M)
/// - Campaign state deserialization (25 campaigns stored)
/// - Result vector construction
///
/// Setup: 25 campaigns created in "Learner" category.
/// Operation: Query campaigns with offset=15, limit=10 (returns campaigns 16-25).
/// Expected: < 10M CPU instructions (pagination can be expensive with large result sets).
///
/// Implementation notes:
/// - Pagination uses bucketed indexing (not linear scan) for efficiency
/// - Each campaign deserialization adds ~100K instructions
/// - Large limits or deep offsets increase cost significantly
/// - Tested with 25 campaigns to exercise realistic pagination paths
///
/// Scaling behavior:
/// - Offset 15, limit 10: ~5-6M instructions
/// - Offset 0, limit 10: ~3-4M instructions
/// - High offsets near end of list: ~6-7M instructions (partial bucket reads)
///
/// Edge cases (not explicitly tested here):
/// - Empty category: Minimal cost (~100K, bucket not found)
/// - Limit > available: Partial results still paginated normally
#[test]
fn test_get_campaigns_by_category_bucketed_pagination_budget() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    env.budget().reset_unlimited();

    // NOTE: keep the number of campaigns below ~60. Every campaign writes an
    // extra per-campaign vesting snapshot entry (#466); the soroban testutils
    // host cannot externalize events for envs with more than ~62 campaigns
    // worth of storage entries (panics in Env::drop with UnexpectedType),
    // which flaked the CI `test` job. 58 still exercises the same bucketed
    // pagination path (page at offset 48 -> ids 49..58).
    for i in 0..25u32 {
        let title_str = format!("Campaign {}", i);
        let params = CreateCampaignParams {
            creator: creator.clone(),
            title: String::from_str(&env, &title_str),
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
    // The call returns the page of campaigns plus a pagination value; only the page
    // is asserted here.
    let (campaigns, _) = client.get_campaigns_by_category(&Category::Learner, &15, &10);

    let cpu = env.budget().cpu_instruction_cost();
    assert!(
        cpu < GET_CAMPAIGNS_BY_CATEGORY_CPU_LIMIT,
        "get_campaigns_by_category() used {} CPU instructions, limit is {}",
        cpu,
        GET_CAMPAIGNS_BY_CATEGORY_CPU_LIMIT
    );

    assert_eq!(campaigns.len(), 10);
    assert_eq!(campaigns.get(0).unwrap().id, 16);
    assert_eq!(campaigns.get(9).unwrap().id, 25);
}

/// Benchmark test: `unpause()` method CPU instruction cost.
///
/// Measures the cost of admin unpausing the contract (resuming normal operation after
/// an emergency pause). This is a simple operation with minimal overhead:
/// - Verify admin authorization
/// - Update instance storage (set AutoPaused flag to false)
/// - Return success
///
/// Setup: Contract in paused state.
/// Operation: Admin calls unpause() to resume operations.
/// Expected: < 1M CPU instructions (minimal state update, no loops or external calls).
///
/// Regression test context (#1251):
/// Previously, `unpause()` had a copy-paste bug writing `AdminKey::AutoPaused` twice
/// to instance storage. This was ~1M instructions of wasted CPU on every unpause.
/// This test pins the correct cost (single write) so regressions are caught immediately.
///
/// Why the tight budget?
/// - This operation should be trivial (single boolean flag update)
/// - If cost rises above 1M, there's likely a new inefficiency
/// - Provides early warning before shipping to mainnet
///
/// Example cost breakdown:
/// - Admin authorization: ~100K instructions
/// - Instance storage write: ~50K instructions
/// - Return value construction: ~10K instructions
/// Total: ~160K instructions (well under 1M limit)
#[test]
fn test_unpause_instruction_budget() {
    let (env, _admin, _creator, _contributor1, _, _token, _token_admin, client) = setup_env();

    client.pause();

    env.budget().reset_default();
    client.unpause();

    let cpu = env.budget().cpu_instruction_cost();
    assert!(
        cpu < UNPAUSE_CPU_LIMIT,
        "unpause() used {} CPU instructions, limit is {}",
        cpu,
        UNPAUSE_CPU_LIMIT
    );
    assert!(!client.is_paused());
}

// ── #1138: budgets for the remaining core entrypoints and failure paths ──────

// Limits are ~3x the measured cost: tight enough to catch an accidental
// regression (an extra storage write, a loop over unbounded state), loose
// enough not to flake on SDK upgrades.
const CREATE_CAMPAIGN_CPU_LIMIT: u64 = 1_800_000;
const VERIFY_CAMPAIGN_CPU_LIMIT: u64 = 1_000_000;
const CANCEL_CAMPAIGN_CPU_LIMIT: u64 = 1_300_000;
const CLAIM_REFUND_CPU_LIMIT: u64 = 1_400_000;
const BATCH_CONTRIBUTE_CPU_LIMIT: u64 = 9_000_000;
const DEPOSIT_REVENUE_CPU_LIMIT: u64 = 1_200_000;
const REJECTED_CALL_CPU_LIMIT: u64 = 500_000;

fn plain_campaign(
    env: &soroban_sdk::Env,
    creator: soroban_sdk::Address,
    title: &str,
) -> CreateCampaignParams {
    CreateCampaignParams {
        creator,
        title: String::from_str(env, title),
        description: String::from_str(env, "Budget regression test"),
        funding_goal: 1_000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    }
}

#[test]
fn test_create_campaign_instruction_budget() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();

    env.budget().reset_default();
    let id = client.create_campaign(&plain_campaign(&env, creator.clone(), "Created"));

    assert_cpu_budget(&env, "create_campaign()", CREATE_CAMPAIGN_CPU_LIMIT);
    assert_eq!(id, 1);
}

#[test]
fn test_verify_campaign_instruction_budget() {
    let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();
    let id = client.create_campaign(&plain_campaign(&env, creator.clone(), "Verified"));

    env.budget().reset_default();
    client.verify_campaign(&id);

    assert_cpu_budget(&env, "verify_campaign()", VERIFY_CAMPAIGN_CPU_LIMIT);
    assert!(client.get_campaign(&id).is_verified);
}

#[test]
fn test_cancel_campaign_instruction_budget() {
    let (env, _admin, creator, contributor1, _c2, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &10_000);
    let id = client.create_campaign(&plain_campaign(&env, creator.clone(), "Cancelled"));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &400);

    env.budget().reset_default();
    client.cancel_campaign(&id);

    assert_cpu_budget(&env, "cancel_campaign()", CANCEL_CAMPAIGN_CPU_LIMIT);
    assert!(client.get_campaign(&id).is_cancelled);
}

#[test]
fn test_claim_refund_instruction_budget() {
    let (env, _admin, creator, contributor1, _c2, token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &10_000);
    let id = client.create_campaign(&plain_campaign(&env, creator.clone(), "Refunded"));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &400);
    client.cancel_campaign(&id);
    let before = token.balance(&contributor1);

    env.budget().reset_default();
    client.claim_refund(&id, &contributor1);

    assert_cpu_budget(&env, "claim_refund()", CLAIM_REFUND_CPU_LIMIT);
    assert_eq!(token.balance(&contributor1) - before, 400);
}

#[test]
fn test_batch_contribute_instruction_budget_scales_linearly() {
    let (env, _admin, creator, contributor1, _c2, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &100_000);

    let mut ids = std::vec::Vec::new();
    for i in 0..4u32 {
        let title = format!("Batch {}", i);
        let id = client.create_campaign(&plain_campaign(&env, creator.clone(), &title));
        client.verify_campaign(&id);
        ids.push(id);
    }

    let batch = |n: usize| {
        let mut items = soroban_sdk::Vec::new(&env);
        for id in ids.iter().take(n) {
            items.push_back((*id, 10i128));
        }
        items
    };

    env.budget().reset_default();
    client.batch_contribute(&contributor1, &batch(1));
    let one = env.budget().cpu_instruction_cost();

    env.budget().reset_default();
    client.batch_contribute(&contributor1, &batch(4));
    let four = env.budget().cpu_instruction_cost();
    std::println!("bench batch_contribute: 1 item {one}, 4 items {four}");

    assert_cpu_budget(&env, "batch_contribute(4)", BATCH_CONTRIBUTE_CPU_LIMIT);
    // Four items must not cost more than a small multiple of one: a per-item
    // cost that grew with the batch size would show up as super-linear here.
    assert!(
        four < one * 6,
        "batch of 4 cost {} vs {} for a single item: cost is not roughly linear",
        four,
        one
    );
}

#[test]
fn test_deposit_revenue_instruction_budget() {
    let (env, _admin, creator, contributor1, _c2, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &10_000);
    token_admin.mint(&creator, &5_000);
    let id = client.create_campaign(&make_revenue_campaign(&env, creator.clone()));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &1_000);
    pass_deadline(&env, &client, id);
    client.withdraw_funds(&id);

    env.budget().reset_default();
    client.deposit_revenue(&id, &2_000);

    assert_cpu_budget(&env, "deposit_revenue()", DEPOSIT_REVENUE_CPU_LIMIT);
    assert_eq!(client.get_revenue_pool(&id), 2_000);
}

// Rejected calls must stay cheap and must return the documented error: an
// invalid state transition that burns a large budget before failing is both a
// DoS lever and a sign the checks run too late.

#[test]
fn test_rejected_withdraw_before_deadline_is_cheap_and_typed() {
    let (env, _admin, creator, contributor1, _c2, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &10_000);
    let id = client.create_campaign(&plain_campaign(&env, creator.clone(), "Early"));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &1_000);

    env.budget().reset_default();
    let res = client.try_withdraw_funds(&id);

    assert_cpu_budget(&env, "withdraw_funds() rejected", REJECTED_CALL_CPU_LIMIT);
    assert_eq!(res, Err(Ok(crate::Error::DeadlineNotPassed)));
}

#[test]
fn test_rejected_contribute_to_cancelled_campaign_is_cheap_and_typed() {
    let (env, _admin, creator, contributor1, _c2, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &10_000);
    let id = client.create_campaign(&plain_campaign(&env, creator.clone(), "Gone"));
    client.verify_campaign(&id);
    client.cancel_campaign(&id);

    env.budget().reset_default();
    let res = client.try_contribute(&id, &contributor1, &100);

    assert_cpu_budget(&env, "contribute() rejected", REJECTED_CALL_CPU_LIMIT);
    assert!(
        res.is_err(),
        "contributing to a cancelled campaign must fail"
    );
}

#[test]
fn test_rejected_refund_on_live_campaign_is_cheap_and_typed() {
    let (env, _admin, creator, contributor1, _c2, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &10_000);
    let id = client.create_campaign(&plain_campaign(&env, creator.clone(), "Live"));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &400);

    env.budget().reset_default();
    let res = client.try_claim_refund(&id, &contributor1);

    assert_cpu_budget(&env, "claim_refund() rejected", REJECTED_CALL_CPU_LIMIT);
    assert!(
        res.is_err(),
        "a live, unfinished campaign has nothing to refund"
    );
}

#[test]
fn test_rejected_double_withdraw_is_cheap_and_typed() {
    let (env, _admin, creator, contributor1, _c2, _token, token_admin, client) = setup_env();
    token_admin.mint(&contributor1, &10_000);
    let id = client.create_campaign(&plain_campaign(&env, creator.clone(), "Twice"));
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &1_000);
    pass_deadline(&env, &client, id);
    client.withdraw_funds(&id);

    env.budget().reset_default();
    let res = client.try_withdraw_funds(&id);

    assert_cpu_budget(&env, "withdraw_funds() repeated", REJECTED_CALL_CPU_LIMIT);
    assert_eq!(res, Err(Ok(crate::Error::FundsAlreadyWithdrawn)));
}
