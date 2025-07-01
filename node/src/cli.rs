use sc_cli::RunCmd;

#[derive(Debug, clap::Parser)]
pub struct Cli {
	#[command(subcommand)]
	pub subcommand: Option<Subcommand>,

	#[clap(flatten)]
	pub run: RunCmd,
}

#[derive(Debug, clap::Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Subcommand {
	/// Key management cli utilities
	#[command(subcommand)]
	Key(sc_cli::KeySubcommand),

	/// Build a chain specification.
	BuildSpec(sc_cli::BuildSpecCmd),

	/// Validate blocks.
	CheckBlock(sc_cli::CheckBlockCmd),

	/// Export blocks.
	ExportBlocks(sc_cli::ExportBlocksCmd),

	/// Export the state of a given block into a chain spec.
	ExportState(sc_cli::ExportStateCmd),

	/// Import blocks.
	ImportBlocks(sc_cli::ImportBlocksCmd),

	/// Remove the whole chain.
	PurgeChain(sc_cli::PurgeChainCmd),

	/// Revert the chain to a previous state.
	Revert(sc_cli::RevertCmd),

	/// Sub-commands concerned with benchmarking.
	#[command(subcommand)]
	Benchmark(frame_benchmarking_cli::BenchmarkCmd),

	/// Db meta columns information.
	ChainInfo(sc_cli::ChainInfoCmd),

	/// Rebase operations.
	#[command(subcommand)]
	Rebase(RebaseCmd),
}

#[derive(Debug, clap::Subcommand)]
pub enum RebaseCmd {
	/// Export full runtime state for rebasing.
	ExportState {
		/// Block hash to export state from. If not provided, uses latest block.
		#[arg(long)]
		at_block: Option<String>,
		/// Output file for exported state.
		#[arg(long, default_value = "state.bin")]
		output: String,
	},

	/// Export specific pallet state.
	ExportPalletState {
		/// Name of the pallet to export.
		#[arg(long)]
		pallet: String,
		/// Block hash to export state from. If not provided, uses latest block.
		#[arg(long)]
		at_block: Option<String>,
		/// Output file for exported state.
		#[arg(long, default_value = "pallet_state.bin")]
		output: String,
	},

	/// Build a new genesis spec from exported state.
	BuildGenesisSpec {
		/// Path to exported state file.
		#[arg(long)]
		state_file: String,
		/// Base chain spec file to use as template.
		#[arg(long)]
		base_spec: String,
		/// Output genesis spec file.
		#[arg(long, default_value = "rebased_genesis.json")]
		output: String,
	},

	/// Show rebase status and metadata.
	Status,
}
