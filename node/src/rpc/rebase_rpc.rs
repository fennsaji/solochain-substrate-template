//! RPC interface for rebase operations

use jsonrpsee::{
	core::{async_trait, RpcResult},
	proc_macros::rpc,
	types::{error::CALL_EXECUTION_FAILED_CODE, ErrorObject},
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_rebase_api::{RebaseApi, RebaseMetadata, RebaseStatus};
use solochain_template_runtime::{opaque::Block, BlockNumber};
use std::sync::Arc;

/// RPC interface for rebase operations
#[rpc(client, server)]
pub trait RebaseRpc {
	/// Export the full runtime state at current block
	#[method(name = "rebase_export_state")]
	async fn export_state(&self, at_block: Option<String>) -> RpcResult<String>;

	/// Export state for a specific pallet only
	#[method(name = "rebase_export_pallet_state")]
	async fn export_pallet_state(&self, pallet_name: String, at_block: Option<String>) -> RpcResult<String>;

	/// Get rebase metadata and status
	#[method(name = "rebase_get_metadata")]
	async fn get_metadata(&self) -> RpcResult<RebaseMetadata<BlockNumber>>;

	/// Get current rebase status
	#[method(name = "rebase_get_status")]
	async fn get_status(&self) -> RpcResult<RebaseStatus>;

	/// Get next automatic rebase block (if enabled)
	#[method(name = "rebase_get_next_block")]
	async fn get_next_rebase_block(&self) -> RpcResult<Option<BlockNumber>>;

	/// Check if automatic rebasing is enabled
	#[method(name = "rebase_is_auto_enabled")]
	async fn is_auto_rebase_enabled(&self) -> RpcResult<bool>;

	/// Get rebase interval in blocks
	#[method(name = "rebase_get_interval")]
	async fn get_rebase_interval(&self) -> RpcResult<u32>;

	/// Get off-chain worker heartbeat
	#[method(name = "rebase_get_heartbeat")]
	async fn get_heartbeat(&self) -> RpcResult<Option<u32>>;
}

/// Implementation of the rebase RPC interface
pub struct RebaseRpcImpl<C> {
	client: Arc<C>,
}

impl<C> RebaseRpcImpl<C> {
	/// Create new instance of the rebase RPC handler
	pub fn new(client: Arc<C>) -> Self {
		Self { client }
	}
}

#[async_trait]
impl<C> RebaseRpcServer for RebaseRpcImpl<C>
where
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: RebaseApi<Block>,
{
	async fn export_state(&self, at_block: Option<String>) -> RpcResult<String> {
		let hash = match at_block {
			Some(hex) => {
				let bytes = hex::decode(hex.trim_start_matches("0x"))
					.map_err(|e| ErrorObject::owned(CALL_EXECUTION_FAILED_CODE, "Invalid block hash", Some(e.to_string())))?;
				if bytes.len() != 32 {
					return Err(ErrorObject::owned(CALL_EXECUTION_FAILED_CODE, "Invalid block hash length", None::<()>).into());
				}
				let mut hash = [0u8; 32];
				hash.copy_from_slice(&bytes);
				Some(hash.into())
			},
			None => None,
		};

		let api = self.client.runtime_api();
		let at = self.client.info().best_hash;

		let state_data = api.export_state(at, hash)
			.map_err(|e| ErrorObject::owned(CALL_EXECUTION_FAILED_CODE, "Failed to export state", Some(e.to_string())))?
			.map_err(|e| ErrorObject::owned(CALL_EXECUTION_FAILED_CODE, "Runtime error", Some(format!("{:?}", e))))?;

		Ok(hex::encode(state_data))
	}

	async fn export_pallet_state(&self, pallet_name: String, at_block: Option<String>) -> RpcResult<String> {
		let hash = match at_block {
			Some(hex) => {
				let bytes = hex::decode(hex.trim_start_matches("0x"))
					.map_err(|e| ErrorObject::owned(CALL_EXECUTION_FAILED_CODE, "Invalid block hash", Some(e.to_string())))?;
				if bytes.len() != 32 {
					return Err(ErrorObject::owned(CALL_EXECUTION_FAILED_CODE, "Invalid block hash length", None::<()>).into());
				}
				let mut hash = [0u8; 32];
				hash.copy_from_slice(&bytes);
				Some(hash.into())
			},
			None => None,
		};

		let api = self.client.runtime_api();
		let at = self.client.info().best_hash;

		let state_data = api.export_pallet_state(at, pallet_name.into_bytes(), hash)
			.map_err(|e| ErrorObject::owned(CALL_EXECUTION_FAILED_CODE, "Failed to export pallet state", Some(e.to_string())))?
			.map_err(|e| ErrorObject::owned(CALL_EXECUTION_FAILED_CODE, "Runtime error", Some(format!("{:?}", e))))?;

		Ok(hex::encode(state_data))
	}

	async fn get_metadata(&self) -> RpcResult<RebaseMetadata<BlockNumber>> {
		let api = self.client.runtime_api();
		let at = self.client.info().best_hash;

		api.get_rebase_metadata(at)
			.map_err(|e| ErrorObject::owned(CALL_EXECUTION_FAILED_CODE, "Failed to get rebase metadata", Some(e.to_string())).into())
	}

	async fn get_status(&self) -> RpcResult<RebaseStatus> {
		let api = self.client.runtime_api();
		let at = self.client.info().best_hash;

		api.get_rebase_status(at)
			.map_err(|e| ErrorObject::owned(CALL_EXECUTION_FAILED_CODE, "Failed to get rebase status", Some(e.to_string())).into())
	}

	async fn get_next_rebase_block(&self) -> RpcResult<Option<BlockNumber>> {
		let api = self.client.runtime_api();
		let at = self.client.info().best_hash;

		api.get_next_rebase_block(at)
			.map_err(|e| ErrorObject::owned(CALL_EXECUTION_FAILED_CODE, "Failed to get next rebase block", Some(e.to_string())).into())
	}

	async fn is_auto_rebase_enabled(&self) -> RpcResult<bool> {
		let api = self.client.runtime_api();
		let at = self.client.info().best_hash;

		api.is_auto_rebase_enabled(at)
			.map_err(|e| ErrorObject::owned(CALL_EXECUTION_FAILED_CODE, "Failed to check auto rebase status", Some(e.to_string())).into())
	}

	async fn get_rebase_interval(&self) -> RpcResult<u32> {
		let api = self.client.runtime_api();
		let at = self.client.info().best_hash;

		api.get_rebase_interval(at)
			.map_err(|e| ErrorObject::owned(CALL_EXECUTION_FAILED_CODE, "Failed to get rebase interval", Some(e.to_string())).into())
	}

	async fn get_heartbeat(&self) -> RpcResult<Option<u32>> {
		let api = self.client.runtime_api();
		let at = self.client.info().best_hash;

		api.get_heartbeat(at)
			.map_err(|e| ErrorObject::owned(CALL_EXECUTION_FAILED_CODE, "Failed to get heartbeat", Some(e.to_string())).into())
	}
}