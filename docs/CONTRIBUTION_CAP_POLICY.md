# Contribution Cap Policy

## Overview

`max_contribution_per_user` defines the maximum aggregate tokens a single account can contribute to a specific campaign. It is enforced as a **lifetime cumulative cap per campaign**.

---

## Policy Rules & Semantics

1. **Lifetime Cumulative Enforcement**:
   - A contributor's total cumulative contribution across all successful `contribute()` calls cannot exceed `max_contribution_per_user`.
   - Subsequent contributions are checked against `previous_contributed_total + new_amount <= max_contribution_per_user`.

2. **Disabled Cap Semantics (`0`)**:
   - Setting `max_contribution_per_user = 0` disables per-user limits entirely, allowing unlimited contributions per account up to campaign target limits.

3. **Refund Interaction**:
   - Executing `claim_refund()` resets withdrawable contribution balances for failed campaigns, but **does not** erase historical contribution records used for lifetime cap enforcement.
   - This prevents malicious refund/re-contribute looping from exploiting creator-configured per-user limits.

---

## Worked Examples

### Example 1: Single Contribution under Cap
- **Cap**: 1,000 XLM
- **Call 1**: User contributes 600 XLM → **SUCCESS** (Total: 600 XLM)
- **Call 2**: User contributes 400 XLM → **SUCCESS** (Total: 1,000 XLM)
- **Call 3**: User contributes 1 XLM → **REJECTED** (`ExceedsContributionCap`)

### Example 2: Partial Fill to Cap Limit
- **Cap**: 500 XLM
- **Call 1**: User attempts 600 XLM → **REJECTED** (`ExceedsContributionCap`)
- **Call 2**: User contributes 500 XLM → **SUCCESS** (Total: 500 XLM)

### Example 3: Cap Disabled (`0`)
- **Cap**: 0 XLM (Disabled)
- **Call 1**: User contributes 5,000 XLM → **SUCCESS**
- **Call 2**: User contributes 50,000 XLM → **SUCCESS**

---

## Verified Invariant Test Coverage

Refer to `src/tests/test_contribute_caps.rs` for full test suites verifying:
- Single-tx cap exceedance rejection.
- Multi-tx cumulative cap boundary checks.
- Zero-cap unlimited contribution behavior.
- Cap behavior during goal target overflows.
