//! Tests for precision-safe wear and tear deduction calculations (Issue #187)
//!
//! This test module verifies that the precision-loss mitigations work correctly
//! for prorated wear and tear deductions in the LeaseFlow Protocol.

#[cfg(test)]
mod tests {
    use soroban_sdk::{Env, Address};
    use crate::{
        LeaseContract, LeaseError, LeaseInstance, LeaseStatus, DepositStatus,
        CreateLeaseParams, DamageSeverity, wear_tear_precision::{WearTearCalculator, WearTearError},
    };

    #[test]
    fn test_precision_safe_prorated_deduction() {
        let mut calc = WearTearCalculator::new();
        
        // Test exact division (no precision loss)
        let result = calc.calculate_prorated_deduction_ceiling(1000, 50, 100).unwrap();
        assert_eq!(result, 500);
        
        // Test ceiling division (rounds up for landlord benefit)
        let result = calc.calculate_prorated_deduction_ceiling(1000, 1, 3).unwrap();
        assert_eq!(result, 334); // ceil(1000/3) = 334
        
        // Test floor division (rounds down for tenant benefit)
        let result = calc.calculate_prorated_deduction_floor(1000, 1, 3).unwrap();
        assert_eq!(result, 333); // floor(1000/3) = 333
    }

    #[test]
    fn test_precision_safe_percentage_deduction() {
        let mut calc = WearTearCalculator::new();
        
        // Test exact percentage
        let result = calc.calculate_percentage_deduction_ceiling(1000, 5000).unwrap();
        assert_eq!(result, 500); // 50%
        
        // Test with remainder (ceiling)
        let result = calc.calculate_percentage_deduction_ceiling(1000, 3333).unwrap();
        assert_eq!(result, 334); // ceil(1000 * 0.3333) = 334
        
        // Test with remainder (floor)
        let result = calc.calculate_percentage_deduction_floor(1000, 3333).unwrap();
        assert_eq!(result, 333); // floor(1000 * 0.3333) = 333
    }

    #[test]
    fn test_wear_tear_deduction_full_lease() {
        let mut calc = WearTearCalculator::new();
        
        // Full lease period - no proration
        let result = calc
            .calculate_wear_tear_deduction(1000, 25, 365, 365, true)
            .unwrap();
        assert_eq!(result, 250); // 25% of 1000
    }

    #[test]
    fn test_wear_tear_deduction_half_lease() {
        let mut calc = WearTearCalculator::new();
        
        // Half lease period with ceiling division
        let result = calc
            .calculate_wear_tear_deduction(1000, 25, 182, 365, true)
            .unwrap();
        // 25% of 1000 = 250, prorated for 182/365 ≈ 125
        // With ceiling: ceil(250 * 182 / 365) = ceil(124.66) = 125
        assert!(result >= 124 && result <= 126);
    }

    #[test]
    fn test_wear_tear_deduction_quarter_lease() {
        let mut calc = WearTearCalculator::new();
        
        // Quarter lease period
        let result = calc
            .calculate_wear_tear_deduction(1000, 50, 91, 365, true)
            .unwrap();
        // 50% of 1000 = 500, prorated for 91/365 ≈ 125
        // With ceiling: ceil(500 * 91 / 365) = ceil(124.66) = 125
        assert!(result >= 124 && result <= 126);
    }

    #[test]
    fn test_dust_tracking() {
        let mut calc = WearTearCalculator::new();
        
        // Perform operations that create dust
        let _ = calc.calculate_prorated_deduction_ceiling(1000, 1, 3);
        let _ = calc.calculate_prorated_deduction_ceiling(1000, 1, 7);
        let _ = calc.calculate_percentage_deduction_ceiling(1000, 3333);
        
        let report = calc.get_precision_report();
        assert_eq!(report.operation_count, 3);
        assert!(report.cumulative_dust > 0);
    }

    #[test]
    fn test_precision_error_conditions() {
        let mut calc = WearTearCalculator::new();
        
        // Negative amount
        assert!(matches!(
            calc.calculate_prorated_deduction_ceiling(-100, 50, 100),
            Err(WearTearError::NegativeAmount(-100))
        ));
        
        // Division by zero
        assert!(matches!(
            calc.calculate_prorated_deduction_ceiling(100, 50, 0),
            Err(WearTearError::DivisionByZero)
        ));
        
        // Invalid time range
        assert!(matches!(
            calc.calculate_prorated_deduction_ceiling(100, 200, 100),
            Err(WearTearError::InvalidTimeRange { elapsed: 200, total: 100 })
        ));
        
        // Invalid BPS
        assert!(matches!(
            calc.calculate_percentage_deduction_ceiling(100, 10001),
            Err(WearTearError::InvalidBps(10001))
        ));
        
        // Invalid percentage
        assert!(matches!(
            calc.calculate_wear_tear_deduction(100, 101, 50, 100, true),
            Err(WearTearError::InvalidPercentage(101))
        ));
    }

    #[test]
    fn test_precision_report() {
        let mut calc = WearTearCalculator::new();
        
        let _ = calc.calculate_prorated_deduction_ceiling(1000, 1, 3);
        let _ = calc.calculate_prorated_deduction_ceiling(1000, 1, 3);
        let _ = calc.calculate_prorated_deduction_ceiling(1000, 1, 3);
        
        let report = calc.get_precision_report();
        assert_eq!(report.operation_count, 3);
        assert!(report.cumulative_dust > 0);
        assert!(report.average_dust_per_operation > 0);
    }

    #[test]
    fn test_calculator_reset() {
        let mut calc = WearTearCalculator::new();
        
        let _ = calc.calculate_prorated_deduction_ceiling(1000, 1, 3);
        let report = calc.get_precision_report();
        assert!(report.operation_count > 0);
        
        calc.reset();
        let report = calc.get_precision_report();
        assert_eq!(report.operation_count, 0);
        assert_eq!(report.cumulative_dust, 0);
    }

    #[test]
    fn test_edge_cases() {
        let mut calc = WearTearCalculator::new();
        
        // Zero amount
        let result = calc.calculate_prorated_deduction_ceiling(0, 50, 100).unwrap();
        assert_eq!(result, 0);
        
        // Zero elapsed time
        let result = calc.calculate_prorated_deduction_ceiling(1000, 0, 100).unwrap();
        assert_eq!(result, 0);
        
        // Full elapsed time
        let result = calc.calculate_prorated_deduction_ceiling(1000, 100, 100).unwrap();
        assert_eq!(result, 1000);
        
        // Zero percentage
        let result = calc.calculate_percentage_deduction_ceiling(1000, 0).unwrap();
        assert_eq!(result, 0);
        
        // Full percentage (100%)
        let result = calc.calculate_percentage_deduction_ceiling(1000, 10000).unwrap();
        assert_eq!(result, 1000);
    }

    #[test]
    fn test_large_values() {
        let mut calc = WearTearCalculator::new();
        
        // Large amount with small percentage
        let large_amount = 1_000_000_000_000i128; // 1 trillion
        let result = calc.calculate_percentage_deduction_ceiling(large_amount, 1).unwrap();
        assert_eq!(result, 100_000_000); // 0.01% of 1 trillion
        
        // Large time values
        let result = calc.calculate_prorated_deduction_ceiling(
            1_000_000_000,
            31_536_000, // 1 year in seconds
            31_536_000, // 1 year in seconds
        ).unwrap();
        assert_eq!(result, 1_000_000_000);
    }

    #[test]
    fn test_normal_wear_and_tear_zero_deduction() {
        let mut calc = WearTearCalculator::new();
        
        // NormalWearAndTear should result in 0% deduction
        let result = calc
            .calculate_wear_tear_deduction(1000, 0, 365, 365, true)
            .unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_catastrophic_damage_full_deduction() {
        let mut calc = WearTearCalculator::new();
        
        // Catastrophic damage should result in 100% deduction
        let result = calc
            .calculate_wear_tear_deduction(1000, 100, 365, 365, true)
            .unwrap();
        assert_eq!(result, 1000);
    }

    #[test]
    fn test_prorated_catastrophic_damage() {
        let mut calc = WearTearCalculator::new();
        
        // Catastrophic damage with half lease period
        let result = calc
            .calculate_wear_tear_deduction(1000, 100, 182, 365, true)
            .unwrap();
        // 100% of 1000 = 1000, prorated for 182/365 ≈ 499
        assert!(result >= 498 && result <= 500);
    }

    #[test]
    fn test_landlord_vs_tenant_favorable() {
        let mut calc_ceil = WearTearCalculator::new();
        let mut calc_floor = WearTearCalculator::new();
        
        // Same calculation with different rounding methods
        let ceiling_result = calc_ceil
            .calculate_wear_tear_deduction(1000, 25, 1, 3, true)
            .unwrap();
        let floor_result = calc_floor
            .calculate_wear_tear_deduction(1000, 25, 1, 3, false)
            .unwrap();
        
        // Ceiling should be >= floor
        assert!(ceiling_result >= floor_result);
        
        // For non-exact division, ceiling should be strictly greater
        if ceiling_result != floor_result {
            assert_eq!(ceiling_result - floor_result, 1);
        }
    }
}
