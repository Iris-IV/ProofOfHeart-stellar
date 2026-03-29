use soroban_sdk::contracterror;

/// Represents a distinct error type that can occur within the contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotAuthorized = 1,
    CampaignNotFound = 2,
    CampaignNotActive = 3,
    FundingGoalMustBePositive = 4,
    InvalidDuration = 5,
    InvalidRevenueShare = 6,
    RevenueShareOnlyForStartup = 7,
    DeadlinePassed = 8,
    ContributionMustBePositive = 9,
    DeadlineNotPassed = 10,
    FundsAlreadyWithdrawn = 11,
    FundingGoalNotReached = 12,
    NoFundsToWithdraw = 13,
    CampaignAlreadyVerified = 14,
    ValidationFailed = 15,
    AlreadyVoted = 16,
    NotTokenHolder = 17,
    VotingQuorumNotMet = 18,
    VotingThresholdNotMet = 19,
    AlreadyInitialized = 20,
}