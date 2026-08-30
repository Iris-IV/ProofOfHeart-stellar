use super::helpers::*;
use crate::{Category, Error};
use soroban_sdk::{Address, IntoVal, String};

#[test]
fn test_save_and_get_saved_campaigns() {
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    let id1 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 1"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let id2 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 2"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    assert_eq!(
        client.get_saved_campaigns(&contributor1),
        soroban_sdk::vec![&env]
    );

    client.save_campaign(&contributor1, &id1);
    client.save_campaign(&contributor1, &id2);

    let saved = client.get_saved_campaigns(&contributor1);
    assert_eq!(saved.len(), 2);
    assert_eq!(saved.get(0).unwrap(), id1);
    assert_eq!(saved.get(1).unwrap(), id2);
}

#[test]
fn test_save_campaign_nonexistent_fails() {
    let (_env, _admin, _creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    let result = client.try_save_campaign(&contributor1, &999);
    assert_eq!(result, Err(Ok(Error::CampaignNotFound)));
}

#[test]
fn test_save_campaign_duplicate_fails() {
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.save_campaign(&contributor1, &id);
    let result = client.try_save_campaign(&contributor1, &id);
    assert_eq!(result, Err(Ok(Error::CampaignAlreadyBookmarked)));
}

#[test]
fn test_remove_saved_campaign() {
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    let id1 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 1"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let id2 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 2"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.save_campaign(&contributor1, &id1);
    client.save_campaign(&contributor1, &id2);

    client.remove_saved_campaign(&contributor1, &id1);

    let saved = client.get_saved_campaigns(&contributor1);
    assert_eq!(saved.len(), 1);
    assert_eq!(saved.get(0).unwrap(), id2);
}

#[test]
fn test_remove_saved_campaign_not_bookmarked_fails() {
    let (_env, _admin, _creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    let result = client.try_remove_saved_campaign(&contributor1, &1);
    assert_eq!(result, Err(Ok(Error::CampaignNotBookmarked)));
}

#[test]
fn test_saved_campaigns_are_per_wallet() {
    let (env, _admin, creator, contributor1, contributor2, _token, _token_admin, client) =
        setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.save_campaign(&contributor1, &id);

    assert_eq!(client.get_saved_campaigns(&contributor1).len(), 1);
    assert_eq!(client.get_saved_campaigns(&contributor2).len(), 0);
}

#[test]
fn test_remove_saved_campaign_requires_auth_for_the_requested_user() {
    let (env, _admin, creator, contributor1, contributor2, _token, _token_admin, client) =
        setup_env();

    let campaign_id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.save_campaign(&contributor1, &campaign_id);

    client.remove_saved_campaign(&contributor1, &campaign_id);

    // Verify that remove_saved_campaign requires authorization from the specified user (contributor1)
    let auths = env.auths();
    let found = auths.iter().any(|(addr, inv)| {
        *addr == contributor1
            && match &inv.function {
                soroban_sdk::testutils::AuthorizedFunction::Contract((contract, function, _)) => {
                    contract == &client.address
                        && function == &soroban_sdk::Symbol::new(&env, "remove_saved_campaign")
                }
                _ => false,
            }
    });
    assert!(
        found,
        "remove_saved_campaign must record authorization for contributor1"
    );

    // Also verify trying to remove a campaign that contributor2 hasn't bookmarked fails cleanly
    let result = client.try_remove_saved_campaign(&contributor2, &campaign_id);
    assert_eq!(result, Err(Ok(Error::CampaignNotBookmarked)));
}

#[test]
fn test_save_campaign_then_cancel() {
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    let id = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    // Contributor bookmarks the campaign
    client.save_campaign(&contributor1, &id);
    let saved = client.get_saved_campaigns(&contributor1);
    assert_eq!(saved.len(), 1);
    assert_eq!(saved.get(0).unwrap(), id);

    // Creator cancels the campaign
    client.cancel_campaign(&id);

    // A cancelled campaign is no longer a live bookmark: get_saved_campaigns
    // filters it out so clients don't need a per-id lookup to tell a stale
    // bookmark from a live one (#667).
    let saved_after_cancel = client.get_saved_campaigns(&contributor1);
    assert_eq!(saved_after_cancel, soroban_sdk::vec![&env]);

    // The count reflects the filtered (live) list too.
    assert_eq!(client.get_saved_campaigns_count(&contributor1), 0);

    // Campaign is cancelled
    let campaign = client.get_campaign(&id);
    assert!(campaign.is_cancelled);
    assert!(!campaign.is_active);
}

#[test]
fn test_get_saved_returns_insertion_order_after_interleaved_add_remove_add() {
    // Verifies that get_saved returns campaign ids in the order they were saved,
    // even after a mid-list removal. The doc comment promises "in the order they
    // were saved", which should hold after remove operations.
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    // Create three campaigns
    let id1 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 1"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let id2 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 2"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let id3 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 3"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    // Save all three in order: [id1, id2, id3]
    client.save_campaign(&contributor1, &id1);
    client.save_campaign(&contributor1, &id2);
    client.save_campaign(&contributor1, &id3);

    let saved = client.get_saved_campaigns(&contributor1);
    assert_eq!(saved.len(), 3);
    assert_eq!(saved.get(0).unwrap(), id1);
    assert_eq!(saved.get(1).unwrap(), id2);
    assert_eq!(saved.get(2).unwrap(), id3);

    // Remove the middle campaign (id2)
    client.remove_saved_campaign(&contributor1, &id2);

    let saved_after_remove = client.get_saved_campaigns(&contributor1);
    assert_eq!(saved_after_remove.len(), 2);
    assert_eq!(saved_after_remove.get(0).unwrap(), id1);
    assert_eq!(saved_after_remove.get(1).unwrap(), id3);

    // Re-add id2 - it should be appended at the end, not inserted back in its original position
    client.save_campaign(&contributor1, &id2);

    let saved_after_readd = client.get_saved_campaigns(&contributor1);
    assert_eq!(saved_after_readd.len(), 3);
    // Order should reflect insertion order: id1, id3 (from before), then id2 (re-added)
    assert_eq!(saved_after_readd.get(0).unwrap(), id1);
    assert_eq!(saved_after_readd.get(1).unwrap(), id3);
    assert_eq!(saved_after_readd.get(2).unwrap(), id2);
}

#[test]
fn test_get_saved_campaigns_count() {
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    let id1 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 1"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let id2 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 2"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    // Fresh wallet has no live bookmarks.
    assert_eq!(client.get_saved_campaigns_count(&contributor1), 0);

    client.save_campaign(&contributor1, &id1);
    client.save_campaign(&contributor1, &id2);
    assert_eq!(client.get_saved_campaigns_count(&contributor1), 2);

    // Count stays in sync with get_saved_campaigns.
    assert_eq!(
        client.get_saved_campaigns(&contributor1).len(),
        client.get_saved_campaigns_count(&contributor1)
    );
}

#[test]
fn test_batch_save_campaigns() {
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    let id1 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 1"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let id2 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 2"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.batch_save_campaigns(&contributor1, &soroban_sdk::vec![&env, id1, id2]);

    let saved = client.get_saved_campaigns(&contributor1);
    assert_eq!(saved.len(), 2);
    assert_eq!(saved.get(0).unwrap(), id1);
    assert_eq!(saved.get(1).unwrap(), id2);
}

#[test]
fn test_batch_save_campaigns_duplicate_fails_atomically() {
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    let id1 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 1"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let id2 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 2"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.save_campaign(&contributor1, &id1);

    // id1 is already bookmarked, so the whole batch must revert atomically
    // (id2 must not be saved).
    let result = client.try_batch_save_campaigns(&contributor1, &soroban_sdk::vec![&env, id1, id2]);
    assert_eq!(result, Err(Ok(Error::CampaignAlreadyBookmarked)));

    let saved = client.get_saved_campaigns(&contributor1);
    assert_eq!(saved.len(), 1);
    assert_eq!(saved.get(0).unwrap(), id1);
}

#[test]
fn test_batch_save_campaigns_nonexistent_fails_atomically() {
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    let id1 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 1"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    // A nonexistent id in the batch must revert the whole call.
    let result = client.try_batch_save_campaigns(&contributor1, &soroban_sdk::vec![&env, 999, id1]);
    assert_eq!(result, Err(Ok(Error::CampaignNotFound)));

    assert_eq!(
        client.get_saved_campaigns(&contributor1),
        soroban_sdk::vec![&env]
    );
}

#[test]
fn test_clear_saved_campaigns() {
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();

    let id1 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 1"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));
    let id2 = client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(&env, "Campaign 2"),
        String::from_str(&env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ));

    client.save_campaign(&contributor1, &id1);
    client.save_campaign(&contributor1, &id2);
    assert_eq!(client.get_saved_campaigns_count(&contributor1), 2);

    client.clear_saved_campaigns(&contributor1);

    assert_eq!(
        client.get_saved_campaigns(&contributor1),
        soroban_sdk::vec![&env]
    );
    assert_eq!(client.get_saved_campaigns_count(&contributor1), 0);

    // Clearing an already-empty list succeeds with no error.
    client.clear_saved_campaigns(&contributor1);
    assert_eq!(client.get_saved_campaigns_count(&contributor1), 0);
}

// ── #786: every bookmark mutator enforces `require_auth` ─────────────────────
//
// A true negative test — call without authorization, expect a rejection — is
// not expressible here. A failed `require_auth` is a host error that the
// Soroban 20.x native test environment escalates to a non-unwinding panic,
// which aborts the whole test binary: neither `try_*` (it never returns) nor
// `#[should_panic]` (it never unwinds) can observe it. That is why this file
// had no such test.
//
// What is both expressible and sufficient is `env.auths()`. It lists the
// authorizations the last invocation actually *required*, and an entry appears
// only because the contract called `require_auth` for that address. Deleting
// `user.require_auth()` from any of these functions empties the list and fails
// the corresponding test — which is the regression the issue is about.
//
// Each assertion pins the address, the contract, the function name and the
// full argument list, so a guard that authorizes the wrong address, or the
// right address for the wrong call, is caught too.

/// Asserts that the most recent invocation required authorization from
/// `who` for `fn_name` with exactly `args`.
fn assert_required_auth(
    env: &soroban_sdk::Env,
    client: &ProofOfHeartClient,
    who: &Address,
    fn_name: &str,
    args: soroban_sdk::Vec<soroban_sdk::Val>,
) {
    let expected_fn = soroban_sdk::Symbol::new(env, fn_name);
    let found = env.auths().iter().any(|(addr, inv)| {
        *addr == *who
            && match &inv.function {
                soroban_sdk::testutils::AuthorizedFunction::Contract((contract, function, a)) => {
                    contract == &client.address && function == &expected_fn && a == &args
                }
                _ => false,
            }
    });
    assert!(
        found,
        "{} must require authorization from the named wallet with the given arguments",
        fn_name
    );
}

fn bookmark_test_campaign(
    env: &soroban_sdk::Env,
    creator: &Address,
    client: &ProofOfHeartClient,
) -> u32 {
    client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(env, "Campaign"),
        String::from_str(env, "Desc"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ))
}

/// `save_campaign` requires the bookmarking wallet's authorization.
#[test]
fn test_save_campaign_requires_auth_from_the_named_wallet() {
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();
    let id = bookmark_test_campaign(&env, &creator, &client);

    client.save_campaign(&contributor1, &id);

    assert_required_auth(
        &env,
        &client,
        &contributor1,
        "save_campaign",
        (contributor1.clone(), id).into_val(&env),
    );
}

/// `remove_saved_campaign` requires the owning wallet's authorization.
///
/// This is the regression the issue names: without it, anyone could empty
/// someone else's saved list.
#[test]
fn test_remove_saved_campaign_requires_auth_with_exact_arguments() {
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();
    let id = bookmark_test_campaign(&env, &creator, &client);

    client.save_campaign(&contributor1, &id);
    client.remove_saved_campaign(&contributor1, &id);

    assert_required_auth(
        &env,
        &client,
        &contributor1,
        "remove_saved_campaign",
        (contributor1.clone(), id).into_val(&env),
    );
}

/// The batch entry point is gated too — a bulk write must not be an easier
/// path than the single one.
#[test]
fn test_batch_save_campaigns_requires_auth() {
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();
    let id = bookmark_test_campaign(&env, &creator, &client);
    let ids = soroban_sdk::Vec::from_array(&env, [id]);

    client.batch_save_campaigns(&contributor1, &ids);

    assert_required_auth(
        &env,
        &client,
        &contributor1,
        "batch_save_campaigns",
        (contributor1.clone(), ids).into_val(&env),
    );
}

/// Clearing a list is the most destructive bookmark operation, so it gets the
/// same treatment.
#[test]
fn test_clear_saved_campaigns_requires_auth() {
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();
    let id = bookmark_test_campaign(&env, &creator, &client);

    client.save_campaign(&contributor1, &id);
    client.clear_saved_campaigns(&contributor1);

    assert_required_auth(
        &env,
        &client,
        &contributor1,
        "clear_saved_campaigns",
        (contributor1.clone(),).into_val(&env),
    );
}

/// The authorization is demanded for the wallet named in the call, not for
/// whoever happens to be transacting.
///
/// contributor2 invokes, but the list belongs to contributor1, so it is
/// contributor1's authorization that must be required. Under `mock_all_auths`
/// the call still succeeds; what matters is whose signature was demanded.
#[test]
fn test_bookmark_auth_is_demanded_for_the_list_owner_not_the_caller() {
    let (env, _admin, creator, contributor1, contributor2, _token, _token_admin, client) =
        setup_env();
    let id = bookmark_test_campaign(&env, &creator, &client);

    client.save_campaign(&contributor1, &id);

    assert_required_auth(
        &env,
        &client,
        &contributor1,
        "save_campaign",
        (contributor1.clone(), id).into_val(&env),
    );

    // contributor2 never authorized anything on this invocation.
    assert!(
        !env.auths().iter().any(|(addr, _)| *addr == contributor2),
        "save_campaign must not require authorization from an unrelated wallet"
    );
}

/// A read is not a mutation: `get_saved_campaigns` must not demand a
/// signature, or wallets could not display someone else's public list.
#[test]
fn test_reading_saved_campaigns_requires_no_auth() {
    let (env, _admin, creator, contributor1, _c2, _token, _token_admin, client) = setup_env();
    let id = bookmark_test_campaign(&env, &creator, &client);
    client.save_campaign(&contributor1, &id);

    let _ = client.get_saved_campaigns(&contributor1);
    assert!(
        env.auths().is_empty(),
        "get_saved_campaigns must not require authorization"
    );

    let _ = client.get_saved_campaigns_count(&contributor1);
    assert!(
        env.auths().is_empty(),
        "get_saved_campaigns_count must not require authorization"
    );
}
