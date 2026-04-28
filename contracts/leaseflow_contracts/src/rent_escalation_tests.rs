#![cfg(test)]

use soroban_sdk::Env;
use crate::rent_escalation::{
    apply_escalation, build_escalation_config, validate_escalation_config,
    EscalationError, MAX_ESCALATION_RATE_BPS, MAX_RENT_AMOUNT,
};

// --- validate_escalation_config tests ----------------------------------------

#[test]
fn test_fixed_rent_always_valid() {
    // rate_bps == 0 means fixed rent — all other params ignored
    assert!(validate_escalation_config(0, 0, 1_000, 0).is_ok());
    assert!(validate_escalation_config(0, 0, MAX_RENT_AMOUNT, 0).is_ok());
}

#[test]
fn test_valid_escalation_params() {
    // 3% rate, interval 1000 ledgers, cap at 500_000
    assert!(validate_escalation_config(300, 1_000, 10_000, 500_000).is_ok());
}

#[test]
fn test_reject_negative_rate() {
    let err = validate_escalation_config(-1, 1_000, 10_000, 0).unwrap_err();
    assert_eq!(err, EscalationError::EscalationRateNegative);
}

#[test]
fn test_reject_rate_above_max() {
    // MAX + 1 bps (50.01%)
    let err = validate_escalation_config(MAX_ESCALATION_RATE_BPS + 1, 1_000, 10_000, 0).unwrap_err();
    assert_eq!(err, EscalationError::EscalationRateTooHigh);
}

#[test]
fn test_accept_rate_exactly_at_max() {
    // Exactly 50% with a cap to avoid cumulative overflow rejection
    assert!(validate_escalation_config(MAX_ESCALATION_RATE_BPS, 1_000, 1_000, 1_000_000_000).is_ok());
}

#[test]
fn test_reject_zero_interval_when_rate_nonzero() {
    let err = validate_escalation_config(300, 0, 10_000, 0).unwrap_err();
    assert_eq!(err, EscalationError::EscalationIntervalZero);
}

#[test]
fn test_reject_base_rent_above_global_ceiling() {
    let err = validate_escalation_config(300, 1_000, MAX_RENT_AMOUNT + 1, 0).unwrap_err();
    assert_eq!(err, EscalationError::RentAmountExceedsMaxAllowed);
}

#[test]
fn test_reject_cap_below_base_rent() {
    // cap (5_000) < base_rent (10_000)
    let err = validate_escalation_config(300, 1_000, 10_000, 5_000).unwrap_err();
    assert_eq!(err, EscalationError::RentCapBelowBaseRent);
}

#[test]
fn test_reject_cumulative_overflow() {
    // MAX_RENT_AMOUNT at 50% for 120 steps overflows i128
    let err = validate_escalation_config(
        MAX_ESCALATION_RATE_BPS,
        1_000,
        MAX_RENT_AMOUNT,
        0,
    ).unwrap_err();
    assert_eq!(err, EscalationError::CumulativeEscalationOverflow);
}

// --- apply_escalation tests ---------------------------------------------------

#[test]
fn test_apply_escalation_single_step_correct_math() {
    let env = Env::default();
    let config = build_escalation_config(&env, 10_000, 1_000, 1, 200_000).unwrap();

    // Advance ledger so escalation is due
    env.ledger().set_sequence_number(env.ledger().sequence() + 2);

    let updated = apply_escalation(&env, config).unwrap();
    // 10_000 + 10_000 * 1000 / 10_000 = 10_000 + 1_000 = 11_000
    assert_eq!(updated.current_rent, 11_000);
    assert_eq!(updated.steps_applied, 1);
}

#[test]
fn test_apply_escalation_not_yet_due_returns_unchanged() {
    let env = Env::default();
    // Interval = 10_000 ledgers — not advancing time
    let config = build_escalation_config(&env, 10_000, 500, 10_000, 0).unwrap();
    let updated = apply_escalation(&env, config.clone()).unwrap();
    assert_eq!(updated.current_rent, config.current_rent);
    assert_eq!(updated.steps_applied, 0);
}

#[test]
fn test_apply_escalation_fixed_rent_unchanged() {
    let env = Env::default();
    let config = build_escalation_config(&env, 10_000, 0, 0, 0).unwrap();
    env.ledger().set_sequence_number(1_000_000);
    let updated = apply_escalation(&env, config).unwrap();
    assert_eq!(updated.current_rent, 10_000);
}

#[test]
fn test_apply_escalation_stops_at_max_steps() {
    let env = Env::default();
    use crate::rent_escalation::MAX_ESCALATION_STEPS;
    let mut config = build_escalation_config(&env, 1_000, 100, 1, 0).unwrap();
    config.steps_applied = MAX_ESCALATION_STEPS; // Simulate already at limit

    env.ledger().set_sequence_number(env.ledger().sequence() + 2);
    let updated = apply_escalation(&env, config.clone()).unwrap();
    // Should return unchanged
    assert_eq!(updated.current_rent, config.current_rent);
    assert_eq!(updated.steps_applied, MAX_ESCALATION_STEPS);
}

#[test]
fn test_apply_escalation_rejects_when_exceeds_cap() {
    let env = Env::default();
    // 50% rate, cap at 14_000 — step would produce 15_000 > cap
    let config = build_escalation_config(&env, 10_000, 5_000, 1, 14_000).unwrap();
    env.ledger().set_sequence_number(env.ledger().sequence() + 2);

    let err = apply_escalation(&env, config).unwrap_err();
    assert_eq!(err, EscalationError::EscalatedRentExceedsCap);
}

#[test]
fn test_apply_escalation_schedules_next_ledger_correctly() {
    let env = Env::default();
    let interval = 500u32;
    let config = build_escalation_config(&env, 10_000, 200, interval, 0).unwrap();

    let initial_seq = env.ledger().sequence();
    env.ledger().set_sequence_number(initial_seq + interval + 1);

    let updated = apply_escalation(&env, config).unwrap();
    // next_escalation_ledger should be current_ledger + interval
    let expected_next = (initial_seq + interval + 1).saturating_add(interval);
    assert_eq!(updated.next_escalation_ledger, expected_next);
}

// --- build_escalation_config tests -------------------------------------------

#[test]
fn test_build_config_propagates_validation_errors() {
    let env = Env::default();
    let err = build_escalation_config(&env, 10_000, -1, 1_000, 0).unwrap_err();
    assert_eq!(err, EscalationError::EscalationRateNegative);
}

#[test]
fn test_build_config_fixed_rent_sets_max_next_ledger() {
    let env = Env::default();
    let config = build_escalation_config(&env, 10_000, 0, 0, 0).unwrap();
    assert_eq!(config.next_escalation_ledger, u32::MAX);
}
