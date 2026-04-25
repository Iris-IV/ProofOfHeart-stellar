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
    idx: u32,
) -> u32 {
    client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(env, "Campaign"),
        description: String::from_str(env, "Pagination test"),
        funding_goal: 1_000 + idx as i128,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    })
}

#[test]
fn list_campaigns_boundary_cases() {
    let (env, _admin, creator, client) = setup_env();

    for idx in 0..3 {
        let id = create_campaign(&env, &client, &creator, idx);
        assert_eq!(id, idx + 1);
    }

    let first_page = client.list_campaigns(&0, &2);
    assert_eq!(first_page.len(), 2);
    assert_eq!(first_page.get(0).unwrap().id, 1);
    assert_eq!(first_page.get(1).unwrap().id, 2);

    let all = client.list_campaigns(&0, &u32::MAX);
    assert_eq!(all.len(), 3);
    assert_eq!(all.get(0).unwrap().id, 1);
    assert_eq!(all.get(2).unwrap().id, 3);

    let total = client.get_campaign_count();
    assert_eq!(client.list_campaigns(&total, &5).len(), 0);
    assert_eq!(client.list_campaigns(&(total + 1), &5).len(), 0);
    assert_eq!(client.list_campaigns(&0, &0).len(), 0);
}

#[test]
fn list_active_campaigns_boundary_cases_and_sparse_results() {
    let (env, _admin, creator, client) = setup_env();

    for idx in 0..5 {
        let _ = create_campaign(&env, &client, &creator, idx);
    }

    client.cancel_campaign(&2);
    client.cancel_campaign(&4);

    let first_page = client.list_active_campaigns(&0, &2);
    assert_eq!(first_page.len(), 2);
    assert_eq!(first_page.get(0).unwrap().id, 1);
    assert_eq!(first_page.get(1).unwrap().id, 3);

    let sparse_page = client.list_active_campaigns(&1, &2);
    assert_eq!(sparse_page.len(), 2);
    assert_eq!(sparse_page.get(0).unwrap().id, 3);
    assert_eq!(sparse_page.get(1).unwrap().id, 5);

    let all = client.list_active_campaigns(&0, &u32::MAX);
    assert_eq!(all.len(), 3);
    assert_eq!(all.get(0).unwrap().id, 1);
    assert_eq!(all.get(1).unwrap().id, 3);
    assert_eq!(all.get(2).unwrap().id, 5);

    let total = client.get_campaign_count();
    assert_eq!(client.list_active_campaigns(&total, &5).len(), 0);
    assert_eq!(client.list_active_campaigns(&(total + 1), &5).len(), 0);
    assert_eq!(client.list_active_campaigns(&0, &0).len(), 0);
}
