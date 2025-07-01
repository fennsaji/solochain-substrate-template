//! Off-chain Worker Integration for Genesis Rebase Strategy
//!
//! This module implements an off-chain worker that monitors blockchain growth
//! and triggers automatic rebase operations at configured intervals.

use frame_support::{
	pallet_prelude::*,
	traits::{Get, Hooks},
};
use frame_system::pallet_prelude::*;
use sp_runtime::{
	offchain::{
		Duration,
		storage::{StorageValueRef, StorageRetrievalError},
		storage_lock::{StorageLock, BlockAndTime},
	},
	traits::{Zero, Saturating},
	RuntimeDebug,
};
use sp_std::{vec::Vec, collections::vec_deque::VecDeque};
use codec::{Encode, Decode};
use crate::configs::environments::{REBASE_AUTO_ENABLED, REBASE_INTERVAL_BLOCKS};
use sp_std::prelude::*;

/// Off-chain storage keys for persistent data
const REBASE_WORKER_KEY: &[u8] = b"solochain-template::rebase-worker";
const LAST_REBASE_BLOCK_KEY: &[u8] = b"solochain-template::last-rebase-block";
const REBASE_ALERTS_KEY: &[u8] = b"solochain-template::rebase-alerts";

/// Maximum number of rebase alerts to keep in storage
const MAX_REBASE_ALERTS: usize = 10;

/// Lock timeout for off-chain worker operations (10 seconds)
const LOCK_TIMEOUT_DURATION: Duration = Duration::from_millis(10_000);

/// HTTP request timeout for archive health checks (5 seconds)
const HTTP_TIMEOUT: Duration = Duration::from_millis(5_000);

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + Send + Sync {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Grace period for archive storage check failures
		type ArchiveGracePeriod: Get<BlockNumberFor<Self>>;

		/// Rebase threshold warning blocks before interval
		type RebaseWarningBlocks: Get<BlockNumberFor<Self>>;
	}

	/// Rebase metadata stored in off-chain storage
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
	pub struct RebaseMetadata<BlockNumber> {
		pub last_rebase_block: BlockNumber,
		pub total_rebases: u32,
		pub next_scheduled_rebase: BlockNumber,
		pub archive_health_status: ArchiveHealthStatus,
		pub auto_rebase_enabled: bool,
	}

	/// Archive storage health status
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
	pub enum ArchiveHealthStatus {
		Healthy,
		Warning { last_check_failed: bool, consecutive_failures: u32 },
		Critical { unavailable_since: u64 }, // Timestamp
	}

	/// Rebase alert types
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
	pub enum RebaseAlert<BlockNumber> {
		UpcomingRebase { scheduled_block: BlockNumber, blocks_remaining: BlockNumber },
		ArchiveHealthWarning { status: ArchiveHealthStatus },
		AutoRebaseTriggered { block_number: BlockNumber },
		ManualRebaseRequired { reason: Vec<u8> },
	}

	/// Collection of rebase alerts
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
	pub struct RebaseAlerts<BlockNumber> {
		pub alerts: VecDeque<RebaseAlert<BlockNumber>>,
		pub last_updated: BlockNumber,
	}

	impl<BlockNumber> Default for RebaseAlerts<BlockNumber> 
	where 
		BlockNumber: Default,
	{
		fn default() -> Self {
			Self {
				alerts: VecDeque::new(),
				last_updated: BlockNumber::default(),
			}
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Rebase alert generated
		RebaseAlertGenerated { alert_type: Vec<u8>, block_number: BlockNumberFor<T> },
		/// Archive health check completed
		ArchiveHealthChecked { status: Vec<u8>, block_number: BlockNumberFor<T> },
		/// Auto rebase triggered
		AutoRebaseTriggered { scheduled_block: BlockNumberFor<T>, current_block: BlockNumberFor<T> },
		/// Off-chain worker started
		OffchainWorkerStarted { block_number: BlockNumberFor<T> },
		/// Rebase interval reached
		RebaseIntervalReached { next_rebase_block: BlockNumberFor<T> },
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		/// Off-chain worker entry point - runs on every block
		fn offchain_worker(block_number: BlockNumberFor<T>) {
			sp_std::if_std! {
				println!("🔄 Rebase off-chain worker starting at block {:?}", block_number);
			}

			// Early exit if auto rebase is disabled
			if !REBASE_AUTO_ENABLED {
				sp_std::if_std! {
					println!("Auto rebase disabled for this environment");
				}
				return;
			}

			// Get a lock to prevent concurrent execution
			let mut lock = StorageLock::<BlockAndTime<frame_system::Pallet<T>>>::with_block_and_time_deadline(
				REBASE_WORKER_KEY,
				1, // block number offset
				LOCK_TIMEOUT_DURATION,
			);

			if let Ok(_guard) = lock.try_lock() {
				log::debug!("🔒 Acquired rebase worker lock");

				// Run the main worker logic
				if let Err(e) = Self::run_rebase_worker(block_number) {
					log::error!("❌ Rebase worker error: {:?}", e);
				}
			} else {
				log::debug!("⏭️ Rebase worker already running, skipping");
			}
		}
	}

	impl<T: Config> Pallet<T> {
		/// Main off-chain worker logic
		fn run_rebase_worker(block_number: BlockNumberFor<T>) -> Result<(), &'static str> {
			log::info!("🚀 Running rebase worker at block {:?}", block_number);

			// Step 1: Load or initialize rebase metadata
			let mut metadata = Self::load_rebase_metadata(block_number)?;
			
			// Step 2: Check archive storage health
			let archive_status = Self::check_archive_health()?;
			metadata.archive_health_status = archive_status.clone();

			// Step 3: Check if rebase interval has been reached
			let blocks_since_last_rebase = block_number.saturating_sub(metadata.last_rebase_block);
			let rebase_interval: BlockNumberFor<T> = REBASE_INTERVAL_BLOCKS.into();

			if blocks_since_last_rebase >= rebase_interval {
				log::info!("🎯 Rebase interval reached! Blocks since last rebase: {:?}", blocks_since_last_rebase);

				// Check if archive is healthy enough for rebase
				match archive_status {
					ArchiveHealthStatus::Healthy => {
						Self::trigger_automatic_rebase(block_number, &mut metadata)?;
					},
					ArchiveHealthStatus::Warning { consecutive_failures, .. } => {
						if consecutive_failures < 3 {
							log::warn!("⚠️ Archive warning but proceeding with rebase");
							Self::trigger_automatic_rebase(block_number, &mut metadata)?;
						} else {
							Self::generate_alert(RebaseAlert::ManualRebaseRequired { 
								reason: b"Archive health critical".to_vec() 
							}, block_number)?;
						}
					},
					ArchiveHealthStatus::Critical { .. } => {
						Self::generate_alert(RebaseAlert::ManualRebaseRequired { 
							reason: b"Archive storage unavailable".to_vec() 
						}, block_number)?;
					}
				}
			} else {
				// Step 4: Check if approaching rebase interval for warnings
				let warning_threshold: BlockNumberFor<T> = T::RebaseWarningBlocks::get();
				let blocks_until_rebase = rebase_interval.saturating_sub(blocks_since_last_rebase);
				
				if blocks_until_rebase <= warning_threshold && blocks_until_rebase > Zero::zero() {
					Self::generate_alert(RebaseAlert::UpcomingRebase {
						scheduled_block: metadata.next_scheduled_rebase,
						blocks_remaining: blocks_until_rebase,
					}, block_number)?;
				}
			}

			// Step 5: Save updated metadata
			Self::save_rebase_metadata(&metadata)?;

			log::info!("✅ Rebase worker completed successfully");
			Ok(())
		}

		/// Load rebase metadata from off-chain storage
		fn load_rebase_metadata(current_block: BlockNumberFor<T>) -> Result<RebaseMetadata<BlockNumberFor<T>>, &'static str> {
			let storage_ref = StorageValueRef::persistent(REBASE_WORKER_KEY);
			
			match storage_ref.get::<RebaseMetadata<BlockNumberFor<T>>>() {
				Ok(Some(metadata)) => {
					log::debug!("📄 Loaded existing rebase metadata");
					Ok(metadata)
				},
				Ok(None) => {
					log::info!("🆕 Initializing new rebase metadata");
					let rebase_interval: BlockNumberFor<T> = REBASE_INTERVAL_BLOCKS.into();
					Ok(RebaseMetadata {
						last_rebase_block: Zero::zero(),
						total_rebases: 0,
						next_scheduled_rebase: current_block.saturating_add(rebase_interval),
						archive_health_status: ArchiveHealthStatus::Healthy,
						auto_rebase_enabled: REBASE_AUTO_ENABLED,
					})
				},
				Err(StorageRetrievalError::Undecodable) => {
					log::error!("🔥 Corrupted rebase metadata, reinitializing");
					let rebase_interval: BlockNumberFor<T> = REBASE_INTERVAL_BLOCKS.into();
					Ok(RebaseMetadata {
						last_rebase_block: Zero::zero(),
						total_rebases: 0,
						next_scheduled_rebase: current_block.saturating_add(rebase_interval),
						archive_health_status: ArchiveHealthStatus::Healthy,
						auto_rebase_enabled: REBASE_AUTO_ENABLED,
					})
				}
			}
		}

		/// Save rebase metadata to off-chain storage
		fn save_rebase_metadata(metadata: &RebaseMetadata<BlockNumberFor<T>>) -> Result<(), &'static str> {
			let storage_ref = StorageValueRef::persistent(REBASE_WORKER_KEY);
			storage_ref.set(metadata);
			log::debug!("💾 Saved rebase metadata");
			Ok(())
		}

		/// Check archive storage health via HTTP requests
		fn check_archive_health() -> Result<ArchiveHealthStatus, &'static str> {
			// For now, simulate health check - in production this would make HTTP requests
			// to IPFS nodes, cloud storage endpoints, etc.
			
			// TODO: Replace with actual health checks:
			// - IPFS node connectivity
			// - Cloud storage API availability  
			// - Archive data integrity verification
			
			log::debug!("🏥 Checking archive storage health");
			
			// Simulate random health status for demonstration
			let timestamp = sp_io::offchain::timestamp();
			let health_check_result = (timestamp.unix_millis() % 10) < 8; // 80% success rate
			
			if health_check_result {
				log::debug!("✅ Archive storage healthy");
				Ok(ArchiveHealthStatus::Healthy)
			} else {
				log::warn!("⚠️ Archive storage health warning");
				Ok(ArchiveHealthStatus::Warning { 
					last_check_failed: true, 
					consecutive_failures: 1 
				})
			}
		}

		/// Trigger automatic rebase operation
		fn trigger_automatic_rebase(
			block_number: BlockNumberFor<T>, 
			metadata: &mut RebaseMetadata<BlockNumberFor<T>>
		) -> Result<(), &'static str> {
			log::info!("🎉 Triggering automatic rebase at block {:?}", block_number);

			// Update metadata
			metadata.last_rebase_block = block_number;
			metadata.total_rebases = metadata.total_rebases.saturating_add(1);
			let rebase_interval: BlockNumberFor<T> = REBASE_INTERVAL_BLOCKS.into();
			metadata.next_scheduled_rebase = block_number.saturating_add(rebase_interval);

			// Generate alert
			Self::generate_alert(RebaseAlert::AutoRebaseTriggered { block_number }, block_number)?;

			// TODO: In a complete implementation, this would:
			// 1. Export current state using the rebase API
			// 2. Upload state to archive storage
			// 3. Create new genesis configuration
			// 4. Notify network participants via gossip
			// 5. Coordinate migration timing

			log::info!("🔄 Automatic rebase triggered successfully");
			Ok(())
		}

		/// Generate and store rebase alert
		fn generate_alert(
			alert: RebaseAlert<BlockNumberFor<T>>, 
			block_number: BlockNumberFor<T>
		) -> Result<(), &'static str> {
			let storage_ref = StorageValueRef::persistent(REBASE_ALERTS_KEY);
			
			let mut alerts = match storage_ref.get::<RebaseAlerts<BlockNumberFor<T>>>() {
				Ok(Some(existing)) => existing,
				_ => RebaseAlerts::default(),
			};

			// Add new alert
			alerts.alerts.push_back(alert.clone());
			alerts.last_updated = block_number;

			// Trim to maximum size
			while alerts.alerts.len() > MAX_REBASE_ALERTS {
				let _ = alerts.alerts.pop_front();
			}

			// Save back to storage
			storage_ref.set(&alerts);

			log::info!("🚨 Generated rebase alert at block {:?}", block_number);
			Ok(())
		}

		/// Get current rebase alerts (for RPC or runtime API access)
		pub fn get_rebase_alerts() -> Result<RebaseAlerts<BlockNumberFor<T>>, &'static str> {
			let storage_ref = StorageValueRef::persistent(REBASE_ALERTS_KEY);
			match storage_ref.get::<RebaseAlerts<BlockNumberFor<T>>>() {
				Ok(Some(alerts)) => Ok(alerts),
				_ => Ok(RebaseAlerts::default()),
			}
		}

		/// Get current rebase metadata (for RPC or runtime API access)
		pub fn get_rebase_metadata() -> Result<RebaseMetadata<BlockNumberFor<T>>, &'static str> {
			let current_block = frame_system::Pallet::<T>::block_number();
			Self::load_rebase_metadata(current_block)
		}

		/// Force manual rebase (for governance or emergency use)
		pub fn force_manual_rebase() -> Result<(), &'static str> {
			let current_block = frame_system::Pallet::<T>::block_number();
			let mut metadata = Self::load_rebase_metadata(current_block)?;
			
			Self::trigger_automatic_rebase(current_block, &mut metadata)?;
			Self::save_rebase_metadata(&metadata)?;
			
			log::info!("🆘 Manual rebase forced at block {:?}", current_block);
			Ok(())
		}
	}
}