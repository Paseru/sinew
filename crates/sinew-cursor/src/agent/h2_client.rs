//! Shared HTTP/2 client for `agent.v1` Run (TLS + connection reuse).

use std::sync::OnceLock;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::StreamBody;
use hyper::body::Frame;
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use sinew_core::{AppError, Result};
use tokio_stream::wrappers::ReceiverStream;

pub type AgentUploadBody = StreamBody<ReceiverStream<Result<Frame<Bytes>, std::io::Error>>>;

static H2_CLIENT: OnceLock<Client<hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>, AgentUploadBody>> =
    OnceLock::new();
static RUSTLS_PROVIDER: OnceLock<()> = OnceLock::new();

fn ensure_rustls_provider() {
    RUSTLS_PROVIDER.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

pub fn shared_h2_client(
) -> Result<&'static Client<hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>, AgentUploadBody>>
{
    ensure_rustls_provider();
    if let Some(client) = H2_CLIENT.get() {
        return Ok(client);
    }
    let https = HttpsConnectorBuilder::new()
        .with_native_roots()
        .map_err(|err| AppError::Network(err.to_string()))?
        .https_or_http()
        .enable_http2()
        .build();
    let client: Client<_, AgentUploadBody> = Client::builder(TokioExecutor::new())
        .http2_only(true)
        .pool_max_idle_per_host(2)
        .pool_idle_timeout(Duration::from_secs(90))
        .build(https);
    let _ = H2_CLIENT.set(client);
    Ok(H2_CLIENT.get().expect("h2 client init"))
}
