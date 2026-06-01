# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

### Fixed

- `update_campaign_description` now blocks edits once `amount_raised > 0`, preventing bait-and-switch after contributions (#166).
- `claim_creator_revenue` returns `ValidationFailed` when `revenue_share_percentage > 10000` instead of producing negative math or panicking (#167).
- `init` and `update_platform_fee` now reject `platform_fee` values above `1000` with `InvalidPlatformFee` instead of silently capping them.
- `initiate_campaign_transfer` now rejects cancelled or withdrawn campaigns, keeping ownership transfers off terminal campaigns (#323).
- `resume_campaign` now returns `ValidationFailed` when no pause is active, preventing spurious state writes and `campaign_resumed` events (#348).
- `update_campaign` now emits both the updated title and description in `campaign_updated`, allowing full metadata indexing without extra reads (#349).

### Infrastructure

- Resolved pre-existing CI debt surfaced by the `fmt` and `clippy` gates added in #403: test fixture missing bindings restored in `src/test.rs` and `src/tests/test_init.rs`, `result` double-move fixed in `src/tests/test_admin.rs`, `cargo fmt --all` drift cleared across `src/issues_test.rs` and `src/lib.rs`, and clippy lints addressed (`manual_div_ceil` in `src/lib.rs`; `dead_code` suppressed on deferred storage helpers pending the DataKey audit in #409). All three CI jobs (`test`, `fmt`, `clippy`) now exit 0 on a clean checkout (#418).

### Refactored

- Extracted `assert_admin(env, caller)` helper; used in `pause`, `unpause`, and `set_voting_params` to provide a single source of truth for admin authorization (#224).

### Documentation

- Added `CHANGELOG.md` and documented the Keep-a-Changelog convention in `CONTRIBUTING.md` (#227).
