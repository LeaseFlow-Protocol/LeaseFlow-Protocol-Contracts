//! Advanced Yield Integration Examples
//!
//! This module demonstrates how to integrate the advanced safe math optimizations
//! into the main LeaseFlow contract functions for enhanced yield generation
//! scenarios with risk metrics and portfolio management.

use soroban_sdk::{i128, u128, u64, Address, Env, Vec};
use crate::safe_math_128::{SafeMath128, DetailedPrecisionReport, PrecisionQuality};
use crate::enhanced_yield_generation::{EnhancedYieldGenerator, YieldConfig, YieldCalculationInput};
use crate::{LeaseInstance, DataKey, LeaseError};

/// Advanced yield manager with comprehensive risk assessment and portfolio optimization
pub struct AdvancedYieldManager {
    safe_math: SafeMath128,
    yield_generator: EnhancedYieldGenerator,
    risk_metrics: RiskMetrics,
    portfolio_metrics: PortfolioMetrics,
}

/// Risk assessment metrics for yield generation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskMetrics {
    /// Volatility of yields (standard deviation)
    pub volatility: i128,
    /// Maximum drawdown from peak to trough
    pub max_drawdown: i128,
    /// Sharpe ratio approximation (risk-adjusted returns)
    pub sharpe_ratio: i128,
    /// Value at Risk (95% confidence level)
    pub var_95: i128,
    /// Beta coefficient relative to market
    pub beta: i128,
}

/// Portfolio-level metrics for optimization
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioMetrics {
    /// Total portfolio value
    pub total_value: i128,
    /// Weighted average yield rate
    pub weighted_yield_rate: i128,
    /// Portfolio diversity score (0-10000 BPS)
    pub diversity_score: i128,
    /// Concentration risk (0-10000 BPS)
    pub concentration_risk: i128,
    /// Efficiency score (gas per yield)
    pub efficiency_score: i128,
}

/// Advanced yield calculation result with risk analysis
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvancedYieldResult {
    /// Basic yield amount
    pub yield_amount: i128,
    /// Risk-adjusted yield (after risk factors)
    pub risk_adjusted_yield: i128,
    /// Confidence level in calculation (0-10000 BPS)
    pub confidence_level: i128,
    /// Expected precision loss
    pub expected_precision_loss: i128,
    /// Recommended optimization actions
    pub recommendations: Vec<String>,
}

impl AdvancedYieldManager {
    /// Create new advanced yield manager with default configuration
    pub fn new() -> Self {
        Self {
            safe_math: SafeMath128::new(),
            yield_generator: EnhancedYieldGenerator::new(),
            risk_metrics: RiskMetrics::default(),
            portfolio_metrics: PortfolioMetrics::default(),
        }
    }

    /// Calculate advanced yield with comprehensive risk assessment
    pub fn calculate_advanced_yield(
        &mut self,
        lease: &LeaseInstance,
        historical_yields: Option<Vec<i128>>,
        market_data: Option<MarketData>,
    ) -> Result<AdvancedYieldResult, LeaseError> {
        // Step 1: Calculate base yield using enhanced generator
        let base_yield = self.yield_generator.calculate_complex_yield(
            lease.security_deposit,
            self.get_elapsed_time(lease),
            self.get_total_lock_period(lease),
            self.get_deposit_tier(lease.security_deposit),
            lease.start_date,
        ).map_err(|_| LeaseError::InvalidDeduction)?;

        // Step 2: Apply risk adjustments
        let risk_adjusted_yield = self.apply_risk_adjustments(base_yield, historical_yields, market_data)?;

        // Step 3: Calculate confidence level
        let confidence_level = self.calculate_confidence_level(&risk_adjusted_yield, historical_yields)?;

        // Step 4: Get precision expectations
        let precision_report = self.safe_math.get_detailed_precision_report();
        let expected_precision_loss = precision_report.cumulative_precision_loss;

        // Step 5: Generate recommendations
        let recommendations = self.generate_yield_recommendations(&risk_adjusted_yield, &precision_report);

        Ok(AdvancedYieldResult {
            yield_amount: base_yield,
            risk_adjusted_yield,
            confidence_level,
            expected_precision_loss,
            recommendations,
        })
    }

    /// Calculate portfolio-level yield optimization
    pub fn optimize_portfolio_yield(
        &mut self,
        leases: &[LeaseInstance],
        target_risk_level: u32, // Risk tolerance in BPS
    ) -> Result<PortfolioOptimizationResult, LeaseError> {
        // Step 1: Calculate individual yields for all leases
        let mut individual_yields = Vec::new();
        let mut total_principal = 0i128;

        for lease in leases {
            let yield_result = self.calculate_advanced_yield(lease, None, None)?;
            individual_yields.push((lease.security_deposit, yield_result.risk_adjusted_yield));
            total_principal = total_principal.checked_add(lease.security_deposit)
                .ok_or(LeaseError::InvalidDeduction)?;
        }

        // Step 2: Calculate portfolio metrics
        self.update_portfolio_metrics(&individual_yields, total_principal)?;

        // Step 3: Optimize based on risk tolerance
        let optimization_strategy = self.determine_optimization_strategy(target_risk_level);
        let optimized_allocation = self.optimize_allocation(&individual_yields, optimization_strategy)?;

        // Step 4: Calculate expected portfolio performance
        let expected_performance = self.calculate_expected_portfolio_performance(&optimized_allocation)?;

        Ok(PortfolioOptimizationResult {
            current_allocation: individual_yields,
            optimized_allocation,
            expected_performance,
            risk_metrics: self.risk_metrics.clone(),
            portfolio_metrics: self.portfolio_metrics.clone(),
        })
    }

    /// Perform stress testing on yield calculations
    pub fn stress_test_yield_scenarios(
        &mut self,
        base_lease: &LeaseInstance,
        stress_scenarios: &[StressScenario],
    ) -> Result<StressTestResults, LeaseError> {
        let mut scenario_results = Vec::new();

        for scenario in stress_scenarios {
            // Apply stress factors to lease parameters
            let stressed_lease = self.apply_stress_factors(base_lease, scenario);

            // Calculate yield under stress conditions
            let yield_result = self.calculate_advanced_yield(&stressed_lease, None, None)?;

            // Calculate stress impact
            let baseline_yield = self.calculate_advanced_yield(base_lease, None, None)?;
            let yield_impact = self.safe_math.safe_percentage_change(
                baseline_yield.risk_adjusted_yield,
                yield_result.risk_adjusted_yield,
            ).map_err(|_| LeaseError::InvalidDeduction)?;

            scenario_results.push(StressScenarioResult {
                scenario: scenario.clone(),
                stressed_yield: yield_result,
                yield_impact_bps: yield_impact,
                survivable: yield_result.risk_adjusted_yield > 0,
            });
        }

        Ok(StressTestResults {
            scenario_results,
            worst_case_scenario: self.find_worst_case_scenario(&scenario_results),
            stress_resilience_score: self.calculate_stress_resilience(&scenario_results),
        })
    }

    /// Generate comprehensive yield report
    pub fn generate_yield_report(&mut self, lease: &LeaseInstance) -> YieldReport {
        let precision_report = self.safe_math.get_detailed_precision_report();
        let yield_result = self.calculate_advanced_yield(lease, None, None).unwrap_or_else(|_| {
            AdvancedYieldResult {
                yield_amount: 0,
                risk_adjusted_yield: 0,
                confidence_level: 0,
                expected_precision_loss: 0,
                recommendations: vec!["Error in yield calculation".to_string()],
            }
        });

        YieldReport {
            lease_id: 0, // Would be filled with actual lease ID
            security_deposit: lease.security_deposit,
            base_yield: yield_result.yield_amount,
            risk_adjusted_yield: yield_result.risk_adjusted_yield,
            confidence_level: yield_result.confidence_level,
            precision_quality: precision_report.precision_quality,
            precision_loss: precision_report.cumulative_precision_loss,
            efficiency_score: precision_report.efficiency_score,
            recommendations: yield_result.recommendations,
            risk_metrics: self.risk_metrics.clone(),
            timestamp: 0, // Would be filled with actual timestamp
        }
    }

    // Private helper methods

    fn apply_risk_adjustments(
        &mut self,
        base_yield: i128,
        historical_yields: Option<Vec<i128>>,
        market_data: Option<MarketData>,
    ) -> Result<i128, LeaseError> {
        let mut adjusted_yield = base_yield;

        // Apply volatility adjustment
        if let Some(yields) = historical_yields {
            if yields.len() >= 2 {
                let volatility = self.safe_math.safe_volatility_calculation(&yields)
                    .map_err(|_| LeaseError::InvalidDeduction)?;
                
                // Reduce yield based on volatility (higher volatility = lower confidence)
                let volatility_adjustment = self.safe_math.safe_bps_division_floor(
                    adjusted_yield,
                    (volatility.min(5000) as u32) // Cap at 50% reduction
                ).map_err(|_| LeaseError::InvalidDeduction)?;
                
                adjusted_yield = adjusted_yield.checked_sub(volatility_adjustment)
                    .ok_or(LeaseError::InvalidDeduction)?;
            }
        }

        // Apply market correlation adjustment
        if let Some(market) = market_data {
            let market_adjustment = self.safe_math.safe_bps_division_floor(
                adjusted_yield,
                (market.correlation_risk.min(3000) as u32) // Cap at 30% reduction
            ).map_err(|_| LeaseError::InvalidDeduction)?;
            
            adjusted_yield = adjusted_yield.checked_sub(market_adjustment)
                .ok_or(LeaseError::InvalidDeduction)?;
        }

        Ok(adjusted_yield)
    }

    fn calculate_confidence_level(
        &mut self,
        yield_result: &AdvancedYieldResult,
        historical_yields: Option<Vec<i128>>,
    ) -> Result<i128, LeaseError> {
        let mut confidence = 8000; // Base 80% confidence

        // Adjust based on precision quality
        let precision_report = self.safe_math.get_detailed_precision_report();
        match precision_report.precision_quality {
            PrecisionQuality::Perfect => confidence += 1500,
            PrecisionQuality::Excellent => confidence += 1000,
            PrecisionQuality::Good => confidence += 500,
            PrecisionQuality::Fair => confidence -= 500,
            PrecisionQuality::Poor => confidence -= 1000,
            PrecisionQuality::Critical => confidence -= 2000,
        }

        // Adjust based on historical data availability
        if let Some(yields) = historical_yields {
            if yields.len() >= 10 {
                confidence += 500; // More data = higher confidence
            } else if yields.len() < 3 {
                confidence -= 1000; // Less data = lower confidence
            }
        } else {
            confidence -= 1500; // No historical data = lower confidence
        }

        // Ensure confidence is within valid range
        confidence = confidence.max(1000).min(10000); // 10% to 100%

        Ok(confidence)
    }

    fn generate_yield_recommendations(
        &self,
        yield_result: &AdvancedYieldResult,
        precision_report: &DetailedPrecisionReport,
    ) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Precision-based recommendations
        match precision_report.precision_quality {
            PrecisionQuality::Critical => {
                recommendations.push("CRITICAL: High precision loss detected. Consider using higher precision arithmetic.".to_string());
            }
            PrecisionQuality::Poor => {
                recommendations.push("Consider implementing batch processing to reduce cumulative precision loss.".to_string());
            }
            _ => {
                recommendations.push("Precision levels are acceptable.".to_string());
            }
        }

        // Yield-based recommendations
        if yield_result.confidence_level < 5000 {
            recommendations.push("Low confidence in yield calculation. Consider additional risk mitigation.".to_string());
        }

        if yield_result.expected_precision_loss > 1000 {
            recommendations.push("High precision loss expected. Monitor calculations closely.".to_string());
        }

        // Efficiency recommendations
        if precision_report.efficiency_score < 70 {
            recommendations.push("Low efficiency detected. Consider optimizing calculation patterns.".to_string());
        }

        recommendations
    }

    fn get_elapsed_time(&self, lease: &LeaseInstance) -> u64 {
        // In a real implementation, this would calculate actual elapsed time
        // For now, return a reasonable default
        86400 * 30 // 30 days
    }

    fn get_total_lock_period(&self, lease: &LeaseInstance) -> u64 {
        // In a real implementation, this would calculate actual lock period
        86400 * 365 // 1 year
    }

    fn get_deposit_tier(&self, deposit_amount: i128) -> u32 {
        // Determine deposit tier based on amount
        match deposit_amount {
            0..=99_999 => 1,
            100_000..=999_999 => 2,
            1_000_000..=9_999_999 => 3,
            _ => 4,
        }
    }

    fn update_portfolio_metrics(
        &mut self,
        individual_yields: &[(i128, i128)],
        total_principal: i128,
    ) -> Result<(), LeaseError> {
        // Calculate weighted average yield rate
        let weighted_yield = self.safe_math.safe_weighted_average(individual_yields)
            .map_err(|_| LeaseError::InvalidDeduction)?;

        // Calculate diversity score (based on distribution of yields)
        let diversity_score = self.calculate_diversity_score(individual_yields)?;

        // Calculate concentration risk
        let concentration_risk = self.calculate_concentration_risk(individual_yields, total_principal)?;

        self.portfolio_metrics = PortfolioMetrics {
            total_value: total_principal,
            weighted_yield_rate: weighted_yield,
            diversity_score,
            concentration_risk,
            efficiency_score: 8000, // Default efficiency
        };

        Ok(())
    }

    fn calculate_diversity_score(&mut self, yields: &[(i128, i128)]) -> Result<i128, LeaseError> {
        if yields.len() <= 1 {
            return Ok(0); // No diversity with single yield
        }

        // Calculate coefficient of variation as diversity measure
        let yield_values: Vec<i128> = yields.iter().map(|(_, yield_amount)| *yield_amount).collect();
        let mean_yield = self.safe_math.safe_weighted_average(
            &yield_values.iter().map(|&y| (y, 1)).collect::<Vec<_>>()
        ).map_err(|_| LeaseError::InvalidDeduction)?;

        let volatility = self.safe_math.safe_volatility_calculation(&yield_values)
            .map_err(|_| LeaseError::InvalidDeduction)?;

        // Lower coefficient of variation = higher diversity score
        let cv_ratio = if mean_yield > 0 {
            volatility.checked_mul(10000).and_then(|x| x.checked_div(mean_yield))
                .ok_or(LeaseError::InvalidDeduction)?
        } else {
            10000 // Maximum risk
        };

        // Convert to diversity score (inverse of CV ratio)
        let diversity_score = 10000.checked_sub(cv_ratio.min(10000))
            .ok_or(LeaseError::InvalidDeduction)?;

        Ok(diversity_score)
    }

    fn calculate_concentration_risk(&mut self, yields: &[(i128, i128)], total: i128) -> Result<i128, LeaseError> {
        // Calculate Herfindahl-Hirschman Index for concentration
        let mut hhi = 0i128;

        for &(principal, _) in yields {
            if total > 0 {
                let market_share = principal.checked_mul(10000)
                    .and_then(|x| x.checked_div(total))
                    .ok_or(LeaseError::InvalidDeduction)?;
                
                let share_squared = market_share.checked_mul(market_share)
                    .ok_or(LeaseError::InvalidDeduction)?;
                
                hhi = hhi.checked_add(share_squared)
                    .ok_or(LeaseError::InvalidDeduction)?;
            }
        }

        // Normalize HHI to 0-10000 BPS scale
        let concentration_risk = hhi.checked_div(10000)
            .ok_or(LeaseError::InvalidDeduction)?;

        Ok(concentration_risk.min(10000))
    }

    fn determine_optimization_strategy(&self, target_risk_level: u32) -> OptimizationStrategy {
        match target_risk_level {
            0..=2000 => OptimizationStrategy::Conservative,
            2001..=5000 => OptimizationStrategy::Balanced,
            5001..=8000 => OptimizationStrategy::Aggressive,
            _ => OptimizationStrategy::Speculative,
        }
    }

    fn optimize_allocation(
        &mut self,
        current_allocation: &[(i128, i128)],
        strategy: OptimizationStrategy,
    ) -> Result<Vec<(i128, i128)>, LeaseError> {
        // For now, return current allocation (real implementation would optimize)
        Ok(current_allocation.to_vec())
    }

    fn calculate_expected_portfolio_performance(
        &mut self,
        allocation: &[(i128, i128)],
    ) -> Result<ExpectedPerformance, LeaseError> {
        let total_expected_yield: i128 = allocation.iter()
            .try_fold(0, |acc, (_, yield_amount)| {
                acc.checked_add(*yield_amount)
            })
            .ok_or(LeaseError::InvalidDeduction)?;

        let weighted_risk = self.calculate_portfolio_risk(allocation)?;

        Ok(ExpectedPerformance {
            expected_yield: total_expected_yield,
            expected_risk: weighted_risk,
            risk_adjusted_return: self.calculate_risk_adjusted_return(total_expected_yield, weighted_risk)?,
        })
    }

    fn calculate_portfolio_risk(&mut self, allocation: &[(i128, i128)]) -> Result<i128, LeaseError> {
        // Simplified risk calculation (real implementation would be more sophisticated)
        let total_principal: i128 = allocation.iter()
            .try_fold(0, |acc, (principal, _)| acc.checked_add(*principal))
            .ok_or(LeaseError::InvalidDeduction)?;

        if total_principal == 0 {
            return Ok(0);
        }

        let weighted_risk = allocation.iter()
            .try_fold(0, |acc, (principal, yield_amount)| {
                let risk_factor = yield_amount.checked_div(1000) // Simplified risk factor
                    .unwrap_or(0);
                let weighted_risk = principal.checked_mul(risk_factor)
                    .unwrap_or(0);
                acc.checked_add(weighted_risk)
            })
            .ok_or(LeaseError::InvalidDeduction)?;

        weighted_risk.checked_div(total_principal)
            .ok_or(LeaseError::InvalidDeduction)
    }

    fn calculate_risk_adjusted_return(&mut self, expected_yield: i128, risk: i128) -> Result<i128, LeaseError> {
        if risk == 0 {
            return Ok(expected_yield);
        }

        // Sharpe ratio approximation: expected_yield / risk
        expected_yield.checked_mul(10000).and_then(|x| x.checked_div(risk))
            .ok_or(LeaseError::InvalidDeduction)
    }

    fn apply_stress_factors(&self, lease: &LeaseInstance, scenario: &StressScenario) -> LeaseInstance {
        let mut stressed_lease = lease.clone();

        // Apply market stress factor
        stressed_lease.security_deposit = lease.security_deposit
            .checked_mul(10000 - scenario.market_stress_bps as i128)
            .map(|x| x / 10000)
            .unwrap_or(lease.security_deposit);

        stressed_lease
    }

    fn find_worst_case_scenario(&self, results: &[StressScenarioResult]) -> Option<&StressScenarioResult> {
        results.iter().min_by_key(|r| r.stressed_yield.risk_adjusted_yield)
    }

    fn calculate_stress_resilience(&self, results: &[StressScenarioResult]) -> i128 {
        let surviving_scenarios = results.iter().filter(|r| r.survivable).count();
        let total_scenarios = results.len();

        if total_scenarios == 0 {
            return 0;
        }

        (surviving_scenarios as i128 * 10000) / total_scenarios as i128
    }
}

// Supporting data structures

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketData {
    pub correlation_risk: i128,
    pub market_volatility: i128,
    pub risk_free_rate: i128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StressScenario {
    pub name: String,
    pub market_stress_bps: u32,
    pub liquidity_stress_bps: u32,
    pub duration_factor: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StressScenarioResult {
    pub scenario: StressScenario,
    pub stressed_yield: AdvancedYieldResult,
    pub yield_impact_bps: i128,
    pub survivable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StressTestResults {
    pub scenario_results: Vec<StressScenarioResult>,
    pub worst_case_scenario: Option<&StressScenarioResult>,
    pub stress_resilience_score: i128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationStrategy {
    Conservative,
    Balanced,
    Aggressive,
    Speculative,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioOptimizationResult {
    pub current_allocation: Vec<(i128, i128)>,
    pub optimized_allocation: Vec<(i128, i128)>,
    pub expected_performance: ExpectedPerformance,
    pub risk_metrics: RiskMetrics,
    pub portfolio_metrics: PortfolioMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedPerformance {
    pub expected_yield: i128,
    pub expected_risk: i128,
    pub risk_adjusted_return: i128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YieldReport {
    pub lease_id: u64,
    pub security_deposit: i128,
    pub base_yield: i128,
    pub risk_adjusted_yield: i128,
    pub confidence_level: i128,
    pub precision_quality: PrecisionQuality,
    pub precision_loss: i128,
    pub efficiency_score: u32,
    pub recommendations: Vec<String>,
    pub risk_metrics: RiskMetrics,
    pub timestamp: u64,
}

impl Default for RiskMetrics {
    fn default() -> Self {
        Self {
            volatility: 0,
            max_drawdown: 0,
            sharpe_ratio: 0,
            var_95: 0,
            beta: 0,
        }
    }
}

impl Default for PortfolioMetrics {
    fn default() -> Self {
        Self {
            total_value: 0,
            weighted_yield_rate: 0,
            diversity_score: 0,
            concentration_risk: 0,
            efficiency_score: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_advanced_yield_manager_creation() {
        let manager = AdvancedYieldManager::new();
        assert_eq!(manager.portfolio_metrics.total_value, 0);
        assert_eq!(manager.risk_metrics.volatility, 0);
    }

    #[test]
    fn test_diversity_score_calculation() {
        let mut manager = AdvancedYieldManager::new();
        
        // Test with diverse yields
        let diverse_yields = vec![(1000, 100), (2000, 150), (3000, 200)];
        let diversity = manager.calculate_diversity_score(&diverse_yields).unwrap();
        assert!(diversity > 0 && diversity <= 10000);
        
        // Test with identical yields (no diversity)
        let identical_yields = vec![(1000, 100), (2000, 100), (3000, 100)];
        let diversity = manager.calculate_diversity_score(&identical_yields).unwrap();
        assert!(diversity < 5000); // Should be low diversity
    }

    #[test]
    fn test_concentration_risk_calculation() {
        let mut manager = AdvancedYieldManager::new();
        
        // Test with concentrated portfolio
        let concentrated = vec![(9000, 100), (500, 50), (500, 25)]; // 90% in one position
        let risk = manager.calculate_concentration_risk(&concentrated, 10000).unwrap();
        assert!(risk > 8000); // High concentration risk
        
        // Test with diversified portfolio
        let diversified = vec![(3333, 100), (3333, 100), (3334, 100)]; // Even distribution
        let risk = manager.calculate_concentration_risk(&diversified, 10000).unwrap();
        assert!(risk < 4000); // Low concentration risk
    }
}
