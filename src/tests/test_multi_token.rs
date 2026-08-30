//! Per-campaign currencies (#784).
//!
//! A campaign's token is fixed at creation and governs every movement of value
//! for that campaign: contributions, refunds, withdrawals, milestone payouts
//! and revenue. The platform token remains the default and the only currency
//! `create_campaign` can produce, so nothing about existing clients changes.

use super::helpers::*;
use crate::{storage, Category, CreateCampaignParams, Error};
use soroban_sdk::{token::StellarAssetClient as TokenAdminClient, Address, String};

/// Register a second asset and allowlist it, returning its address and
/// clients.
fn second_currency<'a>(
    env: &soroban_sdk::Env,
    admin: &Address,
    client: &ProofOfHeartClient<'a>,
) -> (Address, TokenClient<'a>, TokenAdminClient<'a>) {
    let addr = env.register_stellar_asset_contract(admin.clone());
    client.set_token_allowed(&addr, &true);
    (
        addr.clone(),
        TokenClient::new(env, &addr),
        TokenAdminClient::new(env, &addr),
    )
}

fn params(env: &soroban_sdk::Env, creator: &Address, title: &str) -> CreateCampaignParams {
    CreateCampaignParams {
        creator: creator.clone(),
        title: String::from_str(env, title),
        description: String::from_str(env, "A campaign in a chosen currency"),
        funding_goal: 5000,
        duration_days: 30,
        category: Category::Learner,
        has_revenue_sharing: false,
        revenue_share_percentage: 0,
        max_contribution_per_user: 0i128,
    }
}

// ── Defaults and backward compatibility ──────────────────────────────────────

/// A campaign created the ordinary way is denominated in the platform token.
#[test]
fn test_default_campaign_uses_the_platform_token() {
    let (env, _admin, creator, _, _, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&params(&env, &creator, "Default"));

    assert_eq!(client.get_campaign_token(&id), client.get_token());
}

/// The default path writes no currency key at all.
///
/// This is load-bearing rather than an implementation detail: a persistent
/// entry per campaign is permanent rent for a value that is already the
/// default, and enough of it exhausts the host budget in tests that create
/// dozens of campaigns in one invocation.
#[test]
fn test_default_campaign_stores_no_currency_key() {
    let (env, _admin, creator, _, _, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&params(&env, &creator, "Default"));

    env.as_contract(&client.address, || {
        assert!(
            !storage::has_campaign_token(&env, id),
            "the platform-token default must not cost a storage entry"
        );
    });
}

/// Naming the platform token explicitly is accepted and behaves exactly like
/// `create_campaign`.
#[test]
fn test_explicit_platform_token_is_equivalent_to_the_default() {
    let (env, _admin, creator, _, _, _token, _token_admin, client) = setup_env();
    let platform = client.get_token();

    let id = client.create_campaign_with_token(&params(&env, &creator, "Explicit"), &platform);

    assert_eq!(client.get_campaign_token(&id), platform);
    env.as_contract(&client.address, || {
        assert!(!storage::has_campaign_token(&env, id));
    });
}

// ── Choosing a currency ──────────────────────────────────────────────────────

/// A campaign can be denominated in an allowlisted non-default token.
#[test]
fn test_campaign_can_use_an_allowlisted_token() {
    let (env, admin, creator, _, _, _token, _token_admin, client) = setup_env();
    let (usdc, _usdc, _usdc_admin) = second_currency(&env, &admin, &client);

    let id = client.create_campaign_with_token(&params(&env, &creator, "USDC Campaign"), &usdc);

    assert_eq!(client.get_campaign_token(&id), usdc);
    assert_ne!(client.get_campaign_token(&id), client.get_token());
}

/// A token the admin has not allowlisted is refused.
///
/// The allowlist is what stops a creator naming an arbitrary contract as their
/// campaign's currency, which would hand every contributor a `transfer` call
/// into code the platform never reviewed.
#[test]
fn test_campaign_cannot_use_an_unallowlisted_token() {
    let (env, admin, creator, _, _, _token, _token_admin, client) = setup_env();
    let rogue = env.register_stellar_asset_contract(admin.clone());

    let res = client.try_create_campaign_with_token(&params(&env, &creator, "Rogue"), &rogue);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    // No campaign was created.
    assert_eq!(client.get_campaign_count(), 0);
}

/// The allowlist is admin-controlled and readable.
#[test]
fn test_token_allowlist_round_trip() {
    let (env, admin, _creator, _, _, _token, _token_admin, client) = setup_env();
    let candidate = env.register_stellar_asset_contract(admin.clone());

    assert!(!client.is_token_allowed(&candidate));

    client.set_token_allowed(&candidate, &true);
    assert!(client.is_token_allowed(&candidate));

    client.set_token_allowed(&candidate, &false);
    assert!(!client.is_token_allowed(&candidate));
}

/// The platform token is always allowed without an explicit entry, so an
/// upgrade cannot silently break `create_campaign`.
#[test]
fn test_platform_token_is_allowed_without_an_entry() {
    let (_env, _admin, _creator, _, _, _token, _token_admin, client) = setup_env();

    assert!(client.is_token_allowed(&client.get_token()));
}

/// Removing a token from the allowlist stops new campaigns but leaves existing
/// ones alone — their currency was fixed when contributors funded them.
#[test]
fn test_disallowing_a_token_does_not_affect_existing_campaigns() {
    let (env, admin, creator, _, _, _token, _token_admin, client) = setup_env();
    let (usdc, _usdc, _usdc_admin) = second_currency(&env, &admin, &client);

    let id = client.create_campaign_with_token(&params(&env, &creator, "Existing"), &usdc);

    client.set_token_allowed(&usdc, &false);

    // The existing campaign keeps its currency.
    assert_eq!(client.get_campaign_token(&id), usdc);

    // A new one in that currency is refused.
    let res = client.try_create_campaign_with_token(&params(&env, &creator, "New"), &usdc);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

// ── Value actually moves in the campaign's currency ──────────────────────────

/// Contributions are pulled in the campaign's own token, and the platform
/// token is untouched.
#[test]
fn test_contribution_moves_the_campaign_currency() {
    let (env, admin, creator, contributor1, _, platform, platform_admin, client) = setup_env();
    let (usdc, usdc_token, usdc_admin) = second_currency(&env, &admin, &client);

    usdc_admin.mint(&contributor1, &5000);
    platform_admin.mint(&contributor1, &5000);

    let id = client.create_campaign_with_token(&params(&env, &creator, "USDC Campaign"), &usdc);
    client.verify_campaign(&id);

    client.contribute(&id, &contributor1, &1000);

    assert_eq!(usdc_token.balance(&contributor1), 4000);
    assert_eq!(usdc_token.balance(&client.address), 1000);

    // The platform token was not touched.
    assert_eq!(platform.balance(&contributor1), 5000);
    assert_eq!(platform.balance(&client.address), 0);

    let _ = usdc;
}

/// A refund is paid back in the same currency the contribution arrived in.
///
/// This is the failure a shared global token would produce: funds pulled in
/// one asset and repaid in another.
#[test]
fn test_refund_returns_the_campaign_currency() {
    let (env, admin, creator, contributor1, _, platform, platform_admin, client) = setup_env();
    let (usdc, usdc_token, usdc_admin) = second_currency(&env, &admin, &client);

    usdc_admin.mint(&contributor1, &5000);
    platform_admin.mint(&client.address, &10_000);

    let id = client.create_campaign_with_token(&params(&env, &creator, "USDC Campaign"), &usdc);
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &1000);

    // Cancel so the contribution becomes refundable.
    client.cancel_campaign(&id);
    client.claim_refund(&id, &contributor1);

    assert_eq!(usdc_token.balance(&contributor1), 5000);
    assert_eq!(usdc_token.balance(&client.address), 0);

    // The contract's platform-token balance is exactly as it was: the refund
    // did not come out of the wrong pot.
    assert_eq!(platform.balance(&client.address), 10_000);

    let _ = usdc;
}

/// Two campaigns in different currencies keep their funds separate.
#[test]
fn test_campaigns_in_different_currencies_do_not_share_funds() {
    let (env, admin, creator, contributor1, contributor2, platform, platform_admin, client) =
        setup_env();
    let (usdc, usdc_token, usdc_admin) = second_currency(&env, &admin, &client);

    platform_admin.mint(&contributor1, &5000);
    usdc_admin.mint(&contributor2, &5000);

    let native_id = client.create_campaign(&params(&env, &creator, "Native"));
    let usdc_id = client.create_campaign_with_token(&params(&env, &creator, "USDC"), &usdc);
    client.verify_campaign(&native_id);
    client.verify_campaign(&usdc_id);

    client.contribute(&native_id, &contributor1, &1000);
    client.contribute(&usdc_id, &contributor2, &2000);

    assert_eq!(platform.balance(&client.address), 1000);
    assert_eq!(usdc_token.balance(&client.address), 2000);

    // Each campaign's accounting reflects only its own currency.
    assert_eq!(client.get_campaign(&native_id).amount_raised, 1000);
    assert_eq!(client.get_campaign(&usdc_id).amount_raised, 2000);
}

/// A batch spanning two currencies pulls each in its own token.
///
/// The batch used to make a single aggregate transfer, which silently assumed
/// every campaign shared one currency.
#[test]
fn test_batch_contribute_across_currencies() {
    let (env, admin, creator, contributor1, _, platform, platform_admin, client) = setup_env();
    let (usdc, usdc_token, usdc_admin) = second_currency(&env, &admin, &client);

    platform_admin.mint(&contributor1, &5000);
    usdc_admin.mint(&contributor1, &5000);

    let native_id = client.create_campaign(&params(&env, &creator, "Native"));
    let usdc_id = client.create_campaign_with_token(&params(&env, &creator, "USDC"), &usdc);
    client.verify_campaign(&native_id);
    client.verify_campaign(&usdc_id);

    let batch = soroban_sdk::Vec::from_array(&env, [(native_id, 700i128), (usdc_id, 300i128)]);
    client.batch_contribute(&contributor1, &batch);

    assert_eq!(platform.balance(&client.address), 700);
    assert_eq!(usdc_token.balance(&client.address), 300);
    assert_eq!(platform.balance(&contributor1), 4300);
    assert_eq!(usdc_token.balance(&contributor1), 4700);
}

/// Two campaigns sharing one currency inside a batch are still settled in a
/// single transfer per currency, not one per contribution.
#[test]
fn test_batch_contribute_groups_by_currency() {
    let (env, admin, creator, contributor1, _, platform, platform_admin, client) = setup_env();
    let (usdc, usdc_token, usdc_admin) = second_currency(&env, &admin, &client);

    platform_admin.mint(&contributor1, &5000);
    usdc_admin.mint(&contributor1, &5000);

    let a = client.create_campaign_with_token(&params(&env, &creator, "USDC A"), &usdc);
    let b = client.create_campaign_with_token(&params(&env, &creator, "USDC B"), &usdc);
    let c = client.create_campaign(&params(&env, &creator, "Native"));
    for id in [a, b, c] {
        client.verify_campaign(&id);
    }

    let batch = soroban_sdk::Vec::from_array(&env, [(a, 100i128), (b, 200i128), (c, 300i128)]);
    client.batch_contribute(&contributor1, &batch);

    assert_eq!(usdc_token.balance(&client.address), 300);
    assert_eq!(platform.balance(&client.address), 300);
    assert_eq!(client.get_contribution(&a, &contributor1), 100);
    assert_eq!(client.get_contribution(&b, &contributor1), 200);
    assert_eq!(client.get_contribution(&c, &contributor1), 300);
}

/// A withdrawal pays the creator in the campaign's currency.
#[test]
fn test_withdrawal_pays_in_the_campaign_currency() {
    let (env, admin, creator, contributor1, _, platform, platform_admin, client) = setup_env();
    let (usdc, usdc_token, usdc_admin) = second_currency(&env, &admin, &client);

    usdc_admin.mint(&contributor1, &10_000);
    platform_admin.mint(&client.address, &10_000);

    let id = client.create_campaign_with_token(&params(&env, &creator, "USDC Campaign"), &usdc);
    client.verify_campaign(&id);
    client.contribute(&id, &contributor1, &5000);

    client.withdraw_funds(&id);

    // The creator was paid in USDC, and the contract's platform-token balance
    // is untouched.
    assert!(usdc_token.balance(&creator) > 0);
    assert_eq!(platform.balance(&creator), 0);
    assert_eq!(platform.balance(&client.address), 10_000);
}

// ── Voting stays denominated in the platform token ───────────────────────────

/// Voting weight is measured in the platform token regardless of the
/// campaign's currency.
///
/// Otherwise a creator could denominate their campaign in an obscure token and
/// hand voting rights over their own verification to whoever holds it.
#[test]
fn test_voting_uses_the_platform_token_not_the_campaign_currency() {
    let (env, admin, creator, contributor1, _, _platform, platform_admin, client) = setup_env();
    let (usdc, _usdc_token, usdc_admin) = second_currency(&env, &admin, &client);

    let id = client.create_campaign_with_token(&params(&env, &creator, "USDC Campaign"), &usdc);

    // A voter rich in the campaign's currency but holding no platform token
    // has no say.
    usdc_admin.mint(&contributor1, &1_000_000);
    let res = client.try_vote_on_campaign(&id, &contributor1, &true);
    assert_eq!(res.unwrap_err().unwrap(), Error::NotTokenHolder);

    // Give them the platform token and the vote lands.
    platform_admin.mint(&contributor1, &1_000_000);
    client.vote_on_campaign(&id, &contributor1, &true);
    assert_eq!(client.get_approve_votes(&id), 1);
}

// ── Authorization ────────────────────────────────────────────────────────────

/// Only the admin can change the allowlist.
#[test]
fn test_allowlist_requires_admin_authorization() {
    let (env, admin, _creator, _, _, _token, _token_admin, client) = setup_env();
    let candidate = env.register_stellar_asset_contract(admin.clone());

    client.set_token_allowed(&candidate, &true);

    let required = env.auths().iter().any(|(addr, _)| *addr == admin);
    assert!(
        required,
        "set_token_allowed must require the admin's authorization"
    );
}

/// An unknown campaign id reads as the platform token rather than trapping, so
/// a client can query any id.
#[test]
fn test_unknown_campaign_reads_as_the_platform_token() {
    let (_env, _admin, _creator, _, _, _token, _token_admin, client) = setup_env();

    assert_eq!(client.get_campaign_token(&999), client.get_token());
}

/// A generated address that is not a token contract cannot be allowlisted into
/// use by accident: it is refused at creation because it was never allowed.
#[test]
fn test_arbitrary_address_is_not_a_currency() {
    let (env, _admin, creator, _, _, _token, _token_admin, client) = setup_env();
    let not_a_token = Address::generate(&env);

    assert!(!client.is_token_allowed(&not_a_token));
    let res = client.try_create_campaign_with_token(&params(&env, &creator, "Bogus"), &not_a_token);
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}
