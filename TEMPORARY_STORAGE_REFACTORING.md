# Temporary Storage Refactoring for Ephemeral Metadata

## Issue #181: Refactor state variables to leverage temporary storage for ephemeral metadata during lease negotiations

### Overview
This refactoring optimizes Soroban smart contract storage by moving ephemeral metadata from persistent storage to temporary storage. This reduces on-chain rent costs and improves security by ensuring temporary data automatically expires.

### Changes Made

#### 1. DataKey Enum Updates
Added comments to mark ephemeral keys and added missing keys:

- **PendingSlash(u64)** - Marked as Temporary: 24-hour veto timelock
- **VetoVote(u64, Address)** - Marked as Temporary: ephemeral during veto period
- **PendingFeeUpdate** - Marked as Temporary: 7-day fee update timelock
- **GovernanceRound(u64)** - Marked as Temporary: 7-day voting period
- **TreasuryVote(u64, Address)** - Marked as Temporary: during voting period
- **VotingPowerSnapshot(u64, Address)** - Marked as Temporary: ephemeral snapshot
- **DaoApprovalRequest(u64)** - Marked as Temporary: ephemeral approval requests
- **OracleRateLimit(BytesN<32>, u64)** - Added: Temporary key for oracle rate limiting (1-hour windows)
- **AssetCondition(u64)** - Added: Asset condition tracking
- **AssetFreezeProof(u64, Address, u128)** - Added: RWA asset freeze proof storage
- **FrozenAsset(u64, Address, u128)** - Added: Frozen asset tracking
- **WhitelistedRegistry(Address)** - Added: Whitelisted asset registry tracking

#### 2. Temporary Storage Helper Functions
Added comprehensive helper functions for ephemeral storage operations:

**PendingSlashVeto (24-hour TTL):**
- `get_pending_slash(env, lease_id)` - Load from temporary storage
- `set_pending_slash(env, lease_id, pending_slash)` - Store with 17,280 ledger TTL
- `remove_pending_slash(env, lease_id)` - Remove from temporary storage

**VetoVote (24-hour TTL):**
- `get_veto_vote(env, lease_id, voter)` - Load vote status
- `set_veto_vote(env, lease_id, voter, voted)` - Store with 17,280 ledger TTL

**PendingFeeUpdate (7-day TTL):**
- `get_pending_fee_update(env)` - Load from temporary storage
- `set_pending_fee_update(env, pending_update)` - Store with 120,960 ledger TTL
- `remove_pending_fee_update(env)` - Remove from temporary storage

**GovernanceRound (7-day TTL):**
- `get_governance_round(env, round_id)` - Load from temporary storage
- `set_governance_round(env, round_id, round)` - Store with 120,960 ledger TTL
- `remove_governance_round(env, round_id)` - Remove from temporary storage

**TreasuryVote (7-day TTL):**
- `get_treasury_vote(env, round_id, voter)` - Load vote from temporary storage
- `set_treasury_vote(env, round_id, vote)` - Store with 120,960 ledger TTL

**VotingPowerSnapshot (7-day TTL):**
- `get_voting_power_snapshot(env, round_id, voter)` - Load snapshot
- `set_voting_power_snapshot(env, round_id, voter, power)` - Store with 120,960 ledger TTL

**DaoApprovalRequest (24-hour TTL):**
- `get_dao_approval_request(env, request_id)` - Load approval status
- `set_dao_approval_request(env, request_id, approved)` - Store with 17,280 ledger TTL
- `remove_dao_approval_request(env, request_id)` - Remove from temporary storage

#### 3. Refactored Storage Operations

**PendingSlashVeto Storage:**
- Changed from `env.storage().instance().set()` to `env.storage().temporary().set()`
- Updated to use `set_pending_slash()` helper function
- TTL: 17,280 ledgers (~24 hours at ~5s per ledger)

**OracleRateLimit Storage:**
- Changed from `env.storage().instance().has()` to `env.storage().temporary().has()`
- Changed from `env.storage().instance().set()` to `env.storage().temporary().set()`
- TTL: 720 ledgers (~1 hour at ~5s per ledger)
- Purpose: Prevent oracle spamming with 1 update per hour maximum

#### 4. Data Structure Updates

**LeaseInstance Struct:**
Added RWA asset management fields:
- `asset_registry_address: Option<Address>` - Optional asset registry contract address
- `asset_id: Option<u128>` - Optional RWA asset ID

**CreateLeaseParams Struct:**
Added RWA asset management fields:
- `asset_registry_address: Option<Address>` - Optional asset registry contract address
- `asset_id: Option<u128>` - Optional RWA asset ID

**create_lease_instance Function:**
- Initialized new asset registry fields from params

### Benefits

1. **Reduced Storage Costs**: Ephemeral data no longer incurs persistent storage rent
2. **Automatic Cleanup**: Temporary storage automatically expires, preventing stale data accumulation
3. **Security Hardening**: Temporary data cannot persist indefinitely, reducing attack surface
4. **Improved Performance**: Temporary storage operations are cheaper and faster
5. **Better Resource Management**: Automatic TTL management reduces manual cleanup requirements

### TTL Calculations

- **24-hour TTL**: 17,280 ledgers (assuming ~5s per ledger close)
- **7-day TTL**: 120,960 ledgers (assuming ~5s per ledger close)
- **1-hour TTL**: 720 ledgers (assuming ~5s per ledger close)

### Storage Tier Strategy

**Persistent Storage (Instance/Persistent):**
- Lease data
- User permissions
- Long-term configuration
- Historical records

**Temporary Storage:**
- Time-limited governance data (voting, vetoes)
- Rate limiting counters
- Ephemeral negotiation metadata
- Retry counters with automatic reset

### Backward Compatibility

The refactoring maintains backward compatibility by:
- Keeping the same DataKey enum variants
- Providing helper functions that abstract storage type
- Existing functions continue to work with updated storage layer

### Testing Recommendations

1. Verify temporary storage TTL expiration behavior
2. Test governance flows with temporary storage
3. Validate oracle rate limiting with temporary storage
4. Ensure data recovery after temporary storage eviction
5. Test concurrent access to temporary storage

### Files Modified

- `contracts/leaseflow_contracts/src/lib.rs`:
  - DataKey enum updates
  - Helper function additions
  - Storage operation refactoring
  - Data structure field additions
