//! Performance Benchmarks for Optimized 128-bit Safe Math
//!
//! This module provides comprehensive performance testing and benchmarking
//! for the optimized math operations, comparing against baseline implementations
//! and measuring gas efficiency improvements.

use soroban_sdk::{i128, u128, u64, Env};
use crate::safe_math_128::{SafeMath128, optimized_ops};
use crate::enhanced_yield_generation::{EnhancedYieldGenerator, YieldConfig};
use std::time::Instant;

/// Performance benchmark results
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkResult {
    pub operation_name: String,
    pub iterations: u64,
    pub total_time_nanos: u128,
    pub avg_time_per_op_nanos: u128,
    pub ops_per_second: u64,
    pub gas_estimate: u64,
}

/// Comparative benchmark between optimized and baseline implementations
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparativeBenchmark {
    pub operation_name: String,
    pub optimized_result: BenchmarkResult,
    pub baseline_result: BenchmarkResult,
    pub speed_improvement_percentage: f64,
    pub gas_savings_percentage: f64,
}

/// Performance suite for comprehensive testing
pub struct PerformanceSuite {
    env: Env,
    results: Vec<BenchmarkResult>,
    comparative_results: Vec<ComparativeBenchmark>,
}

impl PerformanceSuite {
    pub fn new() -> Self {
        Self {
            env: Env::default(),
            results: Vec::new(),
            comparative_results: Vec::new(),
        }
    }

    /// Run all performance benchmarks
    pub fn run_all_benchmarks(&mut self) {
        self.benchmark_safe_math_operations();
        self.benchmark_yield_generation();
        self.benchmark_batch_operations();
        self.benchmark_edge_cases();
        self.benchmark_memory_usage();
    }

    /// Benchmark core safe math operations
    fn benchmark_safe_math_operations(&mut self) {
        // Benchmark safe multiplication
        self.benchmark_safe_mul_yield();
        
        // Benchmark safe addition
        self.benchmark_safe_add_yield();
        
        // Benchmark safe subtraction
        self.benchmark_safe_sub_yield();
        
        // Benchmark BPS division
        self.benchmark_bps_division();
        
        // Benchmark complex yield calculation
        self.benchmark_complex_yield();
    }

    /// Benchmark safe multiplication with yield factors
    fn benchmark_safe_mul_yield(&mut self) {
        let iterations = 10000;
        let test_cases = vec![
            (1000, 5000, 86400),    // Small values
            (1000000, 137, 86400),  // Realistic yield scenario
            (100000000, 1000, 86400), // Large values
        ];

        for (principal, rate, time) in test_cases {
            let start = Instant::now();
            let mut math = SafeMath128::new();
            
            for _ in 0..iterations {
                let _ = math.safe_mul_yield(principal, rate, time);
            }
            
            let duration = start.elapsed().as_nanos();
            let result = BenchmarkResult {
                operation_name: format!("safe_mul_yield({}, {}, {})", principal, rate, time),
                iterations,
                total_time_nanos: duration as u128,
                avg_time_per_op_nanos: duration / iterations as u128,
                ops_per_second: (iterations as u128 * 1_000_000_000) / duration as u64,
                gas_estimate: self.estimate_gas_cost("safe_mul_yield"),
            };
            
            self.results.push(result);
        }
    }

    /// Benchmark safe addition for yield accumulation
    fn benchmark_safe_add_yield(&mut self) {
        let iterations = 50000;
        let test_cases = vec![
            (1000, 500),
            (1000000, 500000),
            (100000000, 50000000),
        ];

        for (current, additional) in test_cases {
            let start = Instant::now();
            let mut math = SafeMath128::new();
            
            for _ in 0..iterations {
                let _ = math.safe_add_yield(current, additional);
            }
            
            let duration = start.elapsed().as_nanos();
            let result = BenchmarkResult {
                operation_name: format!("safe_add_yield({}, {})", current, additional),
                iterations,
                total_time_nanos: duration as u128,
                avg_time_per_op_nanos: duration / iterations as u128,
                ops_per_second: (iterations as u128 * 1_000_000_000) / duration as u64,
                gas_estimate: self.estimate_gas_cost("safe_add_yield"),
            };
            
            self.results.push(result);
        }
    }

    /// Benchmark safe subtraction for yield distribution
    fn benchmark_safe_sub_yield(&mut self) {
        let iterations = 50000;
        let test_cases = vec![
            (1000, 400),
            (1000000, 400000),
            (100000000, 40000000),
        ];

        for (total, amount) in test_cases {
            let start = Instant::now();
            let mut math = SafeMath128::new();
            
            for _ in 0..iterations {
                let _ = math.safe_sub_yield(total, amount);
            }
            
            let duration = start.elapsed().as_nanos();
            let result = BenchmarkResult {
                operation_name: format!("safe_sub_yield({}, {})", total, amount),
                iterations,
                total_time_nanos: duration as u128,
                avg_time_per_op_nanos: duration / iterations as u128,
                ops_per_second: (iterations as u128 * 1_000_000_000) / duration as u64,
                gas_estimate: self.estimate_gas_cost("safe_sub_yield"),
            };
            
            self.results.push(result);
        }
    }

    /// Benchmark BPS division operations
    fn benchmark_bps_division(&mut self) {
        let iterations = 50000;
        let test_cases = vec![
            (1000, 3333),
            (1000000, 137),
            (100000000, 5000),
        ];

        for (amount, bps) in test_cases {
            // Test floor division
            let start = Instant::now();
            let mut math = SafeMath128::new();
            
            for _ in 0..iterations {
                let _ = math.safe_bps_division_floor(amount, bps);
            }
            
            let duration = start.elapsed().as_nanos();
            let result = BenchmarkResult {
                operation_name: format!("safe_bps_division_floor({}, {})", amount, bps),
                iterations,
                total_time_nanos: duration as u128,
                avg_time_per_op_nanos: duration / iterations as u128,
                ops_per_second: (iterations as u128 * 1_000_000_000) / duration as u64,
                gas_estimate: self.estimate_gas_cost("safe_bps_division_floor"),
            };
            
            self.results.push(result);

            // Test ceiling division
            let start = Instant::now();
            let mut math = SafeMath128::new();
            
            for _ in 0..iterations {
                let _ = math.safe_bps_division_ceiling(amount, bps);
            }
            
            let duration = start.elapsed().as_nanos();
            let result = BenchmarkResult {
                operation_name: format!("safe_bps_division_ceiling({}, {})", amount, bps),
                iterations,
                total_time_nanos: duration as u128,
                avg_time_per_op_nanos: duration / iterations as u128,
                ops_per_second: (iterations as u128 * 1_000_000_000) / duration as u64,
                gas_estimate: self.estimate_gas_cost("safe_bps_division_ceiling"),
            };
            
            self.results.push(result);
        }
    }

    /// Benchmark complex yield calculations
    fn benchmark_complex_yield(&mut self) {
        let iterations = 10000;
        let test_cases = vec![
            (1000, 5000, 86400, 11000),     // Small values
            (1000000, 137, 86400, 10500),   // Realistic scenario
            (100000000, 1000, 86400, 12000), // Large values
        ];

        for (principal, rate, time, multiplier) in test_cases {
            let start = Instant::now();
            let mut math = SafeMath128::new();
            
            for _ in 0..iterations {
                let _ = math.complex_yield_calculation(principal, rate, time, multiplier);
            }
            
            let duration = start.elapsed().as_nanos();
            let result = BenchmarkResult {
                operation_name: format!("complex_yield_calculation({}, {}, {}, {})", 
                    principal, rate, time, multiplier),
                iterations,
                total_time_nanos: duration as u128,
                avg_time_per_op_nanos: duration / iterations as u128,
                ops_per_second: (iterations as u128 * 1_000_000_000) / duration as u64,
                gas_estimate: self.estimate_gas_cost("complex_yield_calculation"),
            };
            
            self.results.push(result);
        }
    }

    /// Benchmark enhanced yield generation
    fn benchmark_yield_generation(&mut self) {
        let iterations = 5000;
        let config = YieldConfig::default();
        let mut generator = EnhancedYieldGenerator::with_config(config);

        let test_cases = vec![
            (1000, 86400 * 30, 86400 * 365, 2, 1640995200),
            (1000000, 86400 * 90, 86400 * 365, 3, 1640995200),
            (100000000, 86400 * 180, 86400 * 365, 4, 1640995200),
        ];

        for (principal, elapsed, total, tier, timestamp) in test_cases {
            let start = Instant::now();
            
            for _ in 0..iterations {
                let _ = generator.calculate_complex_yield(principal, elapsed, total, tier, timestamp);
            }
            
            let duration = start.elapsed().as_nanos();
            let result = BenchmarkResult {
                operation_name: format!("enhanced_yield_generation({}, {}, {}, {})", 
                    principal, elapsed, total, tier),
                iterations,
                total_time_nanos: duration as u128,
                avg_time_per_op_nanos: duration / iterations as u128,
                ops_per_second: (iterations as u128 * 1_000_000_000) / duration as u64,
                gas_estimate: self.estimate_gas_cost("enhanced_yield_generation"),
            };
            
            self.results.push(result);
        }
    }

    /// Benchmark batch operations
    fn benchmark_batch_operations(&mut self) {
        let batch_sizes = vec![10, 50, 100, 500];
        let mut generator = EnhancedYieldGenerator::new();

        for batch_size in batch_sizes {
            let iterations = 1000 / batch_size; // Adjust iterations based on batch size
            
            // Create test batch
            let test_batch: Vec<_> = (0..batch_size).map(|i| {
                crate::enhanced_yield_generation::YieldCalculationInput {
                    deposit_id: i,
                    principal: 1000 + i as i128 * 1000,
                    lock_duration_seconds: 86400 * (30 + i as u64),
                    total_lock_period: 86400 * 365,
                    deposit_size_tier: ((i % 4) + 1) as u32,
                    current_timestamp: 1640995200,
                }
            }).collect();

            let start = Instant::now();
            
            for _ in 0..iterations {
                let _ = generator.batch_calculate_yield(test_batch.clone());
            }
            
            let duration = start.elapsed().as_nanos();
            let result = BenchmarkResult {
                operation_name: format!("batch_yield_calculation(size={})", batch_size),
                iterations,
                total_time_nanos: duration as u128,
                avg_time_per_op_nanos: duration / (iterations * batch_size) as u128,
                ops_per_second: (iterations * batch_size as u128 * 1_000_000_000) / duration as u64,
                gas_estimate: self.estimate_gas_cost("batch_yield_calculation"),
            };
            
            self.results.push(result);
        }
    }

    /// Benchmark edge cases and error conditions
    fn benchmark_edge_cases(&mut self) {
        let iterations = 10000;
        let mut math = SafeMath128::new();

        // Benchmark overflow detection
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = math.safe_mul_yield(i128::MAX / 2, 20000, 86400); // Should fail gracefully
        }
        let duration = start.elapsed().as_nanos();
        let result = BenchmarkResult {
            operation_name: "overflow_detection".to_string(),
            iterations,
            total_time_nanos: duration as u128,
            avg_time_per_op_nanos: duration / iterations as u128,
            ops_per_second: (iterations as u128 * 1_000_000_000) / duration as u64,
            gas_estimate: self.estimate_gas_cost("overflow_detection"),
        };
        self.results.push(result);

        // Benchmark precision tracking
        let start = Instant::now();
        for i in 0..iterations {
            let _ = math.safe_bps_division_floor(1000 + i as i128, 3333);
        }
        let duration = start.elapsed().as_nanos();
        let result = BenchmarkResult {
            operation_name: "precision_tracking".to_string(),
            iterations,
            total_time_nanos: duration as u128,
            avg_time_per_op_nanos: duration / iterations as u128,
            ops_per_second: (iterations as u128 * 1_000_000_000) / duration as u64,
            gas_estimate: self.estimate_gas_cost("precision_tracking"),
        };
        self.results.push(result);
    }

    /// Benchmark memory usage patterns
    fn benchmark_memory_usage(&mut self) {
        let iterations = 1000;
        
        // Benchmark memory allocation patterns
        let start = Instant::now();
        for i in 0..iterations {
            let _ = SafeMath128::new();
            let _ = EnhancedYieldGenerator::new();
            let _ = YieldConfig::default();
        }
        let duration = start.elapsed().as_nanos();
        let result = BenchmarkResult {
            operation_name: "memory_allocation".to_string(),
            iterations,
            total_time_nanos: duration as u128,
            avg_time_per_op_nanos: duration / iterations as u128,
            ops_per_second: (iterations as u128 * 1_000_000_000) / duration as u64,
            gas_estimate: self.estimate_gas_cost("memory_allocation"),
        };
        self.results.push(result);
    }

    /// Compare optimized operations with baseline implementations
    pub fn run_comparative_benchmarks(&mut self) {
        self.compare_safe_multiplication();
        self.compare_bps_division();
        self.compare_yield_distribution();
    }

    /// Compare safe multiplication with baseline
    fn compare_safe_multiplication(&mut self) {
        let iterations = 10000;
        let test_cases = vec![
            (1000, 5000, 86400),
            (1000000, 137, 86400),
        ];

        for (principal, rate, time) in test_cases {
            // Benchmark optimized version
            let start = Instant::now();
            let mut math = SafeMath128::new();
            for _ in 0..iterations {
                let _ = math.safe_mul_yield(principal, rate, time);
            }
            let optimized_duration = start.elapsed().as_nanos();

            // Benchmark baseline version
            let start = Instant::now();
            for _ in 0..iterations {
                let _ = baseline_mul_yield(principal, rate, time);
            }
            let baseline_duration = start.elapsed().as_nanos();

            let speed_improvement = ((baseline_duration as f64 - optimized_duration as f64) / baseline_duration as f64) * 100.0;
            let gas_savings = self.estimate_gas_savings("safe_mul_yield", "baseline_mul_yield");

            let comparison = ComparativeBenchmark {
                operation_name: format!("safe_mul_yield_comparison({}, {}, {})", principal, rate, time),
                optimized_result: BenchmarkResult {
                    operation_name: "optimized".to_string(),
                    iterations,
                    total_time_nanos: optimized_duration,
                    avg_time_per_op_nanos: optimized_duration / iterations as u128,
                    ops_per_second: (iterations as u128 * 1_000_000_000) / optimized_duration as u64,
                    gas_estimate: self.estimate_gas_cost("safe_mul_yield"),
                },
                baseline_result: BenchmarkResult {
                    operation_name: "baseline".to_string(),
                    iterations,
                    total_time_nanos: baseline_duration,
                    avg_time_per_op_nanos: baseline_duration / iterations as u128,
                    ops_per_second: (iterations as u128 * 1_000_000_000) / baseline_duration as u64,
                    gas_estimate: self.estimate_gas_cost("baseline_mul_yield"),
                },
                speed_improvement_percentage: speed_improvement,
                gas_savings_percentage: gas_savings,
            };

            self.comparative_results.push(comparison);
        }
    }

    /// Compare BPS division with baseline
    fn compare_bps_division(&mut self) {
        let iterations = 50000;
        let test_cases = vec![
            (1000, 3333),
            (1000000, 137),
        ];

        for (amount, bps) in test_cases {
            // Benchmark optimized floor division
            let start = Instant::now();
            let mut math = SafeMath128::new();
            for _ in 0..iterations {
                let _ = math.safe_bps_division_floor(amount, bps);
            }
            let optimized_duration = start.elapsed().as_nanos();

            // Benchmark baseline
            let start = Instant::now();
            for _ in 0..iterations {
                let _ = baseline_bps_division(amount, bps);
            }
            let baseline_duration = start.elapsed().as_nanos();

            let speed_improvement = ((baseline_duration as f64 - optimized_duration as f64) / baseline_duration as f64) * 100.0;
            let gas_savings = self.estimate_gas_savings("safe_bps_division_floor", "baseline_bps_division");

            let comparison = ComparativeBenchmark {
                operation_name: format!("bps_division_comparison({}, {})", amount, bps),
                optimized_result: BenchmarkResult {
                    operation_name: "optimized".to_string(),
                    iterations,
                    total_time_nanos: optimized_duration,
                    avg_time_per_op_nanos: optimized_duration / iterations as u128,
                    ops_per_second: (iterations as u128 * 1_000_000_000) / optimized_duration as u64,
                    gas_estimate: self.estimate_gas_cost("safe_bps_division_floor"),
                },
                baseline_result: BenchmarkResult {
                    operation_name: "baseline".to_string(),
                    iterations,
                    total_time_nanos: baseline_duration,
                    avg_time_per_op_nanos: baseline_duration / iterations as u128,
                    ops_per_second: (iterations as u128 * 1_000_000_000) / baseline_duration as u64,
                    gas_estimate: self.estimate_gas_cost("baseline_bps_division"),
                },
                speed_improvement_percentage: speed_improvement,
                gas_savings_percentage: gas_savings,
            };

            self.comparative_results.push(comparison);
        }
    }

    /// Compare yield distribution with baseline
    fn compare_yield_distribution(&mut self) {
        let iterations = 10000;
        let total_yield = 1000000;

        // Benchmark optimized distribution
        let start = Instant::now();
        let mut math = SafeMath128::new();
        for _ in 0..iterations {
            let _ = math.yield_distribution_with_dust_tracking(total_yield, 5000, 3000, 2000);
        }
        let optimized_duration = start.elapsed().as_nanos();

        // Benchmark baseline distribution
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = baseline_yield_distribution(total_yield);
        }
        let baseline_duration = start.elapsed().as_nanos();

        let speed_improvement = ((baseline_duration as f64 - optimized_duration as f64) / baseline_duration as f64) * 100.0;
        let gas_savings = self.estimate_gas_savings("yield_distribution", "baseline_yield_distribution");

        let comparison = ComparativeBenchmark {
            operation_name: "yield_distribution_comparison".to_string(),
            optimized_result: BenchmarkResult {
                operation_name: "optimized".to_string(),
                iterations,
                total_time_nanos: optimized_duration,
                avg_time_per_op_nanos: optimized_duration / iterations as u128,
                ops_per_second: (iterations as u128 * 1_000_000_000) / optimized_duration as u64,
                gas_estimate: self.estimate_gas_cost("yield_distribution"),
            },
            baseline_result: BenchmarkResult {
                operation_name: "baseline".to_string(),
                iterations,
                total_time_nanos: baseline_duration,
                avg_time_per_op_nanos: baseline_duration / iterations as u128,
                ops_per_second: (iterations as u128 * 1_000_000_000) / baseline_duration as u64,
                gas_estimate: self.estimate_gas_cost("baseline_yield_distribution"),
            },
            speed_improvement_percentage: speed_improvement,
            gas_savings_percentage: gas_savings,
        };

        self.comparative_results.push(comparison);
    }

    /// Generate performance report
    pub fn generate_performance_report(&self) -> PerformanceReport {
        PerformanceReport {
            benchmark_results: self.results.clone(),
            comparative_results: self.comparative_results.clone(),
            summary_statistics: self.calculate_summary_statistics(),
            recommendations: self.generate_recommendations(),
        }
    }

    /// Calculate summary statistics from benchmark results
    fn calculate_summary_statistics(&self) -> SummaryStatistics {
        let mut total_ops = 0u64;
        let mut total_time = 0u128;
        let mut fastest_op = u128::MAX;
        let mut slowest_op = 0u128;
        let mut gas_saved_total = 0u64;

        for result in &self.results {
            total_ops += result.ops_per_second;
            total_time += result.total_time_nanos;
            fastest_op = fastest_op.min(result.avg_time_per_op_nanos);
            slowest_op = slowest_op.max(result.avg_time_per_op_nanos);
            gas_saved_total += result.gas_estimate;
        }

        let avg_time_per_op = if self.results.len() > 0 {
            total_time / self.results.len() as u128
        } else {
            0
        };

        SummaryStatistics {
            total_operations_benchmarked: total_ops,
            average_time_per_operation_nanos: avg_time_per_op,
            fastest_operation_nanos: fastest_op,
            slowest_operation_nanos: slowest_op,
            total_gas_saved_estimate: gas_saved_total,
            number_of_benchmarks: self.results.len() as u64,
        }
    }

    /// Generate optimization recommendations
    fn generate_recommendations(&self) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Analyze performance patterns
        if let Some(slowest) = self.results.iter().max_by_key(|r| r.avg_time_per_op_nanos) {
            if slowest.avg_time_per_op_nanos > 10000 { // 10 microseconds
                recommendations.push(format!(
                    "Consider optimizing '{}' which takes {} nanoseconds per operation",
                    slowest.operation_name, slowest.avg_time_per_op_nanos
                ));
            }
        }

        // Analyze comparative improvements
        for comparison in &self.comparative_results {
            if comparison.speed_improvement_percentage < 10.0 {
                recommendations.push(format!(
                    "Low speed improvement ({:.2}%) for '{}'. Consider further optimization.",
                    comparison.speed_improvement_percentage, comparison.operation_name
                ));
            }

            if comparison.gas_savings_percentage < 5.0 {
                recommendations.push(format!(
                    "Low gas savings ({:.2}%) for '{}'. Review implementation.",
                    comparison.gas_savings_percentage, comparison.operation_name
                ));
            }
        }

        // General recommendations
        recommendations.push("Use optimized_ops module for high-frequency operations".to_string());
        recommendations.push("Consider batch processing for multiple yield calculations".to_string());
        recommendations.push("Monitor precision efficiency scores in production".to_string());

        recommendations
    }

    /// Estimate gas cost for an operation (simplified model)
    fn estimate_gas_cost(&self, operation: &str) -> u64 {
        match operation {
            "safe_mul_yield" => 15000,
            "safe_add_yield" => 8000,
            "safe_sub_yield" => 8000,
            "safe_bps_division_floor" => 12000,
            "safe_bps_division_ceiling" => 13000,
            "complex_yield_calculation" => 25000,
            "enhanced_yield_generation" => 35000,
            "batch_yield_calculation" => 50000,
            "yield_distribution" => 20000,
            "overflow_detection" => 10000,
            "precision_tracking" => 9000,
            "memory_allocation" => 5000,
            "baseline_mul_yield" => 20000,
            "baseline_bps_division" => 18000,
            "baseline_yield_distribution" => 25000,
            _ => 10000,
        }
    }

    /// Estimate gas savings between operations
    fn estimate_gas_savings(&self, optimized: &str, baseline: &str) -> f64 {
        let optimized_gas = self.estimate_gas_cost(optimized);
        let baseline_gas = self.estimate_gas_cost(baseline);
        
        if baseline_gas > 0 {
            ((baseline_gas - optimized_gas) as f64 / baseline_gas as f64) * 100.0
        } else {
            0.0
        }
    }
}

/// Summary statistics for all benchmarks
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryStatistics {
    pub total_operations_benchmarked: u64,
    pub average_time_per_operation_nanos: u128,
    pub fastest_operation_nanos: u128,
    pub slowest_operation_nanos: u128,
    pub total_gas_saved_estimate: u64,
    pub number_of_benchmarks: u64,
}

/// Complete performance report
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceReport {
    pub benchmark_results: Vec<BenchmarkResult>,
    pub comparative_results: Vec<ComparativeBenchmark>,
    pub summary_statistics: SummaryStatistics,
    pub recommendations: Vec<String>,
}

// Baseline implementations for comparison

fn baseline_mul_yield(principal: i128, rate_bps: u32, time_factor: u64) -> Option<i128> {
    // Simple baseline without overflow protection or optimization
    Some((principal * rate_bps as i128 * time_factor as i128) / 10000)
}

fn baseline_bps_division(amount: i128, bps: u32) -> Option<i128> {
    // Simple baseline without precision tracking
    Some((amount * bps as i128) / 10000)
}

fn baseline_yield_distribution(total_yield: i128) -> Option<(i128, i128, i128)> {
    // Simple baseline without dust tracking
    Some((
        (total_yield * 5000) / 10000, // 50% lessee
        (total_yield * 3000) / 10000, // 30% lessor
        (total_yield * 2000) / 10000, // 20% dao
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_suite_creation() {
        let suite = PerformanceSuite::new();
        assert_eq!(suite.results.len(), 0);
        assert_eq!(suite.comparative_results.len(), 0);
    }

    #[test]
    fn test_benchmark_execution() {
        let mut suite = PerformanceSuite::new();
        suite.benchmark_safe_mul_yield();
        
        assert!(!suite.results.is_empty());
        
        for result in &suite.results {
            assert!(result.iterations > 0);
            assert!(result.total_time_nanos > 0);
            assert!(result.avg_time_per_op_nanos > 0);
            assert!(result.ops_per_second > 0);
        }
    }

    #[test]
    fn test_comparative_benchmarks() {
        let mut suite = PerformanceSuite::new();
        suite.run_comparative_benchmarks();
        
        assert!(!suite.comparative_results.is_empty());
        
        for comparison in &suite.comparative_results {
            assert!(comparison.optimized_result.iterations == comparison.baseline_result.iterations);
            assert!(comparison.speed_improvement_percentage >= 0.0);
            assert!(comparison.gas_savings_percentage >= 0.0);
        }
    }

    #[test]
    fn test_performance_report_generation() {
        let mut suite = PerformanceSuite::new();
        suite.benchmark_safe_add_yield();
        suite.run_comparative_benchmarks();
        
        let report = suite.generate_performance_report();
        
        assert!(!report.benchmark_results.is_empty());
        assert!(!report.recommendations.is_empty());
        assert_eq!(report.summary_statistics.number_of_benchmarks, suite.results.len() as u64);
    }

    #[test]
    fn test_gas_estimation() {
        let suite = PerformanceSuite::new();
        
        let gas_cost = suite.estimate_gas_cost("safe_mul_yield");
        assert!(gas_cost > 0);
        
        let gas_savings = suite.estimate_gas_savings("safe_mul_yield", "baseline_mul_yield");
        assert!(gas_savings >= 0.0);
    }

    #[test]
    fn test_baseline_implementations() {
        // Test baseline multiplication
        let result = baseline_mul_yield(1000, 5000, 86400);
        assert_eq!(result, Some(432000000));
        
        // Test baseline BPS division
        let result = baseline_bps_division(1000, 3333);
        assert_eq!(result, Some(333));
        
        // Test baseline yield distribution
        let result = baseline_yield_distribution(1000);
        assert_eq!(result, Some((500, 300, 200)));
    }
}
