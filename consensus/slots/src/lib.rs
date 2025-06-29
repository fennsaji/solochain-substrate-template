// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Slots functionality for Substrate.
//!
//! Some consensus algorithms have a concept of *slots*, which are intervals in
//! time during which certain events can and/or must occur.  This crate
//! provides generic functionality for slots.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod aux_schema;
mod slots;

pub use aux_schema::{check_equivocation, MAX_SLOT_CAPACITY, PRUNING_BOUND};
pub use slots::Slots;
pub use slots::{time_until_next_slot, SlotInfo};

// SlotTrigger enum is defined later in this file and will be automatically public

use futures::{future::Either, Future, TryFutureExt};
use futures::prelude::*;
use futures_timer::Delay;
use log::{debug, info, warn};
use sc_consensus::{BlockImport, JustificationSyncLink};
use sc_telemetry::{telemetry, TelemetryHandle, CONSENSUS_DEBUG, CONSENSUS_INFO, CONSENSUS_WARN};
use sp_arithmetic::traits::BaseArithmetic;
use sp_consensus::{Proposal, Proposer, SelectChain, SyncOracle};
use sp_consensus_slots::{Slot, SlotDuration};
use sp_inherents::CreateInherentDataProviders;
use sp_runtime::traits::{Block as BlockT, HashingFor, Header as HeaderT};
use std::sync::Arc;
use std::{
	fmt::Debug,
	ops::Deref,
	time::{Duration, Instant},
};
use tokio::time::{sleep_until, Instant as TokioInstant};
use sc_transaction_pool_api::MaintainedTransactionPool;
use std::time::SystemTime;

const LOG_TARGET: &str = "slots";
const BLOCK_INTERVAL: u64  = 60 * 60; // 1 hour

/// Slot Trigger
pub enum SlotTrigger {
	/// Command to create block
	CreateBlock,
	/// NoAction
	NoAction,
}

/// The changes that need to applied to the storage to create the state for a block.
///
/// See [`sp_state_machine::StorageChanges`] for more information.
pub type StorageChanges<Block> = sp_state_machine::StorageChanges<HashingFor<Block>>;

/// The result of [`SlotWorker::on_slot`].
#[derive(Debug, Clone)]
pub struct SlotResult<Block: BlockT, Proof> {
	/// The block that was built.
	pub block: Block,
	/// The storage proof that was recorded while building the block.
	pub storage_proof: Proof,
}

/// A worker that should be invoked at every new slot.
///
/// The implementation should not make any assumptions of the slot being bound to the time or
/// similar. The only valid assumption is that the slot number is always increasing.
#[async_trait::async_trait]
pub trait SlotWorker<B: BlockT, Proof> {
	/// Called when a new slot is triggered.
	///
	/// Returns a future that resolves to a [`SlotResult`] iff a block was successfully built in
	/// the slot. Otherwise `None` is returned.
	async fn on_slot(&mut self, slot_info: SlotInfo<B>) -> Option<SlotResult<B, Proof>>;
}

/// A skeleton implementation for `SlotWorker` which tries to claim a slot at
/// its beginning and tries to produce a block if successfully claimed, timing
/// out if block production takes too long.
#[async_trait::async_trait]
pub trait SimpleSlotWorker<B: BlockT> {
	/// A handle to a `BlockImport`.
	type BlockImport: BlockImport<B> + Send + 'static;

	/// A handle to a `SyncOracle`.
	type SyncOracle: SyncOracle;

	/// A handle to a `JustificationSyncLink`, allows hooking into the sync module to control the
	/// justification sync process.
	type JustificationSyncLink: JustificationSyncLink<B>;

	/// The type of future resolving to the proposer.
	type CreateProposer: Future<Output = Result<Self::Proposer, sp_consensus::Error>>
		+ Send
		+ Unpin
		+ 'static;

	/// The type of proposer to use to build blocks.
	type Proposer: Proposer<B> + Send;

	/// Data associated with a slot claim.
	type Claim: Send + Sync + 'static;

	/// Auxiliary data necessary for authoring.
	type AuxData: Send + Sync + 'static;

	/// The logging target to use when logging messages.
	fn logging_target(&self) -> &'static str;

	/// A handle to a `BlockImport`.
	fn block_import(&mut self) -> &mut Self::BlockImport;

	/// Returns the auxiliary data necessary for authoring.
	fn aux_data(
		&self,
		header: &B::Header,
		slot: Slot,
	) -> Result<Self::AuxData, sp_consensus::Error>;

	/// Returns the number of authorities.
	/// None indicate that the authorities information is incomplete.
	fn authorities_len(&self, aux_data: &Self::AuxData) -> Option<usize>;

	/// Tries to claim the given slot, returning an object with claim data if successful.
	async fn claim_slot(
		&mut self,
		header: &B::Header,
		slot: Slot,
		aux_data: &Self::AuxData,
	) -> Option<Self::Claim>;

	/// Notifies the given slot. Similar to `claim_slot`, but will be called no matter whether we
	/// need to author blocks or not.
	fn notify_slot(&self, _header: &B::Header, _slot: Slot, _aux_data: &Self::AuxData) {}

	/// Return the pre digest data to include in a block authored with the given claim.
	fn pre_digest_data(&self, slot: Slot, claim: &Self::Claim) -> Vec<sp_runtime::DigestItem>;

	/// Returns a function which produces a `BlockImportParams`.
	async fn block_import_params(
		&self,
		header: B::Header,
		header_hash: &B::Hash,
		body: Vec<B::Extrinsic>,
		storage_changes: StorageChanges<B>,
		public: Self::Claim,
		aux_data: Self::AuxData,
	) -> Result<sc_consensus::BlockImportParams<B>, sp_consensus::Error>;

	/// Whether to force authoring if offline.
	fn force_authoring(&self) -> bool;

	/// Returns whether the block production should back off.
	///
	/// By default this function always returns `false`.
	///
	/// An example strategy that back offs if the finalized head is lagging too much behind the tip
	/// is implemented by [`BackoffAuthoringOnFinalizedHeadLagging`].
	fn should_backoff(&self, _slot: Slot, _chain_head: &B::Header) -> bool {
		false
	}

	/// Returns a handle to a `SyncOracle`.
	fn sync_oracle(&mut self) -> &mut Self::SyncOracle;

	/// Returns a handle to a `JustificationSyncLink`.
	fn justification_sync_link(&mut self) -> &mut Self::JustificationSyncLink;

	/// Returns a `Proposer` to author on top of the given block.
	fn proposer(&mut self, block: &B::Header) -> Self::CreateProposer;

	/// Returns a [`TelemetryHandle`] if any.
	fn telemetry(&self) -> Option<TelemetryHandle>;

	/// Remaining duration for proposing.
	fn proposing_remaining_duration(&self, slot_info: &SlotInfo<B>) -> Duration;

	/// Propose a block by `Proposer`.
	async fn propose(
		&mut self,
		proposer: Self::Proposer,
		claim: &Self::Claim,
		slot_info: SlotInfo<B>,
		end_proposing_at: Instant,
	) -> Option<Proposal<B, <Self::Proposer as Proposer<B>>::Proof>> {
		let slot = slot_info.slot;
		let telemetry = self.telemetry();
		let log_target = self.logging_target();

		let inherent_data =
			Self::create_inherent_data(&slot_info, &log_target, end_proposing_at).await?;

		let proposing_remaining_duration =
			end_proposing_at.saturating_duration_since(Instant::now());
		let logs = self.pre_digest_data(slot, claim);

		// deadline our production to 98% of the total time left for proposing. As we deadline
		// the proposing below to the same total time left, the 2% margin should be enough for
		// the result to be returned.
		let proposing = proposer
			.propose(
				inherent_data,
				sp_runtime::generic::Digest { logs },
				proposing_remaining_duration.mul_f32(0.98),
				slot_info.block_size_limit,
			)
			.map_err(|e| sp_consensus::Error::ClientImport(e.to_string()));

		let proposal = match futures::future::select(
			proposing,
			Delay::new(proposing_remaining_duration),
		)
		.await
		{
			Either::Left((Ok(p), _)) => p,
			Either::Left((Err(err), _)) => {
				warn!(target: log_target, "Proposing failed: {}", err);

				return None
			},
			Either::Right(_) => {
				info!(
					target: log_target,
					"⌛️ Discarding proposal for slot {}; block production took too long", slot,
				);
				// If the node was compiled with debug, tell the user to use release optimizations.
				#[cfg(build_type = "debug")]
				info!(
					target: log_target,
					"👉 Recompile your node in `--release` mode to mitigate this problem.",
				);
				telemetry!(
					telemetry;
					CONSENSUS_INFO;
					"slots.discarding_proposal_took_too_long";
					"slot" => *slot,
				);

				return None
			},
		};

		Some(proposal)
	}

	/// Calls `create_inherent_data` and handles errors.
	async fn create_inherent_data(
		slot_info: &SlotInfo<B>,
		logging_target: &str,
		end_proposing_at: Instant,
	) -> Option<sp_inherents::InherentData> {
		let remaining_duration = end_proposing_at.saturating_duration_since(Instant::now());
		let delay = Delay::new(remaining_duration);
		let cid = slot_info.create_inherent_data.create_inherent_data();
		let inherent_data = match futures::future::select(delay, cid).await {
			Either::Right((Ok(data), _)) => data,
			Either::Right((Err(err), _)) => {
				warn!(
					target: logging_target,
					"Unable to create inherent data for block {:?}: {}",
					slot_info.chain_head.hash(),
					err,
				);

				return None
			},
			Either::Left(_) => {
				warn!(
					target: logging_target,
					"Creating inherent data took more time than we had left for slot {} for block {:?}.",
					slot_info.slot,
					slot_info.chain_head.hash(),
				);

				return None
			},
		};

		Some(inherent_data)
	}

	/// Implements [`SlotWorker::on_slot`].
	async fn on_slot(
		&mut self,
		slot_info: SlotInfo<B>,
	) -> Option<SlotResult<B, <Self::Proposer as Proposer<B>>::Proof>>
	where
		Self: Sync,
	{
		let slot = slot_info.slot;
		let telemetry = self.telemetry();
		let logging_target = self.logging_target();

		let proposing_remaining_duration = self.proposing_remaining_duration(&slot_info);

		let end_proposing_at = if proposing_remaining_duration == Duration::default() {
			debug!(
				target: logging_target,
				"Skipping proposal slot {} since there's no time left to propose", slot,
			);

			return None
		} else {
			Instant::now() + proposing_remaining_duration
		};

		let aux_data = match self.aux_data(&slot_info.chain_head, slot) {
			Ok(aux_data) => aux_data,
			Err(err) => {
				warn!(
					target: logging_target,
					"Unable to fetch auxiliary data for block {:?}: {}",
					slot_info.chain_head.hash(),
					err,
				);

				telemetry!(
					telemetry;
					CONSENSUS_WARN;
					"slots.unable_fetching_authorities";
					"slot" => ?slot_info.chain_head.hash(),
					"err" => ?err,
				);

				return None
			},
		};

		self.notify_slot(&slot_info.chain_head, slot, &aux_data);

		let authorities_len = self.authorities_len(&aux_data);

		if !self.force_authoring() &&
			self.sync_oracle().is_offline() &&
			authorities_len.map(|a| a > 1).unwrap_or(false)
		{
			debug!(target: logging_target, "Skipping proposal slot. Waiting for the network.");
			telemetry!(
				telemetry;
				CONSENSUS_DEBUG;
				"slots.skipping_proposal_slot";
				"authorities_len" => authorities_len,
			);

			return None
		}

		let claim = self.claim_slot(&slot_info.chain_head, slot, &aux_data).await?;

		if self.should_backoff(slot, &slot_info.chain_head) {
			return None
		}

		debug!(target: logging_target, "Starting authorship at slot: {slot}");

		telemetry!(telemetry; CONSENSUS_DEBUG; "slots.starting_authorship"; "slot_num" => slot);

		let proposer = match self.proposer(&slot_info.chain_head).await {
			Ok(p) => p,
			Err(err) => {
				warn!(target: logging_target, "Unable to author block in slot {slot:?}: {err}");

				telemetry!(
					telemetry;
					CONSENSUS_WARN;
					"slots.unable_authoring_block";
					"slot" => *slot,
					"err" => ?err
				);

				return None
			},
		};

		let proposal = self.propose(proposer, &claim, slot_info, end_proposing_at).await?;

		let (block, storage_proof) = (proposal.block, proposal.proof);
		let (header, body) = block.deconstruct();
		let header_num = *header.number();
		let header_hash = header.hash();
		let parent_hash = *header.parent_hash();

		let block_import_params = match self
			.block_import_params(
				header,
				&header_hash,
				body.clone(),
				proposal.storage_changes,
				claim,
				aux_data,
			)
			.await
		{
			Ok(bi) => bi,
			Err(err) => {
				warn!(target: logging_target, "Failed to create block import params: {}", err);

				return None
			},
		};

		info!(
			target: logging_target,
			"🔖 Pre-sealed block for proposal at {}. Hash now {:?}, previously {:?}.",
			header_num,
			block_import_params.post_hash(),
			header_hash,
		);

		telemetry!(
			telemetry;
			CONSENSUS_INFO;
			"slots.pre_sealed_block";
			"header_num" => ?header_num,
			"hash_now" => ?block_import_params.post_hash(),
			"hash_previously" => ?header_hash,
		);

		let header = block_import_params.post_header();
		match self.block_import().import_block(block_import_params).await {
			Ok(res) => {
				res.handle_justification(
					&header.hash(),
					*header.number(),
					self.justification_sync_link(),
				);
			},
			Err(err) => {
				warn!(
					target: logging_target,
					"Error with block built on {:?}: {}", parent_hash, err,
				);

				telemetry!(
					telemetry;
					CONSENSUS_WARN;
					"slots.err_with_block_built_on";
					"hash" => ?parent_hash,
					"err" => ?err,
				);
			},
		}

		Some(SlotResult { block: B::new(header, body), storage_proof })
	}
}

/// A type that implements [`SlotWorker`] for a type that implements [`SimpleSlotWorker`].
///
/// This is basically a workaround for Rust not supporting specialization. Otherwise we could
/// implement [`SlotWorker`] for any `T` that implements [`SimpleSlotWorker`], but currently
/// that would prevent downstream users to implement [`SlotWorker`] for their own types.
pub struct SimpleSlotWorkerToSlotWorker<T>(pub T);

#[async_trait::async_trait]
impl<T: SimpleSlotWorker<B> + Send + Sync, B: BlockT>
	SlotWorker<B, <T::Proposer as Proposer<B>>::Proof> for SimpleSlotWorkerToSlotWorker<T>
{
	async fn on_slot(
		&mut self,
		slot_info: SlotInfo<B>,
	) -> Option<SlotResult<B, <T::Proposer as Proposer<B>>::Proof>> {
		self.0.on_slot(slot_info).await
	}
}

/// Slot specific extension that the inherent data provider needs to implement.
pub trait InherentDataProviderExt {
	/// The current slot that will be found in the [`InherentData`](`sp_inherents::InherentData`).
	fn slot(&self) -> Slot;
}

/// Small macro for implementing `InherentDataProviderExt` for inherent data provider tuple.
macro_rules! impl_inherent_data_provider_ext_tuple {
	( S $(, $TN:ident)* $( , )?) => {
		impl<S, $( $TN ),*>  InherentDataProviderExt for (S, $($TN),*)
		where
			S: Deref<Target = Slot>,
		{
			fn slot(&self) -> Slot {
				*self.0.deref()
			}
		}
	}
}

impl_inherent_data_provider_ext_tuple!(S);
impl_inherent_data_provider_ext_tuple!(S, A);
impl_inherent_data_provider_ext_tuple!(S, A, B);
impl_inherent_data_provider_ext_tuple!(S, A, B, C);
impl_inherent_data_provider_ext_tuple!(S, A, B, C, D);
impl_inherent_data_provider_ext_tuple!(S, A, B, C, D, E);
impl_inherent_data_provider_ext_tuple!(S, A, B, C, D, E, F);
impl_inherent_data_provider_ext_tuple!(S, A, B, C, D, E, F, G);
impl_inherent_data_provider_ext_tuple!(S, A, B, C, D, E, F, G, H);
impl_inherent_data_provider_ext_tuple!(S, A, B, C, D, E, F, G, H, I);
impl_inherent_data_provider_ext_tuple!(S, A, B, C, D, E, F, G, H, I, J);

/// Start a new slot worker.
///
/// Every time a new slot is triggered, `worker.on_slot` is called and the future it returns is
/// polled until completion, unless we are major syncing.
pub async fn start_slot_worker<B, C, W, SO, CIDP, Proof>(
	slot_duration: SlotDuration,
	client: C,
	mut worker: W,
	sync_oracle: SO,
	create_inherent_data_providers: CIDP,
) where
	B: BlockT,
	C: SelectChain<B>,
	W: SlotWorker<B, Proof>,
	SO: SyncOracle + Send,
	CIDP: CreateInherentDataProviders<B, ()> + Send + 'static,
	CIDP::InherentDataProviders: InherentDataProviderExt + Send,
{
	let mut slots = Slots::new(
		slot_duration.as_duration(),
		create_inherent_data_providers,
		client,
		sync_oracle,
	);

	loop {
		let slot_info = slots.next_slot().await;
		let _ = worker.on_slot(slot_info).await;
	}
}

/// Start a new slot worker.
pub async fn start_slot_worker_v2<B, C, W, SO, CIDP, Proof, CS>(
	slot_duration: SlotDuration,
	client: C,
	mut worker: W,
	sync_oracle: SO,
	create_inherent_data_providers: CIDP,
    mut command_stream: CS, // Create Block stream
) where
	B: BlockT,
	C: SelectChain<B>,
	W: SlotWorker<B, Proof>,
	SO: SyncOracle + Send,
	CIDP: CreateInherentDataProviders<B, ()> + Send + 'static,
	CIDP::InherentDataProviders: InherentDataProviderExt + Send,
	CS: Stream<Item = SlotTrigger> + Unpin + 'static, // Adjust the stream type as necessary
{
	let mut slots = Slots::new(
		slot_duration.as_duration(),
		create_inherent_data_providers,
		client,
		sync_oracle,
	);
	
	let one_hour = Duration::from_secs(BLOCK_INTERVAL);
	let mut slot_interval = tokio::time::interval_at(TokioInstant::now() + one_hour, one_hour); // Start after first hour, not immediately

	loop {
		tokio::select! {
			// Process command stream for slot triggers
			slot_trigger_opt = command_stream.next() => {
				match slot_trigger_opt {
					Some(SlotTrigger::CreateBlock) => {
						info!("New block creation request received from trigger stream");
						let slot_info = slots.next_slot_v2().await;

						info!("Creating block for trigger stream slot: {}", slot_info.slot);
						let _ = worker.on_slot(slot_info).await;

						// Reset the interval after a command stream trigger
						slot_interval.reset();
					},
					Some(_) => {
						// Other trigger types - do nothing for now
					},
					None => {
						// Stream ended - this should not happen with a properly implemented stream
						// but we'll log it and continue with interval-based processing
						warn!("Event-driven command stream ended, falling back to interval-based block production");
						
						// Continue the loop - we'll only process the interval branch now
						continue;
					}
				}
			},

			// Run on_slot every hour if no command_stream triggers reset it
			_ = slot_interval.tick() => {
				info!("Interval block creation triggered");
				let slot_info = slots.next_slot_v2().await;

				info!("Creating block for interval slot: {}", slot_info.slot);
				let _ = worker.on_slot(slot_info).await;
			},
		}
	}
}


/// Start a new slot worker.
pub async fn start_slot_worker_v3<B, C, W, SO, CIDP, Proof, TExPool>(
	slot_duration: SlotDuration,
	client: C,
	mut worker: W,
	sync_oracle: SO,
	create_inherent_data_providers: CIDP,
    pool: Arc<TExPool>,
) where
	B: BlockT,
	C: SelectChain<B>,
	W: SlotWorker<B, Proof>,
	SO: SyncOracle + Send,
	CIDP: CreateInherentDataProviders<B, ()> + Send + 'static,
	CIDP::InherentDataProviders: InherentDataProviderExt + Send,
    TExPool: MaintainedTransactionPool<Block = B, Hash = <B as BlockT>::Hash> + 'static,
{
	let mut slots = Slots::new(
		slot_duration.as_duration(),
		create_inherent_data_providers,
		client,
		sync_oracle,
	);
	// Calculate the initial next full hour based on system time
	let hourly_trigger_seconds = BLOCK_INTERVAL;
	let mut next_hourly_trigger = next_time_boundary(Duration::from_secs(hourly_trigger_seconds));
	let transaction_check_ms= 100;
	let mut next_100ms_trigger = next_time_boundary(Duration::from_millis(transaction_check_ms));

	loop {

		tokio::select! {
			// Transaction pool check
			_ = sleep_until(next_100ms_trigger)  => {
				let pool_status = pool.status();
				if pool_status.ready > 0 {
					let slot_info = slots.next_slot().await;
					info!(target: LOG_TARGET, "Transaction pool check triggered with {} ready transactions", pool_status.ready);
					let _ = worker.on_slot(slot_info).await;
					 next_hourly_trigger = next_time_boundary(Duration::from_secs(hourly_trigger_seconds)); // Reset the hourly trigger after a transaction-based trigger
				}
				next_100ms_trigger = next_time_boundary(Duration::from_millis(transaction_check_ms)); // Schedule next 100ms interval trigger
			},

			// Hourly trigger at exact hours
			_ = sleep_until(next_hourly_trigger) => {
				let slot_info = slots.next_slot().await;
				info!(target: LOG_TARGET, "Hourly block creation triggered at top of the hour");
				let _ = worker.on_slot(slot_info).await;
				 next_hourly_trigger = next_time_boundary(Duration::from_secs(hourly_trigger_seconds)); // Schedule next exact hour trigger
			},
		}
	}
}

/// Helper function to calculate the next time boundary
fn next_time_boundary(interval: Duration) -> TokioInstant {
    let now = SystemTime::now();
    let millis_since_epoch = now.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();
    let interval_ms = interval.as_millis();
    let next_boundary_ms = interval_ms - (millis_since_epoch % interval_ms);
    TokioInstant::now() + Duration::from_millis(next_boundary_ms as u64)
}


/// A header which has been checked
pub enum CheckedHeader<H, S> {
	/// A header which has slot in the future. this is the full header (not stripped)
	/// and the slot in which it should be processed.
	Deferred(H, Slot),
	/// A header which is fully checked, including signature. This is the pre-header
	/// accompanied by the seal components.
	///
	/// Includes the digest item that encoded the seal.
	Checked(H, S),
}

/// A unit type wrapper to express the proportion of a slot.
pub struct SlotProportion(f32);

impl SlotProportion {
	/// Create a new proportion.
	///
	/// The given value `inner` should be in the range `[0,1]`. If the value is not in the required
	/// range, it is clamped into the range.
	pub fn new(inner: f32) -> Self {
		Self(inner.clamp(0.0, 1.0))
	}

	/// Returns the inner that is guaranteed to be in the range `[0,1]`.
	pub fn get(&self) -> f32 {
		self.0
	}
}

/// The strategy used to calculate the slot lenience used to increase the block proposal time when
/// slots have been skipped with no blocks authored.
pub enum SlotLenienceType {
	/// Increase the lenience linearly with the number of skipped slots.
	Linear,
	/// Increase the lenience exponentially with the number of skipped slots.
	Exponential,
}

impl SlotLenienceType {
	fn as_str(&self) -> &'static str {
		match self {
			SlotLenienceType::Linear => "linear",
			SlotLenienceType::Exponential => "exponential",
		}
	}
}

/// Calculate the remaining duration for block proposal taking into account whether any slots have
/// been skipped and applying the given lenience strategy. If `max_block_proposal_slot_portion` is
/// not none this method guarantees that the returned duration must be lower or equal to
/// `slot_info.duration * max_block_proposal_slot_portion`.
pub fn proposing_remaining_duration<Block: BlockT>(
	parent_slot: Option<Slot>,
	slot_info: &SlotInfo<Block>,
	block_proposal_slot_portion: &SlotProportion,
	max_block_proposal_slot_portion: Option<&SlotProportion>,
	slot_lenience_type: SlotLenienceType,
	log_target: &str,
) -> Duration {
	use sp_runtime::traits::Zero;

	let proposing_duration = slot_info.duration.mul_f32(block_proposal_slot_portion.get());

	let slot_remaining = slot_info
		.ends_at
		.checked_duration_since(std::time::Instant::now())
		.unwrap_or_default();

	let proposing_duration = std::cmp::min(slot_remaining, proposing_duration);

	// If parent is genesis block, we don't require any lenience factor.
	if slot_info.chain_head.number().is_zero() {
		return proposing_duration
	}

	let parent_slot = match parent_slot {
		Some(parent_slot) => parent_slot,
		None => return proposing_duration,
	};

	let slot_lenience = match slot_lenience_type {
		SlotLenienceType::Exponential => slot_lenience_exponential(parent_slot, slot_info),
		SlotLenienceType::Linear => slot_lenience_linear(parent_slot, slot_info),
	};

	if let Some(slot_lenience) = slot_lenience {
		let lenient_proposing_duration =
			proposing_duration + slot_lenience.mul_f32(block_proposal_slot_portion.get());

		// if we defined a maximum portion of the slot for proposal then we must make sure the
		// lenience doesn't go over it
		let lenient_proposing_duration =
			if let Some(max_block_proposal_slot_portion) = max_block_proposal_slot_portion {
				std::cmp::min(
					lenient_proposing_duration,
					slot_info.duration.mul_f32(max_block_proposal_slot_portion.get()),
				)
			} else {
				lenient_proposing_duration
			};

		debug!(
			target: log_target,
			"No block for {} slots. Applying {} lenience, total proposing duration: {}ms",
			slot_info.slot.saturating_sub(parent_slot + 1),
			slot_lenience_type.as_str(),
			lenient_proposing_duration.as_millis(),
		);

		lenient_proposing_duration
	} else {
		proposing_duration
	}
}

/// Calculate a slot duration lenience based on the number of missed slots from current
/// to parent. If the number of skipped slots is greater than 0 this method will apply
/// an exponential backoff of at most `2^7 * slot_duration`, if no slots were skipped
/// this method will return `None.`
pub fn slot_lenience_exponential<Block: BlockT>(
	parent_slot: Slot,
	slot_info: &SlotInfo<Block>,
) -> Option<Duration> {
	// never give more than 2^this times the lenience.
	const BACKOFF_CAP: u64 = 7;

	// how many slots it takes before we double the lenience.
	const BACKOFF_STEP: u64 = 2;

	// we allow a lenience of the number of slots since the head of the
	// chain was produced, minus 1 (since there is always a difference of at least 1)
	//
	// exponential back-off.
	// in normal cases we only attempt to issue blocks up to the end of the slot.
	// when the chain has been stalled for a few slots, we give more lenience.
	let skipped_slots = *slot_info.slot.saturating_sub(parent_slot + 1);

	if skipped_slots == 0 {
		None
	} else {
		let slot_lenience = skipped_slots / BACKOFF_STEP;
		let slot_lenience = std::cmp::min(slot_lenience, BACKOFF_CAP);
		let slot_lenience = 1 << slot_lenience;
		Some(slot_lenience * slot_info.duration)
	}
}

/// Calculate a slot duration lenience based on the number of missed slots from current
/// to parent. If the number of skipped slots is greater than 0 this method will apply
/// a linear backoff of at most `20 * slot_duration`, if no slots were skipped
/// this method will return `None.`
pub fn slot_lenience_linear<Block: BlockT>(
	parent_slot: Slot,
	slot_info: &SlotInfo<Block>,
) -> Option<Duration> {
	// never give more than 20 times more lenience.
	const BACKOFF_CAP: u64 = 20;

	// we allow a lenience of the number of slots since the head of the
	// chain was produced, minus 1 (since there is always a difference of at least 1)
	//
	// linear back-off.
	// in normal cases we only attempt to issue blocks up to the end of the slot.
	// when the chain has been stalled for a few slots, we give more lenience.
	let skipped_slots = *slot_info.slot.saturating_sub(parent_slot + 1);

	if skipped_slots == 0 {
		None
	} else {
		let slot_lenience = std::cmp::min(skipped_slots, BACKOFF_CAP);
		// We cap `slot_lenience` to `20`, so it should always fit into an `u32`.
		Some(slot_info.duration * (slot_lenience as u32))
	}
}

/// Trait for providing the strategy for when to backoff block authoring.
pub trait BackoffAuthoringBlocksStrategy<N> {
	/// Returns true if we should backoff authoring new blocks.
	fn should_backoff(
		&self,
		chain_head_number: N,
		chain_head_slot: Slot,
		finalized_number: N,
		slow_now: Slot,
		logging_target: &str,
	) -> bool;
}

/// A simple default strategy for how to decide backing off authoring blocks if the number of
/// unfinalized blocks grows too large.
#[derive(Clone)]
pub struct BackoffAuthoringOnFinalizedHeadLagging<N> {
	/// The max interval to backoff when authoring blocks, regardless of delay in finality.
	pub max_interval: N,
	/// The number of unfinalized blocks allowed before starting to consider to backoff authoring
	/// blocks. Note that depending on the value for `authoring_bias`, there might still be an
	/// additional wait until block authorship starts getting declined.
	pub unfinalized_slack: N,
	/// Scales the backoff rate. A higher value effectively means we backoff slower, taking longer
	/// time to reach the maximum backoff as the unfinalized head of chain grows.
	pub authoring_bias: N,
}

/// These parameters is supposed to be some form of sensible defaults.
impl<N: BaseArithmetic> Default for BackoffAuthoringOnFinalizedHeadLagging<N> {
	fn default() -> Self {
		Self {
			// Never wait more than 100 slots before authoring blocks, regardless of delay in
			// finality.
			max_interval: 100.into(),
			// Start to consider backing off block authorship once we have 50 or more unfinalized
			// blocks at the head of the chain.
			unfinalized_slack: 50.into(),
			// A reasonable default for the authoring bias, or reciprocal interval scaling, is 2.
			// Effectively meaning that consider the unfinalized head suffix length to grow half as
			// fast as in actuality.
			authoring_bias: 2.into(),
		}
	}
}

impl<N> BackoffAuthoringBlocksStrategy<N> for BackoffAuthoringOnFinalizedHeadLagging<N>
where
	N: BaseArithmetic + Copy,
{
	fn should_backoff(
		&self,
		chain_head_number: N,
		chain_head_slot: Slot,
		finalized_number: N,
		slot_now: Slot,
		logging_target: &str,
	) -> bool {
		// This should not happen, but we want to keep the previous behaviour if it does.
		if slot_now <= chain_head_slot {
			return false
		}

		// There can be race between getting the finalized number and getting the best number.
		// So, better be safe than sorry.
		let unfinalized_block_length = chain_head_number.saturating_sub(finalized_number);
		let interval =
			unfinalized_block_length.saturating_sub(self.unfinalized_slack) / self.authoring_bias;
		let interval = interval.min(self.max_interval);

		// We're doing arithmetic between block and slot numbers.
		let interval: u64 = interval.unique_saturated_into();

		// If interval is nonzero we backoff if the current slot isn't far enough ahead of the chain
		// head.
		if *slot_now <= *chain_head_slot + interval {
			info!(
				target: logging_target,
				"Backing off claiming new slot for block authorship: finality is lagging.",
			);
			true
		} else {
			false
		}
	}
}

impl<N> BackoffAuthoringBlocksStrategy<N> for () {
	fn should_backoff(
		&self,
		_chain_head_number: N,
		_chain_head_slot: Slot,
		_finalized_number: N,
		_slot_now: Slot,
		_logging_target: &str,
	) -> bool {
		false
	}
}


#[cfg(test)]
mod test {
	use super::*;
	use sp_runtime::traits::NumberFor;
	use std::time::{Duration, Instant};
	use substrate_test_runtime_client::runtime::{Block, Header};

	const SLOT_DURATION: Duration = Duration::from_millis(6000);

	fn slot(slot: u64) -> super::slots::SlotInfo<Block> {
		super::slots::SlotInfo {
			slot: slot.into(),
			duration: SLOT_DURATION,
			create_inherent_data: Box::new(()),
			ends_at: Instant::now() + SLOT_DURATION,
			chain_head: Header::new(
				1,
				Default::default(),
				Default::default(),
				Default::default(),
				Default::default(),
			),
			block_size_limit: None,
		}
	}

	#[test]
	fn linear_slot_lenience() {
		// if no slots are skipped there should be no lenience
		assert_eq!(super::slot_lenience_linear(1u64.into(), &slot(2)), None);

		// otherwise the lenience is incremented linearly with
		// the number of skipped slots.
		for n in 3..=22 {
			assert_eq!(
				super::slot_lenience_linear(1u64.into(), &slot(n)),
				Some(SLOT_DURATION * (n - 2) as u32),
			);
		}

		// but we cap it to a maximum of 20 slots
		assert_eq!(super::slot_lenience_linear(1u64.into(), &slot(23)), Some(SLOT_DURATION * 20));
	}

	#[test]
	fn exponential_slot_lenience() {
		// if no slots are skipped there should be no lenience
		assert_eq!(super::slot_lenience_exponential(1u64.into(), &slot(2)), None);

		// otherwise the lenience is incremented exponentially every two slots
		for n in 3..=17 {
			assert_eq!(
				super::slot_lenience_exponential(1u64.into(), &slot(n)),
				Some(SLOT_DURATION * 2u32.pow((n / 2 - 1) as u32)),
			);
		}

		// but we cap it to a maximum of 14 slots
		assert_eq!(
			super::slot_lenience_exponential(1u64.into(), &slot(18)),
			Some(SLOT_DURATION * 2u32.pow(7)),
		);

		assert_eq!(
			super::slot_lenience_exponential(1u64.into(), &slot(19)),
			Some(SLOT_DURATION * 2u32.pow(7)),
		);
	}

	#[test]
	fn proposing_remaining_duration_should_apply_lenience_based_on_proposal_slot_proportion() {
		assert_eq!(
			proposing_remaining_duration(
				Some(0.into()),
				&slot(2),
				&SlotProportion(0.25),
				None,
				SlotLenienceType::Linear,
				"test",
			),
			SLOT_DURATION.mul_f32(0.25 * 2.0),
		);
	}

	#[test]
	fn proposing_remaining_duration_should_never_exceed_max_proposal_slot_proportion() {
		assert_eq!(
			proposing_remaining_duration(
				Some(0.into()),
				&slot(100),
				&SlotProportion(0.25),
				Some(SlotProportion(0.9)).as_ref(),
				SlotLenienceType::Exponential,
				"test",
			),
			SLOT_DURATION.mul_f32(0.9),
		);
	}

	#[derive(PartialEq, Debug)]
	struct HeadState {
		head_number: NumberFor<Block>,
		head_slot: u64,
		slot_now: NumberFor<Block>,
	}

	impl HeadState {
		fn author_block(&mut self) {
			// Add a block to the head, and set latest slot to the current
			self.head_number += 1;
			self.head_slot = self.slot_now;
			// Advance slot to next
			self.slot_now += 1;
		}

		fn dont_author_block(&mut self) {
			self.slot_now += 1;
		}
	}

	#[test]
	fn should_never_backoff_when_head_not_advancing() {
		let strategy = BackoffAuthoringOnFinalizedHeadLagging::<NumberFor<Block>> {
			max_interval: 100,
			unfinalized_slack: 5,
			authoring_bias: 2,
		};

		let head_number = 1;
		let head_slot = 1;
		let finalized_number = 1;
		let slot_now = 2;

		let should_backoff: Vec<bool> = (slot_now..1000)
			.map(|s| {
				strategy.should_backoff(
					head_number,
					head_slot.into(),
					finalized_number,
					s.into(),
					"slots",
				)
			})
			.collect();

		// Should always be false, since the head isn't advancing
		let expected: Vec<bool> = (slot_now..1000).map(|_| false).collect();
		assert_eq!(should_backoff, expected);
	}

	#[test]
	fn should_stop_authoring_if_blocks_are_still_produced_when_finality_stalled() {
		let strategy = BackoffAuthoringOnFinalizedHeadLagging::<NumberFor<Block>> {
			max_interval: 100,
			unfinalized_slack: 5,
			authoring_bias: 2,
		};

		let mut head_number = 1;
		let mut head_slot = 1;
		let finalized_number = 1;
		let slot_now = 2;

		let should_backoff: Vec<bool> = (slot_now..300)
			.map(move |s| {
				let b = strategy.should_backoff(
					head_number,
					head_slot.into(),
					finalized_number,
					s.into(),
					"slots",
				);
				// Chain is still advancing (by someone else)
				head_number += 1;
				head_slot = s;
				b
			})
			.collect();

		// Should always be true after a short while, since the chain is advancing but finality is
		// stalled
		let expected: Vec<bool> = (slot_now..300).map(|s| s > 8).collect();
		assert_eq!(should_backoff, expected);
	}

	#[test]
	fn should_never_backoff_if_max_interval_is_reached() {
		let strategy = BackoffAuthoringOnFinalizedHeadLagging::<NumberFor<Block>> {
			max_interval: 100,
			unfinalized_slack: 5,
			authoring_bias: 2,
		};

		// The limit `max_interval` is used when the unfinalized chain grows to
		// 	`max_interval * authoring_bias + unfinalized_slack`,
		// which for the above parameters becomes
		// 	100 * 2 + 5 = 205.
		// Hence we trigger this with head_number > finalized_number + 205.
		let head_number = 207;
		let finalized_number = 1;

		// The limit is then used once the current slot is `max_interval` ahead of slot of the head.
		let head_slot = 1;
		let slot_now = 2;
		let max_interval = strategy.max_interval;

		let should_backoff: Vec<bool> = (slot_now..200)
			.map(|s| {
				strategy.should_backoff(
					head_number,
					head_slot.into(),
					finalized_number,
					s.into(),
					"slots",
				)
			})
			.collect();

		// Should backoff (true) until we are `max_interval` number of slots ahead of the chain
		// head slot, then we never backoff (false).
		let expected: Vec<bool> = (slot_now..200).map(|s| s <= max_interval + head_slot).collect();
		assert_eq!(should_backoff, expected);
	}

	#[test]
	fn should_backoff_authoring_when_finality_stalled() {
		let param = BackoffAuthoringOnFinalizedHeadLagging {
			max_interval: 100,
			unfinalized_slack: 5,
			authoring_bias: 2,
		};

		let finalized_number = 2;
		let mut head_state = HeadState { head_number: 4, head_slot: 10, slot_now: 11 };

		let should_backoff = |head_state: &HeadState| -> bool {
			<dyn BackoffAuthoringBlocksStrategy<NumberFor<Block>>>::should_backoff(
				&param,
				head_state.head_number,
				head_state.head_slot.into(),
				finalized_number,
				head_state.slot_now.into(),
				"slots",
			)
		};

		let backoff: Vec<bool> = (head_state.slot_now..200)
			.map(|_| {
				if should_backoff(&head_state) {
					head_state.dont_author_block();
					true
				} else {
					head_state.author_block();
					false
				}
			})
			.collect();

		// Gradually start to backoff more and more frequently
		let expected = [
			false, false, false, false, false, // no effect
			true, false, true, false, // 1:1
			true, true, false, true, true, false, // 2:1
			true, true, true, false, true, true, true, false, // 3:1
			true, true, true, true, false, true, true, true, true, false, // 4:1
			true, true, true, true, true, false, true, true, true, true, true, false, // 5:1
			true, true, true, true, true, true, false, true, true, true, true, true, true,
			false, // 6:1
			true, true, true, true, true, true, true, false, true, true, true, true, true, true,
			true, false, // 7:1
			true, true, true, true, true, true, true, true, false, true, true, true, true, true,
			true, true, true, false, // 8:1
			true, true, true, true, true, true, true, true, true, false, true, true, true, true,
			true, true, true, true, true, false, // 9:1
			true, true, true, true, true, true, true, true, true, true, false, true, true, true,
			true, true, true, true, true, true, true, false, // 10:1
			true, true, true, true, true, true, true, true, true, true, true, false, true, true,
			true, true, true, true, true, true, true, true, true, false, // 11:1
			true, true, true, true, true, true, true, true, true, true, true, true, false, true,
			true, true, true, true, true, true, true, true, true, true, true, false, // 12:1
			true, true, true, true,
		];

		assert_eq!(backoff.as_slice(), &expected[..]);
	}

	#[test]
	fn should_never_wait_more_than_max_interval() {
		let param = BackoffAuthoringOnFinalizedHeadLagging {
			max_interval: 100,
			unfinalized_slack: 5,
			authoring_bias: 2,
		};

		let finalized_number = 2;
		let starting_slot = 11;
		let mut head_state = HeadState { head_number: 4, head_slot: 10, slot_now: starting_slot };

		let should_backoff = |head_state: &HeadState| -> bool {
			<dyn BackoffAuthoringBlocksStrategy<NumberFor<Block>>>::should_backoff(
				&param,
				head_state.head_number,
				head_state.head_slot.into(),
				finalized_number,
				head_state.slot_now.into(),
				"slots",
			)
		};

		let backoff: Vec<bool> = (head_state.slot_now..40000)
			.map(|_| {
				if should_backoff(&head_state) {
					head_state.dont_author_block();
					true
				} else {
					head_state.author_block();
					false
				}
			})
			.collect();

		let slots_claimed: Vec<usize> = backoff
			.iter()
			.enumerate()
			.filter(|&(_i, x)| x == &false)
			.map(|(i, _x)| i + starting_slot as usize)
			.collect();

		let last_slot = backoff.len() + starting_slot as usize;
		let mut last_two_claimed = slots_claimed.iter().rev().take(2);

		// Check that we claimed all the way to the end. Check two slots for when we have an uneven
		// number of slots_claimed.
		let expected_distance = param.max_interval as usize + 1;
		assert_eq!(last_slot - last_two_claimed.next().unwrap(), 92);
		assert_eq!(last_slot - last_two_claimed.next().unwrap(), 92 + expected_distance);

		let intervals: Vec<_> = slots_claimed.windows(2).map(|x| x[1] - x[0]).collect();

		// The key thing is that the distance between claimed slots is capped to `max_interval + 1`
		// assert_eq!(max_observed_interval, Some(&expected_distance));
		assert_eq!(intervals.iter().max(), Some(&expected_distance));

		// But lets assert all distances, which we expect to grow linearly until `max_interval + 1`
		let expected_intervals: Vec<_> =
			(0..497).map(|i| (i / 2).clamp(1, expected_distance)).collect();

		assert_eq!(intervals, expected_intervals);
	}

	fn run_until_max_interval(param: BackoffAuthoringOnFinalizedHeadLagging<u64>) -> (u64, u64) {
		let finalized_number = 0;
		let mut head_state = HeadState { head_number: 0, head_slot: 0, slot_now: 1 };

		let should_backoff = |head_state: &HeadState| -> bool {
			<dyn BackoffAuthoringBlocksStrategy<NumberFor<Block>>>::should_backoff(
				&param,
				head_state.head_number,
				head_state.head_slot.into(),
				finalized_number,
				head_state.slot_now.into(),
				"slots",
			)
		};

		// Number of blocks until we reach the max interval
		let block_for_max_interval =
			param.max_interval * param.authoring_bias + param.unfinalized_slack;

		while head_state.head_number < block_for_max_interval {
			if should_backoff(&head_state) {
				head_state.dont_author_block();
			} else {
				head_state.author_block();
			}
		}

		let slot_time = 6;
		let time_to_reach_limit = slot_time * head_state.slot_now;
		(block_for_max_interval, time_to_reach_limit)
	}

	// Denoting
	// 	C: unfinalized_slack
	// 	M: authoring_bias
	// 	X: max_interval
	// then the number of slots to reach the max interval can be computed from
	// 	(start_slot + C) + M * sum(n, 1, X)
	// or
	// 	(start_slot + C) + M * X*(X+1)/2
	fn expected_time_to_reach_max_interval(
		param: &BackoffAuthoringOnFinalizedHeadLagging<u64>,
	) -> (u64, u64) {
		let c = param.unfinalized_slack;
		let m = param.authoring_bias;
		let x = param.max_interval;
		let slot_time = 6;

		let block_for_max_interval = x * m + c;

		// The 1 is because we start at slot_now = 1.
		let expected_number_of_slots = (1 + c) + m * x * (x + 1) / 2;
		let time_to_reach = expected_number_of_slots * slot_time;

		(block_for_max_interval, time_to_reach)
	}

	#[test]
	fn time_to_reach_upper_bound_for_smaller_slack() {
		let param = BackoffAuthoringOnFinalizedHeadLagging {
			max_interval: 100,
			unfinalized_slack: 5,
			authoring_bias: 2,
		};
		let expected = expected_time_to_reach_max_interval(&param);
		let (block_for_max_interval, time_to_reach_limit) = run_until_max_interval(param);
		assert_eq!((block_for_max_interval, time_to_reach_limit), expected);
		// Note: 16 hours is 57600 sec
		assert_eq!((block_for_max_interval, time_to_reach_limit), (205, 60636));
	}

	#[test]
	fn time_to_reach_upper_bound_for_larger_slack() {
		let param = BackoffAuthoringOnFinalizedHeadLagging {
			max_interval: 100,
			unfinalized_slack: 50,
			authoring_bias: 2,
		};
		let expected = expected_time_to_reach_max_interval(&param);
		let (block_for_max_interval, time_to_reach_limit) = run_until_max_interval(param);
		assert_eq!((block_for_max_interval, time_to_reach_limit), expected);
		assert_eq!((block_for_max_interval, time_to_reach_limit), (250, 60906));
	}
}
