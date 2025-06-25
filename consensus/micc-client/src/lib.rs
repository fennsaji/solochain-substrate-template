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

//! Micc (Authority-round) consensus in substrate.
//!
//! Micc works by having a list of authorities A who are expected to roughly
//! agree on the current time. Time is divided up into discrete slots of t
//! seconds each. For each slot s, the author of that slot is A[s % |A|].
//!
//! The author is allowed to issue one block but not more during that slot,
//! and it will be built upon the longest valid chain that has been seen.
//!
//! Blocks from future steps will be either deferred or rejected depending on how
//! far in the future they are.
//!
//! NOTE: Micc itself is designed to be generic over the crypto used.
#![forbid(missing_docs, unsafe_code)]
use std::{fmt::Debug, marker::PhantomData, pin::Pin, sync::Arc, time::Duration};

use codec::Codec;
use futures::prelude::*;

use log::info;
use sc_client_api::{backend::AuxStore, BlockOf};
use sc_consensus::{BlockImport, BlockImportParams, ForkChoiceStrategy, StateAction};
use sc_consensus_slots::{
	BackoffAuthoringBlocksStrategy, InherentDataProviderExt, SimpleSlotWorkerToSlotWorker, SlotInfo, SlotTrigger, StorageChanges
};
use sc_telemetry::TelemetryHandle;
use sp_api::{Core, ProvideRuntimeApi};
use sp_application_crypto::{AppPublic, ByteArray};
use sp_blockchain::HeaderBackend;
use sp_consensus::{BlockOrigin, Environment, Error as ConsensusError, Proposer, SelectChain};
use sp_consensus_slots::Slot;
use sp_core::crypto::Pair;
use sp_inherents::CreateInherentDataProviders;
use sp_keystore::KeystorePtr;
use sp_runtime::traits::{Block as BlockT, Header, Member, NumberFor};
use sc_transaction_pool_api::MaintainedTransactionPool;
use futures::StreamExt;
use tokio_stream::wrappers::IntervalStream;
use tokio::time;

mod import_queue;
pub mod standalone;
pub mod event_driven;

pub use crate::standalone::{find_pre_digest, slot_duration};
pub use import_queue::{
	build_verifier, import_queue, BuildVerifierParams, CheckForEquivocation, ImportQueueParams,
	MiccVerifier,
};
pub use sc_consensus_slots::SlotProportion;
pub use sp_consensus::SyncOracle;
pub use sp_consensus_micc::{
	digests::CompatibleDigestItem,
	inherents::{InherentDataProvider, InherentType as MiccInherent, INHERENT_IDENTIFIER},
	ConsensusLog, MiccApi, SlotDuration, MICC_ENGINE_ID,
};

const LOG_TARGET: &str = "micc";

type AuthorityId<P> = <P as Pair>::Public;

/// Run `MICC` in a compatibility mode.
///
/// This is required for when the chain was launched and later there
/// was a consensus breaking change.
#[derive(Debug, Clone)]
pub enum CompatibilityMode<N> {
	/// Don't use any compatibility mode.
	None,
	/// Call `initialize_block` before doing any runtime calls.
	///
	/// Previously the node would execute `initialize_block` before fetching the authorities
	/// from the runtime. This behaviour changed in: <https://github.com/paritytech/substrate/pull/9132>
	///
	/// By calling `initialize_block` before fetching the authorities, on a block that
	/// would enact a new validator set, the block would already be build/sealed by an
	/// authority of the new set. With this mode disabled (the default) a block that enacts a new
	/// set isn't sealed/built by an authority of the new set, however to make new nodes be able to
	/// sync old chains this compatibility mode exists.
	UseInitializeBlock {
		/// The block number until this compatibility mode should be executed. The first runtime
		/// call in the context of the `until` block (importing it/building it) will disable the
		/// compatibility mode (i.e. at `until` the default rules will apply). When enabling this
		/// compatibility mode the `until` block should be a future block on which all nodes will
		/// have upgraded to a release that includes the updated compatibility mode configuration.
		/// At `until` block there will be a hard fork when the authority set changes, between the
		/// old nodes (running with `initialize_block`, i.e. without the compatibility mode
		/// configuration) and the new nodes.
		until: N,
	},
}

impl<N> Default for CompatibilityMode<N> {
	fn default() -> Self {
		Self::None
	}
}

/// Parameters of [`start_micc`].
pub struct StartMiccParams<C, SC, I, PF, SO, L, CIDP, BS, N> {
	/// The duration of a slot.
	pub slot_duration: SlotDuration,
	/// The client to interact with the chain.
	pub client: Arc<C>,
	/// A select chain implementation to select the best block.
	pub select_chain: SC,
	/// The block import.
	pub block_import: I,
	/// The proposer factory to build proposer instances.
	pub proposer_factory: PF,
	/// The sync oracle that can give us the current sync status.
	pub sync_oracle: SO,
	/// Hook into the sync module to control the justification sync process.
	pub justification_sync_link: L,
	/// Something that can create the inherent data providers.
	pub create_inherent_data_providers: CIDP,
	/// Should we force the authoring of blocks?
	pub force_authoring: bool,
	/// The backoff strategy when we miss slots.
	pub backoff_authoring_blocks: Option<BS>,
	/// The keystore used by the node.
	pub keystore: KeystorePtr,
	/// The proportion of the slot dedicated to proposing.
	///
	/// The block proposing will be limited to this proportion of the slot from the starting of the
	/// slot. However, the proposing can still take longer when there is some lenience factor
	/// applied, because there were no blocks produced for some slots.
	pub block_proposal_slot_portion: SlotProportion,
	/// The maximum proportion of the slot dedicated to proposing with any lenience factor applied
	/// due to no blocks being produced.
	pub max_block_proposal_slot_portion: Option<SlotProportion>,
	/// Telemetry instance used to report telemetry metrics.
	pub telemetry: Option<TelemetryHandle>,
	/// Compatibility mode that should be used.
	///
	/// If in doubt, use `Default::default()`.
	pub compatibility_mode: CompatibilityMode<N>,
}

/// Start the micc worker with event-driven block production.
/// This is the main function that replaces polling with transaction pool event monitoring.
pub fn start_micc<P, B, C, SC, I, PF, SO, L, CIDP, BS, Error, TExPool>(
    params: StartMiccParams<C, SC, I, PF, SO, L, CIDP, BS, NumberFor<B>>,
    pool: Arc<TExPool>,
) -> Result<impl Future<Output = ()>, ConsensusError>
where
    P: Pair,
    P::Public: AppPublic + Member,
    P::Signature: TryFrom<Vec<u8>> + Member + Codec,
    B: BlockT,
    C: ProvideRuntimeApi<B> + BlockOf + AuxStore + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: MiccApi<B, AuthorityId<P>>,
    SC: SelectChain<B> + 'static,
    I: BlockImport<B> + Send + Sync + 'static,
    PF: Environment<B, Error = Error> + Send + Sync + 'static,
    PF::Proposer: Proposer<B, Error = Error>,
    SO: SyncOracle + Send + Sync + Clone + 'static,
    L: sc_consensus::JustificationSyncLink<B>,
    CIDP: CreateInherentDataProviders<B, ()> + Send + 'static,
    CIDP::InherentDataProviders: InherentDataProviderExt + Send,
    BS: BackoffAuthoringBlocksStrategy<NumberFor<B>> + Send + Sync + 'static,
    Error: std::error::Error + Send + From<ConsensusError> + 'static,
    TExPool: MaintainedTransactionPool<Block = B, Hash = <B as BlockT>::Hash> + 'static,
{
    info!(target: LOG_TARGET, "Starting MICC consensus with true event-driven block production");
    
    // Use the new true event-driven implementation with import notifications
    start_micc_true_event_driven::<P, B, C, SC, I, PF, SO, L, CIDP, BS, Error, TExPool>(
        params, 
        pool,
        None, // Use default configuration
    )
}

/// Start the micc worker. The returned future should be run in a futures executor.
pub fn start_micc_v2<P, B, C, SC, I, PF, SO, L, CIDP, BS, Error, TExPool>(
    StartMiccParams {
        slot_duration,
        client,
        select_chain,
        block_import,
        proposer_factory,
        sync_oracle,
        justification_sync_link,
        create_inherent_data_providers,
        force_authoring,
        backoff_authoring_blocks,
        keystore,
        block_proposal_slot_portion,
        max_block_proposal_slot_portion,
        telemetry,
        compatibility_mode,
    }: StartMiccParams<C, SC, I, PF, SO, L, CIDP, BS, NumberFor<B>>,
    pool: Arc<TExPool>, // Use reference to the transaction pool
) -> Result<impl Future<Output = ()>, ConsensusError>
where
    P: Pair,
    P::Public: AppPublic + Member,
    P::Signature: TryFrom<Vec<u8>> + Member + Codec,
    B: BlockT,
    C: ProvideRuntimeApi<B> + BlockOf + AuxStore + HeaderBackend<B> + Send + Sync,
    C::Api: MiccApi<B, AuthorityId<P>>,
    SC: SelectChain<B>,
    I: BlockImport<B> + Send + Sync + 'static,
    PF: Environment<B, Error = Error> + Send + Sync + 'static,
    PF::Proposer: Proposer<B, Error = Error>,
    SO: SyncOracle + Send + Sync + Clone,
    L: sc_consensus::JustificationSyncLink<B>,
    CIDP: CreateInherentDataProviders<B, ()> + Send + 'static,
    CIDP::InherentDataProviders: InherentDataProviderExt + Send,
    BS: BackoffAuthoringBlocksStrategy<NumberFor<B>> + Send + Sync + 'static,
    Error: std::error::Error + Send + From<ConsensusError> + 'static,
    TExPool: MaintainedTransactionPool<Block = B, Hash = <B as BlockT>::Hash> + 'static,
{
    let worker = build_micc_worker::<P, _, _, _, _, _, _, _, _>(BuildMiccWorkerParams {
        client,
        block_import,
        proposer_factory,
        keystore,
        sync_oracle: sync_oracle.clone(),
        justification_sync_link,
        force_authoring,
        backoff_authoring_blocks,
        telemetry,
        block_proposal_slot_portion,
        max_block_proposal_slot_portion,
        compatibility_mode,
    });
    
    // Check if there are any transactions in pool every 500 ms
    let interval = time::interval(Duration::from_millis(slot_duration.as_millis() as u64));
    let interval_stream = IntervalStream::new(interval)
        .map(move |_event| {
			let pool_status = pool.status();
			if pool_status.ready > 0 {
				info!(target: LOG_TARGET, "Transaction pool has {} ready transactions", pool_status.ready);
				return SlotTrigger::CreateBlock;
			}
			SlotTrigger::NoAction
		});

	info!(target: LOG_TARGET, "Starting Micc slot worker");

	Ok(sc_consensus_slots::start_slot_worker_v2(
		slot_duration,
		select_chain,
		SimpleSlotWorkerToSlotWorker(worker),
		sync_oracle,
		create_inherent_data_providers,
		interval_stream,
	))
}

/// Start the micc worker with true event-driven block production.
/// This completely replaces polling with transaction pool event monitoring for optimal efficiency.
pub fn start_micc_event_driven<P, B, C, SC, I, PF, SO, L, CIDP, BS, Error, TExPool>(
    params: StartMiccParams<C, SC, I, PF, SO, L, CIDP, BS, NumberFor<B>>,
    pool: Arc<TExPool>,
    event_config: Option<crate::event_driven::EventDrivenConfig>,
) -> Result<impl Future<Output = ()>, ConsensusError>
where
    P: Pair,
    P::Public: AppPublic + Member,
    P::Signature: TryFrom<Vec<u8>> + Member + Codec,
    B: BlockT,
    C: ProvideRuntimeApi<B> + BlockOf + AuxStore + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: MiccApi<B, AuthorityId<P>>,
    SC: SelectChain<B> + 'static,
    I: BlockImport<B> + Send + Sync + 'static,
    PF: Environment<B, Error = Error> + Send + Sync + 'static,
    PF::Proposer: Proposer<B, Error = Error>,
    SO: SyncOracle + Send + Sync + Clone + 'static,
    L: sc_consensus::JustificationSyncLink<B>,
    CIDP: CreateInherentDataProviders<B, ()> + Send + 'static,
    CIDP::InherentDataProviders: InherentDataProviderExt + Send,
    BS: BackoffAuthoringBlocksStrategy<NumberFor<B>> + Send + Sync + 'static,
    Error: std::error::Error + Send + From<ConsensusError> + 'static,
    TExPool: MaintainedTransactionPool<Block = B, Hash = <B as BlockT>::Hash> + 'static,
{
    use crate::event_driven::create_event_driven_stream;
    
    let config = event_config.unwrap_or_default();
    
    let worker = build_micc_worker::<P, _, _, _, _, _, _, _, _>(BuildMiccWorkerParams {
        client: params.client,
        block_import: params.block_import,
        proposer_factory: params.proposer_factory,
        keystore: params.keystore,
        sync_oracle: params.sync_oracle.clone(),
        justification_sync_link: params.justification_sync_link,
        force_authoring: params.force_authoring,
        backoff_authoring_blocks: params.backoff_authoring_blocks,
        telemetry: params.telemetry,
        block_proposal_slot_portion: params.block_proposal_slot_portion,
        max_block_proposal_slot_portion: params.max_block_proposal_slot_portion,
        compatibility_mode: params.compatibility_mode,
    });

    info!(target: LOG_TARGET, "Starting true event-driven Micc consensus");

    // Create event-driven stream for transaction pool monitoring
    let event_stream = create_event_driven_stream::<B, TExPool>(pool, config);

    // Use the existing start_slot_worker_v2 with our event-driven stream
    Ok(sc_consensus_slots::start_slot_worker_v2(
        params.slot_duration,
        params.select_chain,
        SimpleSlotWorkerToSlotWorker(worker),
        params.sync_oracle,
        params.create_inherent_data_providers,
        event_stream,
    ))
}

/// Start the micc worker with the new true event-driven approach using import notifications.
/// This provides 0ms response time to transaction arrival for optimal block production.
pub fn start_micc_true_event_driven<P, B, C, SC, I, PF, SO, L, CIDP, BS, Error, TExPool>(
    params: StartMiccParams<C, SC, I, PF, SO, L, CIDP, BS, NumberFor<B>>,
    pool: Arc<TExPool>,
    event_config: Option<crate::event_driven::EventDrivenConfig>,
) -> Result<impl Future<Output = ()>, ConsensusError>
where
    P: Pair,
    P::Public: AppPublic + Member,
    P::Signature: TryFrom<Vec<u8>> + Member + Codec,
    B: BlockT,
    C: ProvideRuntimeApi<B> + BlockOf + AuxStore + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: MiccApi<B, AuthorityId<P>>,
    SC: SelectChain<B> + 'static,
    I: BlockImport<B> + Send + Sync + 'static,
    PF: Environment<B, Error = Error> + Send + Sync + 'static,
    PF::Proposer: Proposer<B, Error = Error>,
    SO: SyncOracle + Send + Sync + Clone + 'static,
    L: sc_consensus::JustificationSyncLink<B>,
    CIDP: CreateInherentDataProviders<B, ()> + Send + 'static,
    CIDP::InherentDataProviders: InherentDataProviderExt + Send,
    BS: BackoffAuthoringBlocksStrategy<NumberFor<B>> + Send + Sync + 'static,
    Error: std::error::Error + Send + From<ConsensusError> + 'static,
    TExPool: MaintainedTransactionPool<Block = B, Hash = <B as BlockT>::Hash> + 'static,
{
    use crate::event_driven::create_true_event_driven_stream;
    
    let config = event_config.unwrap_or_default();
    
    let worker = build_micc_worker::<P, _, _, _, _, _, _, _, _>(BuildMiccWorkerParams {
        client: params.client,
        block_import: params.block_import,
        proposer_factory: params.proposer_factory,
        keystore: params.keystore,
        sync_oracle: params.sync_oracle.clone(),
        justification_sync_link: params.justification_sync_link,
        force_authoring: params.force_authoring,
        backoff_authoring_blocks: params.backoff_authoring_blocks,
        telemetry: params.telemetry,
        block_proposal_slot_portion: params.block_proposal_slot_portion,
        max_block_proposal_slot_portion: params.max_block_proposal_slot_portion,
        compatibility_mode: params.compatibility_mode,
    });

    info!(target: LOG_TARGET, "Starting TRUE event-driven Micc consensus with import notifications");

    // Create true event-driven stream using transaction pool import notifications
    let event_stream = create_true_event_driven_stream::<B, TExPool>(pool, config);

    // Use the existing start_slot_worker_v2 with our true event-driven stream
    Ok(sc_consensus_slots::start_slot_worker_v2(
        params.slot_duration,
        params.select_chain,
        SimpleSlotWorkerToSlotWorker(worker),
        params.sync_oracle,
        params.create_inherent_data_providers,
        event_stream,
    ))
}

/// Parameters of [`build_micc_worker`].
pub struct BuildMiccWorkerParams<C, I, PF, SO, L, BS, N> {
	/// The client to interact with the chain.
	pub client: Arc<C>,
	/// The block import.
	pub block_import: I,
	/// The proposer factory to build proposer instances.
	pub proposer_factory: PF,
	/// The sync oracle that can give us the current sync status.
	pub sync_oracle: SO,
	/// Hook into the sync module to control the justification sync process.
	pub justification_sync_link: L,
	/// Should we force the authoring of blocks?
	pub force_authoring: bool,
	/// The backoff strategy when we miss slots.
	pub backoff_authoring_blocks: Option<BS>,
	/// The keystore used by the node.
	pub keystore: KeystorePtr,
	/// The proportion of the slot dedicated to proposing.
	///
	/// The block proposing will be limited to this proportion of the slot from the starting of the
	/// slot. However, the proposing can still take longer when there is some lenience factor
	/// applied, because there were no blocks produced for some slots.
	pub block_proposal_slot_portion: SlotProportion,
	/// The maximum proportion of the slot dedicated to proposing with any lenience factor applied
	/// due to no blocks being produced.
	pub max_block_proposal_slot_portion: Option<SlotProportion>,
	/// Telemetry instance used to report telemetry metrics.
	pub telemetry: Option<TelemetryHandle>,
	/// Compatibility mode that should be used.
	///
	/// If in doubt, use `Default::default()`.
	pub compatibility_mode: CompatibilityMode<N>,
}

/// Build the micc worker.
///
/// The caller is responsible for running this worker, otherwise it will do nothing.
pub fn build_micc_worker<P, B, C, PF, I, SO, L, BS, Error>(
	BuildMiccWorkerParams {
		client,
		block_import,
		proposer_factory,
		sync_oracle,
		justification_sync_link,
		backoff_authoring_blocks,
		keystore,
		block_proposal_slot_portion,
		max_block_proposal_slot_portion,
		telemetry,
		force_authoring,
		compatibility_mode,
	}: BuildMiccWorkerParams<C, I, PF, SO, L, BS, NumberFor<B>>,
) -> impl sc_consensus_slots::SimpleSlotWorker<
	B,
	Proposer = PF::Proposer,
	BlockImport = I,
	SyncOracle = SO,
	JustificationSyncLink = L,
	Claim = P::Public,
	AuxData = Vec<AuthorityId<P>>,
>
where
	B: BlockT,
	C: ProvideRuntimeApi<B> + BlockOf + AuxStore + HeaderBackend<B> + Send + Sync,
	C::Api: MiccApi<B, AuthorityId<P>>,
	PF: Environment<B, Error = Error> + Send + Sync + 'static,
	PF::Proposer: Proposer<B, Error = Error>,
	P: Pair,
	P::Public: AppPublic + Member,
	P::Signature: TryFrom<Vec<u8>> + Member + Codec,
	I: BlockImport<B> + Send + Sync + 'static,
	Error: std::error::Error + Send + From<ConsensusError> + 'static,
	SO: SyncOracle + Send + Sync + Clone,
	L: sc_consensus::JustificationSyncLink<B>,
	BS: BackoffAuthoringBlocksStrategy<NumberFor<B>> + Send + Sync + 'static,
{
	MiccWorker {
		client,
		block_import,
		env: proposer_factory,
		keystore,
		sync_oracle,
		justification_sync_link,
		force_authoring,
		backoff_authoring_blocks,
		telemetry,
		block_proposal_slot_portion,
		max_block_proposal_slot_portion,
		compatibility_mode,
		_phantom: PhantomData::<fn() -> P>,
	}
}

struct MiccWorker<C, E, I, P, SO, L, BS, N> {
	client: Arc<C>,
	block_import: I,
	env: E,
	keystore: KeystorePtr,
	sync_oracle: SO,
	justification_sync_link: L,
	force_authoring: bool,
	backoff_authoring_blocks: Option<BS>,
	block_proposal_slot_portion: SlotProportion,
	max_block_proposal_slot_portion: Option<SlotProportion>,
	telemetry: Option<TelemetryHandle>,
	compatibility_mode: CompatibilityMode<N>,
	_phantom: PhantomData<fn() -> P>,
}

#[async_trait::async_trait]
impl<B, C, E, I, P, Error, SO, L, BS> sc_consensus_slots::SimpleSlotWorker<B>
	for MiccWorker<C, E, I, P, SO, L, BS, NumberFor<B>>
where
	B: BlockT,
	C: ProvideRuntimeApi<B> + BlockOf + HeaderBackend<B> + Sync,
	C::Api: MiccApi<B, AuthorityId<P>>,
	E: Environment<B, Error = Error> + Send + Sync,
	E::Proposer: Proposer<B, Error = Error>,
	I: BlockImport<B> + Send + Sync + 'static,
	P: Pair,
	P::Public: AppPublic + Member,
	P::Signature: TryFrom<Vec<u8>> + Member + Codec,
	SO: SyncOracle + Send + Clone + Sync,
	L: sc_consensus::JustificationSyncLink<B>,
	BS: BackoffAuthoringBlocksStrategy<NumberFor<B>> + Send + Sync + 'static,
	Error: std::error::Error + Send + From<ConsensusError> + 'static,
{
	type BlockImport = I;
	type SyncOracle = SO;
	type JustificationSyncLink = L;
	type CreateProposer =
		Pin<Box<dyn Future<Output = Result<E::Proposer, ConsensusError>> + Send + 'static>>;
	type Proposer = E::Proposer;
	type Claim = P::Public;
	type AuxData = Vec<AuthorityId<P>>;

	fn logging_target(&self) -> &'static str {
		"micc"
	}

	fn block_import(&mut self) -> &mut Self::BlockImport {
		&mut self.block_import
	}

	fn aux_data(&self, header: &B::Header, _slot: Slot) -> Result<Self::AuxData, ConsensusError> {
		authorities(
			self.client.as_ref(),
			header.hash(),
			*header.number() + 1u32.into(),
			&self.compatibility_mode,
		)
	}

	fn authorities_len(&self, authorities: &Self::AuxData) -> Option<usize> {
		Some(authorities.len())
	}

	async fn claim_slot(
		&mut self,
		_header: &B::Header,
		slot: Slot,
		authorities: &Self::AuxData,
	) -> Option<Self::Claim> {
		// For force authoring (dev mode), allow any authority in keystore to claim any slot
		if self.force_authoring {
			// Try to find any authority key in our keystore that can sign
			for authority in authorities {
				if self.keystore.has_keys(&[(authority.to_raw_vec(), sp_application_crypto::key_types::MICC)]) {
					log::info!(target: "micc", "🔧 Force authoring: claiming slot {} with available authority", slot);
					return Some(authority.clone());
				}
			}
			log::debug!(target: "micc", "🔧 Force authoring: no authority keys available in keystore for slot {}", slot);
			return None;
		}
		
		// Normal mode: use strict slot assignment
		crate::standalone::claim_slot::<P>(slot, authorities, &self.keystore).await
	}

	fn pre_digest_data(&self, slot: Slot, _claim: &Self::Claim) -> Vec<sp_runtime::DigestItem> {
		vec![crate::standalone::pre_digest::<P>(slot)]
	}

	async fn block_import_params(
		&self,
		header: B::Header,
		header_hash: &B::Hash,
		body: Vec<B::Extrinsic>,
		storage_changes: StorageChanges<B>,
		public: Self::Claim,
		_authorities: Self::AuxData,
	) -> Result<sc_consensus::BlockImportParams<B>, ConsensusError> {
		let signature_digest_item =
			crate::standalone::seal::<_, P>(header_hash, &public, &self.keystore)?;

		let mut import_block = BlockImportParams::new(BlockOrigin::Own, header);
		import_block.post_digests.push(signature_digest_item);
		import_block.body = Some(body);
		import_block.state_action =
			StateAction::ApplyChanges(sc_consensus::StorageChanges::Changes(storage_changes));
		import_block.fork_choice = Some(ForkChoiceStrategy::LongestChain);

		Ok(import_block)
	}

	fn force_authoring(&self) -> bool {
		self.force_authoring
	}

	fn should_backoff(&self, slot: Slot, chain_head: &B::Header) -> bool {
		if let Some(ref strategy) = self.backoff_authoring_blocks {
			if let Ok(chain_head_slot) = find_pre_digest::<B, P::Signature>(chain_head) {
				return strategy.should_backoff(
					*chain_head.number(),
					chain_head_slot,
					self.client.info().finalized_number,
					slot,
					self.logging_target(),
				)
			}
		}
		false
	}

	fn sync_oracle(&mut self) -> &mut Self::SyncOracle {
		&mut self.sync_oracle
	}

	fn justification_sync_link(&mut self) -> &mut Self::JustificationSyncLink {
		&mut self.justification_sync_link
	}

	fn proposer(&mut self, block: &B::Header) -> Self::CreateProposer {
		self.env
			.init(block)
			.map_err(|e| ConsensusError::ClientImport(format!("{:?}", e)))
			.boxed()
	}

	fn telemetry(&self) -> Option<TelemetryHandle> {
		self.telemetry.clone()
	}

	fn proposing_remaining_duration(&self, slot_info: &SlotInfo<B>) -> std::time::Duration {
		let parent_slot = find_pre_digest::<B, P::Signature>(&slot_info.chain_head).ok();

		sc_consensus_slots::proposing_remaining_duration(
			parent_slot,
			slot_info,
			&self.block_proposal_slot_portion,
			self.max_block_proposal_slot_portion.as_ref(),
			sc_consensus_slots::SlotLenienceType::Exponential,
			self.logging_target(),
		)
	}
}

/// Micc Errors
#[derive(Debug, thiserror::Error)]
pub enum Error<B: BlockT> {
	/// Multiple Micc pre-runtime headers
	#[error("Multiple Micc pre-runtime headers")]
	MultipleHeaders,
	/// No Micc pre-runtime digest found
	#[error("No Micc pre-runtime digest found")]
	NoDigestFound,
	/// Header is unsealed
	#[error("Header {0:?} is unsealed")]
	HeaderUnsealed(B::Hash),
	/// Header has a bad seal
	#[error("Header {0:?} has a bad seal")]
	HeaderBadSeal(B::Hash),
	/// Slot Author not found
	#[error("Slot Author not found")]
	SlotAuthorNotFound,
	/// Bad signature
	#[error("Bad signature on {0:?}")]
	BadSignature(B::Hash),
	/// Client Error
	#[error(transparent)]
	Client(sp_blockchain::Error),
	/// Unknown inherent error for identifier
	#[error("Unknown inherent error for identifier: {}", String::from_utf8_lossy(.0))]
	UnknownInherentError(sp_inherents::InherentIdentifier),
	/// Inherents Error
	#[error("Inherent error: {0}")]
	Inherent(sp_inherents::Error),
}

impl<B: BlockT> From<Error<B>> for String {
	fn from(error: Error<B>) -> String {
		error.to_string()
	}
}

impl<B: BlockT> From<crate::standalone::PreDigestLookupError> for Error<B> {
	fn from(e: crate::standalone::PreDigestLookupError) -> Self {
		match e {
			crate::standalone::PreDigestLookupError::MultipleHeaders => Error::MultipleHeaders,
			crate::standalone::PreDigestLookupError::NoDigestFound => Error::NoDigestFound,
		}
	}
}

fn authorities<A, B, C>(
	client: &C,
	parent_hash: B::Hash,
	context_block_number: NumberFor<B>,
	compatibility_mode: &CompatibilityMode<NumberFor<B>>,
) -> Result<Vec<A>, ConsensusError>
where
	A: Codec + Debug,
	B: BlockT,
	C: ProvideRuntimeApi<B>,
	C::Api: MiccApi<B, A>,
{
	let runtime_api = client.runtime_api();

	match compatibility_mode {
		CompatibilityMode::None => {},
		// Use `initialize_block` until we hit the block that should disable the mode.
		CompatibilityMode::UseInitializeBlock { until } =>
			if *until > context_block_number {
				runtime_api
					.initialize_block(
						parent_hash,
						&B::Header::new(
							context_block_number,
							Default::default(),
							Default::default(),
							parent_hash,
							Default::default(),
						),
					)
					.map_err(|_| ConsensusError::InvalidAuthoritiesSet)?;
			},
	}

	runtime_api
		.authorities(parent_hash)
		.ok()
		.ok_or(ConsensusError::InvalidAuthoritiesSet)
}

#[cfg(test)]
mod tests {
	use super::*;
	use parking_lot::Mutex;
	use sc_block_builder::BlockBuilderBuilder;
	use sc_client_api::BlockchainEvents;
	use sc_consensus::BoxJustificationImport;
	use sc_consensus_slots::{BackoffAuthoringOnFinalizedHeadLagging, SimpleSlotWorker};
	use sc_keystore::LocalKeystore;
	use sc_network_test::{Block as TestBlock, *};
	use sp_application_crypto::{key_types::MICC, AppCrypto};
	use sp_consensus::{DisableProofRecording, NoNetwork as DummyOracle, Proposal};
	use sp_consensus_micc::sr25519::AuthorityPair;
	use sp_inherents::InherentData;
	use sp_keyring::sr25519::Keyring;
	use sp_keystore::Keystore;
	use sp_runtime::{
		traits::{Block as BlockT, Header as _},
		Digest,
	};
	use sp_timestamp::Timestamp;
	use std::{
		task::Poll,
		time::{Duration, Instant},
	};
	use substrate_test_runtime_client::{
		runtime::{Header, H256},
		TestClient,
	};

	const SLOT_DURATION_MS: u64 = 1000;

	type Error = sp_blockchain::Error;

	struct DummyFactory(Arc<TestClient>);
	struct DummyProposer(Arc<TestClient>);

	impl Environment<TestBlock> for DummyFactory {
		type Proposer = DummyProposer;
		type CreateProposer = futures::future::Ready<Result<DummyProposer, Error>>;
		type Error = Error;

		fn init(&mut self, _: &<TestBlock as BlockT>::Header) -> Self::CreateProposer {
			futures::future::ready(Ok(DummyProposer(self.0.clone())))
		}
	}

	impl Proposer<TestBlock> for DummyProposer {
		type Error = Error;
		type Proposal = future::Ready<Result<Proposal<TestBlock, ()>, Error>>;
		type ProofRecording = DisableProofRecording;
		type Proof = ();

		fn propose(
			self,
			_: InherentData,
			digests: Digest,
			_: Duration,
			_: Option<usize>,
		) -> Self::Proposal {
			let r = BlockBuilderBuilder::new(&*self.0)
				.on_parent_block(self.0.chain_info().best_hash)
				.fetch_parent_block_number(&*self.0)
				.unwrap()
				.with_inherent_digests(digests)
				.build()
				.unwrap()
				.build();

			future::ready(r.map(|b| Proposal {
				block: b.block,
				proof: (),
				storage_changes: b.storage_changes,
			}))
		}
	}

	type MiccVerifier = import_queue::MiccVerifier<
		PeersFullClient,
		AuthorityPair,
		Box<
			dyn CreateInherentDataProviders<
				TestBlock,
				(),
				InherentDataProviders = (InherentDataProvider,),
			>,
		>,
		u64,
	>;
	type MiccPeer = Peer<(), PeersClient>;

	#[derive(Default)]
	pub struct MiccTestNet {
		peers: Vec<MiccPeer>,
	}

	impl TestNetFactory for MiccTestNet {
		type Verifier = MiccVerifier;
		type PeerData = ();
		type BlockImport = PeersClient;

		fn make_verifier(&self, client: PeersClient, _peer_data: &()) -> Self::Verifier {
			let client = client.as_client();
			let slot_duration = slot_duration(&*client).expect("slot duration available");

			assert_eq!(slot_duration.as_millis() as u64, SLOT_DURATION_MS);
			import_queue::MiccVerifier::new(
				client,
				Box::new(|_, _| async {
					let slot = InherentDataProvider::from_timestamp_and_slot_duration(
						Timestamp::current(),
						SlotDuration::from_millis(SLOT_DURATION_MS),
					);
					Ok((slot,))
				}),
				CheckForEquivocation::Yes,
				None,
				CompatibilityMode::None,
			)
		}

		fn make_block_import(
			&self,
			client: PeersClient,
		) -> (
			BlockImportAdapter<Self::BlockImport>,
			Option<BoxJustificationImport<Block>>,
			Self::PeerData,
		) {
			(client.as_block_import(), None, ())
		}

		fn peer(&mut self, i: usize) -> &mut MiccPeer {
			&mut self.peers[i]
		}

		fn peers(&self) -> &Vec<MiccPeer> {
			&self.peers
		}

		fn peers_mut(&mut self) -> &mut Vec<MiccPeer> {
			&mut self.peers
		}

		fn mut_peers<F: FnOnce(&mut Vec<MiccPeer>)>(&mut self, closure: F) {
			closure(&mut self.peers);
		}
	}

	#[tokio::test]
	async fn authoring_blocks() {
		sp_tracing::try_init_simple();
		let net = MiccTestNet::new(3);

		let peers = &[(0, Keyring::Alice), (1, Keyring::Bob), (2, Keyring::Charlie)];

		let net = Arc::new(Mutex::new(net));
		let mut import_notifications = Vec::new();
		let mut micc_futures = Vec::new();

		let mut keystore_paths = Vec::new();
		for (peer_id, key) in peers {
			let mut net = net.lock();
			let peer = net.peer(*peer_id);
			let client = peer.client().as_client();
			let select_chain = peer.select_chain().expect("full client has a select chain");
			let keystore_path = tempfile::tempdir().expect("Creates keystore path");
			let keystore = Arc::new(
				LocalKeystore::open(keystore_path.path(), None).expect("Creates keystore."),
			);

			keystore
				.sr25519_generate_new(MICC, Some(&key.to_seed()))
				.expect("Creates authority key");
			keystore_paths.push(keystore_path);

			let environ = DummyFactory(client.clone());
			import_notifications.push(
				client
					.import_notification_stream()
					.take_while(|n| {
						future::ready(!(n.origin != BlockOrigin::Own && n.header.number() < &5))
					})
					.for_each(move |_| future::ready(())),
			);

			let slot_duration = slot_duration(&*client).expect("slot duration available");

			micc_futures.push(
				start_micc::<AuthorityPair, _, _, _, _, _, _, _, _, _, _>(StartMiccParams {
					slot_duration,
					block_import: client.clone(),
					select_chain,
					client,
					proposer_factory: environ,
					sync_oracle: DummyOracle,
					justification_sync_link: (),
					create_inherent_data_providers: |_, _| async {
						let slot = InherentDataProvider::from_timestamp_and_slot_duration(
							Timestamp::current(),
							SlotDuration::from_millis(SLOT_DURATION_MS),
						);

						Ok((slot,))
					},
					force_authoring: false,
					backoff_authoring_blocks: Some(
						BackoffAuthoringOnFinalizedHeadLagging::default(),
					),
					keystore,
					block_proposal_slot_portion: SlotProportion::new(0.5),
					max_block_proposal_slot_portion: None,
					telemetry: None,
					compatibility_mode: CompatibilityMode::None,
				})
				.expect("Starts micc"),
			);
		}

		future::select(
			future::poll_fn(move |cx| {
				net.lock().poll(cx);
				Poll::<()>::Pending
			}),
			future::select(future::join_all(micc_futures), future::join_all(import_notifications)),
		)
		.await;
	}

	#[tokio::test]
	async fn current_node_authority_should_claim_slot() {
		let net = MiccTestNet::new(4);

		let mut authorities = vec![
			Keyring::Alice.public().into(),
			Keyring::Bob.public().into(),
			Keyring::Charlie.public().into(),
		];

		let keystore_path = tempfile::tempdir().expect("Creates keystore path");
		let keystore = LocalKeystore::open(keystore_path.path(), None).expect("Creates keystore.");
		let public = keystore
			.sr25519_generate_new(AuthorityPair::ID, None)
			.expect("Key should be created");
		authorities.push(public.into());

		let net = Arc::new(Mutex::new(net));

		let mut net = net.lock();
		let peer = net.peer(3);
		let client = peer.client().as_client();
		let environ = DummyFactory(client.clone());

		let mut worker = MiccWorker {
			client: client.clone(),
			block_import: client,
			env: environ,
			keystore: keystore.into(),
			sync_oracle: DummyOracle,
			justification_sync_link: (),
			force_authoring: false,
			backoff_authoring_blocks: Some(BackoffAuthoringOnFinalizedHeadLagging::default()),
			telemetry: None,
			block_proposal_slot_portion: SlotProportion::new(0.5),
			max_block_proposal_slot_portion: None,
			compatibility_mode: Default::default(),
			_phantom: PhantomData::<fn() -> AuthorityPair>,
		};

		let head = Header::new(
			1,
			H256::from_low_u64_be(0),
			H256::from_low_u64_be(0),
			Default::default(),
			Default::default(),
		);
		assert!(worker.claim_slot(&head, 0.into(), &authorities).await.is_none());
		assert!(worker.claim_slot(&head, 1.into(), &authorities).await.is_none());
		assert!(worker.claim_slot(&head, 2.into(), &authorities).await.is_none());
		assert!(worker.claim_slot(&head, 3.into(), &authorities).await.is_some());
		assert!(worker.claim_slot(&head, 4.into(), &authorities).await.is_none());
		assert!(worker.claim_slot(&head, 5.into(), &authorities).await.is_none());
		assert!(worker.claim_slot(&head, 6.into(), &authorities).await.is_none());
		assert!(worker.claim_slot(&head, 7.into(), &authorities).await.is_some());
	}

	#[tokio::test]
	async fn on_slot_returns_correct_block() {
		let net = MiccTestNet::new(4);

		let keystore_path = tempfile::tempdir().expect("Creates keystore path");
		let keystore = LocalKeystore::open(keystore_path.path(), None).expect("Creates keystore.");
		keystore
			.sr25519_generate_new(AuthorityPair::ID, Some(&Keyring::Alice.to_seed()))
			.expect("Key should be created");

		let net = Arc::new(Mutex::new(net));

		let mut net = net.lock();
		let peer = net.peer(3);
		let client = peer.client().as_client();
		let environ = DummyFactory(client.clone());

		let mut worker = MiccWorker {
			client: client.clone(),
			block_import: client.clone(),
			env: environ,
			keystore: keystore.into(),
			sync_oracle: DummyOracle,
			justification_sync_link: (),
			force_authoring: false,
			backoff_authoring_blocks: Option::<()>::None,
			telemetry: None,
			block_proposal_slot_portion: SlotProportion::new(0.5),
			max_block_proposal_slot_portion: None,
			compatibility_mode: Default::default(),
			_phantom: PhantomData::<fn() -> AuthorityPair>,
		};

		let head = client.expect_header(client.info().genesis_hash).unwrap();

		let res = worker
			.on_slot(SlotInfo {
				slot: 0.into(),
				ends_at: Instant::now() + Duration::from_secs(100),
				create_inherent_data: Box::new(()),
				duration: Duration::from_millis(1000),
				chain_head: head,
				block_size_limit: None,
			})
			.await
			.unwrap();

		// The returned block should be imported and we should be able to get its header by now.
		assert!(client.header(res.block.hash()).unwrap().is_some());
	}
}
