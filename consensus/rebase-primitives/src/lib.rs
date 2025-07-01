//! Rebase API primitives for Genesis Block Rebasing
//! 
//! This crate provides the runtime API trait and associated types for implementing
//! blockchain rebasing functionality in Substrate-based blockchains.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_runtime::traits::NumberFor;
use sp_runtime::DispatchError;
use sp_std::vec::Vec;

#[cfg(feature = "std")]
use std::result::Result;
#[cfg(not(feature = "std"))]
use sp_std::result::Result;

/// Metadata about rebase operations
#[derive(Encode, Decode, TypeInfo, Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct RebaseMetadata<BlockNumber> {
    /// Block number of the last rebase operation
    pub last_rebase_block: BlockNumber,
    /// Total number of rebases performed
    pub rebase_count: u32,
    /// Number of archived block ranges
    pub archive_count: u32,
    /// Next scheduled rebase block (if automatic rebasing is enabled)
    pub next_scheduled_rebase: Option<BlockNumber>,
}

/// Storage backend types for archival
#[derive(Encode, Decode, TypeInfo, Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub enum StorageBackend {
    /// Local filesystem storage
    Local,
    /// IPFS distributed storage
    IPFS,
    /// Amazon S3 cloud storage
    S3,
    /// Google Cloud Storage
    GCS,
    /// Azure Blob Storage
    Azure,
}

/// Rebase operation status
#[derive(Encode, Decode, TypeInfo, Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub enum RebaseStatus {
    /// No rebase in progress
    Idle,
    /// State export in progress
    ExportingState,
    /// Archiving blocks
    Archiving,
    /// Creating new genesis
    CreatingGenesis,
    /// Waiting for governance approval
    PendingApproval,
    /// Rebase failed with error message
    Failed(Vec<u8>),
}

/// Runtime API for rebase operations
sp_api::decl_runtime_apis! {
    /// API for blockchain rebasing operations
    pub trait RebaseApi {
        /// Export the full runtime state at a specific block
        fn export_state(at_block: Option<Block::Hash>) -> Result<Vec<u8>, DispatchError>;
        
        /// Export state for a specific pallet only
        fn export_pallet_state(pallet_name: Vec<u8>, at_block: Option<Block::Hash>) -> Result<Vec<u8>, DispatchError>;
        
        /// Validate that a state root matches the current state
        fn validate_state_root(state_root: [u8; 32]) -> bool;
        
        /// Get rebase metadata and status
        fn get_rebase_metadata() -> RebaseMetadata<NumberFor<Block>>;
        
        /// Get current rebase status
        fn get_rebase_status() -> RebaseStatus;
        
        /// Calculate the next automatic rebase block (if enabled)
        fn get_next_rebase_block() -> Option<NumberFor<Block>>;
        
        /// Check if automatic rebasing is enabled for current environment
        fn is_auto_rebase_enabled() -> bool;
        
        /// Get the rebase interval in blocks for current environment
        fn get_rebase_interval() -> u32;
        
        /// Get off-chain worker heartbeat (last block number it ran on)
        fn get_heartbeat() -> Option<u32>;
    }
}