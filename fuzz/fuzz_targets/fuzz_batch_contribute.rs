//! Fuzz batch contribution accounting across campaign currencies (#1462).

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};
use soroban_sdk::{Address, Env, String, Vec};

use proof_of_heart::{Category, CreateCampaignParams, ProofOfHeart, ProofOfHeartClient};

#[derive(Debug, Arbitrary)]
struct BatchItem {
    campaign_ref: u16,
    amount_hi: i64,
    amount_lo: u64,
}

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    campaign_count_raw: u8,
    campaign_token_choices: std::vec::Vec<u8>,
    items: std::vec::Vec<BatchItem>,
}

fn arbitrary_i128(hi: i64, lo: u64) -> i128 {
    ((hi as i128) << 64) | lo as i128
}

fuzz_target!(|input: FuzzInput| {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contributor = Address::generate(&env);
    let contract_id = env.register_contract(None, ProofOfHeart);
    let client = ProofOfHeartClient::new(&env, &contract_id);

    let platform_token = env.register_stellar_asset_contract(admin.clone());
    if !matches!(client.try_init(&admin, &platform_token, &0u32), Ok(Ok(()))) {
        return;
    }

    let mut token_addresses = std::vec![platform_token.clone()];
    let mut token_admins = std::vec![StellarAssetClient::new(&env, &platform_token)];
    let mut tokens: std::vec::Vec<TokenClient<'_>> =
        std::vec![TokenClient::new(&env, &platform_token)];
    for _ in 1..3 {
        let address = env.register_stellar_asset_contract(admin.clone());
        if !matches!(client.try_set_token_allowed(&address, &true), Ok(Ok(()))) {
            return;
        }
        token_admins.push(StellarAssetClient::new(&env, &address));
        tokens.push(TokenClient::new(&env, &address));
        token_addresses.push(address);
    }

    let campaign_count = (input.campaign_count_raw as usize % 5) + 1;
    let mut campaign_ids = std::vec::Vec::with_capacity(campaign_count);
    let mut campaign_tokens = std::vec::Vec::with_capacity(campaign_count);
    for index in 0..campaign_count {
        let token_index = input
            .campaign_token_choices
            .get(index)
            .copied()
            .unwrap_or(index as u8) as usize
            % token_addresses.len();
        let creator = Address::generate(&env);
        let params = CreateCampaignParams {
            creator,
            title: String::from_str(&env, "Batch fuzz"),
            description: String::from_str(&env, "Batch accounting invariant"),
            funding_goal: 100_000 + (index as i128 * 10_000),
            duration_days: 30,
            category: Category::Learner,
            has_revenue_sharing: false,
            revenue_share_percentage: 0,
            max_contribution_per_user: 0,
        };
        let result = if token_index == 0 {
            client.try_create_campaign(&params)
        } else {
            client.try_create_campaign_with_token(&params, &token_addresses[token_index])
        };
        let campaign_id = match result {
            Ok(Ok(id)) => id,
            _ => return,
        };
        if !matches!(client.try_verify_campaign(&campaign_id), Ok(Ok(()))) {
            return;
        }
        campaign_ids.push(campaign_id);
        campaign_tokens.push(token_index);
    }

    for token_admin in &token_admins {
        token_admin.mint(&contributor, &10_000_000_000_000_000);
    }

    let mut batch: Vec<(u32, i128)> = Vec::new(&env);
    for item in input.items.iter().take(25) {
        let selector = item.campaign_ref as usize % (campaign_count + 1);
        let campaign_id = if selector < campaign_count {
            campaign_ids[selector]
        } else {
            u32::MAX - item.campaign_ref as u32
        };
        batch.push_back((campaign_id, arbitrary_i128(item.amount_hi, item.amount_lo)));
    }

    let _ = client.try_batch_contribute(&contributor, &batch);

    for (token_index, token) in tokens.iter().enumerate() {
        let expected_escrow: i128 = campaign_ids
            .iter()
            .zip(campaign_tokens.iter())
            .filter(|(_, assigned_token)| **assigned_token == token_index)
            .map(|(campaign_id, _)| client.get_campaign(campaign_id).effective_amount_raised)
            .sum();
        assert_eq!(
            token.balance(&client.address),
            expected_escrow,
            "token balance must equal live campaign escrow for token {token_index}"
        );
        for campaign_id in campaign_ids.iter() {
            let campaign = client.get_campaign(campaign_id);
            assert!(campaign.amount_raised <= campaign.funding_goal);
        }
    }
});
