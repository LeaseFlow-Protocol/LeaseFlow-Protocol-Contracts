// Tests for Lease Creation Rate Limiting (Issue #179)

#[cfg(test)]
mod tests {
    use soroban_sdk::{Address, Env, Symbol};
    use crate::lease_creation_rate_limit::LeaseCreationRateLimit;
    use crate::LeaseError;

    #[test]
    fn test_initialize_address() {
        let env = Env::default();
        let address = Address::generate(&env);

        // Initialize address for rate limiting
        let result = LeaseCreationRateLimit::initialize_address(&env, &address);
        assert!(result.is_ok());

        // Should not error on re-initialization
        let result = LeaseCreationRateLimit::initialize_address(&env, &address);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_creation_limits_within_threshold() {
        let env = Env::default();
        let address = Address::generate(&env);

        // Initialize address
        LeaseCreationRateLimit::initialize_address(&env, &address).unwrap();

        // Should allow creation within limits
        let result = LeaseCreationRateLimit::check_creation_limits(&env, &address);
        assert!(result.is_ok());
    }

    #[test]
    fn test_record_creation() {
        let env = Env::default();
        let address = Address::generate(&env);

        // Initialize address
        LeaseCreationRateLimit::initialize_address(&env, &address).unwrap();

        // Record a creation
        let result = LeaseCreationRateLimit::record_creation(&env, &address);
        assert!(result.is_ok());

        // Check stats
        let stats = LeaseCreationRateLimit::get_creation_stats(&env, &address).unwrap();
        assert_eq!(stats.0, 1); // creations_1h
        assert_eq!(stats.1, 1); // creations_24h
        assert!(!stats.2); // is_rate_limited
    }

    #[test]
    fn test_multiple_recordings() {
        let env = Env::default();
        let address = Address::generate(&env);

        // Initialize address
        LeaseCreationRateLimit::initialize_address(&env, &address).unwrap();

        // Record multiple creations
        for _ in 0..5 {
            LeaseCreationRateLimit::record_creation(&env, &address).unwrap();
        }

        // Check stats
        let stats = LeaseCreationRateLimit::get_creation_stats(&env, &address).unwrap();
        assert_eq!(stats.0, 5); // creations_1h
        assert_eq!(stats.1, 5); // creations_24h
    }

    #[test]
    fn test_rate_limit_exceeded() {
        let env = Env::default();
        let address = Address::generate(&env);

        // Initialize address
        LeaseCreationRateLimit::initialize_address(&env, &address).unwrap();

        // Record creations up to the limit (10 per hour)
        for _ in 0..10 {
            LeaseCreationRateLimit::record_creation(&env, &address).unwrap();
        }

        // Next creation should fail rate limit check
        let result = LeaseCreationRateLimit::check_creation_limits(&env, &address);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), LeaseError::RateLimitExceeded);

        // Check that address is rate-limited
        let stats = LeaseCreationRateLimit::get_creation_stats(&env, &address).unwrap();
        assert!(stats.2); // is_rate_limited
    }

    #[test]
    fn test_reset_rate_limit() {
        let env = Env::default();
        let address = Address::generate(&env);

        // Initialize address
        LeaseCreationRateLimit::initialize_address(&env, &address).unwrap();

        // Record creations up to the limit
        for _ in 0..10 {
            LeaseCreationRateLimit::record_creation(&env, &address).unwrap();
        }

        // Should be rate-limited
        let result = LeaseCreationRateLimit::check_creation_limits(&env, &address);
        assert!(result.is_err());

        // Reset rate limit
        LeaseCreationRateLimit::reset_rate_limit(&env, &address).unwrap();

        // Should no longer be rate-limited
        let stats = LeaseCreationRateLimit::get_creation_stats(&env, &address).unwrap();
        assert!(!stats.2); // is_rate_limited

        // But still has 10 creations, so still blocked by limit
        let result = LeaseCreationRateLimit::check_creation_limits(&env, &address);
        assert!(result.is_err());
    }

    #[test]
    fn test_different_addresses_independent() {
        let env = Env::default();
        let address1 = Address::generate(&env);
        let address2 = Address::generate(&env);

        // Initialize both addresses
        LeaseCreationRateLimit::initialize_address(&env, &address1).unwrap();
        LeaseCreationRateLimit::initialize_address(&env, &address2).unwrap();

        // Rate limit address1
        for _ in 0..10 {
            LeaseCreationRateLimit::record_creation(&env, &address1).unwrap();
        }

        // address1 should be rate-limited
        let result = LeaseCreationRateLimit::check_creation_limits(&env, &address1);
        assert!(result.is_err());

        // address2 should still be able to create leases
        let result = LeaseCreationRateLimit::check_creation_limits(&env, &address2);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_creation_stats() {
        let env = Env::default();
        let address = Address::generate(&env);

        // Initialize address
        LeaseCreationRateLimit::initialize_address(&env, &address).unwrap();

        // Record some creations
        for _ in 0..3 {
            LeaseCreationRateLimit::record_creation(&env, &address).unwrap();
        }

        // Get stats
        let stats = LeaseCreationRateLimit::get_creation_stats(&env, &address).unwrap();
        assert_eq!(stats.0, 3); // creations_1h
        assert_eq!(stats.1, 3); // creations_24h
        assert!(!stats.2); // is_rate_limited
        assert!(stats.3.is_none()); // rate_limit_until
    }
}
