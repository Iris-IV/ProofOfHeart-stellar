//! Tag-based campaign discovery entrypoints (#798, #1330):
//! `add_campaign_tag`, `get_campaigns_by_tag` and `get_campaign_tags`.
extern crate alloc;
use alloc::format;

use super::helpers::*;
use crate::storage::TAG_CAMPAIGNS_BUCKET_SIZE;
use crate::{Category, Error, CAMPAIGN_TAG_MAX_LEN, LIST_MAX_LIMIT, MAX_TAGS_PER_CAMPAIGN};
use soroban_sdk::{
    testutils::{AuthorizedFunction, AuthorizedInvocation, Events},
    vec, Address, Env, IntoVal, String, Symbol, TryFromVal,
};

fn create(env: &Env, client: &ProofOfHeartClient<'_>, creator: &Address, title: &str) -> u32 {
    client.create_campaign(&make_params(
        creator.clone(),
        String::from_str(env, title),
        String::from_str(env, "Tag test"),
        1000,
        30,
        Category::Learner,
        false,
        0,
        0i128,
    ))
}

fn tag(env: &Env, s: &str) -> String {
    String::from_str(env, s)
}

fn ids(page: &soroban_sdk::Vec<crate::Campaign>) -> alloc::vec::Vec<u32> {
    page.iter().map(|c| c.id).collect()
}

// ── add_campaign_tag: happy path, auth, event ───────────────────────────────

#[test]
fn test_add_campaign_tag_stores_tag_and_indexes_campaign() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = create(&env, &client, &creator, "Solar");

    client.add_campaign_tag(&id, &tag(&env, "energy"));
    client.add_campaign_tag(&id, &tag(&env, "climate"));

    assert_eq!(
        client.get_campaign_tags(&id),
        vec![&env, tag(&env, "energy"), tag(&env, "climate")]
    );
    let (page, cursor) = client.get_campaigns_by_tag(&tag(&env, "energy"), &0, &10);
    assert_eq!(ids(&page), [id]);
    assert_eq!(cursor, 1);
}

#[test]
fn test_add_campaign_tag_requires_creator_auth() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = create(&env, &client, &creator, "Solar");

    let t = tag(&env, "energy");
    client.add_campaign_tag(&id, &t);

    let auths = env.auths();
    let (auth_addr, invocation) = auths.last().unwrap();
    assert_eq!(auth_addr, &creator);
    assert_eq!(
        invocation,
        &AuthorizedInvocation {
            function: AuthorizedFunction::Contract((
                client.address.clone(),
                Symbol::new(&env, "add_campaign_tag"),
                (id, t).into_val(&env),
            )),
            sub_invocations: Default::default(),
        }
    );
}

#[test]
fn test_add_campaign_tag_rejects_non_creator() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = create(&env, &client, &creator, "Solar");

    // Drop the blanket auth mock: nobody has signed, so the creator's
    // `require_auth` must fail and nothing may be written.
    env.set_auths(&[]);
    assert!(client
        .try_add_campaign_tag(&id, &tag(&env, "energy"))
        .is_err());
    assert_eq!(client.get_campaign_tags(&id).len(), 0);
}

#[test]
fn test_add_campaign_tag_emits_campaign_tag_added() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = create(&env, &client, &creator, "Solar");

    client.add_campaign_tag(&id, &tag(&env, "energy"));

    let events = env.events().all();
    let (contract, topics, data) = events.last().unwrap();
    assert_eq!(contract, client.address);
    assert_eq!(
        topics,
        (String::from_str(&env, "campaign_tag_added"), id).into_val(&env)
    );
    assert_eq!(
        String::try_from_val(&env, &data).unwrap(),
        tag(&env, "energy")
    );
}

// ── add_campaign_tag: guards ────────────────────────────────────────────────

#[test]
fn test_add_campaign_tag_unknown_campaign() {
    let (env, _admin, _creator, _, _, _, _, client) = setup_env();
    let res = client.try_add_campaign_tag(&999, &tag(&env, "energy"));
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotFound);
}

#[test]
fn test_add_campaign_tag_rejected_while_paused() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = create(&env, &client, &creator, "Solar");
    client.pause();

    let res = client.try_add_campaign_tag(&id, &tag(&env, "energy"));
    assert_eq!(res.unwrap_err().unwrap(), Error::ContractPaused);

    client.unpause();
    client.add_campaign_tag(&id, &tag(&env, "energy"));
}

#[test]
fn test_add_campaign_tag_rejected_for_cancelled_campaign() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = create(&env, &client, &creator, "Solar");
    client.cancel_campaign(&id);

    let res = client.try_add_campaign_tag(&id, &tag(&env, "energy"));
    assert_eq!(res.unwrap_err().unwrap(), Error::CampaignNotActive);
}

#[test]
fn test_add_campaign_tag_rejects_empty_tag() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = create(&env, &client, &creator, "Solar");
    let res = client.try_add_campaign_tag(&id, &tag(&env, ""));
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
}

#[test]
fn test_add_campaign_tag_length_boundary() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = create(&env, &client, &creator, "Solar");

    let max = "a".repeat(CAMPAIGN_TAG_MAX_LEN as usize);
    client.add_campaign_tag(&id, &tag(&env, &max));

    let too_long = "b".repeat(CAMPAIGN_TAG_MAX_LEN as usize + 1);
    let res = client.try_add_campaign_tag(&id, &tag(&env, &too_long));
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
    assert_eq!(client.get_campaign_tags(&id).len(), 1);
}

#[test]
fn test_add_campaign_tag_rejects_duplicate() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = create(&env, &client, &creator, "Solar");
    client.add_campaign_tag(&id, &tag(&env, "energy"));

    let res = client.try_add_campaign_tag(&id, &tag(&env, "energy"));
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);

    // The index must not list the campaign twice.
    let (page, _) = client.get_campaigns_by_tag(&tag(&env, "energy"), &0, &10);
    assert_eq!(ids(&page), [id]);
}

#[test]
fn test_add_campaign_tag_rejects_over_limit() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = create(&env, &client, &creator, "Solar");
    for i in 0..MAX_TAGS_PER_CAMPAIGN {
        client.add_campaign_tag(&id, &tag(&env, &format!("t{i}")));
    }

    let res = client.try_add_campaign_tag(&id, &tag(&env, "one-too-many"));
    assert_eq!(res.unwrap_err().unwrap(), Error::ValidationFailed);
    assert_eq!(client.get_campaign_tags(&id).len(), MAX_TAGS_PER_CAMPAIGN);
}

// ── get_campaigns_by_tag / get_campaign_tags ────────────────────────────────

#[test]
fn test_get_campaign_tags_empty_for_untagged_or_unknown_campaign() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = create(&env, &client, &creator, "Solar");
    assert_eq!(client.get_campaign_tags(&id).len(), 0);
    assert_eq!(client.get_campaign_tags(&999).len(), 0);
}

#[test]
fn test_get_campaigns_by_tag_invalid_or_unknown_tag_returns_empty_page() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let id = create(&env, &client, &creator, "Solar");
    client.add_campaign_tag(&id, &tag(&env, "energy"));

    let too_long = "x".repeat(CAMPAIGN_TAG_MAX_LEN as usize + 1);
    for t in ["", "unknown", "Energy", too_long.as_str()] {
        let (page, cursor) = client.get_campaigns_by_tag(&tag(&env, t), &3, &10);
        assert_eq!(page.len(), 0, "tag {t:?}");
        assert_eq!(cursor, 3, "cursor must be echoed back for {t:?}");
    }
}

#[test]
fn test_get_campaigns_by_tag_paginates_in_tagging_order() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    let a = create(&env, &client, &creator, "A");
    let b = create(&env, &client, &creator, "B");
    let c = create(&env, &client, &creator, "C");
    let untagged = create(&env, &client, &creator, "D");
    let t = tag(&env, "energy");
    // Tag out of creation order: the index follows tagging order.
    for id in [c, a, b] {
        client.add_campaign_tag(&id, &t);
    }

    let (page, cursor) = client.get_campaigns_by_tag(&t, &0, &2);
    assert_eq!(ids(&page), [c, a]);
    assert_eq!(cursor, 2);
    let (page, cursor) = client.get_campaigns_by_tag(&t, &cursor, &2);
    assert_eq!(ids(&page), [b]);
    assert_eq!(cursor, 3);
    let (page, cursor) = client.get_campaigns_by_tag(&t, &cursor, &2);
    assert_eq!(page.len(), 0);
    assert_eq!(cursor, 3);
    assert!(!ids(&client.get_campaigns_by_tag(&t, &0, &10).0).contains(&untagged));

    // `limit = 0` is an empty page.
    assert_eq!(client.get_campaigns_by_tag(&t, &0, &0).0.len(), 0);
}

#[test]
fn test_get_campaigns_by_tag_crosses_bucket_boundary() {
    let (env, _admin, creator, _, _, _, _, client) = setup_env();
    env.cost_estimate().budget().reset_unlimited();

    let t = tag(&env, "bulk");
    let total = TAG_CAMPAIGNS_BUCKET_SIZE + 5;
    for i in 0..total {
        let id = create(&env, &client, &creator, &format!("Bulk {i}"));
        client.add_campaign_tag(&id, &t);
    }

    // A single page straddling the bucket 0 / bucket 1 boundary.
    let start = TAG_CAMPAIGNS_BUCKET_SIZE - 2;
    let (page, cursor) = client.get_campaigns_by_tag(&t, &start, &4);
    assert_eq!(
        ids(&page),
        [start + 1, start + 2, start + 3, start + 4] // campaign ids are 1-based
    );
    assert_eq!(cursor, start + 4);

    // Walking every page yields each campaign exactly once, in order.
    let mut seen = alloc::vec::Vec::new();
    let mut offset = 0;
    loop {
        let (page, next) = client.get_campaigns_by_tag(&t, &offset, &LIST_MAX_LIMIT);
        if page.is_empty() {
            break;
        }
        seen.extend(ids(&page));
        offset = next;
    }
    let expected: alloc::vec::Vec<u32> = (1..=total).collect();
    assert_eq!(seen, expected);

    // `limit` is capped at LIST_MAX_LIMIT.
    let (page, _) = client.get_campaigns_by_tag(&t, &0, &u32::MAX);
    assert_eq!(page.len(), LIST_MAX_LIMIT);
}
