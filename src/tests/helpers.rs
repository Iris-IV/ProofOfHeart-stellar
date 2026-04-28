pub use crate::storage::set_min_campaign_funding_goal;
pub use crate::{Category, CreateCampaignParams, ProofOfHeart, ProofOfHeartClient};
pub use soroban_sdk::token::Client as TokenClient;
pub use soroban_sdk::token::StellarAssetClient as TokenAdminClient;
pub use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Env, IntoVal, String, Symbol,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn make_params(
    creator: Address,
    title: String,
    description: String,
    funding_goal: i128,
    duration_days: u64,
    category: Category,
    has_revenue_sharing: bool,
    revenue_share_percentage: u32,
    max_contribution_per_user: i128,
) -> CreateCampaignParams {
    CreateCampaignParams {
        creator,
        title,
        description,
        funding_goal,
        duration_days,
        category,
        has_revenue_sharing,
        revenue_share_percentage,
        max_contribution_per_user,
    }
}

pub(crate) fn setup_env_with_default_min<'a>() -> (
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

pub(crate) fn setup_env<'a>() -> (
    Env,
    Address,
    Address,
    Address,
    Address,
    TokenClient<'a>,
    TokenAdminClient<'a>,
    ProofOfHeartClient<'a>,
) {
    let setup = setup_env_with_default_min();
    setup.0.as_contract(&setup.7.address, || {
        set_min_campaign_funding_goal(&setup.0, 1)
    });
    setup
}
