//! Runtime configuration module with environment-specific parameters.
//!
//! This module provides the core runtime configuration along with environment-specific
//! parameter overrides for development, staging, and production deployments.

// This is free and unencumbered software released into the public domain.
//
// Anyone is free to copy, modify, publish, use, compile, sell, or
// distribute this software, either in source code form or as a compiled
// binary, for any purpose, commercial or non-commercial, and by any
// means.
//
// In jurisdictions that recognize copyright laws, the author or authors
// of this software dedicate any and all copyright interest in the
// software to the public domain. We make this dedication for the benefit
// of the public at large and to the detriment of our heirs and
// successors. We intend this dedication to be an overt act of
// relinquishment in perpetuity of all present and future rights to this
// software under copyright law.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
// OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
// ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
// OTHER DEALINGS IN THE SOFTWARE.
//
// For more information, please refer to <http://unlicense.org>

pub mod environments;

// Substrate and Polkadot dependencies
use frame_support::{
	derive_impl, parameter_types,
	traits::{ConstBool, ConstU128, ConstU32, ConstU64, VariantCountOf},
	weights::{
		constants::{RocksDbWeight, WEIGHT_REF_TIME_PER_SECOND},
		Weight,
	},
};
use frame_system::limits::{BlockLength, BlockWeights};
use sp_consensus_micc::sr25519::AuthorityId as MiccId;
use sp_runtime::Perbill;
use sp_version::RuntimeVersion;

// Local module imports
use super::{
	AccountId, Micc, Balance, Block, BlockNumber, Hash, Nonce, PalletInfo, Runtime,
	RuntimeCall, RuntimeEvent, RuntimeFreezeReason, RuntimeHoldReason, RuntimeOrigin, RuntimeTask,
	System, EXISTENTIAL_DEPOSIT, SLOT_DURATION, VERSION,
};

// Import environment-specific configuration
use environments::*;

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

parameter_types! {
	/// Environment-specific block hash count
	/// Development: 250 blocks (2 minutes at 500ms)
	/// Local: 1200 blocks (10 minutes)  
	/// Staging/Production: 2400+ blocks (20+ minutes)
	pub const BlockHashCount: BlockNumber = CONSENSUS_BLOCK_HASH_COUNT;
	pub const Version: RuntimeVersion = VERSION;

	/// We allow for 2 seconds of compute with a 6 second average block time.
	/// Note: Comment reflects original 6s blocks, but we use 500ms blocks
	pub RuntimeBlockWeights: BlockWeights = BlockWeights::with_sensible_defaults(
		Weight::from_parts(WEIGHT_REF_TIME_PER_SECOND / 2, u64::MAX),
		NORMAL_DISPATCH_RATIO,
	);
	pub RuntimeBlockLength: BlockLength = BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	
	/// Environment-specific SS58 address prefix
	/// Development/Local: 42 (generic Substrate prefix)
	/// Staging/Production: Unique registered prefix (TODO: register and update)
	/// For production, register a unique prefix at: https://github.com/paritytech/ss58-registry
	pub const SS58Prefix: u8 = NETWORK_SS58_PREFIX;
}

/// The default types are being injected by [`derive_impl`](`frame_support::derive_impl`) from
/// [`SoloChainDefaultConfig`](`struct@frame_system::config_preludes::SolochainDefaultConfig`),
/// but overridden as needed.
#[derive_impl(frame_system::config_preludes::SolochainDefaultConfig)]
impl frame_system::Config for Runtime {
	/// The block type for the runtime.
	type Block = Block;
	/// Block & extrinsics weights: base values and limits.
	type BlockWeights = RuntimeBlockWeights;
	/// The maximum length of a block (in bytes).
	type BlockLength = RuntimeBlockLength;
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The type for storing how many extrinsics an account has signed.
	type Nonce = Nonce;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	type BlockHashCount = BlockHashCount;
	/// The weight of database operations that the runtime can invoke.
	type DbWeight = RocksDbWeight;
	/// Version of the runtime.
	type Version = Version;
	/// The data to be stored in an account.
	type AccountData = pallet_balances::AccountData<Balance>;
	/// This is used as an identifier of the chain. Environment-specific prefix.
	type SS58Prefix = SS58Prefix;
	/// Environment-specific max consumers (16 for dev/local, 32 for staging/production)
	type MaxConsumers = frame_support::traits::ConstU32<NETWORK_MAX_CONSUMERS>;
}

impl pallet_micc::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AuthorityId = MiccId;
	type DisabledValidators = ();
	/// Environment-specific max authorities
	/// Development: 10, Local: 5, Staging: 21, Production: 32
	type MaxAuthorities = ConstU32<CONSENSUS_MAX_AUTHORITIES>;
	/// Environment-specific multiple blocks per slot
	/// Development: true (flexible), Others: false (secure)
	type AllowMultipleBlocksPerSlot = ConstBool<CONSENSUS_ALLOW_MULTIPLE_BLOCKS>;
	type SlotDuration = pallet_micc::MinimumPeriodTimesTwo<Runtime>;
}

impl pallet_grandpa::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;

	type WeightInfo = ();
	/// Environment-specific max authorities (matches MICC configuration)
	type MaxAuthorities = ConstU32<CONSENSUS_MAX_AUTHORITIES>;
	type MaxNominators = ConstU32<0>;
	type MaxSetIdSessionEntries = ConstU64<0>;

	type KeyOwnerProof = sp_core::Void;
	type EquivocationReportSystem = ();
}

impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = Micc;
	type MinimumPeriod = ConstU64<{ SLOT_DURATION / 2 }>;
	type WeightInfo = ();
}

impl pallet_balances::Config for Runtime {
	type MaxLocks = ConstU32<50>;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU128<EXISTENTIAL_DEPOSIT>;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
	type FreezeIdentifier = RuntimeFreezeReason;
	type MaxFreezes = VariantCountOf<RuntimeFreezeReason>;
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type DoneSlashHandler = ();
}


impl pallet_sudo::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type WeightInfo = pallet_sudo::weights::SubstrateWeight<Runtime>;
}

impl pallet_rate_limiter::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = pallet_balances::Pallet<Runtime>;
	/// Environment-specific default transactions per block
	/// Development: 1000, Local: 500, Staging: 200, Production: 100
	type DefaultTransactionsPerBlock = ConstU32<RATE_LIMIT_DEFAULT_TXS_PER_BLOCK>;
	/// Environment-specific default transactions per minute
	/// Development: 6000, Local: 3000, Staging: 1200, Production: 600
	type DefaultTransactionsPerMinute = ConstU32<RATE_LIMIT_DEFAULT_TXS_PER_MINUTE>;
	/// Minimum balance of 0 UNIT required to submit transactions (fee-free system)
	type MinimumBalance = ConstU128<{ 0 * super::UNIT }>;
	/// Environment-specific max transactions per account in pool
	/// Development: 1000, Local: 500, Staging: 200, Production: 100
	type MaxTransactionsPerAccount = ConstU32<RATE_LIMIT_MAX_TXS_PER_ACCOUNT>;
	/// Environment-specific max bytes per account in transaction pool
	/// Development: 2MB, Local: 1MB, Staging/Production: 512KB
	type MaxBytesPerAccount = ConstU32<RATE_LIMIT_MAX_BYTES_PER_ACCOUNT>;
	/// Environment-specific max transactions per minute per account
	/// Development: 600, Local: 300, Staging: 120, Production: 60
	type MaxTransactionsPerMinute = ConstU32<RATE_LIMIT_MAX_TXS_PER_MINUTE>;
}
