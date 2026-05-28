[BUG] vote_on_campaign accepts votes after deadline or after funds withdrawn

Summary
voting::cast_vote only checks is_verified, is_cancelled, is_active, token balance, and dedup. It does not check the campaign deadline or funds_withdrawn flag. Voting on a campaign whose deadline has passed (or whose creator has already withdrawn) is meaningless and pollutes vote counts.

Where
src/voting.rs cast_vote (line ~52)

Fix
Reject if env.ledger().timestamp() > campaign.deadline.
Reject if campaign.funds_withdrawn.
Map both to Error::CampaignNotActive (or a new Error::VotingClosed).
Acceptance criteria
 Tests cover post-deadline and post-withdrawal vote attempts.