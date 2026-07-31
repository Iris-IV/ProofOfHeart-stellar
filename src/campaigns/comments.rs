use soroban_sdk::{Address, Env, String};

use crate::errors::Error;
use crate::lifecycle::{get_campaign_or_error, require_not_paused};
use crate::storage::{
    bump_instance_ttl, get_admin, get_campaign_comment_count, is_comment_removed,
    set_campaign_comment_count, set_comment_removed,
};

pub(crate) fn add_campaign_comment(
    env: &Env,
    campaign_id: u32,
    commenter: Address,
    comment_hash: String,
) -> Result<u32, Error> {
    commenter.require_auth();
    require_not_paused(env)?;

    // Just verify the campaign exists
    let _campaign = get_campaign_or_error(env, campaign_id)?;

    bump_instance_ttl(env);

    let count = get_campaign_comment_count(env, campaign_id);
    let comment_id = count.checked_add(1).ok_or(Error::Overflow)?;

    set_campaign_comment_count(env, campaign_id, comment_id);

    env.events().publish(
        ("campaign_comment_added", campaign_id, comment_id),
        (commenter, comment_hash),
    );

    Ok(comment_id)
}

pub(crate) fn remove_campaign_comment(
    env: &Env,
    campaign_id: u32,
    comment_id: u32,
    caller: Address,
) -> Result<(), Error> {
    caller.require_auth();
    require_not_paused(env)?;

    let campaign = get_campaign_or_error(env, campaign_id)?;

    // Only the campaign creator or the admin can remove comments.
    if caller != campaign.creator && caller != get_admin(env) {
        return Err(Error::NotAuthorized);
    }

    let count = get_campaign_comment_count(env, campaign_id);
    if comment_id == 0 || comment_id > count {
        return Err(Error::InvalidCommentId);
    }

    if is_comment_removed(env, campaign_id, comment_id) {
        return Err(Error::CommentAlreadyRemoved);
    }

    bump_instance_ttl(env);
    set_comment_removed(env, campaign_id, comment_id);

    env.events()
        .publish(("campaign_comment_removed", campaign_id, comment_id), caller);

    Ok(())
}
