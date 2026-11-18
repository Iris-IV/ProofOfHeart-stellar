extern crate alloc;

use super::helpers::*;
use crate::{storage::set_version, Error, CONTRACT_VERSION};

#[test]
fn test_migrate_from_v1_to_v2() {
    let (env, admin, _creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();

    // Simulate an older contract at version 1
    env.as_contract(&client.address, || {
        set_version(&env, 1);
    });

    // Verify that get_version returns 1 before migration
    let version_before = client.get_version();
    assert_eq!(version_before, 1);

    // Perform migration with correct expected old version
    let result = client.migrate(&admin, &1);
    assert!(result.is_ok());

    // Verify that get_version returns the new version after migration
    let version_after = client.get_version();
    assert_eq!(version_after, CONTRACT_VERSION);
    assert_eq!(version_after, 2);
}

#[test]
fn test_migrate_rejects_wrong_expected_version() {
    let (env, admin, _creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();

    // Simulate an older contract at version 1
    env.as_contract(&client.address, || {
        set_version(&env, 1);
    });

    // Attempt migration with wrong expected version
    let result = client.try_migrate(&admin, &2);
    assert_eq!(result.unwrap_err().unwrap(), Error::ValidationFailed);

    // Version should remain unchanged
    let version_after = client.get_version();
    assert_eq!(version_after, 1);
}

#[test]
fn test_migrate_rejects_non_admin() {
    let (env, _admin, creator, _contributor1, _contributor2, _token, _token_admin, client) =
        setup_env();

    // Simulate an older contract at version 1
    env.as_contract(&client.address, || {
        set_version(&env, 1);
    });

    // Attempt migration with non-admin address
    let result = client.try_migrate(&creator, &1);
    assert_eq!(result.unwrap_err().unwrap(), Error::NotAuthorized);

    // Version should remain unchanged
    let version_after = client.get_version();
    assert_eq!(version_after, 1);
}
