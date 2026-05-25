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

const OPENAI_OAUTH_AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
const OPENAI_OAUTH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const OPENAI_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const OPENAI_OAUTH_SCOPE: &str =
    "openid profile email offline_access api.connectors.read api.connectors.invoke";
const REFRESH_SKEW_MS: i64 = 60_000;

#[derive(Clone)]
pub enum Credential {
    ApiKey(String),
    OAuth(Arc<Mutex<OAuthToken>>),
}

pub struct OAuthToken {
    access: String,
    refresh: String,
    expires_at_ms: i64,
    account_id: Option<String>,
    source_path: Option<PathBuf>,
}

pub struct BearerToken {
    pub token: String,
    pub account_id: Option<String>,
    pub is_oauth: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAiAuthStatus {
    pub connected: bool,
    pub email: Option<String>,
    pub account_id: Option<String>,
    pub plan_type: Option<String>,
    pub expires_at_ms: Option<i64>,
    pub last_refresh_ms: Option<i64>,
}

impl OpenAiAuthStatus {
    pub fn disconnected() -> Self {
        Self {
            connected: false,
            email: None,
            account_id: None,
            plan_type: None,
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
    id_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    plan_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at_ms: Option<i64>,
}

impl Credential {
    pub fn from_api_key(token: impl Into<String>) -> Self {
        Self::ApiKey(token.into())
    }

    pub fn from_oauth_parts(
        access: impl Into<String>,
        refresh: impl Into<String>,
        expires_at_ms: i64,
        account_id: Option<String>,
        source_path: Option<PathBuf>,
    ) -> Self {
        Self::OAuth(Arc::new(Mutex::new(OAuthToken {
            access: access.into(),
            refresh: refresh.into(),
            expires_at_ms,
            account_id,
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

        if payload.provider != "openai" || payload.auth_mode != "oauth" {
            return Ok(None);
        }

        if payload.tokens.access_token.is_empty() {
            return Err(AppError::Auth(
                "openai oauth is missing access token".into(),
            ));
        }

        let expires_at_ms = payload
            .tokens
            .expires_at_ms
            .or_else(|| token_expiry_ms(&payload.tokens.access_token))
            .unwrap_or(0);

        Ok(Some(Self::OAuth(Arc::new(Mutex::new(OAuthToken {
            access: payload.tokens.access_token,
            refresh: payload.tokens.refresh_token,
            expires_at_ms,
            account_id: payload.tokens.account_id,
            source_path: Some(path.to_path_buf()),
        })))))
    }

    pub fn is_oauth(&self) -> bool {
        matches!(self, Self::OAuth(_))
    }

    pub async fn bearer(&self, http: &reqwest::Client) -> Result<BearerToken> {
        match self {
            Self::ApiKey(key) => Ok(BearerToken {
                token: key.clone(),
                account_id: None,
                is_oauth: false,
            }),
            Self::OAuth(state) => {
                {
                    let guard = state.lock().await;
                    if !is_expired(guard.expires_at_ms) {
                        return Ok(BearerToken {
                            token: guard.access.clone(),
                            account_id: guard.account_id.clone(),
                            is_oauth: true,
                        });
                    }
                    if guard.refresh.is_empty() {
                        return Err(AppError::Auth(
                            "openai oauth token expired and cannot be refreshed".into(),
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
                let expires_at_ms = token_expiry_ms(&access).unwrap_or_else(|| {
                    now_ms() + (fresh.expires_in.unwrap_or(3600) as i64 * 1000) - REFRESH_SKEW_MS
                });
                let account_id = fresh.account_id.or_else(|| token_account_id(&access));

                let (source_path, account_id) = {
                    let mut guard = state.lock().await;
                    guard.access = access.clone();
                    guard.refresh = refresh.clone();
                    guard.expires_at_ms = expires_at_ms;
                    if account_id.is_some() {
                        guard.account_id = account_id;
                    }
                    (guard.source_path.clone(), guard.account_id.clone())
                };

                if let Some(path) = source_path {
                    if let Err(err) = persist_refresh(
                        &path,
                        fresh.id_token.as_deref(),
                        &access,
                        &refresh,
                        &account_id,
                        expires_at_ms,
                    ) {
                        tracing::warn!(error = %err, "failed to persist refreshed openai oauth token");
                    }
                }

                Ok(BearerToken {
                    token: access,
                    account_id,
                    is_oauth: true,
                })
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

#[derive(Debug, Deserialize)]
struct RefreshBody {
    #[serde(default)]
    id_token: Option<String>,
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    account_id: Option<String>,
}

async fn refresh_token(http: &reqwest::Client, refresh_token: &str) -> Result<RefreshBody> {
    let response = http
        .post(OPENAI_OAUTH_TOKEN_URL)
        .header("content-type", "application/x-www-form-urlencoded")
        .header("accept", "application/json")
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", OPENAI_CLIENT_ID),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .map_err(|err| AppError::Network(format!("openai oauth refresh failed: {err}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Auth(format!(
            "openai oauth refresh failed with {status}: {body}"
        )));
    }

    response
        .json()
        .await
        .map_err(|err| AppError::Decode(format!("invalid openai oauth refresh body: {err}")))
}

fn persist_refresh(
    path: &Path,
    id_token: Option<&str>,
    access: &str,
    refresh: &str,
    account_id: &Option<String>,
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
    let previous_account_id = root.tokens.account_id.take();
    root.tokens.account_id = account_id
        .clone()
        .or_else(|| token_account_id(access))
        .or(previous_account_id);
    let previous_plan_type = root.tokens.plan_type.take();
    let id_token_plan_type = root.tokens.id_token.as_deref().and_then(token_plan_type);
    root.tokens.plan_type =
        token_plan_type(access).or_else(|| id_token_plan_type.or(previous_plan_type));
    root.last_refresh_ms = Some(now_ms());

    write_auth_file(path, &root)
}

pub fn default_auth_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("dev", "williampeynichou", "claakecode")
        .ok_or_else(|| AppError::Auth("unable to resolve local data directory".into()))?;
    Ok(dirs.data_local_dir().join("openai-auth.json"))
}

pub fn load_default_auth_status() -> Result<OpenAiAuthStatus> {
    load_auth_status(&default_auth_path()?)
}

pub fn load_auth_status(path: &Path) -> Result<OpenAiAuthStatus> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(OpenAiAuthStatus::disconnected());
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
        .append_pair("response_type", "code")
        .append_pair("client_id", OPENAI_CLIENT_ID)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", OPENAI_OAUTH_SCOPE)
        .append_pair("code_challenge", &pkce.code_challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("id_token_add_organizations", "true")
        .append_pair("codex_cli_simplified_flow", "true")
        .append_pair("state", state)
        .append_pair("originator", "claakecode_desktop");
    format!("{OPENAI_OAUTH_AUTHORIZE_URL}?{}", serializer.finish())
}

#[derive(Debug, Deserialize)]
struct AuthCodeTokenBody {
    id_token: String,
    access_token: String,
    refresh_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
}

pub async fn exchange_oauth_code(
    http: &reqwest::Client,
    code: &str,
    redirect_uri: &str,
    pkce: &PkceCodes,
) -> Result<OpenAiAuthStatus> {
    let response = http
        .post(OPENAI_OAUTH_TOKEN_URL)
        .header("content-type", "application/x-www-form-urlencoded")
        .header("accept", "application/json")
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", OPENAI_CLIENT_ID),
            ("code_verifier", &pkce.code_verifier),
        ])
        .send()
        .await
        .map_err(|err| AppError::Network(format!("openai oauth exchange failed: {err}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Auth(format!(
            "openai oauth exchange failed with {status}: {body}"
        )));
    }

    let body: AuthCodeTokenBody = response
        .json()
        .await
        .map_err(|err| AppError::Decode(format!("invalid openai oauth body: {err}")))?;
    save_oauth_tokens(
        &default_auth_path()?,
        &body.id_token,
        &body.access_token,
        &body.refresh_token,
        body.expires_in,
    )
}

fn save_oauth_tokens(
    path: &Path,
    id_token: &str,
    access_token: &str,
    refresh_token: &str,
    expires_in: Option<u64>,
) -> Result<OpenAiAuthStatus> {
    let expires_at_ms = token_expiry_ms(access_token)
        .unwrap_or_else(|| now_ms() + (expires_in.unwrap_or(3600) as i64 * 1000) - REFRESH_SKEW_MS);
    let account_id = token_account_id(access_token)
        .or_else(|| token_account_id(id_token))
        .filter(|value| !value.is_empty());
    let auth = StoredAuth {
        provider: "openai".into(),
        auth_mode: "oauth".into(),
        tokens: StoredTokens {
            access_token: access_token.into(),
            refresh_token: refresh_token.into(),
            id_token: Some(id_token.into()),
            account_id,
            email: token_email(id_token).or_else(|| token_email(access_token)),
            plan_type: token_plan_type(access_token).or_else(|| token_plan_type(id_token)),
            expires_at_ms: Some(expires_at_ms),
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

fn status_from_auth(auth: &StoredAuth) -> OpenAiAuthStatus {
    if auth.provider != "openai" || auth.auth_mode != "oauth" {
        return OpenAiAuthStatus::disconnected();
    }
    OpenAiAuthStatus {
        connected: !auth.tokens.access_token.is_empty(),
        email: auth.tokens.email.clone(),
        account_id: auth
            .tokens
            .account_id
            .clone()
            .or_else(|| token_account_id(&auth.tokens.access_token)),
        plan_type: auth
            .tokens
            .plan_type
            .clone()
            .or_else(|| token_plan_type(&auth.tokens.access_token)),
        expires_at_ms: auth
            .tokens
            .expires_at_ms
            .or_else(|| token_expiry_ms(&auth.tokens.access_token)),
        last_refresh_ms: auth.last_refresh_ms,
    }
}

fn token_expiry_ms(token: &str) -> Option<i64> {
    jwt_payload(token)
        .and_then(|payload| payload.get("exp").and_then(|value| value.as_i64()))
        .map(|seconds| seconds * 1000)
}

fn token_account_id(token: &str) -> Option<String> {
    jwt_payload(token).and_then(|payload| {
        payload
            .get("https://api.openai.com/auth")
            .and_then(|auth| auth.get("chatgpt_account_id"))
            .or_else(|| payload.get("chatgpt_account_id"))
            .or_else(|| payload.get("account_id"))
            .and_then(|value| value.as_str())
            .map(str::to_string)
    })
}

fn token_email(token: &str) -> Option<String> {
    jwt_payload(token).and_then(|payload| {
        payload
            .get("https://api.openai.com/profile")
            .and_then(|profile| profile.get("email"))
            .or_else(|| payload.get("email"))
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
    })
}

fn token_plan_type(token: &str) -> Option<String> {
    jwt_payload(token).and_then(|payload| {
        payload
            .get("https://api.openai.com/auth")
            .and_then(|auth| auth.get("chatgpt_plan_type"))
            .or_else(|| payload.get("chatgpt_plan_type"))
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(plan_label)
    })
}

fn plan_label(raw: &str) -> String {
    match raw {
        "free" => "Free",
        "plus" => "Plus",
        "pro" => "Pro",
        "go" => "Go",
        "team" | "business" | "self_serve_business" | "self_serve_business_usage_based" => {
            "Business"
        }
        "enterprise" | "enterprise_cbp_usage_based" | "hc" => "Enterprise",
        "edu" => "Edu",
        other => other,
    }
    .to_string()
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
