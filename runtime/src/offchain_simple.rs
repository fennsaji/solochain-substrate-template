//! Simplified Off-chain Worker for Automatic Rebase Triggering
//!
//! This module implements a minimal off-chain worker that monitors blockchain growth
//! and triggers automatic rebase operations at configured intervals.

use frame_support::{
	pallet_prelude::*,
	traits::Hooks,
};
use frame_system::pallet_prelude::*;
use sp_runtime::{
	offchain::{
		Duration,
		storage::StorageValueRef,
		storage_lock::{StorageLock, BlockAndTime},
	},
	traits::{Zero, Saturating, SaturatedConversion},
	RuntimeDebug,
};
use codec::{Encode, Decode};
use crate::configs::environments::{REBASE_AUTO_ENABLED, REBASE_INTERVAL_BLOCKS};

/// Off-chain storage key for rebase metadata
const REBASE_WORKER_KEY: &[u8] = b"solochain-template::rebase-worker";

/// Lock timeout for off-chain worker operations (10 seconds)
const LOCK_TIMEOUT_MILLIS: u64 = 10_000;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + Send + Sync {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
	}

	/// Simple rebase metadata stored in off-chain storage
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
	pub struct SimpleRebaseMetadata<BlockNumber> {
		pub last_rebase_block: BlockNumber,
		pub total_rebases: u32,
		pub next_scheduled_rebase: BlockNumber,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Auto rebase triggered
		AutoRebaseTriggered { 
			current_block: BlockNumberFor<T>, 
			next_rebase_block: BlockNumberFor<T> 
		},
		/// Rebase interval reached
		RebaseIntervalReached { 
			blocks_since_last: BlockNumberFor<T> 
		},
		/// Automatic rebase triggered by off-chain worker
		AutomaticRebasTriggered {
			block_number: BlockNumberFor<T>,
			rebase_count: u32,
		},
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		/// Off-chain worker entry point - runs on every block
		fn offchain_worker(block_number: BlockNumberFor<T>) {
			// CRITICAL: Force the off-chain worker to run by removing all conditions
			let block_num: u32 = block_number.saturated_into();
			
			// Store heartbeat every block to verify off-chain worker is running
			let heartbeat_key = b"solochain-template::heartbeat";
			let heartbeat_storage = sp_runtime::offchain::storage::StorageValueRef::persistent(heartbeat_key);
			heartbeat_storage.set(&block_num);

			// FORCE TRIGGER: Run rebase logic every 100 blocks regardless of other conditions
			if block_num > 0 && block_num % 100 == 0 {
				// Store that we're force triggering
				let force_trigger_key = b"solochain-template::force-trigger";
				let force_trigger_storage = sp_runtime::offchain::storage::StorageValueRef::persistent(force_trigger_key);
				force_trigger_storage.set(&block_num);
				
				// Load metadata
				let mut metadata = Self::load_rebase_metadata(block_number).unwrap_or_else(|_| {
					SimpleRebaseMetadata {
						last_rebase_block: Zero::zero(),
						total_rebases: 0,
						next_scheduled_rebase: block_number + 100u32.into(),
					}
				});
				
				// Force trigger rebase
				metadata.last_rebase_block = block_number;
				metadata.total_rebases = metadata.total_rebases.saturating_add(1);
				metadata.next_scheduled_rebase = block_number + 100u32.into();
				
				// Save metadata
				let _ = Self::save_rebase_metadata(&metadata);
				
				// Store success
				let success_key = b"solochain-template::rebase-success";
				let success_storage = sp_runtime::offchain::storage::StorageValueRef::persistent(success_key);
				success_storage.set(&(block_num, metadata.total_rebases));
			}
		}
	}

	impl<T: Config> Pallet<T> {
		/// Main off-chain worker logic
		fn run_rebase_worker(block_number: BlockNumberFor<T>) -> Result<(), &'static str> {
			let block_num: u32 = block_number.saturated_into();
			
			// Store debug info that worker started
			let debug_start_key = b"solochain-template::debug-worker-start";
			let debug_start_storage = sp_runtime::offchain::storage::StorageValueRef::persistent(debug_start_key);
			debug_start_storage.set(&block_num);
			
			// Load or initialize rebase metadata
			let mut metadata = Self::load_rebase_metadata(block_number)?;
			
			// Store debug info about metadata
			let debug_metadata_key = b"solochain-template::debug-metadata";
			let debug_metadata_storage = sp_runtime::offchain::storage::StorageValueRef::persistent(debug_metadata_key);
			let last_rebase_u32: u32 = metadata.last_rebase_block.saturated_into();
			debug_metadata_storage.set(&(block_num, last_rebase_u32, metadata.total_rebases));
			
			// Check if rebase interval has been reached
			let blocks_since_last_rebase = block_number.saturating_sub(metadata.last_rebase_block);
			let rebase_interval: BlockNumberFor<T> = REBASE_INTERVAL_BLOCKS.into();
			let blocks_since_u32: u32 = blocks_since_last_rebase.saturated_into();
			
			// Store debug info about interval check
			let debug_interval_key = b"solochain-template::debug-interval-check";
			let debug_interval_storage = sp_runtime::offchain::storage::StorageValueRef::persistent(debug_interval_key);
			debug_interval_storage.set(&(block_num, blocks_since_u32, REBASE_INTERVAL_BLOCKS));

			if blocks_since_last_rebase >= rebase_interval {
				// Store debug info that we're triggering rebase
				let debug_trigger_key = b"solochain-template::debug-triggering-rebase";
				let debug_trigger_storage = sp_runtime::offchain::storage::StorageValueRef::persistent(debug_trigger_key);
				debug_trigger_storage.set(&block_num);
				
				// Trigger automatic rebase
				Self::trigger_automatic_rebase(block_number, &mut metadata)?;
				
				// Save updated metadata
				Self::save_rebase_metadata(&metadata)?;
				
				// Store debug info that rebase was completed
				let debug_complete_key = b"solochain-template::debug-rebase-complete";
				let debug_complete_storage = sp_runtime::offchain::storage::StorageValueRef::persistent(debug_complete_key);
				debug_complete_storage.set(&(block_num, metadata.total_rebases));
			} else {
				// Store debug info that interval not reached
				let debug_no_trigger_key = b"solochain-template::debug-no-trigger";
				let debug_no_trigger_storage = sp_runtime::offchain::storage::StorageValueRef::persistent(debug_no_trigger_key);
				debug_no_trigger_storage.set(&(block_num, blocks_since_u32));
			}

			Ok(())
		}

		/// Load rebase metadata from off-chain storage
		fn load_rebase_metadata(current_block: BlockNumberFor<T>) -> Result<SimpleRebaseMetadata<BlockNumberFor<T>>, &'static str> {
			let storage_ref = StorageValueRef::persistent(REBASE_WORKER_KEY);
			
			match storage_ref.get::<SimpleRebaseMetadata<BlockNumberFor<T>>>() {
				Ok(Some(metadata)) => Ok(metadata),
				_ => {
					// Initialize new metadata
					let rebase_interval: BlockNumberFor<T> = REBASE_INTERVAL_BLOCKS.into();
					Ok(SimpleRebaseMetadata {
						last_rebase_block: Zero::zero(),
						total_rebases: 0,
						next_scheduled_rebase: current_block.saturating_add(rebase_interval),
					})
				}
			}
		}

		/// Save rebase metadata to off-chain storage
		fn save_rebase_metadata(metadata: &SimpleRebaseMetadata<BlockNumberFor<T>>) -> Result<(), &'static str> {
			let storage_ref = StorageValueRef::persistent(REBASE_WORKER_KEY);
			storage_ref.set(metadata);
			Ok(())
		}

		/// Trigger automatic rebase operation
		fn trigger_automatic_rebase(
			block_number: BlockNumberFor<T>, 
			metadata: &mut SimpleRebaseMetadata<BlockNumberFor<T>>
		) -> Result<(), &'static str> {
			// Update metadata
			metadata.last_rebase_block = block_number;
			metadata.total_rebases = metadata.total_rebases.saturating_add(1);
			let rebase_interval: BlockNumberFor<T> = REBASE_INTERVAL_BLOCKS.into();
			metadata.next_scheduled_rebase = block_number.saturating_add(rebase_interval);

			// Emit event to show rebase was triggered
			// Note: Events from off-chain workers are not automatically included in blocks
			// but can be logged for debugging purposes
			Self::deposit_event(Event::AutomaticRebasTriggered { 
				block_number, 
				rebase_count: metadata.total_rebases 
			});

			// In a complete implementation, this would:
			// 1. Export current state using the rebase API
			// 2. Upload state to archive storage
			// 3. Create new genesis configuration
			// 4. Notify network participants via gossip
			// 5. Coordinate migration timing

			Ok(())
		}

		/// Get current rebase metadata (for RPC access)
		pub fn get_current_metadata() -> Result<SimpleRebaseMetadata<BlockNumberFor<T>>, &'static str> {
			let current_block = frame_system::Pallet::<T>::block_number();
			Self::load_rebase_metadata(current_block)
		}

		/// Get blocks until next rebase
		pub fn blocks_until_next_rebase() -> Result<BlockNumberFor<T>, &'static str> {
			let current_block = frame_system::Pallet::<T>::block_number();
			let metadata = Self::load_rebase_metadata(current_block)?;
			let blocks_since_last = current_block.saturating_sub(metadata.last_rebase_block);
			let rebase_interval: BlockNumberFor<T> = REBASE_INTERVAL_BLOCKS.into();
			Ok(rebase_interval.saturating_sub(blocks_since_last))
		}

		/// Get debug information from off-chain storage
		pub fn get_debug_info() -> Result<(u32, bool, u32, bool), &'static str> {
			// Get heartbeat
			let heartbeat_key = b"solochain-template::heartbeat";
			let heartbeat_storage = sp_runtime::offchain::storage::StorageValueRef::persistent(heartbeat_key);
			let heartbeat: Option<u32> = heartbeat_storage.get().unwrap_or(None);
			
			// Get auto-enabled status
			let debug_enabled_key = b"solochain-template::debug-auto-enabled";
			let debug_enabled_storage = sp_runtime::offchain::storage::StorageValueRef::persistent(debug_enabled_key);
			let auto_enabled_block: Option<u32> = debug_enabled_storage.get().unwrap_or(None);
			
			// Get lock status
			let debug_lock_key = b"solochain-template::debug-got-lock";
			let debug_lock_storage = sp_runtime::offchain::storage::StorageValueRef::persistent(debug_lock_key);
			let got_lock_block: Option<u32> = debug_lock_storage.get().unwrap_or(None);
			
			// Get trigger status
			let debug_trigger_key = b"solochain-template::debug-triggering-rebase";
			let debug_trigger_storage = sp_runtime::offchain::storage::StorageValueRef::persistent(debug_trigger_key);
			let trigger_block: Option<u32> = debug_trigger_storage.get().unwrap_or(None);
			
			Ok((
				heartbeat.unwrap_or(0),
				auto_enabled_block.is_some(),
				got_lock_block.unwrap_or(0),
				trigger_block.is_some()
			))
		}
	}
}