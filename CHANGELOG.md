# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

### Added

- `remove_personal_cap(campaign_id, contributor)` public entrypoint exposing removal of a contributor's personal contribution cap, restoring the campaign-wide `max_contribution_per_user` as the only bound. Emits `personal_cap_removed` and returns `PersonalCapNotFound` when no cap is set (#503).

### Fixed

- Reverted two accidental merges that shipped a broken duplicate `ProofOfHeartContract` contract (a stray `list_active_campaigns(tag_filter)`, category max-goal cap functions, and a `remove_personal_cap` impl referencing non-existent storage keys), plus orphaned TypeScript SDK files and stray frontend React files with no build setup. The real `ProofOfHeart` contract is now the only contract in the crate.
### Removed

- Removed the dead `BlockContributionCount` storage key variant and its unused `get_block_contribution_count` / `set_block_contribution_count` helpers. Only the per-campaign `BlockCampaignContributionCount` is actually used by the anomaly-detection burst guard (#435).

### Fixed

- `resume_campaign` now checks the global `AutoPaused` flag before `require_active_campaign`, returning early with `ValidationFailed` when the contract is not auto-paused instead of failing on an unrelated campaign-state check. The admin recovery path via `unpause()` also clears `AutoPaused` alongside `Paused` so neither flag can permanently lock the contract (#436).

- `cancel_campaign` now rejects with `GoalMetCancellationNotAllowed` when `amount_raised >= funding_goal` and funds have not yet been withdrawn, preventing rug-pull-adjacent behaviour where a creator could cancel after reaching the goal and force all contributors to self-serve refunds (#164).

- `update_campaign_description` now blocks edits once `amount_raised > 0`, preventing bait-and-switch after contributions (#166).
- `claim_creator_revenue` returns `ValidationFailed` when `revenue_share_percentage > 10000` instead of producing negative math or panicking (#167).
- `init` and `update_platform_fee` now reject `platform_fee` values above `1000` with `InvalidPlatformFee` instead of silently capping them.
- `initiate_campaign_transfer` now rejects cancelled or withdrawn campaigns, keeping ownership transfers off terminal campaigns (#323).
- `resume_campaign` now returns `ValidationFailed` when no pause is active, preventing spurious state writes and `campaign_resumed` events (#348).
- `update_campaign` now emits both the updated title and description in `campaign_updated`, allowing full metadata indexing without extra reads (#349).

### Infrastructure

- Added a `Makefile` with a `build-docker` target utilizing the `stellar/rs-soroban-sdk` image to guarantee WASM binary reproducibility, allowing anyone to verify that the deployed on-chain bytecode matches the source (#533).
- Resolved pre-existing CI debt surfaced by the `fmt` and `clippy` gates added in #403: test fixture missing bindings restored in `src/test.rs` and `src/tests/test_init.rs`, `result` double-move fixed in `src/tests/test_admin.rs`, `cargo fmt --all` drift cleared across `src/issues_test.rs` and `src/lib.rs`, and clippy lints addressed (`manual_div_ceil` in `src/lib.rs`; `dead_code` suppressed on deferred storage helpers pending the DataKey audit in #409). All three CI jobs (`test`, `fmt`, `clippy`) now exit 0 on a clean checkout (#418).

### Refactored

- Extracted `assert_admin(env, caller)` helper; used in `pause`, `unpause`, and `set_voting_params` to provide a single source of truth for admin authorization (#224).

### Documentation

- Added `CHANGELOG.md` and documented the Keep-a-Changelog convention in `CONTRIBUTING.md` (#227).
