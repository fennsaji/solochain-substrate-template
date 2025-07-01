# 🚀 Solochain Substrate Template - MICC Consensus

A production-ready Substrate-based blockchain template with **MICC (Metamui Instant Confirmation Consensus)** and **fee-free transactions**. Includes comprehensive environment-specific configuration for development, local testing, staging, and production deployments.

## ⚡ Quick Start

```bash
# 1. Build the blockchain
cargo build --release

# 2. Start development blockchain  
./target/release/solochain-template-node --dev

# 3. Connect via browser
# Open: https://polkadot.js.org/apps/?rpc=ws://localhost:9944

# 4. Send fee-free transactions!
```

## 🌍 Environment-Specific Deployment

This blockchain supports **four distinct environments** with optimized configurations:

### 🛠️ **Development Environment** (Default)
**Purpose**: Local development and testing with maximum flexibility

```bash
# Quick start (uses development configuration)
./target/release/solochain-template-node --dev

# Or use build script
./scripts/build-environment.sh development release
./target/release/solochain-template-node --dev
```

**Configuration**:
- **Max Authorities**: 10 (flexible for testing)
- **Rate Limiting**: 1000 tx/block, 6000 tx/minute (very permissive)
- **Memory Limits**: 2MB per account
- **Block Production**: Flexible (allows multiple blocks per slot)
- **Block History**: 250 blocks (2 minutes)

**Features**:
- Alice has sudo access
- High transaction limits for testing
- Flexible validator setup
- Temporary database (--dev flag)

---

### 🏠 **Local Testnet Environment**
**Purpose**: Multi-node testing on local machine

```bash
# Build with local testnet configuration
./scripts/build-environment.sh local-testnet release

# Start local testnet
./target/release/solochain-template-node --chain local
```

**Configuration**:
- **Max Authorities**: 5 (small validator set)
- **Rate Limiting**: 500 tx/block, 3000 tx/minute (moderate)
- **Memory Limits**: 1MB per account
- **Block Production**: Secure (no multiple blocks per slot)
- **Block History**: 1200 blocks (10 minutes)

**Use Cases**:
- Multi-node testing
- Network behavior validation
- Performance testing

---

### 🧪 **Staging Environment**
**Purpose**: Production-like testing environment

```bash
# Build with staging configuration
./scripts/build-environment.sh staging release

# Start staging node
./target/release/solochain-template-node --chain staging --validator
```

**Configuration**:
- **Max Authorities**: 21 (production-like validator set)
- **Rate Limiting**: 200 tx/block, 1200 tx/minute (conservative)
- **Memory Limits**: 512KB per account
- **Block Production**: Secure (strict consensus rules)
- **Block History**: 2400 blocks (20 minutes)

**Requirements**:
- Insert staging validator keys before starting
- Configure staging genesis accounts
- Test all production procedures

---

### 🚀 **Production Environment**
**Purpose**: Live production deployment

```bash
# Build with production configuration (includes security prompts)
./scripts/build-environment.sh production release

# Start production node (requires production keys)
./target/release/solochain-template-node --chain production --validator
```

**Configuration**:
- **Max Authorities**: 32 (enterprise validator set)
- **Rate Limiting**: 100 tx/block, 600 tx/minute (secure)
- **Memory Limits**: 512KB per account (spam protection)
- **Block Production**: Maximum security
- **Block History**: 7200 blocks (1 hour)

**⚠️ Security Requirements**:
- **Generate production validator keys** (never use development keys)
- **Register unique SS58 prefix** (replace generic prefix 42)
- **Remove sudo access** (disabled in production genesis)
- **Configure infrastructure security** (DDoS protection, firewalls)
- **Enable monitoring and alerting**

---

## 📋 **Environment Comparison Table**

| Feature | Development | Local Testnet | Staging | Production |
|---------|-------------|---------------|---------|------------|
| **Use Case** | Local dev | Multi-node test | Pre-production | Live deployment |
| **Max Authorities** | 10 | 5 | 21 | 32 |
| **Tx Per Block** | 1000 | 500 | 200 | 100 |
| **Tx Per Minute** | 6000 | 3000 | 1200 | 600 |
| **Memory/Account** | 2MB | 1MB | 512KB | 512KB |
| **Block History** | 250 (2min) | 1200 (10min) | 2400 (20min) | 7200 (1hr) |
| **Sudo Access** | ✅ Alice | ⚠️ Limited | ⚠️ Limited | ❌ Disabled |
| **Multiple Blocks/Slot** | ✅ Yes | ❌ No | ❌ No | ❌ No |

---

## 🛠️ **Build and Deployment Scripts**

### Using Environment-Specific Build Script (Recommended)

```bash
# Development build
./scripts/build-environment.sh development release

# Local testnet build  
./scripts/build-environment.sh local-testnet release

# Staging build
./scripts/build-environment.sh staging release

# Production build (with security warnings)
./scripts/build-environment.sh production release
```

### Manual Cargo Commands

```bash
# Development (default - no features)
cargo build --release

# Local testnet
cargo build --release --features local-testnet

# Staging
cargo build --release --features staging

# Production
cargo build --release --features production
```

### Build Artifacts

Builds are organized in the `builds/` directory:
```
builds/
├── development-release/
│   ├── solochain-template-node
│   └── BUILD_INFO.md
├── local-testnet-release/
│   ├── solochain-template-node  
│   └── BUILD_INFO.md
├── staging-release/
│   ├── solochain-template-node
│   └── BUILD_INFO.md
└── production-release/
    ├── solochain-template-node
    └── BUILD_INFO.md
```

---

## 🚀 **Startup Commands**

### Development Startup
```bash
# Quick development start (temporary database)
./target/release/solochain-template-node --dev

# Development with persistent data
mkdir -p ./dev-data
./target/release/solochain-template-node --dev --base-path ./dev-data

# Development with detailed logging
RUST_LOG=debug ./target/release/solochain-template-node --dev
```

### Local Testnet Startup
```bash
# Single node local testnet
./target/release/solochain-template-node --chain local

# Multi-node local testnet (Node 1)
./target/release/solochain-template-node \
  --chain local \
  --validator \
  --port 30333 \
  --rpc-port 9944 \
  --base-path ./node1-data

# Multi-node local testnet (Node 2)  
./target/release/solochain-template-node \
  --chain local \
  --validator \
  --port 30334 \
  --rpc-port 9945 \
  --base-path ./node2-data \
  --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/NODE1_PEER_ID
```

### Staging Startup
```bash
# Insert staging keys first
./target/release/solochain-template-node key insert \
  --base-path ./staging-data \
  --chain staging \
  --scheme Sr25519 \
  --suri "YOUR_STAGING_SEED" \
  --key-type micc

# Start staging validator
./target/release/solochain-template-node \
  --chain staging \
  --validator \
  --base-path ./staging-data \
  --port 30333 \
  --rpc-port 9944
```

### Production Startup
```bash
# 1. Insert production keys (SECURE SEED REQUIRED)
./target/release/solochain-template-node key insert \
  --base-path ./production-data \
  --chain production \
  --scheme Sr25519 \
  --suri "YOUR_SECURE_PRODUCTION_SEED" \
  --key-type micc

./target/release/solochain-template-node key insert \
  --base-path ./production-data \
  --chain production \
  --scheme Ed25519 \
  --suri "YOUR_SECURE_PRODUCTION_SEED" \
  --key-type gran

# 2. Start production validator
./target/release/solochain-template-node \
  --chain production \
  --validator \
  --base-path ./production-data \
  --port 30333 \
  --rpc-port 9944 \
  --no-telemetry
```

---

## ✨ Key Features

- 🆓 **Zero Transaction Fees** - No fees required for any transactions
- ⚡ **MICC Consensus** - Custom event-driven consensus mechanism  
- 🏗️ **Event-Driven Blocks** - Blocks created when transactions arrive
- 🌐 **Full RPC API** - Complete Substrate JSON-RPC interface
- 🔗 **Polkadot-JS Compatible** - Works with all standard Substrate tools
- 🛡️ **GRANDPA Finality** - Proven Byzantine fault-tolerant finality
- 🌍 **Environment-Specific Configuration** - Optimized for each deployment context
- 🔒 **Production Security** - Comprehensive spam protection and security controls

## 🛡️ **Security & Rate Limiting**

### Spam Protection
The blockchain includes comprehensive **multi-layer spam protection**:

- **Per-Block Limits**: Prevents transaction flooding in single blocks
- **Time-Based Limits**: Rate limiting over time windows
- **Per-Account Limits**: Memory and transaction count limits per user
- **Emergency Controls**: System-wide pause functionality
- **Environment Tuning**: Conservative limits for production, permissive for development

### Transaction Extensions
```rust
// Integrated into transaction validation pipeline
pub type TxExtension = (
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_rate_limiter::CheckRateLimit<Runtime>, // 🔒 SPAM PROTECTION
    frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
    frame_system::WeightReclaim<Runtime>,
);
```

---

## 🧪 **Testing & Validation**

### Development Testing
```bash
# Start development blockchain
./target/release/solochain-template-node --dev

# Run all tests
cargo test

# Test specific component
cargo test -p pallet-rate-limiter
cargo test -p solochain-template-runtime
```

### Environment Validation
```bash
# Validate configuration for current environment
cargo run --features production --bin validate-config

# Check build configuration
./scripts/build-environment.sh staging release
cat builds/staging-release/BUILD_INFO.md
```

### Network Health Check
```bash
# Check node status
curl -H "Content-Type: application/json" \
     -d '{"id":1, "jsonrpc":"2.0", "method": "system_health", "params":[]}' \
     http://localhost:9944/

# Check block production
curl -H "Content-Type: application/json" \
     -d '{"id":1, "jsonrpc":"2.0", "method": "chain_getHeader", "params":[]}' \
     http://localhost:9944/
```

---

## 🔧 **Development & Customization**

### Build Requirements
- Rust 1.87.0 or later
- Substrate development environment

### Custom Pallets
The template includes modular consensus components:
- **MICC Consensus Pallet** (`consensus/micc/`) - Core consensus logic
- **MICC Client** (`consensus/micc-client/`) - Client consensus implementation  
- **MICC Primitives** (`consensus/micc-primitives/`) - Core consensus types
- **Rate Limiter Pallet** (`pallets/rate-limiter/`) - Spam protection
- **Slots Module** (`consensus/slots/`) - Time slot management

### Adding Custom Pallets
Add to `runtime/src/lib.rs`:

```rust
#[runtime::pallet_index(6)]
pub type YourPallet = your_pallet;
```

---

## 📚 **Documentation**

- **[Environment Configuration Guide](docs/ENVIRONMENT_CONFIGURATION.md)** - Detailed environment setup
- **[Security Audit Report](docs/AUDIT_FINDINGS.md)** - Comprehensive security analysis
- **[Production Security Guide](docs/PRODUCTION_SECURITY_GUIDE.md)** - Production deployment
- **[Security Fixes Tasks](docs/SECURITY_FIXES_TASKS.md)** - Implementation status

## 🔒 **Production Security Checklist**

Before production deployment:

### Security Implementation: ✅ **COMPLETE**
- [x] Multi-layer spam protection implemented
- [x] Consensus security vulnerabilities eliminated  
- [x] Resource exhaustion attacks mitigated
- [x] Panic-based vulnerabilities removed
- [x] Environment-specific configuration system
- [x] Comprehensive monitoring infrastructure

### Production Customization: **REQUIRED**
- [ ] Generate cryptographically secure validator keys
- [ ] Register unique SS58 prefix for network identity
- [ ] Create production chain specification without development keys
- [ ] Configure environment-specific parameters
- [ ] Remove or secure sudo access for production

### Operational Security: **RECOMMENDED**
- [ ] Deploy infrastructure DDoS protection
- [ ] Configure monitoring and alerting systems
- [ ] Implement validator firewall rules
- [ ] Enable equivocation slashing after testing
- [ ] Configure secure key rotation procedures

---

## 🏗️ **Architecture**

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Frontend      │    │   RPC API       │    │   MICC Node     │
│                 │───▶│                 │───▶│                 │
│ Polkadot-JS     │    │ ws://localhost  │    │ Event-driven    │
│ Custom Apps     │    │ :9944           │    │ Consensus       │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                                               │
                                               ▼
                                       ┌─────────────────┐
                                       │   Substrate     │
                                       │   Runtime       │
                                       │ - Rate Limiter  │
                                       │ - Balances      │
                                       │ - MICC          │
                                       │ - GRANDPA       │
                                       │ - System        │
                                       └─────────────────┘
```

## 🌟 **What Makes This Special**

### Fee-Free Transactions with Security
- **Zero transaction fees** for all operations
- **Comprehensive spam protection** prevents abuse
- **Multi-layer rate limiting** maintains network health
- **Emergency controls** for threat response

### Advanced Consensus
- **Event-driven block production** for instant finality
- **500ms block time** optimized for global networks
- **Enhanced equivocation detection** prevents double-spending
- **Authority performance monitoring** ensures network health

### Production-Ready Security
- **Environment-specific configuration** prevents misconfiguration
- **Compile-time safety** eliminates runtime configuration errors
- **Comprehensive security audit** validates implementation
- **Production hardening** with conservative security parameters

---

## 🤝 **Contributing**

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 **License**

This project is licensed under the Apache 2.0 License - see the [LICENSE](LICENSE) file for details.

## 🆘 **Support**

- **Issues**: [GitHub Issues](https://github.com/your-repo/issues)
- **Discussions**: [GitHub Discussions](https://github.com/your-repo/discussions)
- **Documentation**: See `docs/` directory

---

**🎉 Happy Building!** Your environment-configured, production-ready, fee-free blockchain is ready to deploy across development, staging, and production environments.