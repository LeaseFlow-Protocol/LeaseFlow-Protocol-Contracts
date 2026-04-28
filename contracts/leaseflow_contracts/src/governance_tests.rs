//! Tests for SEP-12 Identity, Timelock Upgrade, and Oracle Governance modules

#![cfg(test)]

use super::*;
use crate::sep12_identity::{Sep12IdentityModule, IdentityProvider, IdentityProviderStatus, LessorTrustedProviders};
use crate::timelock_upgrade::{TimelockUpgradeModule, UpgradeProposal, UpgradeProposalStatus};
use crate::oracle_governance::{OracleGovernanceModule, OracleGovernanceRecord, OracleGovernanceStatus};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, BytesN, Env, String, Vec};

fn create_test_bytesn32(env: &Env, value: u8) -> BytesN<32> {
    let mut bytes = [0u8; 32];
    bytes[0] = value;
    BytesN::from_array(env, &bytes)
}

// ==================== SEP-12 Identity Tests ====================

#[test]
fn test_register_identity_provider() {
    let env = Env::default();
    let provider_address = Address::generate(&env);
    let provider_id = create_test_bytesn32(&env, 1);
    let name = String::from_str(&env, "Test Identity Provider");

    env.mock_all_auths();
    
    let result = Sep12IdentityModule::register_identity_provider(
        env.clone(),
        provider_id.clone(),
        provider_address.clone(),
        name.clone(),
    );
    
    assert!(result.is_ok());
    
    let provider = Sep12IdentityModule::get_provider(env.clone(), provider_id.clone()).unwrap();
    assert_eq!(provider.provider_id, provider_id);
    assert_eq!(provider.provider_address, provider_address);
    assert_eq!(provider.name, name);
    assert_eq!(provider.status, IdentityProviderStatus::Active);
    assert_eq!(provider.trust_score, 5000);
}

#[test]
fn test_register_duplicate_provider_fails() {
    let env = Env::default();
    let provider_address = Address::generate(&env);
    let provider_id = create_test_bytesn32(&env, 1);
    let name = String::from_str(&env, "Test Provider");

    env.mock_all_auths();
    
    let result1 = Sep12IdentityModule::register_identity_provider(
        env.clone(),
        provider_id.clone(),
        provider_address.clone(),
        name.clone(),
    );
    assert!(result1.is_ok());
    
    let result2 = Sep12IdentityModule::register_identity_provider(
        env.clone(),
        provider_id.clone(),
        provider_address.clone(),
        name.clone(),
    );
    assert!(result2.is_err());
}

#[test]
fn test_add_trusted_provider() {
    let env = Env::default();
    let lessor = Address::generate(&env);
    let provider_address = Address::generate(&env);
    let provider_id = create_test_bytesn32(&env, 1);
    let name = String::from_str(&env, "Trusted Provider");

    env.mock_all_auths();
    
    Sep12IdentityModule::register_identity_provider(
        env.clone(),
        provider_id.clone(),
        provider_address,
        name,
    ).unwrap();
    
    let result = Sep12IdentityModule::add_trusted_provider(
        env.clone(),
        lessor.clone(),
        provider_id.clone(),
    );
    assert!(result.is_ok());
    
    assert!(Sep12IdentityModule::is_provider_trusted(
        env.clone(),
        lessor.clone(),
        provider_id.clone()
    ));
    
    let trusted = Sep12IdentityModule::get_trusted_providers(env.clone(), lessor).unwrap();
    assert_eq!(trusted.trusted_providers.len(), 1);
}

#[test]
fn test_remove_trusted_provider() {
    let env = Env::default();
    let lessor = Address::generate(&env);
    let provider_address = Address::generate(&env);
    let provider_id = create_test_bytesn32(&env, 1);
    let name = String::from_str(&env, "Provider");

    env.mock_all_auths();
    
    Sep12IdentityModule::register_identity_provider(
        env.clone(),
        provider_id.clone(),
        provider_address,
        name,
    ).unwrap();
    
    Sep12IdentityModule::add_trusted_provider(
        env.clone(),
        lessor.clone(),
        provider_id.clone(),
    ).unwrap();
    
    let result = Sep12IdentityModule::remove_trusted_provider(
        env.clone(),
        lessor.clone(),
        provider_id.clone(),
    );
    assert!(result.is_ok());
    
    assert!(!Sep12IdentityModule::is_provider_trusted(
        env.clone(),
        lessor,
        provider_id
    ));
}

#[test]
fn test_suspend_and_revoke_provider() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let provider_address = Address::generate(&env);
    let provider_id = create_test_bytesn32(&env, 1);
    let name = String::from_str(&env, "Provider");
    let reason = String::from_str(&env, "Compliance violation");

    env.mock_all_auths();
    
    Sep12IdentityModule::register_identity_provider(
        env.clone(),
        provider_id.clone(),
        provider_address,
        name,
    ).unwrap();
    
    Sep12IdentityModule::add_governance_council_member(
        env.clone(),
        admin.clone(),
        admin.clone(),
    ).unwrap();
    
    let result = Sep12IdentityModule::suspend_provider(
        env.clone(),
        admin.clone(),
        provider_id.clone(),
        reason.clone(),
    );
    assert!(result.is_ok());
    
    let provider = Sep12IdentityModule::get_provider(env.clone(), provider_id.clone()).unwrap();
    assert_eq!(provider.status, IdentityProviderStatus::Suspended);
}

#[test]
fn test_revocation_proposal_flow() {
    let env = Env::default();
    let proposer = Address::generate(&env);
    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);
    let provider_address = Address::generate(&env);
    let provider_id = create_test_bytesn32(&env, 1);
    let name = String::from_str(&env, "Provider");
    let reason = String::from_str(&env, "Security breach");

    env.mock_all_auths();
    
    Sep12IdentityModule::register_identity_provider(
        env.clone(),
        provider_id.clone(),
        provider_address,
        name,
    ).unwrap();
    
    for member in [proposer.clone(), voter1.clone(), voter2.clone(), voter3.clone()] {
        Sep12IdentityModule::add_governance_council_member(
            env.clone(),
            member.clone(),
            member.clone(),
        ).unwrap();
    }
    
    let proposal_id = Sep12IdentityModule::create_revocation_proposal(
        env.clone(),
        proposer.clone(),
        provider_id.clone(),
        reason.clone(),
    ).unwrap();
    
    for voter in [proposer, voter1, voter2] {
        Sep12IdentityModule::vote_on_revocation(
            env.clone(),
            voter,
            proposal_id,
            true,
        ).unwrap();
    }
    
    let result = Sep12IdentityModule::execute_revocation(
        env.clone(),
        proposer.clone(),
        proposal_id,
    );
    assert!(result.is_ok());
    
    let provider = Sep12IdentityModule::get_provider(env.clone(), provider_id).unwrap();
    assert_eq!(provider.status, IdentityProviderStatus::Revoked);
}

// ==================== Timelock Upgrade Tests ====================

#[test]
fn test_initialize_timelock() {
    let env = Env::default();
    let admin = Address::generate(&env);

    env.mock_all_auths();
    
    let result = TimelockUpgradeModule::initialize_timelock(
        env.clone(),
        admin.clone(),
        1_209_600,
        3,
    );
    assert!(result.is_ok());
    
    assert!(TimelockUpgradeModule::is_signer(env.clone(), admin));
    assert_eq!(TimelockUpgradeModule::get_timelock_duration(env), 1_209_600);
}

#[test]
fn test_propose_upgrade() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let new_wasm_hash = create_test_bytesn32(&env, 100);
    let description = String::from_str(&env, "Security patch v2.0");

    env.mock_all_auths();
    
    TimelockUpgradeModule::initialize_timelock(
        env.clone(),
        admin.clone(),
        1_209_600,
        3,
    ).unwrap();
    
    let proposal_id = TimelockUpgradeModule::propose_upgrade(
        env.clone(),
        admin.clone(),
        new_wasm_hash.clone(),
        description.clone(),
    ).unwrap();
    
    assert_eq!(proposal_id, 0);
    
    let proposal = TimelockUpgradeModule::get_proposal(env.clone(), proposal_id).unwrap();
    assert_eq!(proposal.proposer, admin);
    assert_eq!(proposal.new_wasm_hash, new_wasm_hash);
    assert_eq!(proposal.status, UpgradeProposalStatus::Pending);
    assert_eq!(proposal.required_approvals, 3);
}

#[test]
fn test_approve_upgrade() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);
    let new_wasm_hash = create_test_bytesn32(&env, 100);
    let description = String::from_str(&env, "Update");

    env.mock_all_auths();
    
    TimelockUpgradeModule::initialize_timelock(
        env.clone(),
        admin.clone(),
        1_209_600,
        3,
    ).unwrap();
    
    TimelockUpgradeModule::add_signer(env.clone(), admin.clone(), signer1.clone()).unwrap();
    TimelockUpgradeModule::add_signer(env.clone(), admin.clone(), signer2.clone()).unwrap();
    
    let proposal_id = TimelockUpgradeModule::propose_upgrade(
        env.clone(),
        admin.clone(),
        new_wasm_hash.clone(),
        description,
    ).unwrap();
    
    TimelockUpgradeModule::approve_upgrade(env.clone(), admin.clone(), proposal_id).unwrap();
    TimelockUpgradeModule::approve_upgrade(env.clone(), signer1.clone(), proposal_id).unwrap();
    TimelockUpgradeModule::approve_upgrade(env.clone(), signer2.clone(), proposal_id).unwrap();
    
    let proposal = TimelockUpgradeModule::get_proposal(env.clone(), proposal_id).unwrap();
    assert_eq!(proposal.approval_count, 3);
    assert_eq!(proposal.status, UpgradeProposalStatus::Approved);
}

#[test]
fn test_cancel_upgrade() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let new_wasm_hash = create_test_bytesn32(&env, 100);
    let description = String::from_str(&env, "Update");
    let reason = String::from_str(&env, "Critical bug found");

    env.mock_all_auths();
    
    TimelockUpgradeModule::initialize_timelock(
        env.clone(),
        admin.clone(),
        1_209_600,
        3,
    ).unwrap();
    
    let proposal_id = TimelockUpgradeModule::propose_upgrade(
        env.clone(),
        admin.clone(),
        new_wasm_hash,
        description,
    ).unwrap();
    
    let result = TimelockUpgradeModule::cancel_upgrade(
        env.clone(),
        admin.clone(),
        proposal_id,
        reason.clone(),
    );
    assert!(result.is_ok());
    
    let proposal = TimelockUpgradeModule::get_proposal(env.clone(), proposal_id).unwrap();
    assert_eq!(proposal.status, UpgradeProposalStatus::Cancelled);
}

#[test]
fn test_timelock_prevents_early_execution() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);
    let new_wasm_hash = create_test_bytesn32(&env, 100);
    let description = String::from_str(&env, "Update");

    env.mock_all_auths();
    
    TimelockUpgradeModule::initialize_timelock(
        env.clone(),
        admin.clone(),
        1_209_600,
        3,
    ).unwrap();
    
    TimelockUpgradeModule::add_signer(env.clone(), admin.clone(), signer1.clone()).unwrap();
    TimelockUpgradeModule::add_signer(env.clone(), admin.clone(), signer2.clone()).unwrap();
    
    let proposal_id = TimelockUpgradeModule::propose_upgrade(
        env.clone(),
        admin.clone(),
        new_wasm_hash,
        description,
    ).unwrap();
    
    TimelockUpgradeModule::approve_upgrade(env.clone(), admin.clone(), proposal_id).unwrap();
    TimelockUpgradeModule::approve_upgrade(env.clone(), signer1.clone(), proposal_id).unwrap();
    TimelockUpgradeModule::approve_upgrade(env.clone(), signer2.clone(), proposal_id).unwrap();
    
    let result = TimelockUpgradeModule::execute_upgrade(
        env.clone(),
        admin.clone(),
        proposal_id,
    );
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Timelock period not elapsed");
}

// ==================== Oracle Governance Tests ====================

#[test]
fn test_whitelist_oracle() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let oracle_pubkey = create_test_bytesn32(&env, 50);
    let reason = String::from_str(&env, "Verified provider");

    env.mock_all_auths();
    
    OracleGovernanceModule::initialize_governance(
        env.clone(),
        admin.clone(),
    ).unwrap();
    
    let result = OracleGovernanceModule::whitelist_oracle(
        env.clone(),
        admin.clone(),
        oracle_pubkey.clone(),
        reason.clone(),
    );
    assert!(result.is_ok());
    
    assert!(OracleGovernanceModule::is_oracle_active(env.clone(), oracle_pubkey.clone()));
    assert!(!OracleGovernanceModule::is_oracle_blacklisted(env.clone(), oracle_pubkey.clone()));
}

#[test]
fn test_emergency_blacklist() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let oracle_pubkey = create_test_bytesn32(&env, 50);
    let reason = String::from_str(&env, "Compromised key");
    let affected_leases = Vec::from_array(&env, [1u64, 2u64, 3u64]);

    env.mock_all_auths();
    
    OracleGovernanceModule::initialize_governance(
        env.clone(),
        admin.clone(),
    ).unwrap();
    
    OracleGovernanceModule::whitelist_oracle(
        env.clone(),
        admin.clone(),
        oracle_pubkey.clone(),
        String::from_str(&env, "Initial"),
    ).unwrap();
    
    let result = OracleGovernanceModule::emergency_blacklist(
        env.clone(),
        admin.clone(),
        oracle_pubkey.clone(),
        reason.clone(),
        affected_leases.clone(),
    );
    assert!(result.is_ok());
    
    assert!(!OracleGovernanceModule::is_oracle_active(env.clone(), oracle_pubkey.clone()));
    assert!(OracleGovernanceModule::is_oracle_blacklisted(env.clone(), oracle_pubkey.clone()));
    
    let record = OracleGovernanceModule::get_oracle_record(env.clone(), oracle_pubkey).unwrap();
    assert_eq!(record.status, OracleGovernanceStatus::Blacklisted);
    assert_eq!(record.affected_leases.len(), 3);
}

#[test]
fn test_suspend_and_reactivate_oracle() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let oracle_pubkey = create_test_bytesn32(&env, 50);

    env.mock_all_auths();
    
    OracleGovernanceModule::initialize_governance(
        env.clone(),
        admin.clone(),
    ).unwrap();
    
    OracleGovernanceModule::whitelist_oracle(
        env.clone(),
        admin.clone(),
        oracle_pubkey.clone(),
        String::from_str(&env, "Initial"),
    ).unwrap();
    
    let result = OracleGovernanceModule::suspend_oracle(
        env.clone(),
        admin.clone(),
        oracle_pubkey.clone(),
        String::from_str(&env, "Maintenance"),
    );
    assert!(result.is_ok());
    
    assert!(!OracleGovernanceModule::is_oracle_active(env.clone(), oracle_pubkey.clone()));
    
    let result = OracleGovernanceModule::reactivate_oracle(
        env.clone(),
        admin.clone(),
        oracle_pubkey.clone(),
    );
    assert!(result.is_ok());
    
    assert!(OracleGovernanceModule::is_oracle_active(env.clone(), oracle_pubkey));
}

#[test]
fn test_migrate_lease_oracle() {
    let env = Env::default();
    let caller = Address::generate(&env);
    let admin = Address::generate(&env);
    let old_oracle = create_test_bytesn32(&env, 50);
    let new_oracle = create_test_bytesn32(&env, 51);
    let lease_id = 123u64;

    env.mock_all_auths();
    
    OracleGovernanceModule::initialize_governance(
        env.clone(),
        admin.clone(),
    ).unwrap();
    
    OracleGovernanceModule::whitelist_oracle(
        env.clone(),
        admin.clone(),
        new_oracle.clone(),
        String::from_str(&env, "New oracle"),
    ).unwrap();
    
    let result = OracleGovernanceModule::migrate_lease_oracle(
        env.clone(),
        caller.clone(),
        lease_id,
        old_oracle.clone(),
        new_oracle.clone(),
    );
    assert!(result.is_ok());
    
    let active_oracle = OracleGovernanceModule::get_lease_oracle(env.clone(), lease_id);
    assert_eq!(active_oracle, Some(new_oracle.clone()));
    
    let migration = OracleGovernanceModule::get_migration_history(env.clone(), lease_id);
    assert!(migration.is_some());
    let migration = migration.unwrap();
    assert_eq!(migration.old_oracle, old_oracle);
    assert_eq!(migration.new_oracle, new_oracle);
    assert_eq!(migration.lease_id, lease_id);
}

#[test]
fn test_governance_admin_management() {
    let env = Env::default();
    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);

    env.mock_all_auths();
    
    OracleGovernanceModule::initialize_governance(
        env.clone(),
        admin1.clone(),
    ).unwrap();
    
    OracleGovernanceModule::add_governance_admin(
        env.clone(),
        admin1.clone(),
        admin2.clone(),
    ).unwrap();
    
    let result = OracleGovernanceModule::whitelist_oracle(
        env.clone(),
        admin2.clone(),
        create_test_bytesn32(&env, 60),
        String::from_str(&env, "Test"),
    );
    assert!(result.is_ok());
    
    OracleGovernanceModule::remove_governance_admin(
        env.clone(),
        admin1.clone(),
        admin2.clone(),
    ).unwrap();
    
    let result = OracleGovernanceModule::whitelist_oracle(
        env.clone(),
        admin2.clone(),
        create_test_bytesn32(&env, 61),
        String::from_str(&env, "Test"),
    );
    assert!(result.is_err());
}
