#![cfg(test)]

use crate::{
    DataKey, LeaseContract, LeaseContractClient, LeaseInstance, LeaseStatus, OracleConfig,
    OracleRetryStats, OracleStatus, OracleTier, UsageRights,
};
use soroban_sdk::{testutils::Address as TestAddress, Address, BytesN, Env, Symbol, String, Vec};

#[test]
fn test_ephemeral_storage_tier_usage() {
    let env = Env::default();
    let contract_id = env.register_contract(None, LeaseContract);
    let client = LeaseContractClient::new(&env, &contract_id);
    
    let lessor = Address::generate(&env);
    
    // Simulate some logic that would use ephemeral storage (e.g., Oracle Retry Stats)
    // Directly inject an OracleRetryStats into temporary storage to verify it persists
    let oracle_pubkey = BytesN::from_array(&env, &[1; 32]);
    let stats = OracleRetryStats {
        demotion_timestamp: Some(123456789),
        failure_count: 2,
    };
    
    env.as_contract(&contract_id, || {
        let key = DataKey::OracleRetryStats(oracle_pubkey.clone());
        env.storage().temporary().set(&key, &stats);
        
        let loaded_stats: OracleRetryStats = env.storage().temporary().get(&key).unwrap();
        assert_eq!(loaded_stats.failure_count, 2);
    });
}

#[test]
fn test_velocity_tracker_ephemeral_storage() {
    let env = Env::default();
    let contract_id = env.register_contract(None, LeaseContract);
    let lessor = Address::generate(&env);
    
    env.as_contract(&contract_id, || {
        use crate::velocity_guard::VelocityTracker;
        let tracker = VelocityTracker {
            lessor: lessor.clone(),
            total_leases: 1,
            terminations_24h: 1,
            last_termination_times: Vec::new(&env),
            is_paused: false,
            pause_timestamp: None,
        };
        
        let key = DataKey::VelocityTracker(lessor.clone());
        env.storage().temporary().set(&key, &tracker);
        
        let loaded_tracker: VelocityTracker = env.storage().temporary().get(&key).unwrap();
        assert_eq!(loaded_tracker.terminations_24h, 1);
    });
}

#[test]
fn test_usage_rights_persistent_storage() {
    let env = Env::default();
    let contract_id = env.register_contract(None, LeaseContract);
    let tenant = Address::generate(&env);
    let nft_contract = Address::generate(&env);
    let token_id: u128 = 42;
    
    env.as_contract(&contract_id, || {
        let usage_rights = UsageRights {
            renter: tenant.clone(),
            nft_contract: nft_contract.clone(),
            token_id,
            lease_id: Symbol::new(&env, "lease_1"),
            valid_until: 987654321,
        };
        
        let key = DataKey::UsageRights(nft_contract.clone(), token_id);
        env.storage().persistent().set(&key, &usage_rights);
        
        let loaded_rights: UsageRights = env.storage().persistent().get(&key).unwrap();
        assert_eq!(loaded_rights.valid_until, 987654321);
    });
}
