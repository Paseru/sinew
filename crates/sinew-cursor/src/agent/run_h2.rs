//! Native HTTP/2 `agent.v1.AgentService/Run` client (Connect+proto).

use std::collections::HashMap;
use std::time::Duration;

use base64::Engine as _;
use bytes::Bytes;
use http_body_util::{BodyExt, StreamBody};
use hyper::body::Frame;
use hyper::{Method, Request, StatusCode};
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use reqwest::header::HeaderMap;
use sinew_core::{AppError, Result};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::warn;

use crate::connect::decode_connect_frames;
use crate::identity::CursorIdeIdentity;

use super::connect_proto::frame_connect_proto;
use super::run_request::{build_run_request, RunRequestInput};
use super::server_decode::{decode_server_message, parse_connect_end, BridgeEvent};
use super::state::PersistedAgentConversation;
use super::transcript::TranscriptTurn;

const API2_RUN: &str = "https://api2.cursor.sh/agent.v1.AgentService/Run";
const IDLE_AFTER_TEXT_MS: u64 = 2500;
const MAX_TURN_MS: u64 = 120_000;

pub struct AgentRunConfig {
    pub token: String,
    pub model_id: String,
    pub system_prompt: String,
    pub user_text: String,
    pub conversation_id: String,
    pub history_turns: Vec<TranscriptTurn>,
    pub persisted: Option<PersistedAgentConversation>,
}

pub async fn run_agent_stream(
    identity: &CursorIdeIdentity,
    config: AgentRunConfig,
) -> Result<mpsc::Receiver<Result<BridgeEvent>>> {
    let (event_tx, event_rx) = mpsc::channel(128);
    let identity = identity.clone();
    tokio::spawn(async move {
        if let Err(err) = run_agent_stream_inner(&identity, config, event_tx.clone()).await {
            let _ = event_tx.send(Err(err)).await;
        }
    });
    Ok(event_rx)
}

async fn run_agent_stream_inner(
    identity: &CursorIdeIdentity,
    config: AgentRunConfig,
    event_tx: mpsc::Sender<Result<BridgeEvent>>,
) -> Result<()> {
    let built = build_run_request(&RunRequestInput {
        model_id: &config.model_id,
        system_prompt: &config.system_prompt,
        user_text: &config.user_text,
        conversation_id: &config.conversation_id,
        history_turns: &config.history_turns,
        persisted: config.persisted.clone(),
    })?;

    let mut blob_store = built.blob_store;
    let initial_frame = frame_connect_proto(&built.request_bytes);

    let https = HttpsConnectorBuilder::new()
        .with_native_roots()
        .map_err(|err| AppError::Network(err.to_string()))?
        .https_or_http()
        .enable_http2()
        .build();
    type UploadBody = StreamBody<ReceiverStream<Result<Frame<Bytes>, std::io::Error>>>;
    let client: Client<_, UploadBody> = Client::builder(TokioExecutor::new())
        .http2_only(true)
        .build(https);

    let (body_tx, body_rx) = mpsc::channel(64);
    let body = StreamBody::new(ReceiverStream::new(body_rx));
    let _ = body_tx
        .send(Ok(Frame::data(Bytes::from(initial_frame))))
        .await;

    let session_id = uuid::Uuid::new_v4().to_string();
    let request_id = uuid::Uuid::new_v4().to_string();
    let mut headers = HeaderMap::new();
    identity.apply_agent_authenticated(
        &mut headers,
        &session_id,
        &request_id,
        &config.token,
    );
    let mut req_builder = Request::builder()
        .method(Method::POST)
        .uri(API2_RUN)
        .header("content-type", "application/connect+proto")
        .header("te", "trailers")
        .header("connect-protocol-version", "1");
    for (name, value) in headers.iter() {
        if let Ok(val) = value.to_str() {
            req_builder = req_builder.header(name.as_str(), val);
        }
    }
    let request = req_builder
        .body(body)
        .map_err(|err| AppError::Network(err.to_string()))?;

    let response = client
        .request(request)
        .await
        .map_err(|err| AppError::Network(format!("agent Run HTTP/2: {err}")))?;

    if response.status() != StatusCode::OK {
        return Err(AppError::Network(format!(
            "agent Run failed: {}",
            response.status()
        )));
    }

    let (frame_tx, mut frame_rx) = mpsc::channel::<Vec<u8>>(64);
    let body_tx_upload = body_tx.clone();
    tokio::spawn(async move {
        while let Some(frame) = frame_rx.recv().await {
            if body_tx_upload
                .send(Ok(Frame::data(Bytes::from(frame))))
                .await
                .is_err()
            {
                break;
            }
        }
        drop(body_tx_upload);
    });

    let mut saw_text = false;
    let mut last_text_at: Option<std::time::Instant> = None;
    let mut output_tokens = 0u32;
    let mut pending = Vec::new();
    let started = std::time::Instant::now();

    let mut response_body = response.into_body();
    loop {
        let frame_result =
            tokio::time::timeout(Duration::from_millis(500), response_body.frame()).await;
        match frame_result {
            Ok(Some(Ok(frame))) => {
                if let Some(chunk) = frame.data_ref() {
                    pending.extend_from_slice(chunk);
                    while let Ok(frames) = decode_connect_frames(&mut pending) {
                        for payload in frames {
                            if payload.is_empty() {
                                continue;
                            }
                            if let Some(err) = parse_connect_end(&payload) {
                                let _ = event_tx.send(Err(AppError::Network(err))).await;
                                return Ok(());
                            }
                            match decode_server_message(&payload) {
                                Ok(events) => {
                                    for ev in handle_server_events(
                                        &events,
                                        &mut blob_store,
                                        &frame_tx,
                                    )
                                    .await?
                                    {
                                        if matches!(ev, BridgeEvent::Text(_)) {
                                            saw_text = true;
                                            last_text_at = Some(std::time::Instant::now());
                                        }
                                        if let BridgeEvent::Usage {
                                            output_tokens: o,
                                            ..
                                        } = &ev
                                        {
                                            output_tokens =
                                                output_tokens.saturating_add(*o);
                                        }
                                        if matches!(ev, BridgeEvent::ToolRequest { .. }) {
                                            return Err(AppError::Provider(
                                                "Rust agent bridge: exec/MCP loop not implemented yet"
                                                    .into(),
                                            ));
                                        }
                                        if matches!(
                                            ev,
                                            BridgeEvent::StepCompleted | BridgeEvent::TurnEnded
                                        ) {
                                            return Ok(());
                                        }
                                        if event_tx.send(Ok(ev)).await.is_err() {
                                            return Ok(());
                                        }
                                    }
                                }
                                Err(err) => {
                                    warn!("agent server decode: {err}");
                                }
                            }
                        }
                    }
                }
            }
            Ok(Some(Err(e))) => {
                return Err(AppError::Network(format!("agent Run body: {e}")));
            }
            Ok(None) => break,
            Err(_) => {
                if saw_text {
                    if let Some(at) = last_text_at {
                        if at.elapsed() >= Duration::from_millis(IDLE_AFTER_TEXT_MS) {
                            break;
                        }
                    }
                }
                if started.elapsed() >= Duration::from_millis(MAX_TURN_MS) {
                    break;
                }
            }
        }
    }

    if !saw_text {
        return Err(AppError::Network(
            "agent Run stream ended without text".into(),
        ));
    }
    Ok(())
}

async fn handle_server_events(
    events: &[BridgeEvent],
    blob_store: &mut HashMap<String, Vec<u8>>,
    frame_tx: &mpsc::Sender<Vec<u8>>,
) -> Result<Vec<BridgeEvent>> {
    let mut out = Vec::new();
    for ev in events {
        match ev {
            BridgeEvent::Checkpoint { checkpoint_b64, .. } => {
                let blobs: HashMap<String, String> = blob_store
                    .iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            base64::engine::general_purpose::STANDARD.encode(v),
                        )
                    })
                    .collect();
                out.push(BridgeEvent::Checkpoint {
                    checkpoint_b64: checkpoint_b64.clone(),
                    blobs,
                });
            }
            other => out.push(other.clone()),
        }
    }
    let _ = frame_tx;
    Ok(out)
}
