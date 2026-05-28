mod bridge;
mod client_proto;
mod connect_proto;
mod conversation_id;
mod exec_handler;
mod h2_client;
mod proto_dynamic;
mod retry;
mod proto_pool;
mod run_h2;
mod run_request;
mod rust_bridge;
mod server_decode;
mod setup;
mod state;
mod tools;
mod transcript;
#[cfg(test)]
mod models;
pub mod transport;

pub use bridge::stream_via_node_bridge;
pub use rust_bridge::stream_via_rust_bridge;

use crate::identity::CursorIdeIdentity;
use sinew_core::{ProviderRequest, ProviderStream, Result};

/// Composer `agent.v1` stream: Rust by default, Node if `SINEW_CURSOR_BRIDGE=node`.
pub async fn stream_via_agent_bridge(
    identity: &CursorIdeIdentity,
    token: String,
    request: ProviderRequest,
) -> Result<ProviderStream> {
    if transport::prefer_node_bridge() {
        stream_via_node_bridge(identity, token, request).await
    } else {
        stream_via_rust_bridge(identity, token, request).await
    }
}
pub use setup::{bridge_directory, ensure_agent_bridge_ready, set_bridge_directory};
#[cfg(test)]
pub use models::{fetch_usable_models, scan_model_ids, API2_BASE, GET_USABLE_MODELS};
