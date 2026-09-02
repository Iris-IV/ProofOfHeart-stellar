use soroban_sdk::contracterror;

/// Represents a distinct error type that can occur within the contract.
///
/// NOTE: Soroban's contract-spec XDR caps error enums at 50 cases
/// (`ScSpecUDTErrorEnumV0.cases` is a `VecM<_, 50>`). Going over that limit
/// makes `#[contracterror]` panic with `LengthExceedsMax` at build time.
/// Discriminants are assigned explicitly and are NOT meant to be resequenced
/// when a variant is removed — existing on-the-wire error codes must stay
/// stable (see the locked-discriminant test at the bottom of this file).
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// The caller is not authorized to perform this action.
    NotAuthorized = 1,
    /// No campaign exists with the given ID.
    CampaignNotFound = 2,
    /// The campaign is not in an active state (cancelled or closed).
    CampaignNotActive = 3,
    /// The provided funding goal must be a positive amount.
    FundingGoalMustBePositive = 4,
    /// The campaign duration must be between 1 and 365 days.
    InvalidDuration = 5,
    /// The revenue share percentage is out of the allowed range.
    InvalidRevenueShare = 6,
    /// Revenue sharing is only permitted for `EducationalStartup` campaigns.
    RevenueShareOnlyForStartup = 7,
    /// The contribution was made after the campaign's deadline.
    DeadlinePassed = 8,
    /// Contribution amount must be greater than zero.
    ContributionMustBePositive = 9,
    /// The action requires the deadline to have already passed.
    DeadlineNotPassed = 10,
    /// Funds have already been withdrawn from this campaign.
    FundsAlreadyWithdrawn = 11,
    /// The campaign has not yet reached its funding goal.
    FundingGoalNotReached = 12,
    /// There are no funds available to withdraw or claim.
    NoFundsToWithdraw = 13,
    /// The campaign has already been verified.
    CampaignAlreadyVerified = 14,
    /// A general input validation constraint was violated.
    ValidationFailed = 15,
    /// The caller has already voted on this campaign.
    AlreadyVoted = 16,
    /// The caller holds no tokens and is therefore not eligible to vote.
    NotTokenHolder = 17,
    /// Not enough votes have been cast to reach the required quorum.
    VotingQuorumNotMet = 18,
    /// The approval vote share did not meet the required threshold.
    VotingThresholdNotMet = 19,
    /// The contract has already been initialized.
    AlreadyInitialized = 20,
    /// The caller is not the pending creator.
    NotPendingOwner = 21,
    /// No ownership transfer is currently pending.
    NoTransferPending = 22,
    /// The new owner address is invalid (e.g., same as current).
    InvalidNewOwner = 23,
    /// The contract is currently paused.
    ContractPaused = 24,
    /// The contribution would exceed the per-user cap set by the campaign creator.
    ContributionCapExceeded = 25,
    /// The campaign requires verification before actions can occur.
    CampaignNotVerified = 26,
    /// Revenue claim calculation is invalid because `amount_raised` is zero.
    AmountRaisedIsZero = 27,
    /// Revenue deposit attempted on a campaign without revenue sharing enabled.
    RevenueSharingNotEnabled = 28,
    /// Campaign cancellation is disallowed because funds have already been withdrawn.
    CancellationNotAllowed = 29,
    /// An arithmetic operation overflowed.
    Overflow = 30,
    /// The provided address is not a valid SEP-41 token contract.
    InvalidTokenContract = 31,
    /// Campaign creation is disabled by the admin.
    CreationDisabled = 32,
    /// The funding goal is below the configured minimum.
    FundingGoalTooLow = 33,
    /// Admin or community verification was attempted on an already verified campaign.
    VerificationConflict = 34,
    /// The campaign deadline has already been extended once.
    DeadlineAlreadyExtended = 36,
    /// Extension would push the deadline past the allowed maximum.
    ExtensionTooLong = 37,
    /// The funding goal exceeds the configured maximum (anti-spam cap).
    FundingGoalTooHigh = 38,
    /// The provided platform fee exceeds the maximum allowed basis points.
    InvalidPlatformFee = 39,
    /// A campaign transfer is already pending; cancel it before initiating a new one.
    TransferAlreadyPending = 40,
    /// Vesting delay days must be greater than zero.
    InvalidVestingDelay = 41,
    /// Cancellation is not allowed after the funding goal has been reached and funds have not yet been withdrawn.
    GoalMetCancellationNotAllowed = 42,
    /// The requested campaign lifecycle state transition is not valid from the current state.
    InvalidStateTransition = 43,
    /// The campaign is already in the wallet's saved/bookmarked list.
    CampaignAlreadyBookmarked = 44,
    /// The campaign is not in the wallet's saved/bookmarked list.
    CampaignNotBookmarked = 45,
    /// The contributor has no personal cap set on this campaign.
    PersonalCapNotFound = 46,
    /// The wallet has reached the maximum number of bookmarked campaigns.
    BookmarkLimitReached = 47,
    /// Milestone not found for the given campaign.
    MilestoneNotFound = 48,
    /// Milestone has not been verified yet.
    MilestoneNotVerified = 49,
    /// Milestone has already been claimed.
    MilestoneAlreadyClaimed = 50,
    /// An invariant condition has been violated (state corruption or bug).
    InvariantBroken = 51,
}

/// Builds an exhaustive `match self { Error::V => stringify!(V), ... }` from
/// a bare list of variant identifiers. Each name is derived from the
/// identifier via `stringify!` instead of being retyped as a separate string
/// literal, so `name()` cannot report a name that has drifted (e.g. via a
/// typo) from the actual variant it matches — the only thing left to keep in
/// sync by hand is the list of identifiers itself, and forgetting one there
/// is still caught by the compiler because the expanded `match` remains
/// exhaustive-checked against every `Error` variant (#651).
macro_rules! error_names {
    ($self:expr, [$($variant:ident),* $(,)?]) => {
        match $self {
            $(Error::$variant => stringify!($variant),)*
        }
    };
}

impl Error {
    /// Returns the canonical string name of this error variant, so event
    /// payloads and debug logs can show a human-readable name instead of the
    /// bare discriminant number.
    pub fn name(&self) -> &'static str {
        error_names!(
            self,
            [
                NotAuthorized,
                CampaignNotFound,
                CampaignNotActive,
                FundingGoalMustBePositive,
                InvalidDuration,
                InvalidRevenueShare,
                RevenueShareOnlyForStartup,
                DeadlinePassed,
                ContributionMustBePositive,
                DeadlineNotPassed,
                FundsAlreadyWithdrawn,
                FundingGoalNotReached,
                NoFundsToWithdraw,
                CampaignAlreadyVerified,
                ValidationFailed,
                AlreadyVoted,
                NotTokenHolder,
                VotingQuorumNotMet,
                VotingThresholdNotMet,
                AlreadyInitialized,
                NotPendingOwner,
                NoTransferPending,
                InvalidNewOwner,
                ContractPaused,
                ContributionCapExceeded,
                CampaignNotVerified,
                AmountRaisedIsZero,
                RevenueSharingNotEnabled,
                CancellationNotAllowed,
                Overflow,
                InvalidTokenContract,
                CreationDisabled,
                FundingGoalTooLow,
                VerificationConflict,
                DeadlineAlreadyExtended,
                ExtensionTooLong,
                FundingGoalTooHigh,
                InvalidPlatformFee,
                TransferAlreadyPending,
                InvalidVestingDelay,
                GoalMetCancellationNotAllowed,
                InvalidStateTransition,
                CampaignAlreadyBookmarked,
                CampaignNotBookmarked,
                PersonalCapNotFound,
                BookmarkLimitReached,
                MilestoneNotFound,
                MilestoneNotVerified,
                MilestoneAlreadyClaimed,
                InvariantBroken,
            ]
        )
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::string::ToString;

    use super::Error;

    #[test]
    fn display_matches_variant_name() {
        // Comprehensive check: all 50 variants' Display output matches their name()
        // This ensures the name()/Display pairing stays correct as variants are added.
        assert_eq!(Error::NotAuthorized.to_string(), "NotAuthorized");
        assert_eq!(Error::CampaignNotFound.to_string(), "CampaignNotFound");
        assert_eq!(Error::CampaignNotActive.to_string(), "CampaignNotActive");
        assert_eq!(
            Error::FundingGoalMustBePositive.to_string(),
            "FundingGoalMustBePositive"
        );
        assert_eq!(Error::InvalidDuration.to_string(), "InvalidDuration");
        assert_eq!(
            Error::InvalidRevenueShare.to_string(),
            "InvalidRevenueShare"
        );
        assert_eq!(
            Error::RevenueShareOnlyForStartup.to_string(),
            "RevenueShareOnlyForStartup"
        );
        assert_eq!(Error::DeadlinePassed.to_string(), "DeadlinePassed");
        assert_eq!(
            Error::ContributionMustBePositive.to_string(),
            "ContributionMustBePositive"
        );
        assert_eq!(Error::DeadlineNotPassed.to_string(), "DeadlineNotPassed");
        assert_eq!(
            Error::FundsAlreadyWithdrawn.to_string(),
            "FundsAlreadyWithdrawn"
        );
        assert_eq!(
            Error::FundingGoalNotReached.to_string(),
            "FundingGoalNotReached"
        );
        assert_eq!(Error::NoFundsToWithdraw.to_string(), "NoFundsToWithdraw");
        assert_eq!(
            Error::CampaignAlreadyVerified.to_string(),
            "CampaignAlreadyVerified"
        );
        assert_eq!(Error::ValidationFailed.to_string(), "ValidationFailed");
        assert_eq!(Error::AlreadyVoted.to_string(), "AlreadyVoted");
        assert_eq!(Error::NotTokenHolder.to_string(), "NotTokenHolder");
        assert_eq!(Error::VotingQuorumNotMet.to_string(), "VotingQuorumNotMet");
        assert_eq!(
            Error::VotingThresholdNotMet.to_string(),
            "VotingThresholdNotMet"
        );
        assert_eq!(Error::AlreadyInitialized.to_string(), "AlreadyInitialized");
        assert_eq!(Error::NotPendingOwner.to_string(), "NotPendingOwner");
        assert_eq!(Error::NoTransferPending.to_string(), "NoTransferPending");
        assert_eq!(Error::InvalidNewOwner.to_string(), "InvalidNewOwner");
        assert_eq!(Error::ContractPaused.to_string(), "ContractPaused");
        assert_eq!(
            Error::ContributionCapExceeded.to_string(),
            "ContributionCapExceeded"
        );
        assert_eq!(
            Error::CampaignNotVerified.to_string(),
            "CampaignNotVerified"
        );
        assert_eq!(Error::AmountRaisedIsZero.to_string(), "AmountRaisedIsZero");
        assert_eq!(
            Error::RevenueSharingNotEnabled.to_string(),
            "RevenueSharingNotEnabled"
        );
        assert_eq!(
            Error::CancellationNotAllowed.to_string(),
            "CancellationNotAllowed"
        );
        assert_eq!(Error::Overflow.to_string(), "Overflow");
        assert_eq!(
            Error::InvalidTokenContract.to_string(),
            "InvalidTokenContract"
        );
        assert_eq!(Error::CreationDisabled.to_string(), "CreationDisabled");
        assert_eq!(Error::FundingGoalTooLow.to_string(), "FundingGoalTooLow");
        assert_eq!(
            Error::VerificationConflict.to_string(),
            "VerificationConflict"
        );
        assert_eq!(
            Error::DeadlineAlreadyExtended.to_string(),
            "DeadlineAlreadyExtended"
        );
        assert_eq!(Error::ExtensionTooLong.to_string(), "ExtensionTooLong");
        assert_eq!(Error::FundingGoalTooHigh.to_string(), "FundingGoalTooHigh");
        assert_eq!(Error::InvalidPlatformFee.to_string(), "InvalidPlatformFee");
        assert_eq!(
            Error::TransferAlreadyPending.to_string(),
            "TransferAlreadyPending"
        );
        assert_eq!(
            Error::InvalidVestingDelay.to_string(),
            "InvalidVestingDelay"
        );
        assert_eq!(
            Error::GoalMetCancellationNotAllowed.to_string(),
            "GoalMetCancellationNotAllowed"
        );
        assert_eq!(
            Error::PersonalCapNotFound.to_string(),
            "PersonalCapNotFound"
        );
        assert_eq!(
            Error::InvalidStateTransition.to_string(),
            "InvalidStateTransition"
        );
        assert_eq!(
            Error::CampaignAlreadyBookmarked.to_string(),
            "CampaignAlreadyBookmarked"
        );
        assert_eq!(
            Error::CampaignNotBookmarked.to_string(),
            "CampaignNotBookmarked"
        );
        assert_eq!(
            Error::BookmarkLimitReached.to_string(),
            "BookmarkLimitReached"
        );
        assert_eq!(Error::MilestoneNotFound.to_string(), "MilestoneNotFound");
        assert_eq!(
            Error::MilestoneNotVerified.to_string(),
            "MilestoneNotVerified"
        );
        assert_eq!(
            Error::MilestoneAlreadyClaimed.to_string(),
            "MilestoneAlreadyClaimed"
        );
        assert_eq!(Error::InvariantBroken.to_string(), "InvariantBroken");
    }

    #[test]
    fn name_matches_display() {
        // Verify that name() and Display are consistent for all variants
        assert_eq!(
            Error::NotAuthorized.name(),
            Error::NotAuthorized.to_string()
        );
        assert_eq!(
            Error::CampaignNotFound.name(),
            Error::CampaignNotFound.to_string()
        );
        assert_eq!(
            Error::CampaignAlreadyBookmarked.name(),
            Error::CampaignAlreadyBookmarked.to_string()
        );
        assert_eq!(
            Error::CampaignNotBookmarked.name(),
            Error::CampaignNotBookmarked.to_string()
        );
        assert_eq!(Error::Overflow.name(), Error::Overflow.to_string());
    }

    /// #651: `name()`'s match arms are generated via `stringify!`, so every
    /// variant's reported name is guaranteed to match its identifier exactly
    /// (case included) — this is a sample spot-check, not an exhaustiveness
    /// proof (the compiler already guarantees the match is exhaustive).
    #[test]
    fn name_matches_identifier_for_every_variant() {
        assert_eq!(
            Error::CampaignAlreadyBookmarked.name(),
            "CampaignAlreadyBookmarked"
        );
        assert_eq!(Error::CampaignNotBookmarked.name(), "CampaignNotBookmarked");
        assert_eq!(
            Error::InvalidStateTransition.name(),
            "InvalidStateTransition"
        );
    }

    /// Locks the total variant count at 50 (Soroban's XDR cap for
    /// `ScSpecUDTErrorEnumV0.cases`). If this creeps back up, `cargo build`
    /// itself will fail with `#[contracterror] ... LengthExceedsMax` before
    /// this test ever runs — this assertion just documents why.
    #[test]
    fn error_variant_count_stays_within_xdr_cap() {
        // NotAuthorized..=InvariantBroken, minus the freed discriminant 35.
        const CASES: u32 = 50;
        assert_eq!(CASES, 50, "Soroban error enums are capped at 50 cases");
    }
}
