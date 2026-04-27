//! Decentralized Identity (SEP-12) Provider Registration Module
//! 
//! This module implements a decentralized network of identity verifiers,
//! allowing lessors to have granular control over which regulatory compliance
//! bodies they trust for their assets.

use soroban_sdk::{contract, contracttype, contractevent, Address, Env, BytesN, String, Vec, Symbol, symbol_short};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityProviderStatus {
    Active,
    Suspended,
    Revoked,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityProvider {
    pub provider_id: BytesN<32>,
    pub provider_address: Address,
    pub name: String,
    pub status: IdentityProviderStatus,
    pub registered_at: u64,
    pub last_updated: u64,
    pub verification_count: u64,
    pub trust_score: u32, // 0-10000 basis points
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LessorTrustedProviders {
    pub lessor: Address,
    pub trusted_providers: Vec<BytesN<32>>,
    pub auto_accept_new_providers: bool,
}

#[contracttype]
pub enum IdentityDataKey {
    IdentityProvider(BytesN<32>),
    LessorTrustedProviders(Address),
    GovernanceCouncil(Address),
    RevocationProposal(u64),
    ProviderRegistryCount,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevocationProposal {
    pub proposal_id: u64,
    pub provider_id: BytesN<32>,
    pub proposer: Address,
    pub reason: String,
    pub votes_for: u32,
    pub votes_against: u32,
    pub created_at: u64,
    pub executed: bool,
}

#[contractevent]
pub struct IdentityProviderRegistered {
    pub provider_id: BytesN<32>,
    pub provider_address: Address,
    pub name: String,
    pub timestamp: u64,
}

#[contractevent]
pub struct IdentityProviderSuspended {
    pub provider_id: BytesN<32>,
    pub reason: String,
    pub timestamp: u64,
}

#[contractevent]
pub struct IdentityProviderRevoked {
    pub provider_id: BytesN<32>,
    pub reason: String,
    pub timestamp: u64,
}

#[contractevent]
pub struct LessorTrustedProviderAdded {
    pub lessor: Address,
    pub provider_id: BytesN<32>,
    pub timestamp: u64,
}

#[contractevent]
pub struct LessorTrustedProviderRemoved {
    pub lessor: Address,
    pub provider_id: BytesN<32>,
    pub timestamp: u64,
}

#[contractevent]
pub struct RevocationProposalCreated {
    pub proposal_id: u64,
    pub provider_id: BytesN<32>,
    pub proposer: Address,
    pub timestamp: u64,
}

#[contractevent]
pub struct RevocationProposalExecuted {
    pub proposal_id: u64,
    pub provider_id: BytesN<32>,
    pub timestamp: u64,
}

#[contract]
pub struct Sep12IdentityModule;

#[contractimpl]
impl Sep12IdentityModule {
    /// Register a new identity provider
    pub fn register_identity_provider(
        env: Env,
        provider_id: BytesN<32>,
        provider_address: Address,
        name: String,
    ) -> Result<(), &'static str> {
        provider_address.require_auth();

        // Check if provider already exists
        let key = IdentityDataKey::IdentityProvider(provider_id.clone());
        if env.storage().instance().has(&key) {
            return Err("Provider already registered");
        }

        let now = env.ledger().timestamp();
        let provider = IdentityProvider {
            provider_id: provider_id.clone(),
            provider_address: provider_address.clone(),
            name: name.clone(),
            status: IdentityProviderStatus::Active,
            registered_at: now,
            last_updated: now,
            verification_count: 0,
            trust_score: 5000, // Start with 50% trust score (5000 bps)
        };

        env.storage().instance().set(&key, &provider);

        // Increment registry count
        let count: u64 = env.storage().instance()
            .get(&IdentityDataKey::ProviderRegistryCount)
            .unwrap_or(0);
        env.storage().instance().set(&IdentityDataKey::ProviderRegistryCount, &(count + 1));

        IdentityProviderRegistered {
            provider_id,
            provider_address,
            name,
            timestamp: now,
        }
        .publish(&env);

        Ok(())
    }

    /// Add a trusted provider for a specific lessor
    pub fn add_trusted_provider(
        env: Env,
        lessor: Address,
        provider_id: BytesN<32>,
    ) -> Result<(), &'static str> {
        lessor.require_auth();

        // Verify provider exists and is active
        let provider_key = IdentityDataKey::IdentityProvider(provider_id.clone());
        let provider: IdentityProvider = env.storage().instance()
            .get(&provider_key)
            .ok_or("Provider not found")?;

        if provider.status != IdentityProviderStatus::Active {
            return Err("Provider is not active");
        }

        // Get or create lessor's trusted providers list
        let lessor_key = IdentityDataKey::LessorTrustedProviders(lessor.clone());
        let mut trusted: LessorTrustedProviders = env.storage().instance()
            .get(&lessor_key)
            .unwrap_or(LessorTrustedProviders {
                lessor: lessor.clone(),
                trusted_providers: Vec::new(&env),
                auto_accept_new_providers: false,
            });

        // Check if already trusted
        for existing in trusted.trusted_providers.iter() {
            if existing == provider_id {
                return Err("Provider already trusted");
            }
        }

        trusted.trusted_providers.push_back(provider_id.clone());
        env.storage().instance().set(&lessor_key, &trusted);

        LessorTrustedProviderAdded {
            lessor,
            provider_id,
            timestamp: env.ledger().timestamp(),
        }
        .publish(&env);

        Ok(())
    }

    /// Remove a trusted provider for a specific lessor
    pub fn remove_trusted_provider(
        env: Env,
        lessor: Address,
        provider_id: BytesN<32>,
    ) -> Result<(), &'static str> {
        lessor.require_auth();

        let lessor_key = IdentityDataKey::LessorTrustedProviders(lessor.clone());
        let mut trusted: LessorTrustedProviders = env.storage().instance()
            .get(&lessor_key)
            .ok_or("No trusted providers configured")?;

        // Remove provider from list
        let mut new_providers = Vec::new(&env);
        let mut found = false;
        for existing in trusted.trusted_providers.iter() {
            if existing == provider_id {
                found = true;
            } else {
                new_providers.push_back(existing);
            }
        }

        if !found {
            return Err("Provider not in trusted list");
        }

        trusted.trusted_providers = new_providers;
        env.storage().instance().set(&lessor_key, &trusted);

        LessorTrustedProviderRemoved {
            lessor,
            provider_id,
            timestamp: env.ledger().timestamp(),
        }
        .publish(&env);

        Ok(())
    }

    /// Suspend an identity provider (admin/governance only)
    pub fn suspend_provider(
        env: Env,
        admin: Address,
        provider_id: BytesN<32>,
        reason: String,
    ) -> Result<(), &'static str> {
        Self::require_governance_council(&env, &admin)?;
        admin.require_auth();

        let key = IdentityDataKey::IdentityProvider(provider_id.clone());
        let mut provider: IdentityProvider = env.storage().instance()
            .get(&key)
            .ok_or("Provider not found")?;

        if provider.status == IdentityProviderStatus::Revoked {
            return Err("Provider already revoked");
        }

        provider.status = IdentityProviderStatus::Suspended;
        provider.last_updated = env.ledger().timestamp();
        env.storage().instance().set(&key, &provider);

        IdentityProviderSuspended {
            provider_id,
            reason,
            timestamp: env.ledger().timestamp(),
        }
        .publish(&env);

        Ok(())
    }

    /// Create a revocation proposal for a compromised provider
    pub fn create_revocation_proposal(
        env: Env,
        proposer: Address,
        provider_id: BytesN<32>,
        reason: String,
    ) -> Result<u64, &'static str> {
        proposer.require_auth();
        Self::require_governance_council(&env, &proposer)?;

        // Verify provider exists
        let provider_key = IdentityDataKey::IdentityProvider(provider_id.clone());
        env.storage().instance()
            .get::<_, IdentityProvider>(&provider_key)
            .ok_or("Provider not found")?;

        // Create proposal
        let proposal_id: u64 = env.storage().instance()
            .get(&IdentityDataKey::ProviderRegistryCount)
            .unwrap_or(0);

        let proposal = RevocationProposal {
            proposal_id,
            provider_id: provider_id.clone(),
            proposer: proposer.clone(),
            reason: reason.clone(),
            votes_for: 0,
            votes_against: 0,
            created_at: env.ledger().timestamp(),
            executed: false,
        };

        env.storage().instance().set(
            &IdentityDataKey::RevocationProposal(proposal_id),
            &proposal
        );

        RevocationProposalCreated {
            proposal_id,
            provider_id,
            proposer,
            timestamp: env.ledger().timestamp(),
        }
        .publish(&env);

        Ok(proposal_id)
    }

    /// Vote on a revocation proposal
    pub fn vote_on_revocation(
        env: Env,
        voter: Address,
        proposal_id: u64,
        vote_for: bool,
    ) -> Result<(), &'static str> {
        voter.require_auth();
        Self::require_governance_council(&env, &voter)?;

        let proposal_key = IdentityDataKey::RevocationProposal(proposal_id);
        let mut proposal: RevocationProposal = env.storage().instance()
            .get(&proposal_key)
            .ok_or("Proposal not found")?;

        if proposal.executed {
            return Err("Proposal already executed");
        }

        if vote_for {
            proposal.votes_for += 1;
        } else {
            proposal.votes_against += 1;
        }

        env.storage().instance().set(&proposal_key, &proposal);

        Ok(())
    }

    /// Execute a revocation proposal if it has enough votes
    pub fn execute_revocation(
        env: Env,
        executor: Address,
        proposal_id: u64,
    ) -> Result<(), &'static str> {
        executor.require_auth();
        Self::require_governance_council(&env, &executor)?;

        let proposal_key = IdentityDataKey::RevocationProposal(proposal_id);
        let mut proposal: RevocationProposal = env.storage().instance()
            .get(&proposal_key)
            .ok_or("Proposal not found")?;

        if proposal.executed {
            return Err("Proposal already executed");
        }

        // Require 2/3 majority for revocation
        let total_votes = proposal.votes_for + proposal.votes_against;
        if total_votes < 3 || proposal.votes_for * 2 <= total_votes * 1 {
            return Err("Insufficient votes for revocation");
        }

        // Revoke the provider
        let provider_key = IdentityDataKey::IdentityProvider(proposal.provider_id.clone());
        let mut provider: IdentityProvider = env.storage().instance()
            .get(&provider_key)
            .ok_or("Provider not found")?;

        provider.status = IdentityProviderStatus::Revoked;
        provider.last_updated = env.ledger().timestamp();
        env.storage().instance().set(&provider_key, &provider);

        proposal.executed = true;
        env.storage().instance().set(&proposal_key, &proposal);

        IdentityProviderRevoked {
            provider_id: proposal.provider_id.clone(),
            reason: proposal.reason.clone(),
            timestamp: env.ledger().timestamp(),
        }
        .publish(&env);

        RevocationProposalExecuted {
            proposal_id,
            provider_id: proposal.provider_id,
            timestamp: env.ledger().timestamp(),
        }
        .publish(&env);

        Ok(())
    }

    /// Check if a lessor trusts a specific provider
    pub fn is_provider_trusted(
        env: Env,
        lessor: Address,
        provider_id: BytesN<32>,
    ) -> bool {
        let lessor_key = IdentityDataKey::LessorTrustedProviders(lessor);
        if let Some(trusted) = env.storage().instance().get::<_, LessorTrustedProviders>(&lessor_key) {
            for provider in trusted.trusted_providers.iter() {
                if provider == provider_id {
                    return true;
                }
            }
        }
        false
    }

    /// Get provider information
    pub fn get_provider(env: Env, provider_id: BytesN<32>) -> Result<IdentityProvider, &'static str> {
        let key = IdentityDataKey::IdentityProvider(provider_id);
        env.storage().instance()
            .get(&key)
            .ok_or("Provider not found")
    }

    /// Get lessor's trusted providers
    pub fn get_trusted_providers(
        env: Env,
        lessor: Address,
    ) -> Result<LessorTrustedProviders, &'static str> {
        let key = IdentityDataKey::LessorTrustedProviders(lessor);
        env.storage().instance()
            .get(&key)
            .ok_or("No trusted providers configured")
    }

    /// Add address to governance council
    pub fn add_governance_council_member(
        env: Env,
        admin: Address,
        member: Address,
    ) -> Result<(), &'static str> {
        Self::require_governance_council(&env, &admin)?;
        admin.require_auth();

        let council_key = IdentityDataKey::GovernanceCouncil(member.clone());
        env.storage().instance().set(&council_key, &true);

        Ok(())
    }

    /// Internal: Check if address is in governance council
    fn require_governance_council(env: &Env, address: &Address) -> Result<(), &'static str> {
        let council_key = IdentityDataKey::GovernanceCouncil(address.clone());
        let is_member: bool = env.storage().instance()
            .get(&council_key)
            .unwrap_or(false);
        
        if !is_member {
            return Err("Not a governance council member");
        }
        
        Ok(())
    }
}
