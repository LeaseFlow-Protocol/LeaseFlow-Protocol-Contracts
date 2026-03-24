#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol, String};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LeaseStatus {
    Active,
    Expired,
    Disputed,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lease {
    pub landlord: Address,
    pub tenant: Address,
    pub rent_amount: i128,
    pub deposit_amount: i128,
    pub start_date: u64,
    pub end_date: u64,
    pub status: LeaseStatus,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Receipt {
    pub lease_id: Symbol,
    pub month: u32,
    pub amount: i128,
    pub date: u64,
}

#[contract]
pub struct LeaseContract;

const DAY_IN_LEDGERS: u32 = 17280; // Assuming 5s ledger time
const MONTH_IN_LEDGERS: u32 = DAY_IN_LEDGERS * 30;
const YEAR_IN_LEDGERS: u32 = DAY_IN_LEDGERS * 365;

#[contractimpl]
impl LeaseContract {
    /// Initializes a lease in Persistent storage.
    pub fn initialize_lease(
        env: Env,
        lease_id: Symbol,
        landlord: Address,
        tenant: Address,
        rent_amount: i128,
        deposit_amount: i128,
        duration_seconds: u64,
    ) -> bool {
        landlord.require_auth();

        let start_date = env.ledger().timestamp();
        let end_date = start_date.saturating_add(duration_seconds);

        let lease = Lease {
            landlord,
            tenant,
            rent_amount,
            deposit_amount,
            start_date,
            end_date,
            status: LeaseStatus::Active,
        };

        // Core identity and contract terms stored in PERSISTENT storage to survive ledger expirations
        let key = (symbol_short!("lease"), lease_id.clone());
        env.storage().persistent().set(&key, &lease);
        
        // Initial TTL extension for core data to live for at least the lease duration (approx 1 year)
        env.storage().persistent().extend_ttl(&key, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);

        true
    }

    /// Processes rent payment, saves receipt in Instance storage, and extends TTL.
    pub fn pay_rent(env: Env, lease_id: Symbol, month: u32, amount: i128) -> bool {
        let key = (symbol_short!("lease"), lease_id.clone());
        let lease: Lease = env.storage().persistent().get(&key).expect("Lease not found");
        
        lease.tenant.require_auth();

        // Individual monthly payment receipts use INSTANCE storage to keep costs lower 
        // as they are accessed primarily during active lease management
        let receipt = Receipt {
            lease_id: lease_id.clone(),
            month,
            amount,
            date: env.ledger().timestamp(),
        };
        
        let receipt_key = (symbol_short!("receipt"), lease_id.clone(), month);
        env.storage().instance().set(&receipt_key, &receipt);

        // Auto-extend TTL of the contract instance and its data during payment
        // This keeps the contract "alive" for the duration of the 12-month lease 
        // without manual "rent" payments to the network
        env.storage().instance().extend_ttl(MONTH_IN_LEDGERS, YEAR_IN_LEDGERS);
        
        // Also extend the persistent lease record TTL to ensure core data persists
        env.storage().persistent().extend_ttl(&key, MONTH_IN_LEDGERS, YEAR_IN_LEDGERS);

        true
    }

    /// Returns the lease details from Persistent storage.
    pub fn get_lease(env: Env, lease_id: Symbol) -> Lease {
        let key = (symbol_short!("lease"), lease_id);
        env.storage().persistent().get(&key).expect("Lease not found")
    }

    /// Returns a specific receipt from Instance storage.
    pub fn get_receipt(env: Env, lease_id: Symbol, month: u32) -> Receipt {
        let key = (symbol_short!("receipt"), lease_id, month);
        env.storage().instance().get(&key).expect("Receipt not found")
    }
    
    /// Triggered to keep the contract instance and state alive manually if needed.
    pub fn extend_ttl(env: Env, lease_id: Symbol) {
        let key = (symbol_short!("lease"), lease_id);
        if env.storage().persistent().has(&key) {
            env.storage().persistent().extend_ttl(&key, MONTH_IN_LEDGERS, YEAR_IN_LEDGERS);
        }
        env.storage().instance().extend_ttl(MONTH_IN_LEDGERS, YEAR_IN_LEDGERS);
    }
}

mod test;