//! Oracle Whitelist/Blacklist Governance Module
//! 
//! This module provides rapid response capabilities to excise compromised
//! hardware or software Oracles while protecting existing active leases.

use soroban_sdk::{contract, contracttype, contractevent, Address, Env, BytesN, String, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OracleGovernanceStatus {
    Whitelisted,
    Blacklisted,
    Suspended,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleGovernanceRecord {
    pub oracle_pubkey: BytesN<32>,
    pub status: OracleGovernanceStatus,
    pub added_at: u64,
    pub updated_at: u64,
    pub reason: String,
    pub affected_leases: Vec<u64>,
    pub added_by: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleMigration {
    pub lease_id: u64,
    pub old_oracle: BytesN<32>,
    pub new_oracle: BytesN<32>,
    pub migrated_at: u64,
    pub migrated_by: Address,
}

#[contracttype]
pub enum OracleGovernanceDataKey {
    OracleRecord(BytesN<32>),
    WhitelistCount,
    BlacklistCount,
    GovernanceAdmin(Address),
    ActiveLeaseOracle(u64),
    MigrationHistory(u64),
}

#[contractevent]
pub struct OracleWhitelisted {
    pub oracle_pubkey: BytesN<32>,
    pub added_by: Address,
    pub timestamp: u64,
}

#[contractevent]
pub struct OracleBlacklisted {
    pub oracle_pubkey: BytesN<32>,
    pub reason: String,
    pub blacklisted_by: Address,
    pub timestamp: u64,
}

#[contractevent]
pub struct OracleSuspended {
    pub oracle_pubkey: BytesN<32>,
    pub reason: String,
    pub suspended_by: Address,
    pub timestamp: u64,
}

#[contractevent]
pub struct OracleReactivated {
    pub oracle_pubkey: BytesN<32>,
    pub reactivated_by: Address,
    pub timestamp: u64,
}

#[contractevent]
pub struct LeaseOracleMigrated {
    pub lease_id: u64,
    pub old_oracle: BytesN<32>,
    pub new_oracle: BytesN<32>,
    pub migrated_by: Address,
    pub timestamp: u64,
}

#[contractevent]
pub struct EmergencyBlacklist {
    pub oracle_pubkey: BytesN<32>,
    pub reason: String,
    pub emergency_by: Address,
    pub affected_leases_count: u32,
    pub timestamp: u64,
}

#[contract]
pub struct OracleGovernanceModule;

#[contractimpl]
impl OracleGovernanceModule {
    /// Add oracle to whitelist
    pub fn whitelist_oracle(
        env: Env,
        admin: Address,
        oracle_pubkey: BytesN<32>,
        reason: String,
    ) -> Result<(), &'static str> {
        Self::require_admin(&env, &admin)?;
        admin.require_auth();

        let key = OracleGovernanceDataKey::OracleRecord(oracle_pubkey.clone());
        
        if env.storage().instance().has(&key) {
            let existing: OracleGovernanceRecord = env.storage().instance().get(&key).unwrap();
            if existing.status != OracleGovernanceStatus::Blacklisted {
                return Err("Oracle already in registry");
            }
        }

        let now = env.ledger().timestamp();
        let record = OracleGovernanceRecord {
            oracle_pubkey: oracle_pubkey.clone(),
            status: OracleGovernanceStatus::Whitelisted,
            added_at: now,
            updated_at: now,
            reason: reason.clone(),
            affected_leases: Vec::new(&env),
            added_by: admin.clone(),
        };

        env.storage().instance().set(&key, &record);

        let count: u64 = env.storage().instance()
            .get(&OracleGovernanceDataKey::WhitelistCount)
            .unwrap_or(0);
        env.storage().instance().set(&OracleGovernanceDataKey::WhitelistCount, &(count + 1));

        OracleWhitelisted {
            oracle_pubkey,
            added_by: admin,
            timestamp: now,
        }
        .publish(&env);

        Ok(())
    }

    /// Emergency blacklist - rapid response to compromised oracle
    pub fn emergency_blacklist(
        env: Env,
        admin: Address,
        oracle_pubkey: BytesN<32>,
        reason: String,
        affected_leases: Vec<u64>,
    ) -> Result<(), &'static str> {
        Self::require_admin(&env, &admin)?;
        admin.require_auth();

        let key = OracleGovernanceDataKey::OracleRecord(oracle_pubkey.clone());
        let now = env.ledger().timestamp();

        let record = if env.storage().instance().has(&key) {
            let mut existing: OracleGovernanceRecord = env.storage().instance().get(&key).unwrap();
            existing.status = OracleGovernanceStatus::Blacklisted;
            existing.updated_at = now;
            existing.reason = reason.clone();
            existing.affected_leases = affected_leases.clone();
            existing
        } else {
            OracleGovernanceRecord {
                oracle_pubkey: oracle_pubkey.clone(),
                status: OracleGovernanceStatus::Blacklisted,
                added_at: now,
                updated_at: now,
                reason: reason.clone(),
                affected_leases: affected_leases.clone(),
                added_by: admin.clone(),
            }
        };

        env.storage().instance().set(&key, &record);

        let count: u64 = env.storage().instance()
            .get(&OracleGovernanceDataKey::BlacklistCount)
            .unwrap_or(0);
        env.storage().instance().set(&OracleGovernanceDataKey::BlacklistCount, &(count + 1));

        EmergencyBlacklist {
            oracle_pubkey: oracle_pubkey.clone(),
            reason: reason.clone(),
            emergency_by: admin.clone(),
            affected_leases_count: affected_leases.len() as u32,
            timestamp: now,
        }
        .publish(&env);

        OracleBlacklisted {
            oracle_pubkey,
            reason,
            blacklisted_by: admin,
            timestamp: now,
        }
        .publish(&env);

        Ok(())
    }

    /// Suspend oracle (temporary, less severe than blacklist)
    pub fn suspend_oracle(
        env: Env,
        admin: Address,
        oracle_pubkey: BytesN<32>,
        reason: String,
    ) -> Result<(), &'static str> {
        Self::require_admin(&env, &admin)?;
        admin.require_auth();

        let key = OracleGovernanceDataKey::OracleRecord(oracle_pubkey.clone());
        let mut record: OracleGovernanceRecord = env.storage().instance()
            .get(&key)
            .ok_or("Oracle not found in registry")?;

        if record.status == OracleGovernanceStatus::Blacklisted {
            return Err("Oracle already blacklisted");
        }

        record.status = OracleGovernanceStatus::Suspended;
        record.updated_at = env.ledger().timestamp();
        record.reason = reason.clone();

        env.storage().instance().set(&key, &record);

        OracleSuspended {
            oracle_pubkey,
            reason,
            suspended_by: admin,
            timestamp: env.ledger().timestamp(),
        }
        .publish(&env);

        Ok(())
    }

    /// Reactivate a suspended oracle
    pub fn reactivate_oracle(
        env: Env,
        admin: Address,
        oracle_pubkey: BytesN<32>,
    ) -> Result<(), &'static str> {
        Self::require_admin(&env, &admin)?;
        admin.require_auth();

        let key = OracleGovernanceDataKey::OracleRecord(oracle_pubkey.clone());
        let mut record: OracleGovernanceRecord = env.storage().instance()
            .get(&key)
            .ok_or("Oracle not found in registry")?;

        if record.status == OracleGovernanceStatus::Blacklisted {
            return Err("Cannot reactivate blacklisted oracle");
        }

        record.status = OracleGovernanceStatus::Whitelisted;
        record.updated_at = env.ledger().timestamp();
        record.reason = String::from_str(&env, "Reactivated by governance");

        env.storage().instance().set(&key, &record);

        OracleReactivated {
            oracle_pubkey,
            reactivated_by: admin,
            timestamp: env.ledger().timestamp(),
        }
        .publish(&env);

        Ok(())
    }

    /// Migrate a lease from compromised oracle to healthy oracle
    pub fn migrate_lease_oracle(
        env: Env,
        caller: Address,
        lease_id: u64,
        old_oracle: BytesN<32>,
        new_oracle: BytesN<32>,
    ) -> Result<(), &'static str> {
        caller.require_auth();

        let new_oracle_key = OracleGovernanceDataKey::OracleRecord(new_oracle.clone());
        let new_oracle_record: OracleGovernanceRecord = env.storage().instance()
            .get(&new_oracle_key)
            .ok_or("New oracle not whitelisted")?;

        if new_oracle_record.status != OracleGovernanceStatus::Whitelisted {
            return Err("New oracle not active");
        }

        let migration = OracleMigration {
            lease_id,
            old_oracle: old_oracle.clone(),
            new_oracle: new_oracle.clone(),
            migrated_at: env.ledger().timestamp(),
            migrated_by: caller.clone(),
        };

        env.storage().instance().set(
            &OracleGovernanceDataKey::MigrationHistory(lease_id),
            &migration
        );

        env.storage().instance().set(
            &OracleGovernanceDataKey::ActiveLeaseOracle(lease_id),
            &new_oracle
        );

        LeaseOracleMigrated {
            lease_id,
            old_oracle,
            new_oracle,
            migrated_by: caller,
            timestamp: env.ledger().timestamp(),
        }
        .publish(&env);

        Ok(())
    }

    /// Check if oracle is whitelisted and active
    pub fn is_oracle_active(env: Env, oracle_pubkey: BytesN<32>) -> bool {
        let key = OracleGovernanceDataKey::OracleRecord(oracle_pubkey);
        if let Some(record) = env.storage().instance().get::<_, OracleGovernanceRecord>(&key) {
            return record.status == OracleGovernanceStatus::Whitelisted;
        }
        false
    }

    /// Check if oracle is blacklisted
    pub fn is_oracle_blacklisted(env: Env, oracle_pubkey: BytesN<32>) -> bool {
        let key = OracleGovernanceDataKey::OracleRecord(oracle_pubkey);
        if let Some(record) = env.storage().instance().get::<_, OracleGovernanceRecord>(&key) {
            return record.status == OracleGovernanceStatus::Blacklisted;
        }
        false
    }

    /// Get oracle governance record
    pub fn get_oracle_record(
        env: Env,
        oracle_pubkey: BytesN<32>,
    ) -> Result<OracleGovernanceRecord, &'static str> {
        let key = OracleGovernanceDataKey::OracleRecord(oracle_pubkey);
        env.storage().instance()
            .get(&key)
            .ok_or("Oracle not found in registry")
    }

    /// Get active oracle for a lease
    pub fn get_lease_oracle(env: Env, lease_id: u64) -> Option<BytesN<32>> {
        env.storage().instance()
            .get(&OracleGovernanceDataKey::ActiveLeaseOracle(lease_id))
    }

    /// Get migration history for a lease
    pub fn get_migration_history(env: Env, lease_id: u64) -> Option<OracleMigration> {
        env.storage().instance()
            .get(&OracleGovernanceDataKey::MigrationHistory(lease_id))
    }

    /// Add governance admin
    pub fn add_governance_admin(
        env: Env,
        caller: Address,
        new_admin: Address,
    ) -> Result<(), &'static str> {
        Self::require_admin(&env, &caller)?;
        caller.require_auth();

        env.storage().instance().set(
            &OracleGovernanceDataKey::GovernanceAdmin(new_admin.clone()),
            &true
        );

        Ok(())
    }

    /// Remove governance admin
    pub fn remove_governance_admin(
        env: Env,
        caller: Address,
        admin_to_remove: Address,
    ) -> Result<(), &'static str> {
        Self::require_admin(&env, &caller)?;
        caller.require_auth();

        env.storage().instance().remove(
            &OracleGovernanceDataKey::GovernanceAdmin(admin_to_remove)
        );

        Ok(())
    }

    /// Initialize governance admin
    pub fn initialize_governance(
        env: Env,
        admin: Address,
    ) -> Result<(), &'static str> {
        admin.require_auth();
        
        if env.storage().instance().has(&OracleGovernanceDataKey::GovernanceAdmin(admin.clone())) {
            return Err("Already initialized");
        }

        env.storage().instance().set(
            &OracleGovernanceDataKey::GovernanceAdmin(admin.clone()),
            &true
        );

        Ok(())
    }

    /// Internal: Verify admin
    fn require_admin(env: &Env, address: &Address) -> Result<(), &'static str> {
        let is_admin: bool = env.storage().instance()
            .get(&OracleGovernanceDataKey::GovernanceAdmin(address.clone()))
            .unwrap_or(false);
        
        if !is_admin {
            return Err("Not a governance admin");
        }
        
        Ok(())
    }
}
