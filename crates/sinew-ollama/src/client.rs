use std::{collections::HashMap, sync::Arc, time::Instant};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sinew_core::{
    AppError, ChatMessage, ModelCapabilities, ModelRef, Part, Provider, ProviderRequest,
    ProviderStream, Result, Role, TokenEstimate, ToolDescriptor,
};

use crate::{
    auth::{self, normalize_base_url},
    model_info::PROVIDER_ID,
    stream::map_stream,
    wire,
};

const USER_AGENT: &str = "Sinew/0.1";

#[derive(Clone)]
pub struct OllamaConfig {
    pub base_url: String,
    pub models: Vec<ModelCapabilities>,
}

impl OllamaConfig {
    pub fn new(base_url: impl Into<String>, models: Vec<ModelCapabilities>) -> Self {
        Self {
            base_url: normalize_base_url(&base_url.into()),
            models,
        }
    }

    pub fn from_default_sources(models: Vec<ModelCapabilities>) -> Result<Self> {
        let base_url = auth::load_default_base_url()?.ok_or_else(|| {
            AppError::Auth("Ollama is not connected. Connect it in Settings > Providers.".into())
        })?;
        Ok(Self::new(base_url, models))
    }
}

pub struct OllamaProvider {
    config: OllamaConfig,
    http: reqwest::Client,
    models: Arc<HashMap<String, ModelCapabilities>>,
}

impl OllamaProvider {
    pub fn new(config: OllamaConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .map_err(|err| AppError::Network(err.to_string()))?;
        let models = config
            .models
            .iter()
            .map(|caps| (caps.model.name.clone(), caps.clone()))
            .collect::<HashMap<_, _>>();
        Ok(Self {
            config,
            http,
            models: Arc::new(models),
        })
    }

    pub fn from_default_sources(models: Vec<ModelCapabilities>) -> Result<Self> {
        Self::new(OllamaConfig::from_default_sources(models)?)
    }

    fn endpoint(&self, route: &str) -> String {
        format!("{}{}", self.config.base_url.trim_end_matches('/'), route)
    }
}

#[async_trait]
impl Provider for OllamaProvider {
    fn name(&self) -> &str {
        PROVIDER_ID
    }

    fn capabilities(&self, model: &ModelRef) -> Option<ModelCapabilities> {
        if model.provider != PROVIDER_ID {
            return None;
        }
        self.models.get(&model.name).cloned()
    }

    async fn estimate_tokens(&self, request: ProviderRequest) -> Result<TokenEstimate> {
        if request.model.provider != PROVIDER_ID {
            return Err(AppError::Unsupported(format!(
                "ollama provider cannot count model provider {}",
                request.model.provider
            )));
        }
        Ok(TokenEstimate {
            input_tokens: rough_token_estimate(&request),
            exact: false,
        })
    }

    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream> {
        if request.model.provider != PROVIDER_ID {
            return Err(AppError::Unsupported(format!(
                "ollama provider cannot run model provider {}",
                request.model.provider
            )));
        }

        let caps = self.capabilities(&request.model).ok_or_else(|| {
            AppError::Unsupported(format!("model `{}` is not supported", request.model.name))
        })?;
        if !caps.supports_images && request_contains_images(&request) {
            return Err(AppError::InvalidRequest(format!(
                "Ollama model `{}` does not support image input",
                request.model.name
            )));
        }

        let body = build_chat_request(&request, &caps)?;
        let req_start = Instant::now();
        let model_name = request.model.name.clone();
        let response = self
            .http
            .post(self.endpoint("/v1/chat/completions"))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|err| AppError::Network(err.to_string()))?;
        let http_ms = req_start.elapsed().as_millis();
        tracing::debug!(provider = "ollama", model = model_name, http_ms, "HTTP round-trip");
        if !response.status().is_success() {
            return Err(read_http_error(response).await);
        }

        Ok(map_stream(response.bytes_stream(), request.model.name))
    }
}

fn build_chat_request<'a>(
    request: &'a ProviderRequest,
    caps: &ModelCapabilities,
) -> Result<wire::ChatCompletionsRequest<'a>> {
    Ok(wire::ChatCompletionsRequest {
        model: &request.model.name,
        messages: to_wire_messages(request, caps.supports_images)?,
        tools: request.tools.iter().map(to_wire_tool).collect(),
        max_tokens: Some(
            request
                .max_output_tokens
                .unwrap_or(caps.max_output_tokens)
                .min(caps.max_output_tokens),
        ),
        temperature: request.temperature,
        stream: true,
        stream_options: Some(wire::StreamOptions {
            include_usage: true,
        }),
    })
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

fn to_wire_messages<'a>(
    request: &'a ProviderRequest,
    supports_images: bool,
) -> Result<Vec<wire::WireMessage<'a>>> {
    let mut messages = Vec::new();
    if let Some(system) = request
        .system_prompt
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        messages.push(wire::WireMessage::System {
            role: "system",
            content: wire::WireContent::Text(system.to_string()),
        });
    }

    for message in &request.transcript {
        match message.role {
            Role::User => push_user_messages(message, &mut messages, supports_images),
            Role::Assistant => push_assistant_message(message, &mut messages),
        }
    }

    Ok(messages)
}

fn push_user_messages<'a>(
    message: &'a ChatMessage,
    messages: &mut Vec<wire::WireMessage<'a>>,
    supports_images: bool,
) {
    let mut builder = ContentBuilder::new(supports_images);
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
                let mut result = ContentBuilder::new(supports_images);
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

fn push_assistant_message<'a>(
    message: &'a ChatMessage,
    messages: &mut Vec<wire::WireMessage<'a>>,
) {
    let mut text = String::new();
    let mut tool_calls = Vec::new();

    for part in &message.parts {
        if part_is_ui_only(part) {
            continue;
        }
        match part {
            Part::Text { text: value, .. } => text.push_str(value),
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
            Part::Thinking { .. } | Part::Image { .. } | Part::ToolResult { .. } => {}
        }
    }

    if text.is_empty() && tool_calls.is_empty() {
        return;
    }

    let content = (!text.is_empty()).then_some(wire::WireContent::Text(text));
    messages.push(wire::WireMessage::Assistant {
        role: "assistant",
        content,
        tool_calls,
    });
}

#[derive(Default)]
struct ContentBuilder {
    text: String,
    blocks: Vec<wire::WireContentBlock>,
    has_media: bool,
    supports_images: bool,
}

impl ContentBuilder {
    fn new(supports_images: bool) -> Self {
        Self {
            supports_images,
            ..Self::default()
        }
    }

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
        if !self.supports_images {
            self.push_text(&format!("\n[Image omitted: {media_type}]\n"));
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

fn request_contains_images(request: &ProviderRequest) -> bool {
    request.transcript.iter().any(|message| {
        message.parts.iter().any(|part| match part {
            Part::Image { .. } => true,
            Part::ToolResult { images, .. } => !images.is_empty(),
            Part::Text { .. } | Part::Thinking { .. } | Part::ToolCall { .. } => false,
        })
    })
}

fn part_is_ui_only(part: &Part) -> bool {
    part_meta(part)
        .and_then(|meta| meta.get("ui_only"))
        .and_then(Value::as_bool)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaCatalogModel {
    pub id: String,
    pub name: String,
    pub context_window: u32,
    pub max_output_tokens: u32,
    pub supports_images: bool,
    pub supports_thinking: bool,
    pub supports_tools: bool,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagEntry>,
}

#[derive(Debug, Deserialize)]
struct TagEntry {
    #[serde(default)]
    name: String,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ShowResponse {
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    model_info: HashMap<String, Value>,
}

fn ollama_http() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|err| AppError::Network(err.to_string()))
}

pub async fn validate_endpoint(base_url: &str) -> Result<()> {
    let base_url = normalize_base_url(base_url);
    if base_url.is_empty() {
        return Err(AppError::Auth("Ollama address cannot be empty".into()));
    }
    let http = ollama_http()?;
    let response = http
        .get(format!("{}/api/tags", base_url))
        .send()
        .await
        .map_err(|err| {
            AppError::Network(format!(
                "Unable to reach Ollama at {base_url}: {err}. Is `ollama serve` running?"
            ))
        })?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(read_http_error(response).await)
    }
}

pub async fn fetch_model_catalog(base_url: &str) -> Result<Vec<OllamaCatalogModel>> {
    let base_url = normalize_base_url(base_url);
    if base_url.is_empty() {
        return Err(AppError::Auth("Ollama address cannot be empty".into()));
    }
    let http = ollama_http()?;
    let response = http
        .get(format!("{}/api/tags", base_url))
        .send()
        .await
        .map_err(|err| {
            AppError::Network(format!("Unable to reach Ollama at {base_url}: {err}"))
        })?;
    if !response.status().is_success() {
        return Err(read_http_error(response).await);
    }
    let tags: TagsResponse = response
        .json()
        .await
        .map_err(|err| AppError::Decode(format!("invalid Ollama tags body: {err}")))?;

    let mut catalog = Vec::with_capacity(tags.models.len());
    for entry in tags.models {
        let id = entry.model.unwrap_or_default();
        let id = if id.trim().is_empty() { entry.name.clone() } else { id };
        let id = id.trim().to_string();
        if id.is_empty() {
            continue;
        }
        let details = fetch_model_details(&http, &base_url, &id).await;
        catalog.push(catalog_model_from_details(id, details));
    }
    Ok(catalog)
}

async fn fetch_model_details(
    http: &reqwest::Client,
    base_url: &str,
    model: &str,
) -> Option<ShowResponse> {
    let response = http
        .post(format!("{}/api/show", base_url))
        .json(&serde_json::json!({ "model": model }))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.json::<ShowResponse>().await.ok()
}

fn catalog_model_from_details(id: String, details: Option<ShowResponse>) -> OllamaCatalogModel {
    let (context_window, supports_images, supports_thinking, supports_tools) = match &details {
        Some(show) => {
            let context = show
                .model_info
                .iter()
                .find(|(key, _)| key.ends_with(".context_length"))
                .and_then(|(_, value)| value.as_u64())
                .map(|value| value.min(u32::MAX as u64) as u32)
                .unwrap_or(8_192)
                .max(1);
            let caps = &show.capabilities;
            (
                context,
                caps.iter().any(|c| c == "vision"),
                caps.iter().any(|c| c == "thinking"),
                caps.iter().any(|c| c == "tools"),
            )
        }
        None => (8_192, false, false, true),
    };
    let max_output_tokens = context_window.min(16_384).max(1);
    OllamaCatalogModel {
        id: id.clone(),
        name: id,
        context_window,
        max_output_tokens,
        supports_images,
        supports_thinking,
        supports_tools,
    }
}

async fn read_http_error(response: reqwest::Response) -> AppError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let parsed: std::result::Result<wire::ApiErrorEnvelope, _> = serde_json::from_str(&body);
    let message = parsed
        .ok()
        .and_then(|payload| {
            if payload.error.message.trim().is_empty() {
                None
            } else {
                Some(payload.error.message)
            }
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(body);

    if status == reqwest::StatusCode::NOT_FOUND {
        AppError::InvalidRequest(if message.trim().is_empty() {
            "Ollama model not found. Pull it first with `ollama pull <model>`.".into()
        } else {
            message
        })
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
