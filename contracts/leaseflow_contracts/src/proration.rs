#![no_std]

/// Howard Hinnant's algorithm to convert days since the Unix epoch (1970-01-01) into (Year, Month, Day).
pub fn timestamp_to_ymd(timestamp: u64) -> (u64, u8, u8) {
    let days_since_epoch = timestamp / 86400;
    
    let z = days_since_epoch + 719468;
    let era = z / 146097;
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    
    let year = if m <= 2 { y + 1 } else { y };
    (year, m as u8, d as u8)
}

/// Checks if a given civil year is a leap year.
pub fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Returns the number of days in the specified month of the specified year.
pub fn days_in_month(year: u64, month: u8) -> u64 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 => if is_leap_year(year) { 29 } else { 28 },
        _ => 31,
    }
}

/// Calculates the prorated first month's rent down to the exact second.
/// Automatically handles varying month lengths and leap years.
pub fn calculate_first_month_rent(start_date: u64, rent_amount: i128) -> i128 {
    let (year, month, day) = timestamp_to_ymd(start_date);
    let total_days = days_in_month(year, month);
    let total_month_secs = total_days * 86400;
    
    // Seconds elapsed in the month before move-in.
    let days_elapsed = (day as u64) - 1;
    let secs_in_current_day_elapsed = start_date % 86400;
    let elapsed_secs = days_elapsed * 86400 + secs_in_current_day_elapsed;
    
    // Remaining seconds occupied down to the exact second.
    let occupied_secs = total_month_secs.saturating_sub(elapsed_secs);
    
    // Formula: (Rent * occupied_seconds) / total_month_seconds
    // i128 is used directly to prevent overflow issues during large multiplications.
    (rent_amount.saturating_mul(occupied_secs as i128)) / (total_month_secs as i128)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_to_ymd() {
        // 1970-01-01T00:00:00Z
        assert_eq!(timestamp_to_ymd(0), (1970, 1, 1));
        // 2024-02-29T12:00:00Z (Leap year)
        assert_eq!(timestamp_to_ymd(1709208000), (2024, 2, 29));
        // 2023-02-28T23:59:59Z (Non-leap year)
        assert_eq!(timestamp_to_ymd(1677628799), (2023, 2, 28));
    }

    #[test]
    fn test_calculate_first_month_rent() {
        // Standard Rent (3000 stroops or whatever unit)
        let rent = 3000_0000000_i128;
        
        // 30 days month (April 2024)
        // 2024-04-01T00:00:00Z = 1711929600
        let exact_first_rent = calculate_first_month_rent(1711929600, rent);
        assert_eq!(exact_first_rent, rent); // 100% of the month

        // Exact half month (April 16, 00:00:00) -> 15 days elapsed out of 30
        let half_rent = calculate_first_month_rent(1711929600 + (15 * 86400), rent);
        assert_eq!(half_rent, rent / 2);

        // February Leap Year (29 days) -> 2024-02-01: 1706745600
        let feb_leap_first = calculate_first_month_rent(1706745600, rent);
        assert_eq!(feb_leap_first, rent);

        // February Non-Leap Year (28 days) -> 2023-02-01: 1675209600
        // exactly 14 days elapsed (2023-02-15) -> exactly 50%
        let feb_non_leap_half = calculate_first_month_rent(1675209600 + (14 * 86400), rent);
        assert_eq!(feb_non_leap_half, rent / 2);
    }
}
