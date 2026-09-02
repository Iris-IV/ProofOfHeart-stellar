# Fix: quorum cap can trap campaigns in permanent verification failure

## Summary

Issue #883: `src/voting.rs` currently permits `MAX_VOTES_QUORUM = 1000` even when the actual number of eligible voters in a campaign or community can never reach that level.

This creates a governance edge case where the configured quorum can be higher than the maximum feasible voter population, leaving a campaign permanently unable to satisfy verification. In practice, this means the community can become locked out of a valid verification path without any path to recover.

## Root cause

The quorum boundary is defined as a fixed constant in `src/voting.rs`:

- `MAX_VOTES_QUORUM = 1000`

The contract validates quorum using a simple upper bound without grounding that value in a documented population model for eligible voters. If the number of real eligible voters is below 1000, then a quorum requirement of 1000 is unreachable, and the verification flow can become permanently impossible.

This is especially risky because the config also serves as a governance parameter. Once the quorum is set too high relative to the actual participant set, campaigns cannot reach the required votes even when the community is otherwise healthy.

## Why this matters

The issue is not just theoretical. A fixed 1000-vote quorum can exceed the maximum realistic voter count for:

- small or early-stage communities,
- local or niche campaign cohorts,
- low-participation governance contexts,
- any environment where the eligible voter set is intentionally capped below 1000.

Once that happens, the quorum gate can never be met, making community verification impossible for the life of the campaign configuration.

## Proposed fix

The contract should not allow an unbounded quorum value without evidence that the target population can support it.

Preferred approaches:

1. Tie the maximum quorum to a documented eligibility model.
   - Example: “quorum may not exceed the number of eligible voters as derived from the configured community population model.”
   - This should be enforced by validation code, not a fixed constant alone.

2. Expose an emergency override.
   - Add a narrowly scoped admin-controlled override path for exceptional situations.
   - Require explicit governance or admin action and document the conditions under which it is permissible.

3. Add explicit validation and runtime checks.
   - Reject quorum settings above the largest population supported by the documented model.
   - Include a clear failure mode when a configured quorum is impossible to satisfy.

## Recommended direction

The strongest fix is to validate quorum against a defined voter population model and then preserve a limited emergency override for exceptional operational recovery. A constant of 1000 is not a safe default unless the protocol explicitly defines a larger or bounded population model that makes that value reachable.

## Verification

The issue should be considered resolved only when:

- the maximum quorum is validated against the documented population model,
- the impossible-quorum case is covered by a regression test,
- and any emergency override is explicitly gated and explained in the docs.
