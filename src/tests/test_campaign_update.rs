use super::helpers::*;
use crate::{Category, Error, MaybePendingCreator};
use soroban_sdk::{
    testutils::{AuthorizedFunction, AuthorizedInvocation},
    Address, IntoVal, String, Symbol,
};

#[test]
fn test_update_campaign_allows_verified_campaign_before_contributions() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Original Title"),
        String::from_str(&env, "Original Description"), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    client.verify_campaign(&campaign_id);

    let new_title = String::from_str(&env, "New Title");
    let new_desc = String::from_str(&env, "New Description");
    let res = client.try_update_campaign(&campaign_id, &new_title, &new_desc);
    assert!(res.is_ok());

    let updated = client.get_campaign(&campaign_id);
    assert_eq!(updated.title, new_title);
    assert_eq!(updated.description, new_desc);
}

#[test]
fn test_update_campaign_allows_verified_campaign_with_votes_before_contributions() {
    let (env, _admin, creator, contributor1, contributor2, _, token_admin, client) = setup_env();
    let voter3 = Address::generate(&env);

    token_admin.mint(&contributor1, &100);
    token_admin.mint(&contributor2, &100);
    token_admin.mint(&voter3, &100);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Original Title"),
        String::from_str(&env, "Original Description"), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));

    client.vote_on_campaign(&campaign_id, &contributor1, &true);
    client.vote_on_campaign(&campaign_id, &contributor2, &true);
    client.vote_on_campaign(&campaign_id, &voter3, &true);
    client.verify_campaign_with_votes(&campaign_id);
    assert!(client.get_campaign(&campaign_id).is_verified);

    let new_title = String::from_str(&env, "New Title");
    let new_desc = String::from_str(&env, "New Description");
    assert!(client.try_update_campaign(&campaign_id, &new_title, &new_desc).is_ok());

    let updated = client.get_campaign(&campaign_id);
    assert_eq!(updated.title, new_title);
    assert_eq!(updated.description, new_desc);
}

#[test]
fn test_update_campaign_description_success() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Original Title"),
        String::from_str(&env, "Original description"), 1_000, 30,
        Category::Learner, false, 0, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let new_desc = String::from_str(&env, "Updated description with more detail");
    assert!(client.try_update_campaign_description(&campaign_id, &new_desc).is_ok());

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.description, new_desc);
    assert_eq!(campaign.funding_goal, 1_000);
}

#[test]
fn test_update_campaign_description_rejects_cancelled() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Title"),
        String::from_str(&env, "Desc"), 1_000, 30,
        Category::Learner, false, 0, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);
    client.cancel_campaign(&campaign_id);

    let res = client.try_update_campaign_description(&campaign_id, &String::from_str(&env, "New desc"));
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);
}

#[test]
fn test_update_campaign_description_rejects_empty() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Title"),
        String::from_str(&env, "Desc"), 1_000, 30,
        Category::Learner, false, 0, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let res = client.try_update_campaign_description(&campaign_id, &String::from_str(&env, ""));
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_update_campaign_description_not_found() {
    let (env, _, _, _, _, _, _, client) = setup_env();
    let res = client.try_update_campaign_description(&999, &String::from_str(&env, "Some desc"));
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotFound);
}

#[test]
fn test_campaign_ownership_transfer_flow() {
    let (env, _admin, creator, contributor1, contributor2, _, _, client) = setup_env();
    let new_creator = contributor1;

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Transfer Test"),
        String::from_str(&env, "Desc"), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    client.initiate_campaign_transfer(&campaign_id, &new_creator);
    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.pending_creator, MaybePendingCreator::Some(new_creator.clone()));
    assert_eq!(campaign.creator, creator);

    client.accept_campaign_transfer(&campaign_id);
    let campaign_after = client.get_campaign(&campaign_id);
    assert_eq!(campaign_after.creator, new_creator.clone());
    assert_eq!(campaign_after.pending_creator, MaybePendingCreator::None);

    let updated_description = String::from_str(&env, "Managed by the transferred owner");
    client.update_campaign_description(&campaign_id, &updated_description);

    let auths = env.auths();
    let (auth_addr, invocation) = auths.last().unwrap();
    assert_eq!(auth_addr, &new_creator);
    assert_eq!(
        invocation,
        &AuthorizedInvocation {
            function: AuthorizedFunction::Contract((
                client.address.clone(),
                Symbol::new(&env, "update_campaign_description"),
                (campaign_id, updated_description).into_val(&env),
            )),
            sub_invocations: Default::default(),
        }
    );

    let campaign_id_2 = client.create_campaign(&make_params(
        new_creator.clone(), String::from_str(&env, "Cancel Test"),
        String::from_str(&env, "Desc"), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id_2);
    client.initiate_campaign_transfer(&campaign_id_2, &contributor2);
    client.cancel_campaign_transfer(&campaign_id_2);
    let final_campaign = client.get_campaign(&campaign_id_2);
    assert_eq!(final_campaign.pending_creator, MaybePendingCreator::None);
}

#[test]
fn test_campaign_transfer_validations() {
    let (env, _admin, creator, contributor1, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Transfer Guardrails"),
        String::from_str(&env, "Desc"), 1000, 30,
        Category::Publisher, false, 0, 0i128,
    ));
    let _ = client.try_verify_campaign(&campaign_id);

    let res = client.try_initiate_campaign_transfer(&campaign_id, &creator);
    assert_eq!(res.unwrap_err().unwrap(), Error::InvalidNewOwner);

    let res = client.try_accept_campaign_transfer(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoTransferPending);

    client.initiate_campaign_transfer(&campaign_id, &contributor1);
    client.cancel_campaign_transfer(&campaign_id);

    let auths = env.auths();
    let (auth_addr, _) = auths.last().unwrap();
    assert_eq!(auth_addr, &creator);

    let campaign = client.get_campaign(&campaign_id);
    assert_eq!(campaign.pending_creator, MaybePendingCreator::None);

    let res = client.try_cancel_campaign_transfer(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::NoTransferPending);
}

#[test]
fn test_cancel_campaign_already_cancelled_is_terminal() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Terminal Test"),
        String::from_str(&env, "Already cancelled"), 1000, 30,
        Category::Learner, false, 0, 0i128,
    ));

    client.cancel_campaign(&campaign_id);
    let campaign = client.get_campaign(&campaign_id);
    assert!(campaign.is_cancelled);
    assert!(!campaign.is_active);

    let res = client.try_cancel_campaign(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);
}

#[test]
fn test_cancel_campaign_after_withdrawal_is_terminal() {
    let (env, _admin, creator, contributor1, _, _, token_admin, client) = setup_env();

    token_admin.mint(&contributor1, &2000);

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(), String::from_str(&env, "Withdrawal Terminal"),
        String::from_str(&env, "Funds already out"), 1000, 30,
        Category::Educator, false, 0, 0i128,
    ));
    client.verify_campaign(&campaign_id);
    client.contribute(&campaign_id, &contributor1, &1000);
    client.withdraw_funds(&campaign_id);

    let campaign = client.get_campaign(&campaign_id);
    assert!(campaign.funds_withdrawn);
    assert!(!campaign.is_active);

    let res = client.try_cancel_campaign(&campaign_id);
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);
}
