# LeaseFlow Protocol Architecture: Storage Tiers

The LeaseFlow Protocol utilizes Soroban's tiered storage model to optimize for economic efficiency and prevent permanent ledger bloat. Data is categorized into three tiers based on its lifecycle and criticality.

## 1. Instance Storage
Instance storage is tied to the contract instance. It is used for global configuration and metadata that is small in size and frequently accessed.

| Data Type | Key Pattern | Description |
|-----------|-------------|-------------|
| **Admin Address** | `DataKey::Admin` | The administrative authority of the contract. |
| **Global Config** | `DataKey::ProtocolFeeConfig` | Protocol-wide parameters (e.g., fee basis points). |
| **Whitelists** | `DataKey::WhitelistedOracle(pubkey)` | List of authorized oracle public keys. |
| **Hierarchy** | `DataKey::FallbackHierarchy` | Primary/Backup oracle configuration. |

## 2. Persistent Storage
Persistent storage is used for critical state that must survive indefinitely or for long durations. This data represents the core economic state of the protocol.

| Data Type | Key Pattern | Description |
|-----------|-------------|-------------|
| **Lease State** | `DataKey::LeaseInstance(id)` | Core lease terms, rent rates, and active status. |
| **Security Deposits** | Part of `LeaseInstance` | Funds held in escrow until lease termination. |
| **Billing Records** | `DataKey::Receipt(id, index)` | Proof of rent payments and billing history. |
| **Usage Rights** | `DataKey::UsageRights(addr, id)` | Cryptographic proof of access for lessees. |
| **Yield Deployments** | `DataKey::YieldDeployment(id)` | Tracking of escrowed funds deployed to yield protocols. |
| **Tombstones** | `DataKey::LeaseTombstone(id)` | Cryptographic proof of pruned historical leases. |

## 3. Temporary Storage
Temporary storage is used for ephemeral data that only needs to survive for a short window or within a single transaction. This prevents permanent ledger bloat.

| Data Type | Key Pattern | Description |
|-----------|-------------|-------------|
| **Reentrancy Lock** | `DataKey::NonReentrant` | Prevents nested call attacks within a single transaction. |
| **Velocity Tracking** | `DataKey::VelocityTracker(lessor)` | Tracks termination frequency within a rolling 24-hour window. |
| **Oracle Retry Stats** | `DataKey::OracleRetryStats(pubkey)` | Tracks consecutive oracle failures and demotion timestamps. |
| **Verification Cache** | `AccessDataKey::VerificationCache` | Caches IoT/System access verification results (5-minute TTL). |

## Storage Lifecycle Management
- **TTL Extension**: The protocol automatically extends the TTL of Persistent and Temporary entries during interaction to ensure they remain available while relevant.
- **Pruning**: Finalized leases (Terminated/Expired) are subject to pruning after a 60-day cooldown, moving them from Persistent storage to a lightweight Tombstone.
- **Eviction Recovery**: For Temporary data (like velocity trackers), the protocol is designed to default to a "fresh" state if data is evicted, ensuring graceful recovery without blocking operations.
