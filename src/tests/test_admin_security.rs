//! Security validation tests for admin functions (#993).
//!
//! Tests the access control and parameter validation helpers introduced to
//! prevent unauthorized operations, duplicate signers, invalid quorums, and
//! out-of-bounds percentage values.

#![cfg(test)]

use crate::errors::Error;
use crate::tests::helpers::TestEnv;
use soroban_sdk::{testutils::Address as _, Address};

#[test]
fn test_emergency_pause_signers_rejects_duplicates() {
    let env = TestEnv::default();
    let admin = env.admin.clone();
    let signer1 = Address::generate(&env.env);
    let signer2 = Address::generate(&env.env);

    // Create a list with duplicate addresses
    let mut signers = soroban_sdk::Vec::new(&env.env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());
    signers.push_back(signer1.clone()); // Duplicate

    // Should reject due to duplicate signer
    let result = env.contract.set_emergency_pause_signers(&admin, &signers);
    assert_eq!(result, Err(Error::ValidationFailed));
}

#[test]
fn test_emergency_pause_signers_accepts_unique_addresses() {
    let env = TestEnv::default();
    let admin = env.admin.clone();
    let signer1 = Address::generate(&env.env);
    let signer2 = Address::generate(&env.env);
    let signer3 = Address::generate(&env.env);

    let mut signers = soroban_sdk::Vec::new(&env.env);
    signers.push_back(signer1);
    signers.push_back(signer2);
    signers.push_back(signer3);

    // Should succeed with unique signers
    let result = env.contract.set_emergency_pause_signers(&admin, &signers);
    assert!(result.is_ok());
}

#[test]
fn test_voting_params_rejects_zero_quorum() {
    let env = TestEnv::default();
    let admin = env.admin.clone();

    // Attempt to set quorum to 0 (ungovernable)
    let result = env.contract.set_voting_params(&admin, &0, &5000);
    assert_eq!(result, Err(Error::ValidationFailed));
}

#[test]
fn test_voting_params_rejects_excessive_quorum() {
    let env = TestEnv::default();
    let admin = env.admin.clone();

    // Attempt to set impossibly high quorum (1001 > MAX_REASONABLE_QUORUM)
    let result = env.contract.set_voting_params(&admin, &1001, &5000);
    assert_eq!(result, Err(Error::ValidationFailed));
}

#[test]
fn test_voting_params_accepts_reasonable_quorum() {
    let env = TestEnv::default();
    let admin = env.admin.clone();

    // Valid quorum values should be accepted
    let result1 = env.contract.set_voting_params(&admin, &1, &5000);
    assert!(result1.is_ok());

    let result2 = env.contract.set_voting_params(&admin, &100, &5000);
    assert!(result2.is_ok());

    let result3 = env.contract.set_voting_params(&admin, &1000, &5000);
    assert!(result3.is_ok());
}

#[test]
fn test_voting_params_rejects_threshold_above_100_percent() {
    let env = TestEnv::default();
    let admin = env.admin.clone();

    // Attempt to set approval threshold above 10,000 bps (100%)
    let result = env.contract.set_voting_params(&admin, &10, &10001);
    assert_eq!(result, Err(Error::ValidationFailed));
}

#[test]
fn test_voting_params_accepts_valid_threshold() {
    let env = TestEnv::default();
    let admin = env.admin.clone();

    // Valid thresholds at boundaries
    let result1 = env.contract.set_voting_params(&admin, &10, &0);
    assert!(result1.is_ok());

    let result2 = env.contract.set_voting_params(&admin, &10, &5000);
    assert!(result2.is_ok());

    let result3 = env.contract.set_voting_params(&admin, &10, &10000);
    assert!(result3.is_ok());
}

#[test]
fn test_emergency_pause_signers_rejects_empty_list() {
    let env = TestEnv::default();
    let admin = env.admin.clone();

    let empty_signers = soroban_sdk::Vec::new(&env.env);

    // Should reject empty signer list
    let result = env.contract.set_emergency_pause_signers(&admin, &empty_signers);
    assert_eq!(result, Err(Error::ValidationFailed));
}

#[test]
fn test_duplicate_detection_with_two_identical_addresses() {
    let env = TestEnv::default();
    let admin = env.admin.clone();
    let signer = Address::generate(&env.env);

    let mut signers = soroban_sdk::Vec::new(&env.env);
    signers.push_back(signer.clone());
    signers.push_back(signer.clone()); // Exact duplicate

    let result = env.contract.set_emergency_pause_signers(&admin, &signers);
    assert_eq!(result, Err(Error::ValidationFailed));
}

#[test]
fn test_duplicate_detection_with_three_addresses_last_duplicate() {
    let env = TestEnv::default();
    let admin = env.admin.clone();
    let signer1 = Address::generate(&env.env);
    let signer2 = Address::generate(&env.env);

    let mut signers = soroban_sdk::Vec::new(&env.env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2);
    signers.push_back(signer1); // Duplicate of first

    let result = env.contract.set_emergency_pause_signers(&admin, &signers);
    assert_eq!(result, Err(Error::ValidationFailed));
}
