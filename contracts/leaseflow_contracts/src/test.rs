#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, BytesN, String};
use crate::LeaseContractClient;

#[test]
fn test_lease_initialization() {
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env};

#[test]
fn test_lease_and_late_fees() {
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short,
    testutils::Address as _,
    Address, Env,
};

// ── Mock NFT Contract ─────────────────────────────────────────────────────────
//
// This pretends to be a real NFT contract for testing purposes.
// It records the last transfer so we can assert it happened correctly.

#[contract]
pub struct MockNftContract;

// We store the last transfer details in contract storage so the test can read them.
#[contracttype]
pub struct TransferRecord {
    pub from: Address,
    pub to: Address,
    pub token_id: u128,
}

#[contractimpl]
impl MockNftContract {
    pub fn transfer_from(
        env: Env,
        _spender: Address, // we ignore spender in the mock, a real contract would check it
        from: Address,
        to: Address,
        token_id: u128,
    ) {
        // Record the transfer so we can assert on it in the test
        env.storage().instance().set(
            &symbol_short!("last_xfr"),
            &TransferRecord { from, to, token_id },
        );
    }

    // Helper to read back the last recorded transfer
    pub fn get_last_transfer(env: Env) -> TransferRecord {
        env.storage()
            .instance()
            .get(&symbol_short!("last_xfr"))
            .expect("No transfer recorded")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};

#[test]
fn test_lease_with_nft() {
    let env = Env::default();
    // In tests we need to disable auth checks so require_auth() doesn't fail
    env.mock_all_auths();

    // Deploy the mock NFT contract
    let nft_id = env.register(MockNftContract, ());
    let nft_client = MockNftContractClient::new(&env, &nft_id);

    // Deploy the lease contract
    let lease_id = env.register(LeaseContract, ());
    let lease_client = LeaseContractClient::new(&env, &lease_id);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let rent_amount = 1000i128;
    let deposit_amount = 2000i128;
    let start_date = 1640995200u64; // Jan 1, 2022
    let end_date = 1672531200u64; // Jan 1, 2023
    let property_uri = String::from_str(&env, "ipfs://QmHash123");

    client.initialize_lease(
        &landlord,
        &tenant,
        &rent_amount,
        &deposit_amount,
        &start_date,
        &end_date,
        &property_uri,
    );
    
    let lease = client.get_lease();
    assert_eq!(lease.landlord, landlord);
    assert_eq!(lease.tenant, tenant);
    assert_eq!(lease.rent_amount, rent_amount);
    assert_eq!(lease.deposit_amount, deposit_amount);
    assert_eq!(lease.start_date, start_date);
    assert_eq!(lease.end_date, end_date);
    assert_eq!(lease.property_uri, property_uri);
    assert_eq!(lease.status, LeaseStatus::Pending);
}

#[test]
fn test_lease_activation() {
    let env = Env::default();
    let contract_id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(&env, &contract_id);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let rent_amount = 1000i128;
    let deposit_amount = 2000i128;
    let start_date = 1640995200u64;
    let end_date = 1672531200u64;
    let property_uri = String::from_str(&env, "ipfs://QmHash123");

    client.initialize_lease(
        &landlord,
        &tenant,
        &rent_amount,
        &deposit_amount,
        &start_date,
        &end_date,
        &property_uri,
    );
    
    // Activate lease
    client.activate_lease(&tenant);
    
    let lease = client.get_lease();
    assert_eq!(lease.status, LeaseStatus::Active);
}

#[test]
fn test_property_uri_update() {
    let env = Env::default();
    let contract_id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(&env, &contract_id);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let rent_amount = 1000i128;
    let deposit_amount = 2000i128;
    let start_date = 1640995200u64;
    let end_date = 1672531200u64;
    let property_uri = String::from_str(&env, "ipfs://QmHash123");

    client.initialize_lease(
        &landlord,
        &tenant,
        &rent_amount,
        &deposit_amount,
        &start_date,
        &end_date,
        &property_uri,
    );
    
    // Update property URI
    let new_property_uri = String::from_str(&env, "ipfs://QmNewHash456");
    client.update_property_uri(&landlord, &new_property_uri);
    
    let lease = client.get_lease();
    assert_eq!(lease.property_uri, new_property_uri);
}

#[test]
fn test_lease_amendment() {
    let env = Env::default();
    let contract_id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(&env, &contract_id);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let rent_amount = 1000i128;
    let deposit_amount = 2000i128;
    let start_date = 1640995200u64;
    let end_date = 1672531200u64;
    let property_uri = String::from_str(&env, "ipfs://QmHash123");

    client.initialize_lease(
        &landlord,
        &tenant,
        &rent_amount,
        &deposit_amount,
        &start_date,
        &end_date,
        &property_uri,
    );
    
    client.activate_lease(&tenant);
    
    // Create amendment with new rent and end date
    let new_rent = Some(1200i128);
    let new_end_date = Some(1704067200u64); // Jan 1, 2024
    let landlord_sig = BytesN::from_array(&env, &[1u8; 32]);
    let tenant_sig = BytesN::from_array(&env, &[2u8; 32]);
    
    let amendment = LeaseAmendment {
        new_rent_amount: new_rent,
        new_end_date: new_end_date,
        landlord_signature: landlord_sig,
        tenant_signature: tenant_sig,
    };
    
    client.amend_lease(&amendment);
    
    let lease = client.get_lease();
    assert_eq!(lease.rent_amount, 1200i128);
    assert_eq!(lease.end_date, 1704067200u64);
}

#[test]
fn test_deposit_release_full_refund() {
    let env = Env::default();
    let contract_id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(&env, &contract_id);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let rent_amount = 1000i128;
    let deposit_amount = 2000i128;
    let start_date = 1640995200u64;
    let end_date = 1672531200u64;
    let property_uri = String::from_str(&env, "ipfs://QmHash123");

    client.initialize_lease(
        &landlord,
        &tenant,
        &rent_amount,
        &deposit_amount,
        &start_date,
        &end_date,
        &property_uri,
    );
    
    client.activate_lease(&tenant);
    
    // Release full deposit
    let release = DepositRelease::FullRefund;
    client.release_deposit(&release);
}

#[test]
fn test_deposit_release_partial_refund() {
    let env = Env::default();
    let contract_id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(&env, &contract_id);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let rent_amount = 1000i128;
    let deposit_amount = 2000i128;
    let start_date = 1640995200u64;
    let end_date = 1672531200u64;
    let property_uri = String::from_str(&env, "ipfs://QmHash123");

    client.initialize_lease(
        &landlord,
        &tenant,
        &rent_amount,
        &deposit_amount,
        &start_date,
        &end_date,
        &property_uri,
    );
    
    client.activate_lease(&tenant);
    
    // Release partial deposit
    let partial = DepositReleasePartial {
        tenant_amount: 1500i128,
        landlord_amount: 500i128,
    };
    let release = DepositRelease::PartialRefund(partial);
    client.release_deposit(&release);
}

#[test]
fn test_deposit_release_disputed() {
    let env = Env::default();
    let contract_id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(&env, &contract_id);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let rent_amount = 1000i128;
    let deposit_amount = 2000i128;
    let start_date = 1640995200u64;
    let end_date = 1672531200u64;
    let property_uri = String::from_str(&env, "ipfs://QmHash123");

    client.initialize_lease(
        &landlord,
        &tenant,
        &rent_amount,
        &deposit_amount,
        &start_date,
        &end_date,
        &property_uri,
    );
    
    client.activate_lease(&tenant);
    
    // Mark deposit as disputed
    let release = DepositRelease::Disputed;
    client.release_deposit(&release);
    
    let lease = client.get_lease();
    assert_eq!(lease.status, LeaseStatus::Disputed);
    let token_id: u128 = 42;
    let rent_amount: i128 = 1000;

    // Create the lease — this should trigger transfer_from on the mock NFT
    lease_client.create_lease_with_nft(
    &landlord,
    &tenant,
    &rent_amount,
    &nft_id,
    &token_id,
);
    let lease_id = symbol_short!("lease");
    let amount = 1000i128;
    let grace_period_end = 100_000u64;
    let late_fee_flat = 20i128;
    let late_fee_per_day = 5i128;

    client.create_lease(
        &landlord,
        &tenant,
        &amount,
        &grace_period_end,
        &late_fee_flat,
        &late_fee_per_day,
    );

    let lease = client.get_lease();
    assert_eq!(lease.amount, 1000);
    assert_eq!(lease.debt, 0);

    // Time travels to 2 days later after grace period 
    // Wait, let's explicitly set the ledger
    // 2 days = 172800 secs. Let's add 176400 to make it exactly 2 full days and some balance.
    env.ledger().with_mut(|li| {
        li.timestamp = 100_000 + 176400; 
    });

    // Make a partial payment that covers part of the debt but no rent.
    // Flat fee $20 + (2 days * $5) = $30. Let's pay 25.
    client.pay_rent(&25);

    let updated_lease1 = client.get_lease();
    assert_eq!(updated_lease1.debt, 5);
    assert_eq!(updated_lease1.rent_paid, 0);
    assert_eq!(updated_lease1.flat_fee_applied, true);
    assert_eq!(updated_lease1.days_late_charged, 2);

    // Pay enough to clear the rest of debt and the full rent.
    // Remaining Debt = 5. Rent = 1000. Total = 1005.
    client.pay_rent(&1005);
    
    let updated_lease2 = client.get_lease();
    assert_eq!(updated_lease2.debt, 0);
    assert_eq!(updated_lease2.rent_paid, 0); // resets because rent was fully paid
    assert_eq!(updated_lease2.grace_period_end, 100_000 + 2592000);
    assert_eq!(updated_lease2.flat_fee_applied, false);
    assert_eq!(updated_lease2.days_late_charged, 0);
    let duration = 86_400u64; // 1 day

    client.create_lease(&lease_id, &landlord, &tenant, &amount, &duration);
    let lease = client.get_lease(&lease_id);

    // ── Assert: lease was stored correctly ────────────────────────────────────
    let lease = lease_client.get_lease();
    assert_eq!(lease.landlord, landlord);
    assert_eq!(lease.tenant, tenant);
    assert_eq!(lease.amount, rent_amount);
    assert_eq!(lease.nft_contract, Some(nft_id.clone()));
    assert_eq!(lease.token_id, Some(token_id));
    assert!(lease.active);

    // ── Assert: transfer_from was called with the right arguments ─────────────
    // This is the key acceptance criterion: verify transfer_from works correctly
    let transfer = nft_client.get_last_transfer();
    assert_eq!(transfer.from, landlord,   "NFT should move FROM the landlord");
    assert_eq!(transfer.to, tenant,       "NFT should move TO the tenant");
    assert_eq!(transfer.token_id, token_id, "Token ID should match");
    assert_eq!(lease.expiry_time, duration); // ledger timestamp starts at 0 in tests
}

#[test]
fn test_add_funds() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(&env, &contract_id);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let lease_id = symbol_short!("lease1");
    let initial_amount = 1000i128;
    let duration = 86_400u64; // 1 day

    client.create_lease(&lease_id, &landlord, &tenant, &initial_amount, &duration);

    let before = client.get_lease(&lease_id);
    let added_amount = 500i128;

    client.add_funds(&lease_id, &added_amount);

    let after = client.get_lease(&lease_id);

    assert_eq!(after.amount, initial_amount + added_amount);
    assert_eq!(
        after.expiry_time,
        before.expiry_time + (added_amount as u64 * SECS_PER_UNIT)
    );
    assert_eq!(after.landlord, landlord);
    assert_eq!(after.tenant, tenant);
    assert!(after.active);
}

#[test]
fn test_original_lease_fields_unchanged() {
    let env = Env::default();

    let lease_id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(&env, &lease_id);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    client.create_lease(&landlord, &tenant, &500i128);

    let lease = client.get_lease();
    assert_eq!(lease.landlord, landlord);
    assert_eq!(lease.tenant, tenant);
    assert_eq!(lease.amount, 500);
    assert!(lease.active);
    assert_eq!(lease.nft_contract, None);
    assert_eq!(lease.token_id, None);
}