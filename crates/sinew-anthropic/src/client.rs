use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::Serialize;
use sinew_core::{
    AppError, ChatMessage, Effort, ModelCapabilities, ModelRef, Part, Provider, ProviderRequest,
    ProviderStream, Result, Role, TokenEstimate, ToolDescriptor,
};

use crate::{
    auth::{Credential, ANTHROPIC_RECONNECT_MESSAGE},
    cli_version, model_info,
    stream::map_stream,
    wire,
};

const BASE_URL: &str = "https://api.anthropic.com";
const API_VERSION: &str = "2023-06-01";
// The advertised Claude Code version lives in `cli_version`, which refreshes
// itself from the npm registry instead of going stale here.
const CODE_SYSTEM_PREFIX: &str = "You are Claude Code, Anthropic's official CLI for Claude.";
// Note: we intentionally do NOT advertise `context-1m-2025-08-07` here.
// All models currently shipped in the app (Opus 4.6/4.7/4.8, Sonnet 4.6/5) already
// expose a 1M context window natively, and Haiku 4.5 does not support that
// beta at all. Sending it inconditionally caused:
//   * Sonnet 4.6 → server-side tier gating → `rate_limit_error: Extra usage
//     is required for long context requests` even for trivial prompts.
//   * Haiku 4.5 → `invalid_request_error: The long context beta is not yet
//     available for this subscription` which we then mis-classified as a
//     context-length overflow and triggered auto-compaction on tiny inputs.
const COMMON_BETA: &str = "fine-grained-tool-streaming-2025-05-14";
const OAUTH_BETA: &str = "claude-code-20250219,oauth-2025-04-20";
const CACHE_BREAKPOINTS: usize = 4;
const ANTHROPIC_MAX_IMAGE_BASE64_BYTES: usize = 5 * 1024 * 1024;
const ANTHROPIC_SKIPPED_IMAGE_TOO_LARGE: &str =
    "[Skipped: image is too large for Anthropic and exceeds the 5 MiB limit. Try compressing it first.]";

#[derive(Clone)]
pub struct AnthropicConfig {
    pub credential: Credential,
    pub base_url: String,
    pub api_version: String,
    pub extra_beta: Option<String>,
}

impl AnthropicConfig {
    pub fn new(credential: Credential) -> Self {
        Self {
            credential,
            base_url: BASE_URL.into(),
            api_version: API_VERSION.into(),
            extra_beta: None,
        }
    }

    pub fn from_default_sources() -> Result<Self> {
        if let Some(credential) = Credential::load_default()? {
            return Ok(Self::new(credential));
        }

        Err(AppError::Auth(
            "no anthropic credential found. Connect Anthropic in Settings > Providers.".into(),
        ))
    }
}

pub struct AnthropicProvider {
    config: AnthropicConfig,
    http: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(config: AnthropicConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(cli_version::user_agent())
            .build()
            .map_err(|err| AppError::Network(err.to_string()))?;
        Ok(Self { config, http })
    }

    pub fn from_default_sources() -> Result<Self> {
        Self::new(AnthropicConfig::from_default_sources()?)
    }

    async fn post(&self, route: &str) -> Result<(reqwest::RequestBuilder, String)> {
        // Cheap no-op unless the cached Claude Code version lookup expired.
        cli_version::ensure_fresh(&self.http);
        let token = self.config.credential.bearer_or_key(&self.http).await?;
        let is_oauth = self.config.credential.is_oauth();
        let mut request = self
            .http
            .post(format!(
                "{}{}",
                self.config.base_url.trim_end_matches('/'),
                route
            ))
            .header("anthropic-version", &self.config.api_version)
            .header("content-type", "application/json")
            .header("anthropic-dangerous-direct-browser-access", "true")
            .header("anthropic-beta", self.beta_header(is_oauth))
            .header("user-agent", cli_version::user_agent());

        if is_oauth {
            request = request
                .header("authorization", format!("Bearer {token}"))
                .header("x-app", "cli");
        } else {
            request = request.header("x-api-key", token.clone());
        }

        Ok((request, token))
    }

    async fn send_json_accept<T: Serialize + ?Sized>(
        &self,
        route: &str,
        body: &T,
        accept: &'static str,
    ) -> Result<reqwest::Response> {
        let (request, token) = self.post(route).await?;
        let response = request
            .header("accept", accept)
            .json(body)
            .send()
            .await
            .map_err(|err| AppError::Network(err.to_string()))?;

        if response.status() != reqwest::StatusCode::UNAUTHORIZED
            || !self.config.credential.is_oauth()
        {
            return Ok(response);
        }

        self.config
            .credential
            .force_refresh(&self.http, &token)
            .await
            .map_err(map_refresh_failure)?;

        let (request, _) = self.post(route).await?;
        request
            .header("accept", accept)
            .json(body)
            .send()
            .await
            .map_err(|err| AppError::Network(err.to_string()))
    }

    /// Send a JSON body, retrying once when Anthropic rejects the Claude Code
    /// version we advertise ("version X or newer is required").
    ///
    /// Without this, any model gated behind a newer CLI than the one we ship
    /// stays broken until the app is rebuilt. With it, the first rejected
    /// request teaches us the required version and the retry goes through.
    async fn send_json_version_aware<T: Serialize + ?Sized>(
        &self,
        route: &str,
        body: &T,
        accept: &'static str,
        retry_transient: bool,
    ) -> Result<reqwest::Response> {
        let response = self.send_json_accept(route, body, accept).await?;
        if response.status().is_success() || !response.status().is_client_error() {
            return Ok(response);
        }

        let status = response.status();
        let delay_ms = retry_after_ms(&response);
        let message = error_message(&response.text().await.unwrap_or_default());

        let Some(required) = cli_version::adopt_required_version(&message) else {
            return Err(classify_http_error(
                status,
                message,
                retry_transient,
                delay_ms,
            ));
        };

        tracing::info!(
            %required,
            "anthropic requires a newer claude-cli version; retrying the request"
        );

        let retry = self.send_json_accept(route, body, accept).await?;
        if retry.status().is_success() || !retry.status().is_client_error() {
            return Ok(retry);
        }

        let status = retry.status();
        let delay_ms = retry_after_ms(&retry);
        let message = error_message(&retry.text().await.unwrap_or_default());
        Err(classify_http_error(
            status,
            message,
            retry_transient,
            delay_ms,
        ))
    }

    fn beta_header(&self, is_oauth: bool) -> String {
        let mut values = Vec::new();
        if is_oauth {
            values.push(OAUTH_BETA.to_string());
        }
        values.push(COMMON_BETA.to_string());
        if let Some(extra) = &self.config.extra_beta {
            if !extra.is_empty() {
                values.push(extra.clone());
            }
        }
        values.join(",")
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn capabilities(&self, model: &ModelRef) -> Option<ModelCapabilities> {
        if model.provider != "anthropic" {
            return None;
        }
        Some(model_info::capabilities(model))
    }

    async fn estimate_tokens(&self, request: ProviderRequest) -> Result<TokenEstimate> {
        if request.model.provider != "anthropic" {
            return Err(AppError::Unsupported(format!(
                "anthropic provider cannot count model provider {}",
                request.model.provider
            )));
        }

        let is_oauth = self.config.credential.is_oauth();
        let body = wire::CountTokensRequest {
            model: &request.model.name,
            system: build_system_blocks(is_oauth, request.system_prompt.as_deref(), false),
            messages: request
                .transcript
                .iter()
                .filter_map(|message| to_wire_message(message, false).transpose())
                .collect::<Result<Vec<_>>>()?,
            tools: request
                .tools
                .iter()
                .map(|tool| to_wire_tool(tool, false))
                .collect(),
        };

        let response = self
            .send_json_version_aware(
                "/v1/messages/count_tokens",
                &body,
                "application/json",
                false,
            )
            .await?;

        if !response.status().is_success() {
            return Err(read_http_error(response, false).await);
        }

        let counted: wire::CountTokensResponse = response
            .json()
            .await
            .map_err(|err| AppError::Decode(err.to_string()))?;
        Ok(TokenEstimate {
            input_tokens: counted.input_tokens,
            exact: true,
        })
    }

    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream> {
        let caps = model_info::capabilities(&request.model);
        let is_oauth = self.config.credential.is_oauth();
        let (thinking, output_config) = effort_to_output(request.effective_effort());
        let mut cache_budget = CACHE_BREAKPOINTS;
        let cache_tools = take_cache_breakpoint(&mut cache_budget, !request.tools.is_empty());
        let cache_system = take_cache_breakpoint(
            &mut cache_budget,
            has_system_blocks(is_oauth, request.system_prompt.as_deref()),
        );
        let stable_message_count = request
            .cache_stable_message_count
            .unwrap_or(request.transcript.len())
            .min(request.transcript.len());
        let cached_messages =
            cache_message_indices(&request.transcript[..stable_message_count], cache_budget);
        let body = wire::MessagesRequest {
            model: &request.model.name,
            max_tokens: request.output_token_budget(&caps),
            system: build_system_blocks(is_oauth, request.system_prompt.as_deref(), cache_system),
            messages: request
                .transcript
                .iter()
                .enumerate()
                .filter_map(|(index, message)| {
                    to_wire_message(message, cached_messages.contains(&index)).transpose()
                })
                .collect::<Result<Vec<_>>>()?,
            tools: request
                .tools
                .iter()
                .enumerate()
                .map(|(index, tool)| {
                    to_wire_tool(tool, cache_tools && index + 1 == request.tools.len())
                })
                .collect(),
            thinking,
            output_config,
            temperature: request.temperature,
            stream: true,
        };

        let response = self
            .send_json_version_aware("/v1/messages", &body, "text/event-stream", true)
            .await?;

        if !response.status().is_success() {
            return Err(read_http_error(response, true).await);
        }

        Ok(map_stream(response.bytes_stream()))
    }
}

fn build_system_blocks<'a>(
    is_oauth: bool,
    user_system: Option<&'a str>,
    cache_last: bool,
) -> Vec<wire::SystemText<'a>> {
    let mut blocks = Vec::new();
    if is_oauth {
        blocks.push(wire::SystemText {
            kind: "text",
            text: CODE_SYSTEM_PREFIX,
            cache_control: None,
        });
    }
    if let Some(text) = user_system {
        if !text.trim().is_empty() {
            blocks.push(wire::SystemText {
                kind: "text",
                text,
                cache_control: None,
            });
        }
    }
    if cache_last {
        if let Some(block) = blocks.last_mut() {
            block.cache_control = Some(cache_control());
        }
    }
    blocks
}

fn has_system_blocks(is_oauth: bool, user_system: Option<&str>) -> bool {
    is_oauth || user_system.is_some_and(|text| !text.trim().is_empty())
}

fn cache_control() -> wire::CacheControl {
    wire::CacheControl { kind: "ephemeral" }
}

fn take_cache_breakpoint(budget: &mut usize, condition: bool) -> bool {
    if !condition || *budget == 0 {
        return false;
    }
    *budget -= 1;
    true
}

fn cache_message_indices(history: &[ChatMessage], limit: usize) -> Vec<usize> {
    if limit == 0 {
        return Vec::new();
    }

    let mut indices = history
        .iter()
        .enumerate()
        .rev()
        .filter_map(|(index, message)| message.parts.iter().any(cacheable_part).then_some(index))
        .take(limit)
        .collect::<Vec<_>>();
    indices.reverse();
    indices
}

fn cacheable_part(part: &Part) -> bool {
    if part_is_ui_only(part) {
        return false;
    }
    match part {
        Part::Text { text, .. } => !text.is_empty(),
        Part::Image { data, .. } => !data.trim().is_empty(),
        Part::Thinking { .. } => false,
        Part::ToolCall { .. } | Part::ToolResult { .. } => true,
    }
}

fn effort_to_output(
    effort: Option<Effort>,
) -> (Option<wire::ThinkingConfig>, Option<wire::OutputConfig>) {
    let Some(effort) = effort else {
        return (None, None);
    };
    if matches!(effort, Effort::None) {
        return (None, None);
    }

    (
        Some(wire::ThinkingConfig {
            kind: "adaptive",
            budget_tokens: None,
            display: Some("summarized"),
        }),
        Some(wire::OutputConfig {
            effort: match effort {
                Effort::Low => "low",
                Effort::Medium => "medium",
                Effort::High => "high",
                Effort::Max => "max",
                Effort::Xhigh => "xhigh",
                Effort::None => unreachable!(),
            },
        }),
    )
}

fn to_wire_tool(tool: &ToolDescriptor, cache: bool) -> wire::WireTool<'_> {
    wire::WireTool {
        name: &tool.name,
        description: &tool.description,
        input_schema: &tool.input_schema,
        cache_control: cache.then(cache_control),
    }
}

fn to_wire_message(message: &ChatMessage, cache: bool) -> Result<Option<wire::WireMessage<'_>>> {
    let role = match message.role {
        Role::User => "user",
        Role::Assistant => "assistant",
    };

    let cache_part_index = cache.then(|| {
        message
            .parts
            .iter()
            .rposition(cacheable_part)
            .unwrap_or(usize::MAX)
    });
    let mut content = Vec::new();
    for (index, part) in message.parts.iter().enumerate() {
        if part_is_ui_only(part) {
            continue;
        }
        let cache_control = (cache_part_index == Some(index)).then(cache_control);
        match part {
            Part::Text { text, .. } => {
                if !text.is_empty() {
                    content.push(wire::WirePart::Text {
                        text,
                        cache_control,
                    });
                }
            }
            Part::Image {
                media_type, data, ..
            } => {
                if image_base64_fits_anthropic(data) {
                    content.push(wire::WirePart::Image {
                        source: image_source(media_type, data),
                        cache_control,
                    });
                } else {
                    content.push(wire::WirePart::Text {
                        text: ANTHROPIC_SKIPPED_IMAGE_TOO_LARGE,
                        cache_control,
                    });
                }
            }
            Part::Thinking { text, meta } => {
                let signature = meta
                    .as_ref()
                    .and_then(|meta| meta.get("signature"))
                    .and_then(|value| value.as_str());
                if let Some(signature) = signature {
                    content.push(wire::WirePart::Thinking {
                        thinking: text,
                        signature,
                    });
                }
            }
            Part::ToolCall {
                id, name, input, ..
            } => {
                content.push(wire::WirePart::ToolUse {
                    id,
                    name,
                    input,
                    cache_control,
                });
            }
            Part::ToolResult {
                tool_call_id,
                content: text,
                images,
                is_error,
                ..
            } => {
                let mut skipped_oversized_image = false;
                let inline_images = images
                    .iter()
                    .filter(|image| !image.data.trim().is_empty())
                    .filter(|image| {
                        let keep = image_base64_fits_anthropic(&image.data);
                        skipped_oversized_image |= !keep;
                        keep
                    })
                    .collect::<Vec<_>>();
                let result_content = if inline_images.is_empty() && !skipped_oversized_image {
                    wire::ToolResultContent::Text(text)
                } else {
                    let mut blocks = Vec::new();
                    if !text.trim().is_empty() {
                        blocks.push(wire::ToolResultBlock::Text { text });
                    }
                    blocks.extend(inline_images.into_iter().map(|image| {
                        wire::ToolResultBlock::Image {
                            source: image_source(&image.media_type, &image.data),
                        }
                    }));
                    if skipped_oversized_image {
                        blocks.push(wire::ToolResultBlock::Text {
                            text: ANTHROPIC_SKIPPED_IMAGE_TOO_LARGE,
                        });
                    }
                    wire::ToolResultContent::Blocks(blocks)
                };
                content.push(wire::WirePart::ToolResult {
                    tool_use_id: tool_call_id,
                    content: result_content,
                    is_error: *is_error,
                    cache_control,
                });
            }
        }
    }

    Ok((!content.is_empty()).then_some(wire::WireMessage { role, content }))
}

fn data_media_type(data: &str) -> Option<&'static str> {
    let bytes = BASE64_STANDARD.decode(data).ok()?;
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.len() >= 3 && bytes[0] == 0xff && bytes[1] == 0xd8 && bytes[2] == 0xff {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

fn image_source<'a>(media_type: &'a str, data: &'a str) -> wire::ImageSource<'a> {
    wire::ImageSource::Base64 {
        media_type: data_media_type(data).unwrap_or(media_type),
        data,
    }
}

fn image_base64_fits_anthropic(data: &str) -> bool {
    data.len() <= ANTHROPIC_MAX_IMAGE_BASE64_BYTES
}

fn part_is_ui_only(part: &Part) -> bool {
    part_meta(part)
        .and_then(|meta| meta.get("ui_only"))
        .and_then(|value| value.as_bool())
        == Some(true)
}

fn part_meta(part: &Part) -> Option<&serde_json::Value> {
    match part {
        Part::Text { meta, .. }
        | Part::Image { meta, .. }
        | Part::Thinking { meta, .. }
        | Part::ToolCall { meta, .. }
        | Part::ToolResult { meta, .. } => meta.as_ref(),
    }
}

fn map_refresh_failure(err: AppError) -> AppError {
    tracing::warn!(error = %err, "failed to refresh anthropic oauth token after auth failure");
    match err {
        AppError::Network(_) => AppError::Network(
            "Could not refresh Anthropic login. Check your connection and try again.".into(),
        ),
        _ => AppError::Auth(ANTHROPIC_RECONNECT_MESSAGE.into()),
    }
}

async fn read_http_error(response: reqwest::Response, retry_transient: bool) -> AppError {
    let status = response.status();
    let delay_ms = retry_after_ms(&response);
    let body = response.text().await.unwrap_or_default();
    classify_http_error(status, error_message(&body), retry_transient, delay_ms)
}

/// Turn a raw API error body into `kind: message`, falling back to the raw text.
fn error_message(body: &str) -> String {
    let parsed: std::result::Result<wire::ApiErrorEnvelope, _> = serde_json::from_str(body);
    parsed
        .map(|payload| format!("{}: {}", payload.error.kind, payload.error.message))
        .unwrap_or_else(|_| body.to_string())
}

fn classify_http_error(
    status: reqwest::StatusCode,
    message: String,
    retry_transient: bool,
    delay_ms: Option<u64>,
) -> AppError {
    if status == reqwest::StatusCode::UNAUTHORIZED {
        tracing::warn!(error = %message, "anthropic oauth request was rejected after refresh");
        AppError::Auth(ANTHROPIC_RECONNECT_MESSAGE.into())
    } else if retry_transient && is_transient_http_status(status) {
        AppError::RetryableStream {
            message: format!("HTTP {status}: {message}"),
            delay_ms,
        }
    } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        AppError::RateLimit(message)
    } else if status.is_client_error() {
        if is_context_length_message(&message) {
            AppError::ContextLength(message)
        } else {
            AppError::InvalidRequest(message)
        }
    } else {
        AppError::Provider(format!("HTTP {status}: {message}"))
    }
}

fn is_transient_http_status(status: reqwest::StatusCode) -> bool {
    matches!(
        status.as_u16(),
        408 | 409 | 429 | 500 | 502 | 503 | 504 | 529
    )
}

fn retry_after_ms(response: &reqwest::Response) -> Option<u64> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(|seconds| seconds.saturating_mul(1_000).min(60_000))
}

/// Recognise the real "prompt is too long" family of errors and avoid
/// false positives on every 4xx that happens to mention the word "context"
/// (e.g. `The long context beta is not yet available for this subscription`),
/// which would otherwise trick the agent into triggering auto-compaction on
/// a perfectly small history.
fn is_context_length_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    if lower.contains("beta") || lower.contains("not yet available") {
        return false;
    }
    lower.contains("prompt is too long")
        || lower.contains("input is too long")
        || lower.contains("too many tokens")
        || lower.contains("context window")
        || lower.contains("context length")
        || lower.contains("maximum context")
        || lower.contains("exceed") && lower.contains("context")
}

#[cfg(test)]
mod tests {
    use sinew_core::{ChatMessage, Part, Role, ToolResultImage};

    use super::*;

    #[test]
    fn oversized_tool_result_images_are_replaced_with_text_note() {
        let message = ChatMessage {
            role: Role::User,
            parts: vec![Part::ToolResult {
                tool_call_id: "call_1".into(),
                content: "path: hero.png".into(),
                images: vec![ToolResultImage {
                    media_type: "image/png".into(),
                    data: "a".repeat(ANTHROPIC_MAX_IMAGE_BASE64_BYTES + 1),
                    path: None,
                }],
                is_error: false,
                meta: None,
            }],
        };

        let wire_message = to_wire_message(&message, false)
            .expect("tool result should convert")
            .expect("message should not be empty");

        let [wire::WirePart::ToolResult { content, .. }] = wire_message.content.as_slice() else {
            panic!("expected a single tool result part");
        };
        let wire::ToolResultContent::Blocks(blocks) = content else {
            panic!("expected block tool result content");
        };

        assert_eq!(blocks.len(), 2);
        assert!(matches!(
            &blocks[0],
            wire::ToolResultBlock::Text { text } if *text == "path: hero.png"
        ));
        assert!(matches!(
            &blocks[1],
            wire::ToolResultBlock::Text { text } if *text == ANTHROPIC_SKIPPED_IMAGE_TOO_LARGE
        ));
        assert!(blocks
            .iter()
            .all(|block| !matches!(block, wire::ToolResultBlock::Image { .. })));
    }

    #[test]
    fn tool_result_image_media_type_is_corrected_from_base64_bytes() {
        let data = BASE64_STANDARD.encode(b"\x89PNG\r\n\x1a\nrest");
        let message = ChatMessage {
            role: Role::User,
            parts: vec![Part::ToolResult {
                tool_call_id: "call_1".into(),
                content: "path: image.webp".into(),
                images: vec![ToolResultImage {
                    media_type: "image/webp".into(),
                    data,
                    path: None,
                }],
                is_error: false,
                meta: None,
            }],
        };

        let wire_message = to_wire_message(&message, false)
            .expect("tool result should convert")
            .expect("message should not be empty");

        let [wire::WirePart::ToolResult { content, .. }] = wire_message.content.as_slice() else {
            panic!("expected a single tool result part");
        };
        let wire::ToolResultContent::Blocks(blocks) = content else {
            panic!("expected block tool result content");
        };
        let wire::ToolResultBlock::Image {
            source: wire::ImageSource::Base64 { media_type, .. },
        } = &blocks[1]
        else {
            panic!("expected image block");
        };

        assert_eq!(*media_type, "image/png");
    }

    #[test]
    fn beta_unavailable_error_is_not_classified_as_context_length() {
        // Real Anthropic 400 on Haiku 4.5 when we still sent the
        // `context-1m-2025-08-07` beta header.
        let message = "invalid_request_error: The long context beta is not yet available for this subscription.";
        assert!(!is_context_length_message(message));
    }

    #[test]
    fn real_context_overflow_is_classified_as_context_length() {
        let cases = [
            "invalid_request_error: prompt is too long: 250000 tokens > 200000 maximum",
            "invalid_request_error: input length and `max_tokens` exceed context window: 12345",
            "invalid_request_error: too many tokens in the request",
        ];
        for message in cases {
            assert!(
                is_context_length_message(message),
                "expected `{message}` to be classified as a context-length error"
            );
        }
    }

    #[test]
    fn common_beta_header_no_longer_advertises_long_context() {
        assert!(
            !COMMON_BETA.contains("context-1m"),
            "context-1m beta must not be advertised globally; it triggers tier-gating on Sonnet 4.6 and 400s on Haiku 4.5"
        );
    }

    /// Regression test for the "Claude Code X does not support this model;
    /// version Y or newer is required" gate: the provider must adopt the
    /// demanded version and replay the request instead of surfacing a 400.
    #[tokio::test]
    async fn retries_once_with_the_version_the_server_demands() {
        use std::sync::{Arc, Mutex};
        use tokio::io::AsyncWriteExt;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let recorded = Arc::new(Mutex::new(Vec::<String>::new()));
        let recorder = recorded.clone();

        tokio::spawn(async move {
            for attempt in 0..2 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let head = read_request(&mut socket).await;
                let user_agent = head
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.trim()
                            .eq_ignore_ascii_case("user-agent")
                            .then(|| value.trim().to_string())
                    })
                    .unwrap_or_default();
                recorder.lock().unwrap().push(user_agent);

                let body = if attempt == 0 {
                    r#"{"type":"error","error":{"type":"invalid_request_error","message":"Claude Code 2.1.251 does not support this model; version 9.9.9 or newer is required. Run 'claude update', or update the Claude desktop app, then try again."}}"#
                } else {
                    "event: message_stop\ndata: {}\n\n"
                };
                let status = if attempt == 0 {
                    "400 Bad Request"
                } else {
                    "200 OK"
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                socket.write_all(response.as_bytes()).await.unwrap();
                socket.flush().await.unwrap();
            }
        });

        let provider = AnthropicProvider::new(AnthropicConfig {
            credential: Credential::from_oauth_parts("test-access", "test-refresh", i64::MAX, None),
            base_url: format!("http://{addr}"),
            api_version: "2023-06-01".into(),
            extra_beta: None,
        })
        .unwrap();

        let request = ProviderRequest::new(
            ModelRef::new("anthropic", "claude-opus-5-5"),
            vec![ChatMessage::user_text("ping")],
        );
        let _stream = provider
            .stream(request)
            .await
            .expect("the version-gated request should be retried and succeed");

        let recorded = recorded.lock().unwrap().clone();
        assert_eq!(
            recorded.len(),
            2,
            "expected exactly one retry: {recorded:?}"
        );
        assert!(
            recorded[0].starts_with("claude-cli/"),
            "unexpected first user-agent: {}",
            recorded[0]
        );
        assert_eq!(recorded[1], "claude-cli/9.9.9");
    }

    async fn read_request(socket: &mut tokio::net::TcpStream) -> String {
        use tokio::io::AsyncReadExt;

        let mut buffer = [0u8; 1024];
        let mut received = Vec::new();
        let head_end = loop {
            let read = socket.read(&mut buffer).await.unwrap();
            if read == 0 {
                break received.len();
            }
            received.extend_from_slice(&buffer[..read]);
            if let Some(index) = received.windows(4).position(|w| w == b"\r\n\r\n") {
                break index + 4;
            }
        };
        let head = String::from_utf8_lossy(&received[..head_end]).to_string();
        let content_length: usize = head
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.trim()
                    .eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse().ok())?
            })
            .unwrap_or(0);
        let mut pending = content_length.saturating_sub(received.len() - head_end);
        while pending > 0 {
            let read = socket.read(&mut buffer).await.unwrap();
            if read == 0 {
                break;
            }
            pending = pending.saturating_sub(read);
        }
        head
    }
}
