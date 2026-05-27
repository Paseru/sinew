use std::{
    path::PathBuf,
    sync::OnceLock,
};

use base64::Engine as _;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use rusqlite::Connection;
use sinew_core::{AppError, Result};

pub const CURSOR_CLIENT_VERSION: &str = "3.5.33";

static FALLBACK_MACHINE_ID: OnceLock<String> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct CursorIdeIdentity {
    pub client_version: String,
    pub machine_id: String,
    pub timezone: String,
}

impl CursorIdeIdentity {
    pub fn load() -> Self {
        let machine_id = read_ide_machine_id().unwrap_or_else(fallback_machine_id);
        let client_version = read_ide_client_version().unwrap_or_else(|| CURSOR_CLIENT_VERSION.into());
        let timezone = read_local_timezone();
        Self {
            client_version,
            machine_id,
            timezone,
        }
    }

    pub fn user_agent(&self) -> String {
        format!("Cursor/{}", self.client_version)
    }

    pub fn apply(&self, headers: &mut HeaderMap, session_id: &str, request_id: &str) {
        set_header(headers, "user-agent", &self.user_agent());
        set_header(headers, "x-cursor-client-version", &self.client_version);
        set_header(headers, "x-cursor-client-type", "ide");
        set_header(headers, "x-cursor-client-device-type", "desktop");
        set_header(headers, "x-cursor-client-os", "windows");
        set_header(headers, "x-cursor-client-arch", "x64");
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

fn set_header(headers: &mut HeaderMap, name: &str, value: &str) {
    if let (Ok(header_name), Ok(header_value)) = (
        HeaderName::from_bytes(name.as_bytes()),
        HeaderValue::from_str(value),
    ) {
        headers.insert(header_name, header_value);
    }
}

fn fallback_machine_id() -> String {
    FALLBACK_MACHINE_ID
        .get_or_init(|| uuid::Uuid::new_v4().to_string())
        .clone()
}

fn default_ide_state_db() -> PathBuf {
    if let Some(base) = std::env::var_os("APPDATA") {
        return PathBuf::from(base)
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("state.vscdb");
    }
    PathBuf::from(r"C:\Users\julie\AppData\Roaming\Cursor\User\globalStorage\state.vscdb")
}

fn read_ide_machine_id() -> Option<String> {
    read_ide_item("storage.serviceMachineId").ok()
}

fn read_ide_client_version() -> Option<String> {
    let path = default_cursor_product_json();
    if !path.exists() {
        return None;
    }
    let contents = std::fs::read_to_string(path).ok()?;
    let payload: serde_json::Value = serde_json::from_str(&contents).ok()?;
    payload
        .get("version")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn read_local_timezone() -> String {
    std::env::var("TZ").unwrap_or_else(|_| {
        if cfg!(windows) {
            "Europe/Paris".into()
        } else {
            "UTC".into()
        }
    })
}

fn default_cursor_product_json() -> PathBuf {
    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        let path = PathBuf::from(base)
            .join("Programs")
            .join("cursor")
            .join("resources")
            .join("app")
            .join("product.json");
        if path.exists() {
            return path;
        }
    }
    PathBuf::from(
        r"C:\Users\julie\AppData\Local\Programs\cursor\resources\app\product.json",
    )
}

fn read_ide_item(key: &str) -> Result<String> {
    let path = default_ide_state_db();
    if !path.exists() {
        return Err(AppError::Auth("Cursor IDE state db not found".into()));
    }
    let connection = Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|err| AppError::Auth(format!("unable to open Cursor state db: {err}")))?;
    connection
        .query_row(
            "SELECT value FROM ItemTable WHERE key = ?1",
            [key],
            |row| read_sqlite_value(row),
        )
        .map_err(|err| AppError::Auth(format!("missing Cursor key `{key}`: {err}")))
}

fn read_sqlite_value(row: &rusqlite::Row<'_>) -> rusqlite::Result<String> {
    use rusqlite::types::ValueRef;
    match row.get_ref(0)? {
        ValueRef::Text(text) => Ok(String::from_utf8_lossy(text).trim().to_string()),
        ValueRef::Blob(blob) => Ok(String::from_utf8_lossy(blob).trim().to_string()),
        _ => Err(rusqlite::Error::InvalidColumnType(
            0,
            "value".into(),
            rusqlite::types::Type::Text,
        )),
    }
}
