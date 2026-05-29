 [BUG] init silently clamps platform_fee instead of returning an error

Summary
init accepts any u32 for platform_fee and silently clamps it to PLATFORM_FEE_MAX_BPS (1000). The deployer asks for 12% and gets 10% with no signal.

let valid_fee = if platform_fee > PLATFORM_FEE_MAX_BPS { PLATFORM_FEE_MAX_BPS } else { platform_fee };
Where
src/lib.rs init (line 75) and the same pattern in update_platform_fee (line 700).

Fix
Return Err(Error::InvalidPlatformFee) (new error variant) when the value exceeds the cap. Same for update_platform_fee.

Acceptance criteria
 init with platform_fee=2000 returns the new error.
 update_platform_fee(2000) returns the new error.
 Existing tests updated; behavior documented in CHANGELOG.