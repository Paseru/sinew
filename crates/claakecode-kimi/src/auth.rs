use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use directories::ProjectDirs;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use claakecode_core::{AppError, Result};

const KIMI_OAUTH_HOST: &str = "https://auth.kimi.com";
const KIMI_CLIENT_ID: &str = "17e5f671-d194-4dfb-9706-5516cb48c098";
const REFRESH_SKEW_MS: i64 = 5 * 60_000;
const POLL_TIMEOUT_SECONDS: u64 = 10 * 60;
pub(crate) const KIMI_RECONNECT_MESSAGE: &str =
    "Kimi login expired. Please reconnect Kimi in Settings > Providers.";

#[derive(Clone)]
pub enum Credential {
    OAuth(Arc<Mutex<OAuthToken>>),
}

pub struct OAuthToken {
    access: String,
    refresh: String,
    expires_at_ms: i64,
    scope: Option<String>,
    token_type: Option<String>,
    source_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KimiAuthStatus {
    pub connected: bool,
    pub expires_at_ms: Option<i64>,
    pub last_refresh_ms: Option<i64>,
}

impl KimiAuthStatus {
    pub fn disconnected() -> Self {
        Self {
            connected: false,
            expires_at_ms: None,
            last_refresh_ms: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeviceAuthorization {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: Option<u64>,
    pub interval: u64,
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
    scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    token_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at_ms: Option<i64>,
}

impl Credential {
    pub fn from_oauth_parts(
        access: impl Into<String>,
        refresh: impl Into<String>,
        expires_at_ms: i64,
        scope: Option<String>,
        token_type: Option<String>,
        source_path: Option<PathBuf>,
    ) -> Self {
        Self::OAuth(Arc::new(Mutex::new(OAuthToken {
            access: access.into(),
            refresh: refresh.into(),
            expires_at_ms,
            scope,
            token_type,
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

        if payload.provider != "kimi" || payload.auth_mode != "oauth" {
            return Ok(None);
        }

        if payload.tokens.access_token.is_empty() {
            return Err(AppError::Auth("kimi oauth is missing access token".into()));
        }

        Ok(Some(Self::from_oauth_parts(
            payload.tokens.access_token,
            payload.tokens.refresh_token,
            payload.tokens.expires_at_ms.unwrap_or(0),
            payload.tokens.scope,
            payload.tokens.token_type,
            Some(path.to_path_buf()),
        )))
    }

    pub async fn bearer(&self, http: &reqwest::Client) -> Result<String> {
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
                    return Err(AppError::Auth(KIMI_RECONNECT_MESSAGE.into()));
                }

                let fresh = refresh_token(http, &guard.refresh).await?;
                let expires_at_ms = expires_at(fresh.expires_in.unwrap_or(3600));
                guard.access = fresh.access_token.clone();
                guard.refresh = fresh.refresh_token.clone();
                guard.expires_at_ms = expires_at_ms;
                guard.scope = fresh.scope.clone();
                guard.token_type = fresh.token_type.clone();
                let source_path = guard.source_path.clone();
                drop(guard);

                if let Some(path) = source_path {
                    if let Err(err) = persist_refresh(&path, &fresh, expires_at_ms) {
                        tracing::warn!(error = %err, "failed to persist refreshed kimi oauth token");
                    }
                }

                Ok(fresh.access_token)
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct DeviceAuthorizationBody {
    user_code: String,
    device_code: String,
    #[serde(default)]
    verification_uri: Option<String>,
    verification_uri_complete: String,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    interval: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct TokenBody {
    access_token: String,
    refresh_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PendingTokenBody {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

pub async fn request_device_authorization(http: &reqwest::Client) -> Result<DeviceAuthorization> {
    let mut request = http
        .post(format!(
            "{}/api/oauth/device_authorization",
            oauth_host().trim_end_matches('/')
        ))
        .form(&[("client_id", KIMI_CLIENT_ID)]);
    for (key, value) in common_headers()? {
        request = request.header(key, value);
    }

    let response = request
        .send()
        .await
        .map_err(|err| AppError::Network(format!("kimi device authorization failed: {err}")))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Auth(format!(
            "kimi device authorization failed with {status}: {body}"
        )));
    }

    let body: DeviceAuthorizationBody = response
        .json()
        .await
        .map_err(|err| AppError::Decode(format!("invalid kimi device auth body: {err}")))?;
    Ok(DeviceAuthorization {
        user_code: body.user_code,
        device_code: body.device_code,
        verification_uri: body.verification_uri.unwrap_or_default(),
        verification_uri_complete: body.verification_uri_complete,
        expires_in: body.expires_in,
        interval: body.interval.unwrap_or(5).max(1),
    })
}

pub async fn wait_for_device_token(
    http: &reqwest::Client,
    auth: &DeviceAuthorization,
) -> Result<KimiAuthStatus> {
    let expires_after = auth
        .expires_in
        .unwrap_or(POLL_TIMEOUT_SECONDS)
        .min(POLL_TIMEOUT_SECONDS);
    let deadline = std::time::Instant::now() + Duration::from_secs(expires_after);

    loop {
        let response = request_device_token(http, auth).await?;
        if response.status == reqwest::StatusCode::OK {
            let token: TokenBody = serde_json::from_value(response.body)
                .map_err(|err| AppError::Decode(format!("invalid kimi oauth token body: {err}")))?;
            return save_oauth_tokens(&default_auth_path()?, token);
        }

        let pending: PendingTokenBody =
            serde_json::from_value(response.body).unwrap_or(PendingTokenBody {
                error: None,
                error_description: None,
            });
        let error = pending
            .error
            .unwrap_or_else(|| "authorization_pending".into());
        if error == "expired_token" || std::time::Instant::now() >= deadline {
            return Err(AppError::Auth("kimi device code expired".into()));
        }
        if !matches!(
            error.as_str(),
            "authorization_pending" | "slow_down" | "access_denied"
        ) {
            let message = pending
                .error_description
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(error);
            return Err(AppError::Auth(message));
        }

        tokio::time::sleep(Duration::from_secs(auth.interval)).await;
    }
}

struct DeviceTokenResponse {
    status: reqwest::StatusCode,
    body: serde_json::Value,
}

async fn request_device_token(
    http: &reqwest::Client,
    auth: &DeviceAuthorization,
) -> Result<DeviceTokenResponse> {
    let mut request = http
        .post(format!(
            "{}/api/oauth/token",
            oauth_host().trim_end_matches('/')
        ))
        .form(&[
            ("client_id", KIMI_CLIENT_ID),
            ("device_code", auth.device_code.as_str()),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ]);
    for (key, value) in common_headers()? {
        request = request.header(key, value);
    }

    let response = request
        .send()
        .await
        .map_err(|err| AppError::Network(format!("kimi token polling failed: {err}")))?;
    let status = response.status();
    let body = response
        .json::<serde_json::Value>()
        .await
        .map_err(|err| AppError::Decode(format!("invalid kimi token polling body: {err}")))?;
    if status.is_server_error() {
        return Err(AppError::Provider(format!(
            "kimi token polling server error: {status}"
        )));
    }
    Ok(DeviceTokenResponse { status, body })
}

async fn refresh_token(http: &reqwest::Client, refresh_token: &str) -> Result<TokenBody> {
    let mut request = http
        .post(format!(
            "{}/api/oauth/token",
            oauth_host().trim_end_matches('/')
        ))
        .form(&[
            ("client_id", KIMI_CLIENT_ID),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ]);
    for (key, value) in common_headers()? {
        request = request.header(key, value);
    }

    let response = request
        .send()
        .await
        .map_err(|err| AppError::Network(format!("kimi oauth refresh failed: {err}")))?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED
        || response.status() == reqwest::StatusCode::FORBIDDEN
    {
        return Err(AppError::Auth(KIMI_RECONNECT_MESSAGE.into()));
    }
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Auth(format!(
            "kimi oauth refresh failed with {status}: {body}"
        )));
    }

    response
        .json()
        .await
        .map_err(|err| AppError::Decode(format!("invalid kimi oauth refresh body: {err}")))
}

fn save_oauth_tokens(path: &Path, token: TokenBody) -> Result<KimiAuthStatus> {
    let expires_at_ms = expires_at(token.expires_in.unwrap_or(3600));
    let auth = StoredAuth {
        provider: "kimi".into(),
        auth_mode: "oauth".into(),
        tokens: StoredTokens {
            access_token: token.access_token,
            refresh_token: token.refresh_token,
            scope: token.scope,
            token_type: token.token_type,
            expires_at_ms: Some(expires_at_ms),
        },
        last_refresh_ms: Some(now_ms()),
    };
    write_auth_file(path, &auth)?;
    Ok(status_from_auth(&auth))
}

fn persist_refresh(path: &Path, fresh: &TokenBody, expires_at_ms: i64) -> Result<()> {
    let bytes = std::fs::read(path)
        .map_err(|err| AppError::Auth(format!("unable to re-read auth file: {err}")))?;
    let mut root: StoredAuth = serde_json::from_slice(&bytes)
        .map_err(|err| AppError::Auth(format!("unable to parse auth file: {err}")))?;

    root.tokens.access_token = fresh.access_token.clone();
    root.tokens.refresh_token = fresh.refresh_token.clone();
    root.tokens.scope = fresh.scope.clone();
    root.tokens.token_type = fresh.token_type.clone();
    root.tokens.expires_at_ms = Some(expires_at_ms);
    root.last_refresh_ms = Some(now_ms());

    write_auth_file(path, &root)
}

pub fn default_auth_path() -> Result<PathBuf> {
    Ok(data_local_dir()?.join("kimi-auth.json"))
}

fn default_device_id_path() -> Result<PathBuf> {
    Ok(data_local_dir()?.join("kimi-device-id"))
}

fn data_local_dir() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("dev", "williampeynichou", "claakecode")
        .ok_or_else(|| AppError::Auth("unable to resolve local data directory".into()))?;
    Ok(dirs.data_local_dir().to_path_buf())
}

pub fn load_default_auth_status() -> Result<KimiAuthStatus> {
    load_auth_status(&default_auth_path()?)
}

pub fn load_auth_status(path: &Path) -> Result<KimiAuthStatus> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(KimiAuthStatus::disconnected());
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

fn status_from_auth(auth: &StoredAuth) -> KimiAuthStatus {
    if auth.provider != "kimi" || auth.auth_mode != "oauth" {
        return KimiAuthStatus::disconnected();
    }
    KimiAuthStatus {
        connected: !auth.tokens.access_token.is_empty(),
        expires_at_ms: auth.tokens.expires_at_ms,
        last_refresh_ms: auth.last_refresh_ms,
    }
}

fn expires_at(expires_in_seconds: u64) -> i64 {
    now_ms() + (expires_in_seconds as i64 * 1000) - REFRESH_SKEW_MS
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

fn oauth_host() -> String {
    std::env::var("KIMI_CODE_OAUTH_HOST")
        .or_else(|_| std::env::var("KIMI_OAUTH_HOST"))
        .unwrap_or_else(|_| KIMI_OAUTH_HOST.into())
}

pub(crate) fn common_headers() -> Result<Vec<(&'static str, String)>> {
    Ok(vec![
        ("X-Msh-Platform", "kimi_cli".into()),
        ("X-Msh-Version", env!("CARGO_PKG_VERSION").into()),
        ("X-Msh-Device-Name", ascii_header_value(&device_name())),
        ("X-Msh-Device-Model", ascii_header_value(&device_model())),
        ("X-Msh-Os-Version", ascii_header_value(std::env::consts::OS)),
        ("X-Msh-Device-Id", device_id()?),
    ])
}

fn device_name() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "claakecode".into())
}

fn device_model() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

fn device_id() -> Result<String> {
    let path = default_device_id_path()?;
    match std::fs::read_to_string(&path) {
        Ok(value) => {
            let value = value.trim().to_string();
            if !value.is_empty() {
                return Ok(value);
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(AppError::Auth(format!(
                "unable to read kimi device id: {err}"
            )))
        }
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| AppError::Auth(format!("unable to create auth directory: {err}")))?;
    }
    let value = generate_state();
    std::fs::write(&path, &value)
        .map_err(|err| AppError::Auth(format!("unable to write kimi device id: {err}")))?;
    apply_permissions(&path)?;
    Ok(value)
}

fn ascii_header_value(input: &str) -> String {
    let value = input
        .chars()
        .filter(|ch| ch.is_ascii() && !ch.is_control())
        .collect::<String>()
        .trim()
        .to_string();
    if value.is_empty() {
        "unknown".into()
    } else {
        value
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
