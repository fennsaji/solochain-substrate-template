# 🧩 MICC Primitives - Core Consensus Types

> **Fundamental building blocks** for MICC (Metamui Instant Confirmation Consensus) 

## 📋 Overview

MICC Primitives provides the **core types, traits, and cryptographic foundations** that power the MICC consensus system. This crate defines the essential data structures and interfaces used throughout the MICC consensus implementation.

**Key Responsibilities:**
- 🔑 **Cryptographic Types** - Authority keys and signatures
- 🏷️ **Consensus Identity** - Unique engine identifier
- 📊 **Runtime API** - Interface for runtime communication
- 🔄 **Consensus Logs** - Authority change notifications
- 📦 **Inherent Data** - Slot information for blocks

## 🏗️ Core Components

### **1. Consensus Engine Identity**
```rust
/// Unique identifier for MICC consensus engine
pub const MICC_ENGINE_ID: ConsensusEngineId = [b'm', b'i', b'c', b'c'];

/// Inherent identifier for slot information
pub const MICC_INHERENT_IDENTIFIER: InherentIdentifier = *b"miccslot";
```

### **2. Cryptographic Types**

#### **Authority Identification**
```rust
/// Sr25519 signature scheme for MICC consensus
pub type AuthorityId = sp_application_crypto::sr25519::Public;
pub type AuthorityPair = sp_application_crypto::sr25519::Pair;
pub type AuthoritySignature = sp_application_crypto::sr25519::Signature;

/// Cryptographic app-crypto implementation
app_crypto!(sr25519, MICC);
```

#### **Key Features**
- **Sr25519 Cryptography**: State-of-the-art signature scheme
- **HD Key Derivation**: Support for hierarchical deterministic keys
- **Substrate Integration**: Seamless keystore integration

### **3. Slot Types**
```rust
/// Slot number type for consensus rounds
pub type Slot = sp_consensus_slots::Slot;

/// Slot duration in milliseconds
pub type SlotDuration = u64;
```

### **4. Consensus Logs**
```rust
/// Consensus log entries for authority management
#[derive(Decode, Encode, Clone, PartialEq, Eq)]
pub enum ConsensusLog<AuthorityId> {
    /// Authorities have changed
    #[codec(index = 1)]
    AuthoritiesChange(Vec<AuthorityId>),
    
    /// An authority has been disabled
    #[codec(index = 2)]  
    OnDisabled(AuthorityIndex),
}
```

## 🔧 Key Modules

### **1. Digests Module (`digests.rs`)**

Handles pre-runtime digests and consensus seals for blocks.

#### **Pre-Runtime Digests**
```rust
/// Extract slot from block pre-runtime digests
pub fn extract_pre_digest<AuthorityId>(
    header: &Header,
) -> Result<Slot, String> {
    // Parse consensus digest items
    for digest_item in &header.digest().logs {
        if let DigestItem::PreRuntime(engine_id, data) = digest_item {
            if engine_id == &MICC_ENGINE_ID {
                return Slot::decode(&mut &data[..])
                    .map_err(|_| "Invalid slot encoding".to_string());
            }
        }
    }
    Err("No MICC pre-runtime digest found".to_string())
}
```

#### **Consensus Seals**
```rust
/// Create MICC consensus seal for block
pub fn micc_seal<AuthorityId>(
    signature: &AuthoritySignature,
) -> DigestItem {
    DigestItem::Seal(MICC_ENGINE_ID, signature.encode())
}

/// Extract and verify consensus seal
pub fn extract_micc_seal<AuthorityId>(
    header: &Header,
) -> Result<AuthoritySignature, String> {
    // Find and validate MICC seal
    for digest_item in &header.digest().logs {
        if let DigestItem::Seal(engine_id, signature_data) = digest_item {
            if engine_id == &MICC_ENGINE_ID {
                return AuthoritySignature::decode(&mut &signature_data[..])
                    .map_err(|_| "Invalid signature encoding".to_string());
            }
        }
    }
    Err("No MICC seal found".to_string())
}
```

### **2. Inherents Module (`inherents.rs`)**

Manages slot inherent data for block production.

#### **Slot Inherent Provider**
```rust
/// Provides slot information as inherent data
#[derive(Encode, Clone, PartialEq, Eq)]
pub struct InherentDataProvider {
    slot: Slot,
}

impl InherentDataProvider {
    /// Create new inherent data provider for given slot
    pub fn new(slot: Slot) -> Self {
        Self { slot }
    }
    
    /// Create provider from timestamp and slot duration
    pub fn from_timestamp_and_slot_duration(
        timestamp: u64,
        slot_duration: SlotDuration,
    ) -> Self {
        let slot = Slot::from_timestamp(
            Timestamp::new(timestamp),
            SlotDuration::from_millis(slot_duration)
        );
        Self::new(slot)
    }
}

#[async_trait::async_trait]
impl sp_inherents::InherentDataProvider for InherentDataProvider {
    async fn provide_inherent_data(
        &self,
        inherent_data: &mut InherentData,
    ) -> Result<(), sp_inherents::Error> {
        inherent_data.put_data(MICC_INHERENT_IDENTIFIER, &self.slot)
    }
}
```

#### **Inherent Data Validation**
```rust
/// Extract slot from inherent data
pub fn extract_inherent_slot(
    inherent_data: &InherentData,
) -> Result<Slot, sp_inherents::Error> {
    inherent_data
        .get_data::<Slot>(&MICC_INHERENT_IDENTIFIER)?
        .ok_or_else(|| {
            sp_inherents::Error::InherentDataNotFound(
                MICC_INHERENT_IDENTIFIER
            )
        })
}
```

## 🔗 Runtime API

### **MICC Runtime API Definition**
```rust
sp_api::decl_runtime_apis! {
    /// API for MICC consensus
    pub trait MiccApi<AuthorityId: Codec> {
        /// Return the slot duration in milliseconds
        fn slot_duration() -> SlotDuration;
        
        /// Return the current set of authorities
        fn authorities() -> Vec<AuthorityId>;
        
        /// Return the current slot number
        fn current_slot() -> Slot;
        
        /// Submit report about an equivocation
        fn submit_report_equivocation_unsigned_extrinsic(
            equivocation_proof: EquivocationProof<Header, AuthorityId>,
            key_owner_proof: KeyOwnerProof,
        ) -> Option<()>;
        
        /// Generate a key ownership proof
        fn generate_key_ownership_proof(
            slot: Slot,
            authority_id: AuthorityId,
        ) -> Option<KeyOwnerProof>;
    }
}
```

### **Runtime Integration Example**
```rust
// In runtime implementation
impl sp_consensus_micc::MiccApi<Block, MiccId> for Runtime {
    fn slot_duration() -> SlotDuration {
        Micc::slot_duration()
    }
    
    fn authorities() -> Vec<MiccId> {
        Micc::authorities()
    }
    
    fn current_slot() -> Slot {
        Micc::current_slot()
    }
    
    fn submit_report_equivocation_unsigned_extrinsic(
        equivocation_proof: EquivocationProof<Header, MiccId>,
        key_owner_proof: KeyOwnerProof,
    ) -> Option<()> {
        // Handle equivocation reporting
        Some(())
    }
    
    fn generate_key_ownership_proof(
        slot: Slot,
        authority_id: MiccId,
    ) -> Option<KeyOwnerProof> {
        // Generate key ownership proof
        Some(KeyOwnerProof::default())
    }
}
```

## 🔐 Cryptographic Operations

### **Authority Key Management**
```rust
/// Generate new authority keypair
pub fn generate_authority_keypair() -> (AuthorityId, AuthorityPair) {
    let pair = AuthorityPair::generate();
    let public = pair.public();
    (public, pair)
}

/// Sign data with authority key
pub fn sign_with_authority(
    pair: &AuthorityPair,
    data: &[u8],
) -> AuthoritySignature {
    pair.sign(data)
}

/// Verify authority signature
pub fn verify_authority_signature(
    public: &AuthorityId,
    signature: &AuthoritySignature,
    data: &[u8],
) -> bool {
    signature.verify(data, public)
}
```

### **Key Derivation**
```rust
/// Derive authority key from seed
pub fn derive_authority_from_seed(seed: &str) -> AuthorityPair {
    AuthorityPair::from_string(seed, None)
        .expect("Valid seed for authority derivation")
}

/// Well-known development keys
pub mod dev_keys {
    use super::*;
    
    pub fn alice() -> AuthorityPair {
        derive_authority_from_seed("//Alice")
    }
    
    pub fn bob() -> AuthorityPair {
        derive_authority_from_seed("//Bob")
    }
    
    pub fn charlie() -> AuthorityPair {
        derive_authority_from_seed("//Charlie")
    }
}
```

## 🛡️ Security Features

### **Authority Validation**
```rust
/// Validate authority list
pub fn validate_authorities(authorities: &[AuthorityId]) -> Result<(), String> {
    if authorities.is_empty() {
        return Err("Authority list cannot be empty".to_string());
    }
    
    if authorities.len() > MAX_AUTHORITIES {
        return Err("Too many authorities".to_string());
    }
    
    // Check for duplicates
    let mut sorted = authorities.to_vec();
    sorted.sort();
    sorted.dedup();
    
    if sorted.len() != authorities.len() {
        return Err("Duplicate authorities found".to_string());
    }
    
    Ok(())
}
```

### **Signature Verification**
```rust
/// Verify block signature
pub fn verify_block_signature<Header>(
    header: &Header,
    authority: &AuthorityId,
) -> Result<(), String>
where
    Header: HeaderT,
{
    let seal = extract_micc_seal(header)?;
    let pre_hash = header.hash();
    
    if seal.verify(pre_hash.as_ref(), authority) {
        Ok(())
    } else {
        Err("Invalid block signature".to_string())
    }
}
```

## 📊 Constants and Configuration

### **Default Values**
```rust
/// Default slot duration (6 seconds)
pub const DEFAULT_SLOT_DURATION: SlotDuration = 6000;

/// Maximum number of authorities
pub const MAX_AUTHORITIES: usize = 1000;

/// Proposing time as percentage of slot duration
pub const PROPOSING_TIME_PERCENT: f32 = 0.9;

/// Minimum proposing time in milliseconds
pub const MIN_PROPOSING_TIME_MS: u64 = 100;
```

### **Configuration Helpers**
```rust
/// Calculate slot from timestamp
pub fn slot_from_timestamp(
    timestamp: u64,
    slot_duration: SlotDuration,
    genesis_time: u64,
) -> Slot {
    Slot::from((timestamp.saturating_sub(genesis_time)) / slot_duration)
}

/// Calculate next slot timing
pub fn next_slot_timing(
    current_slot: Slot,
    slot_duration: SlotDuration,
    genesis_time: u64,
) -> (Slot, u64) {
    let next_slot = current_slot + 1;
    let next_timestamp = genesis_time + (*next_slot * slot_duration);
    (next_slot, next_timestamp)
}
```

## 🔗 Integration with Other Modules

### **With MICC Pallet**
```rust
// Shared authority and slot types
use sp_consensus_micc::{AuthorityId, Slot, ConsensusLog};

// Runtime API implementation
impl sp_consensus_micc::MiccApi<Block, AuthorityId> for Runtime {
    // API implementation
}
```

### **With MICC Client**
```rust
// Authority verification in block production
use sp_consensus_micc::{
    AuthorityId, 
    AuthoritySignature,
    verify_authority_signature
};

// Inherent data creation
use sp_consensus_micc::inherents::InherentDataProvider;
```

### **With Substrate Core**
```rust
// Consensus engine integration
use sp_consensus_micc::MICC_ENGINE_ID;
use sp_consensus::Error as ConsensusError;

// Runtime API usage
use sp_api::ProvideRuntimeApi;
```

## 🧪 Testing Utilities

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_authority_generation() {
        let (public, pair) = generate_authority_keypair();
        let data = b"test data";
        let signature = sign_with_authority(&pair, data);
        
        assert!(verify_authority_signature(&public, &signature, data));
    }
    
    #[test]
    fn test_slot_calculations() {
        let timestamp = 1000000;
        let slot_duration = 6000;
        let genesis_time = 0;
        
        let slot = slot_from_timestamp(timestamp, slot_duration, genesis_time);
        assert_eq!(*slot, timestamp / slot_duration);
    }
    
    #[test]
    fn test_digest_operations() {
        let slot = Slot::from(42);
        let digest = create_pre_runtime_digest(slot);
        let extracted = extract_pre_digest(&digest).unwrap();
        assert_eq!(slot, extracted);
    }
}
```

## 📚 Related Documentation

- **[MICC Pallet](../micc/README.md)** - Runtime consensus integration
- **[MICC Client](../micc-client/README.md)** - Block production engine  
- **[Slots](../slots/README.md)** - Slot timing utilities
- **[Substrate Cryptography](https://docs.substrate.io/reference/cryptography/)** - Cryptographic primitives
- **[Substrate Consensus](https://docs.substrate.io/fundamentals/consensus/)** - Consensus fundamentals

## 📜 License

Apache-2.0

## 🏷️ Release

Based on Polkadot SDK stable2409 with MICC consensus primitives.

---

> 🔐 **Security Foundation**: MICC Primitives provides the cryptographic and type-safe foundation that ensures the security and integrity of the entire MICC consensus system.