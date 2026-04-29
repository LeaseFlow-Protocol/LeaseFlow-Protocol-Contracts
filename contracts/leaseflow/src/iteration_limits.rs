//! Bounded Iteration Limits Module
//! 
//! Provides safe iteration limits for on-chain lease history and array read operations
//! to prevent gas exhaustion and denial of service attacks.

use soroban_sdk::{contracterror, contracttype, env, Address, Vec, u64};

/// Maximum iteration limits for different operations
pub const MAX_USER_LEASES_ITERATIONS: u32 = 100;
pub const MAX_ACTIVE_LEASES_ITERATIONS: u32 = 200;
pub const MAX_BATCH_OPERATIONS: u32 = 50;
pub const MAX_ASSOCIATED_LEASE_IDS: u32 = 50;
pub const MAX_CREDIT_RECORD_ENTRIES: u32 = 100;

/// Iteration limit configuration
#[derive(Clone, Debug, contracttype)]
pub struct IterationLimits {
    pub max_user_leases: u32,
    pub max_active_leases: u32,
    pub max_batch_operations: u32,
    pub max_associated_lease_ids: u32,
    pub max_credit_record_entries: u32,
}

impl Default for IterationLimits {
    fn default() -> Self {
        Self {
            max_user_leases: MAX_USER_LEASES_ITERATIONS,
            max_active_leases: MAX_ACTIVE_LEASES_ITERATIONS,
            max_batch_operations: MAX_BATCH_OPERATIONS,
            max_associated_lease_ids: MAX_ASSOCIATED_LEASE_IDS,
            max_credit_record_entries: MAX_CREDIT_RECORD_ENTRIES,
        }
    }
}

/// Error types for iteration limit violations
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum IterationLimitError {
    UserLeasesLimitExceeded = 1001,
    ActiveLeasesLimitExceeded = 1002,
    BatchOperationsLimitExceeded = 1003,
    AssociatedLeaseIdsLimitExceeded = 1004,
    CreditRecordLimitExceeded = 1005,
    InvalidIterationLimit = 1006,
}

/// Bounded iterator for safe iteration over collections
pub struct BoundedIterator<'a, T> {
    items: &'a Vec<T>,
    current_index: u32,
    limit: u32,
}

impl<'a, T> BoundedIterator<'a, T> {
    pub fn new(items: &'a Vec<T>, limit: u32) -> Result<Self, IterationLimitError> {
        if limit == 0 {
            return Err(IterationLimitError::InvalidIterationLimit);
        }
        
        Ok(Self {
            items,
            current_index: 0,
            limit,
        })
    }

    pub fn next(&mut self) -> Option<&T> {
        if self.current_index >= self.limit || self.current_index >= self.items.len() {
            return None;
        }
        
        let item = self.items.get(self.current_index);
        self.current_index += 1;
        item
    }

    pub fn count(&self) -> u32 {
        let actual_len = self.items.len();
        if actual_len <= self.limit {
            actual_len
        } else {
            self.limit
        }
    }
}

/// Main iteration limits controller
pub struct IterationController;

impl IterationController {
    /// Get current iteration limits (can be made configurable)
    pub fn get_limits() -> IterationLimits {
        IterationLimits::default()
    }

    /// Validate user leases iteration limit
    pub fn validate_user_leases_iteration(total_leases: u32) -> Result<(), IterationLimitError> {
        let limits = Self::get_limits();
        if total_leases > limits.max_user_leases {
            Err(IterationLimitError::UserLeasesLimitExceeded)
        } else {
            Ok(())
        }
    }

    /// Validate active leases iteration limit
    pub fn validate_active_leases_iteration(total_leases: u32) -> Result<(), IterationLimitError> {
        let limits = Self::get_limits();
        if total_leases > limits.max_active_leases {
            Err(IterationLimitError::ActiveLeasesLimitExceeded)
        } else {
            Ok(())
        }
    }

    /// Validate batch operations limit
    pub fn validate_batch_operations(batch_size: u32) -> Result<(), IterationLimitError> {
        let limits = Self::get_limits();
        if batch_size > limits.max_batch_operations {
            Err(IterationLimitError::BatchOperationsLimitExceeded)
        } else {
            Ok(())
        }
    }

    /// Validate associated lease IDs limit
    pub fn validate_associated_lease_ids(lease_ids_count: u32) -> Result<(), IterationLimitError> {
        let limits = Self::get_limits();
        if lease_ids_count > limits.max_associated_lease_ids {
            Err(IterationLimitError::AssociatedLeaseIdsLimitExceeded)
        } else {
            Ok(())
        }
    }

    /// Validate credit record entries limit
    pub fn validate_credit_record_entries(entries_count: u32) -> Result<(), IterationLimitError> {
        let limits = Self::get_limits();
        if entries_count > limits.max_credit_record_entries {
            Err(IterationLimitError::CreditRecordLimitExceeded)
        } else {
            Ok(())
        }
    }

    /// Create a bounded iterator for user leases
    pub fn create_bounded_user_leases_iterator<T>(
        leases: &Vec<T>,
    ) -> Result<BoundedIterator<T>, IterationLimitError> {
        let limits = Self::get_limits();
        BoundedIterator::new(leases, limits.max_user_leases)
    }

    /// Create a bounded iterator for active leases
    pub fn create_bounded_active_leases_iterator<T>(
        leases: &Vec<T>,
    ) -> Result<BoundedIterator<T>, IterationLimitError> {
        let limits = Self::get_limits();
        BoundedIterator::new(leases, limits.max_active_leases)
    }

    /// Create a bounded iterator for batch operations
    pub fn create_bounded_batch_iterator<T>(
        items: &Vec<T>,
    ) -> Result<BoundedIterator<T>, IterationLimitError> {
        let limits = Self::get_limits();
        BoundedIterator::new(items, limits.max_batch_operations)
    }

    /// Create a bounded iterator for associated lease IDs
    pub fn create_bounded_associated_leases_iterator<T>(
        lease_ids: &Vec<T>,
    ) -> Result<BoundedIterator<T>, IterationLimitError> {
        let limits = Self::get_limits();
        BoundedIterator::new(lease_ids, limits.max_associated_lease_ids)
    }
}

/// Utility functions for iteration limit management
pub struct IterationUtils;

impl IterationUtils {
    /// Check if a collection size exceeds safe limits
    pub fn is_safe_collection_size(size: u32, limit: u32) -> bool {
        size <= limit
    }

    /// Get safe iteration limit for a given collection size
    pub fn get_safe_iteration_limit(collection_size: u32, max_limit: u32) -> u32 {
        if collection_size <= max_limit {
            collection_size
        } else {
            max_limit
        }
    }

    /// Truncate a vector to safe iteration limits
    pub fn truncate_to_safe_limit<T>(vec: &mut Vec<T>, limit: u32) {
        if vec.len() > limit {
            // In Soroban, we need to create a new vector with limited items
            // This is a simplified approach - in practice, you'd need to handle this differently
            // since Vec::truncate() might not be available
            let env = vec.env();
            let mut new_vec = Vec::new(env);
            for i in 0..limit {
                if let Some(item) = vec.get(i) {
                    new_vec.push_back(item);
                }
            }
            *vec = new_vec;
        }
    }

    /// Emit warning event when iteration limits are approached
    pub fn emit_iteration_warning(env: &env::Env, operation: &str, actual_count: u32, limit: u32) {
        env.events().publish(
            soroban_sdk::symbol!("IterationWarning"),
            (operation, actual_count, limit),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn test_bounded_iterator() {
        let env = Env::default();
        let mut vec = Vec::new(&env);
        for i in 0..150 {
            vec.push_back(i);
        }

        let mut iterator = BoundedIterator::new(&vec, 100).unwrap();
        let mut count = 0;
        
        while let Some(_) = iterator.next() {
            count += 1;
        }
        
        assert_eq!(count, 100);
        assert_eq!(iterator.count(), 100);
    }

    #[test]
    fn test_iteration_controller_validation() {
        // Test user leases limit
        assert!(IterationController::validate_user_leases_iteration(50).is_ok());
        assert!(IterationController::validate_user_leases_iteration(150).is_err());

        // Test active leases limit
        assert!(IterationController::validate_active_leases_iteration(100).is_ok());
        assert!(IterationController::validate_active_leases_iteration(250).is_err());

        // Test batch operations limit
        assert!(IterationController::validate_batch_operations(25).is_ok());
        assert!(IterationController::validate_batch_operations(75).is_err());
    }

    #[test]
    fn test_iteration_utils() {
        assert!(IterationUtils::is_safe_collection_size(50, 100));
        assert!(!IterationUtils::is_safe_collection_size(150, 100));

        assert_eq!(IterationUtils::get_safe_iteration_limit(50, 100), 50);
        assert_eq!(IterationUtils::get_safe_iteration_limit(150, 100), 100);
    }
}
