use std::time::Duration;

use base64::Engine as _;
use rand::RngCore;
use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::sync::Notify;

use sinew_core::{AppError, Result};

use crate::identity::CursorIdeIdentity;

use super::composer::{save_oauth_tokens, CursorComposerAuthStatus};

const CURSOR_WEBSITE_URL: &str = "https://cursor.com";
const CURSOR_BACKEND_URL: &str = "https://api2.cursor.sh";
const CURSOR_LOGIN_PATH: &str = "/loginDeepControl";
const POLL_INTERVAL_MS: u64 = 500;
const POLL_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone)]
pub struct CursorLoginChallenge {
    pub auth_url: String,
    pub uuid: String,
    pub verifier: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PollResponse {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StripeProfileResponse {
    #[serde(default)]
    membership_type: Option<String>,
    #[serde(default)]
    individual_membership_type: Option<String>,
    #[serde(default)]
    subscription_status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmailResponse {
    #[serde(default)]
    email: Option<String>,
}

pub fn create_login_challenge() -> CursorLoginChallenge {
    let verifier = generate_verifier();
    let challenge = generate_challenge(&verifier);
    let uuid = uuid::Uuid::new_v4().to_string();
    let auth_url = format!(
        "{CURSOR_WEBSITE_URL}{CURSOR_LOGIN_PATH}?challenge={challenge}&uuid={uuid}&mode=login"
    );
    CursorLoginChallenge {
        auth_url,
        uuid,
        verifier,
    }
}

pub async fn wait_for_oauth_login(
    http: &Client,
    challenge: &CursorLoginChallenge,
    cancel: &Notify,
) -> Result<CursorComposerAuthStatus> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(POLL_TIMEOUT_SECS);

    loop {
        tokio::select! {
            _ = cancel.notified() => {
                return Err(AppError::Auth("Cursor login canceled".into()));
            }
            _ = tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)) => {}
        }

        if tokio::time::Instant::now() >= deadline {
            return Err(AppError::Auth(
                "Cursor login timed out. Complete sign-in in the browser.".into(),
            ));
        }

        let Some((access_token, refresh_token)) = poll_once(http, challenge).await? else {
            continue;
        };

        let profile = fetch_profile(http, &access_token).await.unwrap_or_default();
        return save_oauth_tokens(
            access_token,
            refresh_token,
            profile.email,
            profile.membership_type,
            profile.subscription_status,
        );
    }
}

async fn poll_once(
    http: &Client,
    challenge: &CursorLoginChallenge,
) -> Result<Option<(String, String)>> {
    let identity = CursorIdeIdentity::load();
    let session_id = uuid::Uuid::new_v4().to_string();
    let request_id = uuid::Uuid::new_v4().to_string();
    let mut headers = reqwest::header::HeaderMap::new();
    identity.apply(&mut headers, &session_id, &request_id);

    let url = format!(
        "{CURSOR_BACKEND_URL}/auth/poll?uuid={}&verifier={}",
        urlencoding(challenge.uuid.as_str()),
        urlencoding(challenge.verifier.as_str()),
    );
    let response = http
        .get(url)
        .headers(headers)
        .send()
        .await
        .map_err(|err| AppError::Network(format!("cursor auth poll failed: {err}")))?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Auth(format!(
            "cursor auth poll failed with {status}: {body}"
        )));
    }

    let body: PollResponse = response
        .json()
        .await
        .map_err(|err| AppError::Decode(format!("invalid cursor auth poll body: {err}")))?;

    match (body.access_token, body.refresh_token) {
        (Some(access_token), Some(refresh_token))
            if !access_token.trim().is_empty() && !refresh_token.trim().is_empty() =>
        {
            Ok(Some((access_token, refresh_token)))
        }
        _ => Ok(None),
    }
}

#[derive(Default)]
struct OAuthProfile {
    email: Option<String>,
    membership_type: Option<String>,
    subscription_status: Option<String>,
}

async fn fetch_profile(http: &Client, access_token: &str) -> Result<OAuthProfile> {
    let mut profile = OAuthProfile::default();

    if let Ok(email) = fetch_email(http, access_token).await {
        profile.email = email;
    }

    if let Ok(stripe) = fetch_stripe_profile(http, access_token).await {
        profile.membership_type = stripe
            .individual_membership_type
            .or(stripe.membership_type);
        profile.subscription_status = stripe.subscription_status;
    }

    Ok(profile)
}

async fn fetch_email(http: &Client, access_token: &str) -> Result<Option<String>> {
    let identity = CursorIdeIdentity::load();
    let session_id = uuid::Uuid::new_v4().to_string();
    let request_id = uuid::Uuid::new_v4().to_string();
    let mut headers = reqwest::header::HeaderMap::new();
    identity.apply_authenticated(&mut headers, &session_id, &request_id, access_token);

    let response = http
        .post(format!(
            "{CURSOR_BACKEND_URL}/aiserver.v1.AuthService/GetEmail"
        ))
        .headers(headers)
        .header("authorization", format!("Bearer {access_token}"))
        .header("content-type", "application/json")
        .header("connect-protocol-version", "1")
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|err| AppError::Network(err.to_string()))?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let body: EmailResponse = response
        .json()
        .await
        .map_err(|err| AppError::Decode(err.to_string()))?;
    Ok(body.email.filter(|value| !value.trim().is_empty()))
}

async fn fetch_stripe_profile(http: &Client, access_token: &str) -> Result<StripeProfileResponse> {
    let identity = CursorIdeIdentity::load();
    let session_id = uuid::Uuid::new_v4().to_string();
    let request_id = uuid::Uuid::new_v4().to_string();
    let mut headers = reqwest::header::HeaderMap::new();
    identity.apply_authenticated(&mut headers, &session_id, &request_id, access_token);

    let response = http
        .get(format!("{CURSOR_BACKEND_URL}/auth/full_stripe_profile"))
        .headers(headers)
        .header("authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .map_err(|err| AppError::Network(err.to_string()))?;

    if !response.status().is_success() {
        return Err(AppError::Network(format!(
            "cursor stripe profile returned {}",
            response.status()
        )));
    }

    response
        .json()
        .await
        .map_err(|err| AppError::Decode(err.to_string()))
}

fn generate_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn generate_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

fn urlencoding(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}
