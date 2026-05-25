use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use directories::ProjectDirs;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use claakecode_core::{AppError, Result};

const ANTHROPIC_OAUTH_AUTHORIZE_URL: &str = "https://claude.ai/oauth/authorize";
const ANTHROPIC_OAUTH_TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";
const ANTHROPIC_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const ANTHROPIC_OAUTH_SCOPE: &str =
    "org:create_api_key user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload";
const REFRESH_SKEW_MS: i64 = 5 * 60_000;
pub(crate) const ANTHROPIC_RECONNECT_MESSAGE: &str =
    "Anthropic login expired. Please reconnect Anthropic in Settings > Providers.";

#[derive(Clone)]
pub enum Credential {
    OAuth(Arc<Mutex<OAuthToken>>),
}

pub struct OAuthToken {
    access: String,
    refresh: String,
    expires_at_ms: i64,
    source_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnthropicAuthStatus {
    pub connected: bool,
    pub expires_at_ms: Option<i64>,
    pub last_refresh_ms: Option<i64>,
}

impl AnthropicAuthStatus {
    pub fn disconnected() -> Self {
        Self {
            connected: false,
            expires_at_ms: None,
            last_refresh_ms: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PkceCodes {
    pub code_verifier: String,
    pub code_challenge: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredAuth {
    provider: String,
    auth_mode: String,
    tokens: StoredTokens,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_refresh_ms: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredTokens {
    access_token: String,
    refresh_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at_ms: Option<i64>,
}

impl Credential {
    pub fn from_oauth_parts(
        access: impl Into<String>,
        refresh: impl Into<String>,
        expires_at_ms: i64,
        source_path: Option<PathBuf>,
    ) -> Self {
        Self::OAuth(Arc::new(Mutex::new(OAuthToken {
            access: access.into(),
            refresh: refresh.into(),
            expires_at_ms,
            source_path,
        })))
    }

    pub fn load_default() -> Result<Option<Self>> {
        Self::from_claakecode_auth_file(&default_auth_path()?)
    }

    pub fn from_claakecode_auth_file(path: &Path) -> Result<Option<Self>> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(AppError::Auth(format!("unable to read auth file: {err}"))),
        };

        let payload: StoredAuth = serde_json::from_slice(&bytes)
            .map_err(|err| AppError::Auth(format!("invalid auth file: {err}")))?;

        if payload.provider != "anthropic" || payload.auth_mode != "oauth" {
            return Ok(None);
        }

        if payload.tokens.access_token.is_empty() {
            return Err(AppError::Auth(
                "anthropic oauth is missing access token".into(),
            ));
        }

        Ok(Some(Self::from_oauth_parts(
            payload.tokens.access_token,
            payload.tokens.refresh_token,
            payload.tokens.expires_at_ms.unwrap_or(0),
            Some(path.to_path_buf()),
        )))
    }

    pub fn is_oauth(&self) -> bool {
        true
    }

    pub async fn bearer_or_key(&self, http: &reqwest::Client) -> Result<String> {
        self.oauth_token(http, None).await
    }

    pub async fn force_refresh(
        &self,
        http: &reqwest::Client,
        previous_access: &str,
    ) -> Result<String> {
        self.oauth_token(http, Some(previous_access)).await
    }

    async fn oauth_token(
        &self,
        http: &reqwest::Client,
        refresh_access: Option<&str>,
    ) -> Result<String> {
        match self {
            Self::OAuth(state) => {
                let mut guard = state.lock().await;
                if let Some(previous_access) = refresh_access {
                    if guard.access != previous_access {
                        return Ok(guard.access.clone());
                    }
                } else if !is_expired(guard.expires_at_ms) {
                    return Ok(guard.access.clone());
                }

                if guard.refresh.is_empty() {
                    return Err(AppError::Auth(ANTHROPIC_RECONNECT_MESSAGE.into()));
                }

                let fresh = refresh_token(http, &guard.refresh).await?;
                let expires_at_ms = expires_at(fresh.expires_in);

                guard.access = fresh.access_token.clone();
                guard.refresh = fresh.refresh_token.clone();
                guard.expires_at_ms = expires_at_ms;
                let source_path = guard.source_path.clone();
                drop(guard);

                if let Some(path) = source_path {
                    if let Err(err) = persist_refresh(
                        &path,
                        &fresh.access_token,
                        &fresh.refresh_token,
                        expires_at_ms,
                    ) {
                        tracing::warn!(error = %err, "failed to persist refreshed anthropic oauth token");
                    }
                }

                Ok(fresh.access_token)
            }
        }
    }
}

fn is_expired(expires_at_ms: i64) -> bool {
    now_ms() + REFRESH_SKEW_MS >= expires_at_ms
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn expires_at(expires_in_seconds: u64) -> i64 {
    now_ms() + (expires_in_seconds as i64 * 1000) - REFRESH_SKEW_MS
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

async fn refresh_token(http: &reqwest::Client, refresh_token: &str) -> Result<TokenResponse> {
    let response = http
        .post(ANTHROPIC_OAUTH_TOKEN_URL)
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .json(&serde_json::json!({
            "grant_type": "refresh_token",
            "client_id": ANTHROPIC_CLIENT_ID,
            "refresh_token": refresh_token,
        }))
        .send()
        .await
        .map_err(|err| AppError::Network(format!("anthropic oauth refresh failed: {err}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Auth(format!(
            "anthropic oauth refresh failed with {status}: {body}"
        )));
    }

    response
        .json()
        .await
        .map_err(|err| AppError::Decode(format!("invalid anthropic oauth refresh body: {err}")))
}

fn persist_refresh(path: &Path, access: &str, refresh: &str, expires_at_ms: i64) -> Result<()> {
    let bytes = std::fs::read(path)
        .map_err(|err| AppError::Auth(format!("unable to re-read auth file: {err}")))?;
    let mut root: StoredAuth = serde_json::from_slice(&bytes)
        .map_err(|err| AppError::Auth(format!("unable to parse auth file: {err}")))?;

    root.tokens.access_token = access.to_string();
    root.tokens.refresh_token = refresh.to_string();
    root.tokens.expires_at_ms = Some(expires_at_ms);
    root.last_refresh_ms = Some(now_ms());

    write_auth_file(path, &root)
}

pub fn default_auth_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("dev", "williampeynichou", "claakecode")
        .ok_or_else(|| AppError::Auth("unable to resolve local data directory".into()))?;
    Ok(dirs.data_local_dir().join("anthropic-auth.json"))
}

pub fn load_default_auth_status() -> Result<AnthropicAuthStatus> {
    load_auth_status(&default_auth_path()?)
}

pub fn load_auth_status(path: &Path) -> Result<AnthropicAuthStatus> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AnthropicAuthStatus::disconnected());
        }
        Err(err) => return Err(AppError::Auth(format!("unable to read auth file: {err}"))),
    };
    let payload: StoredAuth = serde_json::from_slice(&bytes)
        .map_err(|err| AppError::Auth(format!("invalid auth file: {err}")))?;
    Ok(status_from_auth(&payload))
}

pub fn delete_default_auth() -> Result<()> {
    let path = default_auth_path()?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(AppError::Auth(format!("unable to delete auth file: {err}"))),
    }
}

pub fn generate_state() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn generate_pkce() -> PkceCodes {
    let code_verifier = generate_state();
    let digest = Sha256::digest(code_verifier.as_bytes());
    let code_challenge = URL_SAFE_NO_PAD.encode(digest);
    PkceCodes {
        code_verifier,
        code_challenge,
    }
}

pub fn oauth_authorize_url(redirect_uri: &str, pkce: &PkceCodes, state: &str) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer
        .append_pair("code", "true")
        .append_pair("client_id", ANTHROPIC_CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", ANTHROPIC_OAUTH_SCOPE)
        .append_pair("code_challenge", &pkce.code_challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state);
    format!("{ANTHROPIC_OAUTH_AUTHORIZE_URL}?{}", serializer.finish())
}

pub async fn exchange_oauth_code(
    http: &reqwest::Client,
    code: &str,
    state: &str,
    redirect_uri: &str,
    pkce: &PkceCodes,
) -> Result<AnthropicAuthStatus> {
    let response = http
        .post(ANTHROPIC_OAUTH_TOKEN_URL)
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .json(&serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": ANTHROPIC_CLIENT_ID,
            "code": code,
            "state": state,
            "redirect_uri": redirect_uri,
            "code_verifier": &pkce.code_verifier,
        }))
        .send()
        .await
        .map_err(|err| AppError::Network(format!("anthropic oauth exchange failed: {err}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Auth(format!(
            "anthropic oauth exchange failed with {status}: {body}"
        )));
    }

    let body: TokenResponse = response
        .json()
        .await
        .map_err(|err| AppError::Decode(format!("invalid anthropic oauth body: {err}")))?;
    save_oauth_tokens(
        &default_auth_path()?,
        &body.access_token,
        &body.refresh_token,
        body.expires_in,
    )
}

fn save_oauth_tokens(
    path: &Path,
    access_token: &str,
    refresh_token: &str,
    expires_in: u64,
) -> Result<AnthropicAuthStatus> {
    let auth = StoredAuth {
        provider: "anthropic".into(),
        auth_mode: "oauth".into(),
        tokens: StoredTokens {
            access_token: access_token.into(),
            refresh_token: refresh_token.into(),
            expires_at_ms: Some(expires_at(expires_in)),
        },
        last_refresh_ms: Some(now_ms()),
    };
    write_auth_file(path, &auth)?;
    Ok(status_from_auth(&auth))
}

fn write_auth_file(path: &Path, auth: &StoredAuth) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| AppError::Auth(format!("unable to create auth directory: {err}")))?;
    }
    let pretty = serde_json::to_vec_pretty(auth)
        .map_err(|err| AppError::Decode(format!("unable to serialize auth file: {err}")))?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, pretty)
        .map_err(|err| AppError::Auth(format!("unable to write temp auth file: {err}")))?;
    apply_permissions(&temp)?;
    std::fs::rename(&temp, path)
        .map_err(|err| AppError::Auth(format!("unable to replace auth file: {err}")))?;
    Ok(())
}

fn status_from_auth(auth: &StoredAuth) -> AnthropicAuthStatus {
    if auth.provider != "anthropic" || auth.auth_mode != "oauth" {
        return AnthropicAuthStatus::disconnected();
    }
    AnthropicAuthStatus {
        connected: !auth.tokens.access_token.is_empty(),
        expires_at_ms: auth.tokens.expires_at_ms,
        last_refresh_ms: auth.last_refresh_ms,
    }
}

#[cfg(unix)]
fn apply_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|err| AppError::Auth(format!("unable to chmod auth file: {err}")))?;
    Ok(())
}

#[cfg(not(unix))]
fn apply_permissions(_path: &Path) -> Result<()> {
    Ok(())
}
