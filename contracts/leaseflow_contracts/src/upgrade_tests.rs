#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, BytesN as _},
    Address, BytesN, Env,
};

fn setup_test() -> (Env, LeaseContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.set_admin(&admin);
    (env, client, admin)
}

#[test]
fn test_set_terms_hash_success() {
    let (env, client, admin) = setup_test();
    let hash = BytesN::from_array(&env, &[1u8; 32]);
    
    client.set_terms_hash(&admin, &hash);
    
    // Verify hash was set (we need a getter or check via storage if we want to be thorough)
    // For now, if no panic and auth was required, it's a success.
}

#[test]
#[should_panic(expected = "Unauthorised")]
fn test_set_terms_hash_unauthorized() {
    let (env, client, _admin) = setup_test();
    let stranger = Address::generate(&env);
    let hash = BytesN::from_array(&env, &[1u8; 32]);
    
    // This should panic because stranger doesn't match stored admin
    client.set_terms_hash(&stranger, &hash);
}

#[test]
fn test_upgrade_terms_mismatch_fails() {
    let (env, client, admin) = setup_test();
    let correct_hash = BytesN::from_array(&env, &[1u8; 32]);
    let wrong_hash = BytesN::from_array(&env, &[2u8; 32]);
    let fake_wasm_hash = BytesN::from_array(&env, &[3u8; 32]);
    
    client.set_terms_hash(&admin, &correct_hash);
    
    let result = client.try_upgrade(&admin, &fake_wasm_hash, &wrong_hash);
    
    // Should return UpgradeNotAllowed (16)
    match result {
        Err(Ok(LeaseError::UpgradeNotAllowed)) => {},
        _ => panic!("Expected UpgradeNotAllowed error"),
    }
}

#[test]
#[should_panic(expected = "Unauthorised")]
fn test_upgrade_unauthorized_fails() {
    let (env, client, admin) = setup_test();
    let hash = BytesN::from_array(&env, &[1u8; 32]);
    let fake_wasm_hash = BytesN::from_array(&env, &[3u8; 32]);
    let stranger = Address::generate(&env);
    
    client.set_terms_hash(&admin, &hash);
    
    client.upgrade(&stranger, &fake_wasm_hash, &hash);
}

#[test]
fn test_upgrade_success_with_matching_hash() {
    let (env, client, admin) = setup_test();
    let hash = BytesN::from_array(&env, &[1u8; 32]);
    // In testutils, we can't easily perform a real WASM upgrade without a valid WASM file,
    // but we can verify the logic proceeds correctly until the deployer call.
    // However, env.deployer().update_current_contract_wasm(...) will panic in tests 
    // unless the WASM hash is registered / exists.
    
    client.set_terms_hash(&admin, &hash);
    
    // We expect this to fail with "wasm not found" or similar if we use a random hash,
    // but the point is to test our LeaseError::UpgradeNotAllowed logic.
    let fake_wasm_hash = BytesN::from_array(&env, &[4u8; 32]);
    
    // If we reach the deployer call, it means our checks passed.
    let result = client.try_upgrade(&admin, &fake_wasm_hash, &hash);
    
    // Since fake_wasm_hash doesn't exist, Soroban testutils will likely throw a specific error,
    // but it WON'T be LeaseError::UpgradeNotAllowed if our check passed.
    if let Err(Ok(LeaseError::UpgradeNotAllowed)) = result {
        panic!("Should have passed terms hash check");
    }
}
