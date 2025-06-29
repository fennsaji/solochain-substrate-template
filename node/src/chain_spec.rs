use sc_service::{ChainType, Properties};
use solochain_template_runtime::WASM_BINARY;

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec;

/// Environment-specific chain configuration
#[derive(Clone, Debug)]
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
