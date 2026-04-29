//! Bitwise compression for lease property attributes.
//!
//! Soroban persistent storage is rented per byte, so packing every boolean and
//! small-enum attribute that can fit in a few bits into a single integer is a
//! cheap and durable optimisation. Each separate `bool` field on a stored
//! struct costs at least one byte (often more after XDR padding); each enum
//! discriminant typically costs four. This module compresses **eight booleans
//! and five small enums** into a single `u32` (4 bytes), eliminating that
//! per-field overhead.
//!
//! # Bit layout
//!
//! ```text
//!  31              19 18    17 16    15 14    13 12    11 10     8 7      0
//! ┌──────────────────┬────────┬────────┬────────┬────────┬────────┬────────┐
//! │     reserved     │ damage │ asset  │ maint. │ deposit│ lease  │ flags  │
//! │     (12 bits)    │ sevty. │ cond.  │ status │ status │ status │ (8 b.) │
//! │                  │ (3 b.) │ (2 b.) │ (2 b.) │ (2 b.) │ (3 b.) │        │
//! └──────────────────┴────────┴────────┴────────┴────────┴────────┴────────┘
//! ```
//!
//! Bits 0–7 — boolean flags (one per bit), see [`flag`].
//! Bits 8–10 — `LeaseStatus` (6 variants).
//! Bits 11–12 — `DepositStatus` (3 variants).
//! Bits 13–14 — `MaintenanceStatus` (4 variants).
//! Bits 15–16 — `AssetCondition` (4 variants).
//! Bits 17–19 — `DamageSeverity` (6 variants).
//! Bits 20–31 — reserved for future attributes.
//!
//! All write operations clear their target field before OR-ing in the new
//! value, so `set_*` calls are idempotent and order-independent. All read
//! operations mask their field, so reserved bits never leak into observable
//! state.
//!
//! # Storage savings
//!
//! Replacing the 8 booleans + 5 enums on `LeaseInstance` with a single `u32`
//! saves roughly 28 XDR bytes per lease record. With persistent-entry rent
//! amortised across the network, that is the dominant win: every avoided
//! byte applies for the full lifetime of the lease.
//!
//! # Migration
//!
//! This module is additive. Call sites can adopt it incrementally by
//! replacing the legacy `lease_flags` `u8` first (its bits map to the low
//! byte of `PropertyAttributes` 1:1) and then folding enum fields into the
//! packed value at the next storage migration window.

use core::convert::TryFrom;

/// Boolean-flag bit positions (bits 0–7 of the packed `u32`).
///
/// The first six bits are byte-compatible with the legacy `lease_flags`
/// constants, so a `u8` flag value can be widened into a `PropertyAttributes`
/// without remapping bits.
pub mod flag {
    /// The lease is currently active.
    pub const ACTIVE: u32 = 1 << 0;
    /// The flat late-fee component has already been charged.
    pub const FLAT_FEE_APPLIED: u32 = 1 << 1;
    /// The lease has experienced at least one late payment.
    pub const HAD_LATE_PAYMENT: u32 = 1 << 2;
    /// A pet is registered against the lease.
    pub const HAS_PET: u32 = 1 << 3;
    /// Yield delegation to the protocol treasury is enabled.
    pub const YIELD_DELEGATION_ENABLED: u32 = 1 << 4;
    /// The lease is paused (rent accrual halted).
    pub const PAUSED: u32 = 1 << 5;
    /// Continuous (sub-monthly) billing is enabled.
    pub const CONTINUOUS_BILLING_ENABLED: u32 = 1 << 6;
    /// The tenant has completed KYC against the registered identity provider.
    pub const KYC_VERIFIED: u32 = 1 << 7;

    /// Mask covering all currently-defined flag bits.
    pub const ALL: u32 = ACTIVE
        | FLAT_FEE_APPLIED
        | HAD_LATE_PAYMENT
        | HAS_PET
        | YIELD_DELEGATION_ENABLED
        | PAUSED
        | CONTINUOUS_BILLING_ENABLED
        | KYC_VERIFIED;
}

// --- Field offsets and widths for the packed enum slots --------------------

const LEASE_STATUS_SHIFT: u32 = 8;
const LEASE_STATUS_BITS: u32 = 3;
const LEASE_STATUS_MASK: u32 = ((1 << LEASE_STATUS_BITS) - 1) << LEASE_STATUS_SHIFT;

const DEPOSIT_STATUS_SHIFT: u32 = 11;
const DEPOSIT_STATUS_BITS: u32 = 2;
const DEPOSIT_STATUS_MASK: u32 = ((1 << DEPOSIT_STATUS_BITS) - 1) << DEPOSIT_STATUS_SHIFT;

const MAINTENANCE_STATUS_SHIFT: u32 = 13;
const MAINTENANCE_STATUS_BITS: u32 = 2;
const MAINTENANCE_STATUS_MASK: u32 =
    ((1 << MAINTENANCE_STATUS_BITS) - 1) << MAINTENANCE_STATUS_SHIFT;

const ASSET_CONDITION_SHIFT: u32 = 15;
const ASSET_CONDITION_BITS: u32 = 2;
const ASSET_CONDITION_MASK: u32 =
    ((1 << ASSET_CONDITION_BITS) - 1) << ASSET_CONDITION_SHIFT;

const DAMAGE_SEVERITY_SHIFT: u32 = 17;
const DAMAGE_SEVERITY_BITS: u32 = 3;
const DAMAGE_SEVERITY_MASK: u32 =
    ((1 << DAMAGE_SEVERITY_BITS) - 1) << DAMAGE_SEVERITY_SHIFT;

/// Maximum legal discriminant for each packed enum.
///
/// Stored values above these maxima are rejected by [`PropertyAttributes::validate`]
/// and treated as data corruption.
pub const MAX_LEASE_STATUS: u8 = 5;
pub const MAX_DEPOSIT_STATUS: u8 = 2;
pub const MAX_MAINTENANCE_STATUS: u8 = 3;
pub const MAX_ASSET_CONDITION: u8 = 3;
pub const MAX_DAMAGE_SEVERITY: u8 = 5;

/// Reasons an attempted decode of a `PropertyAttributes` value can fail.
///
/// These map to invariant violations on the packed integer rather than user
/// error: a malformed value implies the storage cell was written by older or
/// misbehaving code, and callers should treat it the same as a deserialisation
/// failure on any other persistent record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropertyAttrError {
    /// `LeaseStatus` discriminant exceeded `MAX_LEASE_STATUS`.
    InvalidLeaseStatus,
    /// `DepositStatus` discriminant exceeded `MAX_DEPOSIT_STATUS`.
    InvalidDepositStatus,
    /// `MaintenanceStatus` discriminant exceeded `MAX_MAINTENANCE_STATUS`.
    InvalidMaintenanceStatus,
    /// `AssetCondition` discriminant exceeded `MAX_ASSET_CONDITION`.
    InvalidAssetCondition,
    /// `DamageSeverity` discriminant exceeded `MAX_DAMAGE_SEVERITY`.
    InvalidDamageSeverity,
    /// A reserved bit (bits 20–31) was set.
    ReservedBitSet,
}

/// Packed representation of every small-cardinality lease attribute.
///
/// Internally a single `u32`, so it serialises to 4 bytes in Soroban XDR
/// regardless of how many fields are populated. Construct with
/// [`PropertyAttributes::new`] and mutate via the typed setters; never poke
/// at the raw bits directly except through [`PropertyAttributes::raw`] /
/// [`PropertyAttributes::from_raw`] for migration shims.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PropertyAttributes(u32);

impl PropertyAttributes {
    /// Bits reserved for future attributes; setting any of these is rejected
    /// by [`Self::validate`].
    pub const RESERVED_MASK: u32 = !(flag::ALL
        | LEASE_STATUS_MASK
        | DEPOSIT_STATUS_MASK
        | MAINTENANCE_STATUS_MASK
        | ASSET_CONDITION_MASK
        | DAMAGE_SEVERITY_MASK);

    /// Construct an empty attribute set: every flag false, every enum at
    /// discriminant zero.
    pub const fn new() -> Self {
        Self(0)
    }

    /// Wrap a raw `u32` previously produced by [`Self::raw`].
    ///
    /// No validation is performed; pair with [`Self::validate`] when the
    /// source is untrusted (e.g. cross-contract input or legacy storage).
    pub const fn from_raw(bits: u32) -> Self {
        Self(bits)
    }

    /// Return the underlying packed integer for storage.
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Verify that the packed value contains no out-of-range enum
    /// discriminants and no reserved bits set.
    ///
    /// Returning `Ok(())` here means every typed getter on `self` is
    /// guaranteed to succeed.
    pub fn validate(self) -> Result<(), PropertyAttrError> {
        if self.0 & Self::RESERVED_MASK != 0 {
            return Err(PropertyAttrError::ReservedBitSet);
        }
        if self.lease_status_code() > MAX_LEASE_STATUS {
            return Err(PropertyAttrError::InvalidLeaseStatus);
        }
        if self.deposit_status_code() > MAX_DEPOSIT_STATUS {
            return Err(PropertyAttrError::InvalidDepositStatus);
        }
        if self.maintenance_status_code() > MAX_MAINTENANCE_STATUS {
            return Err(PropertyAttrError::InvalidMaintenanceStatus);
        }
        if self.asset_condition_code() > MAX_ASSET_CONDITION {
            return Err(PropertyAttrError::InvalidAssetCondition);
        }
        if self.damage_severity_code() > MAX_DAMAGE_SEVERITY {
            return Err(PropertyAttrError::InvalidDamageSeverity);
        }
        Ok(())
    }

    // --- Boolean flags -----------------------------------------------------

    /// `true` iff every bit in `mask` is set. `mask` should be one of the
    /// constants from [`flag`].
    #[inline]
    pub const fn has_flag(self, mask: u32) -> bool {
        self.0 & mask == mask
    }

    /// Set or clear every bit in `mask` according to `value`.
    #[inline]
    pub fn set_flag(&mut self, mask: u32, value: bool) {
        if value {
            self.0 |= mask;
        } else {
            self.0 &= !mask;
        }
    }

    /// Toggle every bit in `mask`.
    #[inline]
    pub fn toggle_flag(&mut self, mask: u32) {
        self.0 ^= mask;
    }

    // --- Packed enum accessors --------------------------------------------

    #[inline]
    fn write_field(&mut self, mask: u32, shift: u32, code: u8) {
        self.0 = (self.0 & !mask) | (((code as u32) << shift) & mask);
    }

    #[inline]
    const fn read_field(self, mask: u32, shift: u32) -> u8 {
        ((self.0 & mask) >> shift) as u8
    }

    /// Raw discriminant of the packed `LeaseStatus`.
    #[inline]
    pub const fn lease_status_code(self) -> u8 {
        self.read_field(LEASE_STATUS_MASK, LEASE_STATUS_SHIFT)
    }

    /// Pack a `LeaseStatus` discriminant. Values above [`MAX_LEASE_STATUS`]
    /// are rejected so we never write a value that [`Self::validate`] would
    /// later flag as corruption.
    pub fn set_lease_status_code(&mut self, code: u8) -> Result<(), PropertyAttrError> {
        if code > MAX_LEASE_STATUS {
            return Err(PropertyAttrError::InvalidLeaseStatus);
        }
        self.write_field(LEASE_STATUS_MASK, LEASE_STATUS_SHIFT, code);
        Ok(())
    }

    #[inline]
    pub const fn deposit_status_code(self) -> u8 {
        self.read_field(DEPOSIT_STATUS_MASK, DEPOSIT_STATUS_SHIFT)
    }

    pub fn set_deposit_status_code(&mut self, code: u8) -> Result<(), PropertyAttrError> {
        if code > MAX_DEPOSIT_STATUS {
            return Err(PropertyAttrError::InvalidDepositStatus);
        }
        self.write_field(DEPOSIT_STATUS_MASK, DEPOSIT_STATUS_SHIFT, code);
        Ok(())
    }

    #[inline]
    pub const fn maintenance_status_code(self) -> u8 {
        self.read_field(MAINTENANCE_STATUS_MASK, MAINTENANCE_STATUS_SHIFT)
    }

    pub fn set_maintenance_status_code(&mut self, code: u8) -> Result<(), PropertyAttrError> {
        if code > MAX_MAINTENANCE_STATUS {
            return Err(PropertyAttrError::InvalidMaintenanceStatus);
        }
        self.write_field(MAINTENANCE_STATUS_MASK, MAINTENANCE_STATUS_SHIFT, code);
        Ok(())
    }

    #[inline]
    pub const fn asset_condition_code(self) -> u8 {
        self.read_field(ASSET_CONDITION_MASK, ASSET_CONDITION_SHIFT)
    }

    pub fn set_asset_condition_code(&mut self, code: u8) -> Result<(), PropertyAttrError> {
        if code > MAX_ASSET_CONDITION {
            return Err(PropertyAttrError::InvalidAssetCondition);
        }
        self.write_field(ASSET_CONDITION_MASK, ASSET_CONDITION_SHIFT, code);
        Ok(())
    }

    #[inline]
    pub const fn damage_severity_code(self) -> u8 {
        self.read_field(DAMAGE_SEVERITY_MASK, DAMAGE_SEVERITY_SHIFT)
    }

    pub fn set_damage_severity_code(&mut self, code: u8) -> Result<(), PropertyAttrError> {
        if code > MAX_DAMAGE_SEVERITY {
            return Err(PropertyAttrError::InvalidDamageSeverity);
        }
        self.write_field(DAMAGE_SEVERITY_MASK, DAMAGE_SEVERITY_SHIFT, code);
        Ok(())
    }
}

impl From<u32> for PropertyAttributes {
    fn from(bits: u32) -> Self {
        Self::from_raw(bits)
    }
}

impl From<PropertyAttributes> for u32 {
    fn from(attrs: PropertyAttributes) -> Self {
        attrs.raw()
    }
}

/// Widen the legacy 6-bit `lease_flags` `u8` into a `PropertyAttributes`.
///
/// Bits 0–5 of the legacy mask line up with [`flag::ACTIVE`] through
/// [`flag::PAUSED`], so this is a straight zero-extension. Reserved bits 6–7
/// in the legacy `u8` are masked off, matching their previous "unused" status.
impl From<u8> for PropertyAttributes {
    fn from(legacy_flags: u8) -> Self {
        const LEGACY_MASK: u32 = (1 << 6) - 1;
        Self(legacy_flags as u32 & LEGACY_MASK)
    }
}

/// Try to coerce a `u8` into a `PropertyAttributes` rejecting any bits above
/// the legacy-defined six.
impl TryFrom<&[u8]> for PropertyAttributes {
    type Error = PropertyAttrError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() != 4 {
            return Err(PropertyAttrError::ReservedBitSet);
        }
        let raw = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let attrs = Self(raw);
        attrs.validate()?;
        Ok(attrs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_all_zero() {
        let a = PropertyAttributes::new();
        assert_eq!(a.raw(), 0);
        assert!(!a.has_flag(flag::ACTIVE));
        assert_eq!(a.lease_status_code(), 0);
        assert_eq!(a.deposit_status_code(), 0);
        assert_eq!(a.maintenance_status_code(), 0);
        assert_eq!(a.asset_condition_code(), 0);
        assert_eq!(a.damage_severity_code(), 0);
        assert!(a.validate().is_ok());
    }

    #[test]
    fn flag_set_and_clear_round_trip() {
        let mut a = PropertyAttributes::new();
        a.set_flag(flag::ACTIVE, true);
        a.set_flag(flag::HAS_PET, true);
        assert!(a.has_flag(flag::ACTIVE));
        assert!(a.has_flag(flag::HAS_PET));
        assert!(!a.has_flag(flag::PAUSED));

        a.set_flag(flag::HAS_PET, false);
        assert!(!a.has_flag(flag::HAS_PET));
        assert!(a.has_flag(flag::ACTIVE));
    }

    #[test]
    fn flag_set_is_idempotent() {
        let mut a = PropertyAttributes::new();
        a.set_flag(flag::PAUSED, true);
        let after_first = a.raw();
        a.set_flag(flag::PAUSED, true);
        assert_eq!(a.raw(), after_first);
    }

    #[test]
    fn toggle_flag_reverses_state() {
        let mut a = PropertyAttributes::new();
        a.toggle_flag(flag::KYC_VERIFIED);
        assert!(a.has_flag(flag::KYC_VERIFIED));
        a.toggle_flag(flag::KYC_VERIFIED);
        assert!(!a.has_flag(flag::KYC_VERIFIED));
    }

    #[test]
    fn enum_setters_round_trip() {
        let mut a = PropertyAttributes::new();
        a.set_lease_status_code(MAX_LEASE_STATUS).unwrap();
        a.set_deposit_status_code(MAX_DEPOSIT_STATUS).unwrap();
        a.set_maintenance_status_code(MAX_MAINTENANCE_STATUS).unwrap();
        a.set_asset_condition_code(MAX_ASSET_CONDITION).unwrap();
        a.set_damage_severity_code(MAX_DAMAGE_SEVERITY).unwrap();

        assert_eq!(a.lease_status_code(), MAX_LEASE_STATUS);
        assert_eq!(a.deposit_status_code(), MAX_DEPOSIT_STATUS);
        assert_eq!(a.maintenance_status_code(), MAX_MAINTENANCE_STATUS);
        assert_eq!(a.asset_condition_code(), MAX_ASSET_CONDITION);
        assert_eq!(a.damage_severity_code(), MAX_DAMAGE_SEVERITY);
        assert!(a.validate().is_ok());
    }

    #[test]
    fn enum_fields_are_independent() {
        // Set every enum to its max and every flag to true, then verify the
        // bit fields don't bleed into each other.
        let mut a = PropertyAttributes::new();
        for f in [
            flag::ACTIVE,
            flag::FLAT_FEE_APPLIED,
            flag::HAD_LATE_PAYMENT,
            flag::HAS_PET,
            flag::YIELD_DELEGATION_ENABLED,
            flag::PAUSED,
            flag::CONTINUOUS_BILLING_ENABLED,
            flag::KYC_VERIFIED,
        ] {
            a.set_flag(f, true);
        }
        a.set_lease_status_code(MAX_LEASE_STATUS).unwrap();
        a.set_deposit_status_code(MAX_DEPOSIT_STATUS).unwrap();
        a.set_maintenance_status_code(MAX_MAINTENANCE_STATUS).unwrap();
        a.set_asset_condition_code(MAX_ASSET_CONDITION).unwrap();
        a.set_damage_severity_code(MAX_DAMAGE_SEVERITY).unwrap();

        // Now flip one field and verify nothing else changes.
        a.set_lease_status_code(0).unwrap();
        assert_eq!(a.lease_status_code(), 0);
        assert_eq!(a.deposit_status_code(), MAX_DEPOSIT_STATUS);
        assert_eq!(a.maintenance_status_code(), MAX_MAINTENANCE_STATUS);
        assert_eq!(a.asset_condition_code(), MAX_ASSET_CONDITION);
        assert_eq!(a.damage_severity_code(), MAX_DAMAGE_SEVERITY);
        assert_eq!(a.raw() & flag::ALL, flag::ALL);
    }

    #[test]
    fn out_of_range_enum_values_are_rejected() {
        let mut a = PropertyAttributes::new();
        assert_eq!(
            a.set_lease_status_code(MAX_LEASE_STATUS + 1),
            Err(PropertyAttrError::InvalidLeaseStatus)
        );
        assert_eq!(
            a.set_deposit_status_code(MAX_DEPOSIT_STATUS + 1),
            Err(PropertyAttrError::InvalidDepositStatus)
        );
        assert_eq!(
            a.set_maintenance_status_code(MAX_MAINTENANCE_STATUS + 1),
            Err(PropertyAttrError::InvalidMaintenanceStatus)
        );
        assert_eq!(
            a.set_asset_condition_code(MAX_ASSET_CONDITION + 1),
            Err(PropertyAttrError::InvalidAssetCondition)
        );
        assert_eq!(
            a.set_damage_severity_code(MAX_DAMAGE_SEVERITY + 1),
            Err(PropertyAttrError::InvalidDamageSeverity)
        );
        // Failed sets must leave the value unchanged.
        assert_eq!(a.raw(), 0);
    }

    #[test]
    fn validate_catches_reserved_bits() {
        let bad = PropertyAttributes::from_raw(1u32 << 20);
        assert_eq!(bad.validate(), Err(PropertyAttrError::ReservedBitSet));
    }

    #[test]
    fn validate_catches_corrupt_enum_codes() {
        // Manually set the lease_status field to an illegal 6.
        let bad = PropertyAttributes::from_raw((6u32) << LEASE_STATUS_SHIFT);
        assert_eq!(bad.validate(), Err(PropertyAttrError::InvalidLeaseStatus));
    }

    #[test]
    fn legacy_u8_widening_preserves_low_bits() {
        // Legacy bit pattern: ACTIVE | HAS_PET | PAUSED  = 0b0010_1001 = 0x29
        let legacy: u8 = 0b0010_1001;
        let widened: PropertyAttributes = legacy.into();
        assert!(widened.has_flag(flag::ACTIVE));
        assert!(widened.has_flag(flag::HAS_PET));
        assert!(widened.has_flag(flag::PAUSED));
        assert!(!widened.has_flag(flag::CONTINUOUS_BILLING_ENABLED));
        assert!(widened.validate().is_ok());
    }

    #[test]
    fn raw_round_trip_is_lossless() {
        let mut a = PropertyAttributes::new();
        a.set_flag(flag::ACTIVE, true);
        a.set_flag(flag::CONTINUOUS_BILLING_ENABLED, true);
        a.set_lease_status_code(3).unwrap();
        a.set_damage_severity_code(4).unwrap();

        let bits = a.raw();
        let copy = PropertyAttributes::from_raw(bits);
        assert_eq!(a, copy);
    }

    #[test]
    fn reserved_mask_only_covers_unused_bits() {
        // Every documented field bit must be outside RESERVED_MASK.
        let used = flag::ALL
            | LEASE_STATUS_MASK
            | DEPOSIT_STATUS_MASK
            | MAINTENANCE_STATUS_MASK
            | ASSET_CONDITION_MASK
            | DAMAGE_SEVERITY_MASK;
        assert_eq!(used & PropertyAttributes::RESERVED_MASK, 0);
        // And the two together cover every bit.
        assert_eq!(used | PropertyAttributes::RESERVED_MASK, u32::MAX);
    }

    #[test]
    fn storage_footprint_is_four_bytes() {
        // The sole point of this module: the packed value is 4 bytes.
        assert_eq!(core::mem::size_of::<PropertyAttributes>(), 4);
    }
}
