use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use sinew_core::{AppError, Result};

use crate::identity::CursorIdeIdentity;

const CURSOR_AUTH_CLIENT_ID: &str = "KbZUR41cY7W6zRSdpSUJ7I7mLYBKOCmB";
const CURSOR_OAUTH_TOKEN_URL: &str = "https://api2.cursor.sh/oauth/token";
const REFRESH_SKEW_MS: i64 = 120_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorComposerAuthStatus {
    pub connected: bool,
    #[serde(default = "default_disconnected_state")]
    pub connection_state: String,
    pub email: Option<String>,
    pub membership_type: Option<String>,
    pub subscription_status: Option<String>,
    pub source: Option<String>,
    pub expires_at_ms: Option<i64>,
    pub last_sync_ms: Option<i64>,
    pub login_id: Option<String>,
    pub error: Option<String>,
}

fn default_disconnected_state() -> String {
    "disconnected".into()
}

impl CursorComposerAuthStatus {
    pub fn disconnected() -> Self {
        Self {
            connected: false,
            connection_state: default_disconnected_state(),
            email: None,
            membership_type: None,
            subscription_status: None,
            source: None,
            expires_at_ms: None,
            last_sync_ms: None,
            login_id: None,
            error: None,
        }
    }

    pub fn with_connection_state(
        mut self,
        connection_state: impl Into<String>,
        login_id: Option<String>,
        error: Option<String>,
    ) -> Self {
        self.connection_state = connection_state.into();
        self.login_id = login_id;
        self.error = error;
        self
    }
}

#[derive(Debug, Clone)]
pub struct ComposerSession {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub email: Option<String>,
    pub membership_type: Option<String>,
    pub subscription_status: Option<String>,
    pub expires_at_ms: Option<i64>,
    pub source_path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredAuth {
    provider: String,
    auth_mode: String,
    tokens: StoredTokens,
    profile: StoredProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_sync_ms: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredTokens {
    access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at_ms: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct StoredProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    membership_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    subscription_status: Option<String>,
}

pub fn default_composer_auth_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("dev", "hyrak", "sinew")
        .ok_or_else(|| AppError::Auth("unable to resolve local data directory".into()))?;
    Ok(dirs.data_local_dir().join("cursor-composer-auth.json"))
}

pub fn load_composer_auth_status() -> Result<CursorComposerAuthStatus> {
    load_composer_auth_status_from(&default_composer_auth_path()?)
}

pub fn load_composer_auth_status_from(path: &Path) -> Result<CursorComposerAuthStatus> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CursorComposerAuthStatus::disconnected())
        }
        Err(err) => return Err(AppError::Auth(format!("unable to read auth file: {err}"))),
    };
    let auth: StoredAuth = serde_json::from_slice(&bytes)
        .map_err(|err| AppError::Auth(format!("invalid auth file: {err}")))?;
    Ok(status_from_auth(&auth))
}

pub fn save_oauth_tokens(
    access_token: String,
    refresh_token: String,
    email: Option<String>,
    membership_type: Option<String>,
    subscription_status: Option<String>,
) -> Result<CursorComposerAuthStatus> {
    let path = default_composer_auth_path()?;
    let mut auth = StoredAuth {
        provider: PROVIDER_ID.into(),
        auth_mode: "oauth".into(),
        tokens: StoredTokens {
            access_token,
            refresh_token: Some(refresh_token),
            expires_at_ms: None,
        },
        profile: StoredProfile {
            email,
            membership_type,
            subscription_status,
        },
        last_sync_ms: Some(now_ms()),
    };
    auth.tokens.expires_at_ms = jwt_exp_ms(&auth.tokens.access_token);
    write_auth_file(&path, &auth)?;
    Ok(status_from_auth(&auth))
}

pub fn sync_composer_auth_from_ide() -> Result<CursorComposerAuthStatus> {
    Err(AppError::Auth(
        "Direct IDE session sync is disabled. Connect Cursor from Sinew Settings using OAuth.".into(),
    ))
}

pub fn load_composer_session() -> Result<Option<ComposerSession>> {
    let path = default_composer_auth_path()?;
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(AppError::Auth(format!("unable to read auth file: {err}"))),
    };
    let auth: StoredAuth = serde_json::from_slice(&bytes)
        .map_err(|err| AppError::Auth(format!("invalid auth file: {err}")))?;
    if auth.provider != PROVIDER_ID || auth.tokens.access_token.trim().is_empty() {
        return Ok(None);
    }
    if auth.auth_mode != "oauth" {
        return Ok(None);
    }
    Ok(Some(ComposerSession {
        access_token: auth.tokens.access_token,
        refresh_token: auth.tokens.refresh_token,
        email: auth.profile.email,
        membership_type: auth.profile.membership_type,
        subscription_status: auth.profile.subscription_status,
        expires_at_ms: auth.tokens.expires_at_ms,
        source_path: path,
    }))
}

pub async fn ensure_fresh_composer_token(
    http: &reqwest::Client,
    session: &ComposerSession,
) -> Result<String> {
    if !token_needs_refresh(session.expires_at_ms) {
        return Ok(session.access_token.clone());
    }
    let Some(refresh) = session.refresh_token.as_ref() else {
        return Ok(session.access_token.clone());
    };
    let body = serde_json::json!({
        "grant_type": "refresh_token",
        "client_id": CURSOR_AUTH_CLIENT_ID,
        "refresh_token": refresh,
    });
    let identity = CursorIdeIdentity::load();
    let session_id = uuid::Uuid::new_v4().to_string();
    let request_id = uuid::Uuid::new_v4().to_string();
    let mut headers = reqwest::header::HeaderMap::new();
    identity.apply(&mut headers, &session_id, &request_id);

    let response = http
        .post(CURSOR_OAUTH_TOKEN_URL)
        .headers(headers)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|err| AppError::Network(err.to_string()))?;
    if !response.status().is_success() {
        return Ok(session.access_token.clone());
    }
    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|err| AppError::Decode(err.to_string()))?;
    let access = payload
        .get("access_token")
        .and_then(|value| value.as_str())
        .unwrap_or(&session.access_token)
        .to_string();
    let refresh_token = payload
        .get("refresh_token")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .or_else(|| session.refresh_token.clone());
    let expires_at_ms = jwt_exp_ms(&access);
    let mut auth: StoredAuth = serde_json::from_slice(&std::fs::read(&session.source_path)?)
        .map_err(|err| AppError::Auth(format!("invalid auth file: {err}")))?;
    auth.tokens.access_token = access.clone();
    auth.tokens.refresh_token = refresh_token;
    auth.tokens.expires_at_ms = expires_at_ms;
    auth.last_sync_ms = Some(now_ms());
    write_auth_file(&session.source_path, &auth)?;
    Ok(access)
}

pub fn delete_composer_auth() -> Result<()> {
    let path = default_composer_auth_path()?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(AppError::Auth(format!("unable to delete auth file: {err}"))),
    }
}

const PROVIDER_ID: &str = "cursor";

fn status_from_auth(auth: &StoredAuth) -> CursorComposerAuthStatus {
    let mut status = CursorComposerAuthStatus {
        connected: !auth.tokens.access_token.trim().is_empty(),
        connection_state: if auth.tokens.access_token.trim().is_empty() {
            "disconnected".into()
        } else {
            "connected".into()
        },
        email: auth.profile.email.clone(),
        membership_type: auth.profile.membership_type.clone(),
        subscription_status: auth.profile.subscription_status.clone(),
        source: Some(auth.auth_mode.clone()),
        expires_at_ms: auth.tokens.expires_at_ms,
        last_sync_ms: auth.last_sync_ms,
        login_id: None,
        error: None,
    };
    if status.expires_at_ms.is_none() {
        status.expires_at_ms = jwt_exp_ms(&auth.tokens.access_token);
    }
    status
}

fn write_auth_file(path: &Path, auth: &StoredAuth) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| AppError::Auth(format!("unable to create auth directory: {err}")))?;
    }
    let pretty = serde_json::to_vec_pretty(auth)
        .map_err(|err| AppError::Decode(format!("unable to serialize auth file: {err}")))?;
    std::fs::write(path, pretty)
        .map_err(|err| AppError::Auth(format!("unable to write auth file: {err}")))?;
    Ok(())
}

fn token_needs_refresh(expires_at_ms: Option<i64>) -> bool {
    let Some(expires_at_ms) = expires_at_ms else {
        return false;
    };
    expires_at_ms - now_ms() <= REFRESH_SKEW_MS
}

fn jwt_exp_ms(token: &str) -> Option<i64> {
    let payload = token.split('.').nth(1)?;
    let padded = match payload.len() % 4 {
        0 => payload.to_string(),
        n => format!("{}{}", payload, "=".repeat(4 - n)),
    };
    let bytes = base64_decode(&padded).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    json.get("exp")
        .and_then(|value| value.as_i64())
        .map(|seconds| seconds * 1000)
}

fn base64_decode(input: &str) -> Result<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(input)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(input))
        .map_err(|err| AppError::Decode(err.to_string()))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
