This pull request fixes one assigned issue in `ProofOfHeart-stellar`, tightening transfer guardrails so terminal campaigns cannot be reassigned.

### Changes Implemented
* **Transfer Guardrail**: `initiate_campaign_transfer` now rejects cancelled or withdrawn campaigns before setting a pending owner.
* **Regression Coverage**: Added a test covering both cancelled and withdrawn campaign transfer attempts.

### Related Issues
* #324
* #323
* #322
* #298

Closes #323
