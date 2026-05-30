//! Native HTTP/2 `agent.v1.AgentService/Run` client (Connect+proto).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, StreamBody};
use hyper::body::Frame;
use hyper::{Method, Request, StatusCode};
use reqwest::header::HeaderMap;
use sinew_core::{AppError, Result};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, warn};

use crate::connect::decode_connect_frames;
use crate::identity::CursorIdeIdentity;

use super::client_proto::{encode_client_heartbeat, encode_kv_get_blob_result, encode_kv_set_blob_result};
use super::connect_proto::frame_connect_proto;
use super::h2_client::{shared_h2_client, AgentUploadBody};
use super::retry::{
    backoff_before_retry, is_retryable_network_err, is_retryable_status, MAX_RUN_ATTEMPTS,
};
use super::exec_handler::{
    encode_mcp_result, handle_exec_server_message, ExecContext, ExecOutcome, PendingToolRequest,
};
use super::proto_dynamic::get_message_field;
use super::run_request::{build_run_request, RunRequestInput};
use super::server_decode::{
    decode_agent_server_message, decode_server_message, parse_connect_end, BridgeEvent,
};
use super::state::PersistedAgentConversation;
use super::transcript::TranscriptTurn;

const API2_RUN: &str = "https://agent.api5.cursor.sh/agent.v1.AgentService/Run";
const IDLE_AFTER_TEXT_MS: u64 = 2500;
const MAX_TURN_MS: u64 = 120_000;
const HEARTBEAT_INTERVAL_MS: u64 = 15_000;

#[derive(Debug, Clone)]
pub struct ToolResponse {
    pub content: String,
    pub is_error: bool,
}

pub struct AgentRunConfig {
    pub token: String,
    pub model_id: String,
    pub system_prompt: String,
    pub user_text: String,
    pub conversation_id: String,
    pub history_turns: Vec<TranscriptTurn>,
    pub persisted: Option<PersistedAgentConversation>,
    pub workspace_root: String,
    pub tools: Vec<serde_json::Value>,
    pub workspace_snapshot: Option<serde_json::Value>,
}

pub struct AgentRunHandle {
    pub events: mpsc::Receiver<Result<BridgeEvent>>,
    pub tool_responses: mpsc::Sender<ToolResponse>,
}

pub async fn run_agent_stream(
    identity: &CursorIdeIdentity,
    config: AgentRunConfig,
) -> Result<AgentRunHandle> {
    let (event_tx, event_rx) = mpsc::channel(128);
    let (tool_response_tx, tool_response_rx) = mpsc::channel(32);
    let identity = identity.clone();
    tokio::spawn(async move {
        if let Err(err) =
            run_agent_stream_inner(&identity, config, event_tx.clone(), tool_response_rx).await
        {
            let _ = event_tx.send(Err(err)).await;
        }
    });
    Ok(AgentRunHandle {
        events: event_rx,
        tool_responses: tool_response_tx,
    })
}

async fn run_agent_stream_inner(
    identity: &CursorIdeIdentity,
    config: AgentRunConfig,
    event_tx: mpsc::Sender<Result<BridgeEvent>>,
    mut tool_response_rx: mpsc::Receiver<ToolResponse>,
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

    let client = shared_h2_client()?;

    let session_id = uuid::Uuid::new_v4().to_string();
    let request_id = uuid::Uuid::new_v4().to_string();
    let mut headers = HeaderMap::new();
    identity.apply_agent_authenticated(
        &mut headers,
        &session_id,
        &request_id,
        &config.token,
    );

    let mut response = None;
    let mut body_tx = None;
    let mut last_err: Option<AppError> = None;
    for attempt in 0..MAX_RUN_ATTEMPTS {
        if attempt > 0 {
            tokio::time::sleep(backoff_before_retry(attempt - 1)).await;
        }
        let (upload_tx, upload_rx) = mpsc::channel(64);
        let body: AgentUploadBody = StreamBody::new(ReceiverStream::new(upload_rx));
        let _ = upload_tx
            .send(Ok(Frame::data(Bytes::from(initial_frame.clone()))))
            .await;

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

        match client.request(request).await {
            Ok(resp) if resp.status() == StatusCode::OK => {
                response = Some(resp);
                body_tx = Some(upload_tx);
                break;
            }
            Ok(resp) if is_retryable_status(resp.status()) && attempt + 1 < MAX_RUN_ATTEMPTS => {
                warn!(
                    "agent Run HTTP {} — retry {}/{}",
                    resp.status(),
                    attempt + 1,
                    MAX_RUN_ATTEMPTS
                );
                last_err = Some(AppError::Network(format!(
                    "agent Run failed: {}",
                    resp.status()
                )));
            }
            Ok(resp) => {
                return Err(AppError::Network(format!(
                    "agent Run failed: {}",
                    resp.status()
                )));
            }
            Err(err) => {
                let net = AppError::Network(format!("agent Run HTTP/2: {err}"));
                if is_retryable_network_err(&net) && attempt + 1 < MAX_RUN_ATTEMPTS {
                    warn!("agent Run network error — retry {}/{}", attempt + 1, MAX_RUN_ATTEMPTS);
                    last_err = Some(net);
                } else {
                    return Err(net);
                }
            }
        }
    }
    let response = response.ok_or_else(|| {
        last_err.unwrap_or_else(|| AppError::Network("agent Run failed after retries".into()))
    })?;
    let body_tx = body_tx.expect("body_tx set with successful response");

    let (frame_tx, mut frame_rx) = mpsc::channel::<Vec<u8>>(64);
    let body_tx_upload = body_tx;
    let upload_done = Arc::new(tokio::sync::Notify::new());
    let upload_done_worker = upload_done.clone();
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
        upload_done_worker.notify_waiters();
    });

    let frame_tx_hb = frame_tx.clone();
    let hb_stop = tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_millis(HEARTBEAT_INTERVAL_MS));
        loop {
            tick.tick().await;
            match encode_client_heartbeat() {
                Ok(bytes) => {
                    if frame_tx_hb.send(bytes).await.is_err() {
                        break;
                    }
                }
                Err(err) => {
                    warn!("heartbeat encode: {err}");
                }
            }
        }
    });

    let exec_ctx = ExecContext {
        workspace_root: &config.workspace_root,
        tools: &config.tools,
        workspace_snapshot: config.workspace_snapshot.as_ref(),
    };

    let mut saw_text = false;
    let mut saw_thinking = false;
    let mut last_text_at: Option<std::time::Instant> = None;
    let mut output_tokens = 0u32;
    let mut pending = Vec::new();
    let started = std::time::Instant::now();
    let mut finished = false;

    let mut response_body = response.into_body();
    while !finished {
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
                                finished = true;
                                break;
                            }
                            if process_server_payload(
                                &payload,
                                &mut blob_store,
                                &frame_tx,
                                &event_tx,
                                &exec_ctx,
                                &mut tool_response_rx,
                                &mut saw_text,
                                &mut saw_thinking,
                                &mut last_text_at,
                                &mut output_tokens,
                                &mut finished,
                            )
                            .await?
                            {
                                break;
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
                if saw_text || saw_thinking {
                    if let Some(at) = last_text_at {
                        if at.elapsed() >= Duration::from_millis(IDLE_AFTER_TEXT_MS) {
                            break;
                        }
                    } else if saw_thinking && started.elapsed() >= Duration::from_millis(IDLE_AFTER_TEXT_MS) {
                        break;
                    }
                }
                if started.elapsed() >= Duration::from_millis(MAX_TURN_MS) {
                    break;
                }
            }
        }
    }

    hb_stop.abort();
    drop(frame_tx);
    let _ = tokio::time::timeout(Duration::from_secs(2), upload_done.notified()).await;

    if !saw_text && !saw_thinking {
        return Err(AppError::Network(
            "agent Run stream ended without text".into(),
        ));
    }
    let duration_ms = started.elapsed().as_millis();
    debug!(duration_ms, output_tokens, "cursor h2 agent stream finished");
    Ok(())
}

async fn process_server_payload(
    payload: &[u8],
    blob_store: &mut HashMap<String, Vec<u8>>,
    frame_tx: &mpsc::Sender<Vec<u8>>,
    event_tx: &mpsc::Sender<Result<BridgeEvent>>,
    exec_ctx: &ExecContext<'_>,
    tool_response_rx: &mut mpsc::Receiver<ToolResponse>,
    saw_text: &mut bool,
    saw_thinking: &mut bool,
    last_text_at: &mut Option<std::time::Instant>,
    output_tokens: &mut u32,
    finished: &mut bool,
) -> Result<bool> {
    let msg = decode_agent_server_message(payload)?;

    if let Some(exec) = get_message_field(&msg, "exec_server_message") {
        match handle_exec_server_message(&exec, exec_ctx).await? {
            Some(ExecOutcome::Frame(bytes)) => {
                let _ = frame_tx.send(bytes).await;
            }
            Some(ExecOutcome::ToolRequest(PendingToolRequest {
                exec_id,
                exec_msg_id,
                tool_name,
                tool_call_id,
                args,
            })) => {
                let _ = event_tx
                    .send(Ok(BridgeEvent::ToolRequest {
                        exec_id: exec_id.clone(),
                        exec_msg_id: exec_msg_id.clone(),
                        tool_call_id: tool_call_id.clone(),
                        tool_name: tool_name.clone(),
                        args: args.clone(),
                    }))
                    .await;
                let resp = tool_response_rx
                    .recv()
                    .await
                    .unwrap_or(ToolResponse {
                        content: "Error: empty tool response".into(),
                        is_error: true,
                    });
                let id = exec_msg_id.parse::<u32>().unwrap_or(0);
                let bytes = encode_mcp_result(&exec_id, id, &resp.content, resp.is_error)?;
                let _ = frame_tx.send(bytes).await;
            }
            None => {}
        }
        return Ok(false);
    }

    if let Some(kv) = get_message_field(&msg, "kv_server_message") {
        handle_kv_message(&kv, blob_store, frame_tx).await?;
        return Ok(false);
    }

    if let Some(checkpoint) = get_message_field(&msg, "conversation_checkpoint_update") {
        use prost::Message as _;
        let bytes = checkpoint.encode_to_vec();
        use base64::Engine as _;
        let checkpoint_b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        let blobs: HashMap<String, String> = blob_store
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    base64::engine::general_purpose::STANDARD.encode(v),
                )
            })
            .collect();
        let _ = event_tx
            .send(Ok(BridgeEvent::Checkpoint {
                checkpoint_b64,
                blobs,
            }))
            .await;
        return Ok(false);
    }

    match decode_server_message(payload) {
        Ok(events) => {
            for ev in events {
                match &ev {
                    BridgeEvent::Text(_) => {
                        *saw_text = true;
                        *last_text_at = Some(std::time::Instant::now());
                    }
                    BridgeEvent::Thinking(_) => {
                        *saw_thinking = true;
                        *last_text_at = Some(std::time::Instant::now());
                    }
                    BridgeEvent::Usage { output_tokens: o, .. } => {
                        *output_tokens = output_tokens.saturating_add(*o);
                    }
                    BridgeEvent::StepCompleted | BridgeEvent::TurnEnded
                        if (*saw_text || *saw_thinking) => {
                            *finished = true;
                            return Ok(true);
                        }
                    _ => {}
                }
                if event_tx.send(Ok(ev)).await.is_err() {
                    *finished = true;
                    return Ok(true);
                }
            }
        }
        Err(err) => warn!("interaction decode: {err}"),
    }
    Ok(false)
}

async fn handle_kv_message(
    kv: &prost_reflect::DynamicMessage,
    blob_store: &mut HashMap<String, Vec<u8>>,
    frame_tx: &mpsc::Sender<Vec<u8>>,
) -> Result<()> {
    use super::proto_dynamic::{get_bytes_field, get_u32_field, oneof_case};
    let id = get_u32_field(kv, "id").unwrap_or(0);
    match oneof_case(kv).as_deref() {
        Some("get_blob_args") => {
            let blob_id = get_message_field(kv, "get_blob_args")
                .and_then(|args| get_bytes_field(&args, "blob_id"))
                .or_else(|| get_bytes_field(kv, "blob_id"))
                .unwrap_or_default();
            let key = hex::encode(&blob_id);
            let data = blob_store.get(&key).cloned();
            let bytes = encode_kv_get_blob_result(id, data)?;
            let _ = frame_tx.send(bytes).await;
        }
        Some("set_blob_args") => {
            if let (Some(blob_id), Some(blob_data)) = (
                get_bytes_field(kv, "blob_id"),
                get_bytes_field(kv, "blob_data"),
            ) {
                blob_store.insert(hex::encode(&blob_id), blob_data);
            }
            let bytes = encode_kv_set_blob_result(id)?;
            let _ = frame_tx.send(bytes).await;
        }
        _ => {}
    }
    Ok(())
}
