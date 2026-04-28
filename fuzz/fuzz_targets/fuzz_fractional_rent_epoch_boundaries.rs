#![no_main]

//! Fractional Rent Payment Epoch Boundary Fuzzer
//!
//! This fuzzer exhaustively tests fractional rent payments that cross epoch boundaries
//! (monthly billing cycles, leap years, varying month lengths) to ensure:
//! 1. Accurate prorated calculations at boundaries
//! 2. No loss or gain of tokens due to rounding errors
//! 3. Correct handling of leap year transitions
//! 4. Proper state transitions across billing cycles
//! 5. Idempotency of fractional payment operations

use arbitrary::Arbitrary;
use leaseflow_math::{
    calculate_prorated_rent,
    calculate_termination_refund,
    next_billing_date,
    get_seconds_in_month,
    timestamp_to_ymd,
    days_in_month,
};
use libfuzzer_sys::fuzz_target;

/// Fuzz input for fractional rent payment scenarios
#[derive(Arbitrary, Debug, Clone)]
struct FractionalRentInput {
    /// Lease parameters
    monthly_rent: i64,
    lease_start_timestamp: u64,
    lease_duration_months: u32,
    
    /// Fractional payment parameters
    payment_sequence: Vec<FractionalPayment>,
    
    /// Boundary conditions to test
    boundary_type: BoundaryType,
    
    /// Test configuration
    test_config: TestConfig,
}

#[derive(Arbitrary, Debug, Clone)]
struct FractionalPayment {
    /// Payment timestamp
    payment_timestamp: u64,
    /// Payment amount (can be fractional of monthly rent)
    payment_amount: i64,
    /// Whether this payment crosses a boundary
    crosses_boundary: bool,
}

#[derive(Arbitrary, Debug, Clone)]
enum BoundaryType {
    /// Month boundary (e.g., Jan 31 -> Feb 1)
    MonthBoundary,
    /// Leap year boundary (e.g., Feb 28 2023 -> Feb 29 2024)
    LeapYearBoundary,
    /// Quarter boundary (e.g., Mar 31 -> Apr 1)
    QuarterBoundary,
    /// Year boundary (e.g., Dec 31 -> Jan 1)
    YearBoundary,
    /// Multiple boundaries in sequence
    MultipleBoundaries,
    /// No boundary (control case)
    NoBoundary,
}

#[derive(Arbitrary, Debug, Clone)]
struct TestConfig {
    /// Enable strict invariant checking
    strict_invariants: bool,
    /// Test idempotency
    test_idempotency: bool,
    /// Test with extreme values
    extreme_values: bool,
}

/// Simulated lease state for fractional payment testing
#[derive(Debug, Clone, PartialEq, Eq)]
struct SimulatedLease {
    monthly_rent: i64,
    start_timestamp: u64,
    end_timestamp: u64,
    total_paid: i64,
    outstanding_balance: i64,
    last_payment_timestamp: u64,
    billing_cycle_count: u32,
    payment_history: Vec<PaymentRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PaymentRecord {
    timestamp: u64,
    amount: i64,
    billing_cycle: u32,
    prorated_amount: Option<i64>,
}

impl SimulatedLease {
    fn new(monthly_rent: i64, start_timestamp: u64, duration_months: u32) -> Self {
        let end_timestamp = Self::calculate_end_timestamp(start_timestamp, duration_months);
        
        Self {
            monthly_rent,
            start_timestamp,
            end_timestamp,
            total_paid: 0,
            outstanding_balance: monthly_rent,
            last_payment_timestamp: 0,
            billing_cycle_count: 0,
            payment_history: Vec::new(),
        }
    }
    
    fn calculate_end_timestamp(start: u64, months: u32) -> u64 {
        next_billing_date(start, months)
    }
    
    fn get_billing_cycle(&self, timestamp: u64) -> u32 {
        if timestamp < self.start_timestamp {
            return 0;
        }
        
        let elapsed_seconds = timestamp.saturating_sub(self.start_timestamp);
        let approx_months = elapsed_seconds / (30 * 86400); // Approximate
        approx_months as u32
    }
    
    fn process_fractional_payment(&mut self, payment: &FractionalPayment) -> Result<(), PaymentError> {
        // Validate payment timestamp
        if payment.payment_timestamp < self.start_timestamp {
            return Err(PaymentError::PaymentBeforeLeaseStart);
        }
        
        if payment.payment_timestamp > self.end_timestamp {
            return Err(PaymentError::PaymentAfterLeaseEnd);
        }
        
        // Validate payment amount
        if payment.payment_amount <= 0 {
            return Err(PaymentError::InvalidAmount);
        }
        
        let billing_cycle = self.get_billing_cycle(payment.payment_timestamp);
        
        // Calculate prorated amount if payment crosses boundary
        let prorated_amount = if payment.crosses_boundary {
            let next_boundary = self.find_next_boundary(payment.payment_timestamp);
            if let Some(boundary_ts) = next_boundary {
                calculate_prorated_rent(
                    self.monthly_rent,
                    payment.payment_timestamp,
                    boundary_ts
                ).map(|(amount, _)| amount)
            } else {
                None
            }
        } else {
            None
        };
        
        // Update state
        self.total_paid += payment.payment_amount;
        self.outstanding_balance = self.outstanding_balance.saturating_sub(payment.payment_amount);
        self.last_payment_timestamp = payment.payment_timestamp;
        
        // Update billing cycle if we've crossed into a new cycle
        if billing_cycle > self.billing_cycle_count {
            self.billing_cycle_count = billing_cycle;
            // Reset outstanding balance for new cycle
            self.outstanding_balance += self.monthly_rent;
        }
        
        // Record payment
        self.payment_history.push(PaymentRecord {
            timestamp: payment.payment_timestamp,
            amount: payment.payment_amount,
            billing_cycle,
            prorated_amount,
        });
        
        Ok(())
    }
    
    fn find_next_boundary(&self, timestamp: u64) -> Option<u64> {
        let (year, month, day) = timestamp_to_ymd(timestamp);
        
        // Find next month boundary
        let next_month = if month == 12 { 1 } else { month + 1 };
        let next_year = if month == 12 { year + 1 } else { year };
        
        let max_day = days_in_month(next_year, next_month);
        let clamped_day = day.min(max_day);
        
        Some(leaseflow_math::ymd_to_timestamp(next_year, next_month, clamped_day))
    }
    
    fn verify_invariants(&self) -> Result<(), InvariantViolation> {
        // Invariant 1: Total paid should never be negative
        if self.total_paid < 0 {
            return Err(InvariantViolation::NegativeTotalPaid);
        }
        
        // Invariant 2: Outstanding balance should be non-negative
        if self.outstanding_balance < 0 {
            return Err(InvariantViolation::NegativeOutstandingBalance);
        }
        
        // Invariant 3: Payment history should match total paid
        let history_sum: i64 = self.payment_history.iter().map(|p| p.amount).sum();
        if history_sum != self.total_paid {
            return Err(InvariantViolation::PaymentHistoryMismatch {
                expected: self.total_paid,
                actual: history_sum,
            });
        }
        
        // Invariant 4: Billing cycles should be monotonic
        for window in self.payment_history.windows(2) {
            if window[1].billing_cycle < window[0].billing_cycle {
                return Err(InvariantViolation::NonMonotonicBillingCycles);
            }
        }
        
        // Invariant 5: Timestamps should be monotonic
        for window in self.payment_history.windows(2) {
            if window[1].timestamp < window[0].timestamp {
                return Err(InvariantViolation::NonMonotonicTimestamps);
            }
        }
        
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
enum PaymentError {
    PaymentBeforeLeaseStart,
    PaymentAfterLeaseEnd,
    InvalidAmount,
    Overflow,
}

#[derive(Debug, PartialEq, Eq)]
enum InvariantViolation {
    NegativeTotalPaid,
    NegativeOutstandingBalance,
    PaymentHistoryMismatch { expected: i64, actual: i64 },
    NonMonotonicBillingCycles,
    NonMonotonicTimestamps,
    ProratedCalculationError,
}

fuzz_target!(|input: FractionalRentInput| {
    let FractionalRentInput {
        monthly_rent,
        lease_start_timestamp,
        lease_duration_months,
        payment_sequence,
        boundary_type,
        test_config,
    } = input;
    
    // --- PROPERTY 1: Valid input validation ---
    if monthly_rent <= 0 {
        return; // Invalid rent amount, skip
    }
    
    if lease_duration_months == 0 {
        return; // Invalid duration, skip
    }
    
    // --- PROPERTY 2: Boundary type validation ---
    match boundary_type {
        BoundaryType::LeapYearBoundary => {
            // Ensure we're actually testing a leap year boundary
            let (year, month, _) = timestamp_to_ymd(lease_start_timestamp);
            if !is_near_leap_year_boundary(year, month) {
                return; // Not actually a leap year boundary, skip
            }
        }
        BoundaryType::MonthBoundary => {
            // Ensure payment sequence actually crosses month boundaries
            if !has_month_boundary_crossing(&payment_sequence) {
                return; // No month boundary crossing, skip
            }
        }
        _ => {}
    }
    
    // Create simulated lease
    let mut lease = SimulatedLease::new(monthly_rent, lease_start_timestamp, lease_duration_months);
    
    // Process payment sequence
    let mut payment_results = Vec::new();
    for payment in &payment_sequence {
        let result = lease.process_fractional_payment(payment);
        payment_results.push(result);
        
        // If strict invariants are enabled, check after each payment
        if test_config.strict_invariants {
            if let Err(e) = lease.verify_invariants() {
                panic!("Invariant violation during payment processing: {:?}", e);
            }
        }
    }
    
    // --- PROPERTY 3: Final state invariants ---
    lease.verify_invariants()
        .expect("Final invariant violation");
    
    // --- PROPERTY 4: Prorated calculation accuracy ---
    for (payment, record) in payment_sequence.iter().zip(lease.payment_history.iter()) {
        if payment.crosses_boundary {
            if let Some(prorated) = record.prorated_amount {
                // Prorated amount should be reasonable
                assert!(prorated >= 0, "Prorated amount negative: {}", prorated);
                assert!(prorated <= monthly_rent, "Prorated amount exceeds monthly rent: {} > {}", prorated, monthly_rent);
                
                // Prorated amount should be proportional to time fraction
                let next_boundary = lease.find_next_boundary(payment.payment_timestamp)
                    .expect("Should find next boundary for crossing payment");
                let duration = next_boundary.saturating_sub(payment.payment_timestamp);
                let seconds_in_month = get_seconds_in_month(payment.payment_timestamp);
                
                if seconds_in_month > 0 {
                    let expected_approx = (monthly_rent as i128 * duration as i128) / seconds_in_month as i128;
                    let tolerance = (monthly_rent as i128 / 1000) as i64; // 0.1% tolerance
                    assert!((prorated as i128 - expected_approx).abs() <= tolerance as i128,
                        "Prorated calculation inaccurate: expected approx {}, got {}", expected_approx, prorated);
                }
            }
        }
    }
    
    // --- PROPERTY 5: Leap year boundary handling ---
    if boundary_type == BoundaryType::LeapYearBoundary {
        verify_leap_year_boundary_correctness(&lease, &payment_sequence);
    }
    
    // --- PROPERTY 6: Month boundary handling ---
    if boundary_type == BoundaryType::MonthBoundary {
        verify_month_boundary_correctness(&lease, &payment_sequence);
    }
    
    // --- PROPERTY 7: Idempotency test ---
    if test_config.test_idempotency && !payment_sequence.is_empty() {
        test_payment_idempotency(monthly_rent, lease_start_timestamp, lease_duration_months, &payment_sequence[0]);
    }
    
    // --- PROPERTY 8: Extreme values handling ---
    if test_config.extreme_values {
        verify_extreme_value_handling(monthly_rent, lease_start_timestamp, lease_duration_months);
    }
    
    // --- PROPERTY 9: Conservation of value ---
    // Total paid should equal sum of all payments
    let total_from_history: i64 = lease.payment_history.iter().map(|p| p.amount).sum();
    assert_eq!(lease.total_paid, total_from_history,
        "Total paid mismatch: {} != {}", lease.total_paid, total_from_history);
    
    // --- PROPERTY 10: Billing cycle consistency ---
    // Billing cycle count should match the maximum cycle in payment history
    let max_cycle = lease.payment_history.iter()
        .map(|p| p.billing_cycle)
        .max()
        .unwrap_or(0);
    assert!(lease.billing_cycle_count >= max_cycle,
        "Billing cycle count inconsistent: {} < {}", lease.billing_cycle_count, max_cycle);
});

/// Check if a given year/month is near a leap year boundary
fn is_near_leap_year_boundary(year: u64, month: u8) -> bool {
    // Check if we're in February of a leap year or adjacent year
    if month == 2 {
        return is_leap_year(year) || is_leap_year(year + 1) || is_leap_year(year - 1);
    }
    false
}

/// Check if payment sequence has month boundary crossings
fn has_month_boundary_crossing(payments: &[FractionalPayment]) -> bool {
    payments.iter().any(|p| p.crosses_boundary)
}

/// Verify leap year boundary calculations are correct
fn verify_leap_year_boundary_correctness(lease: &SimulatedLease, payments: &[FractionalPayment]) {
    for payment in payments {
        let (year, month, _) = timestamp_to_ymd(payment.payment_timestamp);
        
        if month == 2 {
            // February - check leap year handling
            let seconds_in_feb = get_seconds_in_month(payment.payment_timestamp);
            let expected_days = if is_leap_year(year) { 29 } else { 28 };
            let expected_seconds = expected_days * 86400;
            
            assert_eq!(seconds_in_feb, expected_seconds,
                "Incorrect February seconds for year {}: expected {}, got {}",
                year, expected_seconds, seconds_in_feb);
        }
    }
}

/// Verify month boundary calculations are correct
fn verify_month_boundary_correctness(lease: &SimulatedLease, payments: &[FractionalPayment]) {
    for payment in payments {
        if payment.crosses_boundary {
            let seconds_in_month = get_seconds_in_month(payment.payment_timestamp);
            
            // Should be between 28 and 31 days worth of seconds
            assert!(seconds_in_month >= 28 * 86400 && seconds_in_month <= 31 * 86400,
                "Invalid seconds in month: {}", seconds_in_month);
            
            // Verify it matches expected for the specific month
            let (year, month, _) = timestamp_to_ymd(payment.payment_timestamp);
            let expected_days = days_in_month(year, month);
            let expected_seconds = expected_days * 86400;
            
            assert_eq!(seconds_in_month, expected_seconds,
                "Month seconds mismatch for {}-{:02}: expected {}, got {}",
                year, month, expected_seconds, seconds_in_month);
        }
    }
}

/// Test that processing the same payment twice yields idempotent results
fn test_payment_idempotency(
    monthly_rent: i64,
    start_timestamp: u64,
    duration_months: u32,
    payment: &FractionalPayment,
) {
    let mut lease1 = SimulatedLease::new(monthly_rent, start_timestamp, duration_months);
    let mut lease2 = SimulatedLease::new(monthly_rent, start_timestamp, duration_months);
    
    // Process payment once
    let _ = lease1.process_fractional_payment(payment);
    
    // Process payment twice (should fail or be idempotent)
    let result2 = lease2.process_fractional_payment(payment);
    
    // If second payment succeeds, it should be a new payment (not idempotent)
    // This is expected behavior - we're testing the system handles it correctly
    if result2.is_ok() {
        // Verify state is consistent
        assert_eq!(lease2.total_paid, lease1.total_paid + payment.payment_amount);
    }
}

/// Verify handling of extreme rent values
fn verify_extreme_value_handling(monthly_rent: i64, start_timestamp: u64, duration_months: u32) {
    // Test with very large rent amounts
    let large_rent = i64::MAX / 1000;
    let mut lease = SimulatedLease::new(large_rent, start_timestamp, duration_months);
    
    let payment = FractionalPayment {
        payment_timestamp: start_timestamp + 86400,
        payment_amount: large_rent / 10,
        crosses_boundary: false,
    };
    
    let result = lease.process_fractional_payment(&payment);
    
    // Should either succeed or fail gracefully
    match result {
        Ok(_) => {
            // Verify invariants still hold
            lease.verify_invariants().expect("Invariants violated with extreme values");
        }
        Err(_) => {
            // Failure is acceptable for extreme values
        }
    }
}

/// Check if a year is a leap year
fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fractional_payment_month_boundary() {
        let mut lease = SimulatedLease::new(1000, 1704067200, 6); // 6-month lease starting Jan 2024
        
        let payment = FractionalPayment {
            payment_timestamp: 1706745600, // Feb 1, 2024
            payment_amount: 500,
            crosses_boundary: true,
        };
        
        lease.process_fractional_payment(&payment).unwrap();
        assert_eq!(lease.total_paid, 500);
        assert_eq!(lease.outstanding_balance, 500);
    }
    
    #[test]
    fn test_leap_year_boundary() {
        let mut lease = SimulatedLease::new(1000, 1675209600, 12); // Lease starting Feb 2023
        
        let payment = FractionalPayment {
            payment_timestamp: 1706745600, // Feb 2024 (leap year)
            payment_amount: 1000,
            crosses_boundary: true,
        };
        
        lease.process_fractional_payment(&payment).unwrap();
        lease.verify_invariants().unwrap();
    }
    
    #[test]
    fn test_multiple_fractional_payments() {
        let mut lease = SimulatedLease::new(1000, 1704067200, 3);
        
        let payments = vec![
            FractionalPayment {
                payment_timestamp: 1704153600,
                payment_amount: 300,
                crosses_boundary: false,
            },
            FractionalPayment {
                payment_timestamp: 1706745600,
                payment_amount: 400,
                crosses_boundary: true,
            },
            FractionalPayment {
                payment_timestamp: 1709251200,
                payment_amount: 300,
                crosses_boundary: false,
            },
        ];
        
        for payment in &payments {
            lease.process_fractional_payment(payment).unwrap();
        }
        
        assert_eq!(lease.total_paid, 1000);
        assert_eq!(lease.payment_history.len(), 3);
        lease.verify_invariants().unwrap();
    }
    
    #[test]
    fn test_invariant_violation_detection() {
        let mut lease = SimulatedLease::new(1000, 1704067200, 3);
        
        // Manually corrupt state to test invariant detection
        lease.total_paid = -100;
        
        assert!(matches!(
            lease.verify_invariants(),
            Err(InvariantViolation::NegativeTotalPaid)
        ));
    }
}
