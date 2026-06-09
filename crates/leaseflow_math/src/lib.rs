#![no_std]

/// Calculates the total rental cost given a duration (seconds) and a rate
/// (cost per second). Returns `None` on overflow.
pub fn calculate_total_cost(duration_secs: u64, rate_per_sec: u64) -> Option<u64> {
    duration_secs.checked_mul(rate_per_sec)
}
