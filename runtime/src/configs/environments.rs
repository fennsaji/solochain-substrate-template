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

//! Environment-specific configuration parameters for different deployment contexts.
//! 
//! This module provides configuration overrides for development, staging, and production
//! environments, allowing for appropriate parameter tuning based on deployment context.

use alloc::{vec::Vec, string::{String, ToString}, format};

/// Environment types for configuration selection
#[derive(Clone, Debug, PartialEq)]
pub enum Environment {
    Development,
    Local,
    Staging,
    Production,
}

impl Environment {
    /// Parse environment from string (case-insensitive)
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "dev" | "development" => Ok(Environment::Development),
            "local" => Ok(Environment::Local),
            "staging" => Ok(Environment::Staging),
            "prod" | "production" => Ok(Environment::Production),
            _ => Err(format!("Unknown environment: {}. Valid options: dev, local, staging, production", s)),
        }
    }

    /// Get environment from cargo features
    pub fn from_env() -> Self {
        #[cfg(feature = "production")]
        return Environment::Production;
        #[cfg(feature = "staging")]
        return Environment::Staging;
        #[cfg(feature = "local-testnet")]
        return Environment::Local;
        #[cfg(not(any(feature = "production", feature = "staging", feature = "local-testnet")))]
        return Environment::Development;
    }
}

/// Environment-specific rate limiting parameters
pub struct RateLimitParams {
    pub default_transactions_per_block: u32,
    pub default_transactions_per_minute: u32,
    pub max_transactions_per_account: u32,
    pub max_bytes_per_account: u32,
    pub max_transactions_per_minute: u32,
}

impl RateLimitParams {
    /// Get rate limiting parameters for the environment
    pub fn for_environment(env: &Environment) -> Self {
        match env {
            Environment::Development => Self {
                // Development: Very permissive for testing
                default_transactions_per_block: 1000,
                default_transactions_per_minute: 6000,
                max_transactions_per_account: 1000,
                max_bytes_per_account: 2 * 1024 * 1024, // 2MB
                max_transactions_per_minute: 600,
            },
            Environment::Local => Self {
                // Local testnet: Moderate limits for multi-node testing
                default_transactions_per_block: 500,
                default_transactions_per_minute: 3000,
                max_transactions_per_account: 500,
                max_bytes_per_account: 1 * 1024 * 1024, // 1MB
                max_transactions_per_minute: 300,
            },
            Environment::Staging => Self {
                // Staging: Production-like limits for testing
                default_transactions_per_block: 200,
                default_transactions_per_minute: 1200,
                max_transactions_per_account: 200,
                max_bytes_per_account: 512 * 1024, // 512KB
                max_transactions_per_minute: 120,
            },
            Environment::Production => Self {
                // Production: Conservative limits for security and performance
                default_transactions_per_block: 100,
                default_transactions_per_minute: 600,
                max_transactions_per_account: 100,
                max_bytes_per_account: 512 * 1024, // 512KB
                max_transactions_per_minute: 60,
            },
        }
    }
}

/// Environment-specific consensus parameters
pub struct ConsensusParams {
    pub max_authorities: u32,
    pub allow_multiple_blocks_per_slot: bool,
    pub block_hash_count: u32,
}

impl ConsensusParams {
    /// Get consensus parameters for the environment
    pub fn for_environment(env: &Environment) -> Self {
        match env {
            Environment::Development => Self {
                // Development: Flexible validator count for testing (up to 10)
                max_authorities: 10,
                allow_multiple_blocks_per_slot: true, // Flexible for dev
                block_hash_count: 250, // Lower for faster pruning
            },
            Environment::Local => Self {
                // Local testnet: Small validator set
                max_authorities: 10,
                allow_multiple_blocks_per_slot: false,
                block_hash_count: 1200, // 10 minutes at 500ms blocks
            },
            Environment::Staging => Self {
                // Staging: Production-like validator set
                max_authorities: 21,
                allow_multiple_blocks_per_slot: false,
                block_hash_count: 2400, // 20 minutes at 500ms blocks
            },
            Environment::Production => Self {
                // Production: Secure validator set size
                max_authorities: 32,
                allow_multiple_blocks_per_slot: false,
                block_hash_count: 7200, // 1 hour at 500ms blocks
            },
        }
    }
}

/// Environment-specific network parameters
pub struct NetworkParams {
    pub ss58_prefix: u8,
    pub existential_deposit_multiplier: u128,
    pub max_consumers: u32,
}

impl NetworkParams {
    /// Get network parameters for the environment
    pub fn for_environment(env: &Environment) -> Self {
        match env {
            Environment::Development => Self {
                // Development: Generic Substrate prefix
                ss58_prefix: 42,
                existential_deposit_multiplier: 1, // 1 * UNIT = standard ED
                max_consumers: 16,
            },
            Environment::Local => Self {
                // Local testnet: Generic prefix for testing
                ss58_prefix: 42,
                existential_deposit_multiplier: 1,
                max_consumers: 16,
            },
            Environment::Staging => Self {
                // Staging: Custom prefix for testing (should match production)
                ss58_prefix: 42, // TODO: Replace with registered staging prefix
                existential_deposit_multiplier: 10, // Higher ED for testing
                max_consumers: 32,
            },
            Environment::Production => Self {
                // Production: Unique registered prefix
                ss58_prefix: 42, // TODO: Replace with registered production prefix (e.g., 2048)
                existential_deposit_multiplier: 100, // Higher ED for spam protection
                max_consumers: 32,
            },
        }
    }
}

/// Validation for production environment configuration
pub struct EnvironmentValidator;

impl EnvironmentValidator {
    /// Validate configuration parameters for the current environment
    pub fn validate() -> Result<(), String> {
        let env = Environment::from_env();
        let mut errors = Vec::new();
        
        match env {
            Environment::Production => {
                // Production-specific validations
                if NETWORK_SS58_PREFIX == 42 {
                    errors.push("Production SS58 prefix must not be 42 (generic Substrate prefix)".to_string());
                }
                
                if RATE_LIMIT_DEFAULT_TXS_PER_BLOCK > 200 {
                    errors.push("Production transaction per block limit is too high".to_string());
                }
                
                if CONSENSUS_ALLOW_MULTIPLE_BLOCKS {
                    errors.push("Production should not allow multiple blocks per slot".to_string());
                }
                
                if CONSENSUS_MAX_AUTHORITIES < 3 {
                    errors.push("Production should have at least 3 authorities for security".to_string());
                }
            },
            Environment::Staging => {
                // Staging-specific validations
                if CONSENSUS_MAX_AUTHORITIES < 2 {
                    errors.push("Staging should have at least 2 authorities for testing".to_string());
                }
            },
            Environment::Local => {
                // Local is permissive for testing
            },
            Environment::Development => {
                // Development validations (minimal)
                // Development is permissive for testing
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(format!("{:?} environment validation failed:\n{}", env, errors.join("\n")))
        }
    }
    
    /// Get a summary of current environment configuration
    pub fn get_config_summary() -> String {
        let env = Environment::from_env();
        format!(
            "Environment: {:?}\n\
            Rate Limiting:\n\
            - Default Transactions Per Block: {}\n\
            - Default Transactions Per Minute: {}\n\
            - Max Transactions Per Account: {}\n\
            - Max Bytes Per Account: {} bytes\n\
            - Max Transactions Per Minute: {}\n\
            Consensus:\n\
            - Max Authorities: {}\n\
            - Allow Multiple Blocks Per Slot: {}\n\
            - Block Hash Count: {}\n\
            Network:\n\
            - SS58 Prefix: {}\n\
            - Max Consumers: {}",
            env,
            RATE_LIMIT_DEFAULT_TXS_PER_BLOCK,
            RATE_LIMIT_DEFAULT_TXS_PER_MINUTE,
            RATE_LIMIT_MAX_TXS_PER_ACCOUNT,
            RATE_LIMIT_MAX_BYTES_PER_ACCOUNT,
            RATE_LIMIT_MAX_TXS_PER_MINUTE,
            CONSENSUS_MAX_AUTHORITIES,
            CONSENSUS_ALLOW_MULTIPLE_BLOCKS,
            CONSENSUS_BLOCK_HASH_COUNT,
            NETWORK_SS58_PREFIX,
            NETWORK_MAX_CONSUMERS
        )
    }
}

/// Compile-time environment-specific constants using cfg attributes
/// Environment is determined by CARGO_CFG features set during build

// Rate limiting parameters
#[cfg(feature = "production")]
pub const RATE_LIMIT_DEFAULT_TXS_PER_BLOCK: u32 = 100;
#[cfg(feature = "staging")]
pub const RATE_LIMIT_DEFAULT_TXS_PER_BLOCK: u32 = 200;
#[cfg(feature = "local-testnet")]
pub const RATE_LIMIT_DEFAULT_TXS_PER_BLOCK: u32 = 500;
#[cfg(not(any(feature = "production", feature = "staging", feature = "local-testnet")))]
pub const RATE_LIMIT_DEFAULT_TXS_PER_BLOCK: u32 = 1000; // Development default

#[cfg(feature = "production")]
pub const RATE_LIMIT_DEFAULT_TXS_PER_MINUTE: u32 = 600;
#[cfg(feature = "staging")]
pub const RATE_LIMIT_DEFAULT_TXS_PER_MINUTE: u32 = 1200;
#[cfg(feature = "local-testnet")]
pub const RATE_LIMIT_DEFAULT_TXS_PER_MINUTE: u32 = 3000;
#[cfg(not(any(feature = "production", feature = "staging", feature = "local-testnet")))]
pub const RATE_LIMIT_DEFAULT_TXS_PER_MINUTE: u32 = 6000; // Development default

#[cfg(feature = "production")]
pub const RATE_LIMIT_MAX_TXS_PER_ACCOUNT: u32 = 100;
#[cfg(feature = "staging")]
pub const RATE_LIMIT_MAX_TXS_PER_ACCOUNT: u32 = 200;
#[cfg(feature = "local-testnet")]
pub const RATE_LIMIT_MAX_TXS_PER_ACCOUNT: u32 = 500;
#[cfg(not(any(feature = "production", feature = "staging", feature = "local-testnet")))]
pub const RATE_LIMIT_MAX_TXS_PER_ACCOUNT: u32 = 1000; // Development default

#[cfg(feature = "production")]
pub const RATE_LIMIT_MAX_BYTES_PER_ACCOUNT: u32 = 512 * 1024; // 512KB
#[cfg(feature = "staging")]
pub const RATE_LIMIT_MAX_BYTES_PER_ACCOUNT: u32 = 512 * 1024; // 512KB
#[cfg(feature = "local-testnet")]
pub const RATE_LIMIT_MAX_BYTES_PER_ACCOUNT: u32 = 1024 * 1024; // 1MB
#[cfg(not(any(feature = "production", feature = "staging", feature = "local-testnet")))]
pub const RATE_LIMIT_MAX_BYTES_PER_ACCOUNT: u32 = 2 * 1024 * 1024; // 2MB development

#[cfg(feature = "production")]
pub const RATE_LIMIT_MAX_TXS_PER_MINUTE: u32 = 60;
#[cfg(feature = "staging")]
pub const RATE_LIMIT_MAX_TXS_PER_MINUTE: u32 = 120;
#[cfg(feature = "local-testnet")]
pub const RATE_LIMIT_MAX_TXS_PER_MINUTE: u32 = 300;
#[cfg(not(any(feature = "production", feature = "staging", feature = "local-testnet")))]
pub const RATE_LIMIT_MAX_TXS_PER_MINUTE: u32 = 600; // Development default

// Consensus parameters
#[cfg(feature = "production")]
pub const CONSENSUS_MAX_AUTHORITIES: u32 = 32;
#[cfg(feature = "staging")]
pub const CONSENSUS_MAX_AUTHORITIES: u32 = 21;
#[cfg(feature = "local-testnet")]
pub const CONSENSUS_MAX_AUTHORITIES: u32 = 5;
#[cfg(not(any(feature = "production", feature = "staging", feature = "local-testnet")))]
pub const CONSENSUS_MAX_AUTHORITIES: u32 = 10; // Development default - flexible for testing

#[cfg(any(feature = "production", feature = "staging", feature = "local-testnet"))]
pub const CONSENSUS_ALLOW_MULTIPLE_BLOCKS: bool = false;
#[cfg(not(any(feature = "production", feature = "staging", feature = "local-testnet")))]
pub const CONSENSUS_ALLOW_MULTIPLE_BLOCKS: bool = true; // Development only

#[cfg(feature = "production")]
pub const CONSENSUS_BLOCK_HASH_COUNT: u32 = 7200; // 1 hour at 500ms blocks
#[cfg(feature = "staging")]
pub const CONSENSUS_BLOCK_HASH_COUNT: u32 = 2400; // 20 minutes at 500ms blocks
#[cfg(feature = "local-testnet")]
pub const CONSENSUS_BLOCK_HASH_COUNT: u32 = 1200; // 10 minutes at 500ms blocks
#[cfg(not(any(feature = "production", feature = "staging", feature = "local-testnet")))]
pub const CONSENSUS_BLOCK_HASH_COUNT: u32 = 250; // 2 minutes for development

// Network parameters  
#[cfg(feature = "production")]
pub const NETWORK_SS58_PREFIX: u8 = 42; // TODO: Register unique production prefix (e.g., 2048)
#[cfg(feature = "staging")]
pub const NETWORK_SS58_PREFIX: u8 = 42; // TODO: Register unique staging prefix
#[cfg(not(any(feature = "production", feature = "staging")))]
pub const NETWORK_SS58_PREFIX: u8 = 42; // Generic Substrate prefix for dev/local

#[cfg(any(feature = "production", feature = "staging"))]
pub const NETWORK_MAX_CONSUMERS: u32 = 32;
#[cfg(not(any(feature = "production", feature = "staging")))]
pub const NETWORK_MAX_CONSUMERS: u32 = 16; // Development/Local