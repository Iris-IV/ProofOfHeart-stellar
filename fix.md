#162 [BUG] admin_verify and verify_with_votes do not check is_active or is_cancelled
Repo Avatar
Iris-IV/ProofOfHeart-stellar
Summary
voting::admin_verify and voting::verify_with_votes flip is_verified=true regardless of the campaign's active/cancel state. A cancelled campaign can be marked verified — confusing for indexers and front-end consumers.

Where
src/voting.rs admin_verify (line ~100)
src/voting.rs verify_with_votes (line ~125)
Fix
Before flipping the bit, return Error::CampaignNotActive if is_cancelled || !is_active.

Acceptance criteria
 Negative tests covering both functions on a cancelled campaign.
 No state regression on the happy path.