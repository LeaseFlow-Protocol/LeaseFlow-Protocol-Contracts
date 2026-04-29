//! Native 128-bit Safe Math Optimizations for Complex Yield Generation
//!
//! This module provides highly optimized, overflow-safe mathematical operations
//! specifically designed for complex yield-generation scenarios on locked deposits
//! in the LeaseFlow Protocol. It leverages native 128-bit arithmetic with
//! comprehensive overflow protection and precision tracking.
//!
//! Key optimizations:
//! - Native i128 arithmetic with overflow protection
//! - Zero-cost abstractions where possible
//! - Precision loss tracking for yield calculations
//! - Gas-optimized operations for high-frequency yield harvesting

use soroban_sdk::{i128, u128, u64, Env};

/// Maximum safe value for yield calculations to prevent overflow
/// in complex scenarios (principal * rate * time * multiplier)
pub const MAX_SAFE_YIELD_AMOUNT: i128 = i128::MAX / 1_000_000; // Safety factor

/// Basis points scale (10000 = 100%)
pub const BPS_SCALE: i128 = 10_000;

/// Precision scale for high-precision yield calculations
pub const PRECISION_SCALE: i128 = 1_000_000_000_000; // 12 decimal places

/// Optimized safe math operations for yield generation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeMath128 {
    /// Cumulative precision loss from yield calculations
    pub cumulative_precision_loss: i128,
    /// Number of operations performed
    pub operation_count: u64,
    /// Maximum precision loss observed in single operation
    pub max_single_operation_loss: i128,
}

impl SafeMath128 {
    pub fn new() -> Self {
        Self {
            cumulative_precision_loss: 0,
            operation_count: 0,
            max_single_operation_loss: 0,
        }
    }

    /// Safe multiplication with overflow protection and bounds checking
    /// Optimized for yield calculation scenarios
    pub fn safe_mul_yield(
        &mut self,
        principal: i128,
        rate_bps: u32,
        time_factor: u64,
    ) -> Result<i128, SafeMathError> {
        if principal < 0 {
            return Err(SafeMathError::NegativePrincipal(principal));
        }
        if rate_bps > BPS_SCALE as u32 {
            return Err(SafeMathError::InvalidRate(rate_bps));
        }
        if time_factor == 0 {
            return Ok(0);
        }

        // Check for potential overflow before calculation
        if principal > MAX_SAFE_YIELD_AMOUNT {
            return Err(SafeMathError::PrincipalTooLarge(principal));
        }

        // Optimized calculation: (principal * rate_bps * time_factor) / BPS_SCALE
        let rate_factor = rate_bps as i128;
        
        // First multiply principal by rate (most common overflow point)
        let principal_times_rate = principal.checked_mul(rate_factor)
            .ok_or(SafeMathError::MultiplicationOverflow)?;

        // Then multiply by time factor
        let full_numerator = principal_times_rate.checked_mul(time_factor as i128)
            .ok_or(SafeMathError::MultiplicationOverflow)?;

        // Final division by BPS_SCALE
        let result = full_numerator.checked_div(BPS_SCALE)
            .ok_or(SafeMathError::DivisionError)?;

        // Track precision loss from division
        let remainder = full_numerator.checked_rem(BPS_SCALE).unwrap_or(0);
        self.track_precision_loss(remainder)?;

        // Ensure result doesn't exceed principal by more than reasonable rate
        let max_reasonable_yield = principal.checked_mul(1000) // 100x principal max
            .ok_or(SafeMathError::MultiplicationOverflow)?;
        if result > max_reasonable_yield {
            return Err(SafeMathError::UnreasonableYield(result));
        }

        Ok(result)
    }

    /// Safe addition for yield accumulation with overflow protection
    pub fn safe_add_yield(
        &mut self,
        current_yield: i128,
        additional_yield: i128,
    ) -> Result<i128, SafeMathError> {
        if current_yield < 0 || additional_yield < 0 {
            return Err(SafeMathError::NegativeYield {
                current: current_yield,
                additional: additional_yield,
            });
        }

        let result = current_yield.checked_add(additional_yield)
            .ok_or(SafeMathError::AdditionOverflow)?;

        // Check against maximum safe yield amount
        if result > MAX_SAFE_YIELD_AMOUNT {
            return Err(SafeMathError::YieldExceedsMaximum(result));
        }

        self.operation_count += 1;
        Ok(result)
    }

    /// Safe subtraction for yield distribution with underflow protection
    pub fn safe_sub_yield(
        &mut self,
        total_yield: i128,
        distribution_amount: i128,
    ) -> Result<i128, SafeMathError> {
        if total_yield < 0 || distribution_amount < 0 {
            return Err(SafeMathError::NegativeDistribution {
                total: total_yield,
                amount: distribution_amount,
            });
        }

        if distribution_amount > total_yield {
            return Err(SafeMathError::DistributionExceedsYield {
                total_yield,
                distribution_amount,
            });
        }

        let result = total_yield.checked_sub(distribution_amount)
            .map_err(|_| SafeMathError::SubtractionUnderflow)?;

        self.operation_count += 1;
        Ok(result)
    }

    /// Precision-safe basis points calculation for yield distribution
    /// Uses ceiling division for protocol-favorable calculations
    pub fn safe_bps_division_ceiling(
        &mut self,
        amount: i128,
        bps: u32,
    ) -> Result<i128, SafeMathError> {
        if amount < 0 {
            return Err(SafeMathError::NegativeAmount(amount));
        }
        if bps > BPS_SCALE as u32 {
            return Err(SafeMathError::InvalidBps(bps));
        }

        // Ceiling division: (amount * bps + BPS_SCALE - 1) / BPS_SCALE
        let numerator = amount.checked_mul(bps as i128)
            .ok_or(SafeMathError::MultiplicationOverflow)?;
        
        let ceiling_numerator = numerator.checked_add(BPS_SCALE - 1)
            .ok_or(SafeMathError::AdditionOverflow)?;
        
        let result = ceiling_numerator.checked_div(BPS_SCALE)
            .ok_or(SafeMathError::DivisionError)?;

        // Track precision loss
        let remainder = numerator.checked_rem(BPS_SCALE).unwrap_or(0);
        if remainder > 0 {
            let precision_loss = BPS_SCALE - remainder;
            self.track_precision_loss(precision_loss)?;
        }

        Ok(result)
    }

    /// Precision-safe basis points calculation using floor division
    /// Uses floor division for user-favorable calculations
    pub fn safe_bps_division_floor(
        &mut self,
        amount: i128,
        bps: u32,
    ) -> Result<i128, SafeMathError> {
        if amount < 0 {
            return Err(SafeMathError::NegativeAmount(amount));
        }
        if bps > BPS_SCALE as u32 {
            return Err(SafeMathError::InvalidBps(bps));
        }

        // Floor division: (amount * bps) / BPS_SCALE
        let numerator = amount.checked_mul(bps as i128)
            .ok_or(SafeMathError::MultiplicationOverflow)?;
        
        let result = numerator.checked_div(BPS_SCALE)
            .ok_or(SafeMathError::DivisionError)?;

        // Track precision loss (remainder)
        let remainder = numerator.checked_rem(BPS_SCALE).unwrap_or(0);
        self.track_precision_loss(remainder)?;

        Ok(result)
    }

    /// Complex yield calculation with multiple factors
    /// Formula: principal * (rate_bps/10000) * (time_factor/86400) * (multiplier/1000)
    pub fn complex_yield_calculation(
        &mut self,
        principal: i128,
        rate_bps: u32,
        time_factor: u64,
        multiplier_bps: u32,
    ) -> Result<i128, SafeMathError> {
        if principal > MAX_SAFE_YIELD_AMOUNT / 1000 {
            return Err(SafeMathError::PrincipalTooLarge(principal));
        }

        // Step 1: Calculate base yield using safe multiplication
        let base_yield = self.safe_mul_yield(principal, rate_bps, time_factor)?;

        // Step 2: Apply multiplier using BPS
        let final_yield = if multiplier_bps != 10000 {
            self.safe_bps_division_floor(base_yield, multiplier_bps)?
        } else {
            base_yield
        };

        // Step 3: Ensure result is reasonable
        let max_yield = principal.checked_mul(10000) // 1000x max
            .ok_or(SafeMathError::MultiplicationOverflow)?;
        
        if final_yield > max_yield {
            return Err(SafeMathError::ComplexYieldOverflow {
                principal,
                rate_bps,
                time_factor,
                multiplier_bps,
                result: final_yield,
            });
        }

        Ok(final_yield)
    }

    /// Yield distribution calculation with dust tracking
    /// Ensures total distribution doesn't exceed available yield
    pub fn yield_distribution_with_dust_tracking(
        &mut self,
        total_yield: i128,
        lessee_bps: u32,
        lessor_bps: u32,
        dao_bps: u32,
    ) -> Result<YieldDistribution, SafeMathError> {
        if total_yield < 0 {
            return Err(SafeMathError::NegativeYield {
                current: total_yield,
                additional: 0,
            });
        }

        let total_bps = lessee_bps + lessor_bps + dao_bps;
        if total_bps > BPS_SCALE as u32 {
            return Err(SafeMathError::InvalidDistributionBps(total_bps));
        }

        // Calculate individual shares using ceiling division for protocol safety
        let lessee_share = self.safe_bps_division_ceiling(total_yield, lessee_bps)?;
        let lessor_share = self.safe_bps_division_ceiling(total_yield, lessor_bps)?;
        let dao_share = self.safe_bps_division_ceiling(total_yield, dao_bps)?;

        let total_distributed = lessee_share.checked_add(lessor_share)
            .and_then(|sum| sum.checked_add(dao_share))
            .ok_or(SafeMathError::DistributionOverflow)?;

        // Check if distribution exceeds available yield
        if total_distributed > total_yield {
            return Err(SafeMathError::DistributionExceedsYield {
                total_yield,
                distribution_amount: total_distributed,
            });
        }

        // Calculate dust (unallocated amount due to rounding)
        let dust = total_yield.checked_sub(total_distributed)
            .ok_or(SafeMathError::DustCalculationError)?;

        Ok(YieldDistribution {
            lessee_share,
            lessor_share,
            dao_share,
            dust,
            total_distributed,
        })
    }

    /// Track precision loss for monitoring and optimization
    fn track_precision_loss(&mut self, loss: i128) -> Result<(), SafeMathError> {
        self.cumulative_precision_loss = self.cumulative_precision_loss.checked_add(loss)
            .ok_or(SafeMathError::PrecisionLossOverflow)?;
        self.operation_count += 1;
        self.max_single_operation_loss = self.max_single_operation_loss.max(loss);
        Ok(())
    }

    /// Get comprehensive precision report
    pub fn get_precision_report(&self) -> PrecisionReport {
        PrecisionReport {
            cumulative_precision_loss: self.cumulative_precision_loss,
            operation_count: self.operation_count,
            max_single_operation_loss: self.max_single_operation_loss,
            average_loss_per_operation: if self.operation_count > 0 {
                self.cumulative_precision_loss / self.operation_count as i128
            } else {
                0
            },
            efficiency_score: self.calculate_efficiency_score(),
        }
    }

    /// Calculate efficiency score based on precision loss vs operations
    fn calculate_efficiency_score(&self) -> u32 {
        if self.operation_count == 0 {
            return 100; // Perfect score for no operations
        }

        let loss_per_op = self.cumulative_precision_loss / self.operation_count as i128;
        
        // Score decreases with higher precision loss
        if loss_per_op == 0 {
            100
        } else if loss_per_op < 100 {
            95
        } else if loss_per_op < 1000 {
            85
        } else if loss_per_op < 10000 {
            70
        } else {
            50
        }
    }

    /// Reset tracking state
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Advanced batch multiplication for multiple yield calculations
    /// Optimized for processing multiple deposits simultaneously
    pub fn batch_safe_mul_yield(
        &mut self,
        inputs: &[(i128, u32, u64)], // (principal, rate_bps, time_factor)
    ) -> Result<Vec<i128>, SafeMathError> {
        let mut results = Vec::with_capacity(inputs.len());
        
        for &(principal, rate_bps, time_factor) in inputs {
            let result = self.safe_mul_yield(principal, rate_bps, time_factor)?;
            results.push(result);
        }
        
        Ok(results)
    }

    /// Safe power operation for compound interest calculations
    /// Calculates (base ^ exponent) with overflow protection
    pub fn safe_power(
        &mut self,
        base: i128,
        exponent: u32,
    ) -> Result<i128, SafeMathError> {
        if base == 0 {
            return Ok(0);
        }
        if base == 1 || exponent == 0 {
            return Ok(1);
        }
        if exponent == 1 {
            return Ok(base);
        }

        // Use exponentiation by squaring for efficiency
        let mut result = 1i128;
        let mut current_base = base;
        let mut remaining_exp = exponent;

        while remaining_exp > 0 {
            if remaining_exp % 2 == 1 {
                result = result.checked_mul(current_base)
                    .ok_or(SafeMathError::MultiplicationOverflow)?;
            }
            
            if remaining_exp > 1 {
                current_base = current_base.checked_mul(current_base)
                    .ok_or(SafeMathError::MultiplicationOverflow)?;
            }
            
            remaining_exp /= 2;
        }

        Ok(result)
    }

    /// Safe compound interest calculation
    /// Formula: principal * (1 + rate/n)^(n*t) - principal
    pub fn safe_compound_interest(
        &mut self,
        principal: i128,
        annual_rate_bps: u32,
        periods_per_year: u32,
        years: u32,
    ) -> Result<i128, SafeMathError> {
        if principal < 0 {
            return Err(SafeMathError::NegativePrincipal(principal));
        }
        if annual_rate_bps > BPS_SCALE as u32 {
            return Err(SafeMathError::InvalidRate(annual_rate_bps));
        }
        if periods_per_year == 0 || years == 0 {
            return Ok(0);
        }

        let rate_per_period = annual_rate_bps as i128 / periods_per_year as i128;
        let total_periods = periods_per_year as u32 * years;
        
        // Calculate (1 + rate_per_period/BPS_SCALE) ^ total_periods
        let growth_factor_base = BPS_SCALE + rate_per_period;
        let growth_factor = self.safe_power(growth_factor_base, total_periods)?;
        
        // Calculate final amount: (principal * growth_factor) / BPS_SCALE
        let final_amount = principal.checked_mul(growth_factor)
            .and_then(|x| x.checked_div(BPS_SCALE))
            .ok_or(SafeMathError::MultiplicationOverflow)?;
        
        // Yield is final amount minus principal
        let yield_amount = final_amount.checked_sub(principal)
            .ok_or(SafeMathError::UnderflowError)?;

        // Track precision loss from compound calculations
        let precision_loss = final_amount.checked_rem(BPS_SCALE).unwrap_or(0);
        self.track_precision_loss(precision_loss)?;

        Ok(yield_amount)
    }

    /// Safe logarithm approximation for yield calculations
    /// Uses natural logarithm approximation: ln(x) ≈ 2 * (x-1)/(x+1) for x near 1
    pub fn safe_ln_approximation(
        &mut self,
        value: i128,
    ) -> Result<i128, SafeMathError> {
        if value <= 0 {
            return Err(SafeMathError::InvalidLogInput(value));
        }
        if value == 1 {
            return Ok(0);
        }

        // Scale value for precision
        let scaled_value = value.checked_mul(1000)
            .ok_or(SafeMathError::MultiplicationOverflow)?;
        
        // Use approximation: ln(x) ≈ 2 * (x-1)/(x+1)
        let numerator = scaled_value.checked_sub(1000)
            .ok_or(SafeMathError::UnderflowError)?;
        let denominator = scaled_value.checked_add(1000)
            .ok_or(SafeMathError::AdditionOverflow)?;
        
        let ln_approx = numerator.checked_mul(2000) // 2 * 1000 for scaling
            .and_then(|x| x.checked_div(denominator))
            .ok_or(SafeMathError::DivisionError)?;

        Ok(ln_approx)
    }

    /// Safe square root approximation using Newton's method
    /// Useful for volatility calculations and risk metrics
    pub fn safe_sqrt_approximation(
        &mut self,
        value: i128,
        iterations: u32,
    ) -> Result<i128, SafeMathError> {
        if value < 0 {
            return Err(SafeMathError::NegativeAmount(value));
        }
        if value == 0 || value == 1 {
            return Ok(value);
        }

        let mut guess = value / 2; // Initial guess
        
        for _ in 0..iterations {
            if guess == 0 {
                break;
            }
            
            // Newton's method: guess = (guess + value/guess) / 2
            let value_div_guess = value.checked_div(guess)
                .ok_or(SafeMathError::DivisionError)?;
            let sum = guess.checked_add(value_div_guess)
                .ok_or(SafeMathError::AdditionOverflow)?;
            guess = sum / 2;
        }

        Ok(guess)
    }

    /// Safe percentage change calculation
    /// Calculates ((new_value - old_value) / old_value) * 10000 (in BPS)
    pub fn safe_percentage_change(
        &mut self,
        old_value: i128,
        new_value: i128,
    ) -> Result<i128, SafeMathError> {
        if old_value <= 0 {
            return Err(SafeMathError::DivisionByZero);
        }

        let difference = new_value.checked_sub(old_value)
            .ok_or(SafeMathError::UnderflowError)?;
        
        let change_bps = difference.checked_mul(10000)
            .and_then(|x| x.checked_div(old_value))
            .ok_or(SafeMathError::DivisionError)?;

        Ok(change_bps)
    }

    /// Safe weighted average calculation
    /// Useful for portfolio yield calculations
    pub fn safe_weighted_average(
        &mut self,
        values: &[(i128, i128)], // (value, weight)
    ) -> Result<i128, SafeMathError> {
        if values.is_empty() {
            return Err(SafeMathError::EmptyInput);
        }

        let mut weighted_sum = 0i128;
        let mut total_weight = 0i128;

        for &(value, weight) in values {
            if weight < 0 {
                return Err(SafeMathError::NegativeWeight(weight));
            }
            
            let weighted_value = value.checked_mul(weight)
                .ok_or(SafeMathError::MultiplicationOverflow)?;
            weighted_sum = weighted_sum.checked_add(weighted_value)
                .ok_or(SafeMathError::AdditionOverflow)?;
            total_weight = total_weight.checked_add(weight)
                .ok_or(SafeMathError::AdditionOverflow)?;
        }

        if total_weight == 0 {
            return Err(SafeMathError::DivisionByZero);
        }

        let average = weighted_sum.checked_div(total_weight)
            .ok_or(SafeMathError::DivisionError)?;

        Ok(average)
    }

    /// Safe exponential moving average calculation
    /// EMA(t) = α * value(t) + (1-α) * EMA(t-1)
    pub fn safe_ema(
        &mut self,
        current_value: i128,
        previous_ema: i128,
        alpha_bps: u32, // α in basis points (e.g., 2000 for 20%)
    ) -> Result<i128, SafeMathError> {
        if alpha_bps > BPS_SCALE as u32 {
            return Err(SafeMathError::InvalidBps(alpha_bps));
        }

        let alpha_scaled = alpha_bps as i128;
        let one_minus_alpha = BPS_SCALE - alpha_scaled;

        // α * current_value
        let alpha_component = current_value.checked_mul(alpha_scaled)
            .ok_or(SafeMathError::MultiplicationOverflow)?;
        
        // (1-α) * previous_ema
        let ema_component = previous_ema.checked_mul(one_minus_alpha)
            .ok_or(SafeMathError::MultiplicationOverflow)?;
        
        // Sum and scale
        let sum = alpha_component.checked_add(ema_component)
            .ok_or(SafeMathError::AdditionOverflow)?;
        
        let new_ema = sum.checked_div(BPS_SCALE)
            .ok_or(SafeMathError::DivisionError)?;

        Ok(new_ema)
    }

    /// Safe volatility calculation using standard deviation
    /// Measures yield volatility for risk assessment
    pub fn safe_volatility_calculation(
        &mut self,
        values: &[i128],
    ) -> Result<i128, SafeMathError> {
        if values.len() < 2 {
            return Err(SafeMathError::InsufficientData);
        }

        // Calculate mean
        let sum: i128 = values.iter().try_fold(0, |acc, &x| {
            acc.checked_add(x).ok_or(SafeMathError::AdditionOverflow)
        })?;
        let mean = sum.checked_div(values.len() as i128)
            .ok_or(SafeMathError::DivisionError)?;

        // Calculate variance
        let mut variance_sum = 0i128;
        for &value in values {
            let difference = value.checked_sub(mean)
                .ok_or(SafeMathError::UnderflowError)?;
            let squared = difference.checked_mul(difference)
                .ok_or(SafeMathError::MultiplicationOverflow)?;
            variance_sum = variance_sum.checked_add(squared)
                .ok_or(SafeMathError::AdditionOverflow)?;
        }

        let variance = variance_sum.checked_div(values.len() as i128)
            .ok_or(SafeMathError::DivisionError)?;

        // Calculate standard deviation (square root of variance)
        let volatility = self.safe_sqrt_approximation(variance, 10)?;

        Ok(volatility)
    }

    /// Safe maximum drawdown calculation
    /// Measures maximum loss from peak to trough
    pub fn safe_max_drawdown(
        &mut self,
        values: &[i128],
    ) -> Result<i128, SafeMathError> {
        if values.len() < 2 {
            return Err(SafeMathError::InsufficientData);
        }

        let mut peak = values[0];
        let mut max_drawdown = 0i128;

        for &value in values.iter().skip(1) {
            if value > peak {
                peak = value;
            } else {
                let drawdown = peak.checked_sub(value)
                    .ok_or(SafeMathError::UnderflowError)?;
                let drawdown_bps = drawdown.checked_mul(10000)
                    .and_then(|x| x.checked_div(peak))
                    .ok_or(SafeMathError::DivisionError)?;
                
                if drawdown_bps > max_drawdown {
                    max_drawdown = drawdown_bps;
                }
            }
        }

        Ok(max_drawdown)
    }

    /// Advanced precision tracking with detailed metrics
    pub fn get_detailed_precision_report(&self) -> DetailedPrecisionReport {
        let average_loss_per_operation = if self.operation_count > 0 {
            self.cumulative_precision_loss / self.operation_count as i128
        } else {
            0
        };

        let efficiency_score = self.calculate_efficiency_score();
        let precision_quality = self.assess_precision_quality();

        DetailedPrecisionReport {
            cumulative_precision_loss: self.cumulative_precision_loss,
            operation_count: self.operation_count,
            max_single_operation_loss: self.max_single_operation_loss,
            average_loss_per_operation,
            efficiency_score,
            precision_quality,
            recommended_optimizations: self.get_optimization_recommendations(),
        }
    }

    /// Assess precision quality based on accumulated loss
    fn assess_precision_quality(&self) -> PrecisionQuality {
        if self.operation_count == 0 {
            return PrecisionQuality::Perfect;
        }

        let avg_loss = self.cumulative_precision_loss / self.operation_count as i128;
        
        match avg_loss {
            0 => PrecisionQuality::Perfect,
            1..=10 => PrecisionQuality::Excellent,
            11..=100 => PrecisionQuality::Good,
            101..=1000 => PrecisionQuality::Fair,
            1001..=10000 => PrecisionQuality::Poor,
            _ => PrecisionQuality::Critical,
        }
    }

    /// Get optimization recommendations based on precision metrics
    fn get_optimization_recommendations(&self) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        let quality = self.assess_precision_quality();
        
        match quality {
            PrecisionQuality::Critical => {
                recommendations.push("CRITICAL: High precision loss detected. Consider using higher precision arithmetic.".to_string());
                recommendations.push("Review all division operations for optimization opportunities.".to_string());
            }
            PrecisionQuality::Poor => {
                recommendations.push("Consider implementing ceiling division for protocol-favorable calculations.".to_string());
                recommendations.push("Evaluate batch processing to reduce cumulative precision loss.".to_string());
            }
            PrecisionQuality::Fair => {
                recommendations.push("Monitor precision loss trends over time.".to_string());
                recommendations.push("Consider precision-aware rounding strategies.".to_string());
            }
            PrecisionQuality::Good | PrecisionQuality::Excellent => {
                recommendations.push("Precision is within acceptable ranges.".to_string());
                recommendations.push("Continue monitoring for optimization opportunities.".to_string());
            }
            PrecisionQuality::Perfect => {
                recommendations.push("Perfect precision maintained. Excellent optimization.".to_string());
            }
        }

        if self.max_single_operation_loss > 1000 {
            recommendations.push("Single operation with high precision loss detected. Investigate specific operation.".to_string());
        }

        recommendations
    }
}

/// Yield distribution result with dust tracking
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YieldDistribution {
    pub lessee_share: i128,
    pub lessor_share: i128,
    pub dao_share: i128,
    pub dust: i128,
    pub total_distributed: i128,
}

/// Precision report for optimization monitoring
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrecisionReport {
    pub cumulative_precision_loss: i128,
    pub operation_count: u64,
    pub max_single_operation_loss: i128,
    pub average_loss_per_operation: i128,
    pub efficiency_score: u32,
}

/// Detailed precision report with quality assessment and recommendations
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailedPrecisionReport {
    pub cumulative_precision_loss: i128,
    pub operation_count: u64,
    pub max_single_operation_loss: i128,
    pub average_loss_per_operation: i128,
    pub efficiency_score: u32,
    pub precision_quality: PrecisionQuality,
    pub recommended_optimizations: Vec<String>,
}

/// Precision quality assessment
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrecisionQuality {
    Perfect,    // No precision loss
    Excellent,  // 1-10 loss per operation
    Good,       // 11-100 loss per operation
    Fair,       // 101-1000 loss per operation
    Poor,       // 1001-10000 loss per operation
    Critical,   // >10000 loss per operation
}

/// Safe math operation errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafeMathError {
    NegativePrincipal(i128),
    NegativeAmount(i128),
    NegativeYield { current: i128, additional: i128 },
    NegativeDistribution { total: i128, amount: i128 },
    InvalidRate(u32),
    InvalidBps(u32),
    InvalidDistributionBps(u32),
    PrincipalTooLarge(i128),
    MultiplicationOverflow,
    AdditionOverflow,
    SubtractionUnderflow,
    DivisionError,
    DivisionByZero,
    YieldExceedsMaximum(i128),
    UnreasonableYield(i128),
    DistributionExceedsYield { total_yield: i128, distribution_amount: i128 },
    DistributionOverflow,
    ComplexYieldOverflow {
        principal: i128,
        rate_bps: u32,
        time_factor: u64,
        multiplier_bps: u32,
        result: i128,
    },
    PrecisionLossOverflow,
    DustCalculationError,
    UnderflowError,
    InvalidLogInput(i128),
    EmptyInput,
    NegativeWeight(i128),
    InsufficientData,
}

/// Gas-optimized inline helper functions for common operations
pub mod optimized_ops {
    use super::*;

    /// Inline optimized multiplication for yield calculations
    #[inline(always)]
    pub fn mul_yield_optimized(principal: i128, rate_bps: u32, time_factor: u64) -> Option<i128> {
        if principal == 0 || rate_bps == 0 || time_factor == 0 {
            return Some(0);
        }

        // Fast path for common cases
        if rate_bps == 10000 && time_factor == 1 {
            return Some(principal);
        }

        principal.checked_mul(rate_bps as i128)
            .and_then(|x| x.checked_mul(time_factor as i128))
            .map(|x| x / BPS_SCALE)
    }

    /// Inline optimized BPS calculation
    #[inline(always)]
    pub fn bps_floor_optimized(amount: i128, bps: u32) -> Option<i128> {
        if amount == 0 || bps == 0 {
            return Some(0);
        }
        if bps == 10000 {
            return Some(amount);
        }

        amount.checked_mul(bps as i128).map(|x| x / BPS_SCALE)
    }

    /// Inline optimized BPS ceiling calculation
    #[inline(always)]
    pub fn bps_ceiling_optimized(amount: i128, bps: u32) -> Option<i128> {
        if amount == 0 || bps == 0 {
            return Some(0);
        }
        if bps == 10000 {
            return Some(amount);
        }

        amount.checked_mul(bps as i128)
            .and_then(|x| x.checked_add(BPS_SCALE - 1))
            .map(|x| x / BPS_SCALE)
    }

    /// Fast overflow check for multiplication
    #[inline(always)]
    pub fn will_multiply_overflow(a: i128, b: i128) -> bool {
        if a == 0 || b == 0 {
            return false;
        }
        
        // Check if both are positive and result would exceed max
        a > 0 && b > 0 && a > i128::MAX / b
    }

    /// Fast overflow check for addition
    #[inline(always)]
    pub fn will_add_overflow(a: i128, b: i128) -> bool {
        a > 0 && b > 0 && a > i128::MAX - b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_mul_yield_basic() {
        let mut math = SafeMath128::new();
        
        // Test basic multiplication
        let result = math.safe_mul_yield(1000, 5000, 86400).unwrap(); // 50% for 1 day
        assert_eq!(result, 432000000); // 1000 * 0.5 * 86400
        
        // Test zero values
        assert_eq!(math.safe_mul_yield(1000, 0, 86400).unwrap(), 0);
        assert_eq!(math.safe_mul_yield(0, 5000, 86400).unwrap(), 0);
        assert_eq!(math.safe_mul_yield(1000, 5000, 0).unwrap(), 0);
    }

    #[test]
    fn test_safe_mul_yield_overflow_protection() {
        let mut math = SafeMath128::new();
        
        // Test overflow protection
        let large_principal = MAX_SAFE_YIELD_AMOUNT + 1;
        assert!(matches!(
            math.safe_mul_yield(large_principal, 5000, 86400),
            Err(SafeMathError::PrincipalTooLarge(_))
        ));
    }

    #[test]
    fn test_yield_distribution() {
        let mut math = SafeMath128::new();
        
        let distribution = math.yield_distribution_with_dust_tracking(
            1000, 5000, 3000, 2000
        ).unwrap();
        
        assert_eq!(distribution.lessee_share, 500); // 50%
        assert_eq!(distribution.lessor_share, 300); // 30%
        assert_eq!(distribution.dao_share, 200);    // 20%
        assert_eq!(distribution.total_distributed, 1000);
        assert_eq!(distribution.dust, 0);
    }

    #[test]
    fn test_yield_distribution_with_dust() {
        let mut math = SafeMath128::new();
        
        // Amount that creates dust
        let distribution = math.yield_distribution_with_dust_tracking(
            999, 3333, 3333, 3334
        ).unwrap();
        
        assert!(distribution.dust > 0);
        assert!(distribution.total_distributed <= 999);
    }

    #[test]
    fn test_complex_yield_calculation() {
        let mut math = SafeMath128::new();
        
        let result = math.complex_yield_calculation(
            1000,    // principal
            5000,    // 50% rate
            86400,   // 1 day time factor
            11000,   // 110% multiplier
        ).unwrap();
        
        assert!(result > 0);
        assert!(result < 1000 * 1000); // Should be reasonable
    }

    #[test]
    fn test_precision_tracking() {
        let mut math = SafeMath128::new();
        
        // Perform operations that create precision loss
        let _ = math.safe_bps_division_floor(1000, 3333);
        let _ = math.safe_bps_division_floor(1000, 3333);
        
        let report = math.get_precision_report();
        assert_eq!(report.operation_count, 2);
        assert!(report.cumulative_precision_loss > 0);
        assert!(report.efficiency_score > 0);
    }

    #[test]
    fn test_optimized_operations() {
        // Test optimized multiplication
        assert_eq!(
            optimized_ops::mul_yield_optimized(1000, 5000, 86400),
            Some(432000000)
        );
        
        // Test edge cases
        assert_eq!(optimized_ops::mul_yield_optimized(0, 5000, 86400), Some(0));
        assert_eq!(optimized_ops::mul_yield_optimized(1000, 0, 86400), Some(0));
        assert_eq!(optimized_ops::mul_yield_optimized(1000, 5000, 0), Some(0));
        
        // Test fast path optimizations
        assert_eq!(optimized_ops::bps_floor_optimized(1000, 10000), Some(1000));
        assert_eq!(optimized_ops::bps_ceiling_optimized(1000, 10000), Some(1000));
    }

    #[test]
    fn test_overflow_detection() {
        // Test multiplication overflow detection
        assert!(optimized_ops::will_multiply_overflow(i128::MAX, 2));
        assert!(!optimized_ops::will_multiply_overflow(1000, 1000));
        
        // Test addition overflow detection
        assert!(optimized_ops::will_add_overflow(i128::MAX, 1));
        assert!(!optimized_ops::will_add_overflow(1000, 1000));
    }

    #[test]
    fn test_error_conditions() {
        let mut math = SafeMath128::new();
        
        // Negative principal
        assert!(matches!(
            math.safe_mul_yield(-100, 5000, 86400),
            Err(SafeMathError::NegativePrincipal(_))
        ));
        
        // Invalid rate
        assert!(matches!(
            math.safe_mul_yield(100, 10001, 86400),
            Err(SafeMathError::InvalidRate(_))
        ));
        
        // Distribution exceeding yield
        assert!(matches!(
            math.yield_distribution_with_dust_tracking(100, 6000, 6000, 6000),
            Err(SafeMathError::InvalidDistributionBps(_))
        ));
    }
}
