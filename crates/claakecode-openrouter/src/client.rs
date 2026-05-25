use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use claakecode_core::{
    AppError, ChatMessage, Effort, ModelCapabilities, ModelRef, Part, Provider, ProviderRequest,
    ProviderStream, Result, Role, TokenEstimate, ToolDescriptor,
};

use crate::{
    auth::Credential,
    model_info::{self, PROVIDER_ID},
    stream::map_stream,
    wire,
};

const BASE_URL: &str = "https://openrouter.ai/api/v1";
const USER_AGENT: &str = "ClaakeCode/0.1";
const APP_REFERER: &str = "https://github.com/WilliamPeynichou/ClaakeCode";
const APP_TITLE: &str = "Claake Code";
const CACHE_BREAKPOINTS: usize = 4;

#[derive(Clone)]
pub struct OpenRouterConfig {
    pub credential: Credential,
    pub base_url: String,
    pub app_referer: String,
    pub app_title: String,
    pub models: Vec<ModelCapabilities>,
}

impl OpenRouterConfig {
    pub fn new(credential: Credential, models: Vec<ModelCapabilities>) -> Self {
        Self {
            credential,
            base_url: BASE_URL.into(),
            app_referer: APP_REFERER.into(),
            app_title: APP_TITLE.into(),
            models,
        }
    }

    pub fn from_default_sources(models: Vec<ModelCapabilities>) -> Result<Self> {
        if let Some(credential) = Credential::load_default()? {
            return Ok(Self::new(credential, models));
        }

        Err(AppError::Auth(
            "no OpenRouter API key found. Add one in Settings > Providers.".into(),
        ))
    }
}

pub struct OpenRouterProvider {
    config: OpenRouterConfig,
    http: reqwest::Client,
    models: Arc<HashMap<String, ModelCapabilities>>,
}

impl OpenRouterProvider {
    pub fn new(config: OpenRouterConfig) -> Result<Self> {
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
        Self::new(OpenRouterConfig::from_default_sources(models)?)
    }

    fn post(&self, route: &str) -> reqwest::RequestBuilder {
        request_with_headers(
            &self.http,
            reqwest::Method::POST,
            &self.config.base_url,
            route,
            self.config.credential.api_key(),
            &self.config.app_referer,
            &self.config.app_title,
        )
        .header("content-type", "application/json")
    }
}

#[async_trait]
impl Provider for OpenRouterProvider {
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
                "openrouter provider cannot count model provider {}",
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
                "openrouter provider cannot run model provider {}",
                request.model.provider
            )));
        }

        let caps = self.capabilities(&request.model).ok_or_else(|| {
            AppError::Unsupported(format!("model `{}` is not supported", request.model.name))
        })?;
        if !caps.supports_images && request_contains_images(&request) {
            return Err(AppError::InvalidRequest(format!(
                "OpenRouter model `{}` does not support image input",
                request.model.name
            )));
        }

        let body = build_chat_request(&request, &caps)?;

        let response = self
            .post("/chat/completions")
            .json(&body)
            .send()
            .await
            .map_err(|err| AppError::Network(err.to_string()))?;
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
    let cache_mode = cache_mode_for_model(&request.model.name);
    Ok(wire::ChatCompletionsRequest {
        model: &request.model.name,
        cache_control: top_level_cache_control(cache_mode),
        messages: to_wire_messages(request, caps.supports_images, cache_mode)?,
        tools: request.tools.iter().map(to_wire_tool).collect(),
        max_tokens: Some(
            request
                .max_output_tokens
                .unwrap_or(caps.max_output_tokens)
                .min(caps.max_output_tokens),
        ),
        temperature: request.temperature,
        reasoning: reasoning_config(caps.supports_thinking, request.effective_effort()),
        stream: true,
        stream_options: Some(wire::StreamOptions {
            include_usage: true,
        }),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CacheMode {
    None,
    AnthropicTopLevel,
    ExplicitBreakpoints,
}

fn cache_mode_for_model(model: &str) -> CacheMode {
    let model = model.trim().to_ascii_lowercase();
    if model.starts_with("anthropic/") {
        return CacheMode::AnthropicTopLevel;
    }
    if model.starts_with("google/gemini") || explicit_alibaba_cache_model(&model) {
        return CacheMode::ExplicitBreakpoints;
    }
    CacheMode::None
}

fn explicit_alibaba_cache_model(model: &str) -> bool {
    model.starts_with("deepseek/deepseek-v3.2")
        || matches!(
            model,
            "qwen/qwen3-max"
                | "qwen/qwen-plus"
                | "qwen/qwen3.6-plus"
                | "qwen/qwen3-coder-plus"
                | "qwen/qwen3-coder-flash"
        )
}

fn top_level_cache_control(mode: CacheMode) -> Option<wire::CacheControl> {
    matches!(mode, CacheMode::AnthropicTopLevel).then(cache_control)
}

fn cache_control() -> wire::CacheControl {
    wire::CacheControl {
        kind: "ephemeral",
        ttl: None,
    }
}

fn reasoning_config(
    supports_thinking: bool,
    effort: Option<Effort>,
) -> Option<wire::ReasoningConfig> {
    if !supports_thinking {
        return None;
    }
    let effort = match effort.unwrap_or(Effort::Medium) {
        Effort::None => "none",
        Effort::Low => "low",
        Effort::Medium => "medium",
        Effort::High | Effort::Xhigh | Effort::Max => "high",
    };
    Some(wire::ReasoningConfig {
        effort: Some(effort),
        enabled: None,
        exclude: false,
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
    cache_mode: CacheMode,
) -> Result<Vec<wire::WireMessage<'a>>> {
    let mut messages = Vec::new();
    let explicit_cache = matches!(cache_mode, CacheMode::ExplicitBreakpoints);
    let mut cache_budget = if explicit_cache { CACHE_BREAKPOINTS } else { 0 };
    if let Some(system) = request
        .system_prompt
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        messages.push(wire::WireMessage::System {
            role: "system",
            content: text_content(system, take_cache_breakpoint(&mut cache_budget, true)),
        });
    }

    let stable_message_count = request
        .cache_stable_message_count
        .unwrap_or(request.transcript.len())
        .min(request.transcript.len());
    let cached_messages = if explicit_cache {
        cache_message_indices(&request.transcript[..stable_message_count], cache_budget)
    } else {
        Vec::new()
    };

    for (index, message) in request.transcript.iter().enumerate() {
        let cache = cached_messages.contains(&index);
        match message.role {
            Role::User => push_user_messages(message, &mut messages, supports_images, cache),
            Role::Assistant => push_assistant_message(message, &mut messages, cache),
        }
    }

    Ok(messages)
}

fn text_content(text: &str, cache: bool) -> wire::WireContent {
    if cache {
        wire::WireContent::Blocks(vec![wire::WireContentBlock::Text {
            text: text.to_string(),
            cache_control: Some(cache_control()),
        }])
    } else {
        wire::WireContent::Text(text.to_string())
    }
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
        .filter_map(|(index, message)| {
            message
                .parts
                .iter()
                .any(explicit_cacheable_part)
                .then_some(index)
        })
        .take(limit)
        .collect::<Vec<_>>();
    indices.reverse();
    indices
}

fn explicit_cacheable_part(part: &Part) -> bool {
    if part_is_ui_only(part) {
        return false;
    }
    match part {
        Part::Text { text, .. } => !text.is_empty(),
        Part::ToolResult { content, .. } => !content.is_empty(),
        Part::Image { .. } | Part::Thinking { .. } | Part::ToolCall { .. } => false,
    }
}

fn mark_last_cacheable_message_content(messages: &mut [wire::WireMessage<'_>]) -> bool {
    for message in messages.iter_mut().rev() {
        let content = match message {
            wire::WireMessage::System { content, .. }
            | wire::WireMessage::User { content, .. }
            | wire::WireMessage::Tool { content, .. } => Some(content),
            wire::WireMessage::Assistant { content, .. } => content.as_mut(),
        };
        if let Some(content) = content {
            if mark_content_cache_control(content) {
                return true;
            }
        }
    }
    false
}

fn mark_content_cache_control(content: &mut wire::WireContent) -> bool {
    match content {
        wire::WireContent::Text(text) => {
            if text.is_empty() {
                return false;
            }
            *content = wire::WireContent::Blocks(vec![wire::WireContentBlock::Text {
                text: std::mem::take(text),
                cache_control: Some(cache_control()),
            }]);
            true
        }
        wire::WireContent::Blocks(blocks) => {
            for block in blocks.iter_mut().rev() {
                if let wire::WireContentBlock::Text {
                    text,
                    cache_control: block_cache_control,
                } = block
                {
                    if !text.is_empty() {
                        *block_cache_control = Some(cache_control());
                        return true;
                    }
                }
            }
            false
        }
    }
}

fn push_user_messages<'a>(
    message: &'a ChatMessage,
    messages: &mut Vec<wire::WireMessage<'a>>,
    supports_images: bool,
    cache: bool,
) {
    let start = messages.len();
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
    if cache {
        mark_last_cacheable_message_content(&mut messages[start..]);
    }
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
    cache: bool,
) {
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

    let mut content = (!text.is_empty()).then_some(wire::WireContent::Text(text));
    if cache {
        if let Some(content) = &mut content {
            mark_content_cache_control(content);
        }
    }
    let reasoning = (!reasoning.is_empty()).then_some(reasoning);
    messages.push(wire::WireMessage::Assistant {
        role: "assistant",
        content,
        reasoning,
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
                cache_control: None,
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
                    cache_control: None,
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
pub struct OpenRouterCatalogModel {
    pub id: String,
    pub name: String,
    pub context_window: u32,
    pub max_output_tokens: u32,
    pub supports_images: bool,
    pub supports_thinking: bool,
    pub supports_tools: bool,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelBody>,
}

#[derive(Debug, Deserialize)]
struct ModelBody {
    id: String,
    name: String,
    #[serde(default)]
    context_length: Option<u32>,
    #[serde(default)]
    architecture: Option<ModelArchitecture>,
    #[serde(default)]
    top_provider: Option<TopProvider>,
    #[serde(default)]
    per_request_limits: Option<PerRequestLimits>,
    #[serde(default)]
    supported_parameters: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ModelArchitecture {
    #[serde(default)]
    input_modalities: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct TopProvider {
    #[serde(default)]
    context_length: Option<u32>,
    #[serde(default)]
    max_completion_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct PerRequestLimits {
    #[serde(default)]
    completion_tokens: Option<f64>,
}

pub async fn validate_api_key(api_key: &str) -> Result<()> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(AppError::Auth("OpenRouter API key cannot be empty".into()));
    }
    let http = openrouter_http()?;
    let response = request_with_headers(
        &http,
        reqwest::Method::GET,
        BASE_URL,
        "/auth/key",
        api_key,
        APP_REFERER,
        APP_TITLE,
    )
    .send()
    .await
    .map_err(|err| AppError::Network(format!("OpenRouter key validation failed: {err}")))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        fetch_model_catalog_with_http(&http, api_key)
            .await
            .map(|_| ())
    } else if response.status().is_success() {
        Ok(())
    } else {
        Err(read_http_error(response).await)
    }
}

pub async fn fetch_model_catalog(api_key: &str) -> Result<Vec<OpenRouterCatalogModel>> {
    let http = openrouter_http()?;
    fetch_model_catalog_with_http(&http, api_key).await
}

async fn fetch_model_catalog_with_http(
    http: &reqwest::Client,
    api_key: &str,
) -> Result<Vec<OpenRouterCatalogModel>> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(AppError::Auth("OpenRouter API key cannot be empty".into()));
    }
    let response = request_with_headers(
        http,
        reqwest::Method::GET,
        BASE_URL,
        "/models?output_modalities=text",
        api_key,
        APP_REFERER,
        APP_TITLE,
    )
    .send()
    .await
    .map_err(|err| AppError::Network(format!("OpenRouter model search failed: {err}")))?;
    if !response.status().is_success() {
        return Err(read_http_error(response).await);
    }
    let body: ModelsResponse = response
        .json()
        .await
        .map_err(|err| AppError::Decode(format!("invalid OpenRouter models body: {err}")))?;
    Ok(body.data.into_iter().map(catalog_model_from_body).collect())
}

fn catalog_model_from_body(body: ModelBody) -> OpenRouterCatalogModel {
    let context_window = body
        .context_length
        .or_else(|| {
            body.top_provider
                .as_ref()
                .and_then(|top| top.context_length)
        })
        .unwrap_or(128_000)
        .max(1);
    let max_output_tokens = body
        .top_provider
        .as_ref()
        .and_then(|top| top.max_completion_tokens)
        .or_else(|| {
            body.per_request_limits
                .as_ref()
                .and_then(|limits| limits.completion_tokens)
                .and_then(f64_to_u32)
        })
        .unwrap_or_else(|| context_window.min(16_384))
        .max(1)
        .min(context_window);
    let params = body
        .supported_parameters
        .iter()
        .map(|param| param.as_str())
        .collect::<Vec<_>>();
    let supports_thinking = params.iter().any(|param| {
        matches!(
            *param,
            "reasoning" | "reasoning_effort" | "include_reasoning"
        )
    }) || body.id.contains(":thinking")
        || body.name.to_ascii_lowercase().contains("thinking");
    let supports_images = body
        .architecture
        .as_ref()
        .map(|architecture| {
            architecture
                .input_modalities
                .iter()
                .any(|modality| modality == "image")
        })
        .unwrap_or(false);
    OpenRouterCatalogModel {
        id: body.id,
        name: body.name,
        context_window,
        max_output_tokens,
        supports_images,
        supports_thinking,
        supports_tools: true,
    }
}

fn f64_to_u32(value: f64) -> Option<u32> {
    if value.is_finite() && value > 0.0 && value <= u32::MAX as f64 {
        Some(value as u32)
    } else {
        None
    }
}

fn openrouter_http() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|err| AppError::Network(err.to_string()))
}

fn request_with_headers(
    http: &reqwest::Client,
    method: reqwest::Method,
    base_url: &str,
    route: &str,
    api_key: &str,
    app_referer: &str,
    app_title: &str,
) -> reqwest::RequestBuilder {
    let url = format!("{}{}", base_url.trim_end_matches('/'), route);
    http.request(method, url)
        .bearer_auth(api_key)
        .header("accept", "application/json")
        .header("HTTP-Referer", app_referer)
        .header("X-OpenRouter-Title", app_title)
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
            } else if let Some(kind) = payload.error.kind.filter(|value| !value.trim().is_empty()) {
                Some(format!("{kind}: {}", payload.error.message))
            } else {
                Some(payload.error.message)
            }
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(body);

    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        AppError::Auth(if message.trim().is_empty() {
            "OpenRouter API key is invalid or expired".into()
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

#[allow(dead_code)]
fn _capabilities_for_catalog(model: &OpenRouterCatalogModel) -> ModelCapabilities {
    model_info::capabilities_from_catalog_model(model)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use claakecode_core::{ChatMessage, ModelCapabilities, ModelRef, ProviderRequest};

    use super::{build_chat_request, cache_mode_for_model, model_info, CacheMode};

    fn caps(model: &str) -> ModelCapabilities {
        model_info::capabilities_from_parts(model, 128_000, 8_192, false, false, true)
    }

    #[test]
    fn anthropic_models_use_top_level_cache_control() {
        let request = ProviderRequest::new(
            ModelRef::new(model_info::PROVIDER_ID, "anthropic/claude-sonnet-4.6"),
            vec![ChatMessage::user_text("hello")],
        )
        .with_system("stable system")
        .with_cache_stable_message_count(1);
        let body = build_chat_request(&request, &caps(&request.model.name)).unwrap();
        let value = serde_json::to_value(&body).unwrap();

        assert_eq!(value["cache_control"], json!({ "type": "ephemeral" }));
        assert_eq!(value["messages"][0]["content"], "stable system");
        assert_eq!(value["messages"][1]["content"], "hello");
    }

    #[test]
    fn explicit_cache_models_mark_only_stable_messages() {
        let request = ProviderRequest::new(
            ModelRef::new(model_info::PROVIDER_ID, "google/gemini-2.5-pro"),
            vec![
                ChatMessage::user_text("stable reference"),
                ChatMessage::user_text("current question"),
            ],
        )
        .with_cache_stable_message_count(1);
        let body = build_chat_request(&request, &caps(&request.model.name)).unwrap();
        let value = serde_json::to_value(&body).unwrap();

        assert!(value.get("cache_control").is_none());
        assert_eq!(
            value["messages"][0]["content"],
            json!([
                {
                    "type": "text",
                    "text": "stable reference",
                    "cache_control": { "type": "ephemeral" }
                }
            ])
        );
        assert_eq!(value["messages"][1]["content"], "current question");
    }

    #[test]
    fn automatic_cache_models_do_not_emit_cache_control() {
        let request = ProviderRequest::new(
            ModelRef::new(model_info::PROVIDER_ID, "openai/gpt-5"),
            vec![ChatMessage::user_text("hello")],
        )
        .with_cache_stable_message_count(1);
        let body = build_chat_request(&request, &caps(&request.model.name)).unwrap();
        let value = serde_json::to_value(&body).unwrap();

        assert!(value.get("cache_control").is_none());
        assert_eq!(value["messages"][0]["content"], "hello");
    }

    #[test]
    fn alibaba_cache_detection_avoids_unsupported_snapshots() {
        assert_eq!(
            cache_mode_for_model("qwen/qwen3-coder-plus"),
            CacheMode::ExplicitBreakpoints
        );
        assert_eq!(
            cache_mode_for_model("deepseek/deepseek-v3.2"),
            CacheMode::ExplicitBreakpoints
        );
        assert_eq!(
            cache_mode_for_model("qwen/qwen3.5-plus-02-15"),
            CacheMode::None
        );
    }
}
