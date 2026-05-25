use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use claakecode_core::{AppError, Result};

use crate::model_info::PROVIDER_ID;

#[derive(Clone)]
pub struct Credential {
    api_key: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenRouterAuthStatus {
    pub connected: bool,
    pub key_preview: Option<String>,
    pub last_validated_ms: Option<i64>,
}

impl OpenRouterAuthStatus {
    pub fn disconnected() -> Self {
        Self {
            connected: false,
            key_preview: None,
            last_validated_ms: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredAuth {
    provider: String,
    auth_mode: String,
    tokens: StoredTokens,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_validated_ms: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredTokens {
    api_key: String,
}

impl Credential {
    pub fn from_api_key(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
        }
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
        if payload.provider != PROVIDER_ID || payload.auth_mode != "api_key" {
            return Ok(None);
        }
        let api_key = payload.tokens.api_key.trim();
        if api_key.is_empty() {
            return Err(AppError::Auth("openrouter auth is missing API key".into()));
        }

        Ok(Some(Self::from_api_key(api_key.to_string())))
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }
}

pub fn load_default_api_key() -> Result<Option<String>> {
    Ok(Credential::load_default()?.map(|credential| credential.api_key))
}

pub fn save_default_api_key(api_key: &str) -> Result<OpenRouterAuthStatus> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(AppError::Auth("OpenRouter API key cannot be empty".into()));
    }
    let auth = StoredAuth {
        provider: PROVIDER_ID.into(),
        auth_mode: "api_key".into(),
        tokens: StoredTokens {
            api_key: api_key.to_string(),
        },
        last_validated_ms: Some(now_ms()),
    };
    write_auth_file(&default_auth_path()?, &auth)?;
    Ok(status_from_auth(&auth))
}

pub fn touch_default_auth_validation() -> Result<OpenRouterAuthStatus> {
    let path = default_auth_path()?;
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(OpenRouterAuthStatus::disconnected())
        }
        Err(err) => return Err(AppError::Auth(format!("unable to read auth file: {err}"))),
    };
    let mut auth: StoredAuth = serde_json::from_slice(&bytes)
        .map_err(|err| AppError::Auth(format!("invalid auth file: {err}")))?;
    auth.last_validated_ms = Some(now_ms());
    write_auth_file(&path, &auth)?;
    Ok(status_from_auth(&auth))
}

pub fn load_default_auth_status() -> Result<OpenRouterAuthStatus> {
    load_auth_status(&default_auth_path()?)
}

pub fn load_auth_status(path: &Path) -> Result<OpenRouterAuthStatus> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(OpenRouterAuthStatus::disconnected())
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

fn default_auth_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("dev", "williampeynichou", "claakecode")
        .ok_or_else(|| AppError::Auth("unable to resolve local data directory".into()))?;
    Ok(dirs.data_local_dir().join("openrouter-auth.json"))
}

fn status_from_auth(auth: &StoredAuth) -> OpenRouterAuthStatus {
    if auth.provider != PROVIDER_ID || auth.auth_mode != "api_key" {
        return OpenRouterAuthStatus::disconnected();
    }
    let api_key = auth.tokens.api_key.trim();
    OpenRouterAuthStatus {
        connected: !api_key.is_empty(),
        key_preview: (!api_key.is_empty()).then(|| key_preview(api_key)),
        last_validated_ms: auth.last_validated_ms,
    }
}

fn key_preview(api_key: &str) -> String {
    let chars = api_key.chars().collect::<Vec<_>>();
    if chars.len() <= 12 {
        return "••••".to_string();
    }
    let prefix = chars.iter().take(7).collect::<String>();
    let suffix = chars
        .iter()
        .rev()
        .take(4)
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{prefix}…{suffix}")
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

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
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
