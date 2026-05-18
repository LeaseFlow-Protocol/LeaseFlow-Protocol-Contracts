#![no_std]
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, symbol_short, 
    Address, Env, String, Symbol, BytesN
};

// Re-export the pure math function so contract callers and tests can use it.
// pub use leaseflow_math::calculate_total_cost; // Only if available in dependencies

// ---------------------------------------------------------------------------
// Existing simple Lease struct (preserved for backwards compatibility)
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lease {
    pub landlord: Address,
    pub tenant: Address,
    pub amount: i128,
    pub active: bool,
    /// Optional price at which the tenant can buy out the asset.
    pub buyout_price: Option<i128>,
    /// Total cumulative payments made by the tenant.
    pub cumulative_payments: i128,
}

macro_rules! require {
    ($condition:expr, $error_msg:expr) => {
        if !$condition {
            panic!($error_msg);
        }
    };
}

// ── Rate helpers ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RateType {
    PerSecond,
    PerHour,
    PerDay,
}

pub fn to_per_second(rate: i128, rate_type: RateType) -> i128 {
    match rate_type {
        RateType::PerSecond => rate,
        RateType::PerHour   => rate / 3_600,
        RateType::PerDay    => rate / 86_400,
    }
}

pub const SECS_PER_UNIT: u64 = 86_400;

// ── Status Enums ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DepositStatus {
    Held,
    Settled,
    Disputed,
}

/// Usage rights for NFT renters during lease period
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageRights {
    pub renter: Address,
    pub nft_contract: Address,
    pub token_id: u128,
    pub lease_id: Symbol,
    pub valid_until: u64,
}

/// Lease lifecycle status
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LeaseStatus {
    Pending,
    Active,
    Expired,
    Disputed,
    Terminated,
}

// ── Structs ───────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseInstance {
    pub landlord: Address,
    pub tenant: Address,
    pub rent_amount: i128,
    pub deposit_amount: i128,
    /// Additional security deposit for damage protection in stroops.
    pub security_deposit: i128,
    /// Unix timestamp: lease start.
    pub start_date: u64,
    pub end_date: u64,
    pub status: LeaseStatus,
    pub nft_contract: Option<Address>,
    pub token_id: Option<u128>,
    pub active: bool,
    pub rent_paid: i128,
    pub expiry_time: u64,
    /// IPFS / HTTP URI pointing to the off-chain lease document.
    pub property_uri: String,
    /// Optional price at which the tenant can buy out the asset.
    pub buyout_price: Option<i128>,
    /// Total cumulative payments made by the tenant.
    pub cumulative_payments: i128,
    pub debt: i128,
    pub rent_per_sec: i128,
    pub late_fee_per_sec: i128,
    pub grace_period_end: u64,
    pub late_fee_flat: i128,
    pub flat_fee_applied: bool,
    pub seconds_late_charged: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Receipt {
    pub lease_id: Symbol,
    pub month: u32,
    pub amount: i128,
    pub date: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseAmendment {
    pub new_rent_amount: Option<i128>,
    pub new_end_date: Option<u64>,
    pub landlord_signature: BytesN<32>,
    pub tenant_signature: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateLeaseParams {
    pub tenant: Address,
    pub rent_amount: i128,
    pub deposit_amount: i128,
    pub security_deposit: i128,
    pub start_date: u64,
    pub end_date: u64,
    pub property_uri: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepositReleasePartial {
    pub tenant_amount: i128,
    pub landlord_amount: i128,
}

#[contracttype]
pub enum DepositRelease {
    FullRefund,
    PartialRefund(DepositReleasePartial),
    Disputed,
}

// ── Events ────────────────────────────────────────────────────────────────────

#[contractevent]
pub struct LeaseTerminated {
    pub lease_id: Symbol,
}

/// Emitted when a lease starts and the asset becomes available to the renter.
#[contractevent]
pub struct LeaseStarted {
    pub id: u64,
    pub renter: Address,
    pub rate: i128,
}

/// Emitted when a lease ends and total payment information is available.
#[contractevent]
pub struct LeaseEnded {
    pub id: u64,
    pub duration: u64,
    pub total_paid: i128,
}

/// Emitted when an asset is reclaimed by the landlord or system.
#[contractevent]
pub struct AssetReclaimed {
    pub id: u64,
    pub reason: String,
}

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Lease(Symbol),
    Receipt(Symbol, u32),
    Admin,
    /// Usage rights for NFT renters.
    UsageRights(Address, u128),
}

// ── Error enum ──────────────────────────────────────────────────────────────

/// All errors that can be returned by LeaseContract entry points.
#[contracterror]
#[derive(Debug, Clone, PartialEq)]
pub enum LeaseError {
    LeaseNotFound = 1,
    LeaseNotExpired = 2,
    RentOutstanding = 3,
    DepositNotSettled = 4,
    Unauthorised = 5,
    InvalidDeduction = 6,
    NftTransferFailed = 7,
    NftNotReturned = 8,
    UsageRightsNotFound = 9,
    UsageRightsExpired = 10,
}

// ── Storage Helpers ───────────────────────────────────────────────────────────

const DAY_IN_LEDGERS: u32 = 17280; // Assuming 5s ledger time
const MONTH_IN_LEDGERS: u32 = DAY_IN_LEDGERS * 30;
const YEAR_IN_LEDGERS: u32 = DAY_IN_LEDGERS * 365;

/// Fetch UsageRights from storage, or None.
pub fn load_usage_rights(env: &Env, nft_contract: Address, token_id: u128) -> Option<UsageRights> {
    env.storage().instance().get(&DataKey::UsageRights(nft_contract, token_id))
}

/// Save UsageRights to storage.
pub fn save_usage_rights(env: &Env, nft_contract: Address, token_id: u128, usage_rights: &UsageRights) {
    env.storage()
        .instance()
        .set(&DataKey::UsageRights(nft_contract, token_id), usage_rights);
}

/// Removes UsageRights from storage.
pub fn delete_usage_rights(env: &Env, nft_contract: Address, token_id: u128) {
    env.storage().instance().remove(&DataKey::UsageRights(nft_contract, token_id));
}

/// Fetch a LeaseInstance from persistent storage, or None.
pub fn load_lease(env: &Env, lease_id: &Symbol) -> Option<LeaseInstance> {
    env.storage().persistent().get(&DataKey::Lease(lease_id.clone()))
}

pub fn save_lease(env: &Env, lease_id: &Symbol, lease: &LeaseInstance) {
    let key = DataKey::Lease(lease_id.clone());
    env.storage().persistent().set(&key, lease);
    // identities stored in Persistent storage to survive ledger expirations
    env.storage().persistent().extend_ttl(&key, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);
}

mod nft_contract {
    use soroban_sdk::{contractclient, Address, Env};
    #[contractclient(name = "NftClient")]
    pub trait NftInterface {
        fn transfer_from(env: Env, spender: Address, from: Address, to: Address, token_id: u128);
    }
}

// ── Contract Implementation ───────────────────────────────────────────────────

#[contract]
pub struct LeaseContract;

#[contractimpl]
impl LeaseContract {
    /// Returns the lease stored under `lease_id`.
    pub fn get_lease(env: Env, lease_id: Symbol) -> LeaseInstance {
        env.storage()
            .persistent()
            .get(&DataKey::Lease(lease_id))
            .expect("Lease not found")
    }

    /// Creates a lease **and** immediately transfers an NFT from landlord to
    /// contract escrow. Rate inputs follow the same `RateType` convention.
    pub fn create_lease_with_nft(
        env: Env,
        lease_id: Symbol,
        landlord: Address,
        tenant: Address,
        rent_amount: i128,
        rent_rate_type: RateType,
        duration: u64,
        grace_period_end: u64,
        late_fee_flat: i128,
        late_fee_amount: i128,
        late_fee_rate_type: RateType,
        nft_contract_addr: Address,
        token_id: u128,
    ) -> Symbol {
        landlord.require_auth();

        let nft_client = nft_contract::NftClient::new(&env, &nft_contract_addr);
        // Transfer NFT to contract escrow instead of directly to tenant
        nft_client.transfer_from(
            &env.current_contract_address(),
            &landlord,
            &env.current_contract_address(),
            &token_id,
        );

        let now = env.ledger().timestamp();
        let expiry_time = now.saturating_add(duration);

        let lease = LeaseInstance {
            landlord: landlord.clone(),
            tenant: tenant.clone(),
            rent_amount,
            rent_per_sec: to_per_second(rent_amount, rent_rate_type.clone()),
            late_fee_per_sec: to_per_second(late_fee_amount, late_fee_rate_type),
            deposit_amount: 0,
            security_deposit: 0,
            start_date: now,
            end_date: expiry_time,
            property_uri: String::from_str(&env, ""),
            status: LeaseStatus::Active,
            nft_contract: Some(nft_contract_addr.clone()),
            token_id: Some(token_id),
            active: true,
            grace_period_end,
            late_fee_flat,
            debt: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            rent_paid: 0,
            expiry_time,
            buyout_price: None,
            cumulative_payments: 0,
        };

        // Grant usage rights to the tenant for the lease duration
        let usage_rights = UsageRights {
            renter: tenant.clone(),
            nft_contract: nft_contract_addr.clone(),
            token_id,
            lease_id: lease_id.clone(),
            valid_until: expiry_time,
        };
        save_usage_rights(&env, nft_contract_addr, token_id, &usage_rights);

        save_lease(&env, &lease_id, &lease);
        
        // Emit LeaseStarted event
        LeaseStarted {
            id: now,
            renter: tenant,
            rate: lease.rent_per_sec,
        }.publish(&env);
        
        symbol_short!("created")
    }

    /// Ends a lease and returns the NFT from contract escrow to the landlord.
    /// Only the landlord or tenant can call this function.
    pub fn end_lease(env: Env, lease_id: Symbol, caller: Address) -> Symbol {
        let lease = Self::get_lease(env.clone(), lease_id.clone());
        
        // Authorization: only landlord or tenant can end the lease
        require!(
            lease.landlord == caller || lease.tenant == caller,
            "Unauthorized: Only landlord or tenant can end lease"
        );
        caller.require_auth();
        
        // Check if NFT is associated with this lease
        if let (Some(nft_contract_addr), Some(token_id)) = (lease.nft_contract.clone(), lease.token_id) {
            // Remove usage rights first
            delete_usage_rights(&env, nft_contract_addr.clone(), token_id);
            
            // Transfer NFT back to landlord from escrow
            let nft_client = nft_contract::NftClient::new(&env, &nft_contract_addr);
            nft_client.transfer_from(
                &env.current_contract_address(),
                &env.current_contract_address(),
                &lease.landlord,
                &token_id,
            );
        }
        
        // Update lease status to terminated
        let mut updated_lease = lease;
        updated_lease.status = LeaseStatus::Terminated;
        updated_lease.active = false;
        
        save_lease(&env, &lease_id, &updated_lease);
        
        // Emit event
        LeaseTerminated { lease_id }.publish(&env);
        
        symbol_short!("ended")
    }

    /// Activates a pending lease after the security deposit has been received.
    pub fn activate_lease(env: Env, lease_id: Symbol, tenant: Address) -> Symbol {
        let mut lease = Self::get_lease(env.clone(), lease_id.clone());

        require!(lease.tenant == tenant, "Unauthorized: Only tenant can activate lease");
        require!(lease.status == LeaseStatus::Pending, "Lease is not in pending state");

        lease.status = LeaseStatus::Active;

        save_lease(&env, &lease_id, &lease);
        
        // Emit LeaseStarted event for frontend notification
        let event_id = env.ledger().timestamp();
        LeaseStarted {
            id: event_id,
            renter: tenant,
            rate: lease.rent_per_sec,
        }.publish(&env);
        
        symbol_short!("active")
    }

    /// Updates the property metadata URI.
    pub fn update_property_uri(
        env: Env,
        lease_id: Symbol,
        landlord: Address,
        property_uri: String,
    ) -> Symbol {
        let mut lease = Self::get_lease(env.clone(), lease_id.clone());

        require!(
            lease.landlord == landlord,
            "Unauthorized: Only landlord can update property URI"
        );
        lease.property_uri = property_uri;

        save_lease(&env, &lease_id, &lease);
        symbol_short!("updated")
    }

    /// Amends a lease with both landlord and tenant signatures.
    pub fn amend_lease(env: Env, lease_id: Symbol, amendment: LeaseAmendment) -> Symbol {
        let mut lease = Self::get_lease(env.clone(), lease_id.clone());

        require!(lease.status == LeaseStatus::Active, "Can only amend active leases");

        if let Some(new_rent) = amendment.new_rent_amount {
            lease.rent_amount = new_rent;
        }
        if let Some(new_end_date) = amendment.new_end_date {
            lease.end_date = new_end_date;
        }

        save_lease(&env, &lease_id, &lease);
        symbol_short!("amended")
    }

    /// Releases the security deposit according to `release_type`.
    pub fn release_deposit(
        env: Env,
        lease_id: Symbol,
        release_type: DepositRelease,
    ) -> Symbol {
        let lease = Self::get_lease(env.clone(), lease_id.clone());

        require!(
            lease.status == LeaseStatus::Active || lease.status == LeaseStatus::Expired,
            "Can only release deposit from active or expired leases"
        );

        match release_type {
            DepositRelease::FullRefund => symbol_short!("full_ref"),
            DepositRelease::PartialRefund(partial) => {
                require!(
                    partial.tenant_amount + partial.landlord_amount == lease.deposit_amount,
                    "Amounts must sum to total deposit"
                );
                symbol_short!("partial")
            }
            DepositRelease::Disputed => {
                let mut updated = lease;
                updated.status = LeaseStatus::Disputed;
                save_lease(&env, &lease_id, &updated);
                symbol_short!("disputed")
            }
        }
    }

    /// Checks if a given address has usage rights for a specific NFT.
    pub fn check_usage_rights(env: Env, nft_contract: Address, token_id: u128, user: Address) -> Option<UsageRights> {
        if let Some(usage_rights) = load_usage_rights(&env, nft_contract, token_id) {
            let current_time = env.ledger().timestamp();
            
            // Check if the user is the renter and the rights haven't expired
            if usage_rights.renter == user && current_time <= usage_rights.valid_until {
                return Some(usage_rights);
            }
        }
        None
    }

    /// Processes a rent payment with late fee accrual.
    pub fn pay_rent(env: Env, lease_id: Symbol, payment_amount: i128) -> Symbol {
        let mut lease = Self::get_lease(env.clone(), lease_id.clone());
        require!(lease.active, "Lease is not active");

        let current_time = env.ledger().timestamp();

        // Accrue late fees (all in per-second units)
        if current_time > lease.grace_period_end {
            let seconds_late = current_time - lease.grace_period_end;

            // One-time flat fee applied on the first overdue second.
            if !lease.flat_fee_applied {
                lease.debt += lease.late_fee_flat;
                lease.flat_fee_applied = true;
            }

            // Per-second fee: only charge newly elapsed seconds.
            if seconds_late > lease.seconds_late_charged {
                let newly_accrued = seconds_late - lease.seconds_late_charged;
                lease.debt += (newly_accrued as i128) * lease.late_fee_per_sec;
                lease.seconds_late_charged = seconds_late;
            }
        }
        
        // Apply payment
        lease.rent_paid += payment_amount;
        lease.cumulative_payments += payment_amount;
        
        // Check for buyout completion
        if let Some(buyout_price) = lease.buyout_price {
            if lease.cumulative_payments >= buyout_price {
                lease.active = false;
                lease.status = LeaseStatus::Terminated;
            }
        }

        save_lease(&env, &lease_id, &lease);
        symbol_short!("paid")
    }

    /// Processes rent payment with receipt and saves receipt in storage.
    pub fn pay_rent_with_receipt(env: Env, lease_id: Symbol, month: u32, amount: i128) -> bool {
        let mut lease = load_lease(&env, &lease_id).expect("Lease not found");
        lease.tenant.require_auth();

        // Create receipt
        let receipt = Receipt {
            lease_id: lease_id.clone(),
            month,
            amount,
            date: env.ledger().timestamp(),
        };
        
        // Save receipt
        env.storage()
            .instance()
            .set(&DataKey::Receipt(lease_id.clone(), month), &receipt);
            
        // Update lease
        lease.rent_paid += amount;
        lease.cumulative_payments += amount;
        save_lease(&env, &lease_id, &lease);
        
        true
    }

    /// Sets the buyout price for a lease. Can only be called by the landlord.
    pub fn set_buyout_price(env: Env, lease_id: Symbol, landlord: Address, buyout_price: i128) -> Symbol {
        let mut lease = Self::get_lease(env.clone(), lease_id.clone());
        
        require!(
            lease.landlord == landlord,
            "Unauthorized: Only landlord can set buyout price"
        );
        require!(buyout_price > 0, "Buyout price must be positive");
        
        lease.buyout_price = Some(buyout_price);
        
        save_lease(&env, &lease_id, &lease);
        symbol_short!("buyout_ok")
    }

    /// Gets the buyout price for a lease.
    pub fn get_buyout_price(env: Env, lease_id: Symbol) -> Option<i128> {
        let lease = Self::get_lease(env, lease_id);
        lease.buyout_price
    }

    /// Terminates a lease (admin or landlord function).
    pub fn terminate_lease(
        env: Env,
        lease_id: Symbol,
        caller: Address,
    ) -> Result<(), LeaseError> {
        // Load lease
        let lease = load_lease(&env, &lease_id).ok_or(LeaseError::LeaseNotFound)?;

        // Authorization
        let is_landlord = caller == lease.landlord;
        let is_tenant = caller == lease.tenant;
        let is_admin = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::Admin)
            .map(|admin| admin == caller)
            .unwrap_or(false);

        if !is_landlord && !is_tenant && !is_admin {
            return Err(LeaseError::Unauthorised);
        }
        
        caller.require_auth();

        // State cleanup
        env.storage().persistent().remove(&DataKey::Lease(lease_id.clone()));
        
        // Emit termination event
        LeaseTerminated { lease_id }.publish(&env);
        
        // Emit LeaseEnded event
        let lease_duration = lease.end_date.saturating_sub(lease.start_date);
        LeaseEnded {
            id: env.ledger().timestamp(),
            duration: lease_duration,
            total_paid: lease.rent_paid,
        }.publish(&env);

        Ok(())
    }

    /// Reclaims an asset from a non-compliant lease.
    pub fn reclaim_asset(
        env: Env,
        lease_id: Symbol,
        caller: Address,
        reason: String,
    ) -> Result<(), LeaseError> {
        let lease = Self::get_lease(env.clone(), lease_id.clone());
        
        // Authorization check
        let is_landlord = caller == lease.landlord;
        let is_admin = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::Admin)
            .map(|admin| admin == caller)
            .unwrap_or(false);
            
        require!(
            is_landlord || is_admin,
            "Unauthorized: Only landlord or admin can reclaim asset"
        );
        
        caller.require_auth();

        // Emit AssetReclaimed event
        let event_id = env.ledger().timestamp() as u64;
        AssetReclaimed {
            id: event_id,
            reason: reason.clone(),
        }.publish(&env);

        Ok(())
    }

    /// Concludes a lease and processes security deposit refund.
    pub fn conclude_lease(
        env: Env,
        lease_id: Symbol,
        landlord: Address,
        damage_deduction: i128,
    ) -> Result<i128, LeaseError> {
        let lease = load_lease(&env, &lease_id).ok_or(LeaseError::LeaseNotFound)?;
        
        // Authorization
        if lease.landlord != landlord {
            return Err(LeaseError::Unauthorised);
        }
        
        landlord.require_auth();
        
        // Validate deduction
        if damage_deduction < 0 || damage_deduction > lease.security_deposit {
            return Err(LeaseError::InvalidDeduction);
        }
        
        // Calculate refund
        let refund_amount = lease.security_deposit - damage_deduction;
        
        // Update lease
        let mut updated_lease = lease;
        updated_lease.status = LeaseStatus::Terminated;
        updated_lease.active = false;
        save_lease(&env, &lease_id, &updated_lease);
        
        // Extend TTL
        env.storage().instance().extend_ttl(MONTH_IN_LEDGERS, YEAR_IN_LEDGERS);
        
        Ok(refund_amount)
    }

    /// Creates a full LeaseInstance keyed by lease_id.
    pub fn create_lease_instance(
        env: Env,
        lease_id: Symbol,
        landlord: Address,
        params: CreateLeaseParams,
    ) -> Result<(), LeaseError> {
        landlord.require_auth();
        
        let lease = LeaseInstance {
            landlord: landlord.clone(),
            tenant: params.tenant.clone(),
            rent_amount: params.rent_amount,
            deposit_amount: params.deposit_amount,
            security_deposit: params.security_deposit,
            start_date: params.start_date,
            end_date: params.end_date,
            status: LeaseStatus::Pending,
            property_uri: params.property_uri,
            rent_per_sec: 0,
            grace_period_end: params.end_date,
            late_fee_flat: 0,
            late_fee_per_sec: 0,
            debt: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            rent_paid: 0,
            expiry_time: params.end_date,
            nft_contract: None,
            token_id: None,
            active: true,
            buyout_price: None,
            cumulative_payments: 0,
        };
        
        save_lease(&env, &lease_id, &lease);
        
        // Keep the contract "alive" for the duration of the lease
        env.storage().instance().extend_ttl(MONTH_IN_LEDGERS, YEAR_IN_LEDGERS);
        
        Ok(())
    }

    /// Gets a receipt for a specific month.
    pub fn get_receipt(env: Env, lease_id: Symbol, month: u32) -> Receipt {
        env.storage()
            .instance()
            .get(&DataKey::Receipt(lease_id, month))
            .expect("Receipt not found")
    }

    /// Extends the TTL for a lease.
    pub fn extend_ttl(env: Env, lease_id: Symbol) {
        let key = DataKey::Lease(lease_id);
        if env.storage().persistent().has(&key) {
            env.storage().persistent().extend_ttl(&key, MONTH_IN_LEDGERS, YEAR_IN_LEDGERS);
        }
    }
}
