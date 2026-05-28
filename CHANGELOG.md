# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

### Fixed
- `update_campaign_description` now blocks edits once `amount_raised > 0`, preventing bait-and-switch after contributions (#166).
- `claim_creator_revenue` returns `ValidationFailed` when `revenue_share_percentage > 10000` instead of producing negative math or panicking (#167).
- `initiate_campaign_transfer` now rejects cancelled or withdrawn campaigns, keeping ownership transfers off terminal campaigns (#323).
- `resume_campaign` now returns `ValidationFailed` when no pause is active, preventing spurious state writes and `campaign_resumed` events (#348).
- `update_campaign` now emits both the updated title and description in `campaign_updated`, allowing full metadata indexing without extra reads (#349).

### Refactored
- Extracted `assert_admin(env, caller)` helper; used in `pause`, `unpause`, and `set_voting_params` to provide a single source of truth for admin authorization (#224).

### Documentation
- Added `CHANGELOG.md` and documented the Keep-a-Changelog convention in `CONTRIBUTING.md` (#227).
