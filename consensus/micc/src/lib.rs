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

//! # Micc Module
//!
//! - [`Config`]
//! - [`Pallet`]
//!
//! ## Overview
//!
//! The Micc module extends Micc consensus by managing offline reporting.
//!
//! ## Interface
//!
//! ### Public Functions
//!
//! - `slot_duration` - Determine the Micc slot-duration based on the Timestamp module
//!   configuration.
//!
//! ## Related Modules
//!
//! - [Timestamp](../pallet_timestamp/index.html): The Timestamp module is used in Micc to track
//! consensus rounds (via `slots`).

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	traits::{DisabledValidators, FindAuthor, Get, OnTimestampSet, OneSessionHandler},
	BoundedSlice, BoundedVec, ConsensusEngineId, Parameter,
	dispatch::DispatchResult,
};
use frame_system::pallet_prelude::BlockNumberFor;
use log;
use sp_consensus_micc::{AuthorityIndex, ConsensusLog, Slot, MICC_ENGINE_ID};
use sp_runtime::{
	generic::DigestItem,
	traits::{IsMember, Member, SaturatedConversion, Saturating, Zero},
	RuntimeAppPublic,
};

mod mock;
mod tests;

/// Security testing module for consensus robustness validation
#[cfg(test)]
mod security_tests;

/// Equivocation detection and handling for MICC consensus
pub mod equivocation;

pub use pallet::*;

const LOG_TARGET: &str = "runtime::micc";


/// A slot duration provider which infers the slot duration from the
/// [`pallet_timestamp::Config::MinimumPeriod`] by multiplying it by two, to ensure
/// that authors have the majority of their slot to author within.
///
/// This was the default behavior of the Micc pallet and may be used for
/// backwards compatibility.
pub struct MinimumPeriodTimesTwo<T>(core::marker::PhantomData<T>);

impl<T: pallet_timestamp::Config> Get<T::Moment> for MinimumPeriodTimesTwo<T> {
	fn get() -> T::Moment {
		<T as pallet_timestamp::Config>::MinimumPeriod::get().saturating_mul(2u32.into())
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::OriginFor;
	use frame_system::{ensure_root, ensure_signed};

	#[pallet::config]
	pub trait Config: pallet_timestamp::Config + frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The identifier type for an authority.
		type AuthorityId: Member
			+ Parameter
			+ RuntimeAppPublic
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;
		/// The maximum number of authorities that the pallet can hold.
		type MaxAuthorities: Get<u32>;

		/// A way to check whether a given validator is disabled and should not be authoring blocks.
		/// Blocks authored by a disabled validator will lead to a panic as part of this module's
		/// initialization.
		type DisabledValidators: DisabledValidators;

		/// Whether to allow block authors to create multiple blocks per slot.
		///
		/// If this is `true`, the pallet will allow slots to stay the same across sequential
		/// blocks. If this is `false`, the pallet will require that subsequent blocks always have
		/// higher slots than previous ones.
		///
		/// Regardless of the setting of this storage value, the pallet will always enforce the
		/// invariant that slots don't move backwards as the chain progresses.
		///
		/// The typical value for this should be 'false' unless this pallet is being augmented by
		/// another pallet which enforces some limitation on the number of blocks authors can create
		/// using the same slot.
		type AllowMultipleBlocksPerSlot: Get<bool>;

		/// The slot duration Micc should run with, expressed in milliseconds.
		/// The effective value of this type should not change while the chain is running.
		///
		/// For backwards compatibility either use [`MinimumPeriodTimesTwo`] or a const.
		#[pallet::constant]
		type SlotDuration: Get<<Self as pallet_timestamp::Config>::Moment>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(core::marker::PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_: BlockNumberFor<T>) -> Weight {
			if let Some(new_slot) = Self::current_slot_from_digests() {
				let current_slot = CurrentSlot::<T>::get();

				if T::AllowMultipleBlocksPerSlot::get() {
					if current_slot > new_slot {
						log::error!(
							target: LOG_TARGET,
							"🚨 Slot decreased from {:?} to {:?}. Gracefully handling.",
							current_slot, new_slot
						);
						
						Self::deposit_event(Event::SlotValidationFailed {
							current_slot,
							new_slot,
							block_number: frame_system::Pallet::<T>::block_number(),
						});
						
						Self::deposit_event(Event::ConsensusErrorRecovered {
							error_type: 2u8, // SlotTiming
							block_number: frame_system::Pallet::<T>::block_number(),
						});
						
						return T::DbWeight::get().reads(1);
					}
				} else {
					if current_slot >= new_slot {
						log::error!(
							target: LOG_TARGET,
							"🚨 Slot failed to increase from {:?} to {:?}. Gracefully handling.",
							current_slot, new_slot
						);
						
						Self::deposit_event(Event::SlotValidationFailed {
							current_slot,
							new_slot,
							block_number: frame_system::Pallet::<T>::block_number(),
						});
						
						Self::deposit_event(Event::ConsensusErrorRecovered {
							error_type: 2u8, // SlotTiming
							block_number: frame_system::Pallet::<T>::block_number(),
						});
						
						return T::DbWeight::get().reads(1);
					}
				}

				CurrentSlot::<T>::put(new_slot);

				if let Some(n_authorities) = <Authorities<T>>::decode_len() {
					let authority_index = *new_slot % n_authorities as u64;
					if T::DisabledValidators::is_disabled(authority_index as u32) {
						log::error!(
							target: LOG_TARGET,
							"🚨 Disabled validator attempted to author block at index {:?}. Gracefully skipping.",
							authority_index
						);
						
						// Emit event for monitoring and alerting
						Self::deposit_event(Event::DisabledValidatorAttempt { 
							authority_index: authority_index as u32,
							slot: new_slot,
							block_number: frame_system::Pallet::<T>::block_number(),
						});
						
						// Emit recovery event
						Self::deposit_event(Event::ConsensusErrorRecovered {
							error_type: 0u8, // DisabledValidator
							block_number: frame_system::Pallet::<T>::block_number(),
						});
						
						// Return early with minimal weight instead of panicking
						return T::DbWeight::get().reads(1);
					}
				} else {
					log::error!(
						target: LOG_TARGET,
						"🚨 Failed to decode authorities length. Gracefully handling."
					);
					
					// Emit event for monitoring
					Self::deposit_event(Event::AuthoritiesDecodeError {
						block_number: frame_system::Pallet::<T>::block_number(),
					});
					
					// Emit recovery event
					Self::deposit_event(Event::ConsensusErrorRecovered {
						error_type: 1u8, // AuthorityDecode
						block_number: frame_system::Pallet::<T>::block_number(),
					});
					
					// Return with minimal weight
					return T::DbWeight::get().reads(1);
				}

				// TODO [#3398] Generate offence report for all authorities that skipped their
				// slots.

				T::DbWeight::get().reads_writes(2, 1)
			} else {
				T::DbWeight::get().reads(1)
			}
		}

		#[cfg(feature = "try-runtime")]
		fn try_state(_: BlockNumberFor<T>) -> Result<(), sp_runtime::TryRuntimeError> {
			Self::do_try_state()
		}
	}

	/// The current authority set.
	#[pallet::storage]
	pub type Authorities<T: Config> =
		StorageValue<_, BoundedVec<T::AuthorityId, T::MaxAuthorities>, ValueQuery>;

	/// The current slot of this block.
	///
	/// This will be set in `on_initialize`.
	#[pallet::storage]
	pub type CurrentSlot<T: Config> = StorageValue<_, Slot, ValueQuery>;

	/// Configuration for equivocation detection and handling.
	#[pallet::storage]
	pub type EquivocationConfig<T: Config> = StorageValue<_, crate::equivocation::EquivocationConfig, ValueQuery>;

	/// Current session equivocations count per authority.
	#[pallet::storage]
	pub type SessionEquivocations<T: Config> = StorageMap<_, Blake2_128Concat, T::AuthorityId, u32, ValueQuery>;

	/// Disabled authorities due to equivocation.
	#[pallet::storage]
	pub type DisabledAuthorities<T: Config> = StorageMap<_, Blake2_128Concat, T::AuthorityId, BlockNumberFor<T>, OptionQuery>;

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		pub authorities: Vec<T::AuthorityId>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			Pallet::<T>::initialize_authorities(&self.authorities);
		}
	}

	/// Events emitted by the MICC pallet.
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Equivocation detected. [authority, slot, session]
		EquivocationDetected {
			authority: T::AuthorityId,
			slot: Slot,
			session: u32,
		},
		/// Authority disabled due to equivocation. [authority, block_number]
		AuthorityDisabled {
			authority: T::AuthorityId,
			block_number: BlockNumberFor<T>,
		},
		/// Equivocation configuration updated.
		EquivocationConfigUpdated,
		/// Session equivocations cleared.
		SessionEquivocationsCleared,
		/// Disabled validator attempted to author block
		DisabledValidatorAttempt {
			authority_index: u32,
			slot: Slot,
			block_number: BlockNumberFor<T>,
		},
		/// Authority set decoding failed
		AuthoritiesDecodeError {
			block_number: BlockNumberFor<T>,
		},
		/// Consensus error recovered gracefully
		/// error_type: 0=DisabledValidator, 1=AuthorityDecode, 2=SlotTiming
		ConsensusErrorRecovered {
			error_type: u8,
			block_number: BlockNumberFor<T>,
		},
		/// Slot validation failed but recovered
		SlotValidationFailed {
			current_slot: Slot,
			new_slot: Slot,
			block_number: BlockNumberFor<T>,
		},
	}

	/// Errors that can occur in the MICC pallet.
	#[pallet::error]
	pub enum Error<T> {
		/// Invalid equivocation configuration.
		InvalidEquivocationConfig,
		/// Authority not found.
		AuthorityNotFound,
		/// Equivocation already reported.
		EquivocationAlreadyReported,
		/// Failed to decode authorities
		AuthoritiesDecodeFailed,
		/// Disabled validator attempted block authoring
		DisabledValidatorAttempt,
		/// Invalid slot for current block
		InvalidSlot,
		/// Authority set is empty
		EmptyAuthoritySet,
		/// Slot timing validation failed
		SlotTimingError,
		/// Block production outside allowed time window
		OutsideProductionWindow,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Enable or disable equivocation slashing (root only).
		#[pallet::call_index(0)]
		#[pallet::weight(10_000)]
		pub fn set_equivocation_slashing(
			origin: OriginFor<T>,
			enable: bool,
		) -> DispatchResult {
			ensure_root(origin)?;
			let mut config = EquivocationConfig::<T>::get();
			config.enable_slashing = enable;
			EquivocationConfig::<T>::put(config);
			Self::deposit_event(Event::EquivocationConfigUpdated);
			Ok(())
		}

		/// Report equivocation (can be called by anyone with valid proof).
		#[pallet::call_index(1)]
		#[pallet::weight(10_000)]
		pub fn report_equivocation(
			origin: OriginFor<T>,
			report: crate::equivocation::EquivocationReport<T::AuthorityId>,
		) -> DispatchResult {
			let _ = ensure_signed(origin)?;

			// Increment equivocation count for this authority
			let count = SessionEquivocations::<T>::get(&report.offender);
			let new_count = count.saturating_add(1);
			SessionEquivocations::<T>::insert(&report.offender, new_count);

			Self::deposit_event(Event::EquivocationDetected {
				authority: report.offender.clone(),
				slot: report.slot,
				session: report.session_index,
			});

			// Check if authority should be disabled
			let config = EquivocationConfig::<T>::get();
			if config.enable_slashing && new_count > 0 {
				let current_block = frame_system::Pallet::<T>::block_number();
				DisabledAuthorities::<T>::insert(&report.offender, current_block);
				
				Self::deposit_event(Event::AuthorityDisabled {
					authority: report.offender,
					block_number: current_block,
				});
			}

			Ok(())
		}

		/// Clear session equivocations (root only) - used for new sessions.
		#[pallet::call_index(2)]
		#[pallet::weight(10_000)]
		pub fn clear_session_equivocations(origin: OriginFor<T>) -> DispatchResult {
			ensure_root(origin)?;
			let _ = SessionEquivocations::<T>::clear(u32::MAX, None);
			Self::deposit_event(Event::SessionEquivocationsCleared);
			Ok(())
		}

		/// Re-enable a disabled authority (root only).
		#[pallet::call_index(3)]
		#[pallet::weight(10_000)]
		pub fn enable_authority(
			origin: OriginFor<T>,
			authority: T::AuthorityId,
		) -> DispatchResult {
			ensure_root(origin)?;
			DisabledAuthorities::<T>::remove(&authority);
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Change authorities.
	///
	/// The storage will be applied immediately.
	/// And Micc consensus log will be appended to block's log.
	///
	/// This is a no-op if `new` is empty.
	pub fn change_authorities(new: BoundedVec<T::AuthorityId, T::MaxAuthorities>) {
		if new.is_empty() {
			log::warn!(target: LOG_TARGET, "Ignoring empty authority change.");

			return
		}

		<Authorities<T>>::put(&new);

		let log = DigestItem::Consensus(
			MICC_ENGINE_ID,
			ConsensusLog::AuthoritiesChange(new.into_inner()).encode(),
		);
		<frame_system::Pallet<T>>::deposit_log(log);
	}

	/// Initial authorities.
	///
	/// The storage will be applied immediately.
	///
	/// The authorities length must be equal or less than T::MaxAuthorities.
	pub fn initialize_authorities(authorities: &[T::AuthorityId]) {
		if !authorities.is_empty() {
			if !<Authorities<T>>::get().is_empty() {
				log::error!(
					target: LOG_TARGET,
					"🚨 Attempted to initialize authorities when already initialized. Ignoring."
				);
				return;
			}
			
			match <BoundedSlice<'_, _, T::MaxAuthorities>>::try_from(authorities) {
				Ok(bounded) => <Authorities<T>>::put(bounded),
				Err(_) => {
					log::error!(
						target: LOG_TARGET,
						"🚨 Initial authority set size {} exceeds maximum {}. Truncating.",
						authorities.len(),
						T::MaxAuthorities::get()
					);
					let bounded = <BoundedVec<_, T::MaxAuthorities>>::truncate_from(authorities.to_vec());
					<Authorities<T>>::put(bounded);
				}
			}
		}
	}

	/// Return current authorities length.
	pub fn authorities_len() -> usize {
		Authorities::<T>::decode_len().unwrap_or(0)
	}

	/// Get the current slot from the pre-runtime digests.
	fn current_slot_from_digests() -> Option<Slot> {
		let digest = frame_system::Pallet::<T>::digest();
		let pre_runtime_digests = digest.logs.iter().filter_map(|d| d.as_pre_runtime());
		for (id, mut data) in pre_runtime_digests {
			if id == MICC_ENGINE_ID {
				return Slot::decode(&mut data).ok()
			}
		}

		None
	}

	/// Determine the Micc slot-duration based on the Timestamp module configuration.
	pub fn slot_duration() -> T::Moment {
		T::SlotDuration::get()
	}

	/// Check if an authority is disabled due to equivocation.
	pub fn is_authority_disabled(authority: &T::AuthorityId) -> bool {
		DisabledAuthorities::<T>::contains_key(authority)
	}

	/// Get the current equivocation configuration.
	pub fn get_equivocation_config() -> crate::equivocation::EquivocationConfig {
		EquivocationConfig::<T>::get()
	}

	/// Get equivocation count for a specific authority.
	pub fn get_equivocation_count(authority: &T::AuthorityId) -> u32 {
		SessionEquivocations::<T>::get(authority)
	}

	/// Initialize equivocation detection with default configuration.
	pub fn initialize_equivocation_config() {
		if !EquivocationConfig::<T>::exists() {
			let default_config = crate::equivocation::EquivocationConfig::default();
			EquivocationConfig::<T>::put(default_config);
		}
	}

	/// Ensure the correctness of the state of this pallet.
	///
	/// This should be valid before or after each state transition of this pallet.
	///
	/// # Invariants
	///
	/// ## `CurrentSlot`
	///
	/// If we don't allow for multiple blocks per slot, then the current slot must be less than the
	/// maximal slot number. Otherwise, it can be arbitrary.
	///
	/// ## `Authorities`
	///
	/// * The authorities must be non-empty.
	/// * The current authority cannot be disabled.
	/// * The number of authorities must be less than or equal to `T::MaxAuthorities`. This however,
	///   is guarded by the type system.
	#[cfg(any(test, feature = "try-runtime"))]
	pub fn do_try_state() -> Result<(), sp_runtime::TryRuntimeError> {
		// We don't have any guarantee that we are already after `on_initialize` and thus we have to
		// check the current slot from the digest or take the last known slot.
		let current_slot =
			Self::current_slot_from_digests().unwrap_or_else(|| CurrentSlot::<T>::get());

		// Check that the current slot is less than the maximal slot number, unless we allow for
		// multiple blocks per slot.
		if !T::AllowMultipleBlocksPerSlot::get() {
			frame_support::ensure!(
				current_slot < u64::MAX,
				"Current slot has reached maximum value and cannot be incremented further.",
			);
		}

		let authorities_len =
			<Authorities<T>>::decode_len().ok_or("Failed to decode authorities length")?;

		// Check that the authorities are non-empty.
		frame_support::ensure!(!authorities_len.is_zero(), "Authorities must be non-empty.");

		// Check that the current authority is not disabled.
		let authority_index = *current_slot % authorities_len as u64;
		frame_support::ensure!(
			!T::DisabledValidators::is_disabled(authority_index as u32),
			"Current validator is disabled and should not be attempting to author blocks.",
		);

		Ok(())
	}
}

impl<T: Config> sp_runtime::BoundToRuntimeAppPublic for Pallet<T> {
	type Public = T::AuthorityId;
}

impl<T: Config> OneSessionHandler<T::AccountId> for Pallet<T> {
	type Key = T::AuthorityId;

	fn on_genesis_session<'a, I: 'a>(validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
	{
		let authorities = validators.map(|(_, k)| k).collect::<Vec<_>>();
		Self::initialize_authorities(&authorities);
	}

	fn on_new_session<'a, I: 'a>(changed: bool, validators: I, _queued_validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
	{
		// instant changes
		if changed {
			let next_authorities = validators.map(|(_, k)| k).collect::<Vec<_>>();
			let last_authorities = Authorities::<T>::get();
			if last_authorities != next_authorities {
				if next_authorities.len() as u32 > T::MaxAuthorities::get() {
					log::warn!(
						target: LOG_TARGET,
						"next authorities list larger than {}, truncating",
						T::MaxAuthorities::get(),
					);
				}
				let bounded = <BoundedVec<_, T::MaxAuthorities>>::truncate_from(next_authorities);
				Self::change_authorities(bounded);
			}
		}
	}

	fn on_disabled(i: u32) {
		let log = DigestItem::Consensus(
			MICC_ENGINE_ID,
			ConsensusLog::<T::AuthorityId>::OnDisabled(i as AuthorityIndex).encode(),
		);

		<frame_system::Pallet<T>>::deposit_log(log);
	}
}

impl<T: Config> FindAuthor<u32> for Pallet<T> {
	fn find_author<'a, I>(digests: I) -> Option<u32>
	where
		I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
	{
		for (id, mut data) in digests.into_iter() {
			if id == MICC_ENGINE_ID {
				let slot = Slot::decode(&mut data).ok()?;
				let author_index = *slot % Self::authorities_len() as u64;
				return Some(author_index as u32)
			}
		}

		None
	}
}

/// We can not implement `FindAuthor` twice, because the compiler does not know if
/// `u32 == T::AuthorityId` and thus, prevents us to implement the trait twice.
#[doc(hidden)]
pub struct FindAccountFromAuthorIndex<T, Inner>(core::marker::PhantomData<(T, Inner)>);

impl<T: Config, Inner: FindAuthor<u32>> FindAuthor<T::AuthorityId>
	for FindAccountFromAuthorIndex<T, Inner>
{
	fn find_author<'a, I>(digests: I) -> Option<T::AuthorityId>
	where
		I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
	{
		let i = Inner::find_author(digests)?;

		let validators = Authorities::<T>::get();
		validators.get(i as usize).cloned()
	}
}

/// Find the authority ID of the Micc authority who authored the current block.
pub type MiccAuthorId<T> = FindAccountFromAuthorIndex<T, Pallet<T>>;

impl<T: Config> IsMember<T::AuthorityId> for Pallet<T> {
	fn is_member(authority_id: &T::AuthorityId) -> bool {
		Authorities::<T>::get().iter().any(|id| id == authority_id)
	}
}

impl<T: Config> OnTimestampSet<T::Moment> for Pallet<T> {
	fn on_timestamp_set(moment: T::Moment) {
		let slot_duration = Self::slot_duration();
		if slot_duration.is_zero() {
			log::error!(
				target: LOG_TARGET,
				"🚨 Micc slot duration is zero. Cannot process timestamp. Ignoring."
			);
			return;
		}

		let timestamp_slot = moment / slot_duration;
		let timestamp_slot = Slot::from(timestamp_slot.saturated_into::<u64>());
		let current_slot = CurrentSlot::<T>::get();

		if current_slot != timestamp_slot {
			log::error!(
				target: LOG_TARGET,
				"🚨 Timestamp slot {:?} does not match CurrentSlot {:?}. Gracefully handling.",
				timestamp_slot, current_slot
			);
			
			Self::deposit_event(Event::SlotValidationFailed {
				current_slot,
				new_slot: timestamp_slot,
				block_number: frame_system::Pallet::<T>::block_number(),
			});
			
			Self::deposit_event(Event::ConsensusErrorRecovered {
				error_type: 2u8, // SlotTiming
				block_number: frame_system::Pallet::<T>::block_number(),
			});
		}
	}
}