# Fractional Rent Payment Epoch Boundary Fuzz Tests

## Overview

This fuzz test suite exhaustively tests fractional rent payments that cross epoch boundaries (monthly billing cycles, leap years, varying month lengths) to ensure the LeaseFlow Protocol handles these edge cases correctly.

## Test File

`fuzz_targets/fuzz_fractional_rent_epoch_boundaries.rs`

## Key Test Scenarios

### 1. **Month Boundary Crossings**
- Tests fractional payments that cross from one month to another
- Validates prorated calculations at month boundaries
- Ensures correct handling of varying month lengths (28-31 days)

### 2. **Leap Year Boundary Crossings**
- Tests payments crossing from non-leap year February to leap year February
- Validates correct day count (28 vs 29 days)
- Ensures prorated calculations account for leap year differences

### 3. **Quarter Boundary Crossings**
- Tests payments crossing quarter boundaries (Mar 31 → Apr 1, etc.)
- Validates state transitions across fiscal quarters

### 4. **Year Boundary Crossings**
- Tests payments crossing year boundaries (Dec 31 → Jan 1)
- Ensures correct billing cycle reset

### 5. **Multiple Sequential Boundaries**
- Tests payments crossing multiple boundaries in sequence
- Validates cumulative effects of multiple boundary crossings

## Properties Verified

### Property 1: Valid Input Validation
- Monthly rent must be positive
- Lease duration must be non-zero
- Payment timestamps must be within lease period

### Property 2: Boundary Type Validation
- Leap year boundary tests actually use leap year dates
- Month boundary tests actually cross month boundaries
- Boundary flags match actual timestamp behavior

### Property 3: Final State Invariants
- Total paid never negative
- Outstanding balance never negative
- Payment history matches total paid
- Billing cycles are monotonic
- Timestamps are monotonic

### Property 4: Prorated Calculation Accuracy
- Prorated amounts are non-negative
- Prorated amounts don't exceed monthly rent
- Prorated amounts are proportional to time fraction
- Calculations stay within tolerance (0.1%)

### Property 5: Leap Year Boundary Handling
- February has correct day count (28 or 29)
- Seconds in February match expected values
- Prorated calculations account for leap year

### Property 6: Month Boundary Handling
- Seconds in month are between 28-31 days
- Month-specific day counts are correct
- Boundary detection is accurate

### Property 7: Idempotency
- Processing the same payment twice yields consistent results
- State remains consistent after duplicate payment attempts

### Property 8: Extreme Values
- Very large rent amounts are handled correctly
- Overflow conditions are caught gracefully
- Invariants hold even with extreme values

### Property 9: Conservation of Value
- Total paid equals sum of all payments
- No tokens are lost or created

### Property 10: Billing Cycle Consistency
- Billing cycle count matches payment history
- Cycle transitions are correct

## Running the Fuzz Tests

### Prerequisites
- Rust toolchain installed
- `cargo-fuzz` installed: `cargo install cargo-fuzz`

### Run the fuzzer
```bash
cd fuzz
cargo fuzz run fuzz_fractional_rent_epoch_boundaries
```

### Run with specific corpus
```bash
cargo fuzz run fuzz_fractional_rent_epoch_boundaries -- -seed=12345
```

### Run for limited time
```bash
cargo fuzz run fuzz_fractional_rent_epoch_boundaries -- -max_total_time=300
```

### Run with limited iterations
```bash
cargo fuzz run fuzz_fractional_rent_epoch_boundaries -- -runs=10000
```

## Test Configuration

The fuzzer accepts the following configuration options via the `TestConfig` struct:

- `strict_invariants`: Enable strict invariant checking after each payment
- `test_idempotency`: Test payment idempotency
- `extreme_values`: Test with extreme rent values

## Boundary Types

The fuzzer tests the following boundary types:

1. **MonthBoundary**: Tests crossing from one month to another
2. **LeapYearBoundary**: Tests leap year transitions
3. **QuarterBoundary**: Tests quarter-end transitions
4. **YearBoundary**: Tests year-end transitions
5. **MultipleBoundaries**: Tests sequences of boundary crossings
6. **NoBoundary**: Control case with no boundary crossings

## Integration with LeaseFlow Protocol

This fuzzer tests the mathematical functions from `leaseflow_math` crate:
- `calculate_prorated_rent`: Prorated rent calculations
- `calculate_termination_refund`: Refund calculations
- `next_billing_date`: Billing date calculations
- `get_seconds_in_month`: Month length calculations
- `timestamp_to_ymd`: Timestamp conversion
- `days_in_month`: Day count per month

## Expected Outcomes

When running successfully, the fuzzer should:
- Process millions of test cases without crashes
- Maintain all invariants across all test cases
- Find no violations of the properties listed above
- Generate corpus files in `fuzz/corpus/fuzz_fractional_rent_epoch_boundaries/`

## Troubleshooting

If the fuzzer finds a crash:
1. Check the crash input in `fuzz/artifacts/`
2. Minimize the crash input: `cargo fuzz tmin fuzz_fractional_rent_epoch_boundaries`
3. Add the minimized input as a unit test
4. Fix the underlying issue
5. Re-run the fuzzer to verify the fix

## Contributing

When adding new boundary scenarios:
1. Add the boundary type to the `BoundaryType` enum
2. Add validation logic in the fuzzer
3. Add specific property checks for the new boundary
4. Update this documentation
5. Add unit tests for the new scenario

## Security Implications

These fuzz tests help prevent:
- Rounding errors that could leak tokens
- Incorrect prorated calculations at boundaries
- State corruption across billing cycles
- Exploitation of leap year edge cases
- Double-spending or token loss scenarios
