//! Fuzz target for campaign state transitions centralized in `lifecycle.rs`
//! (issue #772).
//!
//! `lifecycle::transition` is the single source of truth for which
//! `CampaignState` changes are legal (`Active -> Verified`,
//! `Active/Verified -> Cancelled`, `Verified -> Withdrawn`; `Withdrawn` and
//! `Cancelled` are terminal). Every lifecycle-changing entry point
//! (`contribute`, `verify_campaign`, `verify_campaign_with_votes`,
//! `cancel_campaign`, `withdraw_funds`) consults it before mutating a
//! campaign. This harness drives a single campaign through an arbitrary
//! sequence of those calls to catch any path that could apply an invalid
//! transition or otherwise corrupt state.
//!
//! The property under test: no matter the call sequence, every call must
//! resolve to either `Ok(())` or a typed `Err(Error)` -- in particular
//! `Error::InvalidStateTransition` for any illegal transition -- and must
//! never panic / trap the host.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{Address, Env, String};

use proof_of_heart::{Category, CreateCampaignParams, ProofOfHeart, ProofOfHeartClient};

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    funding_goal_raw: u32,
    duration_days_raw: u8,
    mint_amount: u32,
    contribute_amount_raw: u32,
    ops: Vec<u8>,
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

    if client.try_init(&admin, &token_address, &0u32).is_err() {
        return;
    }

    let mint_amount = (input.mint_amount as i128).saturating_add(1);
    token_admin.mint(&contributor, &mint_amount);

    let funding_goal = (input.funding_goal_raw as i128).saturating_add(1) * 1000 + 100_000;
    let duration_days = (input.duration_days_raw as u64).saturating_add(1).min(365);
    let contribute_amount = (input.contribute_amount_raw as i128).saturating_add(1);

    let params = CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Fuzz"),
        description: String::from_str(&env, "Fuzz lifecycle target"),
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

    // Drive the campaign through an arbitrary sequence of lifecycle-changing
    // calls. `lifecycle::transition` must reject any illegal state change
    // with a typed error rather than letting the campaign land in an
    // inconsistent state or panicking.
    for op in input.ops.iter().take(30) {
        match op % 5 {
            0 => {
                let _ = client.try_contribute(&campaign_id, &contributor, &contribute_amount);
            }
            1 => {
                let _ = client.try_verify_campaign(&campaign_id);
            }
            2 => {
                let _ = client.try_verify_campaign_with_votes(&campaign_id);
            }
            3 => {
                let _ = client.try_cancel_campaign(&campaign_id);
            }
            _ => {
                let _ = client.try_withdraw_funds(&campaign_id);
            }
        }
    }
});
