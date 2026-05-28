mod bridge;
mod connect_proto;
mod conversation_id;
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
pub use setup::{bridge_directory, ensure_agent_bridge_ready, set_bridge_directory};
#[cfg(test)]
pub use models::{fetch_usable_models, scan_model_ids, API2_BASE, GET_USABLE_MODELS};
