use crate::{
	benchmarking::{inherent_benchmark_data, RemarkBuilder, TransferKeepAliveBuilder},
	chain_spec,
	cli::{Cli, Subcommand},
	service,
};
use frame_benchmarking_cli::{BenchmarkCmd, ExtrinsicFactory, SUBSTRATE_REFERENCE_HARDWARE};
use sc_cli::SubstrateCli;
use sc_service::PartialComponents;
use solochain_template_runtime::{Block, EXISTENTIAL_DEPOSIT};
use sp_keyring::Sr25519Keyring;
// Temporarily disabled rebase CLI commands
// use sp_api::ProvideRuntimeApi;
// use sp_rebase_api::RebaseApi;
// use std::io::Write;

impl SubstrateCli for Cli {
	fn impl_name() -> String {
		"Substrate Node".into()
	}

	fn impl_version() -> String {
		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
	}

	fn description() -> String {
		env!("CARGO_PKG_DESCRIPTION").into()
	}

	fn author() -> String {
		env!("CARGO_PKG_AUTHORS").into()
	}

	fn support_url() -> String {
		"support.anonymous.an".into()
	}

	fn copyright_start_year() -> i32 {
		2017
	}

	fn load_spec(&self, id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
		Ok(match id {
			"dev" => Box::new(chain_spec::development_chain_spec()?),
			"" | "local" => Box::new(chain_spec::local_chain_spec()?),
			"staging" => Box::new(chain_spec::staging_chain_spec()?),
			"production" => Box::new(chain_spec::production_chain_spec()?),
			path =>
				Box::new(chain_spec::ChainSpec::from_json_file(std::path::PathBuf::from(path))?),
		})
	}
}

/// Parse and run command line arguments
pub fn run() -> sc_cli::Result<()> {
	let cli = Cli::from_args();

	match &cli.subcommand {
		Some(Subcommand::Key(cmd)) => cmd.run(&cli),
		Some(Subcommand::BuildSpec(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
		},
		Some(Subcommand::CheckBlock(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, import_queue, .. } =
					service::new_partial(&config)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		},
		Some(Subcommand::ExportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, .. } = service::new_partial(&config)?;
				Ok((cmd.run(client, config.database), task_manager))
			})
		},
		Some(Subcommand::ExportState(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, .. } = service::new_partial(&config)?;
				Ok((cmd.run(client, config.chain_spec), task_manager))
			})
		},
		Some(Subcommand::ImportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, import_queue, .. } =
					service::new_partial(&config)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		},
		Some(Subcommand::PurgeChain(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.database))
		},
		Some(Subcommand::Revert(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, backend, .. } =
					service::new_partial(&config)?;
				let aux_revert = Box::new(|client, _, blocks| {
					sc_consensus_grandpa::revert(client, blocks)?;
					Ok(())
				});
				Ok((cmd.run(client, backend, Some(aux_revert)), task_manager))
			})
		},
		Some(Subcommand::Benchmark(cmd)) => {
			let runner = cli.create_runner(cmd)?;

			runner.sync_run(|config| {
				// This switch needs to be in the client, since the client decides
				// which sub-commands it wants to support.
				match cmd {
					BenchmarkCmd::Pallet(cmd) => {
						if !cfg!(feature = "runtime-benchmarks") {
							return Err(
								"Runtime benchmarking wasn't enabled when building the node. \
							You can enable it with `--features runtime-benchmarks`."
									.into(),
							);
						}

						cmd.run_with_spec::<sp_runtime::traits::HashingFor<Block>, ()>(Some(
							config.chain_spec,
						))
					},
					BenchmarkCmd::Block(cmd) => {
						let PartialComponents { client, .. } = service::new_partial(&config)?;
						cmd.run(client)
					},
					#[cfg(not(feature = "runtime-benchmarks"))]
					BenchmarkCmd::Storage(_) => Err(
						"Storage benchmarking can be enabled with `--features runtime-benchmarks`."
							.into(),
					),
					#[cfg(feature = "runtime-benchmarks")]
					BenchmarkCmd::Storage(cmd) => {
						let PartialComponents { client, backend, .. } =
							service::new_partial(&config)?;
						let db = backend.expose_db();
						let storage = backend.expose_storage();

						cmd.run(config, client, db, storage)
					},
					BenchmarkCmd::Overhead(cmd) => {
						let PartialComponents { client, .. } = service::new_partial(&config)?;
						let ext_builder = RemarkBuilder::new(client.clone());

						cmd.run(
							config.chain_spec.name().into(),
							client,
							inherent_benchmark_data()?,
							Vec::new(),
							&ext_builder,
							false,
						)
					},
					BenchmarkCmd::Extrinsic(cmd) => {
						let PartialComponents { client, .. } = service::new_partial(&config)?;
						// Register the *Remark* and *TKA* builders.
						let ext_factory = ExtrinsicFactory(vec![
							Box::new(RemarkBuilder::new(client.clone())),
							Box::new(TransferKeepAliveBuilder::new(
								client.clone(),
								Sr25519Keyring::Alice.to_account_id(),
								EXISTENTIAL_DEPOSIT,
							)),
						]);

						cmd.run(client, inherent_benchmark_data()?, Vec::new(), &ext_factory)
					},
					BenchmarkCmd::Machine(cmd) =>
						cmd.run(&config, SUBSTRATE_REFERENCE_HARDWARE.clone()),
				}
			})
		},
		Some(Subcommand::ChainInfo(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run::<Block>(&config))
		},
		Some(Subcommand::Rebase(_cmd)) => {
			eprintln!("Rebase commands are currently being implemented. Please use RPC endpoints for now.");
			Ok(())
		},
		None => {
			let runner = cli.create_runner(&cli.run)?;
			runner.run_node_until_exit(|config| async move {
				match config.network.network_backend.unwrap_or_default() {
					sc_network::config::NetworkBackendType::Libp2p => service::new_full::<
						sc_network::NetworkWorker<
							solochain_template_runtime::opaque::Block,
							<solochain_template_runtime::opaque::Block as sp_runtime::traits::Block>::Hash,
						>,
					>(config)
					.map_err(sc_cli::Error::Service),
					sc_network::config::NetworkBackendType::Litep2p =>
						service::new_full::<sc_network::Litep2pNetworkBackend>(config)
							.map_err(sc_cli::Error::Service),
				}
			})
		},
	}
}

/*
/// Run rebase subcommands (temporarily disabled)
#[allow(dead_code)]
async fn run_rebase_cmd<C>(
	cmd: &RebaseCmd,
	client: std::sync::Arc<C>,
) -> sc_cli::Result<()>
where
	C: ProvideRuntimeApi<Block> + sp_blockchain::HeaderBackend<Block> + Send + Sync + 'static,
	C::Api: RebaseApi<Block>,
{
	match cmd {
		RebaseCmd::ExportState { at_block, output } => {
			let hash = if let Some(hex_hash) = at_block {
				let bytes = hex::decode(hex_hash.trim_start_matches("0x"))
					.map_err(|e| format!("Invalid block hash: {}", e))?;
				if bytes.len() != 32 {
					return Err("Invalid block hash length".into());
				}
				let mut hash = [0u8; 32];
				hash.copy_from_slice(&bytes);
				Some(hash.into())
			} else {
				None
			};

			let api = client.runtime_api();
			let at = client.info().best_hash;

			let state_data = api.export_state(at, hash)
				.map_err(|e| format!("Failed to export state: {}", e))?
				.map_err(|e| format!("Runtime error: {:?}", e))?;

			std::fs::write(output, &state_data)
				.map_err(|e| format!("Failed to write state file: {}", e))?;

			println!("State exported to {}", output);
			println!("Exported {} bytes", state_data.len());
		},

		RebaseCmd::ExportPalletState { pallet, at_block, output } => {
			let hash = if let Some(hex_hash) = at_block {
				let bytes = hex::decode(hex_hash.trim_start_matches("0x"))
					.map_err(|e| format!("Invalid block hash: {}", e))?;
				if bytes.len() != 32 {
					return Err("Invalid block hash length".into());
				}
				let mut hash = [0u8; 32];
				hash.copy_from_slice(&bytes);
				Some(hash.into())
			} else {
				None
			};

			let api = client.runtime_api();
			let at = client.info().best_hash;

			let state_data = api.export_pallet_state(at, pallet.clone().into_bytes(), hash)
				.map_err(|e| format!("Failed to export pallet state: {}", e))?
				.map_err(|e| format!("Runtime error: {:?}", e))?;

			std::fs::write(output, &state_data)
				.map_err(|e| format!("Failed to write pallet state file: {}", e))?;

			println!("Pallet '{}' state exported to {}", pallet, output);
			println!("Exported {} bytes", state_data.len());
		},

		RebaseCmd::BuildGenesisSpec { state_file, base_spec, output } => {
			// Load the exported state
			let state_data = std::fs::read(state_file)
				.map_err(|e| format!("Failed to read state file: {}", e))?;

			// Load the base chain spec
			let base_spec_data = std::fs::read_to_string(base_spec)
				.map_err(|e| format!("Failed to read base spec: {}", e))?;

			let mut spec: serde_json::Value = serde_json::from_str(&base_spec_data)
				.map_err(|e| format!("Failed to parse base spec: {}", e))?;

			// Replace genesis runtime with exported state
			// This is a simplified implementation - real implementation would need proper state formatting
			let state_hex = hex::encode(&state_data);
			if let Some(genesis) = spec.get_mut("genesis") {
				if let Some(runtime) = genesis.get_mut("runtime") {
					runtime["system"] = serde_json::json!({
						"code": state_hex
					});
				}
			}

			// Write the new genesis spec
			let output_data = serde_json::to_string_pretty(&spec)
				.map_err(|e| format!("Failed to serialize spec: {}", e))?;

			std::fs::write(output, output_data)
				.map_err(|e| format!("Failed to write genesis spec: {}", e))?;

			println!("New genesis spec created: {}", output);
			println!("Based on state from: {}", state_file);
		},

		RebaseCmd::Status => {
			let api = client.runtime_api();
			let at = client.info().best_hash;

			let metadata = api.get_rebase_metadata(at)
				.map_err(|e| format!("Failed to get rebase metadata: {}", e))?;

			let status = api.get_rebase_status(at)
				.map_err(|e| format!("Failed to get rebase status: {}", e))?;

			let auto_enabled = api.is_auto_rebase_enabled(at)
				.map_err(|e| format!("Failed to check auto rebase: {}", e))?;

			let interval = api.get_rebase_interval(at)
				.map_err(|e| format!("Failed to get rebase interval: {}", e))?;

			println!("=== Rebase Status ===");
			println!("Current Status: {:?}", status);
			println!("Last Rebase Block: {:?}", metadata.last_rebase_block);
			println!("Total Rebases: {}", metadata.rebase_count);
			println!("Archive Count: {}", metadata.archive_count);
			println!("Auto Rebase Enabled: {}", auto_enabled);
			println!("Rebase Interval: {} blocks", interval);

			if let Some(next_block) = metadata.next_scheduled_rebase {
				println!("Next Scheduled Rebase: Block {:?}", next_block);
			}

			if let Some(next_auto) = api.get_next_rebase_block(at)
				.map_err(|e| format!("Failed to get next rebase block: {}", e))? {
				println!("Next Auto Rebase: Block {:?}", next_auto);
			}
		},
	}

	Ok(())
}
*/
