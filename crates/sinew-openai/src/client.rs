use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Instant,
};

use async_trait::async_trait;
use serde_json::Value;
use sinew_core::{
    AppError, ChatMessage, Effort, ModelCapabilities, ModelRef, Part, Provider, ProviderRequest,
    ProviderStream, Result, Role, ServiceTier, TokenEstimate, ToolDescriptor,
};

use crate::{
    auth::Credential,
    model_info,
    responses_stream::{event_provider_stream, sse_event_stream},
    websocket, wire,
};

const API_BASE_URL: &str = "https://api.openai.com/v1";
const CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
pub(crate) const USER_AGENT: &str = "sinew/0.1";
const FALLBACK_INSTRUCTIONS: &str = "You are Sinew, a concise coding assistant.";

#[derive(Clone)]
pub struct OpenAiConfig {
    pub credential: Credential,
    pub api_base_url: String,
    pub codex_base_url: String,
    pub websocket_enabled: bool,
}

impl OpenAiConfig {
    pub fn new(credential: Credential) -> Self {
        Self {
            credential,
            api_base_url: API_BASE_URL.into(),
            codex_base_url: CODEX_BASE_URL.into(),
            websocket_enabled: true,
        }
    }

    pub fn from_default_sources() -> Result<Self> {
        if let Some(credential) = Credential::load_default()? {
            return Ok(Self::new(credential));
        }

        Err(AppError::Auth(
            "no openai oauth credential found. Connect OpenAI in Settings > Providers".into(),
        ))
    }

    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        if let Some(credential) = Credential::from_sinew_auth_file(path)? {
            return Ok(Self::new(credential));
        }

        Err(AppError::Auth(format!(
            "unable to load openai credential from path {}",
            path.display()
        )))
    }
}

pub struct OpenAiProvider {
    config: OpenAiConfig,
    http: reqwest::Client,
    websocket_fallback_sessions: Arc<Mutex<HashSet<String>>>,
}

impl OpenAiProvider {
    pub fn new(config: OpenAiConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .map_err(|err| AppError::Network(err.to_string()))?;
        Ok(Self {
            config,
            http,
            websocket_fallback_sessions: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    pub fn from_default_sources() -> Result<Self> {
        Self::new(OpenAiConfig::from_default_sources()?)
    }

    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        Self::new(OpenAiConfig::from_file(path)?)
    }

    async fn post(&self, route: &str) -> Result<reqwest::RequestBuilder> {
        let bearer = self.config.credential.bearer(&self.http).await?;
        let base_url = if bearer.is_oauth {
            &self.config.codex_base_url
        } else {
            &self.config.api_base_url
        };
        let mut request = self
            .http
            .post(format!("{}{}", base_url.trim_end_matches('/'), route))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", bearer.token));

        if bearer.is_oauth {
            request = request.header("user-agent", "codex-cli");
            request = request.header("openai-beta", "responses=experimental");
            if let Some(account_id) = bearer.account_id {
                request = request.header("chatgpt-account-id", account_id);
            }
        } else {
            request = request.header("user-agent", USER_AGENT);
        }

        Ok(request)
    }
}

#[async_trait]
impl Provider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn capabilities(&self, model: &ModelRef) -> Option<ModelCapabilities> {
        if model.provider != "openai" && !model.provider.starts_with("openai:") {
            return None;
        }
        Some(model_info::capabilities(model))
    }

    async fn estimate_tokens(&self, request: ProviderRequest) -> Result<TokenEstimate> {
        if request.model.provider != "openai" && !request.model.provider.starts_with("openai:") {
            return Err(AppError::Unsupported(format!(
                "openai provider cannot count model provider {}",
                request.model.provider
            )));
        }

        let is_oauth = self.config.credential.is_oauth();
        if is_oauth {
            return Ok(TokenEstimate {
                input_tokens: rough_token_estimate(&request),
                exact: false,
            });
        }

        let instructions = request
            .system_prompt
            .as_deref()
            .filter(|value| !value.trim().is_empty());

        let body = wire::InputTokensRequest {
            model: &request.model.name,
            instructions,
            input: to_input_items(&request.transcript, !is_oauth)?,
            tools: request.tools.iter().map(to_wire_tool).collect(),
        };

        let response = self
            .post("/responses/input_tokens")
            .await?
            .header("accept", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|err| AppError::Network(err.to_string()))?;

        if !response.status().is_success() {
            return Err(read_http_error(response).await);
        }

        let counted: wire::InputTokensResponse = response
            .json()
            .await
            .map_err(|err| AppError::Decode(err.to_string()))?;
        Ok(TokenEstimate {
            input_tokens: counted.input_tokens,
            exact: true,
        })
    }

    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream> {
        if request.model.provider != "openai" && !request.model.provider.starts_with("openai:") {
            return Err(AppError::Unsupported(format!(
                "openai provider cannot run model provider {}",
                request.model.provider
            )));
        }

        stream_responses_request(
            &self.config,
            &self.http,
            request,
            Arc::clone(&self.websocket_fallback_sessions),
        )
        .await
    }
}

async fn stream_responses_request(
    config: &OpenAiConfig,
    http: &reqwest::Client,
    request: ProviderRequest,
    websocket_fallback_sessions: Arc<Mutex<HashSet<String>>>,
) -> Result<ProviderStream> {
    let bearer = config.credential.bearer(http).await?;
    let is_oauth = bearer.is_oauth;
    let body = build_responses_request(
        &request,
        &request.transcript,
        is_oauth,
        Some(false),
        Some(true),
    )?;

    let default_model = request.model.name.clone();
    let websocket_fallback_key = websocket_fallback_key(&request);

    if config.websocket_enabled
        && !websocket_fallback_is_active(&websocket_fallback_sessions, &websocket_fallback_key)
    {
        let ws_body = serde_json::to_value(&body)?;
        let fallback_sessions_for_stream = Arc::clone(&websocket_fallback_sessions);
        let fallback_key_for_stream = websocket_fallback_key.clone();
        let mark_websocket_fallback = Arc::new(move || {
            mark_websocket_fallback_active(&fallback_sessions_for_stream, &fallback_key_for_stream);
        });
        match websocket::stream_websocket_request(
            config,
            &bearer,
            ws_body,
            default_model.clone(),
            Some(mark_websocket_fallback),
        )
        .await
        {
            Ok(stream) => return Ok(stream),
            Err(err) if should_fallback_to_sse_after_websocket_setup_error(&err) => {
                tracing::warn!(error = %err, "OpenAI websocket unavailable; falling back to SSE");
                mark_websocket_fallback_active(
                    &websocket_fallback_sessions,
                    &websocket_fallback_key,
                );
            }
            Err(err) => return Err(err),
        }
    }

    stream_sse_request_with_bearer(config, http, bearer, body, default_model).await
}

async fn stream_sse_request_with_bearer(
    config: &OpenAiConfig,
    http: &reqwest::Client,
    bearer: crate::auth::BearerToken,
    body: wire::ResponsesRequest<'_>,
    default_model: String,
) -> Result<ProviderStream> {
    let base_url = if bearer.is_oauth {
        &config.codex_base_url
    } else {
        &config.api_base_url
    };
    let mut builder = http
        .post(format!("{}/responses", base_url.trim_end_matches('/')))
        .header("accept", "text/event-stream")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", bearer.token));

    if bearer.is_oauth {
        builder = builder.header("user-agent", "codex-cli");
        builder = builder.header("openai-beta", "responses=experimental");
        if let Some(account_id) = bearer.account_id {
            builder = builder.header("chatgpt-account-id", account_id);
        }
    } else {
        builder = builder.header("user-agent", USER_AGENT);
    }

    let req_start = Instant::now();
    let response = builder
        .json(&body)
        .send()
        .await
        .map_err(|err| AppError::Network(err.to_string()))?;
    let http_ms = req_start.elapsed().as_millis();
    tracing::debug!(
        provider = "openai",
        model = default_model,
        http_ms,
        transport = "sse",
        "OpenAI HTTP round-trip (stream setup)"
    );

    if !response.status().is_success() {
        return Err(read_http_error(response).await);
    }

    Ok(event_provider_stream(
        sse_event_stream(response.bytes_stream()),
        default_model,
        "SSE",
    ))
}

fn websocket_fallback_key(request: &ProviderRequest) -> String {
    request
        .cache_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("cache:{value}"))
        .unwrap_or_else(|| format!("model:{}", request.model.name))
}

fn websocket_fallback_is_active(sessions: &Arc<Mutex<HashSet<String>>>, key: &str) -> bool {
    sessions
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains(key)
}

fn mark_websocket_fallback_active(sessions: &Arc<Mutex<HashSet<String>>>, key: &str) {
    sessions
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(key.to_string());
}

fn should_fallback_to_sse_after_websocket_setup_error(err: &AppError) -> bool {
    matches!(
        err,
        AppError::Network(_)
            | AppError::Stream(_)
            | AppError::RetryableStream { .. }
            | AppError::Provider(_)
    )
}

fn build_responses_request<'a>(
    request: &'a ProviderRequest,
    transcript: &'a [ChatMessage],
    is_oauth: bool,
    store: Option<bool>,
    stream: Option<bool>,
) -> Result<wire::ResponsesRequest<'a>> {
    let caps = model_info::capabilities(&request.model);
    Ok(wire::ResponsesRequest {
        model: &request.model.name,
        instructions: response_instructions(request, is_oauth),
        input: to_input_items(transcript, !is_oauth)?,
        tools: request.tools.iter().map(to_wire_tool).collect(),
        prompt_cache_key: (!is_oauth)
            .then_some(request.cache_key.as_deref())
            .flatten(),
        max_output_tokens: (!is_oauth).then_some(
            request
                .max_output_tokens
                .unwrap_or(caps.max_output_tokens)
                .min(caps.max_output_tokens),
        ),
        reasoning: effort_to_reasoning(request.effective_effort()),
        temperature: request.temperature,
        store,
        stream,
        service_tier: service_tier_param(request.service_tier),
        generate: None,
    })
}

fn response_instructions(request: &ProviderRequest, is_oauth: bool) -> Option<&str> {
    let instructions = request
        .system_prompt
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    if is_oauth {
        Some(instructions.unwrap_or(FALLBACK_INSTRUCTIONS))
    } else {
        instructions
    }
}

fn effort_to_reasoning(effort: Option<Effort>) -> Option<wire::ReasoningConfig> {
    Some(wire::ReasoningConfig {
        effort: match effort.unwrap_or(Effort::Medium) {
            Effort::None => "none",
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::Xhigh | Effort::Max => "xhigh",
        },
        summary: "auto",
    })
}

fn service_tier_param(service_tier: Option<ServiceTier>) -> Option<&'static str> {
    match service_tier {
        Some(ServiceTier::Fast) => Some("priority"),
        Some(ServiceTier::Flex) => Some("flex"),
        None => None,
    }
}

fn to_wire_tool(tool: &ToolDescriptor) -> wire::WireTool<'_> {
    wire::WireTool {
        kind: "function",
        name: &tool.name,
        description: &tool.description,
        parameters: &tool.input_schema,
    }
}

fn to_input_items(
    transcript: &[ChatMessage],
    include_response_items: bool,
) -> Result<Vec<wire::InputItem<'_>>> {
    let mut items = Vec::new();
    for message in transcript {
        let mut content = Vec::new();
        for part in &message.parts {
            if part_is_ui_only(part) {
                continue;
            }
            match part {
                Part::Text { text, .. } => {
                    if text.is_empty() {
                        continue;
                    }
                    let item = match message.role {
                        Role::User => wire::InputContent::InputText { text },
                        Role::Assistant => wire::InputContent::OutputText { text },
                    };
                    content.push(item);
                }
                Part::Image {
                    media_type, data, ..
                } => {
                    if matches!(message.role, Role::User) {
                        content.push(wire::InputContent::InputImage {
                            image_url: format!("data:{media_type};base64,{data}"),
                        });
                    }
                }
                Part::Thinking { meta, .. } => {
                    flush_message_content(message.role, &mut content, &mut items);
                    if include_response_items {
                        if let Some(item) = openai_response_item(meta) {
                            items.push(wire::InputItem::ResponseItem(item));
                        }
                    }
                }
                Part::ToolCall {
                    id,
                    name,
                    input,
                    meta: _,
                } => {
                    flush_message_content(message.role, &mut content, &mut items);
                    items.push(wire::InputItem::FunctionCall {
                        kind: "function_call",
                        call_id: id,
                        name,
                        arguments: input.to_string(),
                    });
                }
                Part::ToolResult {
                    tool_call_id,
                    content: text,
                    images,
                    ..
                } => {
                    flush_message_content(message.role, &mut content, &mut items);
                    let inline_images = images
                        .iter()
                        .filter(|image| !image.data.trim().is_empty())
                        .collect::<Vec<_>>();
                    let output = if inline_images.is_empty() {
                        wire::ToolOutput::Text(text)
                    } else {
                        let mut blocks = Vec::new();
                        if !text.trim().is_empty() {
                            blocks.push(wire::ToolOutputBlock::InputText { text });
                        }
                        blocks.extend(inline_images.into_iter().map(|image| {
                            wire::ToolOutputBlock::InputImage {
                                image_url: format!(
                                    "data:{};base64,{}",
                                    image.media_type, image.data
                                ),
                            }
                        }));
                        wire::ToolOutput::Blocks(blocks)
                    };
                    items.push(wire::InputItem::FunctionCallOutput {
                        kind: "function_call_output",
                        call_id: tool_call_id,
                        output,
                    });
                }
            }
        }

        flush_message_content(message.role, &mut content, &mut items);
    }

    Ok(items)
}

fn flush_message_content<'a>(
    role: Role,
    content: &mut Vec<wire::InputContent<'a>>,
    items: &mut Vec<wire::InputItem<'a>>,
) {
    if content.is_empty() {
        return;
    }
    items.push(wire::InputItem::Message {
        role: match role {
            Role::User => "user",
            Role::Assistant => "assistant",
        },
        content: std::mem::take(content),
    });
}

fn part_is_ui_only(part: &Part) -> bool {
    part_meta(part)
        .and_then(|meta| meta.get("ui_only"))
        .and_then(|value| value.as_bool())
        == Some(true)
}

fn part_meta(part: &Part) -> Option<&Value> {
    match part {
        Part::Text { meta, .. }
        | Part::Image { meta, .. }
        | Part::Thinking { meta, .. }
        | Part::ToolCall { meta, .. }
        | Part::ToolResult { meta, .. } => meta.as_ref(),
    }
}

fn openai_response_item(meta: &Option<serde_json::Value>) -> Option<&serde_json::Value> {
    let meta = meta.as_ref()?;
    if meta.get("provider").and_then(|value| value.as_str()) != Some("openai") {
        return None;
    }
    meta.get("item")
}

fn rough_token_estimate(request: &ProviderRequest) -> u32 {
    let mut chars: usize = 0;
    if let Some(system) = &request.system_prompt {
        chars += system.chars().count();
    }
    for message in &request.transcript {
        for part in &message.parts {
            if part_is_ui_only(part) {
                continue;
            }
            match part {
                Part::Text { text, .. } | Part::Thinking { text, .. } => {
                    chars += text.chars().count()
                }
                Part::Image { data, .. } => {
                    chars += if data.trim().is_empty() { 0 } else { 4_000 };
                }
                Part::ToolCall { name, input, .. } => {
                    chars += name.chars().count();
                    chars += input.to_string().chars().count();
                }
                Part::ToolResult {
                    content, images, ..
                } => {
                    chars += content.chars().count();
                    chars += images
                        .iter()
                        .filter(|image| !image.data.trim().is_empty())
                        .count()
                        * 4_000;
                }
            }
        }
    }
    for tool in &request.tools {
        chars += tool.name.chars().count();
        chars += tool.description.chars().count();
        chars += tool.input_schema.to_string().chars().count();
    }
    ((chars / 4).max(1)).min(u32::MAX as usize) as u32
}

async fn read_http_error(response: reqwest::Response) -> AppError {
    let status = response.status();
    let delay_ms = retry_after_ms(&response);
    let body = response.text().await.unwrap_or_default();
    let parsed: std::result::Result<wire::ApiErrorEnvelope, _> = serde_json::from_str(&body);
    let message = parsed
        .ok()
        .and_then(|payload| {
            let code = payload.error.code.unwrap_or_default();
            let kind = payload.error.kind.trim();
            let error_message = payload.error.message.trim();
            if code.is_empty() && kind.is_empty() && error_message.is_empty() {
                None
            } else if code.is_empty() {
                Some(format!("{kind}: {error_message}").trim().to_string())
            } else {
                Some(
                    format!("{kind} ({code}): {error_message}")
                        .trim()
                        .to_string(),
                )
            }
        })
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| {
            let body = body.trim();
            if body.is_empty() {
                format!("HTTP {status}")
            } else {
                body.to_string()
            }
        });

    if status == reqwest::StatusCode::UNAUTHORIZED {
        AppError::Auth(message)
    } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        AppError::RateLimit(message)
    } else if is_transient_http_status(status) {
        AppError::RetryableStream {
            message: format!("HTTP {status}: {message}"),
            delay_ms,
        }
    } else if status.is_client_error() {
        let lower = message.to_ascii_lowercase();
        if lower.contains("context") || lower.contains("too long") {
            AppError::ContextLength(message)
        } else {
            AppError::InvalidRequest(message)
        }
    } else {
        AppError::Provider(format!("HTTP {status}: {message}"))
    }
}

fn retry_after_ms(response: &reqwest::Response) -> Option<u64> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(|seconds| seconds.saturating_mul(1000))
}

fn is_transient_http_status(status: reqwest::StatusCode) -> bool {
    matches!(
        status,
        reqwest::StatusCode::REQUEST_TIMEOUT
            | reqwest::StatusCode::CONFLICT
            | reqwest::StatusCode::TOO_EARLY
            | reqwest::StatusCode::BAD_GATEWAY
            | reqwest::StatusCode::SERVICE_UNAVAILABLE
            | reqwest::StatusCode::GATEWAY_TIMEOUT
    ) || status.is_server_error()
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sinew_core::{
        ChatMessage, ModelRef, Part, ProviderRequest, Role, ServiceTier, ToolResultImage,
    };

    use super::{build_responses_request, to_input_items};

    #[test]
    fn image_tool_result_uses_responses_input_block_types() {
        let transcript = vec![ChatMessage {
            role: Role::User,
            parts: vec![Part::ToolResult {
                tool_call_id: "call_read".into(),
                content: "path: image.png\n\n[Image attached visually.]".into(),
                images: vec![ToolResultImage {
                    media_type: "image/png".into(),
                    data: "iVBORw0KGgo=".into(),
                    path: None,
                }],
                is_error: false,
                meta: None,
            }],
        }];

        let items = to_input_items(&transcript, true).expect("tool result should serialize");
        let value = serde_json::to_value(items).expect("items should be json");

        assert_eq!(
            value,
            json!([
                {
                    "type": "function_call_output",
                    "call_id": "call_read",
                    "output": [
                        {
                            "type": "input_text",
                            "text": "path: image.png\n\n[Image attached visually.]"
                        },
                        {
                            "type": "input_image",
                            "image_url": "data:image/png;base64,iVBORw0KGgo="
                        }
                    ]
                }
            ])
        );
    }

    #[test]
    fn sse_request_body_uses_responses_stream_shape() {
        let request = ProviderRequest::new(
            ModelRef::new("openai", "gpt-5.5"),
            vec![ChatMessage::user_text("hello")],
        )
        .with_system("be helpful")
        .with_cache_key("conversation-1");
        let body = build_responses_request(
            &request,
            &request.transcript,
            false,
            Some(false),
            Some(true),
        )
        .expect("body should serialize");
        let value = serde_json::to_value(&body).expect("body should be json");

        assert!(value.as_object().unwrap().get("type").is_none());
        assert_eq!(value["store"], false);
        assert_eq!(value["stream"], true);
        assert_eq!(value["prompt_cache_key"], "conversation-1");
        assert_eq!(value["input"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn sse_request_body_maps_fast_service_tier_to_priority() {
        let request = ProviderRequest::new(
            ModelRef::new("openai", "gpt-5.5"),
            vec![ChatMessage::user_text("hello")],
        )
        .with_service_tier(ServiceTier::Fast);
        let body = build_responses_request(
            &request,
            &request.transcript,
            false,
            Some(false),
            Some(true),
        )
        .expect("body should serialize");
        let value = serde_json::to_value(&body).expect("body should be json");

        assert_eq!(value["service_tier"], "priority");
    }
}
