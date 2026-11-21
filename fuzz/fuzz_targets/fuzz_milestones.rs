//! Fuzz milestone payout arithmetic and claim ordering (#1463).

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{Address, Env, String, Vec};

use proof_of_heart::{Category, CreateCampaignParams, Milestone, ProofOfHeart, ProofOfHeartClient};

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    raw_weights: std::vec::Vec<u32>,
    verification_order: std::vec::Vec<u8>,
    claim_order: std::vec::Vec<u8>,
    amount_raw: u32,
    fee_raw: u16,
}

fn shuffled_indices(count: usize, entropy: &[u8]) -> std::vec::Vec<usize> {
    let mut indices: std::vec::Vec<usize> = (0..count).collect();
    for i in (1..count).rev() {
        let byte = entropy.get(i).copied().unwrap_or(i as u8);
        indices.swap(i, byte as usize % (i + 1));
    }
    indices
}

fn valid_payouts(raw_weights: &[u32]) -> std::vec::Vec<u32> {
    let count = raw_weights.len().clamp(1, 10);
    let weights: std::vec::Vec<u64> = (0..count)
        .map(|i| raw_weights.get(i).copied().unwrap_or(1) as u64 + 1)
        .collect();
    let total_weight: u64 = weights.iter().sum();
    let remaining = 10_000 - count as u32;
    let mut assigned = 0u32;
    let mut payouts = std::vec::Vec::with_capacity(count);
    for weight in weights.iter().take(count - 1) {
        let share = (remaining as u64 * weight / total_weight) as u32;
        let payout = share + 1;
        assigned += payout;
        payouts.push(payout);
    }
    payouts.push(10_000 - assigned);
    payouts
}

fuzz_target!(|input: FuzzInput| {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let contributor = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract(admin.clone());
    let token_admin = StellarAssetClient::new(&env, &token_address);
    let token = soroban_sdk::token::Client::new(&env, &token_address);
    let contract_id = env.register_contract(None, ProofOfHeart);
    let client = ProofOfHeartClient::new(&env, &contract_id);

    let fee_bps = input.fee_raw as u32 % 1001;
    if !matches!(
        client.try_init(&admin, &token_address, &fee_bps),
        Ok(Ok(()))
    ) {
        return;
    }

    let amount = 100_000 + (input.amount_raw as i128 % 1_000_000);
    token_admin.mint(&contributor, &amount);
    let campaign_id = match client.try_create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(&env, "Milestone fuzz"),
        description: String::from_str(&env, "Milestone payout conservation"),
        funding_goal: amount,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    }) {
        Ok(Ok(id)) => id,
        _ => return,
    };
    if !matches!(client.try_verify_campaign(&campaign_id), Ok(Ok(()))) {
        return;
    }

    let payouts = valid_payouts(&input.raw_weights);
    let mut milestones = Vec::new(&env);
    for (id, payout_bps) in payouts.iter().enumerate() {
        milestones.push_back(Milestone {
            id: id as u32,
            description: String::from_str(&env, "Fuzz tranche"),
            payout_bps: *payout_bps,
            verified: false,
        });
    }
    if !matches!(
        client.try_set_milestones(&campaign_id, &milestones),
        Ok(Ok(()))
    ) {
        return;
    }
    if !matches!(
        client.try_contribute(&campaign_id, &contributor, &amount),
        Ok(Ok(()))
    ) {
        return;
    }

    for index in shuffled_indices(payouts.len(), &input.verification_order) {
        if !matches!(
            client.try_verify_milestone(&admin, &campaign_id, &(index as u32)),
            Ok(Ok(()))
        ) {
            return;
        }
    }

    let escrowed = token.balance(&client.address);
    let creator_before = token.balance(&creator);
    let admin_before = token.balance(&admin);
    for index in shuffled_indices(payouts.len(), &input.claim_order) {
        if !matches!(
            client.try_claim_milestone(&campaign_id, &(index as u32)),
            Ok(Ok(()))
        ) {
            return;
        }
    }

    let paid_to_creator = token.balance(&creator) - creator_before;
    let paid_as_fee = token.balance(&admin) - admin_before;
    assert_eq!(paid_to_creator + paid_as_fee, escrowed);
    assert_eq!(token.balance(&client.address), 0);
});
