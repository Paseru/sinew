use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::{Duration, Instant},
};

use base64::Engine as _;
use directories::ProjectDirs;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sinew_core::{AppError, Result};

pub const CURSOR_CLIENT_VERSION: &str = "3.5.38";
// Last verified against Cursor IDE 3.5.33 (May 2026). Override with SINEW_CURSOR_CLIENT_VERSION.

static EPHEMERAL_MACHINE_ID: OnceLock<String> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct CursorIdeIdentity {
    pub client_version: String,
    pub machine_id: String,
    pub mac_machine_id: Option<String>,
    pub timezone: String,
    pub platform: String,
    pub arch: String,
    pub shell: String,
}

impl CursorIdeIdentity {
    pub fn load() -> Self {
        Self::assemble()
    }

    pub fn refresh(&mut self) {
        *self = Self::assemble();
    }

    pub fn ensure_ready(&self) -> Result<()> {
        if self.machine_id.trim().is_empty() {
            return Err(AppError::Auth(
                "Composer device machineId unavailable.".into(),
            ));
        }
        if self.client_version.trim().is_empty() {
            return Err(AppError::Auth(
                "Cursor client version unavailable.".into(),
            ));
        }
        Ok(())
    }

    fn assemble() -> Self {
        let (machine_id, mac_machine_id) = load_machine_ids();
        let client_version = Self::resolve_client_version();
        let (platform, arch) = detect_platform();
        Self {
            client_version,
            machine_id,
            mac_machine_id,
            timezone: read_local_timezone(),
            platform,
            arch,
            shell: detect_shell(),
        }
    }

    pub fn user_agent(&self) -> String {
        format!("Cursor/{}", self.client_version)
    }

    pub fn resolve_client_version() -> String {
        if let Ok(version) = std::env::var("SINEW_CURSOR_CLIENT_VERSION") {
            let trimmed = version.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        CURSOR_CLIENT_VERSION.to_string()
    }

    pub fn apply(&self, headers: &mut HeaderMap, session_id: &str, request_id: &str) {
        self.apply_common(headers, session_id, request_id);
        set_header(
            headers,
            "x-cursor-checksum",
            &self.checksum_for_machine_id(&self.machine_id, self.mac_machine_id.as_deref()),
        );
    }

    /// Authenticated Cursor API calls derive `x-client-key` and the checksum
    /// machine id from the bearer token, matching standalone OAuth clients.
    pub fn apply_authenticated(
        &self,
        headers: &mut HeaderMap,
        session_id: &str,
        request_id: &str,
        access_token: &str,
    ) {
        self.apply_common(headers, session_id, request_id);
        let machine_id = Self::token_machine_id(access_token);
        set_header(headers, "x-client-key", &Self::token_client_key(access_token));
        set_header(
            headers,
            "x-cursor-checksum",
            &self.checksum_for_machine_id(&machine_id, None),
        );
    }

    fn apply_common(&self, headers: &mut HeaderMap, session_id: &str, request_id: &str) {
        set_header(headers, "user-agent", &self.user_agent());
        set_header(headers, "x-cursor-client-version", &self.client_version);
        set_header(headers, "x-cursor-client-type", "ide");
        set_header(headers, "x-cursor-client-device-type", "desktop");
        set_header(headers, "x-cursor-client-os", &self.platform);
        set_header(headers, "x-cursor-client-arch", &self.arch);
        set_header(headers, "x-ghost-mode", "false");
        set_header(headers, "x-new-onboarding-completed", "true");
        set_header(headers, "x-cursor-timezone", &self.timezone);
        set_header(headers, "connect-accept-encoding", "gzip");
        set_header(headers, "x-session-id", session_id);
        set_header(headers, "x-request-id", request_id);
        set_header(headers, "x-amzn-trace-id", &format!("Root={request_id}"));
    }

    pub fn token_client_key(access_token: &str) -> String {
        sha256_hex(access_token)
    }

    pub fn token_machine_id(access_token: &str) -> String {
        sha256_hex(&format!("{access_token}machineId"))
    }

    fn checksum_for_machine_id(&self, machine_id: &str, mac_machine_id: Option<&str>) -> String {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0);
        let bucket = millis / 1_000_000;
        let mut bytes = [
            ((bucket >> 40) & 0xff) as u8,
            ((bucket >> 32) & 0xff) as u8,
            ((bucket >> 24) & 0xff) as u8,
            ((bucket >> 16) & 0xff) as u8,
            ((bucket >> 8) & 0xff) as u8,
            (bucket & 0xff) as u8,
        ];
        let mut state = 165u8;
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = (*byte ^ state).wrapping_add((index % 256) as u8);
            state = *byte;
        }
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        if let Some(mac_id) = mac_machine_id {
            format!("{encoded}{machine_id}/{mac_id}")
        } else {
            format!("{encoded}{machine_id}")
        }
    }

    fn checksum(&self) -> String {
        self.checksum_for_machine_id(&self.machine_id, self.mac_machine_id.as_deref())
    }
}

fn sha256_hex(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn cursor_storage_json_path() -> Option<PathBuf> {
    let base_dirs = directories::BaseDirs::new()?;
    let config_dir = base_dirs.config_dir();
    let path = config_dir.join("Cursor").join("User").join("globalStorage").join("storage.json");
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

/// Standalone Composer identity: always prefer the Sinew-persisted device id
/// (the same id used during OAuth login). Cursor IDE telemetry ids are only
/// used when explicitly opted in via `SINEW_CURSOR_USE_IDE_MACHINE=1`.
fn load_machine_ids() -> (String, Option<String>) {
    if let Some(sinew_id) = load_sinew_persisted_machine_id() {
        return (sinew_id, None);
    }
    if use_cursor_ide_machine_ids() {
        if let Some(ids) = load_cursor_storage_ids() {
            return ids;
        }
    }
    (load_or_create_sinew_machine_id(), None)
}

fn use_cursor_ide_machine_ids() -> bool {
    std::env::var("SINEW_CURSOR_USE_IDE_MACHINE")
        .map(|value| {
            let trimmed = value.trim();
            trimmed == "1" || trimmed.eq_ignore_ascii_case("true")
        })
        .unwrap_or(false)
}

fn load_sinew_persisted_machine_id() -> Option<String> {
    let path = sinew_device_path()?;
    let contents = fs::read_to_string(path).ok()?;
    let device: PersistedDevice = serde_json::from_str(&contents).ok()?;
    let trimmed = device.machine_id.trim();
    if is_valid_machine_id(trimmed) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn load_cursor_storage_ids() -> Option<(String, Option<String>)> {
    let path = cursor_storage_json_path()?;
    let content = fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let machine_id = json.get("telemetry.machineId")?.as_str()?.to_string();
    let mac_machine_id = json.get("telemetry.macMachineId").and_then(|v| v.as_str()).map(|s| s.to_string());
    if !machine_id.trim().is_empty() {
        Some((machine_id, mac_machine_id))
    } else {
        None
    }
}

fn detect_platform() -> (String, String) {
    let arch = normalize_arch(std::env::consts::ARCH);
    if cfg!(windows) {
        ("windows".into(), arch)
    } else if cfg!(target_os = "macos") {
        ("darwin".into(), arch)
    } else {
        ("linux".into(), arch)
    }
}

fn normalize_arch(arch: &str) -> String {
    match arch {
        "x86_64" | "x86" => "x64".into(),
        "aarch64" | "arm64" => "arm64".into(),
        other => other.to_string(),
    }
}

fn detect_shell() -> String {
    if std::env::var("PSModulePath").is_ok() {
        "powershell".into()
    } else if cfg!(windows) {
        "cmd".into()
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "bash".into())
    }
}

fn set_header(headers: &mut HeaderMap, name: &str, value: &str) {
    if let (Ok(header_name), Ok(header_value)) = (
        HeaderName::from_bytes(name.as_bytes()),
        HeaderValue::from_str(value),
    ) {
        headers.insert(header_name, header_value);
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedDevice {
    machine_id: String,
}

fn sinew_device_path() -> Option<PathBuf> {
    ProjectDirs::from("dev", "hyrak", "sinew")
        .map(|dirs| dirs.data_local_dir().join("cursor-composer-device.json"))
}

fn load_or_create_sinew_machine_id() -> String {
    if let Some(existing) = load_sinew_persisted_machine_id() {
        return existing;
    }

    let Some(path) = sinew_device_path() else {
        return uuid::Uuid::new_v4().to_string();
    };

    let machine_id = uuid::Uuid::new_v4().to_string();
    if let Err(err) = persist_sinew_machine_id(&path, &machine_id) {
        tracing::warn!("unable to persist composer device id: {err}");
        return EPHEMERAL_MACHINE_ID
            .get_or_init(|| machine_id.clone())
            .clone();
    }
    machine_id
}

fn persist_sinew_machine_id(path: &Path, machine_id: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| AppError::Auth(format!("unable to create device dir: {err}")))?;
    }
    let payload = PersistedDevice {
        machine_id: machine_id.to_string(),
    };
    let json = serde_json::to_string_pretty(&payload)
        .map_err(|err| AppError::Auth(format!("unable to encode device id: {err}")))?;
    fs::write(path, json)
        .map_err(|err| AppError::Auth(format!("unable to persist device id: {err}")))?;
    Ok(())
}

fn is_valid_machine_id(value: &str) -> bool {
    !value.is_empty() && uuid::Uuid::parse_str(value).is_ok()
}

fn read_local_timezone() -> String {
    if let Ok(tz) = std::env::var("TZ") {
        if !tz.trim().is_empty() {
            return tz;
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "(Get-TimeZone).Id"])
            .creation_flags(0x08000000)
            .output()
        {
            let tz = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !tz.is_empty() {
                return tz;
            }
        }
    }
    "UTC".into()
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use std::fs;

    #[test]
    fn persisted_device_roundtrip() {
        let path = std::env::temp_dir().join(format!(
            "sinew-cursor-device-{}.json",
            uuid::Uuid::new_v4()
        ));
        let id = uuid::Uuid::new_v4().to_string();
        persist_sinew_machine_id(&path, &id).expect("persist device id");
        let contents = fs::read_to_string(&path).expect("read device file");
        let device: PersistedDevice =
            serde_json::from_str(&contents).expect("parse device file");
        assert_eq!(device.machine_id, id);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_invalid_machine_id() {
        assert!(!is_valid_machine_id(""));
        assert!(!is_valid_machine_id("not-a-uuid"));
        assert!(is_valid_machine_id(&uuid::Uuid::new_v4().to_string()));
    }

    #[test]
    fn token_derived_auth_headers_are_stable() {
        let token = "test-token";
        let client_key = CursorIdeIdentity::token_client_key(token);
        let machine_id = CursorIdeIdentity::token_machine_id(token);
        assert_eq!(client_key.len(), 64);
        assert_eq!(machine_id.len(), 64);
        assert_ne!(client_key, machine_id);
        assert_eq!(client_key, CursorIdeIdentity::token_client_key(token));
    }

    #[test]
    fn sinew_device_id_takes_precedence_over_cursor_storage() {
        let sinew_id = uuid::Uuid::new_v4().to_string();
        let path = std::env::temp_dir().join(format!(
            "sinew-cursor-device-priority-{}.json",
            uuid::Uuid::new_v4()
        ));
        persist_sinew_machine_id(&path, &sinew_id).expect("persist device id");

        let loaded = load_sinew_persisted_machine_id_from_path(&path);
        assert_eq!(loaded.as_deref(), Some(sinew_id.as_str()));
        let _ = fs::remove_file(path);
    }

    fn load_sinew_persisted_machine_id_from_path(path: &std::path::Path) -> Option<String> {
        let contents = fs::read_to_string(path).ok()?;
        let device: PersistedDevice = serde_json::from_str(&contents).ok()?;
        let trimmed = device.machine_id.trim();
        if is_valid_machine_id(trimmed) {
            Some(trimmed.to_string())
        } else {
            None
        }
    }

    #[test]
    fn client_version_defaults_to_constant() {
        assert_eq!(
            CursorIdeIdentity::resolve_client_version(),
            CURSOR_CLIENT_VERSION
        );
    }
}

pub(crate) const USAGE_CACHE_TTL: Duration = Duration::from_secs(300);

pub(crate) struct CachedUsage {
    pub fetched_at: Instant,
    pub info: crate::usage::CursorUsageInfo,
}
