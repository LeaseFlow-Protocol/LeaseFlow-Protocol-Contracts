#![cfg(test)]
#![allow(clippy::too_many_arguments)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(dead_code)]

use super::*;
use crate::{
    CreateLeaseParams, DataKey, DepositStatus, HistoricalLease, LeaseContract, LeaseContractClient,
    LeaseStatus, MaintenanceStatus, RateType, SubletStatus, UtilityBillStatus, LeaseInstance,
    velocity_guard::VelocityGuard,
};
use soroban_sdk::{
    contract, contractclient, contractimpl, symbol_short,
    testutils::{Address as _, Ledger},
    Address, Env, String,
};

const START: u64 = 1711929600;
const END: u64 = 1714521600; // 30 days after START
const LEASE_ID: u64 = 1;

fn make_env() -> Env {
    let env = Env::default();
    env.ledger().with_mut(|l| l.timestamp = START);
    env.mock_all_auths();
    env
}

fn setup(env: &Env) -> (Address, LeaseContractClient<'_>) {
    let id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(env, &id);
    (id, client)
}

fn make_lease_with_early_termination_fees(
    env: &Env,
    landlord: &Address,
    tenant: &Address,
    early_termination_fee_bps: Option<u32>,
    fixed_penalty: Option<i128>,
) -> LeaseInstance {
    LeaseInstance {
        landlord: landlord.clone(),
        tenant: tenant.clone(),
        rent_amount: 1_000,
        deposit_amount: 500,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(env, "ipfs://QmHash123"),
        status: LeaseStatus::Active,
        nft_contract: None,
        token_id: None,
        rent_paid: 0,
        expiry_time: END,
        buyout_price: None,
        cumulative_payments: 0,
        debt: 0,
        rent_paid_through: START,
        deposit_status: DepositStatus::Held,
        rent_per_sec: 1, // 1 unit per second for easy calculation
        grace_period_end: END,
        late_fee_flat: 0,
        late_fee_per_sec: 0,
        seconds_late_charged: 0,
        withdrawal_address: None,
        rent_withdrawn: 0,
        arbitrators: soroban_sdk::Vec::new(env),
        pause_reason: None,
        paused_at: None,
        pause_initiator: None,
        total_paused_duration: 0,
        rent_pull_authorized_amount: None,
        last_rent_pull_timestamp: None,
        billing_cycle_duration: 2_592_000,
        yield_accumulated: 0,
        equity_balance: 0,
        equity_percentage_bps: 0,
        pet_deposit_amount: 0,
        pet_rent_amount: 0,
        flags: crate::lease_flags::ACTIVE,
        early_termination_fee_bps,
        fixed_penalty,
    }
}

fn seed_lease(env: &Env, contract_id: &Address, lease_id: u64, lease: &LeaseInstance) {
    env.as_contract(contract_id, || save_lease_instance(env, lease_id, lease));
}

fn read_lease(env: &Env, contract_id: &Address, lease_id: u64) -> Option<LeaseInstance> {
    env.as_contract(contract_id, || load_lease_instance_by_id(env, lease_id))
}

#[test]
fn test_velocity_limit_protection_500_terminations() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    // Initialize contract with admin
    client.initialize(&admin, &token);

    // Create 1000 leases to establish a large portfolio
    let lease_ids: Vec<u64> = Vec::new(&env);
    for i in 0..1000 {
        let lease_id = LEASE_ID + i;
        let lease = make_lease_with_early_termination_fees(
            &env,
            &landlord,
            &tenant,
            Some(1000), // 10% fee
            None,
        );
        seed_lease(&env, &id, lease_id, &lease);
        lease_ids.push_back(lease_id);
    }

    // Initialize velocity tracker for landlord
    env.as_contract(&id, || {
        VelocityGuard::initialize_lessor(&env, &landlord).unwrap();
        VelocityGuard::update_portfolio_size(&env, &landlord, 1000).unwrap();
    });

    // Verify initial velocity tracker
    let (total_leases, terminations_24h, velocity_percentage, is_paused) = env.as_contract(&id, || {
        VelocityGuard::get_velocity_stats(&env, &landlord).unwrap()
    });
    assert_eq!(total_leases, 1000);
    assert_eq!(terminations_24h, 0);
    assert_eq!(velocity_percentage, 0);
    assert!(!is_paused);

    // Attempt to terminate 500 leases rapidly
    let mut successful_terminations = 0;
    let mut velocity_limit_hit = false;

    for i in 0..500 {
        let result = client.execute_early_termination(&lease_ids.get(i as u32).unwrap(), &tenant);
        
        match result {
            Ok(()) => successful_terminations += 1,
            Err(LeaseError::VelocityLimitExceeded) => {
                velocity_limit_hit = true;
                break;
            },
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    // Verify velocity limit was triggered before completing all 500
    assert!(velocity_limit_hit, "Velocity limit should have been triggered");
    assert!(successful_terminations < 500, "Should not complete all 500 terminations");

    // Verify landlord is now paused
    let (_, _, _, is_paused) = env.as_contract(&id, || {
        VelocityGuard::get_velocity_stats(&env, &landlord).unwrap()
    });
    assert!(is_paused);

    // Try to terminate another lease - should fail due to pause
    let result = client.execute_early_termination(&lease_ids.get(501).unwrap(), &tenant);
    assert_eq!(result, Err(LeaseError::VelocityLimitExceeded));
}

#[test]
fn test_velocity_threshold_calculation() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    // Initialize contract
    client.initialize(&admin, &token);

    // Create exactly 100 leases to test threshold calculation
    let lease_ids: Vec<u64> = Vec::new(&env);
    for i in 0..100 {
        let lease_id = LEASE_ID + i;
        let lease = make_lease_with_early_termination_fees(
            &env,
            &landlord,
            &tenant,
            Some(1000), // 10% fee
            None,
        );
        seed_lease(&env, &id, lease_id, &lease);
        lease_ids.push_back(lease_id);
    }

    // Initialize velocity tracker
    env.as_contract(&id, || {
        VelocityGuard::initialize_lessor(&env, &landlord).unwrap();
        VelocityGuard::update_portfolio_size(&env, &landlord, 100).unwrap();
    });

    // Terminate 9 leases (9% - should be allowed)
    for i in 0..9 {
        client.execute_early_termination(&lease_ids.get(i).unwrap(), &tenant).unwrap();
    }

    // Should still be allowed (9% < 10% threshold)
    let (_, _, velocity_percentage, is_paused) = env.as_contract(&id, || {
        VelocityGuard::get_velocity_stats(&env, &landlord).unwrap()
    });
    assert_eq!(velocity_percentage, 900); // 9% in basis points
    assert!(!is_paused);

    // Terminate 2 more leases (total 11% - should trigger velocity limit)
    let result = client.execute_early_termination(&lease_ids.get(9).unwrap(), &tenant);
    assert_eq!(result, Err(LeaseError::VelocityLimitExceeded));

    // Verify landlord is paused
    let (_, _, _, is_paused) = env.as_contract(&id, || {
        VelocityGuard::get_velocity_stats(&env, &landlord).unwrap()
    });
    assert!(is_paused);
}

#[test]
fn test_rolling_window_cleanup() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    // Initialize contract
    client.initialize(&admin, &token);

    // Create 50 leases
    let lease_ids: Vec<u64> = Vec::new(&env);
    for i in 0..50 {
        let lease_id = LEASE_ID + i;
        let lease = make_lease_with_early_termination_fees(
            &env,
            &landlord,
            &tenant,
            Some(1000), // 10% fee
            None,
        );
        seed_lease(&env, &id, lease_id, &lease);
        lease_ids.push_back(lease_id);
    }

    // Initialize velocity tracker
    env.as_contract(&id, || {
        VelocityGuard::initialize_lessor(&env, &landlord).unwrap();
        VelocityGuard::update_portfolio_size(&env, &landlord, 50).unwrap();
    });

    // Terminate 5 leases (10% - should trigger limit)
    for i in 0..5 {
        client.execute_early_termination(&lease_ids.get(i).unwrap(), &tenant).unwrap();
    }

    // Verify velocity limit was triggered
    let (_, _, _, is_paused) = env.as_contract(&id, || {
        VelocityGuard::get_velocity_stats(&env, &landlord).unwrap()
    });
    assert!(is_paused);

    // Advance time by 25 hours (past the 24-hour window)
    env.ledger().with_mut(|l| l.timestamp += 86400 * 25);

    // DAO approval to resume
    client.dao_approve_resume(&admin, &landlord, &1);

    // Verify landlord is no longer paused
    let (_, terminations_24h, _, is_paused) = env.as_contract(&id, || {
        VelocityGuard::get_velocity_stats(&env, &landlord).unwrap()
    });
    assert!(!is_paused);
    assert_eq!(terminations_24h, 0); // Should be cleaned up
}

#[test]
fn test_single_user_termination_unaffected() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord1 = Address::generate(&env);
    let landlord2 = Address::generate(&env);
    let tenant = Address::generate(&env);
    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    // Initialize contract
    client.initialize(&admin, &token);

    // Create leases for two different landlords
    let lease_ids1: Vec<u64> = Vec::new(&env);
    let lease_ids2: Vec<u64> = Vec::new(&env);

    // Landlords with different portfolio sizes
    for i in 0..100 {
        let lease_id = LEASE_ID + i;
        let lease = make_lease_with_early_termination_fees(
            &env,
            &landlord1,
            &tenant,
            Some(1000), // 10% fee
            None,
        );
        seed_lease(&env, &id, lease_id, &lease);
        lease_ids1.push_back(lease_id);
    }

    for i in 0..10 {
        let lease_id = LEASE_ID + 100 + i;
        let lease = make_lease_with_early_termination_fees(
            &env,
            &landlord2,
            &tenant,
            Some(1000), // 10% fee
            None,
        );
        seed_lease(&env, &id, lease_id, &lease);
        lease_ids2.push_back(lease_id);
    }

    // Initialize velocity trackers
    env.as_contract(&id, || {
        VelocityGuard::initialize_lessor(&env, &landlord1).unwrap();
        VelocityGuard::update_portfolio_size(&env, &landlord1, 100).unwrap();
        VelocityGuard::initialize_lessor(&env, &landlord2).unwrap();
        VelocityGuard::update_portfolio_size(&env, &landlord2, 10).unwrap();
    });

    // Landlord1 triggers velocity limit (terminate 11 leases)
    for i in 0..11 {
        if i == 10 {
            // This should trigger velocity limit
            let result = client.execute_early_termination(&lease_ids1.get(i).unwrap(), &tenant);
            assert_eq!(result, Err(LeaseError::VelocityLimitExceeded));
        } else {
            client.execute_early_termination(&lease_ids1.get(i).unwrap(), &tenant).unwrap();
        }
    }

    // Verify landlord1 is paused
    let (_, _, _, is_paused1) = env.as_contract(&id, || {
        VelocityGuard::get_velocity_stats(&env, &landlord1).unwrap()
    });
    assert!(is_paused1);

    // Landlord2 should still be able to terminate normally (single-user unaffected)
    for i in 0..2 {
        client.execute_early_termination(&lease_ids2.get(i).unwrap(), &tenant).unwrap();
    }

    // Verify landlord2 is not paused
    let (_, _, _, is_paused2) = env.as_contract(&id, || {
        VelocityGuard::get_velocity_stats(&env, &landlord2).unwrap()
    });
    assert!(!is_paused2);
}

#[test]
fn test_deposit_slash_velocity_protection() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    let oracle = Address::generate(&env);

    // Initialize contract
    client.initialize(&admin, &token);
    client.whitelist_oracle(&admin, &BytesN::from_array(&env, &[1; 32]));

    // Create 100 leases
    let lease_ids: Vec<u64> = Vec::new(&env);
    for i in 0..100 {
        let lease_id = LEASE_ID + i;
        let mut lease = make_lease_with_early_termination_fees(
            &env,
            &landlord,
            &tenant,
            Some(1000), // 10% fee
            None,
        );
        lease.status = LeaseStatus::Terminated; // Need terminated status for deposit slash
        seed_lease(&env, &id, lease_id, &lease);
        lease_ids.push_back(lease_id);
    }

    // Initialize velocity tracker
    env.as_contract(&id, || {
        VelocityGuard::initialize_lessor(&env, &landlord).unwrap();
        VelocityGuard::update_portfolio_size(&env, &landlord, 100).unwrap();
    });

    // Create oracle payload for deposit slashing
    let create_payload = |lease_id: u64| -> crate::OraclePayload {
        lease_id,
        oracle_pubkey: BytesN::from_array(&env, &[1; 32]),
        damage_severity: crate::DamageSeverity::Moderate,
        nonce: 1,
        timestamp: env.ledger().timestamp(),
        signature: BytesN::from_array(&env, &[2; 64]),
    };

    // Mix of terminations and slashes to trigger velocity limit
    for i in 0..8 {
        if i % 2 == 0 {
            // Regular termination
            client.execute_early_termination(&lease_ids.get(i).unwrap(), &tenant).unwrap();
        } else {
            // Slash deposit (also counts toward velocity)
            let payload = create_payload(*lease_ids.get(i).unwrap());
            client.execute_deposit_slash(&payload).unwrap();
        }
    }

    // Try one more operation - should trigger velocity limit
    let payload = create_payload(*lease_ids.get(8).unwrap());
    let result = client.execute_deposit_slash(&payload);
    assert_eq!(result, Err(LeaseError::VelocityLimitExceeded));

    // Verify landlord is paused
    let (_, _, _, is_paused) = env.as_contract(&id, || {
        VelocityGuard::get_velocity_stats(&env, &landlord).unwrap()
    });
    assert!(is_paused);
}

#[test]
fn test_dao_approval_workflow() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    // Initialize contract
    client.initialize(&admin, &token);

    // Create 100 leases
    let lease_ids: Vec<u64> = Vec::new(&env);
    for i in 0..100 {
        let lease_id = LEASE_ID + i;
        let lease = make_lease_with_early_termination_fees(
            &env,
            &landlord,
            &tenant,
            Some(1000), // 10% fee
            None,
        );
        seed_lease(&env, &id, lease_id, &lease);
        lease_ids.push_back(lease_id);
    }

    // Initialize velocity tracker
    env.as_contract(&id, || {
        VelocityGuard::initialize_lessor(&env, &landlord).unwrap();
        VelocityGuard::update_portfolio_size(&env, &landlord, 100).unwrap();
    });

    // Trigger velocity limit
    for i in 0..11 {
        if i == 10 {
            let result = client.execute_early_termination(&lease_ids.get(i).unwrap(), &tenant);
            assert_eq!(result, Err(LeaseError::VelocityLimitExceeded));
        } else {
            client.execute_early_termination(&lease_ids.get(i).unwrap(), &tenant).unwrap();
        }
    }

    // Verify landlord is paused
    let (_, _, _, is_paused) = env.as_contract(&id, || {
        VelocityGuard::get_velocity_stats(&env, &landlord).unwrap()
    });
    assert!(is_paused);

    // Try to terminate - should fail
    let result = client.execute_early_termination(&lease_ids.get(11).unwrap(), &tenant);
    assert_eq!(result, Err(LeaseError::VelocityLimitExceeded));

    // DAO approves resume
    client.dao_approve_resume(&admin, &landlord, &1);

    // Verify landlord is no longer paused
    let (_, _, _, is_paused) = env.as_contract(&id, || {
        VelocityGuard::get_velocity_stats(&env, &landlord).unwrap()
    });
    assert!(!is_paused);

    // Should be able to terminate again
    client.execute_early_termination(&lease_ids.get(11).unwrap(), &tenant).unwrap();
}

#[test]
fn test_storage_optimization() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    // Initialize contract
    client.initialize(&admin, &token);

    // Create 1000 leases
    let lease_ids: Vec<u64> = Vec::new(&env);
    for i in 0..1000 {
        let lease_id = LEASE_ID + i;
        let lease = make_lease_with_early_termination_fees(
            &env,
            &landlord,
            &tenant,
            Some(1000), // 10% fee
            None,
        );
        seed_lease(&env, &id, lease_id, &lease);
        lease_ids.push_back(lease_id);
    }

    // Initialize velocity tracker
    env.as_contract(&id, || {
        VelocityGuard::initialize_lessor(&env, &landlord).unwrap();
        VelocityGuard::update_portfolio_size(&env, &landlord, 1000).unwrap();
    });

    // Terminate 100 leases over time to populate the rolling window
    for i in 0..100 {
        client.execute_early_termination(&lease_ids.get(i).unwrap(), &tenant).unwrap();
        
        // Advance time slightly between terminations
        env.ledger().with_mut(|l| l.timestamp += 100);
    }

    // Check tracker state
    let (_, terminations_24h, _, _) = env.as_contract(&id, || {
        VelocityGuard::get_velocity_stats(&env, &landlord).unwrap()
    });
    assert_eq!(terminations_24h, 100);

    // Advance time past 24 hours
    env.ledger().with_mut(|l| l.timestamp += 86400 + 1000);

    // Create and terminate one more lease to trigger cleanup
    let new_lease_id = LEASE_ID + 1000;
    let lease = make_lease_with_early_termination_fees(
        &env,
        &landlord,
        &tenant,
        Some(1000), // 10% fee
        None,
    );
    seed_lease(&env, &id, new_lease_id, &lease);

    // This should trigger cleanup of old records
    let result = client.execute_early_termination(&new_lease_id, &tenant);

    // Should succeed since old records were cleaned up
    assert!(result.is_ok());

    // Verify cleanup happened
    let (_, terminations_24h, _, _) = env.as_contract(&id, || {
        VelocityGuard::get_velocity_stats(&env, &landlord).unwrap()
    });
    assert_eq!(terminations_24h, 1); // Only the recent termination
}
