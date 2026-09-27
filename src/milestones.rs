use super::*;
use soroban_sdk::token::Client as TokenClient;
use soroban_sdk::token::StellarAssetClient as TokenAdminClient;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::testutils::Ledger;
use soroban_sdk::Address;

fn setup_env<'a>() -> (
    Env,
    Address,
    Address,
    Address,
    Address,
    TokenClient<'a>,
    TokenAdminClient<'a>,
    ProofOfHeartClient<'a>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let contributor1 = Address::generate(&env);
    let contributor2 = Address::generate(&env);

    let token_address = env.register_stellar_asset_contract(admin.clone());
    let token = TokenClient::new(&env, &token_address);
    let token_admin = TokenAdminClient::new(&env, &token_address);

    let contract_id = env.register_contract(None, ProofOfHeart);
    let client = ProofOfHeartClient::new(&env, &contract_id);

    client.init(&admin, &token_address, &300);
    env.as_contract(&client.address, || set_min_campaign_funding_goal(&env, 1));

    (
        env,
        admin,
        creator,
        contributor1,
        contributor2,
        token,
        token_admin,
        client,
    )
}

#[test]
fn test_campaign_payout_full_amount() {
    let (env, admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let title = String::from_str(&env, "Payout Campaign");
    let desc = String::from_str(&env, "Full payout test");
    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title,
        description: desc,
        funding_goal: 1000,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });
    client.verify_campaign(&campaign_id);

    client.contribute(&campaign_id, &contributor1, &1000);

    let campaign = client.get_campaign(&campaign_id);
    let deadline = campaign.deadline;
    env.ledger().with_mut(|li| {
        li.timestamp = deadline + 1;
    });

    client.withdraw_funds(&campaign_id);

    assert_eq!(token.balance(&admin), 30);
    assert_eq!(token.balance(&creator), 970);
}

#[test]
fn test_campaign_payout_with_fee_override() {
    let (env, admin, creator, contributor1, _, token, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &5000);

    let title = String::from_str(&env, "Override Fee Campaign");
    let desc = String::from_str(&env, "Fee override payout");
    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title,
        description: desc,
        funding_goal: 1000,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });
    client.verify_campaign(&campaign_id);

    client.set_campaign_fee_override(&admin, &campaign_id, &1000);

    client.contribute(&campaign_id, &contributor1, &1000);

    let campaign = client.get_campaign(&campaign_id);
    let deadline = campaign.deadline;
    env.ledger().with_mut(|li| {
        li.timestamp = deadline + 1;
    });

    client.withdraw_funds(&campaign_id);

    assert_eq!(token.balance(&admin), 100);
    assert_eq!(token.balance(&creator), 900);
}

#[test]
fn test_campaign_payout_rejects_unverified() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let title = String::from_str(&env, "Unverified Payout");
    let desc = String::from_str(&env, "Cannot withdraw unverified");
    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title,
        description: desc,
        funding_goal: 1000,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });

    let campaign = client.get_campaign(&campaign_id);
    let deadline = campaign.deadline;
    env.ledger().with_mut(|li| {
        li.timestamp = deadline + 1;
    });

    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotVerified);
}

#[test]
fn test_campaign_payout_rejects_zero_balance() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let title = String::from_str(&env, "Empty Payout");
    let desc = String::from_str(&env, "No funds to withdraw");
    let campaign_id = client.create_campaign(&CreateCampaignParams {
        creator: creator.clone(),
        title,
        description: desc,
        funding_goal: 1000,
        duration_days: 30,
        category: Category::Educator,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0,
    });
    client.verify_campaign(&campaign_id);

    let res = client.try_withdraw_funds(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoFundsToWithdraw);
}
