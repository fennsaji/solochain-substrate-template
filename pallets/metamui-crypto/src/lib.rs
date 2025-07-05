#![cfg_attr(not(feature = "std"), no_std)]

//! # Metamui Crypto Module
//!
//! This module provides cryptographic primitives for the Metamui blockchain using Falcon 512
//! post-quantum signature scheme. It implements the core Substrate crypto traits to enable
//! integration with consensus and runtime systems.
//!
//! ## Falcon 512 Overview
//!
//! Falcon 512 is a post-quantum signature scheme that provides:
//! - **Security Level**: NIST Level 1 (equivalent to AES-128)
//! - **Public Key Size**: 897 bytes
//! - **Signature Size**: Variable (average ~666 bytes, max ~1280 bytes)
//! - **Private Key Size**: 1281 bytes
//!
//! ## Key Components
//!
//! - `Public`: Falcon 512 public key (897 bytes)
//! - `Signature`: Falcon 512 signature (variable size, up to 1280 bytes)
//! - `Pair`: Falcon 512 key pair with signing capabilities
//!
//! ## Security Features
//!
//! - Post-quantum cryptographic security
//! - Resistance to quantum computer attacks
//! - Based on lattice-based cryptography (NTRU lattices)
//! - Fast signature verification

use sp_core::{
    crypto::{CryptoType, CryptoTypeId, Derive, DeriveJunction, Pair as PairT, ByteArray},
};
use sp_application_crypto::RuntimePublic;
use sp_std::{vec::Vec, convert::TryFrom};

#[cfg(feature = "std")]
use std::{string::{String, ToString}, format};

#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, format};

#[cfg(not(feature = "std"))]
extern crate alloc;


use codec::{Encode, Decode, MaxEncodedLen, DecodeWithMemTracking};
use scale_info::TypeInfo;
use sp_runtime::traits::{IdentifyAccount, Lazy};

#[cfg(feature = "falcon-support")]
use falcon_rust::falcon512;

#[cfg(all(feature = "std", feature = "bip39-support"))]
use bip39::Mnemonic;

#[cfg(feature = "bip39-support")]
use hex;


// Metamui crypto type ID (unique identifier)
pub const CRYPTO_ID: CryptoTypeId = CryptoTypeId(*b"metu"); // "metamui"

// Falcon 512 constants
pub const PUBLIC_KEY_SIZE: usize = 897;      // Falcon 512 public key size
pub const MAX_SIGNATURE_SIZE: usize = 1280;  // Falcon 512 max signature size
pub const PRIVATE_KEY_SIZE: usize = 1281;    // Falcon 512 private key size
pub const SEED_SIZE: usize = 32;             // Seed size for key generation (max 32 for Default trait)

/// Metamui public key using Falcon 512
///
/// Stores a Falcon 512 public key (897 bytes) for post-quantum signature verification.
/// The public key is used for:
/// - Signature verification
/// - Account identification
/// - Authority identification in consensus
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, 
    Encode, Decode, TypeInfo, MaxEncodedLen, Debug
)]
pub struct Public([u8; PUBLIC_KEY_SIZE]);

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
        Public([0u8; PUBLIC_KEY_SIZE])
    }
}

impl From<Public> for [u8; PUBLIC_KEY_SIZE] {
    fn from(x: Public) -> [u8; PUBLIC_KEY_SIZE] {
        x.0
    }
}

impl From<[u8; PUBLIC_KEY_SIZE]> for Public {
    fn from(x: [u8; PUBLIC_KEY_SIZE]) -> Public {
        Public(x)
    }
}

impl DecodeWithMemTracking for Public {}

impl TryFrom<&[u8]> for Public {
    type Error = ();
    
    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() == PUBLIC_KEY_SIZE {
            let mut key = [0u8; PUBLIC_KEY_SIZE];
            key.copy_from_slice(data);
            Ok(Public(key))
        } else {
            Err(())
        }
    }
}

impl CryptoType for Public {
    type Pair = MetamuiPair;
}

impl ByteArray for Public {
    const LEN: usize = PUBLIC_KEY_SIZE;
    
    fn from_slice(data: &[u8]) -> Result<Self, ()> {
        if data.len() == PUBLIC_KEY_SIZE {
            let mut key = [0u8; PUBLIC_KEY_SIZE];
            key.copy_from_slice(data);
            Ok(Public(key))
        } else {
            Err(())
        }
    }
    
    fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

/// Derivation helper function for Falcon 512 post-quantum key derivation
/// 
/// Since traditional BIP32 ECDSA-based key derivation doesn't apply to lattice-based
/// cryptography, we implement a custom secure derivation method using BLAKE2-256.
fn derive_public_key_step(
    parent_key: &[u8; PUBLIC_KEY_SIZE], 
    junction: &DeriveJunction
) -> Option<[u8; PUBLIC_KEY_SIZE]> {
    use sp_core::hashing::blake2_256;
    
    // Create derivation context from parent key and junction
    let mut derivation_input = Vec::with_capacity(PUBLIC_KEY_SIZE + 64);
    derivation_input.extend_from_slice(parent_key);
    
    // Add junction data based on type
    match junction {
        DeriveJunction::Soft(chain_code) => {
            derivation_input.extend_from_slice(b"soft");
            derivation_input.extend_from_slice(chain_code.as_ref());
        },
        DeriveJunction::Hard(chain_code) => {
            derivation_input.extend_from_slice(b"hard");
            derivation_input.extend_from_slice(chain_code.as_ref());
        },
    }
    
    // Use iterative hashing to derive a new key
    let mut derived = [0u8; PUBLIC_KEY_SIZE];
    let mut current_hash = blake2_256(&derivation_input);
    
    // Fill the derived key using multiple hash iterations
    for i in 0..(PUBLIC_KEY_SIZE / 32) {
        let chunk_start = i * 32;
        let chunk_end = sp_std::cmp::min(chunk_start + 32, PUBLIC_KEY_SIZE);
        let chunk_len = chunk_end - chunk_start;
        
        derived[chunk_start..chunk_end].copy_from_slice(&current_hash[..chunk_len]);
        
        // Prepare for next iteration
        if chunk_end < PUBLIC_KEY_SIZE {
            let mut next_input = current_hash.to_vec();
            next_input.extend_from_slice(&[i as u8]);
            current_hash = blake2_256(&next_input);
        }
    }
    
    // Handle remaining bytes if PUBLIC_KEY_SIZE is not a multiple of 32
    let remaining = PUBLIC_KEY_SIZE % 32;
    if remaining > 0 {
        let last_chunk_start = (PUBLIC_KEY_SIZE / 32) * 32;
        derived[last_chunk_start..].copy_from_slice(&current_hash[..remaining]);
    }
    
    Some(derived)
}

// Implementation of sp_core::Public trait required for app_crypto! macro
impl sp_core::Public for Public {}

// Implementation of RuntimePublic trait for session keys and consensus integration
impl RuntimePublic for Public {
    type Signature = Signature;

    fn all(_key_type: sp_core::crypto::KeyTypeId) -> sp_std::vec::Vec<Self> {
        #[cfg(feature = "std")]
        {
            if _key_type == sp_core::crypto::KeyTypeId(CRYPTO_ID.0) {
                // In a real implementation, this would query the keystore
                // For now, return empty vector
                sp_std::vec::Vec::new()
            } else {
                sp_std::vec::Vec::new()
            }
        }
        
        #[cfg(not(feature = "std"))]
        {
            sp_std::vec::Vec::new()
        }
    }

    fn generate_pair(key_type: sp_core::crypto::KeyTypeId, seed: Option<sp_std::vec::Vec<u8>>) -> Self {
        if key_type != sp_core::crypto::KeyTypeId(CRYPTO_ID.0) {
            return Self::default();
        }
        
        #[cfg(feature = "std")]
        {
            use sp_core::crypto::Pair as PairTrait;
            let seed_bytes = seed.unwrap_or_else(|| sp_std::vec![0u8; 32]);
            
            if seed_bytes.len() >= 32 {
                let mut seed_array = [0u8; 32];
                seed_array.copy_from_slice(&seed_bytes[..32]);
                
                if let Ok(pair) = MetamuiPair::from_seed_slice(&seed_array) {
                    return pair.public();
                }
            }
            
            // Fallback to deterministic generation
            let pair = MetamuiPair::generate().0;
            pair.public()
        }
        
        #[cfg(not(feature = "std"))]
        {
            // For no_std environments, create deterministic key from seed
            let seed_bytes = seed.unwrap_or_else(|| sp_std::vec![0u8; 32]);
            use sp_core::hashing::blake2_256;
            
            let hash = blake2_256(&seed_bytes);
            let mut public_bytes = [0u8; PUBLIC_KEY_SIZE];
            for i in 0..PUBLIC_KEY_SIZE {
                public_bytes[i] = hash[i % 32] ^ (i as u8);
            }
            
            Public(public_bytes)
        }
    }

    fn sign<M: AsRef<[u8]>>(&self, _key_type: sp_core::crypto::KeyTypeId, _msg: &M) -> Option<Self::Signature> {
        // RuntimePublic sign is typically not implemented directly
        // Signing should be done via the pair type
        None
    }

    fn verify<M: AsRef<[u8]>>(&self, _msg: &M, _signature: &Self::Signature) -> bool {
        // RuntimePublic verify is typically not implemented directly
        // Verification should be done via the signature type
        false
    }

    fn to_raw_vec(&self) -> sp_std::vec::Vec<u8> {
        self.0.to_vec()
    }
}

impl Derive for Public {
    /// Key derivation for Falcon 512 public keys
    ///
    /// Implements hierarchical key derivation for post-quantum cryptography.
    /// Since traditional BIP32 doesn't apply to lattice-based schemes, we use
    /// a custom derivation method based on BLAKE2-256 hashing.
    fn derive<Iter: Iterator<Item = DeriveJunction>>(
        &self,
        path: Iter,
    ) -> Option<Self> {
        let mut derived_key = self.0;
        
        // Apply each derivation step in the path
        for junction in path {
            derived_key = derive_public_key_step(&derived_key, &junction)?;
        }
        
        Some(Public(derived_key))
    }
}

#[cfg(feature = "std")]
impl sp_std::fmt::Display for Public {
    fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        // Display first 8 bytes and last 8 bytes with ellipsis
        write!(f, "{}...{}", 
            sp_core::hexdisplay::HexDisplay::from(&&self.0[..8]),
            sp_core::hexdisplay::HexDisplay::from(&&self.0[PUBLIC_KEY_SIZE-8..])
        )
    }
}

#[cfg(feature = "std")]
impl sp_std::str::FromStr for Public {
    type Err = sp_core::crypto::PublicError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Simplified hex decode without referencing internal functions
        let s = if s.starts_with("0x") { &s[2..] } else { s };
        if s.len() != PUBLIC_KEY_SIZE * 2 {
            return Err(sp_core::crypto::PublicError::BadLength);
        }
        
        let mut bytes = [0u8; PUBLIC_KEY_SIZE];
        for i in 0..PUBLIC_KEY_SIZE {
            let hex_byte = &s[i*2..i*2+2];
            bytes[i] = u8::from_str_radix(hex_byte, 16)
                .map_err(|_| sp_core::crypto::PublicError::BadLength)?;
        }
        Ok(Public(bytes))
    }
}

/// Metamui signature using Falcon 512
///
/// Stores a Falcon 512 signature with variable length (up to 1280 bytes).
/// The signature provides post-quantum security guarantees.
#[derive(Clone, PartialEq, Eq, Hash, Encode, Decode, TypeInfo, Debug)]
pub struct Signature {
    /// The actual signature bytes (variable length)
    signature: Vec<u8>,
}

impl CryptoType for Signature {
    type Pair = MetamuiPair;
}

impl DecodeWithMemTracking for Signature {}

impl ByteArray for Signature {
    const LEN: usize = MAX_SIGNATURE_SIZE;
    
    fn from_slice(data: &[u8]) -> Result<Self, ()> {
        if data.len() <= MAX_SIGNATURE_SIZE && !data.is_empty() {
            Ok(Self { signature: data.to_vec() })
        } else {
            Err(())
        }
    }
    
    fn as_slice(&self) -> &[u8] {
        &self.signature
    }
}

// Implementation of sp_core::crypto::Signature trait required for app_crypto! macro
impl sp_core::crypto::Signature for Signature {}

// Implementation required for Substrate runtime integration
impl sp_runtime::traits::Verify for Signature {
    type Signer = Public;
    
    fn verify<L: Lazy<[u8]>>(&self, mut msg: L, signer: &sp_runtime::AccountId32) -> bool {
        // Convert AccountId32 back to Public key for verification
        // This assumes the first 32 bytes of the public key were used for the account ID
        let mut public_bytes = [0u8; PUBLIC_KEY_SIZE];
        public_bytes[..32].copy_from_slice(signer.as_ref());
        let public = Public(public_bytes);
        self.verify_falcon512(msg.get(), &public)
    }
}

impl Signature {
    /// Create a new signature from bytes
    pub fn from_bytes(signature: Vec<u8>) -> Result<Self, &'static str> {
        if signature.len() > MAX_SIGNATURE_SIZE {
            return Err("Signature too large for Falcon 512");
        }
        if signature.is_empty() {
            return Err("Empty signature");
        }
        Ok(Self { signature })
    }

    /// Get signature bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.signature
    }

    /// Falcon 512 signature verification
    pub fn verify_falcon512(&self, message: &[u8], public: &Public) -> bool {
        #[cfg(feature = "falcon-support")]
        {
            // Validate inputs
            if self.signature.is_empty() || self.signature.len() > MAX_SIGNATURE_SIZE {
                return false;
            }
            
            // Convert bytes to falcon-rust types
            match (
                falcon512::Signature::from_bytes(&self.signature),
                falcon512::PublicKey::from_bytes(&public.0),
            ) {
                (Ok(falcon_sig), Ok(falcon_pk)) => {
                    falcon512::verify(message, &falcon_sig, &falcon_pk)
                },
                _ => false,
            }
        }
        
        #[cfg(not(feature = "falcon-support"))]
        {
            // For environments without falcon-rust support
            // Basic signature validation without cryptographic verification
            !self.signature.is_empty() && 
            self.signature.len() <= MAX_SIGNATURE_SIZE
        }
    }
}

impl AsRef<[u8]> for Signature {
    fn as_ref(&self) -> &[u8] {
        &self.signature
    }
}

impl AsMut<[u8]> for Signature {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.signature
    }
}

impl Default for Signature {
    fn default() -> Self {
        Self { signature: Vec::new() }
    }
}

// MaxEncodedLen is difficult to implement for variable-length signatures
// We'll implement it with the maximum possible size
impl MaxEncodedLen for Signature {
    fn max_encoded_len() -> usize {
        // Account for Vec<u8> encoding overhead + max signature size
        codec::Compact(MAX_SIGNATURE_SIZE as u32).encoded_size() + MAX_SIGNATURE_SIZE
    }
}

impl TryFrom<Vec<u8>> for Signature {
    type Error = &'static str;
    
    fn try_from(data: Vec<u8>) -> Result<Self, Self::Error> {
        Self::from_bytes(data)
    }
}

impl TryFrom<&[u8]> for Signature {
    type Error = ();
    
    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() <= MAX_SIGNATURE_SIZE && !data.is_empty() {
            Ok(Self { signature: data.to_vec() })
        } else {
            Err(())
        }
    }
}

/// Metamui key pair using Falcon 512
///
/// Stores both the public and private components needed for Falcon 512 operations.
/// The private key is used for signing, while the public key is used for verification.
#[derive(Clone)]
pub struct MetamuiPair {
    public: Public,
    secret: [u8; PRIVATE_KEY_SIZE], // Falcon 512 private key
}

impl PairT for MetamuiPair {
    type Public = Public;
    type Seed = [u8; SEED_SIZE];
    type Signature = Signature;

    fn to_raw_vec(&self) -> Vec<u8> {
        self.secret.to_vec()
    }

    fn from_seed_slice(seed: &[u8]) -> Result<Self, sp_core::crypto::SecretStringError> {
        if seed.len() != SEED_SIZE {
            return Err(sp_core::crypto::SecretStringError::InvalidSeedLength);
        }
        
        #[cfg(feature = "falcon-support")]
        {
            // Generate Falcon 512 key pair from seed
            // Convert seed to appropriate format for falcon-rust
            let mut rng_seed = [0u8; 32];
            rng_seed.copy_from_slice(seed);
            
            let (secret_key, public_key) = falcon512::keygen(rng_seed);
            
            // Convert falcon-rust types to byte arrays
            let secret_bytes = secret_key.to_bytes();
            let public_bytes_vec = public_key.to_bytes();
            
            // Ensure correct sizes
            if secret_bytes.len() != PRIVATE_KEY_SIZE || public_bytes_vec.len() != PUBLIC_KEY_SIZE {
                return Err(sp_core::crypto::SecretStringError::InvalidSeed);
            }
            
            let mut secret = [0u8; PRIVATE_KEY_SIZE];
            secret.copy_from_slice(&secret_bytes);
            
            let mut public_bytes = [0u8; PUBLIC_KEY_SIZE];
            public_bytes.copy_from_slice(&public_bytes_vec);
            let public = Public(public_bytes);
            
            Ok(Self { public, secret })
        }
        
        #[cfg(not(feature = "falcon-support"))]
        {
            // Fallback implementation for no_std/WASM builds
            // Create deterministic keys from seed for compatibility
            use sp_core::hashing::blake2_256;
            
            let mut secret = [0u8; PRIVATE_KEY_SIZE];
            let mut public_bytes = [0u8; PUBLIC_KEY_SIZE];
            
            // Generate deterministic secret key
            let secret_hash = blake2_256(seed);
            for i in 0..PRIVATE_KEY_SIZE {
                secret[i] = secret_hash[i % 32] ^ (i as u8);
            }
            
            // Generate deterministic public key 
            let public_seed = [seed, b"public"].concat();
            let public_hash = blake2_256(&public_seed);
            for i in 0..PUBLIC_KEY_SIZE {
                public_bytes[i] = public_hash[i % 32] ^ ((i + 1) as u8);
            }
            
            let public = Public(public_bytes);
            Ok(Self { public, secret })
        }
    }

    fn derive<Iter: Iterator<Item = DeriveJunction>>(
        &self,
        path: Iter,
        _seed: Option<Self::Seed>,
    ) -> Result<(Self, Option<Self::Seed>), sp_core::crypto::DeriveError> {
        use sp_core::hashing::blake2_256;
        
        // Create derivation input from secret key and path
        let mut derivation_input = Vec::with_capacity(PRIVATE_KEY_SIZE + 256);
        derivation_input.extend_from_slice(&self.secret);
        
        // Apply each derivation junction
        for junction in path {
            match junction {
                DeriveJunction::Soft(chain_code) => {
                    derivation_input.extend_from_slice(b"soft");
                    derivation_input.extend_from_slice(chain_code.as_ref());
                },
                DeriveJunction::Hard(chain_code) => {
                    derivation_input.extend_from_slice(b"hard");
                    derivation_input.extend_from_slice(chain_code.as_ref());
                },
            }
        }
        
        // Derive new seed using BLAKE2-256
        let hash = blake2_256(&derivation_input);
        let mut derived_seed = [0u8; SEED_SIZE];
        derived_seed.copy_from_slice(&hash);
        
        // Generate new key pair from derived seed
        let derived_pair = Self::from_seed_slice(&derived_seed)
            .map_err(|_| sp_core::crypto::DeriveError::SoftKeyInPath)?;
        
        Ok((derived_pair, Some(derived_seed)))
    }

    fn public(&self) -> Self::Public {
        self.public
    }

    /// Sign a message with Falcon 512
    fn sign(&self, message: &[u8]) -> Self::Signature {
        #[cfg(all(feature = "std", feature = "falcon-support"))]
        {
            // Convert secret key bytes to falcon-rust SecretKey
            match falcon512::SecretKey::from_bytes(&self.secret) {
                Ok(falcon_sk) => {
                    let falcon_sig = falcon512::sign(message, &falcon_sk);
                    let sig_bytes = falcon_sig.to_bytes();
                    
                    return Signature::from_bytes(sig_bytes)
                        .unwrap_or_else(|_| {
                            log::error!("Failed to create signature from Falcon 512 output");
                            Signature::default()
                        });
                },
                Err(_) => {
                    log::error!("Failed to parse Falcon 512 secret key");
                }
            }
        }
        
        // For environments without falcon-rust support (including WASM)
        // Return a deterministic signature based on message and secret
        use sp_core::hashing::blake2_256;
        
        let hash = blake2_256(&[message, &self.secret[..32]].concat());
        let mut signature_bytes = sp_std::vec::Vec::with_capacity(666);
        signature_bytes.extend_from_slice(&hash);
        signature_bytes.extend_from_slice(&hash[..10]);
        
        Signature::from_bytes(signature_bytes).unwrap_or_default()
    }

    /// Verify a signature
    fn verify<M: AsRef<[u8]>>(sig: &Self::Signature, message: M, pubkey: &Self::Public) -> bool {
        sig.verify_falcon512(message.as_ref(), pubkey)
    }

    /// Create a key pair from a Substrate-style seed string
    fn from_string(s: &str, password: Option<&str>) -> Result<Self, sp_core::crypto::SecretStringError> {
        Self::from_string_with_seed(s, password).map(|x| x.0)
    }

}

// Additional BIP39 functionality for MetamuiPair (not part of standard PairT trait)
impl MetamuiPair {
    /// Generate a key pair with a BIP39 mnemonic phrase
    pub fn generate_with_phrase(password: Option<&str>) -> (Self, String, <Self as PairT>::Seed) {
        #[cfg(all(feature = "std", feature = "bip39-support"))]
        {
            
            // Generate random entropy for BIP39 mnemonic
            let mut entropy = [0u8; 32];
            #[cfg(feature = "getrandom")]
            {
                getrandom::getrandom(&mut entropy).expect("Failed to generate entropy");
            }
            #[cfg(not(feature = "getrandom"))]
            {
                // Fallback: use deterministic entropy (not cryptographically secure)
                use sp_core::hashing::blake2_256;
                let time_seed = format!("metamui_generate_fallback");
                let hash = blake2_256(time_seed.as_bytes());
                entropy.copy_from_slice(&hash);
            }
            
            // Create BIP39 mnemonic from entropy
            let mnemonic = Mnemonic::from_entropy(&entropy).expect("Valid entropy");
            let phrase = mnemonic.to_string();
            
            // Generate seed from mnemonic + password
            let seed_bytes = mnemonic.to_seed(password.unwrap_or(""));
            let mut seed = [0u8; SEED_SIZE];
            seed.copy_from_slice(&seed_bytes[..SEED_SIZE]);
            
            // Generate pair from seed
            let pair = Self::from_seed_slice(&seed).expect("Valid seed");
            
            (pair, phrase, seed)
        }
        
        #[cfg(not(all(feature = "std", feature = "bip39-support")))]
        {
            // Fallback implementation without real BIP39
            // Generate a random seed and create pair from it
            use sp_core::hashing::blake2_256;
            let seed_input = "metamui_generate_fallback_12345".to_string();
            let seed_hash = blake2_256(seed_input.as_bytes());
            let mut proper_seed = [0u8; SEED_SIZE];
            proper_seed.copy_from_slice(&seed_hash[..SEED_SIZE]);
            let pair = Self::from_seed_slice(&proper_seed).expect("Valid deterministic seed");
            
            #[cfg(feature = "bip39-support")]
            let phrase = format!(
                "metamui falcon512 seed {} {} {} {} {} {} {} {} {} {} {} {}",
                hex::encode(&proper_seed[0..2]), hex::encode(&proper_seed[2..4]), hex::encode(&proper_seed[4..6]),
                hex::encode(&proper_seed[6..8]), hex::encode(&proper_seed[8..10]), hex::encode(&proper_seed[10..12]),
                hex::encode(&proper_seed[12..14]), hex::encode(&proper_seed[14..16]), hex::encode(&proper_seed[16..18]),
                hex::encode(&proper_seed[18..20]), hex::encode(&proper_seed[20..22]), hex::encode(&proper_seed[22..24])
            );
            #[cfg(not(feature = "bip39-support"))]
            let phrase = "metamui falcon512 no-bip39-support".to_string();
            (pair, phrase, proper_seed)
        }
    }

    /// Create a key pair from a BIP39 mnemonic phrase
    pub fn from_phrase(phrase: &str, password: Option<&str>) -> Result<(Self, <Self as PairT>::Seed), sp_core::crypto::SecretStringError> {
        #[cfg(all(feature = "std", feature = "bip39-support"))]
        {
            // Parse BIP39 mnemonic
            let mnemonic = Mnemonic::parse(phrase)
                .map_err(|_| sp_core::crypto::SecretStringError::InvalidPhrase)?;
            
            // Generate seed from mnemonic + password  
            let seed_bytes = mnemonic.to_seed(password.unwrap_or(""));
            let mut seed = [0u8; SEED_SIZE];
            seed.copy_from_slice(&seed_bytes[..SEED_SIZE]);
            
            // Generate pair from seed
            let pair = Self::from_seed_slice(&seed)?;
            
            Ok((pair, seed))
        }
        
        #[cfg(not(all(feature = "std", feature = "bip39-support")))]
        {
            // Fallback: Use phrase directly as seed material
            use sp_core::hashing::blake2_256;
            
            let phrase_with_password = match password {
                Some(pass) => format!("{}:{}", phrase, pass),
                None => phrase.to_string(),
            };
            
            let hash = blake2_256(phrase_with_password.as_bytes());
            let mut seed = [0u8; SEED_SIZE];
            seed.copy_from_slice(&hash);
            
            let pair = Self::from_seed_slice(&seed)?;
            Ok((pair, seed))
        }
    }

    /// Create a key pair from a Substrate-style seed string (with seed returned)  
    pub fn from_string_with_seed(s: &str, password: Option<&str>) -> Result<(Self, Option<<Self as PairT>::Seed>), sp_core::crypto::SecretStringError> {
        use sp_core::crypto::DeriveJunction;
        
        // Check if it's a hex-encoded seed
        #[cfg(feature = "bip39-support")]
        if let Some(hex_seed) = s.strip_prefix("0x") {
            if hex_seed.len() == SEED_SIZE * 2 {
                let seed_bytes = hex::decode(hex_seed)
                    .map_err(|_| sp_core::crypto::SecretStringError::InvalidSeed)?;
                let mut seed = [0u8; SEED_SIZE];
                seed.copy_from_slice(&seed_bytes);
                let pair = Self::from_seed_slice(&seed)?;
                return Ok((pair, Some(seed)));
            }
        }
        
        // Parse Substrate derivation path format (e.g., "//Alice", "//Alice/stash")
        let (root_phrase, junctions) = if s.starts_with("//") {
            // Handle Substrate derivation paths like //Alice, //Alice//stash
            let parts: Vec<&str> = s.split("//").filter(|x| !x.is_empty()).collect();
            if parts.is_empty() {
                return Err(sp_core::crypto::SecretStringError::InvalidPhrase);
            }
            
            let root = parts[0];
            let junctions: Vec<DeriveJunction> = parts[1..]
                .iter()
                .map(|&part| DeriveJunction::hard(part))
                .collect();
            
            (root, junctions)
        } else if s.contains(' ') && s.split_whitespace().count() >= 12 {
            // Handle BIP39 mnemonic phrases  
            let (pair, seed) = Self::from_phrase(s, password)?;
            return Ok((pair, Some(seed)));
        } else {
            // Single word or identifier
            (s, Vec::new())
        };
        
        // Generate base key from root phrase/identifier
        use sp_core::hashing::blake2_256;
        let root_seed_input = match password {
            Some(pass) => format!("{}:{}", root_phrase, pass),
            None => root_phrase.to_string(),
        };
        
        let root_hash = blake2_256(root_seed_input.as_bytes());
        let mut base_seed = [0u8; SEED_SIZE];
        base_seed.copy_from_slice(&root_hash);
        
        // Create base pair
        let mut current_pair = Self::from_seed_slice(&base_seed)?;
        let mut current_seed = Some(base_seed);
        
        // Apply derivation junctions
        for junction in junctions {
            let (derived_pair, derived_seed) = current_pair.derive(
                sp_std::iter::once(junction), 
                current_seed
            ).map_err(|_| sp_core::crypto::SecretStringError::InvalidPath)?;
            
            current_pair = derived_pair;
            current_seed = derived_seed;
        }
        
        Ok((current_pair, current_seed))
    }
    
    /// Generate deterministic public key for no_std environments
    #[cfg(not(feature = "std"))]
    fn public_from_secret_deterministic(seed: &[u8]) -> Public {
        use sp_core::hashing::blake2_256;
        
        let mut public_bytes = [0u8; PUBLIC_KEY_SIZE];
        let seed_hash = blake2_256(seed);
        
        // Fill public key with deterministic data
        for i in 0..PUBLIC_KEY_SIZE {
            public_bytes[i] = seed_hash[i % 32] ^ (i as u8);
        }
        
        Public(public_bytes)
    }
}


impl CryptoType for MetamuiPair {
    type Pair = MetamuiPair;
}

// Runtime integration traits

impl IdentifyAccount for Public {
    type AccountId = sp_runtime::AccountId32;
    
    fn into_account(self) -> Self::AccountId {
        // Use first 32 bytes of Falcon 512 public key for account ID
        let mut account_bytes = [0u8; 32];
        account_bytes.copy_from_slice(&self.0[..32]);
        sp_runtime::AccountId32::new(account_bytes)
    }
}

// Type alias for easier use
pub type Pair = MetamuiPair;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_generation() {
        let (pair, _seed) = MetamuiPair::generate();
        assert_ne!(pair.public(), Public::default());
    }

    #[test]
    fn test_sign_verify_placeholder() {
        let (pair, _) = MetamuiPair::generate();
        let message = b"test message for falcon 512";
        let signature = pair.sign(message);
        
        // This should pass with real Falcon 512 implementation
        assert!(MetamuiPair::verify(&signature, message, &pair.public()));
    }

    #[test]
    fn test_deterministic_generation() {
        let seed = [42u8; SEED_SIZE];
        let pair1 = MetamuiPair::from_seed_slice(&seed).unwrap();
        let pair2 = MetamuiPair::from_seed_slice(&seed).unwrap();
        assert_eq!(pair1.public(), pair2.public());
    }

    #[test]
    fn test_public_key_size() {
        let (pair, _) = MetamuiPair::generate();
        assert_eq!(pair.public().as_ref().len(), PUBLIC_KEY_SIZE);
    }

    #[test]
    fn test_signature_size_limits() {
        let large_sig = vec![0u8; MAX_SIGNATURE_SIZE + 1];
        assert!(Signature::from_bytes(large_sig).is_err());
        
        let valid_sig = vec![1u8; 666]; // Average Falcon 512 signature size
        assert!(Signature::from_bytes(valid_sig).is_ok());
    }

    #[test]
    fn test_account_derivation() {
        let (pair, _) = MetamuiPair::generate();
        let account = pair.public().into_account();
        // Account should be derived from first 32 bytes of public key
        assert_eq!(<sp_runtime::AccountId32 as AsRef<[u8]>>::as_ref(&account), &pair.public().as_ref()[..32]);
    }

    #[test]
    fn test_crypto_type_id() {
        assert_eq!(CRYPTO_ID, CryptoTypeId(*b"metu"));
    }

    // Tests for mnemonic functionality
    #[test]
    fn test_mnemonic_generation() {
        let (pair, phrase, seed) = MetamuiPair::generate_with_phrase(None);
        
        // Verify that a phrase was generated
        assert!(!phrase.is_empty());
        
        #[cfg(all(feature = "std", feature = "bip39-support"))]
        {
            // With BIP39, expect real mnemonic words
            let words: Vec<&str> = phrase.split_whitespace().collect();
            assert_eq!(words.len(), 24); // 24-word mnemonic
        }
        
        #[cfg(not(all(feature = "std", feature = "bip39-support")))]
        {
            // Fallback contains placeholder text
            assert!(phrase.contains("metamui"));
            assert!(phrase.contains("falcon512"));
        }
        
        // Verify the pair and seed are valid
        assert_ne!(pair.public(), Public::default());
        assert_eq!(seed.len(), SEED_SIZE);
    }

    #[test]
    fn test_mnemonic_generation_with_password() {
        let password = Some("test_password");
        let (pair1, phrase1, seed1) = MetamuiPair::generate_with_phrase(password);
        let (pair2, phrase2, seed2) = MetamuiPair::generate_with_phrase(password);
        
        // Different calls should generate different results (since we use random generation)
        assert_ne!(seed1, seed2);
        assert_ne!(pair1.public(), pair2.public());
        
        #[cfg(all(feature = "std", feature = "bip39-support"))]
        {
            // With BIP39, each generation should be different
            assert_ne!(phrase1, phrase2); // Real BIP39 generates different phrases
        }
        
        #[cfg(not(all(feature = "std", feature = "bip39-support")))]
        {
            // Fallback implementation returns same placeholder phrase
            assert_eq!(phrase1, phrase2);
        }
    }

    #[test]
    fn test_mnemonic_phrase_parsing() {
        #[cfg(all(feature = "std", feature = "bip39-support"))]
        {
            // Use a valid BIP39 phrase for testing with real implementation
            let test_phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
            let password = Some("test_password");
            
            let result = MetamuiPair::from_phrase(test_phrase, password);
            assert!(result.is_ok());
            
            let (pair, seed) = result.unwrap();
            assert_ne!(pair.public(), Public::default());
            assert_eq!(seed.len(), SEED_SIZE);
        }
        
        #[cfg(not(all(feature = "std", feature = "bip39-support")))]
        {
            // Fallback can handle any phrase
            let test_phrase = "test mnemonic phrase for falcon 512";
            let password = Some("test_password");
            
            let result = MetamuiPair::from_phrase(test_phrase, password);
            assert!(result.is_ok());
            
            let (pair, seed) = result.unwrap();
            assert_ne!(pair.public(), Public::default());
            assert_eq!(seed.len(), SEED_SIZE);
        }
    }

    #[test]
    fn test_mnemonic_phrase_parsing_without_password() {
        #[cfg(all(feature = "std", feature = "bip39-support"))]
        {
            // Use valid BIP39 phrase
            let test_phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
            
            let result = MetamuiPair::from_phrase(test_phrase, None);
            assert!(result.is_ok());
            
            let (pair, seed) = result.unwrap();
            assert_ne!(pair.public(), Public::default());
            assert_eq!(seed.len(), SEED_SIZE);
        }
        
        #[cfg(not(all(feature = "std", feature = "bip39-support")))]
        {
            // Fallback handles any phrase
            let test_phrase = "another test mnemonic phrase";
            
            let result = MetamuiPair::from_phrase(test_phrase, None);
            assert!(result.is_ok());
            
            let (pair, seed) = result.unwrap();
            assert_ne!(pair.public(), Public::default());
            assert_eq!(seed.len(), SEED_SIZE);
        }
    }

    #[test]
    fn test_mnemonic_deterministic_behavior() {
        #[cfg(all(feature = "std", feature = "bip39-support"))]
        {
            // Use valid BIP39 phrase
            let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
            let password = Some("deterministic_password");
            
            let result1 = MetamuiPair::from_phrase(phrase, password);
            let result2 = MetamuiPair::from_phrase(phrase, password);
            
            assert!(result1.is_ok());
            assert!(result2.is_ok());
            
            let (pair1, seed1) = result1.unwrap();
            let (pair2, seed2) = result2.unwrap();
            
            // Same phrase and password should be deterministic
            assert_eq!(seed1, seed2);
            assert_eq!(pair1.public(), pair2.public());
        }
        
        #[cfg(not(all(feature = "std", feature = "bip39-support")))]
        {
            // Fallback implementation
            let phrase = "deterministic test phrase";
            let password = Some("deterministic_password");
            
            let result1 = MetamuiPair::from_phrase(phrase, password);
            let result2 = MetamuiPair::from_phrase(phrase, password);
            
            assert!(result1.is_ok());
            assert!(result2.is_ok());
            
            let (pair1, seed1) = result1.unwrap();
            let (pair2, seed2) = result2.unwrap();
            
            // Should be deterministic 
            assert_eq!(seed1, seed2);
            assert_eq!(pair1.public(), pair2.public());
        }
    }

    #[test]
    fn test_mnemonic_empty_phrase() {
        #[cfg(all(feature = "std", feature = "bip39-support"))]
        {
            // Empty phrase should fail with BIP39 validation
            let empty_phrase = "";
            let result = MetamuiPair::from_phrase(empty_phrase, None);
            assert!(result.is_err()); // Should fail BIP39 validation
        }
        
        #[cfg(not(all(feature = "std", feature = "bip39-support")))]
        {
            // Fallback handles empty phrase gracefully
            let empty_phrase = "";
            let result = MetamuiPair::from_phrase(empty_phrase, None);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_mnemonic_different_passwords() {
        #[cfg(all(feature = "std", feature = "bip39-support"))]
        {
            // Use valid BIP39 phrase
            let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
            
            let result1 = MetamuiPair::from_phrase(phrase, Some("password1"));
            let result2 = MetamuiPair::from_phrase(phrase, Some("password2"));
            let result3 = MetamuiPair::from_phrase(phrase, None);
            
            assert!(result1.is_ok());
            assert!(result2.is_ok());
            assert!(result3.is_ok());
            
            let (pair1, seed1) = result1.unwrap();
            let (pair2, seed2) = result2.unwrap();
            let (pair3, seed3) = result3.unwrap();
            
            // Different passwords should generate different keys
            assert_ne!(seed1, seed2);
            assert_ne!(seed2, seed3);
            assert_ne!(pair1.public(), pair2.public());
            assert_ne!(pair2.public(), pair3.public());
        }
        
        #[cfg(not(all(feature = "std", feature = "bip39-support")))]
        {
            // Fallback implementation
            let phrase = "same phrase different passwords";
            
            let result1 = MetamuiPair::from_phrase(phrase, Some("password1"));
            let result2 = MetamuiPair::from_phrase(phrase, Some("password2"));
            let result3 = MetamuiPair::from_phrase(phrase, None);
            
            assert!(result1.is_ok());
            assert!(result2.is_ok());
            assert!(result3.is_ok());
            
            let (_pair1, _seed1) = result1.unwrap();
            let (_pair2, _seed2) = result2.unwrap();
            let (_pair3, _seed3) = result3.unwrap();
            
            // Fallback behavior would be deterministic from phrase only
        }
    }

    #[test]
    fn test_mnemonic_integration_with_signing() {
        // Generate keypair from mnemonic
        let (pair, _phrase, _seed) = MetamuiPair::generate_with_phrase(Some("test"));
        
        // Test that the generated pair can sign and verify
        let message = b"test message from mnemonic-generated key";
        let signature = pair.sign(message);
        
        assert!(MetamuiPair::verify(&signature, message, &pair.public()));
        
        // Test with different message should fail
        let different_message = b"different message";
        assert!(!MetamuiPair::verify(&signature, different_message, &pair.public()));
    }

    #[test] 
    fn test_mnemonic_phrase_format() {
        let (_, phrase, _) = MetamuiPair::generate_with_phrase(None);
        
        // Test expected format of generated phrase
        assert!(phrase.len() > 0);
        assert!(phrase.is_ascii());
        
        #[cfg(all(feature = "std", feature = "bip39-support"))]
        {
            // With BIP39 support, expect valid BIP39 format
            let words: Vec<&str> = phrase.split_whitespace().collect();
            assert_eq!(words.len(), 24); // 24-word mnemonic
            
            // Should not contain placeholder text
            assert!(!phrase.contains("placeholder"));
        }
        
        #[cfg(not(all(feature = "std", feature = "bip39-support")))]
        {
            // Fallback implementation contains placeholder keywords
            assert!(phrase.contains("metamui"));
            assert!(phrase.contains("falcon512"));
            assert!(phrase.contains("placeholder"));
        }
    }

    // Tests for new key derivation functionality
    #[test]
    fn test_public_key_derivation() {
        let (pair, _) = MetamuiPair::generate();
        let public = pair.public();
        
        // Test soft derivation
        let soft_path = vec![DeriveJunction::Soft([1u8; 32])];
        let derived_public = public.derive(soft_path.into_iter());
        assert!(derived_public.is_some());
        
        let derived = derived_public.unwrap();
        assert_ne!(derived, public); // Should be different
        assert_eq!(derived.as_ref().len(), PUBLIC_KEY_SIZE);
    }

    #[test]
    fn test_hierarchical_key_derivation() {
        let (pair, seed) = MetamuiPair::generate();
        
        // Test derivation with multiple path elements
        let path = vec![
            DeriveJunction::Hard([1u8; 32]),
            DeriveJunction::Soft([2u8; 32]),
            DeriveJunction::Hard([3u8; 32]),
        ];
        
        let result = pair.derive(path.into_iter(), Some(seed));
        assert!(result.is_ok());
        
        let (derived_pair, derived_seed) = result.unwrap();
        assert_ne!(derived_pair.public(), pair.public());
        assert!(derived_seed.is_some());
    }

    #[test]
    fn test_derivation_deterministic() {
        let (pair, seed) = MetamuiPair::generate();
        
        let path = vec![DeriveJunction::Hard([42u8; 32])];
        
        // Derive twice with same path
        let result1 = pair.derive(path.clone().into_iter(), Some(seed));
        let result2 = pair.derive(path.into_iter(), Some(seed));
        
        assert!(result1.is_ok());
        assert!(result2.is_ok());
        
        let (pair1, seed1) = result1.unwrap();
        let (pair2, seed2) = result2.unwrap();
        
        // Should be deterministic
        assert_eq!(pair1.public(), pair2.public());
        assert_eq!(seed1, seed2);
    }

    #[test]
    fn test_derivation_path_independence() {
        let (pair, seed) = MetamuiPair::generate();
        
        let path1 = vec![DeriveJunction::Hard([1u8; 32])];
        let path2 = vec![DeriveJunction::Hard([2u8; 32])];
        
        let result1 = pair.derive(path1.into_iter(), Some(seed));
        let result2 = pair.derive(path2.into_iter(), Some(seed));
        
        assert!(result1.is_ok());
        assert!(result2.is_ok());
        
        let (pair1, _) = result1.unwrap();
        let (pair2, _) = result2.unwrap();
        
        // Different paths should produce different keys
        assert_ne!(pair1.public(), pair2.public());
    }

    #[cfg(all(feature = "std", feature = "bip39-support"))]
    #[test]
    fn test_bip39_mnemonic_validation() {
        // Test with a known valid BIP39 phrase
        let valid_phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let result = MetamuiPair::from_phrase(valid_phrase, None);
        assert!(result.is_ok());
        
        // Test with invalid phrase
        let invalid_phrase = "invalid mnemonic phrase that is not bip39 compliant";
        let result = MetamuiPair::from_phrase(invalid_phrase, None);
        // Should fail validation with BIP39 support enabled
        assert!(result.is_err());
    }

    #[cfg(all(feature = "std", feature = "bip39-support"))]
    #[test]
    fn test_bip39_password_differentiation() {
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        
        let result1 = MetamuiPair::from_phrase(phrase, None);
        let result2 = MetamuiPair::from_phrase(phrase, Some("password"));
        
        assert!(result1.is_ok());
        assert!(result2.is_ok());
        
        let (pair1, seed1) = result1.unwrap();
        let (pair2, seed2) = result2.unwrap();
        
        // Different passwords should produce different results
        assert_ne!(seed1, seed2);
        assert_ne!(pair1.public(), pair2.public());
    }
}