//! # Milestone release escrow — functional specification
//!
//! A campaign creator may split the payout of a *successful* campaign into
//! milestones. Instead of one `withdraw_funds` call, the platform admin verifies
//! each milestone after review and the creator claims a share of the escrow per
//! verified milestone (#783).
//!
//! ## Actors and entrypoints
//!
//! | Entrypoint          | Caller  | Purpose                                            |
//! |---------------------|---------|----------------------------------------------------|
//! | `set_milestones`    | creator | Define the milestone plan (once)                   |
//! | `verify_milestone`  | admin   | Mark one milestone as verified (once each)         |
//! | `claim_milestone`   | creator | Release a verified milestone's share of the escrow |
//!
//! ## Lifecycle
//!
//! ```text
//!  no plan ── set_milestones(creator, once) ──► plan defined
//!                                               (all verified = false, none claimed)
//!
//!  per milestone:
//!    unverified ── verify_milestone(admin) ──► verified ── claim_milestone(creator) ──► claimed
//!
//!  when the last milestone is claimed:
//!    campaign.funds_withdrawn = true, campaign.is_active = false
//! ```
//!
//! Milestones may be verified and claimed in any order. Claiming requires the
//! campaign to be verified, not cancelled, not fully withdrawn, and to have
//! reached its funding goal.
//!
//! ## Plan rules (`set_milestones`)
//!
//! * The campaign must exist, must not be cancelled or already withdrawn, and
//!   the contract must not be paused. The creator authorises the call.
//! * A plan holds 1 to `MAX_MILESTONES` (10) milestones.
//! * Each milestone has a unique `id`, a non-empty `description` and a
//!   `payout_bps` in `1..=10_000`.
//! * The `payout_bps` values must sum to exactly `BPS_DENOMINATOR` (10_000).
//! * A plan can be set once; a second call fails with `ValidationFailed`.
//! * Whatever `verified` value the caller passes is ignored: every milestone is
//!   stored with `verified = false`.
//! * Once a plan exists, `withdraw_funds` is refused for the campaign
//!   (`ValidationFailed`) — milestones are the only way to release its funds.
//!
//! ## Release math (`claim_milestone`)
//!
//! All arithmetic is checked; overflow reverts with `Overflow`.
//!
//! ```text
//! fee_total       = ceil(amount_raised * fee_bps / 10_000)      // campaign fee_override, else platform fee
//! total_after_fee = amount_raised - fee_total
//!
//! claimable(i)    = floor(total_after_fee * payout_bps(i) / 10_000)
//! fee_slice(i)    = floor(fee_total       * payout_bps(i) / 10_000)
//! ```
//!
//! Rounding dust is not lost. The claim that leaves no other milestone
//! unclaimed (the remaining basis points equal that milestone's own
//! `payout_bps`) absorbs it:
//!
//! ```text
//! claimable(last) = total_after_fee - sum(claimable(j) for already-claimed j)
//! fee_claim(last) = max(fee_slice(last), fee_total - sum(fee_slice(j) for already-claimed j))
//! ```
//!
//! Worked example: `amount_raised = 1000`, fee 300 bps, three milestones of
//! 3333 / 3333 / 3334 bps.
//!
//! ```text
//! fee_total = ceil(1000 * 300 / 10_000) = 30      total_after_fee = 970
//! milestone 1:  floor(970 * 3333 / 10_000) = 323
//! milestone 2:  floor(970 * 3333 / 10_000) = 323
//! milestone 3:  970 - (323 + 323)          = 324  // last claim takes the dust
//! ```
//!
//! On each claim the fee slice is transferred to the admin and the claimable
//! amount to the creator, both in the campaign's token, and
//! `total_raised_global` is reduced by `fee_claim + claimable`. A claim whose
//! `claimable` is zero or negative fails with `NoFundsToWithdraw`.
//!
//! ## Checks-effects-interactions
//!
//! `claim_milestone` validates first, then records the effects — the milestone
//! is marked claimed, the global total is reduced and, for the final claim, the
//! campaign is closed — and only then calls the token's `transfer`. A milestone
//! therefore cannot be claimed twice, even by a token that calls back into the
//! contract (see `tests/test_cei_reentrancy.rs`).
//!
//! ## Errors
//!
//! | Error                    | When                                                        |
//! |--------------------------|-------------------------------------------------------------|
//! | `ValidationFailed`       | Bad plan (size, bps, ids, description), plan already set, or milestone already verified |
//! | `CampaignNotActive`      | Campaign cancelled (or, when planning, already withdrawn)   |
//! | `CampaignNotVerified`    | Claiming on an unverified campaign                          |
//! | `FundsAlreadyWithdrawn`  | Claiming after the campaign was fully withdrawn             |
//! | `NoFundsToWithdraw`      | Nothing raised, or the computed claim is not positive       |
//! | `FundingGoalNotReached`  | Claiming before `amount_raised >= funding_goal`             |
//! | `MilestoneNotFound`      | No plan, or no milestone with that id                       |
//! | `MilestoneNotVerified`   | Claiming before the admin verified the milestone            |
//! | `MilestoneAlreadyClaimed`| The milestone was already claimed                           |
//! | `Overflow`               | Checked arithmetic overflowed                               |
//!
//! Every entrypoint also reverts while the contract is paused, and requires the
//! caller's authorisation (creator for plan and claim, admin for verification).
//!
//! ## Events
//!
//! | Topics                                              | Data                     |
//! |-----------------------------------------------------|--------------------------|
//! | `("milestones_set", campaign_id)`                   | number of milestones     |
//! | `("milestone_verified", campaign_id, milestone_id)` | `()`                     |
//! | `("milestone_claimed", campaign_id, milestone_id)`  | `(claimable, fee_claim)` |
//!
//! ## Storage
//!
//! The plan is one `Vec<Milestone>` per campaign, plus a claimed flag per
//! `(campaign_id, milestone_id)`; both go through the helpers in `storage.rs`,
//! which extend TTL on write (see `docs/rent.md`). `bump_instance_ttl` runs on
//! every state-changing entrypoint here.

use soroban_sdk::{Address, Env, Vec};

use crate::errors::Error;
use crate::lifecycle::{get_campaign_or_error, require_not_paused};
use crate::storage::{
    bump_instance_ttl, decrement_active_campaign_count, get_admin, get_campaign_milestones,
    get_platform_fee, get_total_raised_global, is_milestone_claimed, set_campaign,
    set_campaign_milestones, set_milestone_claimed, set_total_raised_global,
};
use crate::types::Milestone;

/// Maximum milestones per campaign to bound storage.
const MAX_MILESTONES: u32 = 10;

/// Creator sets custom milestone payouts for a campaign (#783).
/// Each milestone's `payout_bps` is a share of the withdrawable funds
/// (after platform fee). The sum must equal `BPS_DENOMINATOR` (10_000).
/// May only be called once per campaign and before any milestone is
/// verified or claimed.
pub(crate) fn set_milestones(
    env: &Env,
    campaign_id: u32,
    milestones: Vec<Milestone>,
) -> Result<(), Error> {
    let campaign = get_campaign_or_error(env, campaign_id)?;
    campaign.creator.require_auth();
    require_not_paused(env)?;

    if campaign.is_cancelled || campaign.funds_withdrawn {
        return Err(Error::CampaignNotActive);
    }
    if milestones.is_empty() || milestones.len() > MAX_MILESTONES {
        return Err(Error::ValidationFailed);
    }
    // Only once.
    if !get_campaign_milestones(env, campaign_id).is_empty() {
        return Err(Error::ValidationFailed);
    }

    let mut total_bps: u32 = 0;
    let mut seen_ids: Vec<u32> = Vec::new(env);
    for m in milestones.iter() {
        if m.payout_bps == 0 || m.payout_bps > crate::BPS_DENOMINATOR {
            return Err(Error::ValidationFailed);
        }
        if m.description.len() == 0 {
            return Err(Error::ValidationFailed);
        }
        if seen_ids.iter().any(|id| id == m.id) {
            return Err(Error::ValidationFailed);
        }
        seen_ids.push_back(m.id);
        total_bps = total_bps.checked_add(m.payout_bps).ok_or(Error::Overflow)?;
    }
    if total_bps != crate::BPS_DENOMINATOR {
        return Err(Error::ValidationFailed);
    }

    // Force verified=false regardless of input.
    let mut normalized: Vec<Milestone> = Vec::new(env);
    for m in milestones.iter() {
        normalized.push_back(Milestone {
            id: m.id,
            description: m.description.clone(),
            payout_bps: m.payout_bps,
            verified: false,
        });
    }

    bump_instance_ttl(env);
    set_campaign_milestones(env, campaign_id, &normalized);
    env.events()
        .publish(("milestones_set", campaign_id), normalized.len());
    Ok(())
}

/// Admin verifies a milestone after community review (#783).
pub(crate) fn verify_milestone(
    env: &Env,
    admin: Address,
    campaign_id: u32,
    milestone_id: u32,
) -> Result<(), Error> {
    crate::lifecycle::assert_admin(env, &admin)?;
    require_not_paused(env)?;
    let campaign = get_campaign_or_error(env, campaign_id)?;
    if campaign.is_cancelled {
        return Err(Error::CampaignNotActive);
    }

    let milestones = get_campaign_milestones(env, campaign_id);
    if milestones.is_empty() {
        return Err(Error::MilestoneNotFound);
    }

    let mut found = false;
    let mut updated: Vec<Milestone> = Vec::new(env);
    for m in milestones.iter() {
        if m.id == milestone_id {
            if m.verified {
                return Err(Error::ValidationFailed);
            }
            found = true;
            updated.push_back(Milestone {
                id: m.id,
                description: m.description.clone(),
                payout_bps: m.payout_bps,
                verified: true,
            });
        } else {
            updated.push_back(m.clone());
        }
    }
    if !found {
        return Err(Error::MilestoneNotFound);
    }

    bump_instance_ttl(env);
    set_campaign_milestones(env, campaign_id, &updated);
    env.events()
        .publish(("milestone_verified", campaign_id, milestone_id), ());
    Ok(())
}

/// Creator claims funds for a verified milestone. Funds are released
/// proportionally to `payout_bps` (#783).
pub(crate) fn claim_milestone(env: &Env, campaign_id: u32, milestone_id: u32) -> Result<(), Error> {
    let mut campaign = get_campaign_or_error(env, campaign_id)?;
    campaign.creator.require_auth();
    require_not_paused(env)?;

    if campaign.is_cancelled {
        return Err(Error::CampaignNotActive);
    }
    if !campaign.is_verified {
        return Err(Error::CampaignNotVerified);
    }
    if campaign.funds_withdrawn {
        return Err(Error::FundsAlreadyWithdrawn);
    }
    if campaign.amount_raised == 0 {
        return Err(Error::NoFundsToWithdraw);
    }
    if campaign.amount_raised < campaign.funding_goal {
        return Err(Error::FundingGoalNotReached);
    }

    let milestones = get_campaign_milestones(env, campaign_id);
    if milestones.is_empty() {
        return Err(Error::MilestoneNotFound);
    }

    let mut target: Option<Milestone> = None;
    for m in milestones.iter() {
        if m.id == milestone_id {
            target = Some(m.clone());
            break;
        }
    }
    let milestone = target.ok_or(Error::MilestoneNotFound)?;
    if !milestone.verified {
        return Err(Error::MilestoneNotVerified);
    }
    if is_milestone_claimed(env, campaign_id, milestone_id) {
        return Err(Error::MilestoneAlreadyClaimed);
    }

    // Calculate fee and total withdrawable (after fee) - same as withdraw_funds.
    let platform_fee = campaign
        .fee_override
        .unwrap_or_else(|| get_platform_fee(env));
    let fee_total = campaign
        .amount_raised
        .checked_mul(platform_fee as i128)
        .and_then(|n| n.checked_add(crate::BPS_CEIL_OFFSET))
        .ok_or(Error::Overflow)?
        / crate::BPS_DENOMINATOR as i128;
    let total_after_fee = campaign.amount_raised - fee_total;

    // Compute sum of already claimed amounts to handle dust on last milestone.
    let mut claimed_bps: u32 = 0;
    let mut claimed_amount: i128 = 0;
    for m in milestones.iter() {
        if is_milestone_claimed(env, campaign_id, m.id) {
            claimed_bps = claimed_bps
                .checked_add(m.payout_bps)
                .ok_or(Error::Overflow)?;
            let amt = total_after_fee
                .checked_mul(m.payout_bps as i128)
                .ok_or(Error::Overflow)?
                / crate::BPS_DENOMINATOR as i128;
            claimed_amount = claimed_amount.checked_add(amt).ok_or(Error::Overflow)?;
        }
    }

    let mut claimable = total_after_fee
        .checked_mul(milestone.payout_bps as i128)
        .ok_or(Error::Overflow)?
        / crate::BPS_DENOMINATOR as i128;

    // If this is the last unclaimed milestone, absorb dust.
    let remaining_bps = crate::BPS_DENOMINATOR
        .checked_sub(claimed_bps)
        .ok_or(Error::Overflow)?;
    let is_last = remaining_bps == milestone.payout_bps;
    if is_last {
        let remaining = total_after_fee
            .checked_sub(claimed_amount)
            .ok_or(Error::Overflow)?;
        if remaining > claimable {
            claimable = remaining;
        }
    }

    let fee_slice = fee_total
        .checked_mul(milestone.payout_bps as i128)
        .ok_or(Error::Overflow)?
        / crate::BPS_DENOMINATOR as i128;
    // For last milestone, fee dust also goes to fee slice remainder.
    let mut fee_claim = fee_slice;
    if is_last {
        let fee_claimed: i128 = {
            let mut sum = 0i128;
            for m in milestones.iter() {
                if is_milestone_claimed(env, campaign_id, m.id) {
                    let f = fee_total
                        .checked_mul(m.payout_bps as i128)
                        .ok_or(Error::Overflow)?
                        / crate::BPS_DENOMINATOR as i128;
                    sum = sum.checked_add(f).ok_or(Error::Overflow)?;
                }
            }
            sum
        };
        let fee_remaining = fee_total.checked_sub(fee_claimed).ok_or(Error::Overflow)?;
        if fee_remaining > fee_claim {
            fee_claim = fee_remaining;
        }
        // Recompute claimable to keep total consistent if fee dust differs.
        let total_remaining = total_after_fee
            .checked_sub(claimed_amount)
            .ok_or(Error::Overflow)?;
        claimable = total_remaining;
    }

    if claimable <= 0 {
        return Err(Error::NoFundsToWithdraw);
    }

    bump_instance_ttl(env);

    // Mark claimed before transfer (CEI).
    set_milestone_claimed(env, campaign_id, milestone_id);

    // Update global total raised: remove fee_slice + claimable proportionally.
    // For last milestone, remaining is removed anyway.
    let to_deduct = fee_claim.checked_add(claimable).ok_or(Error::Overflow)?;
    let total_raised = get_total_raised_global(env);
    set_total_raised_global(
        env,
        total_raised.checked_sub(to_deduct).ok_or(Error::Overflow)?,
    );

    // Check if all milestones claimed -> mark campaign completed.
    let all_claimed = {
        let mut all = true;
        for m in milestones.iter() {
            if m.id == milestone_id {
                continue;
            }
            if !is_milestone_claimed(env, campaign_id, m.id) {
                all = false;
                break;
            }
        }
        all
    };
    if all_claimed {
        campaign.funds_withdrawn = true;
        campaign.is_active = false;
        set_campaign(env, campaign_id, &campaign);
        decrement_active_campaign_count(env);
    }

    let admin_addr = get_admin(env);
    let client = crate::lifecycle::campaign_token_client(env, campaign_id);
    if fee_claim > 0 {
        client.transfer(&env.current_contract_address(), &admin_addr, &fee_claim);
    }
    client.transfer(
        &env.current_contract_address(),
        &campaign.creator,
        &claimable,
    );

    env.events().publish(
        ("milestone_claimed", campaign_id, milestone_id),
        (claimable, fee_claim),
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::tests::helpers::*;
    use crate::Category;
    use crate::Milestone;
    use soroban_sdk::{String, Vec};

    fn make_milestones(env: &soroban_sdk::Env) -> Vec<Milestone> {
        let mut v = Vec::new(env);
        v.push_back(Milestone {
            id: 1,
            description: String::from_str(env, "m1"),
            payout_bps: 5000,
            verified: false,
        });
        v.push_back(Milestone {
            id: 2,
            description: String::from_str(env, "m2"),
            payout_bps: 5000,
            verified: false,
        });
        v
    }

    #[test]
    fn test_milestone_flow_proportional_payout() {
        let (env, admin, creator, c1, _c2, _token, token_admin, client) = setup_env();
        let id = client.create_campaign(&make_params(
            creator.clone(),
            String::from_str(&env, "C"),
            String::from_str(&env, "D"),
            1000,
            30,
            Category::Learner,
            false,
            0,
            0,
        ));
        // need verified and funded
        client.verify_campaign(&id);
        // fund campaign to goal
        token_admin.mint(&c1, &2000);
        client.contribute(&id, &c1, &1000);
        // set milestones (creator)
        client.set_milestones(&id, &make_milestones(&env));
        // verify first milestone via admin
        client.verify_milestone(&admin, &id, &1);
        // claim first milestone should succeed (5000 bps = 50%)
        client.claim_milestone(&id, &1);
        // second milestone not verified yet -> claim should fail
        let res = client.try_claim_milestone(&id, &2);
        assert_eq!(res, Err(Ok(crate::Error::MilestoneNotVerified)));
        // verify and claim second
        client.verify_milestone(&admin, &id, &2);
        client.claim_milestone(&id, &2);
        // campaign should now be withdrawn
        let camp = client.get_campaign(&id);
        assert!(camp.funds_withdrawn);
        assert!(!camp.is_active);
    }

    #[test]
    fn test_set_milestones_requires_bps_sum_10000() {
        let (env, _admin, creator, _c1, _c2, _token, _token_admin, client) = setup_env();
        let id = client.create_campaign(&make_params(
            creator.clone(),
            String::from_str(&env, "C"),
            String::from_str(&env, "D"),
            1000,
            30,
            Category::Learner,
            false,
            0,
            0,
        ));
        let mut bad = Vec::new(&env);
        bad.push_back(Milestone {
            id: 1,
            description: String::from_str(&env, "m1"),
            payout_bps: 4000,
            verified: false,
        });
        bad.push_back(Milestone {
            id: 2,
            description: String::from_str(&env, "m2"),
            payout_bps: 4000,
            verified: false,
        });
        let res = client.try_set_milestones(&id, &bad);
        assert!(res.is_err());
    }

    #[test]
    fn test_withdraw_funds_blocked_when_milestones_exist() {
        let (env, admin, creator, c1, _c2, _token, token_admin, client) = setup_env();
        let id = client.create_campaign(&make_params(
            creator.clone(),
            String::from_str(&env, "C"),
            String::from_str(&env, "D"),
            1000,
            30,
            Category::Learner,
            false,
            0,
            0,
        ));
        client.verify_campaign(&id);
        token_admin.mint(&c1, &2000);
        client.contribute(&id, &c1, &1000);
        client.set_milestones(&id, &make_milestones(&env));
        client.verify_milestone(&admin, &id, &1);
        client.verify_milestone(&admin, &id, &2);
        // withdraw_funds should be blocked for milestone campaign
        let res = client.try_withdraw_funds(&id);
        assert_eq!(res, Err(Ok(crate::Error::ValidationFailed)));
    }
    /// The worked example in the module docs: 3333 / 3333 / 3334 bps over an
    /// escrow of 1000 with a 300 bps fee. Rounding dust lands on the last claim
    /// and the fee slices add up to exactly the fee.
    #[test]
    fn test_milestone_rounding_dust_goes_to_last_claim() {
        let (env, admin, creator, c1, _c2, token, token_admin, client) = setup_env();
        let id = client.create_campaign(&make_params(
            creator.clone(),
            String::from_str(&env, "C"),
            String::from_str(&env, "D"),
            1000,
            30,
            Category::Learner,
            false,
            0,
            0,
        ));
        client.verify_campaign(&id);
        token_admin.mint(&c1, &2000);
        client.contribute(&id, &c1, &1000);

        let mut plan = Vec::new(&env);
        for (mid, bps) in [(1u32, 3333u32), (2, 3333), (3, 3334)] {
            plan.push_back(Milestone {
                id: mid,
                description: String::from_str(&env, "m"),
                payout_bps: bps,
                verified: false,
            });
        }
        client.set_milestones(&id, &plan);

        let mut creator_received = soroban_sdk::vec![&env];
        for mid in 1..=3u32 {
            client.verify_milestone(&admin, &id, &mid);
            let before = token.balance(&creator);
            client.claim_milestone(&id, &mid);
            creator_received.push_back(token.balance(&creator) - before);
        }

        assert_eq!(creator_received, soroban_sdk::vec![&env, 323i128, 323, 324]);
        // fee_total = 30, split 9 / 9 / 12; the creator gets the other 970.
        assert_eq!(token.balance(&admin), 30);
        assert_eq!(token.balance(&creator), 970);
        assert!(client.get_campaign(&id).funds_withdrawn);
    }
}
