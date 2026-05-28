//! Native Rust `agent.v1` Run client (WIP). Falls back to Node until encode/exec parity.

use sinew_core::{ProviderRequest, ProviderStream, Result};

use crate::identity::CursorIdeIdentity;

use super::bridge::stream_via_node_bridge;

/// Stream via Rust HTTP/2 + prost when `SINEW_CURSOR_BRIDGE=rust`.
///
/// Phase 1: transparent fallback to the Node bridge (same behavior as default).
pub async fn stream_via_rust_bridge(
    identity: &CursorIdeIdentity,
    token: String,
    request: ProviderRequest,
) -> Result<ProviderStream> {
    tracing::warn!(
        "SINEW_CURSOR_BRIDGE=rust: native bridge not complete yet, using Node agent-bridge"
    );
    stream_via_node_bridge(identity, token, request).await
}
