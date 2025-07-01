//! System-level off-chain worker integration for automatic rebase triggering
//!
//! This integrates directly with the system pallet to ensure off-chain worker execution

use frame_system::pallet_prelude::*;
use sp_runtime::{
	offchain::storage::StorageValueRef,
	traits::{Zero, Saturating, SaturatedConversion},
};
use codec::{Encode, Decode};

/// Rebase metadata stored in off-chain storage
#[derive(Encode, Decode, Clone, PartialEq, Eq)]
pub struct RebaseMetadata<BlockNumber> {
	pub last_rebase_block: BlockNumber,
	pub total_rebases: u32,
	pub next_scheduled_rebase: BlockNumber,
}

/// Storage key for rebase metadata
const REBASE_METADATA_KEY: &[u8] = b"solochain-template::rebase-metadata";

/// Storage key for heartbeat
const HEARTBEAT_KEY: &[u8] = b"solochain-template::heartbeat";

/// Rebase interval (100 blocks)
const REBASE_INTERVAL: u32 = 100;

/// Main off-chain worker function that gets called from system pallet
pub fn run_offchain_worker<T: frame_system::Config>(block_number: BlockNumberFor<T>) {
	let block_num: u32 = block_number.saturated_into();
	
	// Store heartbeat to verify off-chain worker is running
	let heartbeat_storage = StorageValueRef::persistent(HEARTBEAT_KEY);
	heartbeat_storage.set(&block_num);
	
	// Check if it's time for a rebase (every 100 blocks)
	if block_num > 0 && block_num % REBASE_INTERVAL == 0 {
		// Load existing metadata
		let metadata_storage = StorageValueRef::persistent(REBASE_METADATA_KEY);
		let mut metadata: RebaseMetadata<BlockNumberFor<T>> = metadata_storage
			.get()
			.unwrap_or(None)
			.unwrap_or_else(|| RebaseMetadata {
				last_rebase_block: Zero::zero(),
				total_rebases: 0,
				next_scheduled_rebase: block_number + REBASE_INTERVAL.into(),
			});
		
		// Update metadata
		metadata.last_rebase_block = block_number;
		metadata.total_rebases = metadata.total_rebases.saturating_add(1);
		metadata.next_scheduled_rebase = block_number + REBASE_INTERVAL.into();
		
		// Save updated metadata
		metadata_storage.set(&metadata);
		
		// Store trigger event
		let trigger_key = b"solochain-template::trigger-event";
		let trigger_storage = StorageValueRef::persistent(trigger_key);
		trigger_storage.set(&(block_num, metadata.total_rebases));
	}
}

/// Get current rebase metadata
pub fn get_rebase_metadata<T: frame_system::Config>() -> RebaseMetadata<BlockNumberFor<T>> {
	let metadata_storage = StorageValueRef::persistent(REBASE_METADATA_KEY);
	metadata_storage
		.get()
		.unwrap_or(None)
		.unwrap_or_else(|| RebaseMetadata {
			last_rebase_block: Zero::zero(),
			total_rebases: 0,
			next_scheduled_rebase: 100u32.into(),
		})
}

/// Get heartbeat (last block the off-chain worker ran)
pub fn get_heartbeat() -> Option<u32> {
	let heartbeat_storage = StorageValueRef::persistent(HEARTBEAT_KEY);
	heartbeat_storage.get().unwrap_or(None)
}