use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use sinew_core::{
    AppError, ChatMessage, ModelCapabilities, ModelRef, Part, Provider, ProviderRequest,
    ProviderStream, Result, Role, TokenEstimate, ToolDescriptor,
};

use crate::{
    auth::Credential,
    model_info::{self, DEEPSEEK_CHAT_MODEL, DEEPSEEK_REASONER_MODEL},
    stream::map_stream,
    wire,
};

const BASE_URL: &str = "https://api.deepseek.com";
const USER_AGENT: &str = "sinew/0.1";

#[derive(Clone)]
pub struct DeepSeekConfig {
    pub credential: Credential,
    pub base_url: String,
}

impl DeepSeekConfig {
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
            "no deepseek api key found. Connect DeepSeek in Settings > Providers.".into(),
        ))
    }
}

pub struct DeepSeekProvider {
    config: DeepSeekConfig,
    http: reqwest::Client,
}

impl DeepSeekProvider {
    pub fn new(config: DeepSeekConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .map_err(|err| AppError::Network(err.to_string()))?;
        Ok(Self { config, http })
    }

    pub fn from_default_sources() -> Result<Self> {
        Self::new(DeepSeekConfig::from_default_sources()?)
    }

    async fn post(&self, route: &str) -> Result<reqwest::RequestBuilder> {
        let request = self
            .http
            .post(format!(
                "{}{}",
                self.config.base_url.trim_end_matches('/'),
                route
            ))
            .bearer_auth(self.config.credential.api_key())
            .header("content-type", "application/json")
            .header("accept", "application/json");
        Ok(request)
    }

    async fn send_json<T: Serialize + ?Sized>(
        &self,
        route: &str,
        body: &T,
    ) -> Result<reqwest::Response> {
        let request = self.post(route).await?;
        request
            .json(body)
            .send()
            .await
            .map_err(|err| AppError::Network(err.to_string()))
    }
}

#[async_trait]
impl Provider for DeepSeekProvider {
    fn name(&self) -> &str {
        "deepseek"
    }

    fn capabilities(&self, model: &ModelRef) -> Option<ModelCapabilities> {
        if model.provider != "deepseek" {
            return None;
        }
        Some(model_info::capabilities(model))
    }

    async fn estimate_tokens(&self, request: ProviderRequest) -> Result<TokenEstimate> {
        if request.model.provider != "deepseek" {
            return Err(AppError::Unsupported(format!(
                "deepseek provider cannot count model provider {}",
                request.model.provider
            )));
        }
        Ok(TokenEstimate {
            input_tokens: rough_token_estimate(&request),
            exact: false,
        })
    }

    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream> {
        if request.model.provider != "deepseek" {
            return Err(AppError::Unsupported(format!(
                "deepseek provider cannot run model provider {}",
                request.model.provider
            )));
        }

        let caps = model_info::capabilities(&request.model);
        let body = wire::ChatCompletionsRequest {
            model: &request.model.name,
            messages: to_wire_messages(&request)?,
            tools: if caps.supports_tools {
                request.tools.iter().map(to_wire_tool).collect()
            } else {
                Vec::new()
            },
            max_tokens: request.max_output_tokens.or(Some(caps.max_output_tokens)),
            temperature: request.temperature,
            stream: true,
        };

        let response = self.send_json("/chat/completions", &body).await?;
        if !response.status().is_success() {
            return Err(read_http_error(response).await);
        }

        Ok(map_stream(response.bytes_stream(), request.model.name))
    }
}

pub async fn validate_api_key(api_key: &str) -> std::result::Result<(), String> {
    let http = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|err| err.to_string())?;

    let body = serde_json::json!({
        "model": DEEPSEEK_CHAT_MODEL,
        "messages": [{"role": "user", "content": "ping"}],
        "max_tokens": 1
    });

    let response = http
        .post(format!("{}/chat/completions", BASE_URL))
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|err| format!("Network error: {err}"))?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("HTTP {}", response.status()))
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
            Part::ToolResult {
                tool_call_id,
                content,
                ..
            } => {
                flush_user_builder(&mut builder, messages);
                let mut result = ContentBuilder::default();
                result.push_text(content);
                let content = result
                    .finish_allow_empty()
                    .unwrap_or_else(|| wire::WireContent::Text(String::new()));
                messages.push(wire::WireMessage::Tool {
                    role: "tool",
                    content,
                    tool_call_id,
                });
            }
            Part::Thinking { .. } | Part::ToolCall { .. } | Part::Image { .. } => {}
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
}

impl ContentBuilder {
    fn push_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.text.push_str(text);
    }

    fn finish(&mut self) -> Option<wire::WireContent> {
        self.finish_inner(false)
    }

    fn finish_allow_empty(&mut self) -> Option<wire::WireContent> {
        self.finish_inner(true)
    }

    fn finish_inner(&mut self, allow_empty_text: bool) -> Option<wire::WireContent> {
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
                Part::Text { text, .. } => chars += text.chars().count(),
                Part::Thinking { text, .. } => chars += text.chars().count(),
                Part::ToolCall { name, input, .. } => {
                    chars += name.chars().count() + input.to_string().chars().count()
                }
                Part::ToolResult { content, .. } => chars += content.chars().count(),
                Part::Image { .. } => {}
            }
        }
    }
    (chars / 4) as u32
}

async fn read_http_error(response: reqwest::Response) -> AppError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    if status == reqwest::StatusCode::UNAUTHORIZED {
        AppError::Auth("DeepSeek API key is invalid or unauthorized. Please verify your API key in Settings > Providers.".into())
    } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        AppError::RateLimit(format!("DeepSeek API rate limit exceeded: {body}"))
    } else if status.is_client_error() {
        if body.contains("context") || body.contains("too long") {
            AppError::ContextLength(body)
        } else {
            AppError::InvalidRequest(body)
        }
    } else {
        AppError::Provider(format!("HTTP {status}: {body}"))
    }
}
