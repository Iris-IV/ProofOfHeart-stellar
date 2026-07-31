#[test]
fn test_cast_vote_no_op_does_not_emit_duplicate_events() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, ProofOfHeartContract);
    let client = ProofOfHeartContractClient::new(&env, &contract_id);

    let voter = Address::generate(&env);
    let campaign_id = 1_u64;

    // First vote: cast initial approval
    client.cast_vote(&voter, &campaign_id, &true);
    let initial_event_count = env.events().all().len();

    // Second vote: duplicate identical vote (no-op update)
    client.cast_vote(&voter, &campaign_id, &true);
    let final_event_count = env.events().all().len();

    // Assert that no new event was emitted for the no-op
    assert_eq!(initial_event_count, final_event_count);
}