use soroban_sdk::contracterror;

/// Represents a distinct error type that can occur within the contract.
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
}
