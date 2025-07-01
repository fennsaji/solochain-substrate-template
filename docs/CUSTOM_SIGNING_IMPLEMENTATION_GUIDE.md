# 🔐 Custom Signing Method Implementation Guide for Substrate/Polkadot SDK

## 📋 **Overview**

This document provides a comprehensive guide for implementing your own custom signing method to override SR25519 and Ed25519 in your Substrate-based blockchain. The implementation involves creating custom cryptographic traits, integrating them with the consensus system, and ensuring proper runtime configuration.

**Target Audience**: Blockchain developers working with Substrate/Polkadot SDK  
**Prerequisites**: Rust programming, basic cryptography knowledge, Substrate framework familiarity

---

## 🧱 **Core Traits Required**

### **1. Core Cryptographic Traits (`sp-core`)**

The fundamental traits that define cryptographic operations in Substrate:

```rust
// The fundamental key pair trait
pub trait Pair: CryptoType + Sized {
    type Public: Public + Hash;
    type Seed: Default + AsRef<[u8]> + AsMut<[u8]> + Clone;
    type Signature: Signature;

    // Key generation methods
    fn generate() -> (Self, Self::Seed);
    fn generate_with_phrase(password: Option<&str>) -> (Self, String, Self::Seed);
    fn from_phrase(phrase: &str, password: Option<&str>) -> Result<(Self, Self::Seed), SecretStringError>;
    fn from_seed_slice(seed: &[u8]) -> Result<Self, SecretStringError>;

    // Key derivation
    fn derive<Iter: Iterator<Item = DeriveJunction>>(
        &self,
        path: Iter,
        seed: Option<Self::Seed>
    ) -> Result<(Self, Option<Self::Seed>), DeriveError>;

    // Cryptographic operations
    fn sign(&self, message: &[u8]) -> Self::Signature;
    fn verify<M: AsRef<[u8]>>(sig: &Self::Signature, message: M, pubkey: &Self::Public) -> bool;
    fn public(&self) -> Self::Public;
}

// Public key trait
pub trait Public: AsRef<[u8]> + AsMut<[u8]> + Default + Derive + CryptoType + PartialEq + Eq + Clone + Send + Sync {
    type Pair: Pair<Public = Self>;
    fn from_slice(data: &[u8]) -> Self;
}

// Signature trait
pub trait Signature: AsRef<[u8]> + AsMut<[u8]> + Default + Eq + PartialEq + Clone + Send + Sync {
    type Public: Public;
    fn verify<L: Lazy<[u8]>>(&self, msg: L, pubkey: &Self::Public) -> bool;
}
```

### **2. Application Crypto Traits (`sp-application-crypto`)**

These traits create application-specific wrappers around core crypto:

```rust
// Application-specific crypto wrapper
pub trait AppCrypto {
    type Public;
    type Pair;
    type Signature;
    const ID: KeyTypeId;           // 4-byte identifier like *b"mycs"
    const CRYPTO_ID: CryptoTypeId; // Unique crypto scheme ID
}

// Application-specific key pair
pub trait AppPair: AppCrypto + Pair<Public = Self::Public, Signature = Self::Signature> {
    type Generic; // The underlying generic pair type
}

// Application-specific public key
pub trait AppPublic: AppCrypto + Public + RuntimeAppPublic {
    type Generic; // The underlying generic public key type
}

// Runtime integration trait
pub trait RuntimeAppPublic: Sized {
    const ID: KeyTypeId;
    const CRYPTO_ID: CryptoTypeId;
    type Signature: Codec + Debug + MaybeDisplay + Eq + PartialEq + Clone + Send + Sync + 'static;
    
    fn all() -> Vec<Self>;
    fn generate_pair(seed: Option<Vec<u8>>) -> Self;
    fn sign<M: AsRef<[u8]>>(&self, msg: &M) -> Option<Self::Signature>;
    fn verify<M: AsRef<[u8]>>(&self, msg: &M, signature: &Self::Signature) -> bool;
    fn to_raw_vec(&self) -> Vec<u8>;
}
```

---

## 🛠️ **Implementation Steps**

### **Step 1: Create Your Custom Crypto Module**

Create a new pallet for your custom cryptographic implementation:

```bash
mkdir -p pallets/custom-crypto/src
```

**File: `pallets/custom-crypto/Cargo.toml`**

```toml
[package]
name = "pallet-custom-crypto"
version = "0.1.0"
authors.workspace = true
edition.workspace = true
license = "Unlicense"
homepage.workspace = true
repository.workspace = true
description = "Custom cryptographic implementation for substrate"

[package.metadata.docs.rs]
targets = ["x86_64-unknown-linux-gnu"]

[dependencies]
codec = { features = ["derive"], workspace = true }
scale-info = { features = ["derive"], workspace = true }

# Substrate
sp-core.workspace = true
sp-std.workspace = true
sp-runtime.workspace = true
sp-application-crypto.workspace = true

# Optional
sp-io = { workspace = true, optional = true }

[features]
default = ["std"]
std = [
    "codec/std",
    "scale-info/std",
    "sp-core/std",
    "sp-std/std",
    "sp-runtime/std",
    "sp-application-crypto/std",
    "sp-io?/std",
]
```

**File: `pallets/custom-crypto/src/lib.rs`**

```rust
#![cfg_attr(not(feature = "std"), no_std)]

use sp_core::{
    crypto::{CryptoType, CryptoTypeId, Derive, Public as PublicT, Signature as SignatureT},
    sr25519::{CRYPTO_ID as SR25519_CRYPTO_ID}, // Use as base or create new
};
use sp_std::{vec::Vec, convert::TryFrom};
use codec::{Encode, Decode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::traits::{Verify, IdentifyAccount};

// Your custom crypto type ID (must be unique)
pub const CRYPTO_ID: CryptoTypeId = CryptoTypeId(*b"mcst"); // "my custom"

/// Custom public key (32 bytes)
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, 
    Encode, Decode, MaxEncodedLen, TypeInfo
)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Public([u8; 32]);

impl PublicT for Public {
    fn from_slice(data: &[u8]) -> Self {
        let mut r = [0u8; 32];
        r.copy_from_slice(data);
        Self(r)
    }
}

impl AsRef<[u8]> for Public {
    fn as_ref(&self) -> &[u8] {
        &self.0[..]
    }
}

impl AsMut<[u8]> for Public {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0[..]
    }
}

impl Default for Public {
    fn default() -> Self {
        Public([0u8; 32])
    }
}

impl CryptoType for Public {
    type Pair = Pair;
}

impl Derive for Public {
    /// Implement key derivation logic
    fn derive<Iter: Iterator<Item = sp_core::crypto::DeriveJunction>>(
        &self,
        _path: Iter,
    ) -> Option<Self> {
        // TODO: Implement your custom key derivation algorithm
        // This is a placeholder - implement based on your cryptographic scheme
        Some(*self)
    }
}

/// Custom signature (64 bytes)
#[derive(
    Clone, Copy, PartialEq, Eq, Hash, 
    Encode, Decode, MaxEncodedLen, TypeInfo
)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Signature([u8; 64]);

impl SignatureT for Signature {
    fn verify<L: sp_core::crypto::Lazy<[u8]>>(&self, msg: L, signer: &Public) -> bool {
        self.verify_custom(msg.get(), signer)
    }
}

impl Signature {
    /// Custom signature verification logic
    pub fn verify_custom(&self, message: &[u8], public: &Public) -> bool {
        // TODO: Implement your custom signature verification algorithm
        // This is where you'd integrate your custom cryptographic scheme
        
        // Example placeholder logic - replace with your actual implementation
        // For demonstration, we're showing a simple hash-based verification
        // In practice, this should be your secure cryptographic verification
        
        #[cfg(feature = "std")]
        {
            use sp_core::hashing::blake2_256;
            
            // Placeholder: Simple hash-based verification (NOT secure, just for example)
            let expected_sig = blake2_256(&[message, public.as_ref()].concat());
            let provided_sig = &self.0[..32]; // Use first 32 bytes for comparison
            
            expected_sig == provided_sig
        }
        
        #[cfg(not(feature = "std"))]
        {
            // For no_std environments, implement your verification here
            // This is a placeholder that always returns true
            // In production, implement proper cryptographic verification
            true
        }
    }
}

impl AsRef<[u8]> for Signature {
    fn as_ref(&self) -> &[u8] {
        &self.0[..]
    }
}

impl AsMut<[u8]> for Signature {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0[..]
    }
}

impl Default for Signature {
    fn default() -> Self {
        Signature([0u8; 64])
    }
}

/// Custom key pair
#[derive(Clone)]
pub struct Pair {
    public: Public,
    secret: [u8; 32], // Your secret key format
}

impl sp_core::crypto::Pair for Pair {
    type Public = Public;
    type Seed = [u8; 32];
    type Signature = Signature;

    fn generate() -> (Self, Self::Seed) {
        // TODO: Implement secure random key generation
        // This is a placeholder - use proper cryptographic randomness
        let seed = [0u8; 32]; // Generate cryptographically secure random seed
        let pair = Self::from_seed_slice(&seed).expect("32 bytes; qed");
        (pair, seed)
    }

    fn generate_with_phrase(password: Option<&str>) -> (Self, String, Self::Seed) {
        // TODO: Implement mnemonic-based generation
        let (pair, seed) = Self::generate();
        let phrase = "custom mnemonic phrase".to_string(); // Generate actual BIP39 phrase
        (pair, phrase, seed)
    }

    fn from_phrase(
        phrase: &str,
        password: Option<&str>,
    ) -> Result<(Self, Self::Seed), sp_core::crypto::SecretStringError> {
        // TODO: Implement phrase-to-key conversion
        // Parse BIP39 mnemonic and derive seed
        let seed = [0u8; 32]; // Derive from phrase + password
        let pair = Self::from_seed_slice(&seed)?;
        Ok((pair, seed))
    }

    fn derive<Iter: Iterator<Item = sp_core::crypto::DeriveJunction>>(
        &self,
        path: Iter,
        _seed: Option<Self::Seed>,
    ) -> Result<(Self, Option<Self::Seed>), sp_core::crypto::DeriveError> {
        // TODO: Implement hierarchical key derivation
        // For now, return the same key (not secure for production)
        Ok((self.clone(), None))
    }

    fn from_seed_slice(seed: &[u8]) -> Result<Self, sp_core::crypto::SecretStringError> {
        if seed.len() != 32 {
            return Err(sp_core::crypto::SecretStringError::InvalidSeedLength);
        }
        
        let mut secret = [0u8; 32];
        secret.copy_from_slice(seed);
        
        // Generate public key from secret
        let public = Self::public_from_secret(&secret);
        
        Ok(Self { public, secret })
    }

    fn sign(&self, message: &[u8]) -> Self::Signature {
        // TODO: Implement your custom signing algorithm
        
        #[cfg(feature = "std")]
        {
            use sp_core::hashing::blake2_256;
            
            // Placeholder: Simple hash-based signing (NOT secure, just for example)
            let hash = blake2_256(&[message, self.public.as_ref()].concat());
            let mut signature_bytes = [0u8; 64];
            signature_bytes[..32].copy_from_slice(&hash);
            signature_bytes[32..].copy_from_slice(&self.secret);
            
            Signature(signature_bytes)
        }
        
        #[cfg(not(feature = "std"))]
        {
            // For no_std, implement your signing algorithm
            Signature([0u8; 64]) // Placeholder
        }
    }

    fn verify<M: AsRef<[u8]>>(sig: &Self::Signature, message: M, pubkey: &Self::Public) -> bool {
        sig.verify_custom(message.as_ref(), pubkey)
    }

    fn public(&self) -> Self::Public {
        self.public
    }
}

impl Pair {
    /// Generate public key from secret key (implement your algorithm)
    fn public_from_secret(secret: &[u8; 32]) -> Public {
        // TODO: Implement your public key generation from secret
        // This is a placeholder - implement based on your cryptographic scheme
        
        #[cfg(feature = "std")]
        {
            use sp_core::hashing::blake2_256;
            let public_bytes = blake2_256(secret);
            Public(public_bytes)
        }
        
        #[cfg(not(feature = "std"))]
        {
            // For no_std, implement your key generation
            Public([0u8; 32]) // Placeholder
        }
    }
}

impl CryptoType for Pair {
    type Pair = Pair;
}

// Runtime integration traits
impl Verify for Signature {
    type Signer = Public;
    
    fn verify<L: sp_core::crypto::Lazy<[u8]>>(&self, msg: L, signer: &Self::Signer) -> bool {
        self.verify_custom(msg.get(), signer)
    }
}

impl IdentifyAccount for Public {
    type AccountId = sp_runtime::AccountId32;
    
    fn into_account(self) -> Self::AccountId {
        sp_runtime::AccountId32::new(self.0)
    }
}

#[cfg(feature = "std")]
impl sp_std::fmt::Display for Public {
    fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        write!(f, "{}", sp_core::hexdisplay::HexDisplay::from(&self.0))
    }
}

#[cfg(feature = "std")]
impl sp_std::str::FromStr for Public {
    type Err = sp_core::crypto::PublicError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let d = sp_core::crypto::hex_decode(s)?;
        if d.len() != 32 {
            return Err(sp_core::crypto::PublicError::BadLength);
        }
        let mut r = [0u8; 32];
        r.copy_from_slice(&d);
        Ok(Public(r))
    }
}

// Additional implementations for full compatibility
impl From<Public> for [u8; 32] {
    fn from(x: Public) -> [u8; 32] {
        x.0
    }
}

impl From<[u8; 32]> for Public {
    fn from(x: [u8; 32]) -> Public {
        Public(x)
    }
}

impl From<Signature> for [u8; 64] {
    fn from(x: Signature) -> [u8; 64] {
        x.0
    }
}

impl From<[u8; 64]> for Signature {
    fn from(x: [u8; 64]) -> Signature {
        Signature(x)
    }
}
```

### **Step 2: Create Application-Specific Wrapper**

Update your MICC primitives to include your custom crypto:

**File: `consensus/micc-primitives/src/custom.rs`**

```rust
use sp_application_crypto::{app_crypto, KeyTypeId};
use sp_core::crypto::CryptoTypeId;

// Your key type identifier for MICC consensus
pub const CUSTOM_MICC: KeyTypeId = KeyTypeId(*b"mccs"); // "my custom consensus signature"

mod app_custom {
    use super::CUSTOM_MICC;
    use pallet_custom_crypto as custom; // Import your custom crypto module
    
    // Create application-specific crypto using your custom scheme
    app_crypto!(custom, CUSTOM_MICC);
}

sp_application_crypto::with_pair! {
    /// Custom authority key pair for MICC consensus
    pub type AuthorityPair = app_custom::Pair;
}

/// Custom authority signature for MICC consensus
pub type AuthoritySignature = app_custom::Signature;

/// Custom authority ID for MICC consensus
pub type AuthorityId = app_custom::Public;

// Implement RuntimeAppPublic for full integration
impl sp_application_crypto::RuntimeAppPublic for AuthorityId {
    const ID: KeyTypeId = CUSTOM_MICC;
    const CRYPTO_ID: CryptoTypeId = pallet_custom_crypto::CRYPTO_ID;
    type Signature = AuthoritySignature;

    fn all() -> sp_std::vec::Vec<Self> {
        sp_io::crypto::public_keys(Self::ID)
            .into_iter()
            .map(|key| Self::from_slice(&key))
            .collect()
    }

    fn generate_pair(seed: Option<sp_std::vec::Vec<u8>>) -> Self {
        sp_io::crypto::sr25519_generate(Self::ID, seed).into()
    }

    fn sign<M: AsRef<[u8]>>(&self, msg: &M) -> Option<Self::Signature> {
        sp_io::crypto::sr25519_sign(Self::ID, self, msg.as_ref())
            .map(|sig| Self::Signature::from_slice(&sig))
    }

    fn verify<M: AsRef<[u8]>>(&self, msg: &M, signature: &Self::Signature) -> bool {
        sp_io::crypto::sr25519_verify(signature, msg.as_ref(), self)
    }

    fn to_raw_vec(&self) -> sp_std::vec::Vec<u8> {
        self.as_ref().to_vec()
    }
}
```

### **Step 3: Update MICC Primitives**

**File: `consensus/micc-primitives/src/lib.rs`**

```rust
// Existing imports...
use sp_runtime::{
    traits::{Block as BlockT, Header as HeaderT, Verify},
    ConsensusEngineId, Justification,
};

// Add your custom module
pub mod custom;

// Keep existing sr25519 and ed25519 modules for backwards compatibility
pub mod sr25519 {
    mod app_sr25519 {
        use sp_application_crypto::{app_crypto, sr25519};
        use crate::MICC;
        app_crypto!(sr25519, MICC);
    }
    
    sp_application_crypto::with_pair! {
        pub type AuthorityPair = app_sr25519::Pair;
    }
    pub type AuthoritySignature = app_sr25519::Signature;
    pub type AuthorityId = app_sr25519::Public;
}

pub mod ed25519 {
    mod app_ed25519 {
        use sp_application_crypto::{app_crypto, ed25519};
        use crate::MICC;
        app_crypto!(ed25519, MICC);
    }
    
    sp_application_crypto::with_pair! {
        pub type AuthorityPair = app_ed25519::Pair;
    }
    pub type AuthoritySignature = app_ed25519::Signature;
    pub type AuthorityId = app_ed25519::Public;
}

// Re-export custom as the default
pub use custom::{AuthorityId, AuthorityPair, AuthoritySignature};

// Rest of the file remains the same...
```

### **Step 4: Update Runtime Configuration**

**File: `runtime/src/configs/mod.rs`**

```rust
use sp_consensus_micc::custom::AuthorityId as CustomMiccId;

// Configure MICC pallet to use your custom crypto
impl pallet_micc::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type DisabledValidators = ();
    type AuthorityId = CustomMiccId; // Use your custom authority ID
    type MaxAuthorities = ConstU32<32>;
    type WeightInfo = ();
}

// Update session keys to use custom crypto
impl_opaque_keys! {
    pub struct SessionKeys {
        pub micc: Micc,     // Your custom consensus keys
        pub grandpa: Grandpa,
    }
}

// Update the transaction extension to ensure signature verification works
pub type TxExtension = (
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_rate_limiter::CheckRateLimit<Runtime>,
    frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
    frame_system::WeightReclaim<Runtime>,
);
```

**File: `runtime/src/lib.rs`**

```rust
// Update the runtime to include your custom crypto pallet
construct_runtime!(
    pub enum Runtime
    {
        System: frame_system,
        Timestamp: pallet_timestamp,
        Micc: pallet_micc,
        Grandpa: pallet_grandpa,
        Balances: pallet_balances,
        Sudo: pallet_sudo,
        RateLimiter: pallet_rate_limiter,
        CustomCrypto: pallet_custom_crypto, // Add your pallet
    }
);

// Update the signature type to support your custom signatures
pub type Signature = sp_runtime::MultiSignature;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
```

### **Step 5: Update Node Service**

**File: `node/src/service.rs`**

```rust
use sp_consensus_micc::custom::AuthorityPair as CustomMiccPair;

// Update the import queue
let import_queue =
    sc_consensus_micc::import_queue::<CustomMiccPair, _, _, _, _, _>(ImportQueueParams {
        block_import: grandpa_block_import.clone(),
        justification_import: Some(Box::new(grandpa_block_import.clone())),
        client: client.clone(),
        create_inherent_data_providers: move |parent_hash, _| {
            let cidp_client = cidp_client.clone();
            async move {
                let slot_duration = sc_consensus_micc::standalone::slot_duration_at(
                    &*cidp_client,
                    parent_hash,
                )?;
                let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

                let slot =
                    sp_consensus_micc::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
                        *timestamp,
                        slot_duration,
                    );

                Ok((slot, timestamp))
            }
        },
        spawner: &task_manager.spawn_essential_handle(),
        registry: config.prometheus_registry(),
        check_for_equivocation: Default::default(),
        telemetry: telemetry.as_ref().map(|x| x.handle()),
        compatibility_mode: Default::default(),
    })?;

// Update consensus startup
if role.is_authority() {
    let proposer_factory = sc_basic_authorship::ProposerFactory::new(
        task_manager.spawn_handle(),
        client.clone(),
        transaction_pool.clone(),
        prometheus_registry.as_ref(),
        telemetry.as_ref().map(|x| x.handle()),
    );

    let slot_duration = sc_consensus_micc::slot_duration(&*client)?;

    let micc = sc_consensus_micc::start_micc::<CustomMiccPair, _, _, _, _, _, _, _, _, _, _, _>(
        StartMiccParams {
            slot_duration,
            client,
            select_chain,
            block_import,
            proposer_factory,
            create_inherent_data_providers: move |_, ()| async move {
                let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

                let slot =
                    sp_consensus_micc::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
                        *timestamp,
                        slot_duration,
                    );

                Ok((slot, timestamp))
            },
            force_authoring,
            backoff_authoring_blocks,
            keystore: keystore_container.keystore(),
            sync_oracle: sync_service.clone(),
            justification_sync_link: sync_service.clone(),
            block_proposal_slot_portion: SlotProportion::new(2f32 / 3f32),
            max_block_proposal_slot_portion: None,
            telemetry: telemetry.as_ref().map(|x| x.handle()),
            compatibility_mode: Default::default(),
        },
        transaction_pool.clone(),
    )?;

    // the MICC authoring task is considered essential
    task_manager
        .spawn_essential_handle()
        .spawn_blocking("micc", Some("block-authoring"), micc);
}
```

### **Step 6: Update Workspace Configuration**

**File: `Cargo.toml`**

```toml
[workspace]
members = [
    "node",
    "runtime",
    "consensus/micc-primitives",
    "consensus/micc-client",
    "consensus/micc",
    "consensus/slots",
    "pallets/rate-limiter",
    "pallets/custom-crypto", # Add your custom crypto pallet
]

[workspace.dependencies]
# Add your custom crypto pallet
pallet-custom-crypto = { path = "./pallets/custom-crypto", default-features = false }

# ... rest of dependencies
```

---

## 📚 **Key Polkadot SDK References**

### **Essential Crates to Study:**

1. **`sp-core`** - Core cryptographic traits and implementations
   - Location: `substrate/primitives/core/src/`
   - Key files: `crypto.rs`, `sr25519.rs`, `ed25519.rs`

2. **`sp-application-crypto`** - Application-specific crypto wrappers
   - Location: `substrate/primitives/application-crypto/src/`
   - Key files: `lib.rs`, `sr25519.rs`, `ed25519.rs`

3. **`sp-runtime`** - Runtime integration traits
   - Location: `substrate/primitives/runtime/src/`
   - Key files: `traits.rs`, `generic/unchecked_extrinsic.rs`

4. **`sp-consensus-aura`** - Similar consensus pattern to study
   - Location: `substrate/primitives/consensus/aura/src/`

### **Repository to Clone and Study:**

```bash
# Clone the Polkadot SDK repository
git clone https://github.com/paritytech/polkadot-sdk.git
cd polkadot-sdk

# Study these key implementations:
substrate/primitives/core/src/sr25519.rs              # SR25519 implementation
substrate/primitives/core/src/ed25519.rs              # Ed25519 implementation
substrate/primitives/application-crypto/src/sr25519.rs # Application crypto wrapper
substrate/primitives/application-crypto/src/ed25519.rs # Application crypto wrapper
substrate/client/consensus/aura/src/lib.rs            # Consensus integration pattern
substrate/client/consensus/babe/src/lib.rs            # Another consensus example
```

### **Documentation References:**

- **Rust Docs**: https://docs.rs/sp-core/latest/sp_core/crypto/
- **Substrate Documentation**: https://docs.substrate.io/reference/cryptography/
- **Polkadot Wiki**: https://wiki.polkadot.network/docs/learn-cryptography

---

## ⚠️ **Important Security Considerations**

### **Cryptographic Security Requirements:**

1. **Algorithm Security**
   - Your signing algorithm must be cryptographically secure
   - Ensure resistance to known attacks (birthday, collision, etc.)
   - Consider quantum resistance if required

2. **Implementation Security**
   - Use constant-time implementations to prevent timing attacks
   - Implement proper key generation with secure randomness
   - Validate all inputs and handle edge cases

3. **Key Management**
   - Secure key storage and handling
   - Proper key derivation functions
   - Secure key backup and recovery procedures

### **Performance Considerations:**

1. **Consensus Requirements**
   - Signing must be fast enough for 500ms block times
   - Verification should be efficient for block validation
   - Consider batch verification if supported

2. **Resource Usage**
   - Keep signature and key sizes reasonable
   - Optimize memory usage for embedded systems
   - Consider network bandwidth for signature propagation

### **Testing Requirements:**

1. **Unit Tests**
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn test_key_generation() {
           let (pair, _seed) = Pair::generate();
           assert_ne!(pair.public(), Public::default());
       }

       #[test]
       fn test_sign_verify() {
           let (pair, _) = Pair::generate();
           let message = b"test message";
           let signature = pair.sign(message);
           assert!(Pair::verify(&signature, message, &pair.public()));
       }

       #[test]
       fn test_deterministic_generation() {
           let seed = [42u8; 32];
           let pair1 = Pair::from_seed_slice(&seed).unwrap();
           let pair2 = Pair::from_seed_slice(&seed).unwrap();
           assert_eq!(pair1.public(), pair2.public());
       }
   }
   ```

2. **Integration Tests**
   - Test with actual consensus
   - Verify session key handling
   - Test transaction signing and verification

3. **Security Tests**
   - Fuzzing with random inputs
   - Performance benchmarks
   - Security audit by cryptography experts

---

## 🔧 **Build and Testing Instructions**

### **Building Your Custom Implementation:**

```bash
# Add your custom crypto pallet to the workspace
echo 'pallet-custom-crypto = { path = "./pallets/custom-crypto", default-features = false }' >> Cargo.toml

# Build the entire project
cargo build --release

# Run tests
cargo test -p pallet-custom-crypto
cargo test -p solochain-template-runtime

# Check that everything compiles
cargo check --all
```

### **Testing Your Implementation:**

```bash
# Test key generation and signing
cargo test -p pallet-custom-crypto test_sign_verify

# Test runtime integration
cargo test -p solochain-template-runtime

# Start development blockchain with custom crypto
./target/release/solochain-template-node --dev

# Insert custom keys (example)
./target/release/solochain-template-node key insert \
  --base-path /tmp/alice \
  --chain local \
  --scheme Custom \  # Your custom scheme name
  --suri "//Alice" \
  --key-type mccs
```

### **Validation Checklist:**

- [ ] Custom crypto module compiles without errors
- [ ] All traits properly implemented
- [ ] Unit tests pass for crypto operations
- [ ] Runtime integrates custom crypto successfully
- [ ] Node starts with custom crypto configuration
- [ ] Keys can be generated and inserted
- [ ] Blocks can be produced and signed
- [ ] Signatures can be verified
- [ ] Performance meets requirements

---

## 🚨 **Common Pitfalls and Solutions**

### **1. Trait Implementation Issues**

**Problem**: Missing trait implementations or incorrect bounds
```rust
error[E0277]: the trait bound `MyPublic: sp_runtime::traits::Verify` is not satisfied
```

**Solution**: Ensure all required traits are implemented:
```rust
impl Verify for MySignature {
    type Signer = MyPublic;
    fn verify<L: sp_core::crypto::Lazy<[u8]>>(&self, msg: L, signer: &Self::Signer) -> bool {
        // Implementation
    }
}
```

### **2. Keystore Integration Problems**

**Problem**: Keys not found in keystore
```rust
Error: Consensus error: Cannot sign: key not found
```

**Solution**: Ensure proper key type registration and insertion:
```rust
// Register your key type
pub const MY_KEY_TYPE: KeyTypeId = KeyTypeId(*b"myky");

// Insert keys properly
./target/release/solochain-template-node key insert \
  --key-type myky \
  --scheme Custom \
  --suri "//Alice"
```

### **3. Runtime Configuration Errors**

**Problem**: Type mismatches in runtime configuration
```rust
error[E0308]: mismatched types: expected `sr25519::Public`, found `custom::Public`
```

**Solution**: Update all type aliases consistently:
```rust
// In runtime configuration
type AuthorityId = sp_consensus_micc::custom::AuthorityId;

// In session keys
impl_opaque_keys! {
    pub struct SessionKeys {
        pub micc: Micc,  // Must match the configured type
        pub grandpa: Grandpa,
    }
}
```

---

## 📖 **Additional Resources**

### **Learning Materials:**

1. **Cryptography Foundations**
   - "Applied Cryptography" by Bruce Schneier
   - "Cryptography Engineering" by Ferguson, Schneier, and Kohno
   - Online courses on cryptographic protocols

2. **Substrate-Specific Resources**
   - Substrate Developer Hub: https://docs.substrate.io/
   - Substrate Seminar videos: https://substrate.io/ecosystem/resources/seminar/
   - Polkadot Wiki: https://wiki.polkadot.network/

3. **Code Examples**
   - Substrate Node Template: https://github.com/substrate-developer-hub/substrate-node-template
   - Polkadot SDK Examples: https://github.com/paritytech/polkadot-sdk/tree/master/substrate/frame/examples

### **Community Support:**

- **Substrate Stack Exchange**: https://substrate.stackexchange.com/
- **Polkadot Discord**: Community channels for developer support
- **GitHub Discussions**: Polkadot SDK repository discussions

---

## 🎯 **Conclusion**

Implementing a custom signing method in Substrate requires careful attention to:

1. **Cryptographic Security** - Your algorithm must be secure and properly implemented
2. **Trait Implementation** - All required traits must be correctly implemented
3. **Integration** - Proper integration with consensus, runtime, and keystore
4. **Testing** - Comprehensive testing for security and functionality
5. **Performance** - Meeting consensus timing requirements

This guide provides the foundation for implementing your custom cryptographic scheme. Remember to:

- Start with thorough testing of your cryptographic primitives
- Gradually integrate with the Substrate framework
- Conduct security audits before production use
- Follow Substrate best practices for consensus integration

**Next Steps:**
1. Implement your core cryptographic algorithm
2. Create comprehensive unit tests
3. Integrate with the runtime step by step
4. Test with actual consensus scenarios
5. Consider formal security audits

Good luck with your custom signing implementation! 🚀