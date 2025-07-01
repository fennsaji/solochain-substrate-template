//! Configuration for the simplified off-chain worker pallet that handles automatic rebase operations.

use crate::{offchain_simple, RuntimeEvent};

impl offchain_simple::pallet::Config for crate::Runtime {
	type RuntimeEvent = RuntimeEvent;
}