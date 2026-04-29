//! Comprehensive Tests for Optimized 128-bit Safe Math
//!
//! This module provides extensive testing for the safe_math_128 module,
//! covering edge cases, overflow scenarios, precision tracking, and
//! performance benchmarks for yield generation scenarios.

use soroban_sdk::{i128, u128, u64, Env};
use crate::safe_math_128::{SafeMath128, optimized_ops, SafeMathError, YieldDistribution};

#[cfg(test)]
mod safe_math_tests {
    use super::*;

    fn create_env() -> Env {
        Env::default()
    }

    #[test]
    fn test_safe_mul_yield_basic_operations() {
        let mut math = SafeMath128::new();
        
        // Test basic multiplication with reasonable values
        let result = math.safe_mul_yield(1000, 5000, 86400).unwrap();
        assert_eq!(result, 432000000); // 1000 * 0.5 * 86400
        
        // Test zero values
        assert_eq!(math.safe_mul_yield(0, 5000, 86400).unwrap(), 0);
        assert_eq!(math.safe_mul_yield(1000, 0, 86400).unwrap(), 0);
        assert_eq!(math.safe_mul_yield(1000, 5000, 0).unwrap(), 0);
        
        // Test unit rate (100%)
        let result = math.safe_mul_yield(1000, 10000, 1).unwrap();
        assert_eq!(result, 1000);
    }

    #[test]
    fn test_safe_mul_yield_overflow_protection() {
        let mut math = SafeMath128::new();
        
        // Test with values that would cause overflow
        let large_principal = i128::MAX / 1000;
        let result = math.safe_mul_yield(large_principal, 5000, 86400);
        assert!(result.is_ok()); // Should be safe
        
        // Test with values that exceed maximum safe amount
        let excessive_principal = crate::safe_math_128::MAX_SAFE_YIELD_AMOUNT + 1;
        let result = math.safe_mul_yield(excessive_principal, 5000, 86400);
        assert!(matches!(result, Err(SafeMathError::PrincipalTooLarge(_))));
    }

    #[test]
    fn test_safe_add_yield_operations() {
        let mut math = SafeMath128::new();
        
        // Test basic addition
        let result = math.safe_add_yield(1000, 500).unwrap();
        assert_eq!(result, 1500);
        
        // Test adding zero
        assert_eq!(math.safe_add_yield(1000, 0).unwrap(), 1000);
        assert_eq!(math.safe_add_yield(0, 1000).unwrap(), 1000);
        
        // Test overflow protection
        let large_amount = crate::safe_math_128::MAX_SAFE_YIELD_AMOUNT;
        let result = math.safe_add_yield(large_amount, 1);
        assert!(matches!(result, Err(SafeMathError::YieldExceedsMaximum(_))));
    }

    #[test]
    fn test_safe_sub_yield_operations() {
        let mut math = SafeMath128::new();
        
        // Test basic subtraction
        let result = math.safe_sub_yield(1000, 400).unwrap();
        assert_eq!(result, 600);
        
        // Test subtracting zero
        assert_eq!(math.safe_sub_yield(1000, 0).unwrap(), 1000);
        
        // Test underflow protection
        let result = math.safe_sub_yield(1000, 1500);
        assert!(matches!(result, Err(SafeMathError::DistributionExceedsYield { .. })));
    }

    #[test]
    fn test_bps_division_precision() {
        let mut math = SafeMath128::new();
        
        // Test exact division
        let result = math.safe_bps_division_floor(1000, 5000).unwrap();
        assert_eq!(result, 500); // 50%
        
        // Test ceiling division
        let result = math.safe_bps_division_ceiling(1000, 5000).unwrap();
        assert_eq!(result, 500); // 50% (exact)
        
        // Test with remainder - floor should round down
        let result = math.safe_bps_division_floor(1000, 3333).unwrap();
        assert_eq!(result, 333); // floor(1000 * 0.3333)
        
        // Test with remainder - ceiling should round up
        let result = math.safe_bps_division_ceiling(1000, 3333).unwrap();
        assert_eq!(result, 334); // ceil(1000 * 0.3333)
    }

    #[test]
    fn test_yield_distribution_with_dust() {
        let mut math = SafeMath128::new();
        
        // Test distribution that creates dust
        let distribution = math.yield_distribution_with_dust_tracking(
            999, 3333, 3333, 3334
        ).unwrap();
        
        assert!(distribution.total_distributed <= 999);
        assert!(distribution.dust > 0);
        assert_eq!(distribution.lessee_share + distribution.lessor_share + distribution.dao_share + distribution.dust, 999);
    }

    #[test]
    fn test_complex_yield_calculation() {
        let mut math = SafeMath128::new();
        
        // Test complex calculation with multiple factors
        let result = math.complex_yield_calculation(
            1000,    // principal
            5000,    // 50% rate
            86400,   // 1 day time factor
            11000,   // 110% multiplier
        ).unwrap();
        
        assert!(result > 0);
        assert!(result < 1000 * 1000); // Should be reasonable
        
        // Test with 100% multiplier (should equal base yield)
        let base_yield = math.safe_mul_yield(1000, 5000, 86400).unwrap();
        let same_yield = math.complex_yield_calculation(1000, 5000, 86400, 10000).unwrap();
        assert_eq!(base_yield, same_yield);
    }

    #[test]
    fn test_precision_tracking() {
        let mut math = SafeMath128::new();
        
        // Perform operations that create precision loss
        let _ = math.safe_bps_division_floor(1000, 3333);
        let _ = math.safe_bps_division_floor(2000, 3333);
        let _ = math.safe_bps_division_ceiling(3000, 3333);
        
        let report = math.get_precision_report();
        assert_eq!(report.operation_count, 3);
        assert!(report.cumulative_precision_loss > 0);
        assert!(report.max_single_operation_loss > 0);
        assert!(report.efficiency_score > 0 && report.efficiency_score <= 100);
    }

    #[test]
    fn test_optimized_operations() {
        // Test optimized multiplication
        assert_eq!(
            optimized_ops::mul_yield_optimized(1000, 5000, 86400),
            Some(432000000)
        );
        
        // Test edge cases that should return 0
        assert_eq!(optimized_ops::mul_yield_optimized(0, 5000, 86400), Some(0));
        assert_eq!(optimized_ops::mul_yield_optimized(1000, 0, 86400), Some(0));
        assert_eq!(optimized_ops::mul_yield_optimized(1000, 5000, 0), Some(0));
        
        // Test fast path optimizations
        assert_eq!(optimized_ops::bps_floor_optimized(1000, 10000), Some(1000));
        assert_eq!(optimized_ops::bps_ceiling_optimized(1000, 10000), Some(1000));
        
        // Test BPS with remainder
        assert_eq!(optimized_ops::bps_floor_optimized(1000, 3333), Some(333));
        assert_eq!(optimized_ops::bps_ceiling_optimized(1000, 3333), Some(334));
    }

    #[test]
    fn test_overflow_detection() {
        // Test multiplication overflow detection
        assert!(optimized_ops::will_multiply_overflow(i128::MAX, 2));
        assert!(!optimized_ops::will_multiply_overflow(1000, 1000));
        assert!(!optimized_ops::will_multiply_overflow(0, i128::MAX));
        
        // Test addition overflow detection
        assert!(optimized_ops::will_add_overflow(i128::MAX, 1));
        assert!(!optimized_ops::will_add_overflow(1000, 1000));
        assert!(!optimized_ops::will_add_overflow(0, i128::MAX));
    }

    #[test]
    fn test_error_conditions() {
        let mut math = SafeMath128::new();
        
        // Test negative inputs
        assert!(matches!(
            math.safe_mul_yield(-100, 5000, 86400),
            Err(SafeMathError::NegativePrincipal(_))
        ));
        
        assert!(matches!(
            math.safe_add_yield(-100, 500),
            Err(SafeMathError::NegativeYield { .. })
        ));
        
        assert!(matches!(
            math.safe_bps_division_floor(-100, 5000),
            Err(SafeMathError::NegativeAmount(_))
        ));
        
        // Test invalid rates
        assert!(matches!(
            math.safe_mul_yield(100, 10001, 86400),
            Err(SafeMathError::InvalidRate(_))
        ));
        
        assert!(matches!(
            math.safe_bps_division_floor(100, 10001),
            Err(SafeMathError::InvalidBps(_))
        ));
        
        // Test distribution errors
        assert!(matches!(
            math.yield_distribution_with_dust_tracking(100, 6000, 6000, 6000),
            Err(SafeMathError::InvalidDistributionBps(_))
        ));
    }

    #[test]
    fn test_extreme_values() {
        let mut math = SafeMath128::new();
        
        // Test with maximum safe values
        let max_safe = crate::safe_math_128::MAX_SAFE_YIELD_AMOUNT;
        let result = math.safe_mul_yield(max_safe, 1, 1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), max_safe / 10000);
        
        // Test with minimum non-zero values
        let result = math.safe_mul_yield(1, 1, 1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0); // 1 * 0.0001 * 1 = 0 (integer division)
        
        // Test BPS with edge cases
        assert_eq!(math.safe_bps_division_floor(1, 1).unwrap(), 0);
        assert_eq!(math.safe_bps_division_ceiling(1, 1).unwrap(), 1);
    }

    #[test]
    fn test_yield_distribution_edge_cases() {
        let mut math = SafeMath128::new();
        
        // Test with zero total yield
        let distribution = math.yield_distribution_with_dust_tracking(0, 5000, 3000, 2000).unwrap();
        assert_eq!(distribution.lessee_share, 0);
        assert_eq!(distribution.lessor_share, 0);
        assert_eq!(distribution.dao_share, 0);
        assert_eq!(distribution.dust, 0);
        
        // Test with very small amounts
        let distribution = math.yield_distribution_with_dust_tracking(1, 5000, 3000, 2000).unwrap();
        assert!(distribution.total_distributed <= 1);
        assert!(distribution.dust >= 0);
        
        // Test with maximum distribution BPS
        let distribution = math.yield_distribution_with_dust_tracking(1000, 10000, 0, 0).unwrap();
        assert_eq!(distribution.lessee_share, 1000);
        assert_eq!(distribution.lessor_share, 0);
        assert_eq!(distribution.dao_share, 0);
    }

    #[test]
    fn test_complex_yield_edge_cases() {
        let mut math = SafeMath128::new();
        
        // Test with zero time factor
        let result = math.complex_yield_calculation(1000, 5000, 0, 10000);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
        
        // Test with zero multiplier
        let result = math.complex_yield_calculation(1000, 5000, 86400, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
        
        // Test with maximum multiplier
        let result = math.complex_yield_calculation(1, 1, 1, crate::safe_math_128::MAX_SAFE_YIELD_AMOUNT as u32);
        assert!(result.is_ok());
    }

    #[test]
    fn test_precision_report_accuracy() {
        let mut math = SafeMath128::new();
        
        // Perform known operations with predictable precision loss
        let _ = math.safe_bps_division_floor(10000, 3333); // Should lose 3334 remainder
        let _ = math.safe_bps_division_floor(20000, 3333); // Should lose 6668 remainder
        
        let report = math.get_precision_report();
        assert_eq!(report.operation_count, 2);
        assert!(report.cumulative_precision_loss > 0);
        assert!(report.average_loss_per_operation > 0);
        
        // Test efficiency score calculation
        assert!(report.efficiency_score > 0 && report.efficiency_score <= 100);
    }

    #[test]
    fn test_reset_functionality() {
        let mut math = SafeMath128::new();
        
        // Perform some operations
        let _ = math.safe_mul_yield(1000, 5000, 86400);
        let _ = math.safe_add_yield(1000, 500);
        
        // Verify state changed
        let report_before = math.get_precision_report();
        assert!(report_before.operation_count > 0);
        
        // Reset and verify state cleared
        math.reset();
        let report_after = math.get_precision_report();
        assert_eq!(report_after.operation_count, 0);
        assert_eq!(report_after.cumulative_precision_loss, 0);
        assert_eq!(report_after.max_single_operation_loss, 0);
        assert_eq!(report_after.efficiency_score, 100);
    }

    #[test]
    fn test_yield_distribution_consistency() {
        let mut math = SafeMath128::new();
        
        // Test that distribution is consistent across multiple runs
        let total_yield = 123456789;
        
        let dist1 = math.yield_distribution_with_dust_tracking(total_yield, 5000, 3000, 2000).unwrap();
        math.reset();
        
        let dist2 = math.yield_distribution_with_dust_tracking(total_yield, 5000, 3000, 2000).unwrap();
        
        assert_eq!(dist1.lessee_share, dist2.lessee_share);
        assert_eq!(dist1.lessor_share, dist2.lessor_share);
        assert_eq!(dist1.dao_share, dist2.dao_share);
        assert_eq!(dist1.dust, dist2.dust);
        assert_eq!(dist1.total_distributed, dist2.total_distributed);
    }

    #[test]
    fn test_mathematical_properties() {
        let mut math = SafeMath128::new();
        
        // Test commutative property of multiplication factors
        let result1 = math.safe_mul_yield(1000, 5000, 86400).unwrap();
        math.reset();
        let result2 = math.safe_mul_yield(1000, 86400, 5000).unwrap(); // Note: this treats 86400 as rate
        
        // These should be different due to BPS scaling, but both should be valid
        assert!(result1 > 0);
        assert!(result2 > 0);
        
        // Test associative property of addition
        let sum1 = math.safe_add_yield(1000, 500).unwrap();
        let sum2 = math.safe_add_yield(sum1, 300).unwrap();
        math.reset();
        
        let sum3 = math.safe_add_yield(500, 300).unwrap();
        let sum4 = math.safe_add_yield(1000, sum3).unwrap();
        
        assert_eq!(sum2, sum4);
    }

    #[test]
    fn test_gas_optimization_scenarios() {
        let mut math = SafeMath128::new();
        
        // Test scenario: many small operations vs one large operation
        let base_amount = 1000;
        
        // Many small operations
        for i in 0..100 {
            let _ = math.safe_add_yield(base_amount, i);
        }
        
        let report_many = math.get_precision_report();
        math.reset();
        
        // One large operation (equivalent total)
        let total_small_ops = (0..100).sum::<i128>();
        let _ = math.safe_add_yield(base_amount * 100, total_small_ops);
        
        let report_one = math.get_precision_report();
        
        // Both should produce valid results, but with different efficiency characteristics
        assert!(report_many.operation_count == 100);
        assert!(report_one.operation_count == 1);
        assert!(report_many.efficiency_score > 0);
        assert!(report_one.efficiency_score > 0);
    }

    #[test]
    fn test_real_world_yield_scenarios() {
        let mut math = SafeMath128::new();
        
        // Scenario 1: Daily yield calculation for a 1-year deposit
        let principal = 1000000; // 1M tokens
        let daily_rate_bps = 137; // ~5% annual rate
        let seconds_in_day = 86400;
        
        let daily_yield = math.safe_mul_yield(principal, daily_rate_bps, seconds_in_day).unwrap();
        assert!(daily_yield > 0 && daily_yield < principal); // Should be reasonable
        
        // Scenario 2: Monthly yield distribution
        let monthly_yield = daily_yield * 30; // Approximate month
        let distribution = math.yield_distribution_with_dust_tracking(monthly_yield, 5000, 3000, 2000).unwrap();
        
        assert!(distribution.lessee_share > 0);
        assert!(distribution.lessor_share > 0);
        assert!(distribution.dao_share > 0);
        assert_eq!(distribution.lessee_share + distribution.lessor_share + distribution.dao_share + distribution.dust, monthly_yield);
        
        // Scenario 3: Compounded yield over time
        let mut accumulated_yield = 0;
        for _day in 0..30 {
            let day_yield = math.safe_mul_yield(principal, daily_rate_bps, seconds_in_day).unwrap();
            accumulated_yield = math.safe_add_yield(accumulated_yield, day_yield).unwrap();
        }
        
        assert!(accumulated_yield > daily_yield); // Should grow over time
    }

    #[test]
    fn test_boundary_conditions() {
        let mut math = SafeMath128::new();
        
        // Test at the boundaries of safe ranges
        let max_principal = crate::safe_math_128::MAX_SAFE_YIELD_AMOUNT;
        let min_rate = 1;
        let max_rate = 10000;
        let min_time = 1;
        let max_time = u64::MAX / 10000; // Prevent overflow in calculation
        
        // Test maximum principal with minimum rate and time
        let result = math.safe_mul_yield(max_principal, min_rate, min_time);
        assert!(result.is_ok());
        assert!(result.unwrap() > 0);
        
        // Test minimum principal with maximum rate and reasonable time
        let result = math.safe_mul_yield(1, max_rate, 86400);
        assert!(result.is_ok());
        assert!(result.unwrap() > 0);
        
        // Test edge case where result should be exactly principal
        let result = math.safe_mul_yield(1000, 10000, 1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1000);
    }

    #[test]
    fn test_batch_safe_mul_yield() {
        let mut math = SafeMath128::new();
        
        let inputs = vec![
            (1000, 5000, 86400),
            (2000, 3000, 43200),
            (500, 8000, 172800),
        ];
        
        let results = math.batch_safe_mul_yield(&inputs).unwrap();
        assert_eq!(results.len(), 3);
        
        // Verify each result matches individual calculation
        for (i, &(principal, rate, time)) in inputs.iter().enumerate() {
            let expected = math.safe_mul_yield(principal, rate, time).unwrap();
            assert_eq!(results[i], expected);
        }
    }

    #[test]
    fn test_safe_power_operations() {
        let mut math = SafeMath128::new();
        
        // Test basic power operations
        assert_eq!(math.safe_power(2, 0).unwrap(), 1);
        assert_eq!(math.safe_power(2, 1).unwrap(), 2);
        assert_eq!(math.safe_power(2, 10).unwrap(), 1024);
        assert_eq!(math.safe_power(3, 3).unwrap(), 27);
        
        // Test edge cases
        assert_eq!(math.safe_power(0, 5).unwrap(), 0);
        assert_eq!(math.safe_power(1, 100).unwrap(), 1);
        
        // Test overflow protection
        let result = math.safe_power(i128::MAX / 2, 2);
        assert!(result.is_ok());
        
        let result = math.safe_power(i128::MAX, 2);
        assert!(matches!(result, Err(SafeMathError::MultiplicationOverflow)));
    }

    #[test]
    fn test_safe_compound_interest() {
        let mut math = SafeMath128::new();
        
        // Test basic compound interest
        let result = math.safe_compound_interest(1000, 1000, 12, 1).unwrap(); // 10% annual, monthly compounding
        assert!(result > 0);
        assert!(result < 1000); // Should be reasonable for one year
        
        // Test with zero periods or years
        assert_eq!(math.safe_compound_interest(1000, 1000, 0, 1).unwrap(), 0);
        assert_eq!(math.safe_compound_interest(1000, 1000, 12, 0).unwrap(), 0);
        
        // Test edge cases
        assert_eq!(math.safe_compound_interest(1000, 0, 12, 1).unwrap(), 0); // 0% rate
        assert!(math.safe_compound_interest(0, 1000, 12, 1).unwrap() == 0); // 0 principal
        
        // Test overflow protection
        let result = math.safe_compound_interest(i128::MAX / 1000, 10000, 365, 10);
        assert!(result.is_ok()); // Should be safe
    }

    #[test]
    fn test_safe_ln_approximation() {
        let mut math = SafeMath128::new();
        
        // Test basic logarithm approximation
        assert_eq!(math.safe_ln_approximation(1).unwrap(), 0);
        
        let result = math.safe_ln_approximation(1000).unwrap();
        assert!(result > 0); // ln(1000) should be positive
        
        // Test edge cases
        assert!(matches!(
            math.safe_ln_approximation(0),
            Err(SafeMathError::InvalidLogInput(_))
        ));
        assert!(matches!(
            math.safe_ln_approximation(-1),
            Err(SafeMathError::InvalidLogInput(_))
        ));
        
        // Test values close to 1 (where approximation works best)
        let result = math.safe_ln_approximation(1100).unwrap(); // ~1.1
        assert!(result > 0 && result < 1000); // Should be reasonable
    }

    #[test]
    fn test_safe_sqrt_approximation() {
        let mut math = SafeMath128::new();
        
        // Test basic square root
        assert_eq!(math.safe_sqrt_approximation(0, 10).unwrap(), 0);
        assert_eq!(math.safe_sqrt_approximation(1, 10).unwrap(), 1);
        
        let result = math.safe_sqrt_approximation(100, 10).unwrap();
        assert!(result >= 9 && result <= 11); // Should be close to 10
        
        let result = math.safe_sqrt_approximation(10000, 10).unwrap();
        assert!(result >= 99 && result <= 101); // Should be close to 100
        
        // Test edge cases
        assert!(matches!(
            math.safe_sqrt_approximation(-1, 10),
            Err(SafeMathError::NegativeAmount(_))
        ));
    }

    #[test]
    fn test_safe_percentage_change() {
        let mut math = SafeMath128::new();
        
        // Test basic percentage change
        let result = math.safe_percentage_change(100, 150).unwrap(); // 50% increase
        assert_eq!(result, 5000); // 50% in BPS
        
        let result = math.safe_percentage_change(100, 50).unwrap(); // 50% decrease
        assert_eq!(result, -5000); // -50% in BPS
        
        // Test no change
        assert_eq!(math.safe_percentage_change(100, 100).unwrap(), 0);
        
        // Test edge cases
        assert!(matches!(
            math.safe_percentage_change(0, 100),
            Err(SafeMathError::DivisionByZero)
        ));
        assert!(matches!(
            math.safe_percentage_change(-1, 100),
            Err(SafeMathError::DivisionByZero)
        ));
    }

    #[test]
    fn test_safe_weighted_average() {
        let mut math = SafeMath128::new();
        
        // Test basic weighted average
        let values = vec![(100, 2), (200, 3), (300, 5)]; // Weighted average should be 233.33
        let result = math.safe_weighted_average(&values).unwrap();
        assert!(result >= 233 && result <= 234); // Should be close to 233.33
        
        // Test single value
        let values = vec![(100, 1)];
        assert_eq!(math.safe_weighted_average(&values).unwrap(), 100);
        
        // Test edge cases
        assert!(matches!(
            math.safe_weighted_average(&[]),
            Err(SafeMathError::EmptyInput)
        ));
        
        assert!(matches!(
            math.safe_weighted_average(&[(100, -1)]),
            Err(SafeMathError::NegativeWeight(_))
        ));
        
        // Test zero weights
        let values = vec![(100, 0), (200, 0)];
        assert!(matches!(
            math.safe_weighted_average(&values),
            Err(SafeMathError::DivisionByZero)
        ));
    }

    #[test]
    fn test_safe_ema() {
        let mut math = SafeMath128::new();
        
        // Test basic EMA calculation
        let result = math.safe_ema(100, 80, 2000).unwrap(); // 20% alpha
        assert!(result > 80 && result < 100); // Should be between old and new values
        
        // Test edge cases
        assert_eq!(math.safe_ema(100, 100, 5000).unwrap(), 100); // Same values
        assert_eq!(math.safe_ema(100, 80, 0).unwrap(), 80); // 0% alpha = old value
        assert_eq!(math.safe_ema(100, 80, 10000).unwrap(), 100); // 100% alpha = new value
        
        // Test invalid alpha
        assert!(matches!(
            math.safe_ema(100, 80, 10001),
            Err(SafeMathError::InvalidBps(_))
        ));
    }

    #[test]
    fn test_safe_volatility_calculation() {
        let mut math = SafeMath128::new();
        
        // Test basic volatility calculation
        let values = vec![100, 110, 90, 120, 80]; // Some variance
        let result = math.safe_volatility_calculation(&values).unwrap();
        assert!(result > 0); // Should have some volatility
        
        // Test constant values (no volatility)
        let values = vec![100, 100, 100, 100, 100];
        assert_eq!(math.safe_volatility_calculation(&values).unwrap(), 0);
        
        // Test edge cases
        assert!(matches!(
            math.safe_volatility_calculation(&[100]),
            Err(SafeMathError::InsufficientData)
        ));
        
        assert!(matches!(
            math.safe_volatility_calculation(&[]),
            Err(SafeMathError::InsufficientData)
        ));
    }

    #[test]
    fn test_safe_max_drawdown() {
        let mut math = SafeMath128::new();
        
        // Test basic drawdown calculation
        let values = vec![100, 120, 80, 150, 70]; // Peak 150, trough 70
        let result = math.safe_max_drawdown(&values).unwrap();
        assert!(result > 0); // Should have drawdown
        
        // Test monotonically increasing (no drawdown)
        let values = vec![100, 110, 120, 130, 140];
        assert_eq!(math.safe_max_drawdown(&values).unwrap(), 0);
        
        // Test edge cases
        assert!(matches!(
            math.safe_max_drawdown(&[100]),
            Err(SafeMathError::InsufficientData)
        ));
    }

    #[test]
    fn test_detailed_precision_report() {
        let mut math = SafeMath128::new();
        
        // Perform some operations
        let _ = math.safe_bps_division_floor(1000, 3333);
        let _ = math.safe_bps_division_floor(2000, 3333);
        
        let report = math.get_detailed_precision_report();
        assert_eq!(report.operation_count, 2);
        assert!(report.cumulative_precision_loss > 0);
        assert!(report.efficiency_score > 0);
        assert!(!report.recommended_optimizations.is_empty());
        
        // Test precision quality assessment
        match report.precision_quality {
            crate::safe_math_128::PrecisionQuality::Perfect => {
                assert_eq!(report.cumulative_precision_loss, 0);
            }
            _ => {
                assert!(report.cumulative_precision_loss > 0);
            }
        }
    }

    #[test]
    fn test_precision_quality_assessment() {
        let mut math = SafeMath128::new();
        
        // Test perfect precision (no operations)
        let report = math.get_detailed_precision_report();
        assert!(matches!(report.precision_quality, crate::safe_math_128::PrecisionQuality::Perfect));
        
        // Perform operations with minimal precision loss
        let _ = math.safe_bps_division_floor(10000, 5000); // Exact division
        let report = math.get_detailed_precision_report();
        assert!(matches!(
            report.precision_quality,
            crate::safe_math_128::PrecisionQuality::Perfect | crate::safe_math_128::PrecisionQuality::Excellent
        ));
        
        // Perform operations with significant precision loss
        for _ in 0..100 {
            let _ = math.safe_bps_division_floor(1, 3333); // Creates precision loss
        }
        let report = math.get_detailed_precision_report();
        assert!(matches!(
            report.precision_quality,
            crate::safe_math_128::PrecisionQuality::Fair | crate::safe_math_128::PrecisionQuality::Poor
        ));
    }

    #[test]
    fn test_optimization_recommendations() {
        let mut math = SafeMath128::new();
        
        // Test with no operations (perfect precision)
        let report = math.get_detailed_precision_report();
        assert!(report.recommended_optimizations.len() > 0);
        assert!(report.recommended_optimizations.iter().any(|rec| 
            rec.contains("Perfect precision")
        ));
        
        // Perform operations with high precision loss
        for _ in 0..1000 {
            let _ = math.safe_bps_division_floor(1, 3333);
        }
        let report = math.get_detailed_precision_report();
        assert!(report.recommended_optimizations.len() > 0);
        assert!(report.recommended_optimizations.iter().any(|rec| 
            rec.contains("CRITICAL") || rec.contains("High precision loss")
        ));
    }

    #[test]
    fn test_advanced_mathematical_properties() {
        let mut math = SafeMath128::new();
        
        // Test power properties
        assert_eq!(math.safe_power(5, 0).unwrap(), 1);
        assert_eq!(math.safe_power(5, 1).unwrap(), 5);
        assert_eq!(math.safe_power(1, 100).unwrap(), 1);
        
        // Test compound interest properties
        let simple_yield = math.safe_mul_yield(1000, 1000, 86400).unwrap(); // Simple interest
        math.reset();
        let compound_yield = math.safe_compound_interest(1000, 1000, 1, 1).unwrap(); // Compound interest
        // Compound should be >= simple for positive rates
        assert!(compound_yield >= simple_yield);
        
        // Test EMA smoothing properties
        let ema_low_alpha = math.safe_ema(100, 80, 1000).unwrap(); // 10% alpha
        math.reset();
        let ema_high_alpha = math.safe_ema(100, 80, 8000).unwrap(); // 80% alpha
        // Higher alpha should be closer to new value
        assert!(ema_high_alpha > ema_low_alpha);
    }

    #[test]
    fn test_real_world_financial_scenarios() {
        let mut math = SafeMath128::new();
        
        // Scenario 1: Portfolio yield calculation
        let deposits = vec![
            (1000000, 500, 86400 * 30),  // Large deposit, 5% rate
            (500000, 800, 86400 * 60),   // Medium deposit, 8% rate
            (200000, 1200, 86400 * 90),  // Small deposit, 12% rate
        ];
        
        let yields = math.batch_safe_mul_yield(&deposits).unwrap();
        assert_eq!(yields.len(), 3);
        assert!(yields.iter().all(|&y| y > 0));
        
        // Calculate portfolio weighted average yield
        let weighted_yield = math.safe_weighted_average(&[
            (yields[0], deposits[0].0),
            (yields[1], deposits[1].0),
            (yields[2], deposits[2].0),
        ]).unwrap();
        assert!(weighted_yield > 0);
        
        // Scenario 2: Risk metrics calculation
        let historical_yields = vec![1000, 1200, 800, 1500, 900, 1100, 1300];
        let volatility = math.safe_volatility_calculation(&historical_yields).unwrap();
        let max_drawdown = math.safe_max_drawdown(&historical_yields).unwrap();
        
        assert!(volatility >= 0);
        assert!(max_drawdown >= 0);
        
        // Scenario 3: Compound interest vs simple interest
        let principal = 1000000;
        let annual_rate = 1000; // 10%
        
        let simple_yield = math.safe_mul_yield(principal, annual_rate, 86400 * 365).unwrap();
        math.reset();
        let compound_yield = math.safe_compound_interest(principal, annual_rate, 12, 1).unwrap();
        
        // Compound should be greater than simple for positive rates
        assert!(compound_yield > simple_yield);
        
        // Scenario 4: EMA trend analysis
        let yields = vec![1000, 1100, 1050, 1200, 1150, 1300];
        let mut ema = yields[0];
        
        for &current_yield in yields.iter().skip(1) {
            ema = math.safe_ema(current_yield, ema, 2000).unwrap(); // 20% alpha
        }
        
        // EMA should be between min and max values
        assert!(ema >= *yields.iter().min().unwrap());
        assert!(ema <= *yields.iter().max().unwrap());
    }
}
