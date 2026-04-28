// Admin Management for Lease Creation Rate Limiting
// Allows administrators to manage rate limit policies and whitelist trusted addresses

use soroban_sdk::{
    contractevent, contracttype, Address, Env, Vec, u32, u64,
};
use crate::{LeaseError, DataKey};

// Rate limit policy configuration
#[derive(Clone)]
#[contracttype]
pub struct RateLimitPolicy {
    pub max_creations_per_hour: u32,
    pub max_creations_per_day: u32,
    pub rate_limit_duration: u64,
    pub enabled: bool,
}

// Whitelisted address for rate limit exemption
#[derive(Clone)]
#[contracttype]
pub struct WhitelistedAddress {
    pub address: Address,
    pub whitelisted_at: u64,
    pub whitelisted_by: Address,
    pub reason: String,
}

// Events
#[contractevent]
pub struct RateLimitPolicyUpdated {
    pub old_max_per_hour: u32,
    pub new_max_per_hour: u32,
    pub old_max_per_day: u32,
    pub new_max_per_day: u32,
    pub updated_by: Address,
    pub timestamp: u64,
}

#[contractevent]
pub struct AddressWhitelisted {
    pub address: Address,
    pub whitelisted_by: Address,
    pub reason: String,
    pub timestamp: u64,
}

#[contractevent]
pub struct AddressRemovedFromWhitelist {
    pub address: Address,
    pub removed_by: Address,
    pub timestamp: u64,
}

#[contractevent]
pub struct RateLimitToggled {
    pub enabled: bool,
    pub toggled_by: Address,
    pub timestamp: u64,
}

// Admin management implementation
pub struct RateLimitAdmin;

impl RateLimitAdmin {
    /// Get the current rate limit policy
    pub fn get_policy(env: &Env) -> RateLimitPolicy {
        env.storage()
            .instance()
            .get(&DataKey::RateLimitPolicy)
            .unwrap_or(RateLimitPolicy {
                max_creations_per_hour: 10,
                max_creations_per_day: 50,
                rate_limit_duration: 3600,
                enabled: true,
            })
    }

    /// Update rate limit policy (admin only)
    pub fn update_policy(
        env: &Env,
        admin: &Address,
        max_per_hour: u32,
        max_per_day: u32,
        rate_limit_duration: u64,
    ) -> Result<(), LeaseError> {
        // Verify admin
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LeaseError::Unauthorised)?;
        
        if admin != &stored_admin {
            return Err(LeaseError::Unauthorised);
        }

        // Validate parameters
        if max_per_hour == 0 || max_per_day == 0 {
            return Err(LeaseError::InvalidDeduction);
        }
        
        if max_per_hour > max_per_day {
            return Err(LeaseError::InvalidDeduction);
        }

        let old_policy = Self::get_policy(env);
        
        let new_policy = RateLimitPolicy {
            max_creations_per_hour: max_per_hour,
            max_creations_per_day: max_per_day,
            rate_limit_duration,
            enabled: old_policy.enabled,
        };

        env.storage().instance().set(&DataKey::RateLimitPolicy, &new_policy);

        // Emit event
        env.events().publish(
            (soroban_sdk::Symbol::short("RATE_LIMIT_POLICY"), admin.clone()),
            RateLimitPolicyUpdated {
                old_max_per_hour: old_policy.max_creations_per_hour,
                new_max_per_hour: max_per_hour,
                old_max_per_day: old_policy.max_creations_per_day,
                new_max_per_day: max_per_day,
                updated_by: admin.clone(),
                timestamp: env.ledger().timestamp(),
            }
        );

        Ok(())
    }

    /// Toggle rate limiting on/off (admin only)
    pub fn toggle_rate_limiting(env: &Env, admin: &Address, enabled: bool) -> Result<(), LeaseError> {
        // Verify admin
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LeaseError::Unauthorised)?;
        
        if admin != &stored_admin {
            return Err(LeaseError::Unauthorised);
        }

        let mut policy = Self::get_policy(env);
        policy.enabled = enabled;

        env.storage().instance().set(&DataKey::RateLimitPolicy, &policy);

        // Emit event
        env.events().publish(
            (soroban_sdk::Symbol::short("RATE_LIMIT_TOGGLE"), admin.clone()),
            RateLimitToggled {
                enabled,
                toggled_by: admin.clone(),
                timestamp: env.ledger().timestamp(),
            }
        );

        Ok(())
    }

    /// Check if an address is whitelisted from rate limits
    pub fn is_whitelisted(env: &Env, address: &Address) -> bool {
        let key = DataKey::RateLimitWhitelist(address.clone());
        env.storage().instance().has(&key)
    }

    /// Add address to whitelist (admin only)
    pub fn whitelist_address(
        env: &Env,
        admin: &Address,
        address: &Address,
        reason: String,
    ) -> Result<(), LeaseError> {
        // Verify admin
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LeaseError::Unauthorised)?;
        
        if admin != &stored_admin {
            return Err(LeaseError::Unauthorised);
        }

        let whitelist_entry = WhitelistedAddress {
            address: address.clone(),
            whitelisted_at: env.ledger().timestamp(),
            whitelisted_by: admin.clone(),
            reason,
        };

        let key = DataKey::RateLimitWhitelist(address.clone());
        env.storage().instance().set(&key, &whitelist_entry);

        // Emit event
        env.events().publish(
            (soroban_sdk::Symbol::short("WHITELIST_ADD"), address.clone()),
            AddressWhitelisted {
                address: address.clone(),
                whitelisted_by: admin.clone(),
                reason: whitelist_entry.reason,
                timestamp: env.ledger().timestamp(),
            }
        );

        Ok(())
    }

    /// Remove address from whitelist (admin only)
    pub fn remove_from_whitelist(
        env: &Env,
        admin: &Address,
        address: &Address,
    ) -> Result<(), LeaseError> {
        // Verify admin
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LeaseError::Unauthorised)?;
        
        if admin != &stored_admin {
            return Err(LeaseError::Unauthorised);
        }

        let key = DataKey::RateLimitWhitelist(address.clone());
        if !env.storage().instance().has(&key) {
            return Err(LeaseError::LeaseNotFound);
        }

        env.storage().instance().remove(&key);

        // Emit event
        env.events().publish(
            (soroban_sdk::Symbol::short("WHITELIST_REMOVE"), address.clone()),
            AddressRemovedFromWhitelist {
                address: address.clone(),
                removed_by: admin.clone(),
                timestamp: env.ledger().timestamp(),
            }
        );

        Ok(())
    }

    /// Get all whitelisted addresses
    pub fn get_whitelist(env: &Env) -> Vec<Address> {
        // Note: This is a simplified implementation
        // In production, you'd want to maintain an index of whitelisted addresses
        // for efficient enumeration
        Vec::new(env)
    }

    /// Get whitelist entry for a specific address
    pub fn get_whitelist_entry(env: &Env, address: &Address) -> Option<WhitelistedAddress> {
        let key = DataKey::RateLimitWhitelist(address.clone());
        env.storage().instance().get(&key)
    }
}
