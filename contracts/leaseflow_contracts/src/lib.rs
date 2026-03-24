#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol, BytesN, String};

macro_rules! require {
    ($condition:expr, $error_msg:expr) => {
        if !$condition {
            panic!($error_msg);
        }
    };
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LeaseStatus {
    Pending,
    Active,
    Expired,
    Disputed,
}
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short,
    Address, Env, Symbol,
};

mod nft_contract {
    use soroban_sdk::{contractclient, Address, Env};
    

    #[allow(dead_code)]
    #[contractclient(name = "NftClient")]
    pub trait NftInterface {
        fn transfer_from(env: Env, spender: Address, from: Address, to: Address, token_id: u128);
    }
}

/// Seconds of lease time granted per unit of funds added (1 day per unit).
pub const SECS_PER_UNIT: u64 = 86_400;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lease {
    pub landlord: Address,
    pub tenant: Address,
    pub rent_amount: i128,
    pub deposit_amount: i128,
    pub start_date: u64,
    pub end_date: u64,
    pub property_uri: String,
    pub status: LeaseStatus,
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
pub struct DepositReleasePartial {
    pub tenant_amount: i128,
    pub landlord_amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DepositRelease {
    FullRefund,
    PartialRefund(DepositReleasePartial),
    Disputed,
    pub amount: i128,
    pub nft_contract: Option<Address>,  // None if no NFT involved
    pub token_id: Option<u128>,         // None if no NFT involved
    pub active: bool,
    pub grace_period_end: u64,
    pub late_fee_flat: i128,
    pub late_fee_per_day: i128,
    pub debt: i128,
    pub flat_fee_applied: bool,
    pub days_late_charged: u64,
    pub rent_paid: i128,
    pub expiry_time: u64,
}

#[contract]
pub struct LeaseContract;

#[contractimpl]
impl LeaseContract {
    /// Initializes a lease with collateral lock (security deposit)
    pub fn initialize_lease(
        env: Env,
        landlord: Address,
        tenant: Address,
        rent_amount: i128,
        deposit_amount: i128,
        start_date: u64,
        end_date: u64,
        property_uri: String,
    ) -> Symbol {
        let lease = Lease {
            landlord: landlord.clone(),
            tenant: tenant.clone(),
            rent_amount,
            deposit_amount,
            start_date,
            end_date,
            property_uri: property_uri.clone(),
            status: LeaseStatus::Pending,
    /// Initializes a simple lease between a landlord and a tenant.
    pub fn create_lease(
        env: Env,
        landlord: Address,
        tenant: Address,
        amount: i128,
        grace_period_end: u64,
        late_fee_flat: i128,
        late_fee_per_day: i128,
    ) -> Symbol {
    /// Original function — unchanged behaviour, no NFT required.
    pub fn create_lease(env: Env, landlord: Address, tenant: Address, amount: i128) -> Symbol {
    /// Initializes a lease between a landlord and a tenant.
    /// `lease_id` uniquely identifies the lease in storage.
    /// `duration` sets the initial lease duration in seconds.
    pub fn create_lease(
        env: Env,
        lease_id: Symbol,
        landlord: Address,
        tenant: Address,
        amount: i128,
        duration: u64,
    ) -> Symbol {
        let expiry_time = env.ledger().timestamp().saturating_add(duration);
        let lease = Lease {
            landlord,
            tenant,
            amount,
            nft_contract: None,
            token_id: None,
            active: true,
            grace_period_end,
            late_fee_flat,
            late_fee_per_day,
            debt: 0,
            flat_fee_applied: false,
            days_late_charged: 0,
            rent_paid: 0,
        };
        
        env.storage()
            .instance()
            .set(&symbol_short!("lease"), &lease);
        
        symbol_short!("pending")
    }
    
    /// Activates lease after security deposit is transferred
    pub fn activate_lease(env: Env, tenant: Address) -> Symbol {
        let mut lease = Self::get_lease(env.clone());
        
        require!(lease.tenant == tenant, "Unauthorized: Only tenant can activate lease");
        require!(lease.status == LeaseStatus::Pending, "Lease is not in pending state");
        
        // In a real implementation, this would verify the token transfer
        // For now, we'll assume the deposit has been transferred
        lease.status = LeaseStatus::Active;
        
        env.storage()
            .instance()
            .set(&symbol_short!("lease"), &lease);
            
        symbol_short!("active")
    }
    
    /// Updates property metadata URI
    pub fn update_property_uri(env: Env, landlord: Address, property_uri: String) -> Symbol {
        let mut lease = Self::get_lease(env.clone());
        
        require!(lease.landlord == landlord, "Unauthorized: Only landlord can update property URI");
        
        lease.property_uri = property_uri.clone();
        
        env.storage()
            .instance()
            .set(&symbol_short!("lease"), &lease);
            
        symbol_short!("updated")
    }
    
    /// Amends lease with both landlord and tenant signatures
    pub fn amend_lease(env: Env, amendment: LeaseAmendment) -> Symbol {
        let mut lease = Self::get_lease(env.clone());
        
        require!(lease.status == LeaseStatus::Active, "Can only amend active leases");
        
        // In a real implementation, this would verify the signatures
        // For now, we'll assume they are valid
        
        if let Some(new_rent) = amendment.new_rent_amount {
            lease.rent_amount = new_rent;
        }
        
        if let Some(new_end_date) = amendment.new_end_date {
            lease.end_date = new_end_date;
        }
        
        env.storage()
            .instance()
            .set(&symbol_short!("lease"), &lease);
            
        symbol_short!("amended")
    }
    
    /// Releases security deposit with conditional logic
    pub fn release_deposit(env: Env, release_type: DepositRelease) -> Symbol {
        let lease = Self::get_lease(env.clone());
        
        require!(lease.status == LeaseStatus::Active || lease.status == LeaseStatus::Expired, 
                 "Can only release deposit from active or expired leases");
        
        match release_type {
            DepositRelease::FullRefund => {
                // In a real implementation, this would transfer full deposit to tenant
                symbol_short!("full_ref")
            }
            DepositRelease::PartialRefund(partial) => {
                require!(partial.tenant_amount + partial.landlord_amount == lease.deposit_amount, 
                         "Amounts must sum to total deposit");
                // In a real implementation, this would transfer amounts accordingly
                symbol_short!("partial")
            }
            DepositRelease::Disputed => {
                let mut updated_lease = lease;
                updated_lease.status = LeaseStatus::Disputed;
                env.storage()
                    .instance()
                    .set(&symbol_short!("lease"), &updated_lease);
                symbol_short!("disputed")
            }
        }
    }

    /// New function — same as above but also transfers an NFT from landlord to tenant.
    pub fn create_lease_with_nft(
        env: Env,
        landlord: Address,
        tenant: Address,
        amount: i128,
        nft_contract: Address,
        token_id: u128,
    ) -> Symbol {
        landlord.require_auth();

        let nft_client = nft_contract::NftClient::new(&env, &nft_contract);
        nft_client.transfer_from(
            &env.current_contract_address(),
            &landlord,
            &tenant,
            &token_id,
        );

        let lease = Lease {
            landlord,
            tenant,
            amount,
            nft_contract: Some(nft_contract),
            token_id: Some(token_id),
            active: true,
            expiry_time,
        };
        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("created")
    }

    pub fn get_lease(env: Env) -> Lease {
    /// Returns the lease details for the given `lease_id`.
    pub fn get_lease(env: Env, lease_id: Symbol) -> Lease {
        env.storage()
            .instance()
            .get(&lease_id)
            .expect("Lease not found")
    }

    /// Processes a rent payment, calculating and clearing debt before applying to rent.
    pub fn pay_rent(env: Env, payment_amount: i128) -> Symbol {
        let mut lease = Self::get_lease(env.clone());
        if !lease.active {
            panic!("Lease is not active");
        }

        let current_time = env.ledger().timestamp();

        // Calculate Debt
        if current_time > lease.grace_period_end {
            let seconds_late = current_time - lease.grace_period_end;
            
            if !lease.flat_fee_applied {
                lease.debt += lease.late_fee_flat;
                lease.flat_fee_applied = true;
            }

            let current_days_late = seconds_late / 86400; // Complete 24h periods
            if current_days_late > lease.days_late_charged {
                let newly_accrued_days = current_days_late - lease.days_late_charged;
                lease.debt += (newly_accrued_days as i128) * lease.late_fee_per_day;
                lease.days_late_charged = current_days_late;
            }
        }

        let mut remaining_payment = payment_amount;

        // Apply to debt first
        if lease.debt > 0 {
            if remaining_payment >= lease.debt {
                remaining_payment -= lease.debt;
                lease.debt = 0;
            } else {
                lease.debt -= remaining_payment;
                remaining_payment = 0;
            }
        }

        // Apply remainder to current month's rent
        if remaining_payment > 0 {
            lease.rent_paid += remaining_payment;
            
            // Advance month if fully paid
            if lease.rent_paid >= lease.amount {
                lease.rent_paid -= lease.amount;
                lease.grace_period_end += 2592000; // 30 days
                lease.flat_fee_applied = false;
                lease.days_late_charged = 0;
            }
        }

        env.storage().instance().set(&symbol_short!("lease"), &lease);
        symbol_short!("paid")
    /// Adds funds to an existing lease, extending `expiry_time` proportionally.
    /// Each unit of `amount` extends the lease by `SECS_PER_UNIT` seconds.
    /// Requires authorization from the tenant.
    pub fn add_funds(env: Env, lease_id: Symbol, amount: i128) -> Symbol {
        assert!(amount > 0, "amount must be positive");

        let mut lease: Lease = env
            .storage()
            .instance()
            .get(&lease_id)
            .expect("Lease not found");

        lease.tenant.require_auth();

        let extra_secs = (amount as u64).saturating_mul(SECS_PER_UNIT);
        lease.amount = lease.amount.saturating_add(amount);
        lease.expiry_time = lease.expiry_time.saturating_add(extra_secs);

        env.storage().instance().set(&lease_id, &lease);

        symbol_short!("extended")
    }
}

mod test;