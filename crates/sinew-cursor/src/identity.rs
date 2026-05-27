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
use sinew_core::{AppError, Result};

pub const CURSOR_CLIENT_VERSION: &str = "3.5.38";
// Last verified against Cursor IDE 3.5.33 (May 2026). Override with SINEW_CURSOR_CLIENT_VERSION.

static EPHEMERAL_MACHINE_ID: OnceLock<String> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct CursorIdeIdentity {
    pub client_version: String,
    pub machine_id: String,
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
        let machine_id = load_or_create_sinew_machine_id();
        let client_version = Self::resolve_client_version();
        let (platform, arch) = detect_platform();
        Self {
            client_version,
            machine_id,
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
        set_header(headers, "user-agent", &self.user_agent());
        set_header(headers, "x-cursor-client-version", &self.client_version);
        set_header(headers, "x-cursor-client-type", "ide");
        set_header(headers, "x-cursor-client-device-type", "desktop");
        set_header(headers, "x-cursor-client-os", &self.platform);
        set_header(headers, "x-cursor-client-arch", &self.arch);
        set_header(headers, "x-ghost-mode", "false");
        set_header(headers, "x-new-onboarding-completed", "true");
        set_header(headers, "x-cursor-timezone", &self.timezone);
        set_header(headers, "x-cursor-checksum", &self.checksum());
        set_header(headers, "x-session-id", session_id);
        set_header(headers, "x-request-id", request_id);
    }

    fn checksum(&self) -> String {
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
        format!("{encoded}{}", self.machine_id)
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
    let Some(path) = sinew_device_path() else {
        return uuid::Uuid::new_v4().to_string();
    };

    if let Ok(contents) = fs::read_to_string(&path) {
        if let Ok(device) = serde_json::from_str::<PersistedDevice>(&contents) {
            let trimmed = device.machine_id.trim();
            if is_valid_machine_id(trimmed) {
                return trimmed.to_string();
            }
        }
    }

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
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "(Get-TimeZone).Id"])
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
