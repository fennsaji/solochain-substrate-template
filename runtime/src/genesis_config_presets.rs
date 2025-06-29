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

use crate::{AccountId, BalancesConfig, RuntimeGenesisConfig, SudoConfig, UNIT};
use alloc::{vec, vec::Vec, format, string::ToString};
use frame_support::build_struct_json_patch;
use serde_json::Value;
use sp_consensus_micc::sr25519::AuthorityId as MiccId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_genesis_builder::{self, PresetId};
use sp_keyring::Sr25519Keyring;
use sp_core::{sr25519, ed25519, Pair};

/// Production-safe configuration with reasonable token allocations
fn production_genesis(
	initial_authorities: Vec<(MiccId, GrandpaId)>,
	endowed_accounts: Vec<AccountId>,
	root: Option<AccountId>, // None to disable sudo for production
	initial_allocation: u128, // Reasonable initial allocation
) -> Value {
	let mut config = build_struct_json_patch!(RuntimeGenesisConfig {
		balances: BalancesConfig {
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, initial_allocation))
				.collect::<Vec<_>>(),
		},
		micc: pallet_micc::GenesisConfig {
			authorities: initial_authorities.iter().map(|x| (x.0.clone())).collect::<Vec<_>>(),
		},
		grandpa: pallet_grandpa::GenesisConfig {
			authorities: initial_authorities.iter().map(|x| (x.1.clone(), 1)).collect::<Vec<_>>(),
		},
	});

	// Only add sudo if a root key is provided
	if let Some(root_key) = root {
		config.as_object_mut().unwrap().insert(
			"sudo".to_string(),
			serde_json::to_value(SudoConfig { key: Some(root_key) }).unwrap()
		);
	}

	config
}

/// Generate secure production validator keys from seed phrases
/// WARNING: In real production, these should be generated offline with proper HSM
pub fn get_authority_keys_from_seed(seed: &str) -> (MiccId, GrandpaId) {
	let sr25519_pair = sr25519::Pair::from_string(&format!("//{}//micc", seed), None)
		.expect("static values are valid; qed");
	let ed25519_pair = ed25519::Pair::from_string(&format!("//{}//grandpa", seed), None)
		.expect("static values are valid; qed");
	
	(
		sr25519_pair.public().into(),
		ed25519_pair.public().into(),
	)
}

/// Generate production-ready account ID from seed
pub fn get_account_id_from_seed(seed: &str) -> AccountId {
	let pair = sr25519::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed");
	AccountId::from(pair.public())
}

/// Staging/Testing genesis configuration with more realistic but still deterministic keys
pub fn staging_config_genesis() -> Value {
	production_genesis(
		// Using deterministic but not well-known seeds for staging
		vec![
			get_authority_keys_from_seed("staging-validator-01"),
			get_authority_keys_from_seed("staging-validator-02"),
		],
		// Staging accounts with moderate allocations
		vec![
			get_account_id_from_seed("staging-alice"),
			get_account_id_from_seed("staging-bob"),
			get_account_id_from_seed("staging-charlie"),
		],
		// Staging sudo key (should be replaced with governance in production)
		Some(get_account_id_from_seed("staging-sudo")),
		// 1 million UNIT tokens per account for staging
		1_000_000 * UNIT,
	)
}

/// Production genesis configuration template
/// WARNING: This uses deterministic keys for demonstration only!
/// In real production, generate cryptographically secure keys with proper entropy!
pub fn production_config_genesis() -> Value {
	production_genesis(
		// PRODUCTION KEYS MUST BE GENERATED SECURELY OFFLINE
		// These are template keys - replace with actual production keys
		vec![
			// TODO: Replace with actual production validator keys
			get_authority_keys_from_seed("production-validator-01-REPLACE-ME"),
			get_authority_keys_from_seed("production-validator-02-REPLACE-ME"),
			get_authority_keys_from_seed("production-validator-03-REPLACE-ME"),
		],
		// Production accounts - minimal initial allocation
		vec![
			// TODO: Replace with actual production account addresses
			get_account_id_from_seed("production-treasury-REPLACE-ME"),
			get_account_id_from_seed("production-operations-REPLACE-ME"),
		],
		// CRITICAL: No sudo key for production (None = disabled)
		None, 
		// Minimal initial allocation: 100 UNIT tokens
		100 * UNIT,
	)
}

// Returns the genesis config presets populated with given parameters.
fn testnet_genesis(
	initial_authorities: Vec<(MiccId, GrandpaId)>,
	endowed_accounts: Vec<AccountId>,
	root: AccountId,
) -> Value {
	build_struct_json_patch!(RuntimeGenesisConfig {
		balances: BalancesConfig {
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, 1u128 << 60))
				.collect::<Vec<_>>(),
		},
		micc: pallet_micc::GenesisConfig {
			authorities: initial_authorities.iter().map(|x| (x.0.clone())).collect::<Vec<_>>(),
		},
		grandpa: pallet_grandpa::GenesisConfig {
			authorities: initial_authorities.iter().map(|x| (x.1.clone(), 1)).collect::<Vec<_>>(),
		},
		sudo: SudoConfig { key: Some(root) },
	})
}

/// Return the development genesis config.
pub fn development_config_genesis() -> Value {
	testnet_genesis(
		vec![(
			sp_keyring::Sr25519Keyring::Alice.public().into(),
			sp_keyring::Ed25519Keyring::Alice.public().into(),
		)],
		vec![
			Sr25519Keyring::Alice.to_account_id(),
			Sr25519Keyring::Bob.to_account_id(),
			Sr25519Keyring::AliceStash.to_account_id(),
			Sr25519Keyring::BobStash.to_account_id(),
		],
		sp_keyring::Sr25519Keyring::Alice.to_account_id(),
	)
}

/// Return the local genesis config preset.
pub fn local_config_genesis() -> Value {
	testnet_genesis(
		vec![
			(
				sp_keyring::Sr25519Keyring::Alice.public().into(),
				sp_keyring::Ed25519Keyring::Alice.public().into(),
			),
			(
				sp_keyring::Sr25519Keyring::Bob.public().into(),
				sp_keyring::Ed25519Keyring::Bob.public().into(),
			),
		],
		Sr25519Keyring::iter()
			.filter(|v| v != &Sr25519Keyring::One && v != &Sr25519Keyring::Two)
			.map(|v| v.to_account_id())
			.collect::<Vec<_>>(),
		Sr25519Keyring::Alice.to_account_id(),
	)
}

/// Provides the JSON representation of predefined genesis config for given `id`.
pub fn get_preset(id: &PresetId) -> Option<Vec<u8>> {
	let patch = match id.as_ref() {
		sp_genesis_builder::DEV_RUNTIME_PRESET => development_config_genesis(),
		sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET => local_config_genesis(),
		"staging" => staging_config_genesis(),
		"production" => production_config_genesis(),
		_ => return None,
	};
	Some(
		serde_json::to_string(&patch)
			.expect("serialization to json is expected to work. qed.")
			.into_bytes(),
	)
}

/// List of supported presets.
pub fn preset_names() -> Vec<PresetId> {
	vec![
		PresetId::from(sp_genesis_builder::DEV_RUNTIME_PRESET),
		PresetId::from(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET),
		PresetId::from("staging"),
		PresetId::from("production"),
	]
}
