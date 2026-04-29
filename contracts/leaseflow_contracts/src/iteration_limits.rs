//! Bounded Iteration Limits Module for LeaseFlow Contracts
//! 
//! Provides safe iteration limits for on-chain lease history and array read operations
//! to prevent gas exhaustion and denial of service attacks.

use soroban_sdk::{contracterror, contracttype, env, Address, Vec, u64};

/// Maximum iteration limits for different operations in leaseflow_contracts
pub const MAX_ACTIVE_LEASES_ITERATIONS: u32 = 200;
pub const MAX_BATCH_OPERATIONS: u32 = 50;
pub const MAX_VELOCITY_TRACKING_ENTRIES: u32 = 100;
pub const MAX_APPROVAL_ENTRIES: u32 = 10;

/// Iteration limit configuration
#[derive(Clone, Debug, contracttype)]
pub struct IterationLimits {
    pub max_active_leases: u32,
    pub max_batch_operations: u32,
    pub max_velocity_tracking: u32,
    pub max_approval_entries: u32,
}

impl Default for IterationLimits {
    fn default() -> Self {
        Self {
            max_active_leases: MAX_ACTIVE_LEASES_ITERATIONS,
            max_batch_operations: MAX_BATCH_OPERATIONS,
            max_velocity_tracking: MAX_VELOCITY_TRACKING_ENTRIES,
            max_approval_entries: MAX_APPROVAL_ENTRIES,
        }
    }
}

/// Error types for iteration limit violations
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum IterationLimitError {
    ActiveLeasesLimitExceeded = 2001,
    BatchOperationsLimitExceeded = 2002,
    VelocityTrackingLimitExceeded = 2003,
    ApprovalEntriesLimitExceeded = 2004,
    InvalidIterationLimit = 2005,
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
    /// Get current iteration limits
    pub fn get_limits() -> IterationLimits {
        IterationLimits::default()
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

    /// Validate velocity tracking limit
    pub fn validate_velocity_tracking(entries_count: u32) -> Result<(), IterationLimitError> {
        let limits = Self::get_limits();
        if entries_count > limits.max_velocity_tracking {
            Err(IterationLimitError::VelocityTrackingLimitExceeded)
        } else {
            Ok(())
        }
    }

    /// Validate approval entries limit
    pub fn validate_approval_entries(entries_count: u32) -> Result<(), IterationLimitError> {
        let limits = Self::get_limits();
        if entries_count > limits.max_approval_entries {
            Err(IterationLimitError::ApprovalEntriesLimitExceeded)
        } else {
            Ok(())
        }
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

    /// Create a bounded iterator for velocity tracking
    pub fn create_bounded_velocity_iterator<T>(
        items: &Vec<T>,
    ) -> Result<BoundedIterator<T>, IterationLimitError> {
        let limits = Self::get_limits();
        BoundedIterator::new(items, limits.max_velocity_tracking)
    }

    /// Create a bounded iterator for approval entries
    pub fn create_bounded_approval_iterator<T>(
        items: &Vec<T>,
    ) -> Result<BoundedIterator<T>, IterationLimitError> {
        let limits = Self::get_limits();
        BoundedIterator::new(items, limits.max_approval_entries)
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

    /// Emit warning event when iteration limits are approached
    pub fn emit_iteration_warning(env: &env::Env, operation: &str, actual_count: u32, limit: u32) {
        env.events().publish(
            soroban_sdk::symbol!("IterationWarning"),
            (operation, actual_count, limit),
        );
    }

    /// Safe bounded search in a vector
    pub fn bounded_contains<T: PartialEq>(vec: &Vec<T>, item: &T, max_search: u32) -> bool {
        let mut search_count = 0u32;
        
        for element in vec.iter() {
            if search_count >= max_search {
                break;
            }
            
            if element == item {
                return true;
            }
            search_count += 1;
        }
        
        false
    }

    /// Safe bounded find operation in a vector
    pub fn bounded_find<T: PartialEq>(vec: &Vec<T>, item: &T, max_search: u32) -> Option<u32> {
        let mut search_count = 0u32;
        
        for (index, element) in vec.iter().enumerate() {
            if search_count >= max_search {
                break;
            }
            
            if element == item {
                return Some(index as u32);
            }
            search_count += 1;
        }
        
        None
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
        for i in 0..250 {
            vec.push_back(i);
        }

        let mut iterator = BoundedIterator::new(&vec, 200).unwrap();
        let mut count = 0;
        
        while let Some(_) = iterator.next() {
            count += 1;
        }
        
        assert_eq!(count, 200);
        assert_eq!(iterator.count(), 200);
    }

    #[test]
    fn test_iteration_controller_validation() {
        // Test active leases limit
        assert!(IterationController::validate_active_leases_iteration(150).is_ok());
        assert!(IterationController::validate_active_leases_iteration(250).is_err());

        // Test batch operations limit
        assert!(IterationController::validate_batch_operations(25).is_ok());
        assert!(IterationController::validate_batch_operations(75).is_err());

        // Test velocity tracking limit
        assert!(IterationController::validate_velocity_tracking(50).is_ok());
        assert!(IterationController::validate_velocity_tracking(150).is_err());
    }

    #[test]
    fn test_iteration_utils() {
        let env = Env::default();
        let mut vec = Vec::new(&env);
        for i in 0..10 {
            vec.push_back(i);
        }

        assert!(IterationUtils::bounded_contains(&vec, &5, 20));
        assert!(!IterationUtils::bounded_contains(&vec, &15, 20));
        
        assert_eq!(IterationUtils::bounded_find(&vec, &5, 20), Some(5));
        assert_eq!(IterationUtils::bounded_find(&vec, &15, 20), None);
    }
}
