## Overview
This PR fixes an issue where `max_contribution_per_user` was not properly enforced over multiple transactions. The `check_contribution_caps` logic now correctly compares `amount + current_lifetime_contribution` against `max_contribution_per_user` instead of just the single transaction `amount`.

## Related Issue
Closes #554

## Changes
- **[MODIFY]** `src/contributions.rs`
  - Ensure `check_contribution_caps` checks `current_lifetime_contribution + amount > campaign.max_contribution_per_user`.
- **[MODIFY]** `src/tests/test_contribute_caps.rs`
  - Verify enforcement logic works correctly across multiple contributions and refunds.

## Verification Results
| Acceptance Criteria | Status |
|---|---|
| Single transaction respects cap | ✅ |
| Multiple transactions respect cap | ✅ |
| `lifetime` contribution logic functions correctly | ✅ |
| Existing tests pass | ✅ |
