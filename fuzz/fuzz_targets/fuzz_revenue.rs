//! Fuzz target for the percentage-based revenue arithmetic in `revenue.rs`
//! (issue #773).
//!
//! `claim_revenue` and `claim_creator_revenue` split a revenue pool between
//! contributors (proportional to their contribution) and the creator, using
//! a chain of `checked_mul`/`checked_div` on `i128` deferred to the last
//! step to avoid intermediate truncation (#375), plus a last-claimant
//! dust-sweep to account for per-claim rounding (#526). This harness
//! fuzzes the contribution amounts (which set the claim denominator), the
//! revenue-share split, the deposit amounts (which set the pool), and the
//! claim order/repetition, to catch precision or overflow issues in that
//! arithmetic.
//!
//! The property under test: `deposit_revenue`, `claim_revenue`, and
//! `claim_creator_revenue` must never panic / trap the host, no matter the
//! pool size, split, or claim sequence -- each call must always resolve to
//! either `Ok(())` or a typed `Err(Error)`.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{Address, Env, String};

use proof_of_heart::{Category, CreateCampaignParams, ProofOfHeart, ProofOfHeartClient};

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    revenue_share_percentage_raw: u32,
    num_contributors_raw: u8,
    contribution_amounts: Vec<u32>,
    deposit_amounts: Vec<u32>,
    claim_order: Vec<u8>,
}

fuzz_target!(|input: FuzzInput| {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);

    let token_address = env.register_stellar_asset_contract(admin.clone());
    let token_admin = StellarAssetClient::new(&env, &token_address);

    let contract_id = env.register_contract(None, ProofOfHeart);
    let client = ProofOfHeartClient::new(&env, &contract_id);

    if client.try_init(&admin, &token_address, &0u32).is_err() {
        return;
    }

    // Revenue sharing is only allowed for `EducationalStartup` campaigns,
    // with a share capped at `REVENUE_SHARE_MAX_BPS` (50%); `create_campaign`
    // rejects anything else, so there's nothing further to fuzz for
    // out-of-range input.
    let revenue_share_percentage = (input.revenue_share_percentage_raw % 5000).saturating_add(1);

    let params = CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Fuzz"),
        description: String::from_str(&env, "Fuzz revenue target"),
        funding_goal: 100_000,
        duration_days: 30,
        category: Category::EducationalStartup,
        has_revenue_sharing: true,
        revenue_share_percentage,
        max_contribution_per_user: 0,
    };

    let campaign_id = match client.try_create_campaign(&params) {
        Ok(Ok(id)) => id,
        _ => return,
    };

    if client.try_verify_campaign(&campaign_id).is_err() {
        return;
    }

    // Fund a handful of contributors and have each contribute an arbitrary
    // amount, so `effective_amount_raised` (the `claim_revenue` denominator)
    // ends up an arbitrary value rather than a single round number. Amounts
    // are kept under the auto-pause single-contribution threshold (200% of
    // the funding goal) so most sequences reach withdrawal.
    let num_contributors = (input.num_contributors_raw % 8).saturating_add(1);
    let mut contributors: Vec<Address> = Vec::new();
    for i in 0..num_contributors {
        let contributor = Address::generate(&env);
        let raw = *input.contribution_amounts.get(i as usize).unwrap_or(&1);
        let amount = ((raw as i128) % 150_000).saturating_add(1);
        token_admin.mint(&contributor, &amount);
        if client
            .try_contribute(&campaign_id, &contributor, &amount)
            .is_ok()
        {
            contributors.push(contributor);
        }
    }

    if client.try_withdraw_funds(&campaign_id).is_err() {
        return;
    }

    // Mint the creator enough to cover every deposit, then deposit revenue
    // in several arbitrary-sized chunks -- claim_revenue/claim_creator_revenue
    // must never panic regardless of the resulting pool size or split.
    let deposit_amounts: Vec<i128> = input
        .deposit_amounts
        .iter()
        .take(10)
        .map(|raw| (*raw as i128).saturating_add(1))
        .collect();
    let total_deposits: i128 = deposit_amounts.iter().sum();
    token_admin.mint(&creator, &total_deposits.max(1));
    for amount in &deposit_amounts {
        let _ = client.try_deposit_revenue(&campaign_id, amount);
    }

    // Claim in an arbitrary order, including repeats, to exercise the
    // last-claimant dust-sweep path (#526) under randomized sequencing.
    for &idx in input.claim_order.iter().take(20) {
        if contributors.is_empty() {
            break;
        }
        let contributor = &contributors[idx as usize % contributors.len()];
        let _ = client.try_claim_revenue(&campaign_id, contributor);
    }
    let _ = client.try_claim_creator_revenue(&campaign_id);
});
