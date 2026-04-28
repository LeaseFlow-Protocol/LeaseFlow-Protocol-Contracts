//! Precision-Safe Wear and Tear Deduction Calculations
//!
//! This module provides precise, mathematically-sound calculations for prorated
//! wear and tear deductions, ensuring minimal precision loss and fair outcomes
//! for both tenants and landlords in the LeaseFlow Protocol.

use soroban_sdk::{i128, u64};

/// Precision-safe wear and tear calculator
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WearTearCalculator {
    /// Cumulative dust from prorated calculations
    pub cumulative_dust: i128,
    /// Number of precision-safe operations performed
    pub operation_count: u64,
}

impl WearTearCalculator {
    pub fn new() -> Self {
        Self {
            cumulative_dust: 0,
            operation_count: 0,
        }
    }

    /// Calculate prorated wear and tear deduction using ceiling division
    /// to ensure the landlord is never disadvantaged by precision loss.
    ///
    /// Formula: (total_amount * elapsed_time) / total_time
    /// Uses ceiling division to round in favor of the landlord (protocol-safe).
    pub fn calculate_prorated_deduction_ceiling(
        &mut self,
        total_amount: i128,
        elapsed_time: u64,
        total_time: u64,
    ) -> Result<i128, WearTearError> {
        if total_amount < 0 {
            return Err(WearTearError::NegativeAmount(total_amount));
        }
        if total_time == 0 {
            return Err(WearTearError::DivisionByZero);
        }
        if elapsed_time > total_time {
            return Err(WearTearError::InvalidTimeRange {
                elapsed: elapsed_time,
                total: total_time,
            });
        }

        // Use ceiling division: (a + b - 1) / b = ceil(a / b)
        let numerator = total_amount.saturating_mul(elapsed_time as i128);
        let denominator = total_time as i128;
        
        let result = if denominator > 0 {
            let ceiling_result = numerator.saturating_add(denominator.saturating_sub(1)) / denominator;
            ceiling_result.min(total_amount) // Cap at total amount
        } else {
            0
        };

        // Track dust: remainder from the division
        let remainder = numerator % denominator;
        self.cumulative_dust = self.cumulative_dust.saturating_add(remainder);
        self.operation_count += 1;

        Ok(result)
    }

    /// Calculate prorated wear and tear deduction using floor division
    /// for tenant-favorable calculations (e.g., refunds).
    ///
    /// Formula: (total_amount * elapsed_time) / total_time
    /// Uses floor division to round in favor of the tenant.
    pub fn calculate_prorated_deduction_floor(
        &mut self,
        total_amount: i128,
        elapsed_time: u64,
        total_time: u64,
    ) -> Result<i128, WearTearError> {
        if total_amount < 0 {
            return Err(WearTearError::NegativeAmount(total_amount));
        }
        if total_time == 0 {
            return Err(WearTearError::DivisionByZero);
        }
        if elapsed_time > total_time {
            return Err(WearTearError::InvalidTimeRange {
                elapsed: elapsed_time,
                total: total_time,
            });
        }

        // Use floor division (standard integer division)
        let numerator = total_amount.saturating_mul(elapsed_time as i128);
        let denominator = total_time as i128;
        
        let result = if denominator > 0 {
            (numerator / denominator).min(total_amount)
        } else {
            0
        };

        // Track dust: remainder from the division
        let remainder = numerator % denominator;
        self.cumulative_dust = self.cumulative_dust.saturating_add(remainder);
        self.operation_count += 1;

        Ok(result)
    }

    /// Calculate percentage-based deduction with precision-safe rounding.
    ///
    /// Formula: (amount * percentage_bps) / 10000
    /// Uses ceiling division for landlord-favorable calculations.
    pub fn calculate_percentage_deduction_ceiling(
        &mut self,
        amount: i128,
        percentage_bps: u32,
    ) -> Result<i128, WearTearError> {
        if amount < 0 {
            return Err(WearTearError::NegativeAmount(amount));
        }
        if percentage_bps > 10000 {
            return Err(WearTearError::InvalidBps(percentage_bps));
        }

        const BPS_SCALE: i128 = 10000;
        
        // Ceiling division: (a * b + scale - 1) / scale
        let numerator = amount.saturating_mul(percentage_bps as i128);
        let result = numerator.saturating_add(BPS_SCALE.saturating_sub(1)) / BPS_SCALE;
        
        // Track dust
        let remainder = numerator % BPS_SCALE;
        self.cumulative_dust = self.cumulative_dust.saturating_add(remainder);
        self.operation_count += 1;

        Ok(result.min(amount))
    }

    /// Calculate percentage-based deduction with floor division.
    ///
    /// Formula: (amount * percentage_bps) / 10000
    /// Uses floor division for tenant-favorable calculations.
    pub fn calculate_percentage_deduction_floor(
        &mut self,
        amount: i128,
        percentage_bps: u32,
    ) -> Result<i128, WearTearError> {
        if amount < 0 {
            return Err(WearTearError::NegativeAmount(amount));
        }
        if percentage_bps > 10000 {
            return Err(WearTearError::InvalidBps(percentage_bps));
        }

        const BPS_SCALE: i128 = 10000;
        
        // Floor division
        let numerator = amount.saturating_mul(percentage_bps as i128);
        let result = numerator / BPS_SCALE;
        
        // Track dust
        let remainder = numerator % BPS_SCALE;
        self.cumulative_dust = self.cumulative_dust.saturating_add(remainder);
        self.operation_count += 1;

        Ok(result.min(amount))
    }

    /// Calculate wear and tear deduction based on damage severity with
    /// time-based proration for partial lease periods.
    ///
    /// This is the main function for Issue #187: precise wear and tear deductions.
    pub fn calculate_wear_tear_deduction(
        &mut self,
        total_deposit: i128,
        damage_severity_percentage: u32, // 0-100
        lease_elapsed_time: u64,
        lease_total_time: u64,
        use_ceiling_division: bool, // true = landlord-favorable, false = tenant-favorable
    ) -> Result<i128, WearTearError> {
        if total_deposit < 0 {
            return Err(WearTearError::NegativeAmount(total_deposit));
        }
        if damage_severity_percentage > 100 {
            return Err(WearTearError::InvalidPercentage(damage_severity_percentage));
        }

        // First, calculate the base deduction based on damage severity
        let base_deduction = if use_ceiling_division {
            self.calculate_percentage_deduction_ceiling(
                total_deposit,
                damage_severity_percentage * 100, // Convert to BPS
            )?
        } else {
            self.calculate_percentage_deduction_floor(
                total_deposit,
                damage_severity_percentage * 100, // Convert to BPS
            )?
        };

        // Then, prorate based on time if lease was not completed
        if lease_elapsed_time < lease_total_time && lease_total_time > 0 {
            if use_ceiling_division {
                self.calculate_prorated_deduction_ceiling(
                    base_deduction,
                    lease_elapsed_time,
                    lease_total_time,
                )
            } else {
                self.calculate_prorated_deduction_floor(
                    base_deduction,
                    lease_elapsed_time,
                    lease_total_time,
                )
            }
        } else {
            // Full lease period - no proration needed
            Ok(base_deduction)
        }
    }

    /// Get the precision report
    pub fn get_precision_report(&self) -> WearTearPrecisionReport {
        WearTearPrecisionReport {
            cumulative_dust: self.cumulative_dust,
            operation_count: self.operation_count,
            average_dust_per_operation: if self.operation_count > 0 {
                self.cumulative_dust / self.operation_count as i128
            } else {
                0
            },
        }
    }

    /// Reset the calculator state
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

/// Precision report for wear and tear calculations
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WearTearPrecisionReport {
    pub cumulative_dust: i128,
    pub operation_count: u64,
    pub average_dust_per_operation: i128,
}

/// Wear and tear calculation errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WearTearError {
    NegativeAmount(i128),
    DivisionByZero,
    InvalidTimeRange { elapsed: u64, total: u64 },
    InvalidBps(u32),
    InvalidPercentage(u32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prorated_deduction_ceiling() {
        let mut calc = WearTearCalculator::new();
        
        // Test exact division
        let result = calc.calculate_prorated_deduction_ceiling(1000, 50, 100).unwrap();
        assert_eq!(result, 500);
        
        // Test ceiling division (rounds up)
        let result = calc.calculate_prorated_deduction_ceiling(1000, 33, 100).unwrap();
        assert_eq!(result, 330); // 1000 * 33 / 100 = 330 (exact)
        
        // Test with remainder (should round up)
        let result = calc.calculate_prorated_deduction_ceiling(1000, 1, 3).unwrap();
        assert_eq!(result, 334); // ceil(1000 / 3) = 334
    }

    #[test]
    fn test_prorated_deduction_floor() {
        let mut calc = WearTearCalculator::new();
        
        // Test exact division
        let result = calc.calculate_prorated_deduction_floor(1000, 50, 100).unwrap();
        assert_eq!(result, 500);
        
        // Test floor division (rounds down)
        let result = calc.calculate_prorated_deduction_floor(1000, 1, 3).unwrap();
        assert_eq!(result, 333); // floor(1000 / 3) = 333
    }

    #[test]
    fn test_percentage_deduction_ceiling() {
        let mut calc = WearTearCalculator::new();
        
        // Test exact percentage
        let result = calc.calculate_percentage_deduction_ceiling(1000, 5000).unwrap();
        assert_eq!(result, 500); // 50%
        
        // Test with remainder (should round up)
        let result = calc.calculate_percentage_deduction_ceiling(1000, 3333).unwrap();
        assert_eq!(result, 334); // ceil(1000 * 0.3333) = 334
    }

    #[test]
    fn test_percentage_deduction_floor() {
        let mut calc = WearTearCalculator::new();
        
        // Test exact percentage
        let result = calc.calculate_percentage_deduction_floor(1000, 5000).unwrap();
        assert_eq!(result, 500); // 50%
        
        // Test with remainder (should round down)
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
    fn test_wear_tear_deduction_prorated() {
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
    fn test_dust_tracking() {
        let mut calc = WearTearCalculator::new();
        
        // Perform operations that create dust
        let _ = calc.calculate_prorated_deduction_ceiling(1000, 1, 3);
        let _ = calc.calculate_prorated_deduction_ceiling(1000, 1, 7);
        
        let report = calc.get_precision_report();
        assert_eq!(report.operation_count, 2);
        assert!(report.cumulative_dust > 0);
    }

    #[test]
    fn test_error_conditions() {
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
}
