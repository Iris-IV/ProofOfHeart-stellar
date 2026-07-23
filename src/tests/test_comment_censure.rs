use super::helpers::*;
use crate::{Category, Error};
use soroban_sdk::BytesN;

fn create_campaign(env: &Env, creator: &Address, client: &ProofOfHeartClient) -> u32 {
    let params = make_params(
        creator.clone(),
        String::from_str(env, "Campaign"),
        String::from_str(env, "Description"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0,
    );
    client.create_campaign(&params)
}

#[test]
fn test_censure_comment_sets_flag_and_emits_event() {
    let (env, admin, creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();
    let campaign_id = create_campaign(&env, &creator, &client);
    let comment_hash = BytesN::from_array(&env, &[7u8; 32]);

    assert!(!client.is_comment_censured(&campaign_id, &comment_hash));

    client.censure_comment(&admin, &campaign_id, &comment_hash);

    assert!(client.is_comment_censured(&campaign_id, &comment_hash));

    let last_event = env.events().all().last().unwrap();
    let expected_topics = (
        String::from_str(&env, "comment_censured"),
        campaign_id,
        admin.clone(),
    )
        .into_val(&env);
    assert_eq!(last_event.1, expected_topics);
    let data: BytesN<32> = soroban_sdk::FromVal::from_val(&env, &last_event.2);
    assert_eq!(data, comment_hash);
}

#[test]
fn test_censure_comment_non_admin_fails() {
    let (env, _admin, creator, contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();
    let campaign_id = create_campaign(&env, &creator, &client);
    let comment_hash = BytesN::from_array(&env, &[1u8; 32]);

    let result = client.try_censure_comment(&contributor1, &campaign_id, &comment_hash);
    assert_eq!(result.unwrap_err().unwrap(), Error::NotAuthorized);
    assert!(!client.is_comment_censured(&campaign_id, &comment_hash));
}

#[test]
fn test_censure_comment_missing_campaign_fails() {
    let (env, admin, _creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();
    let comment_hash = BytesN::from_array(&env, &[2u8; 32]);

    let result = client.try_censure_comment(&admin, &999, &comment_hash);
    assert_eq!(result.unwrap_err().unwrap(), Error::CampaignNotFound);
}

#[test]
fn test_censure_comment_twice_fails() {
    let (env, admin, creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();
    let campaign_id = create_campaign(&env, &creator, &client);
    let comment_hash = BytesN::from_array(&env, &[3u8; 32]);

    client.censure_comment(&admin, &campaign_id, &comment_hash);
    let result = client.try_censure_comment(&admin, &campaign_id, &comment_hash);
    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);
    assert!(client.is_comment_censured(&campaign_id, &comment_hash));
}

#[test]
fn test_comment_censure_is_scoped_per_campaign_and_hash() {
    let (env, admin, creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();
    let campaign_a = create_campaign(&env, &creator, &client);
    let campaign_b = create_campaign(&env, &creator, &client);
    let hash_a = BytesN::from_array(&env, &[4u8; 32]);
    let hash_b = BytesN::from_array(&env, &[5u8; 32]);

    client.censure_comment(&admin, &campaign_a, &hash_a);

    assert!(client.is_comment_censured(&campaign_a, &hash_a));
    assert!(!client.is_comment_censured(&campaign_a, &hash_b));
    assert!(!client.is_comment_censured(&campaign_b, &hash_a));
}
