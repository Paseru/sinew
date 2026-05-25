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

const GOOGLE_OAUTH_AUTHORIZE_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_OAUTH_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USER_INFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
// Antigravity OAuth client (replaces the Gemini Code Assist defaults).
// Source: https://github.com/NoeFabris/opencode-antigravity-auth
const GOOGLE_CLIENT_ID: &str =
    "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com";
const GOOGLE_CLIENT_SECRET: &str = "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";
const GOOGLE_OAUTH_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile https://www.googleapis.com/auth/cclog https://www.googleapis.com/auth/experimentsandconfigs";
const REFRESH_SKEW_MS: i64 = 60_000;

// Bump this number whenever we need to force a one-shot OAuth reset for
// existing installations (e.g. when the schema of the auth file or the
// upstream expectations change in a breaking way). The current value reflects
// the migration that fixes the Antigravity platform/project flow shipped in
// v0.1.14: legacy tokens were valid but the cached project/tier metadata was
// wrong, so we wipe the file and ask users to reconnect once.
const CURRENT_AUTH_MIGRATION: u32 = 1;
const AUTH_MIGRATION_FILE: &str = "google-auth-migrations.json";

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
pub struct GoogleAuthStatus {
    pub connected: bool,
    pub email: Option<String>,
    pub project_id: Option<String>,
    pub user_tier: Option<String>,
    pub expires_at_ms: Option<i64>,
    pub last_refresh_ms: Option<i64>,
}

impl GoogleAuthStatus {
    pub fn disconnected() -> Self {
        Self {
            connected: false,
            email: None,
            project_id: None,
            user_tier: None,
            expires_at_ms: None,
            last_refresh_ms: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleUserData {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_tier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_tier_name: Option<String>,
}

impl GoogleUserData {
    pub fn antigravity_default() -> Self {
        Self {
            project_id: "default".into(),
            user_tier: Some("free-tier".into()),
            user_tier_name: None,
        }
    }

    pub fn is_stale_antigravity_default(&self) -> bool {
        self.project_id == "default" && self.user_tier.as_deref() == Some("free-tier")
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
    user: Option<GoogleUserData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_refresh_ms: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredTokens {
    access_token: String,
    refresh_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    token_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    email: Option<String>,
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

        if payload.provider != "google" || payload.auth_mode != "oauth" {
            return Ok(None);
        }

        if payload.tokens.access_token.is_empty() {
            return Err(AppError::Auth(
                "google oauth is missing access token".into(),
            ));
        }

        Ok(Some(Self::from_oauth_parts(
            payload.tokens.access_token,
            payload.tokens.refresh_token,
            payload.tokens.expires_at_ms.unwrap_or(0),
            Some(path.to_path_buf()),
        )))
    }

    pub async fn bearer(&self, http: &reqwest::Client) -> Result<String> {
        match self {
            Self::OAuth(state) => {
                {
                    let guard = state.lock().await;
                    if !is_expired(guard.expires_at_ms) {
                        return Ok(guard.access.clone());
                    }
                    if guard.refresh.is_empty() {
                        return Err(AppError::Auth(
                            "google oauth token expired and cannot be refreshed".into(),
                        ));
                    }
                }

                let refresh = {
                    let guard = state.lock().await;
                    guard.refresh.clone()
                };
                let fresh = refresh_token(http, &refresh).await?;
                let access = fresh.access_token;
                let refresh = fresh.refresh_token.unwrap_or(refresh);
                let expires_at_ms = expires_at(fresh.expires_in.unwrap_or(3600));

                let source_path = {
                    let mut guard = state.lock().await;
                    guard.access = access.clone();
                    guard.refresh = refresh.clone();
                    guard.expires_at_ms = expires_at_ms;
                    guard.source_path.clone()
                };

                if let Some(path) = source_path {
                    if let Err(err) = persist_refresh(
                        &path,
                        fresh.id_token.as_deref(),
                        &access,
                        &refresh,
                        fresh.scope.as_deref(),
                        fresh.token_type.as_deref(),
                        expires_at_ms,
                    ) {
                        tracing::warn!(error = %err, "failed to persist refreshed google oauth token");
                    }
                }

                Ok(access)
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
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
}

async fn refresh_token(http: &reqwest::Client, refresh_token: &str) -> Result<TokenResponse> {
    let response = http
        .post(GOOGLE_OAUTH_TOKEN_URL)
        .header("content-type", "application/x-www-form-urlencoded")
        .header("accept", "application/json")
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", GOOGLE_CLIENT_ID),
            ("client_secret", GOOGLE_CLIENT_SECRET),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .map_err(|err| AppError::Network(format!("google oauth refresh failed: {err}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Auth(format!(
            "google oauth refresh failed with {status}: {body}"
        )));
    }

    response
        .json()
        .await
        .map_err(|err| AppError::Decode(format!("invalid google oauth refresh body: {err}")))
}

fn persist_refresh(
    path: &Path,
    id_token: Option<&str>,
    access: &str,
    refresh: &str,
    scope: Option<&str>,
    token_type: Option<&str>,
    expires_at_ms: i64,
) -> Result<()> {
    let bytes = std::fs::read(path)
        .map_err(|err| AppError::Auth(format!("unable to re-read auth file: {err}")))?;
    let mut root: StoredAuth = serde_json::from_slice(&bytes)
        .map_err(|err| AppError::Auth(format!("unable to parse auth file: {err}")))?;

    root.tokens.access_token = access.to_string();
    root.tokens.refresh_token = refresh.to_string();
    root.tokens.expires_at_ms = Some(expires_at_ms);
    if let Some(id_token) = id_token {
        root.tokens.id_token = Some(id_token.to_string());
        let previous_email = root.tokens.email.take();
        root.tokens.email = token_email(id_token).or(previous_email);
    }
    if let Some(scope) = scope {
        root.tokens.scope = Some(scope.to_string());
    }
    if let Some(token_type) = token_type {
        root.tokens.token_type = Some(token_type.to_string());
    }
    root.last_refresh_ms = Some(now_ms());

    write_auth_file(path, &root)
}

pub fn default_auth_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("dev", "williampeynichou", "claakecode")
        .ok_or_else(|| AppError::Auth("unable to resolve local data directory".into()))?;
    Ok(dirs.data_local_dir().join("google-auth.json"))
}

pub fn load_default_auth_status() -> Result<GoogleAuthStatus> {
    load_auth_status(&default_auth_path()?)
}

pub fn load_auth_status(path: &Path) -> Result<GoogleAuthStatus> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(GoogleAuthStatus::disconnected());
        }
        Err(err) => return Err(AppError::Auth(format!("unable to read auth file: {err}"))),
    };
    let payload: StoredAuth = serde_json::from_slice(&bytes)
        .map_err(|err| AppError::Auth(format!("invalid auth file: {err}")))?;
    Ok(status_from_auth(&payload))
}

pub fn load_default_user_data() -> Result<Option<GoogleUserData>> {
    let path = default_auth_path()?;
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(AppError::Auth(format!("unable to read auth file: {err}"))),
    };
    let payload: StoredAuth = serde_json::from_slice(&bytes)
        .map_err(|err| AppError::Auth(format!("invalid auth file: {err}")))?;
    if payload.provider != "google" || payload.auth_mode != "oauth" {
        return Ok(None);
    }
    Ok(payload
        .user
        .filter(|user| !user.is_stale_antigravity_default()))
}

pub fn save_default_user_data(user: &GoogleUserData) -> Result<()> {
    let path = default_auth_path()?;
    let bytes = std::fs::read(&path)
        .map_err(|err| AppError::Auth(format!("unable to read auth file: {err}")))?;
    let mut payload: StoredAuth = serde_json::from_slice(&bytes)
        .map_err(|err| AppError::Auth(format!("invalid auth file: {err}")))?;
    if payload.provider != "google" || payload.auth_mode != "oauth" {
        return Err(AppError::Auth(
            "google auth file has unexpected provider".into(),
        ));
    }
    payload.user = Some(user.clone());
    write_auth_file(&path, &payload)
}

pub fn delete_default_auth() -> Result<()> {
    let path = default_auth_path()?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(AppError::Auth(format!("unable to delete auth file: {err}"))),
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthMigrations {
    #[serde(default)]
    applied: u32,
}

fn default_migration_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("dev", "hyrak", "sinew")
        .ok_or_else(|| AppError::Auth("unable to resolve local data directory".into()))?;
    Ok(dirs.data_local_dir().join(AUTH_MIGRATION_FILE))
}

fn read_applied_migration(path: &Path) -> u32 {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice::<AuthMigrations>(&bytes)
            .map(|m| m.applied)
            .unwrap_or(0),
        Err(_) => 0,
    }
}

fn write_applied_migration(path: &Path, applied: u32) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            AppError::Auth(format!("unable to create auth migrations directory: {err}"))
        })?;
    }
    let payload = AuthMigrations { applied };
    let bytes = serde_json::to_vec_pretty(&payload).map_err(|err| {
        AppError::Decode(format!("unable to serialize auth migrations file: {err}"))
    })?;
    std::fs::write(path, bytes)
        .map_err(|err| AppError::Auth(format!("unable to write auth migrations file: {err}")))
}

/// One-shot purge of legacy Google OAuth state.
///
/// Returns `Ok(true)` when the migration ran and the legacy auth file was
/// (potentially) removed, `Ok(false)` when the migration was already applied
/// for this install.
pub fn purge_legacy_oauth_if_needed() -> Result<bool> {
    let migration_path = default_migration_path()?;
    let applied = read_applied_migration(&migration_path);
    if applied >= CURRENT_AUTH_MIGRATION {
        return Ok(false);
    }
    // Best-effort wipe of the legacy auth file. If it does not exist, that's
    // fine; the goal is purely to make sure no stale project/tier metadata is
    // reused after the v0.1.14 fixes.
    delete_default_auth()?;
    write_applied_migration(&migration_path, CURRENT_AUTH_MIGRATION)?;
    Ok(true)
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
        .append_pair("response_type", "code")
        .append_pair("client_id", GOOGLE_CLIENT_ID)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", GOOGLE_OAUTH_SCOPE)
        .append_pair("code_challenge", &pkce.code_challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("state", state);
    format!("{GOOGLE_OAUTH_AUTHORIZE_URL}?{}", serializer.finish())
}

pub async fn exchange_oauth_code(
    http: &reqwest::Client,
    code: &str,
    redirect_uri: &str,
    pkce: &PkceCodes,
) -> Result<GoogleAuthStatus> {
    let response = http
        .post(GOOGLE_OAUTH_TOKEN_URL)
        .header("content-type", "application/x-www-form-urlencoded")
        .header("accept", "application/json")
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", GOOGLE_CLIENT_ID),
            ("client_secret", GOOGLE_CLIENT_SECRET),
            ("code_verifier", &pkce.code_verifier),
        ])
        .send()
        .await
        .map_err(|err| AppError::Network(format!("google oauth exchange failed: {err}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Auth(format!(
            "google oauth exchange failed with {status}: {body}"
        )));
    }

    let body: TokenResponse = response
        .json()
        .await
        .map_err(|err| AppError::Decode(format!("invalid google oauth body: {err}")))?;
    let email = fetch_user_email(http, &body.access_token)
        .await
        .ok()
        .or_else(|| body.id_token.as_deref().and_then(token_email));
    save_oauth_tokens(&default_auth_path()?, body, email)
}

fn save_oauth_tokens(
    path: &Path,
    body: TokenResponse,
    email: Option<String>,
) -> Result<GoogleAuthStatus> {
    let expires_at_ms = expires_at(body.expires_in.unwrap_or(3600));
    let auth = StoredAuth {
        provider: "google".into(),
        auth_mode: "oauth".into(),
        tokens: StoredTokens {
            access_token: body.access_token,
            refresh_token: body.refresh_token.unwrap_or_default(),
            id_token: body.id_token,
            scope: body.scope,
            token_type: body.token_type,
            email,
            expires_at_ms: Some(expires_at_ms),
        },
        user: None,
        last_refresh_ms: Some(now_ms()),
    };
    write_auth_file(path, &auth)?;
    Ok(status_from_auth(&auth))
}

#[derive(Debug, Deserialize)]
struct UserInfo {
    #[serde(default)]
    email: Option<String>,
}

async fn fetch_user_email(http: &reqwest::Client, access_token: &str) -> Result<String> {
    let response = http
        .get(GOOGLE_USER_INFO_URL)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|err| AppError::Network(format!("google userinfo failed: {err}")))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Auth(format!(
            "google userinfo failed with {status}: {body}"
        )));
    }
    let user: UserInfo = response
        .json()
        .await
        .map_err(|err| AppError::Decode(format!("invalid google userinfo body: {err}")))?;
    user.email
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::Decode("google userinfo missing email".into()))
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

fn status_from_auth(auth: &StoredAuth) -> GoogleAuthStatus {
    if auth.provider != "google" || auth.auth_mode != "oauth" {
        return GoogleAuthStatus::disconnected();
    }
    GoogleAuthStatus {
        connected: !auth.tokens.access_token.is_empty(),
        email: auth
            .tokens
            .email
            .clone()
            .or_else(|| auth.tokens.id_token.as_deref().and_then(token_email)),
        project_id: auth.user.as_ref().map(|user| user.project_id.clone()),
        user_tier: auth.user.as_ref().and_then(|user| {
            user.user_tier_name
                .clone()
                .or_else(|| user.user_tier.clone())
        }),
        expires_at_ms: auth.tokens.expires_at_ms,
        last_refresh_ms: auth.last_refresh_ms,
    }
}

fn token_email(token: &str) -> Option<String> {
    jwt_payload(token).and_then(|payload| {
        payload
            .get("email")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
    })
}

fn jwt_payload(token: &str) -> Option<serde_json::Value> {
    let payload = token.split('.').nth(1)?;
    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&decoded).ok()
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
