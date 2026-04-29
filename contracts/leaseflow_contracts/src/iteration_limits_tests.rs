//! Tests for bounded iteration limits functionality in leaseflow_contracts

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, Address, Symbol};

    #[test]
    fn test_iteration_limits_default_values() {
        let limits = IterationController::get_limits();
        assert_eq!(limits.max_active_leases, 200);
        assert_eq!(limits.max_batch_operations, 50);
        assert_eq!(limits.max_velocity_tracking, 100);
        assert_eq!(limits.max_approval_entries, 10);
    }

    #[test]
    fn test_active_leases_validation() {
        // Test within limits
        assert!(IterationController::validate_active_leases_iteration(150).is_ok());
        assert!(IterationController::validate_active_leases_iteration(200).is_ok());
        
        // Test exceeding limits
        assert!(IterationController::validate_active_leases_iteration(201).is_err());
        assert!(IterationController::validate_active_leases_iteration(300).is_err());
    }

    #[test]
    fn test_batch_operations_validation() {
        // Test within limits
        assert!(IterationController::validate_batch_operations(25).is_ok());
        assert!(IterationController::validate_batch_operations(50).is_ok());
        
        // Test exceeding limits
        assert!(IterationController::validate_batch_operations(51).is_err());
        assert!(IterationController::validate_batch_operations(100).is_err());
    }

    #[test]
    fn test_velocity_tracking_validation() {
        // Test within limits
        assert!(IterationController::validate_velocity_tracking(50).is_ok());
        assert!(IterationController::validate_velocity_tracking(100).is_ok());
        
        // Test exceeding limits
        assert!(IterationController::validate_velocity_tracking(101).is_err());
        assert!(IterationController::validate_velocity_tracking(200).is_err());
    }

    #[test]
    fn test_approval_entries_validation() {
        // Test within limits
        assert!(IterationController::validate_approval_entries(5).is_ok());
        assert!(IterationController::validate_approval_entries(10).is_ok());
        
        // Test exceeding limits
        assert!(IterationController::validate_approval_entries(11).is_err());
        assert!(IterationController::validate_approval_entries(20).is_err());
    }

    #[test]
    fn test_bounded_iterator() {
        let env = Env::default();
        
        // Test with small collection
        let mut small_vec = Vec::new(&env);
        for i in 0..10 {
            small_vec.push_back(i);
        }
        
        let mut iterator = BoundedIterator::new(&small_vec, 20).unwrap();
        let mut count = 0;
        let mut values = Vec::new(&env);
        
        while let Some(value) = iterator.next() {
            values.push_back(value);
            count += 1;
        }
        
        assert_eq!(count, 10); // Should iterate through all items
        assert_eq!(iterator.count(), 10);
        
        // Test with large collection and small limit
        let mut large_vec = Vec::new(&env);
        for i in 0..250 {
            large_vec.push_back(i);
        }
        
        let mut iterator = BoundedIterator::new(&large_vec, 200).unwrap();
        let mut count = 0;
        
        while let Some(_) = iterator.next() {
            count += 1;
        }
        
        assert_eq!(count, 200); // Should stop at limit
        assert_eq!(iterator.count(), 200);
    }

    #[test]
    fn test_iteration_utils_bounded_contains() {
        let env = Env::default();
        let mut vec = Vec::new(&env);
        
        // Add some test values
        for i in 0..50 {
            vec.push_back(i * 2); // Even numbers only
        }
        
        // Test finding existing value within limit
        assert!(IterationUtils::bounded_contains(&vec, &20, 30));
        assert!(IterationUtils::bounded_contains(&vec, &0, 10));
        
        // Test finding non-existing value
        assert!(!IterationUtils::bounded_contains(&vec, &21, 30));
        assert!(!IterationUtils::bounded_contains(&vec, &100, 30));
        
        // Test with limited search
        assert!(IterationUtils::bounded_contains(&vec, &80, 50)); // Should find it
        assert!(!IterationUtils::bounded_contains(&vec, &80, 40)); // Should not find it due to limit
    }

    #[test]
    fn test_iteration_utils_bounded_find() {
        let env = Env::default();
        let mut vec = Vec::new(&env);
        
        // Add test values
        for i in 0..30 {
            vec.push_back(i + 10);
        }
        
        // Test finding existing value
        assert_eq!(IterationUtils::bounded_find(&vec, &15, 50), Some(5));
        assert_eq!(IterationUtils::bounded_find(&vec, &25, 50), Some(15));
        
        // Test finding non-existing value
        assert_eq!(IterationUtils::bounded_find(&vec, &5, 50), None);
        assert_eq!(IterationUtils::bounded_find(&vec, &50, 50), None);
        
        // Test with limited search
        assert_eq!(IterationUtils::bounded_find(&vec, &35, 30), Some(25)); // Should find it
        assert_eq!(IterationUtils::bounded_find(&vec, &35, 20), None); // Should not find it due to limit
    }

    #[test]
    fn test_get_active_leases_bounded_iteration() {
        let env = Env::default();
        let contract_id = env.register_contract(None, crate::LeaseContract);
        let client = crate::LeaseContractClient::new(&env, &contract_id);
        
        // Initialize contract
        let admin = Address::generate(&env);
        let token = Address::generate(&env);
        client.initialize(&admin, &token);
        
        // Create many active leases to test bounds
        let landlord = Address::generate(&env);
        let tenant = Address::generate(&env);
        
        // Create leases up to and beyond the limit
        for i in 0..250 {
            let lease_id = 1000 + i;
            
            // Create lease data
            let lease = crate::test_utils::make_lease(&env, lease_id, &landlord, &tenant);
            
            // Store lease instance
            let lease_key = crate::DataKey::LeaseInstance(lease_id);
            env.storage().instance().set(&lease_key, &lease);
            
            // Add to active leases index
            let mut index: soroban_sdk::Vec<u64> = env.storage().instance()
                .get(&crate::DataKey::ActiveLeasesIndex)
                .unwrap_or(soroban_sdk::Vec::new(&env));
            index.push_back(lease_id);
            env.storage().instance().set(&crate::DataKey::ActiveLeasesIndex, &index);
        }
        
        // Should return bounded results
        let result = client.get_active_leases();
        assert!(result.is_ok());
        let active_leases = result.unwrap();
        assert!(active_leases.len() <= 200); // Should be bounded by max_active_leases
    }

    #[test]
    fn test_batch_health_check_bounded_iteration() {
        let env = Env::default();
        
        // Create a large batch of lease IDs
        let mut lease_ids = Vec::new(&env);
        for i in 0..100 {
            lease_ids.push_back(1000 + i);
        }
        
        // Test with batch size within limits
        let small_batch = Vec::from_slice(&env, &[1001u64, 1002u64, 1003u64]);
        let result = crate::collateral_health_monitor::CollateralHealthMonitor::batch_health_check(
            env.clone(), 
            small_batch
        );
        assert!(result.is_ok());
        
        // Test with batch size exceeding limits
        let large_batch = Vec::new(&env);
        for i in 0..75 { // Exceeds max_batch_operations of 50
            large_batch.push_back(1000 + i);
        }
        
        let result = crate::collateral_health_monitor::CollateralHealthMonitor::batch_health_check(
            env.clone(), 
            large_batch
        );
        assert!(result.is_err());
        assert_eq!(result.err(), Some(crate::CollateralHealthError::BatchSizeExceeded));
    }

    #[test]
    fn test_velocity_guard_bounded_iteration() {
        let env = Env::default();
        let lessor = Address::generate(&env);
        
        // Create a velocity tracker with many termination times
        let mut tracker = crate::velocity_guard::VelocityTracker {
            lessor: lessor.clone(),
            total_leases: 200,
            terminations_24h: 150,
            last_termination_times: Vec::new(&env),
            is_paused: false,
            pause_timestamp: None,
        };
        
        // Add many termination timestamps
        let current_time = env.ledger().timestamp();
        for i in 0..150 {
            tracker.last_termination_times.push_back(current_time - (i * 3600)); // Each 1 hour apart
        }
        
        // Test cleanup with bounded iteration
        crate::velocity_guard::VelocityGuard::cleanup_old_terminations(&env, &mut tracker);
        
        // The cleanup should be bounded and not process all entries if they exceed the limit
        assert!(tracker.last_termination_times.len() <= 100); // Should be bounded by max_velocity_tracking
    }

    #[test]
    fn test_iteration_limit_error_codes() {
        // Test that error codes are unique and properly defined
        assert_eq!(IterationLimitError::ActiveLeasesLimitExceeded as u32, 2001);
        assert_eq!(IterationLimitError::BatchOperationsLimitExceeded as u32, 2002);
        assert_eq!(IterationLimitError::VelocityTrackingLimitExceeded as u32, 2003);
        assert_eq!(IterationLimitError::ApprovalEntriesLimitExceeded as u32, 2004);
        assert_eq!(IterationLimitError::InvalidIterationLimit as u32, 2005);
    }

    #[test]
    fn test_main_contract_error_integration() {
        // Test that main contract errors are properly integrated
        assert_eq!(crate::LeaseError::ActiveLeasesLimitExceeded as u32, 42);
        assert_eq!(crate::LeaseError::BatchOperationsLimitExceeded as u32, 43);
        assert_eq!(crate::LeaseError::VelocityTrackingLimitExceeded as u32, 44);
        assert_eq!(crate::LeaseError::ApprovalEntriesLimitExceeded as u32, 45);
    }

    #[test]
    fn test_gas_efficiency_with_bounded_iteration() {
        let env = Env::default();
        
        // Create a large vector to test gas efficiency
        let mut large_vec = Vec::new(&env);
        for i in 0..1000 {
            large_vec.push_back(i);
        }
        
        // Measure gas usage with bounded iteration
        let start_gas = env.storage().instance().budget().cpu_instructions();
        
        let mut iterator = BoundedIterator::new(&large_vec, 200).unwrap();
        let mut count = 0;
        
        while let Some(_) = iterator.next() {
            count += 1;
        }
        
        let end_gas = env.storage().instance().budget().cpu_instructions();
        let gas_used = end_gas - start_gas;
        
        // Should only iterate 200 times regardless of vector size
        assert_eq!(count, 200);
        
        // Gas usage should be proportional to the limit, not the vector size
        assert!(gas_used < 2000000); // Should be reasonable
    }

    #[test]
    fn test_concurrent_iteration_safety() {
        let env = Env::default();
        
        // Create multiple bounded iterators from the same vector
        let mut vec = Vec::new(&env);
        for i in 0..300 {
            vec.push_back(i);
        }
        
        let mut iterator1 = BoundedIterator::new(&vec, 100).unwrap();
        let mut iterator2 = BoundedIterator::new(&vec, 150).unwrap();
        let mut iterator3 = BoundedIterator::new(&vec, 50).unwrap();
        
        let mut count1 = 0;
        let mut count2 = 0;
        let mut count3 = 0;
        
        while let Some(_) = iterator1.next() {
            count1 += 1;
        }
        
        while let Some(_) = iterator2.next() {
            count2 += 1;
        }
        
        while let Some(_) = iterator3.next() {
            count3 += 1;
        }
        
        assert_eq!(count1, 100);
        assert_eq!(count2, 150);
        assert_eq!(count3, 50);
    }

    #[test]
    fn test_edge_cases() {
        let env = Env::default();
        
        // Test empty vector
        let empty_vec: Vec<u32> = Vec::new(&env);
        let mut iterator = BoundedIterator::new(&empty_vec, 10).unwrap();
        let mut count = 0;
        
        while let Some(_) = iterator.next() {
            count += 1;
        }
        
        assert_eq!(count, 0);
        assert_eq!(iterator.count(), 0);
        
        // Test single item vector
        let mut single_vec = Vec::new(&env);
        single_vec.push_back(42);
        
        let mut iterator = BoundedIterator::new(&single_vec, 10).unwrap();
        let mut count = 0;
        let mut found_value = None;
        
        while let Some(value) = iterator.next() {
            found_value = Some(value);
            count += 1;
        }
        
        assert_eq!(count, 1);
        assert_eq!(found_value, Some(42));
        assert_eq!(iterator.count(), 1);
        
        // Test limit equal to vector size
        let mut medium_vec = Vec::new(&env);
        for i in 0..25 {
            medium_vec.push_back(i);
        }
        
        let mut iterator = BoundedIterator::new(&medium_vec, 25).unwrap();
        let mut count = 0;
        
        while let Some(_) = iterator.next() {
            count += 1;
        }
        
        assert_eq!(count, 25);
        assert_eq!(iterator.count(), 25);
    }

    #[test]
    fn test_iteration_warning_events() {
        let env = Env::default();
        
        // Test warning emission for different operations
        IterationUtils::emit_iteration_warning(&env, "test_operation_1", 150, 100);
        IterationUtils::emit_iteration_warning(&env, "test_operation_2", 75, 50);
        IterationUtils::emit_iteration_warning(&env, "test_operation_3", 25, 25);
        
        // Verify events were emitted
        let events = env.events().all();
        assert_eq!(events.len(), 3);
        
        // Check event structure
        for event in events {
            if let Some((topic, data)) = event {
                assert_eq!(topic, soroban_sdk::symbol!("IterationWarning"));
                // data should be (operation, actual_count, limit)
            }
        }
    }

    #[test]
    fn test_bounded_iterator_with_different_types() {
        let env = Env::default();
        
        // Test with Address type
        let mut address_vec = Vec::new(&env);
        for _ in 0..10 {
            address_vec.push_back(Address::generate(&env));
        }
        
        let mut iterator = BoundedIterator::new(&address_vec, 5).unwrap();
        let mut count = 0;
        
        while let Some(_) = iterator.next() {
            count += 1;
        }
        
        assert_eq!(count, 5);
        
        // Test with Symbol type
        let mut symbol_vec = Vec::new(&env);
        for i in 0..10 {
            symbol_vec.push_back(Symbol::short(&format!("sym_{}", i)));
        }
        
        let mut iterator = BoundedIterator::new(&symbol_vec, 7).unwrap();
        let mut count = 0;
        
        while let Some(_) = iterator.next() {
            count += 1;
        }
        
        assert_eq!(count, 7);
    }
}
