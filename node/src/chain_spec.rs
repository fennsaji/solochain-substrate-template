use sc_service::{ChainType, Properties};
use solochain_template_runtime::WASM_BINARY;
use serde_json::{Value, json};

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec;

/// Environment-specific chain configuration
#[derive(Clone, Debug, Copy)]
pub enum Environment {
	Development,
	Local,
	Staging, 
	Production,
}

pub fn development_chain_spec() -> Result<ChainSpec, String> {
	Ok(ChainSpec::builder(
		WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
		None,
	)
	.with_name("Development")
	.with_id("dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_preset_name(sp_genesis_builder::DEV_RUNTIME_PRESET)
	.build())
}

pub fn local_chain_spec() -> Result<ChainSpec, String> {
	Ok(ChainSpec::builder(
		WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
		None,
	)
	.with_name("Local Testnet")
	.with_id("local_testnet")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_preset_name(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET)
	.build())
}

/// Staging chain specification with secure-ish keys but still deterministic for testing
pub fn staging_chain_spec() -> Result<ChainSpec, String> {
	Ok(ChainSpec::builder(
		WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
		None,
	)
	.with_name("Staging Network")
	.with_id("staging")
	.with_chain_type(ChainType::Live)
	.with_genesis_config_preset_name("staging")
	.with_properties({
		let mut props = Properties::new();
		props.insert("tokenDecimals".into(), 12.into());
		props.insert("tokenSymbol".into(), "UNIT".into());
		props.insert("ss58Format".into(), 42.into()); // TODO: Register unique prefix
		props
	})
	.build())
}

/// Production chain specification template
/// WARNING: This template uses placeholder keys that MUST be replaced with actual production keys!
pub fn production_chain_spec() -> Result<ChainSpec, String> {
	Ok(ChainSpec::builder(
		WASM_BINARY.ok_or_else(|| "Production wasm not available".to_string())?,
		None,
	)
	.with_name("Solochain Production Network")
	.with_id("solochain-mainnet")
	.with_chain_type(ChainType::Live)
	.with_genesis_config_preset_name("production")
	.with_properties({
		let mut props = Properties::new();
		props.insert("tokenDecimals".into(), 12.into());
		props.insert("tokenSymbol".into(), "UNIT".into());
		props.insert("ss58Format".into(), 42.into()); // TODO: Register unique SS58 prefix
		props
	})
	.build())
}

/// Get chain specification based on environment
pub fn get_chain_spec(env: Environment) -> Result<ChainSpec, String> {
	match env {
		Environment::Development => development_chain_spec(),
		Environment::Local => local_chain_spec(),
		Environment::Staging => staging_chain_spec(),
		Environment::Production => production_chain_spec(),
	}
}

/// Parse environment from string
pub fn parse_environment(env: &str) -> Result<Environment, String> {
	match env.to_lowercase().as_str() {
		"dev" | "development" => Ok(Environment::Development),
		"local" => Ok(Environment::Local),
		"staging" => Ok(Environment::Staging),
		"prod" | "production" => Ok(Environment::Production),
		_ => Err(format!("Unknown environment: {}. Valid options: dev, local, staging, production", env)),
	}
}

/// Rebase-specific functionality for creating new genesis from exported state
pub mod rebase {
	use super::*;
	use sp_core::H256;
	use sp_runtime::traits::{BlakeTwo256, Hash};

	/// Rebase metadata for tracking genesis history
	#[derive(Debug, Clone)]
	pub struct RebaseMetadata {
		pub original_genesis_hash: H256,
		pub rebase_height: u32,
		pub rebase_timestamp: u64,
		pub state_root: H256,
		pub rebase_version: u32,
	}

	/// Create a new chain spec from exported state
	pub fn create_rebased_chain_spec(
		base_spec_path: &str,
		state_data: &[u8],
		rebase_metadata: Option<RebaseMetadata>,
		output_name: Option<String>,
	) -> Result<ChainSpec, String> {
		// Load the base chain spec
		let base_spec_content = std::fs::read_to_string(base_spec_path)
			.map_err(|e| format!("Failed to read base spec: {}", e))?;

		let mut spec_json: Value = serde_json::from_str(&base_spec_content)
			.map_err(|e| format!("Failed to parse base spec: {}", e))?;

		// Validate the exported state
		let state_root = validate_exported_state(state_data)?;

		// Update the spec with new genesis state
		update_genesis_with_exported_state(&mut spec_json, state_data, state_root)?;

		// Add rebase metadata if provided
		if let Some(metadata) = rebase_metadata {
			add_rebase_metadata(&mut spec_json, metadata)?;
		}

		// Update name if provided
		if let Some(name) = output_name {
			spec_json["name"] = json!(name);
			spec_json["id"] = json!(format!("{}-rebased", name.to_lowercase().replace(" ", "-")));
		}

		// Convert back to ChainSpec
		let updated_spec_str = serde_json::to_string_pretty(&spec_json)
			.map_err(|e| format!("Failed to serialize updated spec: {}", e))?;

		// Create a temporary file and load it as ChainSpec
		let temp_path = format!("/tmp/rebased_spec_{}.json", chrono::Utc::now().timestamp());
		std::fs::write(&temp_path, updated_spec_str)
			.map_err(|e| format!("Failed to write temp spec: {}", e))?;

		let rebased_spec = ChainSpec::from_json_file(std::path::PathBuf::from(&temp_path))
			.map_err(|e| format!("Failed to load rebased spec: {}", e))?;

		// Clean up temp file
		let _ = std::fs::remove_file(&temp_path);

		Ok(rebased_spec)
	}

	/// Validate exported state and return state root
	fn validate_exported_state(state_data: &[u8]) -> Result<H256, String> {
		if state_data.is_empty() {
			return Err("State data is empty".to_string());
		}

		// For now, compute a simple hash of the state data
		// In production, this would validate the actual state trie
		let state_root = BlakeTwo256::hash(state_data);
		Ok(state_root)
	}

	/// Update genesis configuration with exported state
	fn update_genesis_with_exported_state(
		spec_json: &mut Value,
		state_data: &[u8],
		state_root: H256,
	) -> Result<(), String> {
		// Get or create genesis section
		let genesis = spec_json.get_mut("genesis")
			.ok_or("No genesis section in spec")?;

		// Update runtime section with new state
		if let Some(runtime) = genesis.get_mut("runtimeGenesis") {
			// Store the exported state as a custom field
			runtime["rebasedState"] = json!({
				"stateRoot": format!("0x{}", hex::encode(state_root.as_bytes())),
				"stateData": hex::encode(state_data),
				"timestamp": chrono::Utc::now().timestamp()
			});
		} else {
			// Create runtime genesis section if it doesn't exist
			genesis["runtimeGenesis"] = json!({
				"rebasedState": {
					"stateRoot": format!("0x{}", hex::encode(state_root.as_bytes())),
					"stateData": hex::encode(state_data),
					"timestamp": chrono::Utc::now().timestamp()
				}
			});
		}

		Ok(())
	}

	/// Add rebase metadata to the chain spec
	fn add_rebase_metadata(
		spec_json: &mut Value,
		metadata: RebaseMetadata,
	) -> Result<(), String> {
		// Add rebase information to properties
		if !spec_json.as_object().map(|obj| obj.contains_key("properties")).unwrap_or(false) {
			spec_json["properties"] = json!({});
		}
		let properties = spec_json.get_mut("properties")
			.ok_or("Failed to create properties section")?;

		properties["rebaseMetadata"] = json!({
			"originalGenesisHash": format!("0x{}", hex::encode(metadata.original_genesis_hash.as_bytes())),
			"rebaseHeight": metadata.rebase_height,
			"rebaseTimestamp": metadata.rebase_timestamp,
			"stateRoot": format!("0x{}", hex::encode(metadata.state_root.as_bytes())),
			"rebaseVersion": metadata.rebase_version,
			"isRebased": true
		});

		Ok(())
	}

	/// Create a rebased chain spec with automatic naming
	pub fn create_auto_rebased_spec(
		base_env: Environment,
		state_data: &[u8],
		rebase_height: u32,
	) -> Result<ChainSpec, String> {
		let base_spec = get_chain_spec(base_env.clone())?;
		
		// Generate temporary spec file
		let temp_base_path = format!("/tmp/base_spec_{}.json", chrono::Utc::now().timestamp());
		let base_spec_json = serde_json::to_string_pretty(&base_spec.as_json(false))
			.map_err(|e| format!("Failed to serialize base spec: {}", e))?;
		
		std::fs::write(&temp_base_path, base_spec_json)
			.map_err(|e| format!("Failed to write temp base spec: {}", e))?;

		let metadata = RebaseMetadata {
			original_genesis_hash: H256::zero(), // Would be actual genesis hash in production
			rebase_height,
			rebase_timestamp: chrono::Utc::now().timestamp() as u64,
			state_root: validate_exported_state(state_data)?,
			rebase_version: 1,
		};

		let env_name = match base_env {
			Environment::Development => "Development Rebased",
			Environment::Local => "Local Testnet Rebased", 
			Environment::Staging => "Staging Network Rebased",
			Environment::Production => "Production Network Rebased",
		};

		let result = create_rebased_chain_spec(
			&temp_base_path,
			state_data,
			Some(metadata),
			Some(env_name.to_string()),
		);

		// Clean up temp file
		let _ = std::fs::remove_file(&temp_base_path);

		result
	}

	/// Validate a rebased chain spec
	pub fn validate_rebased_spec(spec: &ChainSpec) -> Result<RebaseMetadata, String> {
		let spec_json = spec.as_json(false)
			.map_err(|e| format!("Failed to get spec JSON: {}", e))?;
		
		let spec_value: Value = serde_json::from_str(&spec_json)
			.map_err(|e| format!("Failed to parse spec JSON: {}", e))?;
		
		let properties = spec_value.get("properties")
			.ok_or("No properties section in rebased spec")?;
			
		let rebase_metadata = properties.get("rebaseMetadata")
			.ok_or("No rebase metadata found in spec")?;

		let is_rebased = rebase_metadata.get("isRebased")
			.and_then(|v| v.as_bool())
			.unwrap_or(false);

		if !is_rebased {
			return Err("Chain spec is not marked as rebased".to_string());
		}

		// Extract metadata
		let original_genesis_hash = rebase_metadata.get("originalGenesisHash")
			.and_then(|v| v.as_str())
			.and_then(|s| hex::decode(s.trim_start_matches("0x")).ok())
			.and_then(|bytes| if bytes.len() == 32 {
				let mut hash = [0u8; 32];
				hash.copy_from_slice(&bytes);
				Some(H256::from(hash))
			} else {
				None
			})
			.ok_or("Invalid originalGenesisHash")?;

		let rebase_height = rebase_metadata.get("rebaseHeight")
			.and_then(|v| v.as_u64())
			.map(|v| v as u32)
			.ok_or("Invalid rebaseHeight")?;

		let rebase_timestamp = rebase_metadata.get("rebaseTimestamp")
			.and_then(|v| v.as_u64())
			.ok_or("Invalid rebaseTimestamp")?;

		let state_root = rebase_metadata.get("stateRoot")
			.and_then(|v| v.as_str())
			.and_then(|s| hex::decode(s.trim_start_matches("0x")).ok())
			.and_then(|bytes| if bytes.len() == 32 {
				let mut hash = [0u8; 32];
				hash.copy_from_slice(&bytes);
				Some(H256::from(hash))
			} else {
				None
			})
			.ok_or("Invalid stateRoot")?;

		let rebase_version = rebase_metadata.get("rebaseVersion")
			.and_then(|v| v.as_u64())
			.map(|v| v as u32)
			.ok_or("Invalid rebaseVersion")?;

		Ok(RebaseMetadata {
			original_genesis_hash,
			rebase_height,
			rebase_timestamp,
			state_root,
			rebase_version,
		})
	}
}
