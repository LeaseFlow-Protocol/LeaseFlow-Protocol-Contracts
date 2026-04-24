#![cfg(test)]
#![allow(clippy::too_many_arguments)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(dead_code)]
#![allow(unused_imports)]

use super::*;
use crate::{
    CreateLeaseParams, DamageSeverity, DataKey, DepositStatus, HistoricalLease, LeaseContract,
    LeaseContractClient, LeaseError, LeaseStatus, MaintenanceStatus, OraclePayload, RateType,
};
use soroban_sdk::{
    contract, contractclient, contractimpl, symbol_short,
    testutils::{Address as _, Events, Ledger},
    vec, Address, BytesN, Env, String,
};

const START: u64 = 1711929600;
const END: u64 = 1714521600;
const LEASE_ID: u64 = 1;

#[contract]
pub struct KycMock;

#[contractimpl]
impl KycMock {
    pub fn is_verified(env: Env, address: Address) -> bool {
        env.storage().instance().get(&address).unwrap_or(false)
    }
    pub fn set_verified(env: Env, address: Address, status: bool) {
        env.storage().instance().set(&address, &status);
    }
}

#[contract]
pub struct YieldMock;

#[contractimpl]
impl YieldMock {
    pub fn deposit(env: Env, from: Address, amount: i128) -> i128 {
        let lp_tokens = amount;
        env.storage().instance().set(&from, &lp_tokens);
        lp_tokens
    }

    pub fn withdraw(env: Env, from: Address, lp_tokens: i128) -> i128 {
        let withdrawn = lp_tokens;
        env.storage().instance().remove(&from);
        withdrawn
    }

    pub fn get_balance(env: Env, user: Address) -> i128 {
        env.storage().instance().get(&user).unwrap_or(0)
    }

    pub fn claim_rewards(env: Env, user: Address) -> i128 {
        1000i128
    }
}

#[contract]
pub struct DexMock;

#[contractimpl]
impl DexMock {
    pub fn path_payment(
        env: Env,
        from: Address,
        to: Address,
        amount_in: i128,
        max_slippage_bps: u32,
        path: soroban_sdk::Vec<Address>,
    ) -> i128 {
        let available = env
            .storage()
            .instance()
            .get::<_, i128>(&symbol_short!("liq"))
            .unwrap_or(0);
        if available < amount_in || path.is_empty() {
            panic!("insufficient liquidity");
        }
        let output = amount_in.saturating_mul(9_900) / 10_000;
        let min_out = amount_in.saturating_mul(10_000i128 - max_slippage_bps as i128) / 10_000i128;
        if output < min_out {
            panic!("slippage exceeded");
        }
        output
    }
    pub fn set_liquidity(env: Env, amount: i128) {
        env.storage().instance().set(&symbol_short!("liq"), &amount);
    }
}

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

fn make_lease(env: &Env, landlord: &Address, tenant: &Address) -> LeaseInstance {
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
        active: true,
        rent_paid: 0,
        expiry_time: END,
        buyout_price: None,
        cumulative_payments: 0,
        debt: 0,
        rent_paid_through: START,
        deposit_status: DepositStatus::Held,
        rent_per_sec: 0,
        grace_period_end: END,
        late_fee_flat: 0,
        late_fee_per_sec: 0,
        flat_fee_applied: false,
        seconds_late_charged: 0,
        withdrawal_address: None,
        rent_withdrawn: 0,
        arbitrators: soroban_sdk::Vec::new(env),
        maintenance_status: MaintenanceStatus::None,
        withheld_rent: 0,
        repair_proof_hash: None,
        inspector: None,
        paused: false,
        pause_reason: None,
        paused_at: None,
        pause_initiator: None,
        total_paused_duration: 0,
        rent_pull_authorized_amount: None,
        last_rent_pull_timestamp: None,
        billing_cycle_duration: 2_592_000,
        yield_delegation_enabled: false,
        yield_accumulated: 0,
        equity_balance: 0,
        equity_percentage_bps: 0,
        had_late_payment: false,
        has_pet: false,
        pet_deposit_amount: 0,
        pet_rent_amount: 0,
        payment_token: Address::generate(env),
    }
}

fn seed_lease(env: &Env, contract_id: &Address, lease_id: u64, lease: &LeaseInstance) {
    env.as_contract(contract_id, || save_lease_instance(env, lease_id, lease));
}

fn read_lease(env: &Env, contract_id: &Address, lease_id: u64) -> Option<LeaseInstance> {
    env.as_contract(contract_id, || load_lease_instance_by_id(env, lease_id))
}

#[test]
fn test_oracle_whitelist_management() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let admin = Address::generate(&env);
    let oracle_pubkey = BytesN::from_array(&env, &[1; 32]);
    let unauthorized_user = Address::generate(&env);

    client.set_admin(&admin);

    client.whitelist_oracle(&admin, &oracle_pubkey);

    let is_whitelisted = env.as_contract(&contract_id, || {
        crate::LeaseContract::is_oracle_whitelisted(&env, &oracle_pubkey)
    });
    assert!(is_whitelisted);

    let result = client.try_whitelist_oracle(&unauthorized_user, &oracle_pubkey);
    assert_eq!(result, Err(Ok(LeaseError::Unauthorised)));

    client.remove_oracle(&admin, &oracle_pubkey);

    let is_whitelisted_after_removal = env.as_contract(&contract_id, || {
        crate::LeaseContract::is_oracle_whitelisted(&env, &oracle_pubkey)
    });
    assert!(!is_whitelisted_after_removal);
}

#[test]
fn test_oracle_nonce_management() {
    let env = make_env();
    let (contract_id, _) = setup(&env);
    let oracle_pubkey = BytesN::from_array(&env, &[2; 32]);

    let initial_nonce = env.as_contract(&contract_id, || {
        crate::LeaseContract::get_oracle_nonce(&env, &oracle_pubkey)
    });
    assert_eq!(initial_nonce, 0);

    env.as_contract(&contract_id, || {
        crate::LeaseContract::set_oracle_nonce(&env, &oracle_pubkey, 5);
    });

    let updated_nonce = env.as_contract(&contract_id, || {
        crate::LeaseContract::get_oracle_nonce(&env, &oracle_pubkey)
    });
    assert_eq!(updated_nonce, 5);
}

#[test]
fn test_damage_severity_penalty_calculation() {
    let env = make_env();
    let (contract_id, _) = setup(&env);

    let test_cases = [
        (DamageSeverity::NormalWearAndTear, 0),
        (DamageSeverity::Minor, 10),
        (DamageSeverity::Moderate, 25),
        (DamageSeverity::Major, 50),
        (DamageSeverity::Severe, 75),
        (DamageSeverity::Catastrophic, 100),
    ];

    for (severity, expected_percentage) in test_cases {
        let calculated_percentage = env.as_contract(&contract_id, || {
            crate::LeaseContract::calculate_penalty_percentage(severity)
        });
        assert_eq!(calculated_percentage, expected_percentage);
    }
}

#[test]
fn test_execute_deposit_slash_normal_wear_and_tear() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let oracle_pubkey = BytesN::from_array(&env, &[3; 32]);

    client.set_admin(&admin);
    client.whitelist_oracle(&admin, &oracle_pubkey);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.status = LeaseStatus::Terminated;
    seed_lease(&env, &contract_id, LEASE_ID, &lease);

    let payload = OraclePayload {
        lease_id: LEASE_ID,
        oracle_pubkey: oracle_pubkey.clone(),
        damage_severity: DamageSeverity::NormalWearAndTear,
        nonce: 1,
        timestamp: env.ledger().timestamp(),
        signature: BytesN::from_array(&env, &[0; 64]),
    };

    env.as_contract(&contract_id, || {
        crate::LeaseContract::set_oracle_nonce(&env, &oracle_pubkey, 0);
    });

    let result = client.try_execute_deposit_slash(&payload);
    assert!(result.is_err());
}

#[test]
fn test_execute_deposit_slash_unauthorized_oracle() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let unauthorized_oracle = BytesN::from_array(&env, &[4; 32]);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.status = LeaseStatus::Terminated;
    lease.payment_token = Address::generate(&env);
    seed_lease(&env, &contract_id, LEASE_ID, &lease);

    let payload = OraclePayload {
        lease_id: LEASE_ID,
        oracle_pubkey: unauthorized_oracle,
        damage_severity: DamageSeverity::Minor,
        nonce: 1,
        timestamp: env.ledger().timestamp(),
        signature: BytesN::from_array(&env, &[0; 64]),
    };

    let result = client.try_execute_deposit_slash(&payload);
    assert_eq!(result, Err(Ok(LeaseError::OracleNotWhitelisted)));
}

#[test]
fn test_execute_deposit_slash_invalid_nonce() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let oracle_pubkey = BytesN::from_array(&env, &[5; 32]);

    client.set_admin(&admin);
    client.whitelist_oracle(&admin, &oracle_pubkey);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.status = LeaseStatus::Terminated;
    seed_lease(&env, &contract_id, LEASE_ID, &lease);

    env.as_contract(&contract_id, || {
        crate::LeaseContract::set_oracle_nonce(&env, &oracle_pubkey, 5);
    });

    let payload = OraclePayload {
        lease_id: LEASE_ID,
        oracle_pubkey,
        damage_severity: DamageSeverity::Minor,
        nonce: 3,
        timestamp: env.ledger().timestamp(),
        signature: BytesN::from_array(&env, &[0; 64]),
    };

    let result = client.try_execute_deposit_slash(&payload);
    assert_eq!(result, Err(Ok(LeaseError::InvalidNonce)));
}

#[test]
fn test_execute_deposit_slash_lease_not_terminated() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let oracle_pubkey = BytesN::from_array(&env, &[6; 32]);

    client.set_admin(&admin);
    client.whitelist_oracle(&admin, &oracle_pubkey);

    let lease = make_lease(&env, &landlord, &tenant);
    seed_lease(&env, &contract_id, LEASE_ID, &lease);

    let payload = OraclePayload {
        lease_id: LEASE_ID,
        oracle_pubkey,
        damage_severity: DamageSeverity::Minor,
        nonce: 1,
        timestamp: env.ledger().timestamp(),
        signature: BytesN::from_array(&env, &[0; 64]),
    };

    let result = client.try_execute_deposit_slash(&payload);
    assert_eq!(result, Err(Ok(LeaseError::LeaseNotTerminated)));
}

#[test]
fn test_execute_deposit_slash_deposit_already_settled() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let oracle_pubkey = BytesN::from_array(&env, &[7; 32]);

    client.set_admin(&admin);
    client.whitelist_oracle(&admin, &oracle_pubkey);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.status = LeaseStatus::Terminated;
    lease.deposit_status = DepositStatus::Settled;
    seed_lease(&env, &contract_id, LEASE_ID, &lease);

    let payload = OraclePayload {
        lease_id: LEASE_ID,
        oracle_pubkey,
        damage_severity: DamageSeverity::Minor,
        nonce: 1,
        timestamp: env.ledger().timestamp(),
        signature: BytesN::from_array(&env, &[0; 64]),
    };

    let result = client.try_execute_deposit_slash(&payload);
    assert_eq!(result, Err(Ok(LeaseError::DepositAlreadySettled)));
}

#[test]
fn test_tenant_flagging_functionality() {
    let env = make_env();
    let (contract_id, _) = setup(&env);
    let tenant = Address::generate(&env);
    let reason = String::from_str(&env, "Severe damage exceeding deposit value");

    let is_flagged_initially = env.as_contract(&contract_id, || {
        crate::LeaseContract::is_tenant_flagged(&env, LEASE_ID)
    });
    assert!(!is_flagged_initially);

    env.as_contract(&contract_id, || {
        crate::LeaseContract::flag_tenant(&env, LEASE_ID, tenant.clone(), reason.clone());
    });

    let is_flagged_after = env.as_contract(&contract_id, || {
        crate::LeaseContract::is_tenant_flagged(&env, LEASE_ID)
    });
    assert!(is_flagged_after);
}

#[test]
fn test_signature_timestamp_validation() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let oracle_pubkey = BytesN::from_array(&env, &[8; 32]);

    client.set_admin(&admin);
    client.whitelist_oracle(&admin, &oracle_pubkey);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.status = LeaseStatus::Terminated;
    lease.payment_token = Address::generate(&env);
    seed_lease(&env, &contract_id, LEASE_ID, &lease);

    let future_timestamp = env.ledger().timestamp() + 100000;
    let payload_future = OraclePayload {
        lease_id: LEASE_ID,
        oracle_pubkey: oracle_pubkey.clone(),
        damage_severity: DamageSeverity::Minor,
        nonce: 1,
        timestamp: future_timestamp,
        signature: BytesN::from_array(&env, &[0; 64]),
    };

    let result_future = client.try_execute_deposit_slash(&payload_future);
    assert_eq!(result_future, Err(Ok(LeaseError::InvalidSignature)));

    let old_timestamp = env.ledger().timestamp() - 100000;
    let payload_old = OraclePayload {
        lease_id: LEASE_ID,
        oracle_pubkey,
        damage_severity: DamageSeverity::Minor,
        nonce: 1,
        timestamp: old_timestamp,
        signature: BytesN::from_array(&env, &[0; 64]),
    };

    let result_old = client.try_execute_deposit_slash(&payload_old);
    assert_eq!(result_old, Err(Ok(LeaseError::InvalidSignature)));
}

#[test]
fn test_stablecoin_enforcement() {
    let env = make_env();
    let (_, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let admin = Address::generate(&env);
    let usdc = Address::generate(&env);
    let volatile_token = Address::generate(&env);

    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &usdc);

    let lease_id = symbol_short!("lease1");
    let uri = String::from_str(&env, "ipfs://test");

    let res = client.try_initialize_lease(
        &lease_id,
        &landlord,
        &tenant,
        &5000,
        &10000,
        &31536000,
        &uri,
        &volatile_token,
    );
    assert!(res.is_err());

    client.initialize_lease(
        &lease_id, &landlord, &tenant, &5000, &10000, &31536000, &uri, &usdc,
    );
    let lease = client.get_lease(&lease_id);
    assert_eq!(lease.payment_token, usdc);
}

#[test]
fn test_activate_lease_success() {
    let env = make_env();
    let (_, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);
    let admin = Address::generate(&env);

    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);

    client.create_lease(&landlord, &tenant, &1000i128, &token);
    let result = client.activate_lease(&symbol_short!("lease"), &tenant);

    assert_eq!(result, symbol_short!("active"));
}

#[test]
fn test_lease_basic() {
    let env = make_env();
    let (_, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);
    let admin = Address::generate(&env);

    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);

    let lease_id = symbol_short!("lease1");
    client.initialize_lease(
        &lease_id,
        &landlord,
        &tenant,
        &5000,
        &10000,
        &31536000,
        &String::from_str(&env, "ipfs://test"),
        &token,
    );

    client.activate_lease(&lease_id, &tenant);
    client.pay_rent(&lease_id, &5000);

    let month = 1u32;
    let amount_paid = 5000i128;
    client.pay_rent_receipt(&lease_id, &month, &amount_paid);

    let receipt = client.get_receipt(&lease_id, &month);
    assert_eq!(receipt.lease_id, lease_id);
    assert_eq!(receipt.month, month);
    assert_eq!(receipt.amount, amount_paid);

    client.extend_ttl(&lease_id);
}

#[test]
fn test_terminate_lease_before_end_date_fails() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let lease = make_lease(&env, &landlord, &tenant);
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END - 1);

    let result = client.try_terminate_lease(&LEASE_ID, &landlord);
    assert_eq!(result, Err(Ok(LeaseError::LeaseNotExpired)));
}

#[test]
fn test_terminate_lease_unauthorised_caller_fails() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let stranger = Address::generate(&env);

    let lease = make_lease(&env, &landlord, &tenant);
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    let result = client.try_terminate_lease(&LEASE_ID, &stranger);
    assert_eq!(result, Err(Ok(LeaseError::Unauthorised)));
}

#[test]
fn test_terminate_lease_not_found_fails() {
    let env = make_env();
    let (_, client) = setup(&env);
    let caller = Address::generate(&env);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    let result = client.try_terminate_lease(&99u64, &caller);
    assert_eq!(result, Err(Ok(LeaseError::LeaseNotFound)));
}

#[test]
fn test_terminate_lease_success() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Settled;
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    client.terminate_lease(&LEASE_ID, &landlord);
    assert!(read_lease(&env, &id, LEASE_ID).is_none());
}

#[test]
fn test_reclaim_asset_success() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let reason = String::from_str(&env, "Lease expired - asset returned");

    let lease = make_lease(&env, &landlord, &tenant);
    seed_lease(&env, &id, LEASE_ID, &lease);

    client.reclaim_asset(&LEASE_ID, &landlord, &reason);
}

#[contractclient(name = "MockNftClient")]
pub trait MockNftInterface {
    fn transfer_from(env: Env, spender: Address, from: Address, to: Address, token_id: u128);
    fn owner_of(env: Env, token_id: u128) -> Address;
}

#[test]
fn test_create_lease_instance_with_security_deposit() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 2000,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token.clone(),
        arbitrators: soroban_sdk::Vec::new(&env),
        rent_per_sec: 1,
        grace_period_end: END,
        late_fee_flat: 0,
        late_fee_per_sec: 0,
        equity_percentage_bps: 0,
        has_pet: false,
        pet_deposit_amount: 0,
        pet_rent_amount: 0,
        yield_delegation_enabled: false,
        deposit_asset: None,
        dex_contract: None,
        max_slippage_bps: 0,
        swap_path: soroban_sdk::Vec::new(&env),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    let lease = client.get_lease_instance(&LEASE_ID);
    assert_eq!(lease.landlord, landlord);
    assert_eq!(lease.tenant, tenant);
    assert_eq!(lease.security_deposit, 500);
    assert_eq!(lease.status, LeaseStatus::Pending);
}

#[test]
fn test_cross_asset_deposit_locks_with_swap() {
    let env = make_env();
    let (id, client) = setup(&env);
    let dex_id = env.register(DexMock, ());
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let usdc = Address::generate(&env);
    let xlm = Address::generate(&env);

    DexMockClient::new(&env, &dex_id).set_liquidity(&5_000);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 2000,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: usdc.clone(),
        arbitrators: soroban_sdk::Vec::new(&env),
        rent_per_sec: 1,
        grace_period_end: END,
        late_fee_flat: 0,
        late_fee_per_sec: 0,
        equity_percentage_bps: 0,
        has_pet: false,
        pet_deposit_amount: 0,
        pet_rent_amount: 0,
        yield_delegation_enabled: false,
        deposit_asset: Some(xlm.clone()),
        dex_contract: Some(dex_id.clone()),
        max_slippage_bps: 100,
        swap_path: vec![&env, xlm.clone(), usdc.clone()],
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);
    let lease = read_lease(&env, &id, LEASE_ID).unwrap();
    assert_eq!(lease.security_deposit, 495);
}

#[test]
fn test_tenant_default_scenario_3_months_non_payment() {
    let env = make_env();
    let (_, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);

    let month_in_secs: u64 = 2_592_000;
    let rent_amount = 1000i128;
    let start_date = 10_000_000u64;
    env.ledger().with_mut(|l| l.timestamp = start_date);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount,
        deposit_amount: rent_amount * 2,
        security_deposit: rent_amount,
        start_date,
        end_date: start_date + month_in_secs * 12,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token.clone(),
        arbitrators: soroban_sdk::Vec::new(&env),
        rent_per_sec: 1, // $1 per second
        grace_period_end: start_date + month_in_secs * 12,
        late_fee_flat: 0,
        late_fee_per_sec: 0,
        equity_percentage_bps: 0,
        has_pet: false,
        pet_deposit_amount: 0,
        pet_rent_amount: 0,
        yield_delegation_enabled: false,
        deposit_asset: None,
        dex_contract: None,
        max_slippage_bps: 0,
        swap_path: soroban_sdk::Vec::new(&env),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    env.ledger()
        .with_mut(|l| l.timestamp = start_date + month_in_secs + 1);
    let debt_1 = client.check_tenant_default(&LEASE_ID);
    assert!(debt_1 > 0);

    let three_months = start_date + month_in_secs * 3;
    env.ledger().with_mut(|l| l.timestamp = three_months);
    let debt_3 = client.check_tenant_default(&LEASE_ID);
    assert!(debt_3 > rent_amount * 2);
}

#[test]
fn test_double_sign_prevention() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(&env, &contract_id);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let payment_token = Address::generate(&env);

    let lease_id = 1u64;

    let mut params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1500,
        deposit_amount: 1500,
        security_deposit: 1500,
        start_date: env.ledger().timestamp(),
        end_date: env.ledger().timestamp() + (30 * 86400),
        property_uri: String::from_str(&env, "ipfs://QmLeaseDoc"),
        payment_token: payment_token.clone(),
        arbitrators: soroban_sdk::Vec::new(&env),
        rent_per_sec: 1,
        grace_period_end: env.ledger().timestamp() + (30 * 86400),
        late_fee_flat: 0,
        late_fee_per_sec: 0,
        equity_percentage_bps: 0,
        has_pet: false,
        pet_deposit_amount: 0,
        pet_rent_amount: 0,
        yield_delegation_enabled: false,
        deposit_asset: None,
        dex_contract: None,
        max_slippage_bps: 0,
        swap_path: soroban_sdk::Vec::new(&env),
    };

    let result = client.try_create_lease_instance(&lease_id, &landlord, &params);
    assert!(result.is_ok(), "Initial lease creation should succeed");

    params.rent_amount = 500;

    let malicious_result = client.try_create_lease_instance(&lease_id, &landlord, &params);

    assert!(
        malicious_result.is_err(),
        "Contract must reject attempts to overwrite an existing lease"
    );

    let active_lease = client.get_lease_instance(&lease_id);
    assert_eq!(
        active_lease.rent_amount, 1500,
        "Rent amount should remain untouched at 1500"
    );
}

// ---------------------------------------------------------------------------
// [ISSUE 5] Terminate Bounty Tests
// ---------------------------------------------------------------------------

#[contract]
pub struct TokenMock;

#[contractimpl]
impl TokenMock {
    pub fn mint(env: Env, to: Address, amount: i128) {
        let bal: i128 = env.storage().instance().get(&to).unwrap_or(0);
        env.storage().instance().set(&to, &(bal + amount));
    }
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        let from_bal: i128 = env.storage().instance().get(&from).unwrap_or(0);
        let to_bal: i128 = env.storage().instance().get(&to).unwrap_or(0);
        env.storage().instance().set(&from, &(from_bal - amount));
        env.storage().instance().set(&to, &(to_bal + amount));
    }
    pub fn balance(env: Env, addr: Address) -> i128 {
        env.storage().instance().get(&addr).unwrap_or(0)
    }
}

#[test]
fn test_terminate_lease_bounty_paid() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let fee_recipient = Address::generate(&env);

    let token_id = env.register(TokenMock, ());
    let token_client = TokenMockClient::new(&env, &token_id);
    let platform_fee: i128 = 1_000;
    token_client.mint(&fee_recipient, &platform_fee);

    client.set_admin(&admin);
    client.set_platform_fee(&admin, &platform_fee, &token_id, &fee_recipient);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Settled;
    seed_lease(&env, &contract_id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    client.terminate_lease(&LEASE_ID, &landlord);

    let expected_bounty: i128 = 100;
    assert_eq!(token_client.balance(&landlord), expected_bounty);
    assert_eq!(
        token_client.balance(&fee_recipient),
        platform_fee - expected_bounty
    );
    assert!(read_lease(&env, &contract_id, LEASE_ID).is_none());
}

#[test]
fn test_terminate_lease_no_bounty_without_platform_fee() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Settled;
    seed_lease(&env, &contract_id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    client.terminate_lease(&LEASE_ID, &landlord);
    assert!(read_lease(&env, &contract_id, LEASE_ID).is_none());
}

// Yield Generation Tests

fn create_yield_test_env() -> (Env, Address, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let dao = Address::generate(&env);

    let lease_contract_id = env.register(LeaseContract, ());
    let lease_client = LeaseContractClient::new(&env, &lease_contract_id);

    let yield_contract_id = env.register(YieldMock, ());

    lease_client.set_admin(&admin);
    lease_client.set_platform_fee(&admin, &1000, &Address::generate(&env), &dao);
    lease_client.whitelist_yield_protocol(&admin, &yield_contract_id);
    lease_client.set_liquidity_buffer_amount(&admin, &10000);

    (
        env,
        lease_contract_id,
        yield_contract_id,
        admin,
        landlord,
        tenant,
    )
}

fn create_test_lease_for_yield(
    env: &Env,
    contract_id: &Address,
    landlord: Address,
    tenant: Address,
) {
    let lease_client = LeaseContractClient::new(env, contract_id);
    let payment_token = env.register(TokenMock, ()); // Uses registered mock

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 500,
        security_deposit: 5000,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(env, "test_property"),
        payment_token: payment_token.clone(),
        arbitrators: soroban_sdk::Vec::new(env),
        rent_per_sec: 0,
        grace_period_end: END,
        late_fee_flat: 0,
        late_fee_per_sec: 0,
        equity_percentage_bps: 0,
        has_pet: false,
        pet_deposit_amount: 0,
        pet_rent_amount: 0,
        yield_delegation_enabled: true,
        deposit_asset: None,
        dex_contract: None,
        max_slippage_bps: 500,
        swap_path: soroban_sdk::Vec::new(env),
    };

    lease_client.create_lease_instance(&LEASE_ID, &landlord, &params);
}

#[test]
fn test_deploy_escrow_to_yield_success() {
    let (env, lease_contract_id, yield_contract_id, admin, landlord, tenant) =
        create_yield_test_env();

    create_test_lease_for_yield(&env, &lease_contract_id, landlord, tenant);

    let lease_client = LeaseContractClient::new(&env, &lease_contract_id);

    lease_client.deploy_escrow_to_yield(&LEASE_ID, &yield_contract_id, &2000, &500);

    let deployment = lease_client.get_yield_deployment(&LEASE_ID);
    assert_eq!(deployment.lease_id, LEASE_ID);
    assert_eq!(deployment.principal_amount, 2000);
    assert_eq!(deployment.yield_protocol, yield_contract_id);
    assert_eq!(deployment.lp_tokens, 2000);
    assert!(deployment.active);

    let lease = lease_client.get_lease_instance(&LEASE_ID);
    assert_eq!(lease.security_deposit, 3000);
}

#[test]
fn test_harvest_yield_distribution() {
    let (env, lease_contract_id, yield_contract_id, admin, landlord, tenant) =
        create_yield_test_env();

    create_test_lease_for_yield(&env, &lease_contract_id, landlord, tenant);

    let lease_client = LeaseContractClient::new(&env, &lease_contract_id);

    lease_client.deploy_escrow_to_yield(&LEASE_ID, &yield_contract_id, &2000, &500);

    lease_client.harvest_yield(&LEASE_ID);

    let accumulated = lease_client.get_accumulated_yield(&LEASE_ID);
    assert_eq!(accumulated, 1000); // Verify the harvest worked securely without checking events
}

#[test]
fn test_withdraw_from_yield_success() {
    let (env, lease_contract_id, yield_contract_id, admin, landlord, tenant) =
        create_yield_test_env();

    create_test_lease_for_yield(&env, &lease_contract_id, landlord, tenant);

    let lease_client = LeaseContractClient::new(&env, &lease_contract_id);

    lease_client.deploy_escrow_to_yield(&LEASE_ID, &yield_contract_id, &2000, &500);

    lease_client.withdraw_from_yield(&LEASE_ID, &500);

    let lease = lease_client.get_lease_instance(&LEASE_ID);
    assert_eq!(lease.security_deposit, 5000); // Back to original

    let deployment = lease_client.get_yield_deployment(&LEASE_ID);
    assert!(!deployment.active);
    assert_eq!(deployment.lp_tokens, 0);
}

// ---------------------------------------------------------------------------
// Roommate Split Payment Tests (Issue: Roommate Headache)
// ---------------------------------------------------------------------------

#[test]
fn test_add_authorized_roommate_success() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let roommate = Address::generate(&env);
    let token = Address::generate(&env);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 3000,
        deposit_amount: 0,
        security_deposit: 0,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token.clone(),
        arbitrators: soroban_sdk::Vec::new(&env),
        rent_per_sec: 0,
        grace_period_end: END,
        late_fee_flat: 0,
        late_fee_per_sec: 0,
        equity_percentage_bps: 0,
        has_pet: false,
        pet_deposit_amount: 0,
        pet_rent_amount: 0,
        yield_delegation_enabled: false,
        deposit_asset: None,
        dex_contract: None,
        max_slippage_bps: 0,
        swap_path: soroban_sdk::Vec::new(&env),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    client.add_authorized_payer(&LEASE_ID, &landlord, &roommate);

    let stranger = Address::generate(&env);
    let result = client.try_add_authorized_payer(&LEASE_ID, &stranger, &roommate);
    assert_eq!(result, Err(Ok(LeaseError::Unauthorised)));
}

#[test]
fn test_roommate_split_payment_execution() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let primary_tenant = Address::generate(&env);
    let roommate = Address::generate(&env);

    let token_id = env.register(TokenMock, ());
    let token_client = TokenMockClient::new(&env, &token_id);
    token_client.mint(&roommate, &5000);

    let params = CreateLeaseParams {
        tenant: primary_tenant.clone(),
        rent_amount: 3000,
        deposit_amount: 0,
        security_deposit: 0,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token_id.clone(),
        arbitrators: soroban_sdk::Vec::new(&env),
        rent_per_sec: 0,
        grace_period_end: END,
        late_fee_flat: 0,
        late_fee_per_sec: 0,
        equity_percentage_bps: 0,
        has_pet: false,
        pet_deposit_amount: 0,
        pet_rent_amount: 0,
        yield_delegation_enabled: false,
        deposit_asset: None,
        dex_contract: None,
        max_slippage_bps: 0,
        swap_path: soroban_sdk::Vec::new(&env),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);
    client.add_authorized_payer(&LEASE_ID, &landlord, &roommate);

    client.pay_lease_instance_rent(&LEASE_ID, &roommate, &1000);

    assert_eq!(token_client.balance(&roommate), 4000);
    assert_eq!(token_client.balance(&contract_id), 1000);

    let roommate_bal = client.get_roommate_balance(&LEASE_ID, &roommate);
    assert_eq!(roommate_bal, 1000);

    let lease = client.get_lease_instance(&LEASE_ID);
    assert_eq!(lease.rent_paid, 1000);
    assert_eq!(lease.cumulative_payments, 1000);
}

#[test]
fn test_unauthorized_stranger_cannot_pay_rent() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let primary_tenant = Address::generate(&env);
    let stranger = Address::generate(&env);
    let token_id = env.register(TokenMock, ());

    let params = CreateLeaseParams {
        tenant: primary_tenant.clone(),
        rent_amount: 3000,
        deposit_amount: 0,
        security_deposit: 0,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token_id.clone(),
        arbitrators: soroban_sdk::Vec::new(&env),
        rent_per_sec: 0,
        grace_period_end: END,
        late_fee_flat: 0,
        late_fee_per_sec: 0,
        equity_percentage_bps: 0,
        has_pet: false,
        pet_deposit_amount: 0,
        pet_rent_amount: 0,
        yield_delegation_enabled: false,
        deposit_asset: None,
        dex_contract: None,
        max_slippage_bps: 0,
        swap_path: soroban_sdk::Vec::new(&env),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    let result = client.try_pay_lease_instance_rent(&LEASE_ID, &stranger, &1000);
    assert_eq!(result, Err(Ok(LeaseError::Unauthorised)));
}
