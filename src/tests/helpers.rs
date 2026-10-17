// Tests use soroban-sdk testutils APIs deprecated in SDK 22 (register_contract,
// register_stellar_asset_contract, env.budget()) until they move to the replacements.
#![allow(deprecated)]
extern crate std;

pub use crate::storage::set_min_campaign_funding_goal;
pub use crate::{Category, CreateCampaignParams, ProofOfHeart, ProofOfHeartClient};
pub use soroban_sdk::token::Client as TokenClient;
pub use soroban_sdk::token::StellarAssetClient as TokenAdminClient;
pub use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Env, IntoVal, String,
};

// Mirrors every CreateCampaignParams field positionally; used by most tests.
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
    env.mock_all_auths_allowing_non_root_auth();

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

pub(crate) fn setup_token<'a>(env: &Env, admin: &Address) -> TokenClient<'a> {
    let token_address = env.register_stellar_asset_contract(admin.clone());
    TokenClient::new(env, &token_address)
}

pub(crate) fn setup_contract<'a>(
    env: &Env,
    admin: &Address,
    token_address: &Address,
) -> ProofOfHeartClient<'a> {
    let contract_id = env.register_contract(None, ProofOfHeart);
    let client = ProofOfHeartClient::new(env, &contract_id);
    client.init(admin, token_address, &300);
    client
}

/// Moves the ledger timestamp to `campaign.deadline + 1` (#854 / #1320).
///
/// `withdraw_funds` only succeeds once the funding window has closed, so any
/// test exercising the withdrawal (or post-deadline) path must first move time
/// past the campaign deadline. The deadline is read through the contract's
/// own `get_campaign` view, so no direct storage access (and therefore no
/// `as_contract` wrap) is needed.
pub(crate) fn advance_past_deadline(
    env: &Env,
    client: &ProofOfHeartClient<'_>,
    campaign_id: u32,
) {
    let deadline = client.get_campaign(&campaign_id).deadline;
    env.ledger().with_mut(|l| l.timestamp = deadline + 1);
}

/// Wraps direct storage / TTL / event reads in contract context (#1338).
///
/// Under soroban-sdk 22, `env.storage()`, `env.budget()` TTL queries and
/// `env.events().all()` executed from a bare test (outside any contract
/// invocation) fail with `Error(Storage, InternalError)` or silently return
/// empty results. Every such read must run inside `env.as_contract`; this
/// helper names that intent at the call site.
pub(crate) fn with_contract<T>(
    env: &Env,
    contract: &Address,
    f: impl FnOnce() -> T,
) -> T {
    env.as_contract(contract, f)
}
