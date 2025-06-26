# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Substrate-based solochain template that has been customized with:
- **MICC Consensus**: Replaced Aura with custom MICC (Metamui Instant Confirmation Consensus) 
- **No Transaction Fees**: Transaction payment system has been completely removed
- **Custom Consensus Implementation**: Event-driven block production with transaction pool monitoring

## Key Architecture

### Consensus System
The project uses a custom MICC consensus system instead of the standard Aura:
- **Location**: `consensus/` directory contains the custom consensus modules
- **Components**:
  - `micc/` - Main consensus pallet (pallet-micc)
  - `micc-primitives/` - Consensus primitives (sp-consensus-micc) 
  - `micc-client/` - Client consensus logic (sc-consensus-micc)
  - `slots/` - Slot-based consensus utilities (sc-consensus-slots)

### Runtime Configuration 
- **File**: `runtime/src/lib.rs` - Main runtime composition
- **Pallets**: System, Timestamp, Micc, Grandpa, Balances, Sudo (pallet index 0-5)
- **No Fees**: TxExtension excludes `ChargeTransactionPayment`
- **Configuration**: `runtime/src/configs/mod.rs` contains pallet configurations

### Transaction Extensions
The runtime uses a fee-free transaction extension tuple:
```rust
pub type TxExtension = (
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>, 
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
    frame_system::WeightReclaim<Runtime>,
);
```

### Workspace Structure
- **Root**: Cargo workspace with custom consensus modules in workspace dependencies
- **Node**: `node/` - Blockchain node implementation using MICC consensus
- **Runtime**: `runtime/` - FRAME-based runtime without transaction fees
- **Consensus**: `consensus/` - Custom MICC consensus implementation

## Common Commands

### Building
```bash
# Build the entire project
cargo build --release

# Build specific components
cargo build -p solochain-template-runtime
cargo build -p solochain-template-node
```

### Running
```bash
# Development chain (no persistence)
./target/release/solochain-template-node --dev

# Development chain with detailed logging
RUST_BACKTRACE=1 ./target/release/solochain-template-node -ldebug --dev

# Purge development chain state
./target/release/solochain-template-node purge-chain --dev

# Persistent development chain
mkdir my-chain-state
./target/release/solochain-template-node --dev --base-path ./my-chain-state/
```

### Development
```bash
# Check compilation
cargo check --release

# Check specific packages
cargo check -p solochain-template-node
cargo check -p solochain-template-runtime

# Format code
cargo +nightly fmt

# Run clippy
cargo +nightly clippy

# Generate documentation
cargo +nightly doc --open
```

### Testing
```bash
# Run all tests
cargo test

# Run tests for specific package
cargo test -p pallet-micc
cargo test -p solochain-template-runtime
```

## Development Notes

### Consensus Modifications
- MICC consensus supports event-driven block production with transaction pool monitoring
- Force authoring mode available for development (allows any authority to claim any slot)
- Slot duration configurable via `MinimumPeriodTimesTwo` pattern

### Fee-Free Runtime
- All transaction payment logic has been removed
- Extrinsics execute without fees
- No weight-to-fee or length-to-fee calculations
- RPC endpoints for fee queries are not available

### Node Configuration
- Default development accounts: Alice (sudo), Bob
- Default ports: 9944 (WebSocket RPC), 9933 (HTTP RPC)
- Connect via Polkadot-JS Apps: https://polkadot.js.org/apps/#/explorer?rpc=ws://localhost:9944

### Key Files for Modifications
- **Runtime Changes**: `runtime/src/lib.rs`, `runtime/src/configs/mod.rs`
- **Consensus Changes**: Files in `consensus/` directory
- **Node Service**: `node/src/service.rs` (consensus startup logic)
- **RPC**: `node/src/rpc.rs` (custom RPC endpoints)
- **Chain Spec**: `node/src/chain_spec.rs` (genesis configuration)

### Toolchain
- Rust version: 1.87.0 (specified in rust-toolchain.toml)
- Includes clippy, rustfmt, rust-analyzer
- WebAssembly target: wasm32v1-none