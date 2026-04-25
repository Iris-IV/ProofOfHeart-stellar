use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup_env<'a>() -> (Env, Address, Address, ProofOfHeartClient<'a>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);

    let token_address = env.register_stellar_asset_contract(admin.clone());
    let contract_id = env.register_contract(None, ProofOfHeart);
    let client = ProofOfHeartClient::new(&env, &contract_id);
    client.init(&admin, &token_address, &300);

    (env, admin, creator, client)
}

fn create_campaign(
    env: &Env,
    client: &ProofOfHeartClient<'_>,
    creator: &Address,
    title: &str,
) -> u32 {
    client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(env, title),
        description: String::from_str(env, "Campaign transfer test"),
        funding_goal: 1_000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    })
}

#[test]
fn campaign_transfer_reinitiate_replaces_pending_owner() {
    let (env, _admin, creator, client) = setup_env();
    let pending_one = Address::generate(&env);
    let pending_two = Address::generate(&env);
    let campaign_id = create_campaign(&env, &client, &creator, "Re-initiate transfer");

    client.initiate_campaign_transfer(&campaign_id, &pending_one);
    assert_eq!(
        client.get_campaign(&campaign_id).pending_creator,
        MaybePendingCreator::Some(pending_one)
    );

    client.initiate_campaign_transfer(&campaign_id, &pending_two);

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.creator, creator);
    assert_eq!(
        campaign.pending_creator,
        MaybePendingCreator::Some(pending_two.clone())
    );

    client.accept_campaign_transfer(&campaign_id);

    let transferred = client.get_campaign(&campaign_id);
    assert_eq!(transferred.creator, pending_two);
    assert_eq!(transferred.pending_creator, MaybePendingCreator::None);
}

#[test]
fn campaign_transfer_cancel_then_reinitiate_succeeds() {
    let (env, _admin, creator, client) = setup_env();
    let pending_one = Address::generate(&env);
    let pending_two = Address::generate(&env);
    let campaign_id = create_campaign(&env, &client, &creator, "Cancel and retry");

    client.initiate_campaign_transfer(&campaign_id, &pending_one);
    client.cancel_campaign_transfer(&campaign_id);
    assert_eq!(
        client.get_campaign(&campaign_id).pending_creator,
        MaybePendingCreator::None
    );

    client.initiate_campaign_transfer(&campaign_id, &pending_two.clone());
    client.accept_campaign_transfer(&campaign_id);

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.creator, pending_two);
    assert_eq!(campaign.pending_creator, MaybePendingCreator::None);
}

#[test]
fn campaign_transfer_still_rejects_transfer_to_self() {
    let (env, _admin, creator, client) = setup_env();
    let campaign_id = create_campaign(&env, &client, &creator, "Self transfer");

    let res = client.try_initiate_campaign_transfer(&campaign_id, &creator);
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidNewOwner);
}
