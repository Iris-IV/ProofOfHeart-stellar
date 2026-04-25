# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

### Fixed
- `update_campaign_description` now blocks edits once `amount_raised > 0`, preventing bait-and-switch after contributions (#166).
- `claim_creator_revenue` returns `ValidationFailed` when `revenue_share_percentage > 10000` instead of producing negative math or panicking (#167).

### Refactored
- Extracted `assert_admin(env, caller)` helper; used in `pause`, `unpause`, and `set_voting_params` to provide a single source of truth for admin authorization (#224).

### Documentation
- Added `CHANGELOG.md` and documented the Keep-a-Changelog convention in `CONTRIBUTING.md` (#227).
