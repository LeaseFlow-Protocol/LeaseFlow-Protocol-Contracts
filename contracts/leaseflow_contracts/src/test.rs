#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env, symbol_short};
use crate::{LeaseContract, LeaseContractClient, LeaseStatus};

#[test]
fn test_storage_management_and_ttl() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(&env, &contract_id);
    
    let lease_id = symbol_short!("lease1");
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let rent_amount = 5000i128;
    let deposit_amount = 10000i128;
    let duration = 31_536_000u64; // 1 year

    // ── 1. Create Lease: Core identities in Persistent storage ──────────────────
    client.initialize_lease(
        &lease_id,
        &landlord,
        &tenant,
        &rent_amount,
        &deposit_amount,
        &duration,
    );

    let lease = client.get_lease(&lease_id);
    assert_eq!(lease.landlord, landlord);
    assert_eq!(lease.tenant, tenant);
    assert_eq!(lease.rent_amount, rent_amount);
    assert_eq!(lease.status, LeaseStatus::Active);

    // ── 2. Pay Rent: Monthly receipts in Instance storage ──────────────────────
    let month = 1;
    let amount_paid = 5000i128;
    client.pay_rent(&lease_id, &month, &amount_paid);

    let receipt = client.get_receipt(&lease_id, &month);
    assert_eq!(receipt.lease_id, lease_id);
    assert_eq!(receipt.month, month);
    assert_eq!(receipt.amount, amount_paid);
    assert_eq!(receipt.date, 0); // Ledger starts at 0 

    // ── 3. TTL Extension Check (Simplified) ───────────────────────────────────
    // Testing that the methods execute correctly (TTL is an internal property of ledger entries)
    // We can manually call extend_ttl to verify it's functional
    client.extend_ttl(&lease_id);
}

#[test]
#[should_panic(expected = "Lease not found")]
fn test_get_nonexistent_lease() {
    let env = Env::default();
    let contract_id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(&env, &contract_id);
    client.get_lease(&symbol_short!("ghost"));
}

#[test]
#[should_panic(expected = "Receipt not found")]
fn test_get_nonexistent_receipt() {
    let env = Env::default();
    let contract_id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(&env, &contract_id);
    let lease_id = symbol_short!("lease1");
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    env.mock_all_auths();
    client.initialize_lease(&lease_id, &landlord, &tenant, &500, &1000, &86400);
    client.get_receipt(&lease_id, &1);
}