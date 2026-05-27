use std::sync::Mutex;
use std::time::Instant;

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use sinew_core::{
    AppError, ModelCapabilities, ModelRef, Provider, ProviderRequest, ProviderStream, Result,
    PartKind, StopReason, StreamEvent, TokenEstimate, ToolCallIntro, Usage,
};

use crate::{
    auth::composer::{ensure_fresh_composer_token, load_composer_session, ComposerSession},
    connect::{decode_connect_frames, parse_connect_events, ComposerEvent},
    conversation::build_stream_request,
    identity::{CachedUsage, CursorIdeIdentity, USAGE_CACHE_TTL},
    model_info,
    stream_state::StreamStateStore,
    usage::{fetch_usage, CursorUsageInfo},
};

const COMPOSER_CHAT_URL: &str =
    "https://api2.cursor.sh/aiserver.v1.ChatService/StreamUnifiedChatWithToolsIdempotentSSE";
const AUTO_POOL_THRESHOLD: f64 = 98.0;
const STREAM_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);

#[derive(Clone)]
pub struct CursorConfig {
    pub composer: Option<ComposerSession>,
}

impl CursorConfig {
    pub fn from_default_sources() -> Result<Self> {
        Ok(Self {
            composer: load_composer_session()?,
        })
    }
}

pub struct CursorProvider {
    config: CursorConfig,
    http: Client,
    identity: CursorIdeIdentity,
    stream_state: Mutex<StreamStateStore>,
    usage_cache: Mutex<Option<CachedUsage>>,
}

impl CursorProvider {
    pub fn new(config: CursorConfig) -> Result<Self> {
        let identity = CursorIdeIdentity::load();
        let http = Client::builder()
            .user_agent(identity.user_agent())
            .build()
            .map_err(|err| AppError::Network(err.to_string()))?;
        Ok(Self {
            config,
            http,
            identity,
            stream_state: Mutex::new(StreamStateStore::load()),
            usage_cache: Mutex::new(None),
        })
    }

    pub fn from_default_sources() -> Result<Self> {
        Self::new(CursorConfig::from_default_sources()?)
    }

    pub async fn usage_snapshot(&self) -> Result<Option<CursorUsageInfo>> {
        let Some(session) = self.config.composer.as_ref() else {
            return Ok(None);
        };
        let token = ensure_fresh_composer_token(&self.http, session).await?;
        Ok(Some(
            fetch_usage(&self.http, &self.identity, &token).await?,
        ))
    }

    async fn composer_token(&self) -> Result<String> {
        let session = self.config.composer.as_ref().ok_or_else(|| {
            AppError::Auth("Cursor is not connected. Connect your Cursor account in Settings.".into())
        })?;
        let token = ensure_fresh_composer_token(&self.http, session).await?;
        if self.should_block_for_pool_exhaustion(&token).await? {
            return Err(AppError::Auth(format!(
                "Cursor Auto+Composer pool is exhausted ({:.0}% used). Wait for reset or use Cursor IDE.",
                self.cached_usage(&token).await?.auto_percent_used
            )));
        }
        Ok(token)
    }

    async fn cached_usage(&self, token: &str) -> Result<CursorUsageInfo> {
        if let Ok(guard) = self.usage_cache.lock() {
            if let Some(cached) = guard.as_ref() {
                if cached.fetched_at.elapsed() < USAGE_CACHE_TTL {
                    return Ok(cached.info.clone());
                }
            }
        }
        let usage = fetch_usage(&self.http, &self.identity, token).await?;
        if let Ok(mut guard) = self.usage_cache.lock() {
            *guard = Some(CachedUsage {
                fetched_at: Instant::now(),
                info: usage.clone(),
            });
        }
        Ok(usage)
    }

    async fn should_block_for_pool_exhaustion(&self, token: &str) -> Result<bool> {
        let usage = self.cached_usage(token).await?;
        Ok(usage.auto_percent_used >= AUTO_POOL_THRESHOLD)
    }
}

#[async_trait]
impl Provider for CursorProvider {
    fn name(&self) -> &str {
        "cursor"
    }

    fn capabilities(&self, model: &ModelRef) -> Option<ModelCapabilities> {
        if model.provider != "cursor" {
            return None;
        }
        Some(model_info::capabilities(model))
    }

    async fn estimate_tokens(&self, request: ProviderRequest) -> Result<TokenEstimate> {
        Ok(TokenEstimate {
            input_tokens: rough_token_estimate(&request),
            exact: false,
        })
    }

    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream> {
        if request.model.provider != "cursor" {
            return Err(AppError::Unsupported(format!(
                "cursor provider cannot run model provider {}",
                request.model.provider
            )));
        }
        let token = self.composer_token().await?;
        stream_composer(self, token, request).await
    }
}

async fn stream_composer(
    provider: &CursorProvider,
    token: String,
    request: ProviderRequest,
) -> Result<ProviderStream> {
    let mut identity = provider.identity.clone();
    identity.refresh();
    identity.ensure_ready()?;

    let session_key = request
        .cache_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let (conversation_id, idempotency_key, encryption_key, seqno) = {
        let mut guard = provider.stream_state.lock().map_err(|_| {
            AppError::Provider("cursor stream state lock poisoned".into())
        })?;
        let state = guard.conversation_state(&session_key);
        (
            session_key.clone(),
            state.idempotency_key.clone(),
            state.encryption_key.clone(),
            state.seqno,
        )
    };
    let (payload, next_seqno) = build_stream_request(
        &request,
        &conversation_id,
        &idempotency_key,
        seqno,
        &identity,
        &encryption_key,
    );
    if let Ok(mut guard) = provider.stream_state.lock() {
        guard.update_seqno(&session_key, next_seqno);
    }

    let session_id = uuid::Uuid::new_v4().to_string();
    let request_id = uuid::Uuid::new_v4().to_string();
    let mut headers = reqwest::header::HeaderMap::new();
    identity.apply_authenticated(&mut headers, &session_id, &request_id, &token);

    headers.insert(
        reqwest::header::HeaderName::from_static("x-idempotency-key"),
        reqwest::header::HeaderValue::from_str(&idempotency_key).unwrap(),
    );
    headers.insert(
        reqwest::header::HeaderName::from_static("x-idempotent-encryption-key"),
        reqwest::header::HeaderValue::from_str(&encryption_key).unwrap(),
    );

    let response = provider
        .http
        .post(COMPOSER_CHAT_URL)
        .headers(headers)
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/connect+json")
        .header("connect-protocol-version", "1")
        .header("accept", "application/connect+json")
        .body(payload)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|err| AppError::Network(err.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Network(format!(
            "Composer stream failed ({status}): {body}"
        )));
    }

    let model = request.model.name.clone();
    let mut byte_stream = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut last_text = String::new();
    let mut last_thinking = String::new();
    let mut started = false;
    let text_index = 0usize;
    let thinking_index = 1usize;
    let mut tool_index = 2usize;
    let mut emitted_tools = std::collections::HashSet::<String>::new();
    let mut saw_tool_call = false;
    let mut usage = Usage::default();

    let events = async_stream::try_stream! {
        let mut open_part: Option<(usize, PartKind)> = None;

        loop {
            let next = tokio::time::timeout(STREAM_IDLE_TIMEOUT, byte_stream.next()).await;
            let chunk = match next {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(_) if !started => {
                    Err(AppError::Network(
                        "Composer stream timed out waiting for the first response. Reconnect Cursor in Settings > Providers.".into(),
                    ))?;
                    break;
                }
                Err(_) => break,
            };
            let chunk = chunk.map_err(|err| AppError::Network(err.to_string()))?;
            buffer.extend_from_slice(&chunk);
            for frame in decode_connect_frames(&mut buffer)? {
                for event in parse_connect_events(&frame)? {
                    if !started {
                        started = true;
                        yield StreamEvent::MessageStart { model: model.clone() };
                    }
                    match event {
                        ComposerEvent::Text(text) => {
                            let delta = if text.starts_with(&last_text) {
                                text[last_text.len()..].to_string()
                            } else {
                                text.clone()
                            };
                            last_text = text;
                            if !delta.is_empty() {
                                if open_part.map(|(_, kind)| kind) != Some(PartKind::Text) {
                                    if let Some((idx, _)) = open_part.take() {
                                        yield StreamEvent::PartStop { index: idx };
                                    }
                                    open_part = Some((text_index, PartKind::Text));
                                    yield StreamEvent::PartStart {
                                        index: text_index,
                                        kind: PartKind::Text,
                                        tool: None,
                                    };
                                }
                                yield StreamEvent::TextDelta { index: text_index, delta };
                            }
                        }
                        ComposerEvent::Thinking(thinking) => {
                            let delta = if thinking.starts_with(&last_thinking) {
                                thinking[last_thinking.len()..].to_string()
                            } else {
                                thinking.clone()
                            };
                            last_thinking = thinking;
                            if !delta.is_empty() {
                                if open_part.map(|(_, kind)| kind) != Some(PartKind::Thinking) {
                                    if let Some((idx, _)) = open_part.take() {
                                        yield StreamEvent::PartStop { index: idx };
                                    }
                                    open_part = Some((thinking_index, PartKind::Thinking));
                                    yield StreamEvent::PartStart {
                                        index: thinking_index,
                                        kind: PartKind::Thinking,
                                        tool: None,
                                    };
                                }
                                yield StreamEvent::ThinkingDelta { index: thinking_index, delta };
                            }
                        }
                        ComposerEvent::ToolCall(call) => {
                            if !emitted_tools.insert(call.id.clone()) {
                                continue;
                            }
                            if let Some((idx, _)) = open_part.take() {
                                yield StreamEvent::PartStop { index: idx };
                            }
                            saw_tool_call = true;
                            let input_json = serde_json::to_string(&call.input)
                                .unwrap_or_else(|_| "{}".into());
                            yield StreamEvent::PartStart {
                                index: tool_index,
                                kind: PartKind::ToolCall,
                                tool: Some(ToolCallIntro {
                                    id: call.id.clone(),
                                    name: call.sinew_name.clone(),
                                }),
                            };
                            yield StreamEvent::PartMeta {
                                index: tool_index,
                                meta: json!({ "cursor_tool": call.cursor_tool }),
                            };
                            yield StreamEvent::ToolJsonDelta {
                                index: tool_index,
                                chunk: input_json,
                            };
                            yield StreamEvent::PartStop { index: tool_index };
                            tool_index += 1;
                        }
                        ComposerEvent::Usage(update) => {
                            merge_usage(&mut usage, update);
                            yield StreamEvent::Usage { usage };
                        }
                    }
                }
            }
        }
        if let Some((idx, _)) = open_part.take() {
            yield StreamEvent::PartStop { index: idx };
        }
        if !started {
            yield StreamEvent::MessageStart { model: model.clone() };
        }
        yield StreamEvent::MessageStop {
            stop_reason: if saw_tool_call {
                StopReason::ToolUse
            } else {
                StopReason::EndTurn
            },
            usage,
        };
    };

    Ok(Box::pin(events))
}

fn merge_usage(into: &mut Usage, update: Usage) {
    if update.input_tokens > 0 {
        into.input_tokens = update.input_tokens;
    }
    if update.output_tokens > 0 {
        into.output_tokens = update.output_tokens;
    }
    if update.total_tokens > 0 {
        into.total_tokens = update.total_tokens;
    } else if into.input_tokens > 0 || into.output_tokens > 0 {
        into.total_tokens = into.input_tokens + into.output_tokens;
    }
    if update.reasoning_tokens > 0 {
        into.reasoning_tokens = update.reasoning_tokens;
    }
    if update.cache_read_tokens > 0 {
        into.cache_read_tokens = update.cache_read_tokens;
    }
    if update.cache_creation_tokens > 0 {
        into.cache_creation_tokens = update.cache_creation_tokens;
    }
}

fn rough_token_estimate(request: &ProviderRequest) -> u32 {
    let mut chars = request
        .system_prompt
        .as_ref()
        .map(|value| value.len())
        .unwrap_or(0);
    for message in &request.transcript {
        for part in &message.parts {
            if let sinew_core::Part::Text { text, .. } = part {
                chars += text.len();
            }
        }
    }
    (chars / 4).max(1) as u32
}
