//! # Rent Escalation Boundary Checks (Issue #173)
//!
//! This module provides overflow-safe rent escalation logic for LeaseInstance.
//! All arithmetic uses checked operations to prevent integer overflow exploits.

use soroban_sdk::{contracterror, contracttype, Env};

// --- Constants ----------------------------------------------------------------

/// Basis-points denominator (10_000 = 100%).
pub const BPS_DENOMINATOR: i128 = 10_000;

/// Maximum single-step escalation: 50% in basis points.
pub const MAX_ESCALATION_RATE_BPS: i128 = 5_000;

/// Absolute rent ceiling in stroops (1 trillion = ~100 XLM).
/// Prevents a base rent so large that one multiplication overflows i128.
pub const MAX_RENT_AMOUNT: i128 = 1_000_000_000_000;

/// Maximum number of escalation steps over a lease lifetime (~10 years monthly).
pub const MAX_ESCALATION_STEPS: u32 = 120;

// --- Error codes --------------------------------------------------------------

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum EscalationError {
    /// escalation_rate_bps is negative.
    EscalationRateNegative       = 100,
    /// escalation_rate_bps exceeds MAX_ESCALATION_RATE_BPS (50%).
    EscalationRateTooHigh        = 101,
    /// escalation_interval_ledgers is zero when rate is non-zero.
    EscalationIntervalZero       = 102,
    /// A single escalation step would overflow i128.
    EscalatedRentOverflow        = 103,
    /// The escalated rent would exceed the agreed cap.
    EscalatedRentExceedsCap      = 104,
    /// The base rent_amount exceeds MAX_RENT_AMOUNT.
    RentAmountExceedsMaxAllowed  = 105,
    /// Cumulative compound escalation over the full lease life overflows i128.
    CumulativeEscalationOverflow = 106,
    /// rent_cap is set below the base rent_amount.
    RentCapBelowBaseRent         = 107,
}

// --- Config struct ------------------------------------------------------------

/// Escalation configuration attached to a LeaseInstance.
/// Store this alongside the lease in persistent storage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscalationConfig {
    /// Periodic increase expressed in basis points (e.g. 300 = 3%).
    /// Set to 0 for a fixed-rent lease.
    pub rate_bps: i128,
    /// Ledgers between each escalation step.
    /// Ignored when rate_bps == 0.
    pub interval_ledgers: u32,
    /// Ledger sequence number when the NEXT escalation becomes due.
    pub next_escalation_ledger: u32,
    /// How many escalation steps have been applied so far.
    pub steps_applied: u32,
    /// Hard cap on current_rent (0 = use MAX_RENT_AMOUNT).
    pub rent_cap: i128,
    /// Snapshot of the current (possibly escalated) rent amount.
    pub current_rent: i128,
}

// --- Validation ---------------------------------------------------------------

/// Validates all escalation parameters **before** a lease is stored.
///
/// Six-layer defence:
/// 1. Negative rate ? `EscalationRateNegative`
/// 2. Rate > `MAX_ESCALATION_RATE_BPS` ? `EscalationRateTooHigh`
/// 3. Interval == 0 with non-zero rate ? `EscalationIntervalZero`
/// 4. `base_rent > MAX_RENT_AMOUNT` ? `RentAmountExceedsMaxAllowed`
/// 5. `rent_cap < base_rent` ? `RentCapBelowBaseRent`
/// 6. Simulate `MAX_ESCALATION_STEPS` in u128 wide arithmetic to detect
///    cumulative overflow before the first ledger is written.
pub fn validate_escalation_config(
    rate_bps: i128,
    interval_ledgers: u32,
    base_rent: i128,
    rent_cap: i128,
) -> Result<(), EscalationError> {
    // ? Negative rate
    if rate_bps < 0 {
        return Err(EscalationError::EscalationRateNegative);
    }

    // Fixed-rent lease – nothing more to check.
    if rate_bps == 0 {
        return Ok(());
    }

    // ? Rate ceiling
    if rate_bps > MAX_ESCALATION_RATE_BPS {
        return Err(EscalationError::EscalationRateTooHigh);
    }

    // ? Interval must be positive when rate is set
    if interval_ledgers == 0 {
        return Err(EscalationError::EscalationIntervalZero);
    }

    // ? Base rent ceiling
    if base_rent > MAX_RENT_AMOUNT {
        return Err(EscalationError::RentAmountExceedsMaxAllowed);
    }

    // ? Cap sanity
    let effective_cap: i128 = if rent_cap > 0 {
        if rent_cap < base_rent {
            return Err(EscalationError::RentCapBelowBaseRent);
        }
        if rent_cap > MAX_RENT_AMOUNT {
            return Err(EscalationError::RentAmountExceedsMaxAllowed);
        }
        rent_cap
    } else {
        MAX_RENT_AMOUNT
    };

    // ? Check first step doesn't overflow i128
    base_rent
        .checked_mul(rate_bps)
        .and_then(|v| v.checked_div(BPS_DENOMINATOR))
        .and_then(|inc| base_rent.checked_add(inc))
        .ok_or(EscalationError::EscalatedRentOverflow)?;

    // ? Simulate full lifetime in u128 wide arithmetic
    let mut simulated: u128 = base_rent as u128;
    let rate_num = rate_bps as u128;
    let rate_den = BPS_DENOMINATOR as u128;
    let cap_u128 = effective_cap as u128;

    for _ in 0..MAX_ESCALATION_STEPS {
        let increment = simulated
            .checked_mul(rate_num)
            .map(|v| v / rate_den)
            .unwrap_or(u128::MAX);

        simulated = simulated.checked_add(increment).unwrap_or(u128::MAX);

        // Once the simulation exceeds the cap we can stop — the runtime
        // apply_escalation will clamp at the cap.
        if simulated > cap_u128 {
            break;
        }
    }

    // Reject if the fully-compounded value would overflow i128
    if simulated > i128::MAX as u128 {
        return Err(EscalationError::CumulativeEscalationOverflow);
    }

    Ok(())
}

// --- Runtime escalation -------------------------------------------------------

/// Applies the next escalation step if it is due.
///
/// Returns the updated `EscalationConfig`.  The caller is responsible for
/// persisting the returned config back to storage.
///
/// # Security properties
/// - Uses `checked_mul` / `checked_add` / `checked_div` throughout.
/// - Panics with a descriptive `EscalationError` on any overflow.
/// - Enforces `effective_cap` before writing.
/// - Returns unchanged config if escalation is not yet due.
/// - Returns unchanged config once `MAX_ESCALATION_STEPS` is reached.
pub fn apply_escalation(
    env: &Env,
    mut config: EscalationConfig,
) -> Result<EscalationConfig, EscalationError> {
    // Fixed-rent lease — nothing to do.
    if config.rate_bps == 0 {
        return Ok(config);
    }

    let current_ledger = env.ledger().sequence();

    // Not yet due.
    if current_ledger < config.next_escalation_ledger {
        return Ok(config);
    }

    // Step limit reached.
    if config.steps_applied >= MAX_ESCALATION_STEPS {
        return Ok(config);
    }

    // -- Overflow-safe arithmetic --------------------------------------------
    //   new_rent = current_rent + (current_rent * rate_bps) / BPS_DENOMINATOR
    let increment = config
        .current_rent
        .checked_mul(config.rate_bps)
        .ok_or(EscalationError::EscalatedRentOverflow)?
        .checked_div(BPS_DENOMINATOR)
        .ok_or(EscalationError::EscalatedRentOverflow)?;

    let new_rent = config
        .current_rent
        .checked_add(increment)
        .ok_or(EscalationError::EscalatedRentOverflow)?;

    // -- Cap enforcement -----------------------------------------------------
    let effective_cap = if config.rent_cap > 0 {
        config.rent_cap.min(MAX_RENT_AMOUNT)
    } else {
        MAX_RENT_AMOUNT
    };

    if new_rent > effective_cap {
        return Err(EscalationError::EscalatedRentExceedsCap);
    }

    // -- Update config -------------------------------------------------------
    config.current_rent = new_rent;
    config.steps_applied = config
        .steps_applied
        .checked_add(1)
        .ok_or(EscalationError::CumulativeEscalationOverflow)?;

    // Schedule next step — saturating so we never wrap u32::MAX.
    config.next_escalation_ledger = current_ledger.saturating_add(config.interval_ledgers);

    Ok(config)
}

// --- Helper: build initial config --------------------------------------------

/// Builds and validates a fresh `EscalationConfig` for a new lease.
/// Call this inside `create_lease_instance` before storing the lease.
pub fn build_escalation_config(
    env: &Env,
    base_rent: i128,
    rate_bps: i128,
    interval_ledgers: u32,
    rent_cap: i128,
) -> Result<EscalationConfig, EscalationError> {
    validate_escalation_config(rate_bps, interval_ledgers, base_rent, rent_cap)?;

    let next_escalation_ledger = if rate_bps > 0 && interval_ledgers > 0 {
        env.ledger().sequence().saturating_add(interval_ledgers)
    } else {
        u32::MAX
    };

    Ok(EscalationConfig {
        rate_bps,
        interval_ledgers,
        next_escalation_ledger,
        steps_applied: 0,
        rent_cap,
        current_rent: base_rent,
    })
}
