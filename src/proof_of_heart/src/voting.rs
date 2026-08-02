// contracts/proof_of_heart/src/voting.rs

pub fn cast_vote(env: Env, voter: Address, campaign_id: u64, approve: bool) -> Result<(), Error> {
    voter.require_auth();

    let vote_key = DataKey::Vote(campaign_id, voter.clone());
    
    // Check if a vote already exists for this voter on this campaign
    if let Some(existing_vote) = env.storage().persistent().get::<DataKey, Vote>(&vote_key) {
        // If the vote direction is identical, treat as a no-op and return early
        if existing_vote.approve == approve {
            return Ok(());
        }
    }

    // Proceed with state update and event emission only if vote changed or is new
    let new_vote = Vote {
        voter: voter.clone(),
        campaign_id,
        approve,
        timestamp: env.ledger().timestamp(),
    };

    env.storage().persistent().set(&vote_key, &new_vote);

    // Emit event for state change
    env.events().publish(
        (Symbol::new(&env, "campaign_vote_cast"), campaign_id),
        (voter, approve),
    );

    Ok(())
}