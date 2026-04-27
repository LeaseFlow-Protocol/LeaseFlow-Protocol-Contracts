//! Multi-Sig Timelock for Protocol Wasm Upgrades
//! 
//! This module implements a secure upgrade mechanism with:
//! - 14-day mandatory waiting period
//! - Multi-signature approval
//! - Public on-chain notice
//! - Cancellation capability

use soroban_sdk::{contract, contracttype, contractevent, Address, Env, BytesN, String, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpgradeProposalStatus {
    Pending,
    Approved,
    Executed,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeProposal {
    pub proposal_id: u64,
    pub proposer: Address,
    pub new_wasm_hash: BytesN<32>,
    pub description: String,
    pub status: UpgradeProposalStatus,
    pub created_at: u64,
    pub approvals: Vec<Address>,
    pub approval_count: u32,
    pub required_approvals: u32,
    pub execution_time: u64,
}

#[contracttype]
pub enum TimelockDataKey {
    UpgradeProposal(u64),
    ProposalCount,
    Signer(Address),
    TimelockDuration,
    MinApprovals,
    CurrentWasmHash,
}

#[contractevent]
pub struct UpgradeProposed {
    pub proposal_id: u64,
    pub proposer: Address,
    pub new_wasm_hash: BytesN<32>,
    pub description: String,
    pub execution_time: u64,
    pub timestamp: u64,
}

#[contractevent]
pub struct UpgradeApproved {
    pub proposal_id: u64,
    pub approver: Address,
    pub approval_count: u32,
    pub timestamp: u64,
}

#[contractevent]
pub struct UpgradeExecuted {
    pub proposal_id: u64,
    pub old_wasm_hash: BytesN<32>,
    pub new_wasm_hash: BytesN<32>,
    pub timestamp: u64,
}

#[contractevent]
pub struct UpgradeCancelled {
    pub proposal_id: u64,
    pub cancelled_by: Address,
    pub reason: String,
    pub timestamp: u64,
}

#[contractevent]
pub struct SignerAdded {
    pub signer: Address,
    pub added_by: Address,
    pub timestamp: u64,
}

#[contractevent]
pub struct SignerRemoved {
    pub signer: Address,
    pub removed_by: Address,
    pub timestamp: u64,
}

#[contract]
pub struct TimelockUpgradeModule;

#[contractimpl]
impl TimelockUpgradeModule {
    /// Initialize the timelock module
    pub fn initialize_timelock(
        env: Env,
        admin: Address,
        timelock_duration: u64,
        min_approvals: u32,
    ) -> Result<(), &'static str> {
        admin.require_auth();
        
        if env.storage().instance().has(&TimelockDataKey::TimelockDuration) {
            return Err("Already initialized");
        }

        let duration = if timelock_duration > 0 { timelock_duration } else { 1_209_600 };
        env.storage().instance().set(&TimelockDataKey::TimelockDuration, &duration);

        let min_approvals = if min_approvals > 0 { min_approvals } else { 3 };
        env.storage().instance().set(&TimelockDataKey::MinApprovals, &min_approvals);

        env.storage().instance().set(&TimelockDataKey::Signer(admin.clone()), &true);
        env.storage().instance().set(&TimelockDataKey::ProposalCount, &0u64);

        SignerAdded {
            signer: admin,
            added_by: admin,
            timestamp: env.ledger().timestamp(),
        }
        .publish(&env);

        Ok(())
    }

    /// Add a new signer to the multi-sig council
    pub fn add_signer(
        env: Env,
        caller: Address,
        new_signer: Address,
    ) -> Result<(), &'static str> {
        caller.require_auth();
        Self::require_signer(&env, &caller)?;

        env.storage().instance().set(&TimelockDataKey::Signer(new_signer.clone()), &true);

        SignerAdded {
            signer: new_signer,
            added_by: caller,
            timestamp: env.ledger().timestamp(),
        }
        .publish(&env);

        Ok(())
    }

    /// Remove a signer from the multi-sig council
    pub fn remove_signer(
        env: Env,
        caller: Address,
        signer_to_remove: Address,
    ) -> Result<(), &'static str> {
        caller.require_auth();
        Self::require_signer(&env, &caller)?;

        env.storage().instance().remove(&TimelockDataKey::Signer(signer_to_remove.clone()));

        SignerRemoved {
            signer: signer_to_remove,
            removed_by: caller,
            timestamp: env.ledger().timestamp(),
        }
        .publish(&env);

        Ok(())
    }

    /// Create an upgrade proposal (starts 14-day timelock)
    pub fn propose_upgrade(
        env: Env,
        proposer: Address,
        new_wasm_hash: BytesN<32>,
        description: String,
    ) -> Result<u64, &'static str> {
        proposer.require_auth();
        Self::require_signer(&env, &proposer)?;

        let proposal_count: u64 = env.storage().instance()
            .get(&TimelockDataKey::ProposalCount)
            .unwrap_or(0);
        
        let proposal_id = proposal_count;
        let timelock_duration: u64 = env.storage().instance()
            .get(&TimelockDataKey::TimelockDuration)
            .unwrap_or(1_209_600);
        let required_approvals: u32 = env.storage().instance()
            .get(&TimelockDataKey::MinApprovals)
            .unwrap_or(3);

        let proposal = UpgradeProposal {
            proposal_id,
            proposer: proposer.clone(),
            new_wasm_hash: new_wasm_hash.clone(),
            description: description.clone(),
            status: UpgradeProposalStatus::Pending,
            created_at: env.ledger().timestamp(),
            approvals: Vec::new(&env),
            approval_count: 0,
            required_approvals,
            execution_time: env.ledger().timestamp() + timelock_duration,
        };

        env.storage().instance().set(
            &TimelockDataKey::UpgradeProposal(proposal_id),
            &proposal
        );
        env.storage().instance().set(&TimelockDataKey::ProposalCount, &(proposal_count + 1));

        UpgradeProposed {
            proposal_id,
            proposer,
            new_wasm_hash,
            description,
            execution_time: proposal.execution_time,
            timestamp: env.ledger().timestamp(),
        }
        .publish(&env);

        Ok(proposal_id)
    }

    /// Approve an upgrade proposal
    pub fn approve_upgrade(
        env: Env,
        approver: Address,
        proposal_id: u64,
    ) -> Result<(), &'static str> {
        approver.require_auth();
        Self::require_signer(&env, &approver)?;

        let key = TimelockDataKey::UpgradeProposal(proposal_id);
        let mut proposal: UpgradeProposal = env.storage().instance()
            .get(&key)
            .ok_or("Proposal not found")?;

        if proposal.status == UpgradeProposalStatus::Executed {
            return Err("Proposal already executed");
        }
        if proposal.status == UpgradeProposalStatus::Cancelled {
            return Err("Proposal cancelled");
        }

        for existing_approver in proposal.approvals.iter() {
            if existing_approver == approver {
                return Err("Already approved");
            }
        }

        proposal.approvals.push_back(approver.clone());
        proposal.approval_count += 1;

        if proposal.approval_count >= proposal.required_approvals {
            proposal.status = UpgradeProposalStatus::Approved;
        }

        env.storage().instance().set(&key, &proposal);

        UpgradeApproved {
            proposal_id,
            approver,
            approval_count: proposal.approval_count,
            timestamp: env.ledger().timestamp(),
        }
        .publish(&env);

        Ok(())
    }

    /// Execute the upgrade after timelock period
    pub fn execute_upgrade(
        env: Env,
        executor: Address,
        proposal_id: u64,
    ) -> Result<(), &'static str> {
        executor.require_auth();

        let key = TimelockDataKey::UpgradeProposal(proposal_id);
        let mut proposal: UpgradeProposal = env.storage().instance()
            .get(&key)
            .ok_or("Proposal not found")?;

        if proposal.status != UpgradeProposalStatus::Approved {
            return Err("Proposal not approved");
        }

        if env.ledger().timestamp() < proposal.execution_time {
            return Err("Timelock period not elapsed");
        }

        let old_wasm_hash: BytesN<32> = env.storage().instance()
            .get(&TimelockDataKey::CurrentWasmHash)
            .unwrap_or(BytesN::default(&env));

        env.deployer().update_current_contract_wasm(proposal.new_wasm_hash.clone());

        proposal.status = UpgradeProposalStatus::Executed;
        env.storage().instance().set(&key, &proposal);
        env.storage().instance().set(
            &TimelockDataKey::CurrentWasmHash,
            &proposal.new_wasm_hash
        );

        UpgradeExecuted {
            proposal_id,
            old_wasm_hash,
            new_wasm_hash: proposal.new_wasm_hash,
            timestamp: env.ledger().timestamp(),
        }
        .publish(&env);

        Ok(())
    }

    /// Cancel an upgrade proposal
    pub fn cancel_upgrade(
        env: Env,
        canceller: Address,
        proposal_id: u64,
        reason: String,
    ) -> Result<(), &'static str> {
        canceller.require_auth();
        Self::require_signer(&env, &canceller)?;

        let key = TimelockDataKey::UpgradeProposal(proposal_id);
        let mut proposal: UpgradeProposal = env.storage().instance()
            .get(&key)
            .ok_or("Proposal not found")?;

        if proposal.status == UpgradeProposalStatus::Executed {
            return Err("Proposal already executed");
        }
        if proposal.status == UpgradeProposalStatus::Cancelled {
            return Err("Proposal already cancelled");
        }

        proposal.status = UpgradeProposalStatus::Cancelled;
        env.storage().instance().set(&key, &proposal);

        UpgradeCancelled {
            proposal_id,
            cancelled_by: canceller,
            reason,
            timestamp: env.ledger().timestamp(),
        }
        .publish(&env);

        Ok(())
    }

    /// Get proposal details
    pub fn get_proposal(env: Env, proposal_id: u64) -> Result<UpgradeProposal, &'static str> {
        let key = TimelockDataKey::UpgradeProposal(proposal_id);
        env.storage().instance()
            .get(&key)
            .ok_or("Proposal not found")
    }

    /// Check if address is a signer
    pub fn is_signer(env: Env, address: Address) -> bool {
        env.storage().instance()
            .get(&TimelockDataKey::Signer(address))
            .unwrap_or(false)
    }

    /// Get timelock duration
    pub fn get_timelock_duration(env: Env) -> u64 {
        env.storage().instance()
            .get(&TimelockDataKey::TimelockDuration)
            .unwrap_or(1_209_600)
    }

    /// Internal: Verify signer
    fn require_signer(env: &Env, address: &Address) -> Result<(), &'static str> {
        if !Self::is_signer(env.clone(), address.clone()) {
            return Err("Not authorized signer");
        }
        Ok(())
    }
}
