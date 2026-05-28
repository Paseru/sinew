use reqwest::header::HeaderMap;
use sinew_core::{AppError, Result};

use crate::identity::CursorIdeIdentity;

pub const API2_BASE: &str = "https://api2.cursor.sh";
pub const GET_USABLE_MODELS: &str = "/agent.v1.AgentService/GetUsableModels";

/// Fetch raw `GetUsableModels` protobuf response (OAuth standalone).
pub async fn fetch_usable_models(
    http: &reqwest::Client,
    identity: &CursorIdeIdentity,
    access_token: &str,
) -> Result<Vec<u8>> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let request_id = uuid::Uuid::new_v4().to_string();
    let mut headers = HeaderMap::new();
    identity.apply_agent_authenticated(&mut headers, &session_id, &request_id, access_token);

    let response = http
        .post(format!("{API2_BASE}{GET_USABLE_MODELS}"))
        .headers(headers)
        .body(Vec::new())
        .send()
        .await
        .map_err(|err| AppError::Network(err.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Network(format!(
            "GetUsableModels failed ({status}): {body}"
        )));
    }

    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|err| AppError::Network(err.to_string()))
}

/// Best-effort list of model id substrings from protobuf bytes.
pub fn scan_model_ids(payload: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(payload);
    let mut models = Vec::new();
    for token in text.split(|ch: char| !ch.is_ascii_graphic()) {
        if token.contains("composer") || token.starts_with("gpt-") || token.starts_with("claude-") {
            if token.len() >= 4 && token.len() <= 64 && !models.iter().any(|m| m == token) {
                models.push(token.to_string());
            }
        }
    }
    models
}
