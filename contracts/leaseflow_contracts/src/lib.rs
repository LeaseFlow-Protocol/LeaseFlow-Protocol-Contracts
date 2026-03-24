#![no_std]
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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lease {
    pub landlord: Address,
    pub tenant: Address,
    pub amount: i128,
    pub nft_contract: Option<Address>,  // None if no NFT involved
    pub token_id: Option<u128>,         // None if no NFT involved
    pub active: bool,
}

#[contract]
pub struct LeaseContract;

#[contractimpl]
impl LeaseContract {
    /// Original function — unchanged behaviour, no NFT required.
    pub fn create_lease(env: Env, landlord: Address, tenant: Address, amount: i128) -> Symbol {
        let lease = Lease {
            landlord,
            tenant,
            amount,
            nft_contract: None,
            token_id: None,
            active: true,
        };
        env.storage()
            .instance()
            .set(&symbol_short!("lease"), &lease);
        symbol_short!("created")
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
        };
        env.storage()
            .instance()
            .set(&symbol_short!("lease"), &lease);
        symbol_short!("created")
    }

    pub fn get_lease(env: Env) -> Lease {
        env.storage()
            .instance()
            .get(&symbol_short!("lease"))
            .expect("Lease not found")
    }
}

mod test;