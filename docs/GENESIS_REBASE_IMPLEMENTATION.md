# Genesis Rebase Implementation Plan

## Overview
Implementation plan for periodic blockchain rebasing to create new genesis blocks with current state while pruning historical data, reducing long-term storage and sync burden.

## Implementation Tasks

### Phase 1: Core Infrastructure (High Priority)

#### 1. State Snapshot Capture Mechanism
**Location**: `node/src/rpc.rs`, `runtime/src/apis/` 
- Create custom RPC endpoint `rebase_export_state(at_block: Option<BlockHash>)`
- Implement runtime API for state export at specific block heights
- Add CLI command wrapper for `state_export` with custom parameters
- Support both full state and selective pallet state export

#### 2. Block Archival System
**Location**: `node/src/service.rs`, `consensus/micc-client/src/`
- Integrate with IPFS client for decentralized storage
- Add cloud storage backends (AWS S3, GCP, Azure)
- Implement block range archival with metadata
- Create archive integrity verification system

#### 3. New Genesis Creation
**Location**: `node/src/chain_spec.rs`, `node/src/command.rs`
- Extend ChainSpec builder to accept exported state
- Add CLI command `build-rebased-spec --state-file <path> --output <spec.json>`
- Implement state root validation during spec creation
- Support incremental genesis updates

#### 4. Governance Integration
**Location**: `runtime/src/configs/governance.rs` (new), `pallets/` 
- Add `pallet-rebase-governance` for proposal management
- Integrate with existing `pallet-scheduler` for timed execution
- Create rebase proposal types with validation
- Implement multi-signature approval for production rebasing

### Phase 2: Advanced Features (Medium Priority)

#### 5. Custom RPC Endpoints
**Location**: `node/src/rpc.rs`
- `rebase_get_status()` - Current rebase progress
- `rebase_schedule_proposal(height, approvals)` - Schedule rebase
- `rebase_get_archives()` - List available archives
- `rebase_validate_state(state_hash)` - Validate state integrity

#### 6. Off-chain Worker Integration
**Location**: `runtime/src/offchain.rs` (new)
- Monitor blockchain growth and trigger rebase alerts
- Automatic state export at configured intervals
- Health checks for archive storage availability
- Network coordination messaging
- **Automatic rebase triggering** at configured block intervals (optional)

#### 7. Cryptographic Continuity Validation
**Location**: `consensus/micc-primitives/src/`, `runtime/src/`
- Validator set signature validation across rebase
- State root Merkle proof verification
- Chain of custody tracking for state transitions
- Authority continuity proofs

### Phase 3: Production Features (Medium-Low Priority)

#### 8. Rebase Metadata and Version Tracking
**Location**: `runtime/src/storage.rs`, `pallets/rebase/` (new)
- On-chain rebase history storage
- Version anchoring with block height markers
- Rebase event emission for indexing
- Migration tracking between genesis versions

#### 9. Client Notification System
**Location**: `node/src/network/`, `client/rpc/`
- WebSocket notifications for pending rebases
- Client-side rebase preparation hooks
- Network announcement protocol
- Light client compatibility updates

### Phase 4: Optional Enhancements (Low Priority)

#### 10. Rolling Snapshot System
**Location**: `node/src/state/` (new)
- Multiple snapshot retention policies
- Progressive backup strategies
- Snapshot diff optimization
- Storage cost optimization

#### 11. Partial State Pruning
**Location**: `client/db/`, `consensus/micc-client/src/`
- Configurable pruning depth (retain N recent blocks)
- Selective pallet state retention
- Archive validation with partial state
- Client sync optimization

## File Structure

```
├── consensus/
│   └── micc-client/src/
│       └── rebase.rs                 # Core rebasing logic
├── node/src/
│   ├── command.rs                    # CLI rebase commands
│   ├── rpc.rs                        # Rebase RPC endpoints
│   └── service.rs                    # Rebase service integration
├── runtime/src/
│   ├── apis/
│   │   └── rebase.rs                 # Runtime rebase APIs
│   ├── configs/
│   │   └── rebase.rs                 # Rebase configuration
│   └── offchain.rs                   # Off-chain rebase monitoring
├── pallets/
│   └── rebase/                       # Rebase governance pallet
│       ├── src/lib.rs
│       ├── Cargo.toml
│       └── README.md
└── scripts/
    ├── rebase-demo.sh               # Development testing
    ├── rebase-production.sh         # Production procedures
    └── validate-rebase.sh           # Rebase validation
```

## Configuration

### Environment-Specific Settings
```rust
// In runtime/src/configs/environments.rs
pub struct RebaseParams {
    pub max_archive_retention_blocks: u32,
    pub rebase_proposal_min_approvals: u32,
    pub archive_storage_backends: Vec<StorageBackend>,
    pub state_export_chunk_size: u32,
    pub auto_rebase_enabled: bool,
    pub auto_rebase_interval_blocks: u32,
    pub auto_rebase_min_authorities_online: u32,
}

impl RebaseParams {
    pub fn for_environment(env: &Environment) -> Self {
        match env {
            Environment::Development => Self {
                max_archive_retention_blocks: 1000,
                rebase_proposal_min_approvals: 1,
                archive_storage_backends: vec![StorageBackend::Local],
                state_export_chunk_size: 1024 * 1024, // 1MB
                auto_rebase_enabled: true,
                auto_rebase_interval_blocks: 1000, // Every 1000 blocks (~8 minutes)
                auto_rebase_min_authorities_online: 1,
            },
            Environment::Local => Self {
                max_archive_retention_blocks: 5000,
                rebase_proposal_min_approvals: 2,
                archive_storage_backends: vec![StorageBackend::Local, StorageBackend::IPFS],
                state_export_chunk_size: 2 * 1024 * 1024, // 2MB
                auto_rebase_enabled: false, // Manual testing preferred
                auto_rebase_interval_blocks: 5000,
                auto_rebase_min_authorities_online: 2,
            },
            Environment::Staging => Self {
                max_archive_retention_blocks: 50_000,
                rebase_proposal_min_approvals: 2,
                archive_storage_backends: vec![StorageBackend::IPFS, StorageBackend::S3],
                state_export_chunk_size: 5 * 1024 * 1024, // 5MB
                auto_rebase_enabled: true,
                auto_rebase_interval_blocks: 50_000, // Every 50k blocks (~7 hours)
                auto_rebase_min_authorities_online: 3,
            },
            Environment::Production => Self {
                max_archive_retention_blocks: 100_000,
                rebase_proposal_min_approvals: 3,
                archive_storage_backends: vec![
                    StorageBackend::IPFS,
                    StorageBackend::S3,
                ],
                state_export_chunk_size: 10 * 1024 * 1024, // 10MB
                auto_rebase_enabled: false, // Governance-only for safety
                auto_rebase_interval_blocks: 100_000, // ~14 hours if enabled
                auto_rebase_min_authorities_online: 5,
            },
        }
    }
}
```

## Security Considerations

1. **State Integrity**: Cryptographic validation of exported state
2. **Authority Continuity**: Validator set signature verification
3. **Network Coordination**: Consensus on rebase timing and execution
4. **Archive Security**: Encrypted storage with access controls
5. **Rollback Protection**: Immutable rebase history tracking

## Testing Strategy

1. **Unit Tests**: Individual component validation
2. **Integration Tests**: Full rebase cycle testing
3. **Load Tests**: Large state export/import performance
4. **Security Tests**: Cryptographic validation testing
5. **Network Tests**: Multi-node coordination testing

## Deployment Procedure

1. **Development**: Local testing with mock archives
2. **Staging**: Multi-node rebase simulation
3. **Production**: Governance-approved coordinated rebase

## Success Metrics

- State export/import completion time < 30 minutes
- Archive storage redundancy > 99.9%
- Network downtime during rebase < 5 minutes
- State integrity validation success rate = 100%
- Client notification delivery rate > 95%