// contracts/proof_of_heart/src/storage.rs (or admin.rs)

use soroban_sdk::{contractimpl, Address, Env};
use crate::errors::Error;

#[contractimpl]
impl ProofOfHeartContract {
    /// Removes a personal contribution cap for a contributor on a specific campaign.
    /// Requires authorization from the contributor.
    pub fn remove_personal_cap(
        env: Env,
        campaign_id: u32,
        contributor: Address,
    ) -> Result<(), Error> {
        // Ensure the contributor authorizes the removal of their personal cap
        contributor.require_auth();

        let storage_key = DataKey::PersonalCap(campaign_id, contributor.clone());

        // Check if cap exists before attempting removal
        if !env.storage().persistent().has(&storage_key) {
            return Err(Error::CapNotFound);
        }

        // Remove the personal cap from persistent storage
        env.storage().persistent().remove(&storage_key);

        // Emit event for indexers and off-chain listeners
        env.events().publish(
            (Symbol::new(&env, "personal_cap_removed"), campaign_id),
            contributor,
        );

        Ok(())
    }
}