use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use sinew_core::{AppError, Result};

use crate::model_info::PROVIDER_ID;

pub const DEFAULT_BASE_URL: &str = "http://localhost:11434";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaAuthStatus {
    pub connected: bool,
    pub base_url: Option<String>,
    pub last_validated_ms: Option<i64>,
}

impl OllamaAuthStatus {
    pub fn disconnected() -> Self {
        Self {
            connected: false,
            base_url: None,
            last_validated_ms: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredAuth {
    provider: String,
    auth_mode: String,
    base_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_validated_ms: Option<i64>,
}

pub fn default_base_url() -> String {
    DEFAULT_BASE_URL.to_string()
}

pub fn load_default_base_url() -> Result<Option<String>> {
    let path = default_auth_path()?;
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(AppError::Auth(format!("unable to read auth file: {err}"))),
    };
    let payload: StoredAuth = serde_json::from_slice(&bytes)
        .map_err(|err| AppError::Auth(format!("invalid auth file: {err}")))?;
    if payload.provider != PROVIDER_ID {
        return Ok(None);
    }
    let base_url = normalize_base_url(&payload.base_url);
    if base_url.is_empty() {
        return Ok(None);
    }
    Ok(Some(base_url))
}

pub fn save_default_base_url(base_url: &str) -> Result<OllamaAuthStatus> {
    let base_url = normalize_base_url(base_url);
    if base_url.is_empty() {
        return Err(AppError::Auth("Ollama address cannot be empty".into()));
    }
    let auth = StoredAuth {
        provider: PROVIDER_ID.into(),
        auth_mode: "local".into(),
        base_url,
        last_validated_ms: Some(now_ms()),
    };
    write_auth_file(&default_auth_path()?, &auth)?;
    Ok(status_from_auth(&auth))
}

pub fn touch_default_auth_validation() -> Result<OllamaAuthStatus> {
    let path = default_auth_path()?;
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(OllamaAuthStatus::disconnected())
        }
        Err(err) => return Err(AppError::Auth(format!("unable to read auth file: {err}"))),
    };
    let mut auth: StoredAuth = serde_json::from_slice(&bytes)
        .map_err(|err| AppError::Auth(format!("invalid auth file: {err}")))?;
    auth.last_validated_ms = Some(now_ms());
    write_auth_file(&path, &auth)?;
    Ok(status_from_auth(&auth))
}

pub fn load_default_auth_status() -> Result<OllamaAuthStatus> {
    let path = default_auth_path()?;
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(OllamaAuthStatus::disconnected())
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

pub fn normalize_base_url(base_url: &str) -> String {
    base_url.trim().trim_end_matches('/').to_string()
}

fn default_auth_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("dev", "hyrak", "sinew")
        .ok_or_else(|| AppError::Auth("unable to resolve local data directory".into()))?;
    Ok(dirs.data_local_dir().join("ollama-auth.json"))
}

fn status_from_auth(auth: &StoredAuth) -> OllamaAuthStatus {
    if auth.provider != PROVIDER_ID {
        return OllamaAuthStatus::disconnected();
    }
    let base_url = normalize_base_url(&auth.base_url);
    OllamaAuthStatus {
        connected: !base_url.is_empty(),
        base_url: (!base_url.is_empty()).then_some(base_url),
        last_validated_ms: auth.last_validated_ms,
    }
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
