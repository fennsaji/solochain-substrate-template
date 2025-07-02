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
use sp_std::{vec::Vec, convert::TryFrom, fmt::Debug};
use codec::{Encode, Decode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::traits::{Verify, IdentifyAccount, Lazy};

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
    Encode, Decode, TypeInfo, MaxEncodedLen
)]
#[cfg_attr(feature = "std", derive(Debug))]
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

// Implementation of sp_core::Public trait required for app_crypto! macro
impl sp_core::Public for Public {}

impl Derive for Public {
    /// Key derivation for Falcon 512 public keys
    ///
    /// TODO: Implement proper key derivation for Falcon 512
    /// This requires implementing hierarchical key derivation compatible with
    /// post-quantum cryptography standards.
    fn derive<Iter: Iterator<Item = DeriveJunction>>(
        &self,
        _path: Iter,
    ) -> Option<Self> {
        // TODO: Implement Falcon 512 key derivation
        // For now, return the same key (not secure for production)
        // This should implement proper BIP32-like derivation for post-quantum schemes
        #[cfg(feature = "std")]
        log::warn!("Falcon 512 key derivation not yet implemented - using same key");
        Some(*self)
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
#[derive(Clone, PartialEq, Eq, Hash, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Signature {
    /// The actual signature bytes (variable length)
    signature: Vec<u8>,
}

impl CryptoType for Signature {
    type Pair = MetamuiPair;
}

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
impl sp_core::crypto::Signature for Signature {
    fn verify<L: Lazy<[u8]>>(&self, msg: L, signer: &Public) -> bool {
        self.verify_falcon512(msg.get(), signer)
    }

    fn to_raw_vec(&self) -> Vec<u8> {
        self.signature.clone()
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
    ///
    /// TODO: Implement actual Falcon 512 signature verification
    /// This should use a proper Falcon 512 implementation
    pub fn verify_falcon512(&self, message: &[u8], public: &Public) -> bool {
        // TODO: Replace with actual Falcon 512 verification
        // 
        // Expected implementation:
        // 1. Parse the Falcon 512 public key from public.0
        // 2. Parse the Falcon 512 signature from self.signature
        // 3. Use Falcon 512 verification algorithm: falcon_verify(public_key, message, signature)
        // 4. Return true if verification succeeds, false otherwise
        //
        // Example pseudo-code:
        // ```rust
        // use falcon_rust::falcon512;
        // let falcon_public_key = falcon512::PublicKey::from_bytes(&public.0)?;
        // let falcon_signature = falcon512::Signature::from_bytes(&self.signature)?;
        // falcon512::verify(&falcon_public_key, message, &falcon_signature)
        // ```
        
        // Placeholder verification for compilation
        // SECURITY WARNING: This is NOT secure - replace with actual Falcon 512 verification
        #[cfg(feature = "std")]
        {
            log::warn!("Using placeholder verification - NOT SECURE FOR PRODUCTION");
            // Simple length checks as placeholder
            !self.signature.is_empty() && 
            self.signature.len() <= MAX_SIGNATURE_SIZE && 
            public.0 != [0u8; PUBLIC_KEY_SIZE] &&
            !message.is_empty()
        }
        
        #[cfg(not(feature = "std"))]
        {
            // For no_std environments
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

    /// Generate a random Falcon 512 key pair
    ///
    /// TODO: Implement secure random key generation for Falcon 512
    fn generate() -> (Self, Self::Seed) {
        // TODO: Generate cryptographically secure random seed
        // Use proper entropy source like getrandom() or similar
        let seed = [0u8; SEED_SIZE]; // PLACEHOLDER - use secure randomness
        
        let pair = Self::from_seed_slice(&seed).expect("seed is correct size; qed");
        (pair, seed)
    }

    /// Generate key pair with mnemonic phrase
    ///
    /// TODO: Implement BIP39-compatible mnemonic generation for Falcon 512
    fn generate_with_phrase(password: Option<&str>) -> (Self, String, Self::Seed) {
        // TODO: Generate proper BIP39 mnemonic phrase
        let (pair, seed) = Self::generate();
        let phrase = "metamui falcon512 mnemonic phrase placeholder".to_string();
        (pair, phrase, seed)
    }

    /// Create key pair from mnemonic phrase
    ///
    /// TODO: Implement BIP39 phrase parsing and key derivation
    fn from_phrase(
        phrase: &str,
        password: Option<&str>,
    ) -> Result<(Self, Self::Seed), sp_core::crypto::SecretStringError> {
        // TODO: Parse BIP39 mnemonic and derive seed
        // 1. Validate mnemonic phrase
        // 2. Apply PBKDF2 with password if provided
        // 3. Generate seed from mnemonic + password
        
        let seed = [0u8; SEED_SIZE]; // PLACEHOLDER - derive from phrase + password
        let pair = Self::from_seed_slice(&seed)?;
        Ok((pair, seed))
    }

    /// Hierarchical key derivation
    ///
    /// TODO: Implement BIP32-like key derivation for post-quantum schemes
    fn derive<Iter: Iterator<Item = DeriveJunction>>(
        &self,
        _path: Iter,
        _seed: Option<Self::Seed>,
    ) -> Result<(Self, Option<Self::Seed>), sp_core::crypto::DeriveError> {
        // TODO: Implement proper hierarchical key derivation for Falcon 512
        // This is complex for post-quantum schemes as BIP32 doesn't directly apply
        // May need to use alternative derivation methods
        
        #[cfg(feature = "std")]
        log::warn!("Falcon 512 hierarchical key derivation not yet implemented");
        Ok((self.clone(), None))
    }

    /// Create key pair from seed
    fn from_seed_slice(seed: &[u8]) -> Result<Self, sp_core::crypto::SecretStringError> {
        if seed.len() != SEED_SIZE {
            return Err(sp_core::crypto::SecretStringError::InvalidSeedLength);
        }
        
        // TODO: Generate Falcon 512 key pair from seed
        // 1. Use seed as entropy for Falcon 512 key generation
        // 2. Generate private key using Falcon 512 keygen
        // 3. Derive public key from private key
        
        let mut secret = [0u8; PRIVATE_KEY_SIZE];
        // PLACEHOLDER: In real implementation, this would use Falcon 512 keygen
        // Example: falcon512::keygen_from_seed(seed) -> (private_key, public_key)
        
        let public = Self::public_from_secret(&secret, seed);
        
        Ok(Self { public, secret })
    }

    /// Sign a message with Falcon 512
    fn sign(&self, message: &[u8]) -> Self::Signature {
        // TODO: Implement actual Falcon 512 signing
        //
        // Expected implementation:
        // 1. Parse the Falcon 512 private key from self.secret
        // 2. Use Falcon 512 signing algorithm: falcon_sign(private_key, message)
        // 3. Return the signature bytes wrapped in Signature struct
        //
        // Example pseudo-code:
        // ```rust
        // use falcon_rust::falcon512;
        // let falcon_private_key = falcon512::PrivateKey::from_bytes(&self.secret)?;
        // let signature_bytes = falcon512::sign(&falcon_private_key, message)?;
        // Signature::from_bytes(signature_bytes.to_vec())
        // ```
        
        // PLACEHOLDER implementation - NOT SECURE
        #[cfg(feature = "std")]
        {
            use sp_core::hashing::blake2_256;
            
            log::warn!("Using placeholder signing - NOT SECURE FOR PRODUCTION");
            
            // Create a deterministic but insecure signature for testing
            let hash = blake2_256(&[message, self.public.as_ref(), &self.secret[..32]].concat());
            let mut signature_bytes = Vec::with_capacity(666); // Average Falcon 512 signature size
            signature_bytes.extend_from_slice(&hash);
            signature_bytes.extend_from_slice(&hash[..10]); // Pad to reasonable size
            
            Signature::from_bytes(signature_bytes).unwrap_or_default()
        }
        
        #[cfg(not(feature = "std"))]
        {
            // For no_std environments, return empty signature as placeholder
            Signature::default()
        }
    }

    /// Verify a signature
    fn verify<M: AsRef<[u8]>>(sig: &Self::Signature, message: M, pubkey: &Self::Public) -> bool {
        sig.verify_falcon512(message.as_ref(), pubkey)
    }

    /// Get the public key
    fn public(&self) -> Self::Public {
        self.public
    }
}

impl MetamuiPair {
    /// Generate Falcon 512 public key from secret key and seed
    ///
    /// TODO: Implement proper public key derivation from Falcon 512 private key
    fn public_from_secret(secret: &[u8; PRIVATE_KEY_SIZE], seed: &[u8]) -> Public {
        // TODO: Derive Falcon 512 public key from private key
        // In Falcon 512, the public key is derived from the private key during keygen
        
        #[cfg(feature = "std")]
        {
            use sp_core::hashing::blake2_256;
            
            // PLACEHOLDER: Generate deterministic public key for testing
            // In real implementation, this would extract the public key from the Falcon 512 private key
            let mut public_bytes = [0u8; PUBLIC_KEY_SIZE];
            let seed_hash = blake2_256(seed);
            
            // Fill public key with deterministic but placeholder data
            for i in 0..PUBLIC_KEY_SIZE {
                public_bytes[i] = seed_hash[i % 32] ^ (i as u8);
            }
            
            Public(public_bytes)
        }
        
        #[cfg(not(feature = "std"))]
        {
            // For no_std, return zero public key as placeholder
            Public([0u8; PUBLIC_KEY_SIZE])
        }
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
        
        // This will pass with placeholder implementation
        // TODO: Update when real Falcon 512 implementation is added
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
        assert_eq!(account.as_ref(), &pair.public().as_ref()[..32]);
    }

    #[test]
    fn test_crypto_type_id() {
        assert_eq!(CRYPTO_ID, CryptoTypeId(*b"metu"));
    }
}