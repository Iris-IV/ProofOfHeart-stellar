//! Fuzz target for `contribute()` (issue #506).
//!
//! `contribute` is the primary path through which user funds enter the
//! contract, and it chains together several validation checks (paused,
//! verified, active, deadline, personal/global caps, overflow-checked
//! anomaly-detection thresholds) before touching storage or moving tokens.
//! Property tests cover specific scenarios; this harness instead throws
//! adversarial, arbitrary input at the same entry point to catch edge cases
//! (extreme `i128` amounts, unusual campaign parameters, repeated calls)
//! that hand-written cases miss.
//!
//! The property under test: `contribute` must never panic / trap the host,
//! no matter the amount or campaign state -- it must always resolve to
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
    amount_hi: i64,
    amount_lo: u64,
    funding_goal_raw: u32,
    duration_days_raw: u8,
    mint_amount: u32,
    platform_fee_raw: u16,
    skip_verify: bool,
    contribute_twice: bool,
}

fuzz_target!(|input: FuzzInput| {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let contributor = Address::generate(&env);

    let token_address = env.register_stellar_asset_contract(admin.clone());
    let token_admin = StellarAssetClient::new(&env, &token_address);

    let contract_id = env.register_contract(None, ProofOfHeart);
    let client = ProofOfHeartClient::new(&env, &contract_id);

    // Platform fee must be within bounds accepted by `init`; anything else
    // should be rejected gracefully, in which case there's nothing further
    // to fuzz for this input.
    let platform_fee = (input.platform_fee_raw as u32) % 1001;
    if client.try_init(&admin, &token_address, &platform_fee).is_err() {
        return;
    }

    let mint_amount = (input.mint_amount as i128).saturating_add(1);
    token_admin.mint(&contributor, &mint_amount);

    let funding_goal = (input.funding_goal_raw as i128).saturating_add(1);
    let duration_days = (input.duration_days_raw as u64).saturating_add(1).min(365);

    let params = CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Fuzz"),
        description: String::from_str(&env, "Fuzz target campaign"),
        funding_goal,
        duration_days,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    };

    let campaign_id = match client.try_create_campaign(&params) {
        Ok(Ok(id)) => id,
        _ => return,
    };

    if !input.skip_verify {
        let _ = client.try_verify_campaign(&campaign_id);
    }

    // Combine two arbitrary integers into a wide i128 so both small and
    // extreme (overflow-triggering) contribution amounts get exercised,
    // including negative values and values near i128::MAX/MIN.
    let amount = ((input.amount_hi as i128) << 64) | (input.amount_lo as i128);

    // The call under test must never panic, regardless of amount or
    // campaign state -- only `Ok(())` or a typed `Err(Error)` are allowed.
    let _ = client.try_contribute(&campaign_id, &contributor, &amount);

    if input.contribute_twice {
        let _ = client.try_contribute(&campaign_id, &contributor, &amount);
    }
});
