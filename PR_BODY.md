This pull request implements comprehensive enhancements to `ProofOfHeart-stellar`, bringing stability, event persistence, strict documentation, and precise boundary constraints to the platform.

### Changes Implemented
* **Graceful Failure on Missing Campaigns**: `get_campaign` now returns `Result<Campaign, Error>` applying `ok_or(Error::CampaignNotFound)`. This eradicates panic scenarios when attempting to access a nonexistent or removed campaign dynamically. Associated unit tests have been updated with robust `try_get_campaign` tests.
* **Initialization Events**: The `init` function now persistently triggers an `"initialized"` event right after environmental configurations are registered. This includes details of the globally bounded `platform_fee`, executing `admin`, and the authorized base `token` references.
* **Comprehensive RustDocs**: Applied extensive contextual `///` documentation blocks across all struct typings, categorical enums, parameter variants, and the 23 public contract commands spanning `ProofOfHeart`. These updates encapsulate precise behaviors concerning `# Arguments`, `# Returns`, `# Errors`, and specific `# Authorization` restrictions, fully enabling auto-generated API outputs.
* **Deadline Boundary Constraints**: Injected `test_deadline_boundary` inside `src/test.rs` to securely map Soroban ledger timestamps. This strictly asserts boundaries succeed cleanly exactly at the initialized campaign `deadline`, but yield the appropriate `Error::DeadlinePassed` exactly at `deadline + 1`.

Closes #17, Closes #29, Closes #26, Closes #8
