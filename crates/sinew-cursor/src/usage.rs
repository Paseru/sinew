use reqwest::Client;
use serde::{Deserialize, Serialize};
use sinew_core::{AppError, Result};

const USAGE_URL: &str =
    "https://api2.cursor.sh/aiserver.v1.DashboardService/GetCurrentPeriodUsage";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorUsageInfo {
    pub auto_percent_used: f64,
    pub api_percent_used: f64,
    pub total_percent_used: f64,
    pub auto_limit: Option<u64>,
    pub api_limit: Option<u64>,
    pub membership_type: Option<String>,
}

pub async fn fetch_usage(
    http: &Client,
    identity: &crate::identity::CursorIdeIdentity,
    access_token: &str,
) -> Result<CursorUsageInfo> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let request_id = uuid::Uuid::new_v4().to_string();
    let mut headers = reqwest::header::HeaderMap::new();
    identity.apply(&mut headers, &session_id, &request_id);

    let response = http
        .post(USAGE_URL)
        .headers(headers)
        .header("authorization", format!("Bearer {access_token}"))
        .header("content-type", "application/json")
        .header("connect-protocol-version", "1")
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|err| AppError::Network(err.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Network(format!(
            "Cursor usage endpoint returned {status}: {body}"
        )));
    }

    let payload: UsageResponse = response
        .json()
        .await
        .map_err(|err| AppError::Decode(err.to_string()))?;

    Ok(CursorUsageInfo {
        auto_percent_used: payload
            .plan_usage
            .as_ref()
            .and_then(|usage| usage.auto_percent_used)
            .unwrap_or(0.0),
        api_percent_used: payload
            .plan_usage
            .as_ref()
            .and_then(|usage| usage.api_percent_used)
            .unwrap_or(0.0),
        total_percent_used: payload
            .plan_usage
            .as_ref()
            .and_then(|usage| usage.total_percent_used)
            .unwrap_or(0.0),
        auto_limit: payload
            .plan_usage
            .as_ref()
            .and_then(|usage| usage.auto_limit),
        api_limit: payload
            .plan_usage
            .as_ref()
            .and_then(|usage| usage.api_limit),
        membership_type: None,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageResponse {
    #[serde(default)]
    plan_usage: Option<PlanUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanUsage {
    #[serde(default)]
    auto_percent_used: Option<f64>,
    #[serde(default)]
    api_percent_used: Option<f64>,
    #[serde(default)]
    total_percent_used: Option<f64>,
    #[serde(default)]
    auto_limit: Option<u64>,
    #[serde(default)]
    api_limit: Option<u64>,
}
