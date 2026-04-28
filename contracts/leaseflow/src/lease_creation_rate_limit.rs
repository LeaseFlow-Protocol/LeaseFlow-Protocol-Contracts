// Lease Creation Rate Limiting for LeaseFlow Protocol
// Protects against DoS spam attacks by limiting lease creation frequency

use soroban_sdk::{
    contractevent, contracttype, Address, Env, Vec, u64, u32, Symbol, Map,
};

// Rate limit configuration
const CREATION_WINDOW: u64 = 3600; // 1 hour in seconds
const MAX_CREATIONS_PER_WINDOW: u32 = 10; // Max 10 leases per hour per address
const MAX_CREATIONS_PER_DAY: u32 = 50; // Max 50 leases per day per address

// Creation tracking for address
#[derive(Clone)]
#[contracttype]
pub struct CreationTracker {
    pub address: Address,
    pub creations_1h: u32,
    pub creations_24h: u32,
    pub creation_timestamps_1h: Vec<u64>,
    pub creation_timestamps_24h: Vec<u64>,
    pub is_rate_limited: bool,
    pub rate_limit_until: Option<u64>,
}

// Events
#[contractevent]
pub struct LeaseCreationRateLimitExceeded {
    pub address: Address,
    pub creations_1h: u32,
    pub creations_24h: u32,
    pub timestamp: u64,
}

#[contractevent]
pub struct LeaseCreationRateLimitReset {
    pub address: Address,
    pub reset_timestamp: u64,
}

// Rate limit implementation
pub struct LeaseCreationRateLimit;

impl LeaseCreationRateLimit {
    /// Check rate limits for an address before allowing lease creation
    pub fn check_creation_limits(env: &Env, address: &Address) -> Result<(), Error> {
        // Check if address is currently rate-limited
        if Self::is_rate_limited(env, address) {
            return Err(Error::RateLimitExceeded);
        }
        
        // Get creation tracker
        let mut tracker = Self::get_creation_tracker(env, address)?;
        
        // Clean old creation records
        Self::cleanup_old_creations(env, &mut tracker);
        
        // Check 1-hour limit
        if tracker.creations_1h >= MAX_CREATIONS_PER_WINDOW {
            Self::trigger_rate_limit(env, address, &tracker)?;
            return Err(Error::RateLimitExceeded);
        }
        
        // Check 24-hour limit
        if tracker.creations_24h >= MAX_CREATIONS_PER_DAY {
            Self::trigger_rate_limit(env, address, &tracker)?;
            return Err(Error::RateLimitExceeded);
        }
        
        Ok(())
    }
    
    /// Record a lease creation for rate tracking
    pub fn record_creation(env: &Env, address: &Address) -> Result<(), Error> {
        let mut tracker = Self::get_creation_tracker(env, address)?;
        
        // Add creation timestamp
        let current_time = env.ledger().timestamp();
        tracker.creation_timestamps_1h.push_back(current_time);
        tracker.creation_timestamps_24h.push_back(current_time);
        tracker.creations_1h += 1;
        tracker.creations_24h += 1;
        
        // Update tracker
        Self::save_creation_tracker(env, address, &tracker);
        
        Ok(())
    }
    
    /// Initialize creation tracker for a new address
    pub fn initialize_address(env: &Env, address: &Address) -> Result<(), Error> {
        if Self::has_creation_tracker(env, address) {
            return Ok(());
        }
        
        let tracker = CreationTracker {
            address: address.clone(),
            creations_1h: 0,
            creations_24h: 0,
            creation_timestamps_1h: Vec::new(env),
            creation_timestamps_24h: Vec::new(env),
            is_rate_limited: false,
            rate_limit_until: None,
        };
        
        Self::save_creation_tracker(env, address, &tracker);
        Ok(())
    }
    
    /// Get creation tracker for an address
    pub fn get_creation_tracker(env: &Env, address: &Address) -> Result<CreationTracker, Error> {
        let key = Symbol::short("rate_limit");
        let mut map: Map<Address, CreationTracker> = env.storage().temporary().get(&key).unwrap_or(Map::new(env));
        
        if let Some(tracker) = map.get(address) {
            Ok(tracker)
        } else {
            // If evicted from temporary storage, return a default fresh tracker
            Ok(CreationTracker {
                address: address.clone(),
                creations_1h: 0,
                creations_24h: 0,
                creation_timestamps_1h: Vec::new(env),
                creation_timestamps_24h: Vec::new(env),
                is_rate_limited: false,
                rate_limit_until: None,
            })
        }
    }
    
    /// Check if creation tracker exists
    pub fn has_creation_tracker(env: &Env, address: &Address) -> bool {
        let key = Symbol::short("rate_limit");
        if let Some(map) = env.storage().temporary().get::<_, Map<Address, CreationTracker>>(&key) {
            map.contains_key(address)
        } else {
            false
        }
    }
    
    /// Save creation tracker
    pub fn save_creation_tracker(env: &Env, address: &Address, tracker: &CreationTracker) {
        let key = Symbol::short("rate_limit");
        let mut map: Map<Address, CreationTracker> = env.storage().temporary().get(&key).unwrap_or(Map::new(env));
        map.set(address, tracker);
        
        // Extend TTL to roughly 7 days
        const TRACKER_TTL: u32 = 120_960; // ~7 days at ~5s per ledger
        env.storage().temporary().set(&key, &map);
        env.storage().temporary().extend_ttl(&key, TRACKER_TTL, TRACKER_TTL);
    }
    
    /// Check if address is currently rate-limited
    pub fn is_rate_limited(env: &Env, address: &Address) -> bool {
        let tracker = match Self::get_creation_tracker(env, address) {
            Ok(t) => t,
            Err(_) => return false,
        };
        
        if !tracker.is_rate_limited {
            return false;
        }
        
        // Check if rate limit has expired
        if let Some(until) = tracker.rate_limit_until {
            let current_time = env.ledger().timestamp();
            if current_time >= until {
                // Auto-reset expired rate limit
                let _ = Self::reset_rate_limit(env, address);
                return false;
            }
        }
        
        tracker.is_rate_limited
    }
    
    /// Clean up old creation records
    fn cleanup_old_creations(env: &Env, tracker: &mut CreationTracker) {
        let current_time = env.ledger().timestamp();
        let cutoff_1h = current_time - CREATION_WINDOW;
        let cutoff_24h = current_time - (CREATION_WINDOW * 24);
        
        // Remove old timestamps for 1-hour window
        let mut new_timestamps_1h = Vec::new(env);
        let mut count_1h = 0u32;
        
        for timestamp in tracker.creation_timestamps_1h.iter() {
            if timestamp >= cutoff_1h {
                new_timestamps_1h.push_back(timestamp);
                count_1h += 1;
            }
        }
        
        tracker.creation_timestamps_1h = new_timestamps_1h;
        tracker.creations_1h = count_1h;
        
        // Remove old timestamps for 24-hour window
        let mut new_timestamps_24h = Vec::new(env);
        let mut count_24h = 0u32;
        
        for timestamp in tracker.creation_timestamps_24h.iter() {
            if timestamp >= cutoff_24h {
                new_timestamps_24h.push_back(timestamp);
                count_24h += 1;
            }
        }
        
        tracker.creation_timestamps_24h = new_timestamps_24h;
        tracker.creations_24h = count_24h;
    }
    
    /// Trigger rate limit when thresholds are exceeded
    fn trigger_rate_limit(
        env: &Env,
        address: &Address,
        tracker: &CreationTracker,
    ) -> Result<(), Error> {
        // Emit rate limit exceeded event
        env.events().publish(
            symbol!("RATE_LIMIT"),
            LeaseCreationRateLimitExceeded {
                address: address.clone(),
                creations_1h: tracker.creations_1h,
                creations_24h: tracker.creations_24h,
                timestamp: env.ledger().timestamp(),
            }
        );
        
        // Apply rate limit for 1 hour
        let mut updated_tracker = tracker.clone();
        updated_tracker.is_rate_limited = true;
        updated_tracker.rate_limit_until = Some(env.ledger().timestamp() + CREATION_WINDOW);
        
        Self::save_creation_tracker(env, address, &updated_tracker);
        
        Ok(())
    }
    
    /// Reset rate limit (can be called by admin or after timeout)
    pub fn reset_rate_limit(env: &Env, address: &Address) -> Result<(), Error> {
        let mut tracker = Self::get_creation_tracker(env, address)?;
        
        tracker.is_rate_limited = false;
        tracker.rate_limit_until = None;
        
        Self::save_creation_tracker(env, address, &tracker);
        
        // Emit reset event
        env.events().publish(
            symbol!("RATE_LIMIT_RESET"),
            LeaseCreationRateLimitReset {
                address: address.clone(),
                reset_timestamp: env.ledger().timestamp(),
            }
        );
        
        Ok(())
    }
    
    /// Get creation statistics for monitoring
    pub fn get_creation_stats(env: &Env, address: &Address) -> Result<(u32, u32, bool, Option<u64>), Error> {
        let tracker = Self::get_creation_tracker(env, address)?;
        
        Ok((
            tracker.creations_1h,
            tracker.creations_24h,
            tracker.is_rate_limited,
            tracker.rate_limit_until,
        ))
    }
}
