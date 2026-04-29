//! Tests for bounded iteration limits functionality

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, Address, Symbol};

    #[test]
    fn test_iteration_limits_default_values() {
        let limits = IterationController::get_limits();
        assert_eq!(limits.max_user_leases, 100);
        assert_eq!(limits.max_active_leases, 200);
        assert_eq!(limits.max_batch_operations, 50);
        assert_eq!(limits.max_associated_lease_ids, 50);
        assert_eq!(limits.max_credit_record_entries, 100);
    }

    #[test]
    fn test_user_leases_validation() {
        // Test within limits
        assert!(IterationController::validate_user_leases_iteration(50).is_ok());
        assert!(IterationController::validate_user_leases_iteration(100).is_ok());
        
        // Test exceeding limits
        assert!(IterationController::validate_user_leases_iteration(101).is_err());
        assert!(IterationController::validate_user_leases_iteration(200).is_err());
    }

    #[test]
    fn test_associated_lease_ids_validation() {
        // Test within limits
        assert!(IterationController::validate_associated_lease_ids(25).is_ok());
        assert!(IterationController::validate_associated_lease_ids(50).is_ok());
        
        // Test exceeding limits
        assert!(IterationController::validate_associated_lease_ids(51).is_err());
        assert!(IterationController::validate_associated_lease_ids(100).is_err());
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
        for i in 0..150 {
            large_vec.push_back(i);
        }
        
        let mut iterator = BoundedIterator::new(&large_vec, 50).unwrap();
        let mut count = 0;
        
        while let Some(_) = iterator.next() {
            count += 1;
        }
        
        assert_eq!(count, 50); // Should stop at limit
        assert_eq!(iterator.count(), 50);
    }

    #[test]
    fn test_bounded_iterator_invalid_limit() {
        let env = Env::default();
        let vec = Vec::new(&env);
        
        // Test with zero limit
        assert!(BoundedIterator::new(&vec, 0).is_err());
    }

    #[test]
    fn test_iteration_utils() {
        // Test safe collection size check
        assert!(IterationUtils::is_safe_collection_size(50, 100));
        assert!(!IterationUtils::is_safe_collection_size(150, 100));
        
        // Test safe iteration limit calculation
        assert_eq!(IterationUtils::get_safe_iteration_limit(50, 100), 50);
        assert_eq!(IterationUtils::get_safe_iteration_limit(150, 100), 100);
    }

    #[test]
    fn test_iteration_warning_emission() {
        let env = Env::default();
        
        // Test warning emission (should not panic)
        IterationUtils::emit_iteration_warning(&env, "test_operation", 150, 100);
        
        // Verify the event was emitted by checking the event logs
        let events = env.events().all();
        assert!(!events.is_empty());
        
        // Find our specific event
        let mut found = false;
        for event in events {
            if let Some((topic, data)) = event {
                if topic == soroban_sdk::symbol!("IterationWarning") {
                    found = true;
                    break;
                }
            }
        }
        assert!(found);
    }

    #[test]
    fn test_get_user_leases_bounded_iteration() {
        let env = Env::default();
        let contract_id = env.register_contract(None, crate::LeaseFlowContract);
        let client = crate::LeaseFlowContractClient::new(&env, &contract_id);
        
        // Initialize contract
        let admin = Address::generate(&env);
        let oracle_address = Address::generate(&env);
        client.initialize(&oracle_address);
        
        // Create test user
        let user = Address::generate(&env);
        
        // Test with no leases (should work fine)
        let result = client.get_user_leases(&user);
        assert!(result.is_ok());
        let user_leases = result.unwrap();
        assert_eq!(user_leases.len(), 0);
        
        // Create many leases to test bounds
        let lessor = Address::generate(&env);
        let lessee = user.clone();
        
        // Create leases up to the limit
        for i in 0..95 {
            let lease_id = client.create_lease(
                &lessor,
                &lessee,
                &1000i64, // rent_amount
                &500i64,  // deposit_amount
                &100u64,  // start_date
                &200u64,  // end_date
                &432000u64, // max_grace_period
                &500u32,  // late_fee_rate
                &soroban_sdk::Bytes::from_slice(&env, b"test_property"),
                &None::<crate::FiatPegConfig>,
            ).unwrap();
        }
        
        // Should still work within limits
        let result = client.get_user_leases(&user);
        assert!(result.is_ok());
        let user_leases = result.unwrap();
        assert!(user_leases.len() <= 100); // Should be bounded
    }

    #[test]
    fn test_credit_record_bounded_iteration() {
        let env = Env::default();
        let contract_id = env.register_contract(None, crate::LeaseFlowContract);
        let client = crate::LeaseFlowContractClient::new(&env, &contract_id);
        
        // Initialize contract
        let oracle_address = Address::generate(&env);
        client.initialize(&oracle_address);
        
        let lessee = Address::generate(&env);
        
        // Create a lease first
        let lessor = Address::generate(&env);
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000i64,
            &500i64,
            &100u64,
            &200u64,
            &432000u64,
            &500u32,
            &soroban_sdk::Bytes::from_slice(&env, b"test_property"),
            &None::<crate::FiatPegConfig>,
        ).unwrap();
        
        // Activate lease
        client.activate_lease(&lease_id, &lessee).unwrap();
        
        // Trigger arrears to create credit record
        client.handle_rent_payment_failure(&lease_id).unwrap();
        
        // Check grace period expiry to trigger arrears deduction
        // This should create a credit record with associated lease IDs
        let result = client.check_grace_period_expiry(&lease_id);
        assert!(result.is_ok());
        
        // Verify credit record exists and has bounded associated lease IDs
        let credit_record = client.get_credit_record(&lessee).unwrap();
        assert!(credit_record.associated_lease_ids.len() <= 50); // Should be bounded
    }

    #[test]
    fn test_iteration_limit_error_codes() {
        // Test that error codes are unique and properly defined
        assert_eq!(IterationLimitError::UserLeasesLimitExceeded as u32, 1001);
        assert_eq!(IterationLimitError::ActiveLeasesLimitExceeded as u32, 1002);
        assert_eq!(IterationLimitError::BatchOperationsLimitExceeded as u32, 1003);
        assert_eq!(IterationLimitError::AssociatedLeaseIdsLimitExceeded as u32, 1004);
        assert_eq!(IterationLimitError::CreditRecordLimitExceeded as u32, 1005);
        assert_eq!(IterationLimitError::InvalidIterationLimit as u32, 1006);
    }

    #[test]
    fn test_main_contract_error_integration() {
        // Test that main contract errors are properly integrated
        assert_eq!(crate::Error::UserLeasesLimitExceeded as u32, 21);
        assert_eq!(crate::Error::AssociatedLeaseIdsLimitExceeded as u32, 22);
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
        
        let mut iterator = BoundedIterator::new(&large_vec, 100).unwrap();
        let mut count = 0;
        
        while let Some(_) = iterator.next() {
            count += 1;
        }
        
        let end_gas = env.storage().instance().budget().cpu_instructions();
        let gas_used = end_gas - start_gas;
        
        // Should only iterate 100 times regardless of vector size
        assert_eq!(count, 100);
        
        // Gas usage should be proportional to the limit, not the vector size
        // This is a rough check - in practice, you'd want more sophisticated gas measurement
        assert!(gas_used < 1000000); // Should be reasonable
    }

    #[test]
    fn test_concurrent_iteration_safety() {
        let env = Env::default();
        
        // Create multiple bounded iterators from the same vector
        let mut vec = Vec::new(&env);
        for i in 0..200 {
            vec.push_back(i);
        }
        
        let mut iterator1 = BoundedIterator::new(&vec, 50).unwrap();
        let mut iterator2 = BoundedIterator::new(&vec, 75).unwrap();
        
        let mut count1 = 0;
        let mut count2 = 0;
        
        while let Some(_) = iterator1.next() {
            count1 += 1;
        }
        
        while let Some(_) = iterator2.next() {
            count2 += 1;
        }
        
        assert_eq!(count1, 50);
        assert_eq!(count2, 75);
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
    }
}
