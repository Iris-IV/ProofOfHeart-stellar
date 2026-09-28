# Issue #862: Campaign Cancellation Statistics Clarity

## Summary

The `cancel_campaign` function in `src/campaigns/cancel.rs` removes voting state and bookmarks when a campaign is cancelled, but retains the contributor count until individual refunds are claimed. This causes campaign statistics to remain inflated indefinitely for contributors who never claim their refunds.

## Problem Statement

When a campaign is cancelled:
- ✅ Voting state is properly removed
- ✅ Bookmarks are properly removed
- ❌ Contributor count is retained until individual refunds are claimed
- ❌ Statistics remain inflated for unclaimed refunds

This inconsistency creates confusion about whether cancellation statistics represent:
- **Gross stats**: Including all contributors regardless of refund status
- **Live stats**: Only contributors with active participation

Contributors who never claim their refunds cause stats to remain artificially high indefinitely.

## Expected Behavior

Define a clear policy for cancellation statistics:
1. Clarify whether stats should be "gross" or "live"
2. Implement consistent behavior across all state removal (voting, bookmarks, contributor count)
3. Expose both interpretations if needed:
   - Gross statistics: All historical contributors
   - Live statistics: Only active/current contributors

## Related Files

- `src/campaigns/cancel.rs` - Campaign cancellation logic
- `src/storage.rs` - Storage and state management
- `src/types.rs` - Data type definitions

## Acceptance Criteria

- [ ] Define whether cancellation stats represent gross or live counts
- [ ] Implement consistent behavior for all state removal on cancellation
- [ ] Update documentation to clarify stat definitions
- [ ] Expose both gross and live statistics if applicable
- [ ] Add tests to verify stats behavior during cancellation
- [ ] Handle edge case of unclaimed refunds

## Issue Metadata

- **Assigned to**: @69starman
- **Due Date**: August 31, 2026
- **Program**: Stellar Wave Program - Wave 8
- **Created by**: @overprodigy

## Notes

This issue is part of the Stellar Wave Program's 8th wave. The contributor should ensure the fix is completed on time for maintainer review and to earn associated points in the reward pool.
