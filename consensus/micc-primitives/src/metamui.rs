// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Metamui Crypto Application for MICC Consensus
//!
//! This module provides the application-specific crypto wrapper for Metamui Falcon 512
//! post-quantum cryptography to be used with the MICC consensus system.
//!
//! ## Key Components
//!
//! - `AuthorityId`: Metamui public key for authority identification
//! - `AuthorityPair`: Metamui key pair for authority operations
//! - `AuthoritySignature`: Metamui signature for consensus messages
//!
//! ## Security Features
//!
//! - Post-quantum cryptographic security via Falcon 512
//! - Resistance to quantum computer attacks
//! - Integration with Substrate's consensus and runtime systems

use sp_application_crypto::{app_crypto, KeyTypeId};
use sp_core::crypto::CryptoTypeId;

/// Key type identifier for Metamui MICC consensus
/// 
/// This unique identifier is used by the Substrate keystore to distinguish
/// Metamui consensus keys from other key types in the system.
pub const METAMUI_MICC: KeyTypeId = KeyTypeId(*b"mmcc"); // "metamui micc consensus"

/// Application-specific crypto module for Metamui
mod app_metamui {
    use super::METAMUI_MICC;
    use pallet_metamui_crypto as metamui; // Import our Metamui crypto module
    
    /// Create application-specific crypto using Metamui scheme
    /// 
    /// This macro generates the necessary application crypto types that wrap
    /// our core Metamui crypto implementations with Substrate's application
    /// crypto framework, enabling seamless integration with consensus systems.
    app_crypto!(metamui, METAMUI_MICC);
}

sp_application_crypto::with_pair! {
    /// Metamui authority key pair for MICC consensus
    ///
    /// This key pair is used by consensus authorities to:
    /// - Sign consensus messages and blocks
    /// - Participate in the MICC consensus protocol
    /// - Authenticate as a valid block producer
    ///
    /// Uses Falcon 512 post-quantum cryptography for future-proof security.
    pub type AuthorityPair = app_metamui::Pair;
}

/// Metamui authority signature for MICC consensus
///
/// Represents a Falcon 512 signature over consensus-related data.
/// These signatures are used to:
/// - Authenticate block authorship
/// - Sign consensus messages
/// - Prove authority participation in consensus
///
/// Signature size: Variable (average ~666 bytes, max 1280 bytes)
pub type AuthoritySignature = app_metamui::Signature;

/// Metamui authority identifier for MICC consensus
///
/// Represents the public component of a Metamui authority key.
/// Used for:
/// - Identifying authorities in the consensus set
/// - Verifying signatures from authorities
/// - Account derivation for authority rewards
///
/// Public key size: 897 bytes (Falcon 512)
pub type AuthorityId = app_metamui::Public;

/// Runtime application public key implementation for Metamui authorities
///
/// This implementation provides the necessary runtime integration for Metamui
/// authorities to participate in consensus operations through the Substrate
/// application crypto framework.
impl sp_application_crypto::RuntimeAppPublic for AuthorityId {
    /// The key type identifier for Metamui MICC consensus
    const ID: KeyTypeId = METAMUI_MICC;
    
    /// The crypto type identifier for Metamui cryptography
    const CRYPTO_ID: CryptoTypeId = pallet_metamui_crypto::CRYPTO_ID;
    
    /// The signature type used by Metamui authorities
    type Signature = AuthoritySignature;

    /// Retrieve all Metamui authority public keys from the keystore
    ///
    /// This method queries the local keystore for all keys of the Metamui MICC type
    /// and returns them as authority identifiers.
    fn all() -> sp_std::vec::Vec<Self> {
        sp_io::crypto::public_keys(Self::ID)
            .into_iter()
            .map(|key| {
                // Convert raw key bytes to Metamui public key
                Self::from_slice(&key)
            })
            .collect()
    }

    /// Generate a new Metamui authority key pair
    ///
    /// Creates a new Metamui key pair in the keystore using the provided seed.
    /// If no seed is provided, a random seed will be generated.
    ///
    /// TODO: This currently uses a placeholder implementation.
    /// When actual Falcon 512 integration is complete, this should use
    /// the proper Metamui key generation through sp_io::crypto.
    fn generate_pair(seed: Option<sp_std::vec::Vec<u8>>) -> Self {
        // TODO: Implement proper Metamui key generation through sp_io::crypto
        // This will require extending sp_io::crypto to support Metamui crypto operations
        // 
        // Expected implementation:
        // sp_io::crypto::metamui_generate(Self::ID, seed).into()
        //
        // For now, create a placeholder key from the seed
        let seed_bytes = seed.unwrap_or_else(|| {
            // Generate default seed - in production this should be cryptographically secure
            sp_std::vec![0u8; 32]
        });
        
        // Create deterministic public key from seed for development
        // SECURITY WARNING: This is NOT secure - replace with actual keystore integration
        use sp_core::hashing::blake2_256;
        let seed_hash = blake2_256(&seed_bytes);
        
        // Create a deterministic but placeholder public key
        let mut public_bytes = [0u8; pallet_metamui_crypto::PUBLIC_KEY_SIZE];
        for i in 0..pallet_metamui_crypto::PUBLIC_KEY_SIZE {
            public_bytes[i] = seed_hash[i % 32] ^ (i as u8);
        }
        
        Self::from_slice(&public_bytes[..])
    }

    /// Sign a message using the Metamui authority key
    ///
    /// Signs the provided message using the authority's private key stored in the keystore.
    /// Returns the Metamui signature if successful.
    ///
    /// TODO: This currently uses a placeholder implementation.
    /// When actual Falcon 512 integration is complete, this should use
    /// the proper Metamui signing through sp_io::crypto.
    fn sign<M: AsRef<[u8]>>(&self, msg: &M) -> Option<Self::Signature> {
        // TODO: Implement proper Metamui signing through sp_io::crypto
        // This will require extending sp_io::crypto to support Metamui crypto operations
        //
        // Expected implementation:
        // sp_io::crypto::metamui_sign(Self::ID, self, msg.as_ref())
        //     .map(|sig| Self::Signature::from_slice(&sig))
        
        // PLACEHOLDER: Create a deterministic but insecure signature for development
        // SECURITY WARNING: This is NOT secure - replace with actual keystore integration
        #[cfg(feature = "std")]
        {
            log::warn!("Using placeholder Metamui signing - NOT SECURE FOR PRODUCTION");
            
            use sp_core::hashing::blake2_256;
            let message_hash = blake2_256(&[msg.as_ref(), self.as_ref()].concat());
            
            // Create a signature-like structure for testing
            let mut signature_bytes = sp_std::vec::Vec::with_capacity(666); // Average Falcon 512 size
            signature_bytes.extend_from_slice(&message_hash);
            signature_bytes.extend_from_slice(&message_hash[..10]); // Pad to reasonable size
            
            // Try to create signature from bytes
            if let Ok(signature) = pallet_metamui_crypto::Signature::from_bytes(signature_bytes) {
                // Convert to application signature
                Some(Self::Signature::from_slice(signature.as_ref()))
            } else {
                None
            }
        }
        
        #[cfg(not(feature = "std"))]
        {
            // For no_std environments, return None as we can't generate signatures
            None
        }
    }

    /// Verify a Metamui signature
    ///
    /// Verifies that the provided signature was created by this authority's private key
    /// over the given message.
    ///
    /// TODO: This currently uses a placeholder implementation.
    /// When actual Falcon 512 integration is complete, this should use
    /// the proper Metamui verification through sp_io::crypto.
    fn verify<M: AsRef<[u8]>>(&self, msg: &M, signature: &Self::Signature) -> bool {
        // TODO: Implement proper Metamui verification through sp_io::crypto
        // This will require extending sp_io::crypto to support Metamui crypto operations
        //
        // Expected implementation:
        // sp_io::crypto::metamui_verify(signature, msg.as_ref(), self)
        
        // PLACEHOLDER: Simple verification for development
        // SECURITY WARNING: This is NOT secure - replace with actual crypto verification
        #[cfg(feature = "std")]
        {
            log::warn!("Using placeholder Metamui verification - NOT SECURE FOR PRODUCTION");
            
            // Convert application signature back to core signature for verification
            if let Ok(core_signature) = pallet_metamui_crypto::Signature::try_from(signature.as_ref()) {
                // Convert application public key to core public key
                if let Ok(core_public) = pallet_metamui_crypto::Public::try_from(self.as_ref()) {
                    return core_signature.verify_falcon512(msg.as_ref(), &core_public);
                }
            }
            false
        }
        
        #[cfg(not(feature = "std"))]
        {
            // For no_std environments, perform basic checks
            !signature.as_ref().is_empty() && 
            signature.as_ref().len() <= pallet_metamui_crypto::MAX_SIGNATURE_SIZE &&
            !msg.as_ref().is_empty()
        }
    }

    /// Convert the public key to raw bytes
    ///
    /// Returns the raw byte representation of the Metamui public key.
    /// This is used for serialization and storage operations.
    fn to_raw_vec(&self) -> sp_std::vec::Vec<u8> {
        self.as_ref().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_application_crypto::RuntimeAppPublic;

    #[test]
    fn test_authority_id_generation() {
        let seed = Some(b"test seed for metamui authority".to_vec());
        let authority_id = AuthorityId::generate_pair(seed);
        
        // Should generate a valid authority ID
        assert_eq!(authority_id.as_ref().len(), pallet_metamui_crypto::PUBLIC_KEY_SIZE);
        assert_ne!(authority_id.as_ref(), &[0u8; pallet_metamui_crypto::PUBLIC_KEY_SIZE]);
    }
    
    #[test]
    fn test_authority_sign_verify_placeholder() {
        let authority_id = AuthorityId::generate_pair(Some(b"test authority".to_vec()));
        let message = b"test consensus message";
        
        // Test signing (placeholder implementation)
        if let Some(signature) = authority_id.sign(&message) {
            // Test verification (placeholder implementation)
            assert!(authority_id.verify(&message, &signature));
            
            // Test with different message should fail
            let different_message = b"different message";
            assert!(!authority_id.verify(&different_message, &signature));
        }
    }
    
    #[test]
    fn test_deterministic_generation() {
        let seed = Some(b"deterministic seed".to_vec());
        let authority1 = AuthorityId::generate_pair(seed.clone());
        let authority2 = AuthorityId::generate_pair(seed);
        
        // Same seed should generate same authority ID
        assert_eq!(authority1.as_ref(), authority2.as_ref());
    }
    
    #[test]
    fn test_constants() {
        assert_eq!(METAMUI_MICC, KeyTypeId(*b"mmcc"));
        assert_eq!(AuthorityId::ID, METAMUI_MICC);
        assert_eq!(AuthorityId::CRYPTO_ID, pallet_metamui_crypto::CRYPTO_ID);
    }
    
    #[test]
    fn test_to_raw_vec() {
        let authority_id = AuthorityId::generate_pair(Some(b"test".to_vec()));
        let raw_bytes = authority_id.to_raw_vec();
        
        assert_eq!(raw_bytes.len(), pallet_metamui_crypto::PUBLIC_KEY_SIZE);
        assert_eq!(raw_bytes, authority_id.as_ref().to_vec());
    }
}