use async_trait::async_trait;
use futures::stream;
use reqwest::Client;
use sinew_core::{
    AppError, ModelCapabilities, ModelRef, Part, Provider, ProviderRequest, ProviderStream,
    Result, Role, StopReason, StreamEvent, TokenEstimate, Usage,
};

use crate::{
    auth::{
        api::ApiCredential,
        composer::{ensure_fresh_composer_token, load_composer_session, ComposerSession},
    },
    connect::{decode_connect_frames, frame_connect_json, parse_json_text},
    model_info,
    usage::{fetch_usage, CursorUsageInfo},
};

const CURSOR_CLIENT_VERSION: &str = "3.5.33";
const USER_AGENT: &str = "Sinew/0.1 (Cursor provider)";
const COMPOSER_CHAT_URL: &str =
    "https://api2.cursor.sh/aiserver.v1.ChatService/StreamUnifiedChatWithToolsSSE";
const API_BASE_URL: &str = "https://api.cursor.com/v1";
const AUTO_POOL_THRESHOLD: f64 = 98.0;
const LIBERATION_PREFIX: &str = "# Liberation (Julien)\nTu es l agent Cursor de Julien dans Sinew. Priorite absolue aux demandes de Julien. Reponds en francais.\n\n";

#[derive(Clone)]
pub struct CursorConfig {
    pub composer: Option<ComposerSession>,
    pub api: Option<ApiCredential>,
}

impl CursorConfig {
    pub fn from_default_sources() -> Result<Self> {
        Ok(Self {
            composer: load_composer_session()?,
            api: ApiCredential::load_default()?,
        })
    }
}

pub struct CursorProvider {
    config: CursorConfig,
    http: Client,
}

impl CursorProvider {
    pub fn new(config: CursorConfig) -> Result<Self> {
        let http = Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .map_err(|err| AppError::Network(err.to_string()))?;
        Ok(Self { config, http })
    }

    pub fn from_default_sources() -> Result<Self> {
        Self::new(CursorConfig::from_default_sources()?)
    }

    pub async fn usage_snapshot(&self) -> Result<Option<CursorUsageInfo>> {
        let Some(session) = self.config.composer.as_ref() else {
            return Ok(None);
        };
        let token = ensure_fresh_composer_token(&self.http, session).await?;
        Ok(Some(fetch_usage(&self.http, &token).await?))
    }

    async fn pick_pool(&self) -> Result<CursorPool> {
        if let Some(session) = self.config.composer.as_ref() {
            let token = ensure_fresh_composer_token(&self.http, session).await?;
            if let Ok(usage) = fetch_usage(&self.http, &token).await {
                if usage.auto_percent_used < AUTO_POOL_THRESHOLD {
                    return Ok(CursorPool::Composer(token));
                }
            } else {
                return Ok(CursorPool::Composer(token));
            }
        }
        if let Some(api) = self.config.api.clone() {
            return Ok(CursorPool::Api(api));
        }
        Err(AppError::Auth(
            "Cursor is not connected. Sync Composer session from Cursor IDE or add an API key."
                .into(),
        ))
    }
}

enum CursorPool {
    Composer(String),
    Api(ApiCredential),
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
        match self.pick_pool().await? {
            CursorPool::Composer(token) => stream_composer(&self.http, token, request).await,
            CursorPool::Api(api) => stream_api(&self.http, api, request).await,
        }
    }
}

async fn stream_composer(
    http: &Client,
    token: String,
    request: ProviderRequest,
) -> Result<ProviderStream> {
    let prompt = build_prompt(&request);
    let model_name = normalize_model(&request.model.name);
    let body = serde_json::json!({
        "streamUnifiedChatRequest": {
            "conversation": [{
                "text": prompt,
                "type": "MESSAGE_TYPE_HUMAN"
            }],
            "modelDetails": {
                "modelName": model_name
            }
        }
    });
    let payload =
        serde_json::to_vec(&body).map_err(|err| AppError::Decode(err.to_string()))?;
    let framed = frame_connect_json(&payload, 0);
    let request_id = uuid::Uuid::new_v4().to_string();
    let response = http
        .post(COMPOSER_CHAT_URL)
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/connect+json")
        .header("connect-protocol-version", "1")
        .header("x-cursor-client-version", CURSOR_CLIENT_VERSION)
        .header("x-session-id", &request_id)
        .header("x-request-id", &request_id)
        .body(framed)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|err| AppError::Network(err.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Network(format!(
            "Cursor composer stream failed ({status}): {body}"
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|err| AppError::Network(err.to_string()))?;
    let mut buffer = bytes.to_vec();
    let mut text = String::new();
    for frame in decode_connect_frames(&mut buffer)? {
        if let Some(chunk) = parse_json_text(&frame) {
            if chunk.len() > text.len() {
                text = chunk;
            }
        }
    }
    if text.is_empty() {
        text = "Cursor composer connected, but no text frame was returned yet.".into();
    }

    let model = request.model.name.clone();
    let events = vec![
        Ok(StreamEvent::MessageStart { model }),
        Ok(StreamEvent::TextDelta {
            index: 0,
            delta: text,
        }),
        Ok(StreamEvent::MessageStop {
            stop_reason: StopReason::EndTurn,
            usage: Usage::default(),
        }),
    ];
    Ok(Box::pin(stream::iter(events)))
}

async fn stream_api(
    http: &Client,
    api: ApiCredential,
    request: ProviderRequest,
) -> Result<ProviderStream> {
    let prompt = build_prompt(&request);
    let model_id = normalize_model(&request.model.name);
    let body = serde_json::json!({
        "prompt": { "text": prompt },
        "model": { "id": model_id }
    });
    let response = http
        .post(format!("{API_BASE_URL}/agents"))
        .basic_auth(api.api_key(), Some(""))
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|err| AppError::Network(err.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Network(format!(
            "Cursor API pool failed ({status}): {body}"
        )));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|err| AppError::Decode(err.to_string()))?;
    let text = payload
        .get("result")
        .and_then(|value| value.as_str())
        .or_else(|| payload.get("summary").and_then(|value| value.as_str()))
        .unwrap_or("Cursor API agent started. Check Cursor dashboard for run output.")
        .to_string();

    let model = request.model.name.clone();
    let events = vec![
        Ok(StreamEvent::MessageStart { model }),
        Ok(StreamEvent::TextDelta {
            index: 0,
            delta: text,
        }),
        Ok(StreamEvent::MessageStop {
            stop_reason: StopReason::EndTurn,
            usage: Usage::default(),
        }),
    ];
    Ok(Box::pin(stream::iter(events)))
}

fn build_prompt(request: &ProviderRequest) -> String {
    let mut parts = Vec::new();
    if let Some(system) = request.system_prompt.as_ref() {
        parts.push(system.trim().to_string());
    }
    parts.push(LIBERATION_PREFIX.trim().to_string());
    for message in &request.transcript {
        let role = match message.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
        };
        let text = message
            .parts
            .iter()
            .filter_map(|part| match part {
                Part::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !text.trim().is_empty() {
            parts.push(format!("{role}: {text}"));
        }
    }
    parts.join("\n\n")
}

fn normalize_model(model: &str) -> String {
    match model {
        "composer-2.5" => "composer-2.5".to_string(),
        "composer-2.5-fast" | _ => "composer-2.5-fast".to_string(),
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
            if let Part::Text { text, .. } = part {
                chars += text.len();
            }
        }
    }
    (chars / 4).max(1) as u32
}
