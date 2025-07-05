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

use sp_application_crypto::KeyTypeId;

/// Key type identifier for Metamui MICC consensus
/// 
/// This unique identifier is used by the Substrate keystore to distinguish
/// Metamui consensus keys from other key types in the system.
pub const METAMUI_MICC: KeyTypeId = KeyTypeId(*b"mmcc"); // "metamui micc consensus"

/// Application-specific crypto module for Metamui
mod app_metamui {
    use super::METAMUI_MICC;
    use pallet_metamui_crypto as metamui; // Import our Metamui crypto module
    use sp_application_crypto::app_crypto;
    
    /// Create application-specific crypto using Metamui scheme
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

// The app_crypto! macro automatically provides RuntimeAppPublic implementation
// We don't need to manually implement it here

#[cfg(test)]
mod tests {
    use super::*;
    use sp_application_crypto::{Pair, ByteArray};

    #[test]
    fn test_constants() {
        assert_eq!(METAMUI_MICC, KeyTypeId(*b"mmcc"));
    }
    
    #[test]
    fn test_authority_pair_generation() {
        // Test generating a pair
        let pair = AuthorityPair::generate().0;
        let public = pair.public();
        
        // Should generate a valid authority ID
        assert_eq!(public.as_ref().len(), pallet_metamui_crypto::PUBLIC_KEY_SIZE);
        assert_ne!(public.as_ref(), &[0u8; pallet_metamui_crypto::PUBLIC_KEY_SIZE]);
    }
    
    #[test]
    fn test_authority_sign_verify() {
        let pair = AuthorityPair::generate().0;
        let public = pair.public();
        let message = b"test consensus message";
        
        // Test signing and verification
        let signature = pair.sign(message);
        assert!(AuthorityPair::verify(&signature, message, &public));
        
        // Test with different message should fail
        let different_message = b"different message";
        assert!(!AuthorityPair::verify(&signature, different_message, &public));
    }
    
    #[test]
    fn test_deterministic_generation() {
        let seed = b"deterministic seed";
        let pair1 = AuthorityPair::from_string(seed, None).unwrap();
        let pair2 = AuthorityPair::from_string(seed, None).unwrap();
        
        // Same seed should generate same authority pair
        assert_eq!(pair1.public().as_ref(), pair2.public().as_ref());
    }
}