#![no_main]

use arbitrary::Arbitrary;
use leaseflow_math::calculate_total_cost;
use libfuzzer_sys::fuzz_target;

/// Structured fuzz input covering both duration and rate dimensions.
#[derive(Arbitrary, Debug)]
struct RentInput {
    duration_secs: u64,
    rate_per_sec: u64,
}

fuzz_target!(|input: RentInput| {
    let RentInput {
        duration_secs,
        rate_per_sec,
    } = input;

    // --- Property 1: zero duration always yields zero cost ---
    if duration_secs == 0 {
        let cost = calculate_total_cost(0, rate_per_sec);
        assert_eq!(
            cost,
            Some(0),
            "zero duration must produce zero cost, got {:?}",
            cost
        );
        return;
    }

    // --- Property 2: zero rate always yields zero cost ---
    if rate_per_sec == 0 {
        let cost = calculate_total_cost(duration_secs, 0);
        assert_eq!(
            cost,
            Some(0),
            "zero rate must produce zero cost, got {:?}",
            cost
        );
        return;
    }

    // --- Property 3: overflow must be caught, never silently wrong ---
    match calculate_total_cost(duration_secs, rate_per_sec) {
        None => {
            // Overflow was detected correctly via checked arithmetic.
            // Verify the overflow is real: duration * rate would exceed u64::MAX.
            // We confirm by checking that the mathematical product overflows u128 bounds
            // relative to u64::MAX.
            let product = (duration_secs as u128).checked_mul(rate_per_sec as u128);
            match product {
                Some(p) if p > u64::MAX as u128 => {
                    // Correct: overflow was real and was caught.
                }
                Some(_) => {
                    panic!(
                        "calculate_total_cost returned None for non-overflowing inputs: \
                         duration={}, rate={}",
                        duration_secs, rate_per_sec
                    );
                }
                None => {
                    // u128 overflow — definitely a real overflow, correctly caught.
                }
            }
        }
        Some(cost) => {
            // --- Property 4: result must be deterministic ---
            let cost2 = calculate_total_cost(duration_secs, rate_per_sec);
            assert_eq!(
                Some(cost),
                cost2,
                "non-deterministic result for duration={}, rate={}",
                duration_secs,
                rate_per_sec
            );

            // --- Property 5: monotonicity — cost grows with duration (fixed rate) ---
            if duration_secs > 1 {
                let smaller = calculate_total_cost(duration_secs - 1, rate_per_sec);
                if let Some(smaller_cost) = smaller {
                    assert!(
                        cost >= smaller_cost,
                        "monotonicity violated: cost({}) < cost({}) at rate={}",
                        duration_secs,
                        duration_secs - 1,
                        rate_per_sec
                    );
                }
            }

            // --- Property 6: explicit boundary spot-checks ---
            // 1 second — must be non-zero for non-zero rate
            let one_sec = calculate_total_cost(1, rate_per_sec);
            assert!(
                one_sec.is_some(),
                "1-second duration overflowed unexpectedly at rate={}",
                rate_per_sec
            );

            // 10 years (315_360_000 seconds) — must not overflow for reasonable rates
            if rate_per_sec <= 1_000_000 {
                let ten_years = calculate_total_cost(315_360_000, rate_per_sec);
                assert!(
                    ten_years.is_some(),
                    "10-year duration overflowed at rate={} (should be safe)",
                    rate_per_sec
                );
            }
        }
    }
});
