use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use claakecode_core::{
    AppError, ChatMessage, Effort, ModelCapabilities, ModelRef, Part, Provider, ProviderRequest,
    ProviderStream, Result, Role, TokenEstimate, ToolDescriptor,
};

use crate::{
    auth::{common_headers, Credential, KIMI_RECONNECT_MESSAGE},
    model_info,
    stream::map_stream,
    wire,
};

const BASE_URL: &str = "https://api.kimi.com/coding/v1";
const USER_AGENT: &str = "KimiCLI/0.1 ClaakeCode/0.1";

#[derive(Clone)]
pub struct KimiConfig {
    pub credential: Credential,
    pub base_url: String,
}

impl KimiConfig {
    pub fn new(credential: Credential) -> Self {
        Self {
            credential,
            base_url: BASE_URL.into(),
        }
    }

    pub fn from_default_sources() -> Result<Self> {
        if let Some(credential) = Credential::load_default()? {
            return Ok(Self::new(credential));
        }

        Err(AppError::Auth(
            "no kimi oauth credential found. Connect Kimi in Settings > Providers.".into(),
        ))
    }
}

pub struct KimiProvider {
    config: KimiConfig,
    http: reqwest::Client,
}

impl KimiProvider {
    pub fn new(config: KimiConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .map_err(|err| AppError::Network(err.to_string()))?;
        Ok(Self { config, http })
    }

    pub fn from_default_sources() -> Result<Self> {
        Self::new(KimiConfig::from_default_sources()?)
    }

    async fn post(&self, route: &str) -> Result<(reqwest::RequestBuilder, String)> {
        let token = self.config.credential.bearer(&self.http).await?;
        let mut request = self
            .http
            .post(format!(
                "{}{}",
                self.config.base_url.trim_end_matches('/'),
                route
            ))
            .bearer_auth(&token)
            .header("content-type", "application/json")
            .header("accept", "application/json");
        for (key, value) in common_headers()? {
            request = request.header(key, value);
        }
        Ok((request, token))
    }

    async fn send_json<T: Serialize + ?Sized>(
        &self,
        route: &str,
        body: &T,
    ) -> Result<reqwest::Response> {
        let (request, token) = self.post(route).await?;
        let response = request
            .json(body)
            .send()
            .await
            .map_err(|err| AppError::Network(err.to_string()))?;

        if response.status() != reqwest::StatusCode::UNAUTHORIZED {
            return Ok(response);
        }

        self.config
            .credential
            .force_refresh(&self.http, &token)
            .await
            .map_err(map_refresh_failure)?;

        let (request, _) = self.post(route).await?;
        request
            .json(body)
            .send()
            .await
            .map_err(|err| AppError::Network(err.to_string()))
    }
}

#[async_trait]
impl Provider for KimiProvider {
    fn name(&self) -> &str {
        "kimi"
    }

    fn capabilities(&self, model: &ModelRef) -> Option<ModelCapabilities> {
        if model.provider != "kimi" {
            return None;
        }
        Some(model_info::capabilities(model))
    }

    async fn estimate_tokens(&self, request: ProviderRequest) -> Result<TokenEstimate> {
        if request.model.provider != "kimi" {
            return Err(AppError::Unsupported(format!(
                "kimi provider cannot count model provider {}",
                request.model.provider
            )));
        }
        Ok(TokenEstimate {
            input_tokens: rough_token_estimate(&request),
            exact: false,
        })
    }

    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream> {
        if request.model.provider != "kimi" {
            return Err(AppError::Unsupported(format!(
                "kimi provider cannot run model provider {}",
                request.model.provider
            )));
        }

        let caps = model_info::capabilities(&request.model);
        let (reasoning_effort, thinking) = effort_to_thinking(request.effective_effort());
        let body = wire::ChatCompletionsRequest {
            model: &request.model.name,
            messages: to_wire_messages(&request)?,
            tools: request.tools.iter().map(to_wire_tool).collect(),
            max_tokens: Some(
                request
                    .max_output_tokens
                    .unwrap_or(caps.max_output_tokens)
                    .min(caps.max_output_tokens),
            ),
            temperature: request.temperature,
            prompt_cache_key: request.cache_key.as_deref(),
            reasoning_effort,
            thinking,
            stream: true,
            stream_options: Some(wire::StreamOptions {
                include_usage: true,
            }),
        };

        let response = self.send_json("/chat/completions", &body).await?;
        if !response.status().is_success() {
            return Err(read_http_error(response).await);
        }

        Ok(map_stream(response.bytes_stream(), request.model.name))
    }
}

fn effort_to_thinking(
    effort: Option<Effort>,
) -> (Option<&'static str>, Option<wire::ThinkingConfig>) {
    match effort.unwrap_or(Effort::High) {
        Effort::None => (None, Some(wire::ThinkingConfig { kind: "disabled" })),
        Effort::Low => (Some("low"), Some(wire::ThinkingConfig { kind: "enabled" })),
        Effort::Medium => (
            Some("medium"),
            Some(wire::ThinkingConfig { kind: "enabled" }),
        ),
        Effort::High | Effort::Xhigh | Effort::Max => {
            (Some("high"), Some(wire::ThinkingConfig { kind: "enabled" }))
        }
    }
}

fn to_wire_tool(tool: &ToolDescriptor) -> wire::WireTool<'_> {
    wire::WireTool {
        kind: "function",
        function: wire::WireToolFunction {
            name: &tool.name,
            description: &tool.description,
            parameters: &tool.input_schema,
        },
    }
}

fn to_wire_messages<'a>(request: &'a ProviderRequest) -> Result<Vec<wire::WireMessage<'a>>> {
    let mut messages = Vec::new();
    if let Some(system) = request
        .system_prompt
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        messages.push(wire::WireMessage::System {
            role: "system",
            content: system,
        });
    }

    for message in &request.transcript {
        match message.role {
            Role::User => push_user_messages(message, &mut messages),
            Role::Assistant => push_assistant_message(message, &mut messages),
        }
    }

    Ok(messages)
}

fn push_user_messages<'a>(message: &'a ChatMessage, messages: &mut Vec<wire::WireMessage<'a>>) {
    let mut builder = ContentBuilder::default();
    for part in &message.parts {
        if part_is_ui_only(part) {
            continue;
        }
        match part {
            Part::Text { text, .. } => builder.push_text(text),
            Part::Image {
                media_type, data, ..
            } => builder.push_image(media_type, data),
            Part::ToolResult {
                tool_call_id,
                content,
                images,
                ..
            } => {
                flush_user_builder(&mut builder, messages);
                let mut result = ContentBuilder::default();
                result.push_text(content);
                for image in images {
                    if !image.data.trim().is_empty() {
                        result.push_image(&image.media_type, &image.data);
                    }
                }
                let content = result
                    .finish_allow_empty()
                    .unwrap_or_else(|| wire::WireContent::Text(String::new()));
                messages.push(wire::WireMessage::Tool {
                    role: "tool",
                    content,
                    tool_call_id,
                });
            }
            Part::Thinking { .. } | Part::ToolCall { .. } => {}
        }
    }
    flush_user_builder(&mut builder, messages);
}

fn flush_user_builder<'a>(builder: &mut ContentBuilder, messages: &mut Vec<wire::WireMessage<'a>>) {
    if let Some(content) = builder.finish() {
        messages.push(wire::WireMessage::User {
            role: "user",
            content,
        });
    }
}

fn push_assistant_message<'a>(message: &'a ChatMessage, messages: &mut Vec<wire::WireMessage<'a>>) {
    let mut text = String::new();
    let mut reasoning = String::new();
    let mut tool_calls = Vec::new();

    for part in &message.parts {
        if part_is_ui_only(part) {
            continue;
        }
        match part {
            Part::Text { text: value, .. } => text.push_str(value),
            Part::Thinking { text: value, .. } => reasoning.push_str(value),
            Part::ToolCall {
                id, name, input, ..
            } => tool_calls.push(wire::WireToolCall {
                id,
                kind: "function",
                function: wire::WireToolCallFunction {
                    name,
                    arguments: input.to_string(),
                },
            }),
            Part::Image { .. } | Part::ToolResult { .. } => {}
        }
    }

    if text.is_empty() && reasoning.is_empty() && tool_calls.is_empty() {
        return;
    }

    let content = (!text.is_empty()).then_some(wire::WireContent::Text(text));
    let reasoning_content = (!reasoning.is_empty()).then_some(reasoning);
    messages.push(wire::WireMessage::Assistant {
        role: "assistant",
        content,
        reasoning_content,
        tool_calls,
    });
}

#[derive(Default)]
struct ContentBuilder {
    text: String,
    blocks: Vec<wire::WireContentBlock>,
    has_media: bool,
}

impl ContentBuilder {
    fn push_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if self.has_media {
            self.blocks.push(wire::WireContentBlock::Text {
                text: text.to_string(),
            });
        } else {
            self.text.push_str(text);
        }
    }

    fn push_image(&mut self, media_type: &str, data: &str) {
        if data.trim().is_empty() {
            return;
        }
        if !self.has_media {
            self.has_media = true;
            if !self.text.is_empty() {
                self.blocks.push(wire::WireContentBlock::Text {
                    text: std::mem::take(&mut self.text),
                });
            }
        }
        self.blocks.push(wire::WireContentBlock::ImageUrl {
            image_url: wire::WireImageUrl {
                url: format!("data:{media_type};base64,{data}"),
            },
        });
    }

    fn finish(&mut self) -> Option<wire::WireContent> {
        self.finish_inner(false)
    }

    fn finish_allow_empty(&mut self) -> Option<wire::WireContent> {
        self.finish_inner(true)
    }

    fn finish_inner(&mut self, allow_empty_text: bool) -> Option<wire::WireContent> {
        if self.has_media {
            if self.blocks.is_empty() {
                return None;
            }
            self.has_media = false;
            return Some(wire::WireContent::Blocks(std::mem::take(&mut self.blocks)));
        }
        if self.text.is_empty() && !allow_empty_text {
            return None;
        }
        Some(wire::WireContent::Text(std::mem::take(&mut self.text)))
    }
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
                Part::Image { .. } => chars += 4_000,
                Part::ToolCall { name, input, .. } => {
                    chars += name.chars().count();
                    chars += input.to_string().chars().count();
                }
                Part::ToolResult {
                    content, images, ..
                } => {
                    chars += content.chars().count();
                    chars += images.len() * 4_000;
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

fn map_refresh_failure(err: AppError) -> AppError {
    tracing::warn!(error = %err, "failed to refresh kimi oauth token after auth failure");
    match err {
        AppError::Network(_) => AppError::Network(
            "Could not refresh Kimi login. Check your connection and try again.".into(),
        ),
        _ => AppError::Auth(KIMI_RECONNECT_MESSAGE.into()),
    }
}

async fn read_http_error(response: reqwest::Response) -> AppError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let parsed: std::result::Result<wire::ApiErrorEnvelope, _> = serde_json::from_str(&body);
    let message = parsed
        .ok()
        .map(|payload| {
            let code = payload.error.code.unwrap_or_default();
            if code.is_empty() {
                format!("{}: {}", payload.error.kind, payload.error.message)
            } else {
                format!("{} ({code}): {}", payload.error.kind, payload.error.message)
            }
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(body);

    if status == reqwest::StatusCode::UNAUTHORIZED {
        AppError::Auth(KIMI_RECONNECT_MESSAGE.into())
    } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        AppError::RateLimit(message)
    } else if status.is_client_error() {
        if message.contains("context") || message.contains("too long") {
            AppError::ContextLength(message)
        } else {
            AppError::InvalidRequest(message)
        }
    } else {
        AppError::Provider(format!("HTTP {status}: {message}"))
    }
}
