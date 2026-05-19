use std::{collections::HashSet, path::Path, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

use anyhow::{anyhow, bail, Context, Result};
use reqwest::{header::ACCEPT, Url};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sqlx::{
    mysql::MySqlPoolOptions,
    postgres::{PgPoolOptions, PgSslMode},
    ConnectOptions, Row,
};
use uuid::Uuid;

const DEFAULT_ROW_LIMIT: u32 = 100;
const MAX_ROW_LIMIT: u32 = 10_000;
const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const MIN_TIMEOUT_MS: u64 = 1_000;
const MAX_TIMEOUT_MS: u64 = 300_000;
const STATUS_MESSAGE_LIMIT: usize = 2_000;
const ACTIVITY_PREVIEW_LIMIT: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseSourceEngine {
    #[serde(rename = "postgres")]
    Postgres,
    #[serde(rename = "mysql")]
    Mysql,
    #[serde(rename = "sqlite")]
    Sqlite,
    #[serde(rename = "mssql")]
    Mssql,
    #[serde(rename = "supabaseRest")]
    SupabaseRest,
}

impl Default for DatabaseSourceEngine {
    fn default() -> Self {
        Self::Postgres
    }
}

impl DatabaseSourceEngine {
    pub fn label(self) -> &'static str {
        match self {
            Self::Postgres => "PostgreSQL",
            Self::Mysql => "MySQL / MariaDB",
            Self::Sqlite => "SQLite",
            Self::Mssql => "Microsoft SQL Server",
            Self::SupabaseRest => "Supabase REST",
        }
    }

    pub fn agent_id(self) -> &'static str {
        match self {
            Self::Postgres => "postgresql",
            Self::Mysql => "mysql",
            Self::Sqlite => "sqlite",
            Self::Mssql => "mssql",
            Self::SupabaseRest => "supabase_rest",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseCredentialMode {
    #[serde(rename = "connectionString")]
    ConnectionString,
    #[serde(rename = "fields")]
    Fields,
}

impl Default for DatabaseCredentialMode {
    fn default() -> Self {
        Self::ConnectionString
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseSslMode {
    #[serde(rename = "disabled")]
    Disabled,
    #[serde(rename = "required")]
    Required,
    #[serde(rename = "strict")]
    Strict,
}

impl Default for DatabaseSslMode {
    fn default() -> Self {
        Self::Required
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseConnectionState {
    #[serde(rename = "untested")]
    Untested,
    #[serde(rename = "ok")]
    Ok,
    #[serde(rename = "error")]
    Error,
}

impl Default for DatabaseConnectionState {
    fn default() -> Self {
        Self::Untested
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseConnectionStatus {
    #[serde(default)]
    pub status: DatabaseConnectionState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_ms: Option<i64>,
}

impl Default for DatabaseConnectionStatus {
    fn default() -> Self {
        Self {
            status: DatabaseConnectionState::Untested,
            message: None,
            timestamp_ms: None,
        }
    }
}

impl DatabaseConnectionStatus {
    pub fn from_test(result: &DatabaseConnectionTestResult) -> Self {
        Self {
            status: match result.status {
                DatabaseConnectionTestState::Ok => DatabaseConnectionState::Ok,
                DatabaseConnectionTestState::Error => DatabaseConnectionState::Error,
            },
            message: Some(result.message.clone()),
            timestamp_ms: Some(result.timestamp_ms),
        }
    }

    fn normalized(mut self) -> Self {
        self.message = self
            .message
            .map(|message| clip_chars(message.trim(), STATUS_MESSAGE_LIMIT))
            .filter(|message| !message.is_empty());
        if self.status == DatabaseConnectionState::Untested {
            self.timestamp_ms = None;
            self.message = None;
        }
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseConnectionFields {
    #[serde(default)]
    pub host: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub database: String,
    #[serde(default)]
    pub ssl_mode: DatabaseSslMode,
    #[serde(default)]
    pub ssl_certificate: String,
}

impl DatabaseConnectionFields {
    fn normalized(mut self) -> Self {
        self.host = self.host.trim().to_string();
        self.user = self.user.trim().to_string();
        self.database = self.database.trim().to_string();
        self.ssl_certificate = self.ssl_certificate.trim().to_string();
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseSqliteConfig {
    #[serde(default)]
    pub file_path: String,
    #[serde(default)]
    pub create_if_missing: bool,
}

impl Default for DatabaseSqliteConfig {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            create_if_missing: true,
        }
    }
}

impl DatabaseSqliteConfig {
    fn normalized(mut self) -> Self {
        self.file_path = self.file_path.trim().to_string();
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseSupabaseRestConfig {
    #[serde(default)]
    pub project_url: String,
    #[serde(default)]
    pub anon_key: String,
    #[serde(default)]
    pub service_role_key: String,
    #[serde(default)]
    pub use_service_role: bool,
    #[serde(default)]
    pub allow_rpc: bool,
}

impl DatabaseSupabaseRestConfig {
    fn normalized(mut self) -> Self {
        self.project_url = self.project_url.trim().trim_end_matches('/').to_string();
        self.anon_key = self.anon_key.trim().to_string();
        self.service_role_key = self.service_role_key.trim().to_string();
        self
    }

    pub(crate) fn selected_key(&self) -> Option<&str> {
        if self.use_service_role && !self.service_role_key.trim().is_empty() {
            Some(self.service_role_key.trim())
        } else if !self.anon_key.trim().is_empty() {
            Some(self.anon_key.trim())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseSourceConfig {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub engine: DatabaseSourceEngine,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub default_schema: String,
    #[serde(default = "default_row_limit")]
    pub default_row_limit: u32,
    #[serde(default = "default_timeout_ms")]
    pub default_timeout_ms: u64,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default = "default_true")]
    pub require_confirmation_for_destructive: bool,
    #[serde(default)]
    pub last_connection_status: DatabaseConnectionStatus,
    #[serde(default)]
    pub credential_mode: DatabaseCredentialMode,
    #[serde(default)]
    pub connection_string: String,
    #[serde(default)]
    pub fields: DatabaseConnectionFields,
    #[serde(default)]
    pub sqlite: DatabaseSqliteConfig,
    #[serde(default)]
    pub supabase_rest: DatabaseSupabaseRestConfig,
}

impl Default for DatabaseSourceConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            engine: DatabaseSourceEngine::default(),
            enabled: true,
            note: String::new(),
            default_schema: String::new(),
            default_row_limit: DEFAULT_ROW_LIMIT,
            default_timeout_ms: DEFAULT_TIMEOUT_MS,
            read_only: false,
            require_confirmation_for_destructive: true,
            last_connection_status: DatabaseConnectionStatus::default(),
            credential_mode: DatabaseCredentialMode::default(),
            connection_string: String::new(),
            fields: DatabaseConnectionFields::default(),
            sqlite: DatabaseSqliteConfig::default(),
            supabase_rest: DatabaseSupabaseRestConfig::default(),
        }
    }
}

impl DatabaseSourceConfig {
    pub fn normalized(mut self) -> Self {
        self.id = self.id.trim().to_string();
        self.name = self.name.trim().to_string();
        self.note = self.note.trim().to_string();
        self.default_schema = self.default_schema.trim().to_string();
        self.default_row_limit = self.default_row_limit.clamp(1, MAX_ROW_LIMIT);
        self.default_timeout_ms = self.default_timeout_ms.clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS);
        self.last_connection_status = self.last_connection_status.normalized();
        self.connection_string = self.connection_string.trim().to_string();
        self.fields = self.fields.normalized();
        self.sqlite = self.sqlite.normalized();
        self.supabase_rest = self.supabase_rest.normalized();
        self
    }

    pub fn agent_summary(&self) -> DatabaseSourceSummary {
        DatabaseSourceSummary {
            name: self.name.clone(),
            engine: self.engine.agent_id().to_string(),
            default_schema: self.default_schema.clone(),
            default_row_limit: self.default_row_limit,
            default_timeout_ms: self.default_timeout_ms,
            read_only: self.read_only,
            require_confirmation_for_destructive: self.require_confirmation_for_destructive,
            last_connection_status: self.last_connection_status.clone(),
        }
    }

    pub fn redacted_error_message(&self, message: impl Into<String>) -> String {
        let mut redacted = message.into();
        for secret in self.secret_values() {
            if !secret.is_empty() {
                redacted = redacted.replace(secret, "[redacted]");
            }
        }
        clip_chars(redacted.trim(), STATUS_MESSAGE_LIMIT)
    }

    fn secret_values(&self) -> Vec<&str> {
        let mut values = Vec::new();
        if !self.connection_string.trim().is_empty() {
            values.push(self.connection_string.trim());
        }
        if !self.fields.password.is_empty() {
            values.push(self.fields.password.as_str());
        }
        if !self.supabase_rest.anon_key.trim().is_empty() {
            values.push(self.supabase_rest.anon_key.trim());
        }
        if !self.supabase_rest.service_role_key.trim().is_empty() {
            values.push(self.supabase_rest.service_role_key.trim());
        }
        values
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseSettings {
    #[serde(default)]
    pub sources: Vec<DatabaseSourceConfig>,
}

impl DatabaseSettings {
    pub fn normalized(mut self) -> Self {
        let mut seen_ids = HashSet::new();
        self.sources = self
            .sources
            .into_iter()
            .map(DatabaseSourceConfig::normalized)
            .map(|mut source| {
                if source.id.is_empty() || !seen_ids.insert(source.id.clone()) {
                    loop {
                        let id = Uuid::new_v4().to_string();
                        if seen_ids.insert(id.clone()) {
                            source.id = id;
                            break;
                        }
                    }
                }
                source
            })
            .collect();
        self
    }

    pub fn normalized_for_save(self) -> Result<Self> {
        let normalized = self.normalized();
        normalized.validate_for_save()?;
        Ok(normalized)
    }

    pub fn validate_for_save(&self) -> Result<()> {
        let mut names = HashSet::new();
        for source in &self.sources {
            if source.name.trim().is_empty() {
                bail!("database source name is required");
            }
            let key = source.name.trim().to_ascii_lowercase();
            if !names.insert(key) {
                bail!("database source names must be unique");
            }
            source.validate_shape()?;
        }
        Ok(())
    }

    pub fn active_count(&self) -> usize {
        self.sources.iter().filter(|source| source.enabled).count()
    }

    pub fn enabled_summaries(&self) -> Vec<DatabaseSourceSummary> {
        self.sources
            .iter()
            .filter(|source| source.enabled)
            .map(DatabaseSourceConfig::agent_summary)
            .collect()
    }

    pub fn find_enabled_source(&self, name: &str) -> Option<DatabaseSourceConfig> {
        let requested = name.trim();
        self.sources
            .iter()
            .find(|source| source.enabled && source.name == requested)
            .cloned()
    }
}

impl DatabaseSourceConfig {
    fn validate_shape(&self) -> Result<()> {
        match self.engine {
            DatabaseSourceEngine::Sqlite => {
                if self.sqlite.file_path.trim().is_empty() {
                    bail!("SQLite file path is required");
                }
            }
            DatabaseSourceEngine::SupabaseRest => {
                if self.supabase_rest.project_url.trim().is_empty() {
                    bail!("Supabase project URL is required");
                }
                Url::parse(self.supabase_rest.project_url.trim())
                    .map_err(|_| anyhow!("Supabase project URL is invalid"))?;
                if self.supabase_rest.anon_key.trim().is_empty()
                    && self.supabase_rest.service_role_key.trim().is_empty()
                {
                    bail!("Supabase anon key or service role key is required");
                }
            }
            DatabaseSourceEngine::Postgres
            | DatabaseSourceEngine::Mysql
            | DatabaseSourceEngine::Mssql => match self.credential_mode {
                DatabaseCredentialMode::ConnectionString => {
                    if self.connection_string.trim().is_empty() {
                        bail!("connection string is required");
                    }
                }
                DatabaseCredentialMode::Fields => {
                    if self.fields.host.trim().is_empty() {
                        bail!("host is required");
                    }
                    if self.fields.user.trim().is_empty() {
                        bail!("user is required");
                    }
                    if self.fields.database.trim().is_empty() {
                        bail!("database name is required");
                    }
                }
            },
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseSourceSummary {
    pub name: String,
    pub engine: String,
    pub default_schema: String,
    pub default_row_limit: u32,
    pub default_timeout_ms: u64,
    pub read_only: bool,
    pub require_confirmation_for_destructive: bool,
    pub last_connection_status: DatabaseConnectionStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseConnectionTestState {
    #[serde(rename = "ok")]
    Ok,
    #[serde(rename = "error")]
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseConnectionTestResult {
    pub ok: bool,
    pub source_id: String,
    pub source_name: String,
    pub engine: DatabaseSourceEngine,
    pub status: DatabaseConnectionTestState,
    pub message: String,
    pub timestamp_ms: i64,
    pub duration_ms: u64,
}

pub async fn test_database_source_connection(source: DatabaseSourceConfig) -> DatabaseConnectionTestResult {
    let source = source.normalized();
    let started = Instant::now();
    let timestamp_ms = now_ms();
    let result = match source.engine {
        DatabaseSourceEngine::Sqlite => test_sqlite_connection(&source),
        DatabaseSourceEngine::SupabaseRest => test_supabase_rest_connection(&source).await,
        DatabaseSourceEngine::Postgres => match validate_network_sql_source(&source) {
            Ok(()) => test_postgres_connection(&source).await,
            Err(err) => Err(err),
        },
        DatabaseSourceEngine::Mysql => match validate_network_sql_source(&source) {
            Ok(()) => test_mysql_connection(&source).await,
            Err(err) => Err(err),
        },
        DatabaseSourceEngine::Mssql => validate_network_sql_source(&source).and_then(|()| {
            bail!(
                "Microsoft SQL Server driver is not bundled in this build. Use a Postgres/MySQL/SQLite source, or contact support to enable MSSQL."
            )
        }),
    };

    let duration_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(message) => DatabaseConnectionTestResult {
            ok: true,
            source_id: source.id,
            source_name: source.name,
            engine: source.engine,
            status: DatabaseConnectionTestState::Ok,
            message: clip_chars(message.trim(), STATUS_MESSAGE_LIMIT),
            timestamp_ms,
            duration_ms,
        },
        Err(err) => DatabaseConnectionTestResult {
            ok: false,
            source_id: source.id.clone(),
            source_name: source.name.clone(),
            engine: source.engine,
            status: DatabaseConnectionTestState::Error,
            message: source.redacted_error_message(err.to_string()),
            timestamp_ms,
            duration_ms,
        },
    }
}

fn test_sqlite_connection(source: &DatabaseSourceConfig) -> Result<String> {
    let path = source.sqlite.file_path.trim();
    if path.is_empty() {
        bail!("SQLite file path is required");
    }
    let flags = if source.read_only {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else if source.sqlite.create_if_missing {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    };
    let conn = Connection::open_with_flags(Path::new(path), flags)
        .with_context(|| "unable to open SQLite database")?;
    conn.query_row("select 1", [], |_row| Ok(()))
        .context("SQLite validation query failed")?;
    Ok("Connection successful".to_string())
}

async fn test_supabase_rest_connection(source: &DatabaseSourceConfig) -> Result<String> {
    let project_url = source.supabase_rest.project_url.trim();
    if project_url.is_empty() {
        bail!("Supabase project URL is required");
    }
    let api_key = source
        .supabase_rest
        .selected_key()
        .ok_or_else(|| anyhow!("Supabase anon key or service role key is required"))?;
    let base = Url::parse(project_url).context("Supabase project URL is invalid")?;
    let url = base
        .join("rest/v1/")
        .context("unable to build Supabase REST endpoint")?;
    let timeout = Duration::from_millis(source.default_timeout_ms);
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .context("unable to build Supabase REST client")?;
    let response = client
        .get(url)
        .header("apikey", api_key)
        .bearer_auth(api_key)
        .header(ACCEPT, "application/json, application/openapi+json")
        .send()
        .await
        .context("Supabase REST request failed")?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!(
            "Supabase REST request failed ({status}): {}",
            clip_chars(body.trim(), 1_000)
        );
    }
    Ok("Connection successful".to_string())
}

fn validate_network_sql_source(source: &DatabaseSourceConfig) -> Result<()> {
    match source.credential_mode {
        DatabaseCredentialMode::ConnectionString => {
            if source.connection_string.trim().is_empty() {
                bail!("connection string is required");
            }
        }
        DatabaseCredentialMode::Fields => {
            if source.fields.host.trim().is_empty() {
                bail!("host is required");
            }
            if source.fields.user.trim().is_empty() {
                bail!("user is required");
            }
            if source.fields.database.trim().is_empty() {
                bail!("database name is required");
            }
        }
    }
    Ok(())
}

/// Build the effective connection URL for a network SQL source. Used by the
/// SQL drivers (Postgres/MySQL) when the user picked the "separate fields"
/// mode.
pub(crate) fn build_sql_connection_url(
    source: &DatabaseSourceConfig,
    default_port: u16,
    scheme: &str,
) -> Result<String> {
    match source.credential_mode {
        DatabaseCredentialMode::ConnectionString => {
            let trimmed = source.connection_string.trim();
            if trimmed.is_empty() {
                bail!("connection string is required");
            }
            Ok(trimmed.to_string())
        }
        DatabaseCredentialMode::Fields => {
            let fields = &source.fields;
            let host = fields.host.trim();
            if host.is_empty() {
                bail!("host is required");
            }
            let user = fields.user.trim();
            if user.is_empty() {
                bail!("user is required");
            }
            let database = fields.database.trim();
            if database.is_empty() {
                bail!("database name is required");
            }
            let port = fields.port.unwrap_or(default_port);
            let encoded_user = percent_encode_user(user);
            let auth = if fields.password.is_empty() {
                encoded_user
            } else {
                let pw = percent_encode_password(&fields.password);
                format!("{encoded_user}:{pw}")
            };
            Ok(format!(
                "{scheme}://{auth}@{host}:{port}/{}",
                percent_encode_path(database)
            ))
        }
    }
}

fn percent_encode_user(value: &str) -> String {
    percent_encode_component(value, b":/@?#")
}

fn percent_encode_password(value: &str) -> String {
    percent_encode_component(value, b":/@?#")
}

fn percent_encode_path(value: &str) -> String {
    percent_encode_component(value, b"?#")
}

fn percent_encode_component(value: &str, reserved: &[u8]) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        let needs_encoding = !matches!(
            byte,
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~'
        ) || reserved.contains(&byte);
        if needs_encoding {
            out.push('%');
            out.push_str(&format!("{byte:02X}"));
        } else {
            out.push(byte as char);
        }
    }
    out
}

async fn test_postgres_connection(source: &DatabaseSourceConfig) -> Result<String> {
    let url = build_sql_connection_url(source, 5432, "postgresql")?;
    let timeout = Duration::from_millis(source.default_timeout_ms.max(1_000));
    let mut opts: sqlx::postgres::PgConnectOptions = url
        .parse()
        .context("invalid Postgres connection string")?;
    opts = opts.application_name("claakecode");
    opts = match source.fields.ssl_mode {
        DatabaseSslMode::Disabled => opts.ssl_mode(PgSslMode::Disable),
        DatabaseSslMode::Required => opts.ssl_mode(PgSslMode::Require),
        DatabaseSslMode::Strict => opts.ssl_mode(PgSslMode::VerifyFull),
    };
    opts = opts.log_statements(tracing::log::LevelFilter::Off);
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(timeout)
        .connect_with(opts)
        .await
        .context("unable to connect to Postgres")?;
    let row = sqlx::query("select version()")
        .fetch_one(&pool)
        .await
        .context("Postgres validation query failed")?;
    let version: String = row.try_get(0).unwrap_or_default();
    pool.close().await;
    Ok(format!(
        "Connection successful — {}",
        clip_chars(version.trim(), 160)
    ))
}

async fn test_mysql_connection(source: &DatabaseSourceConfig) -> Result<String> {
    let url = build_sql_connection_url(source, 3306, "mysql")?;
    let timeout = Duration::from_millis(source.default_timeout_ms.max(1_000));
    let mut opts: sqlx::mysql::MySqlConnectOptions = url
        .parse()
        .context("invalid MySQL connection string")?;
    opts = match source.fields.ssl_mode {
        DatabaseSslMode::Disabled => opts.ssl_mode(sqlx::mysql::MySqlSslMode::Disabled),
        DatabaseSslMode::Required => opts.ssl_mode(sqlx::mysql::MySqlSslMode::Required),
        DatabaseSslMode::Strict => opts.ssl_mode(sqlx::mysql::MySqlSslMode::VerifyIdentity),
    };
    opts = opts.log_statements(tracing::log::LevelFilter::Off);
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(timeout)
        .connect_with(opts)
        .await
        .context("unable to connect to MySQL")?;
    let row = sqlx::query("select version()")
        .fetch_one(&pool)
        .await
        .context("MySQL validation query failed")?;
    let version: String = row.try_get(0).unwrap_or_default();
    pool.close().await;
    Ok(format!(
        "Connection successful — {}",
        clip_chars(version.trim(), 160)
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseActivityOperation {
    #[serde(rename = "read")]
    Read,
    #[serde(rename = "write")]
    Write,
    #[serde(rename = "ddl")]
    Ddl,
    #[serde(rename = "destructive")]
    Destructive,
    #[serde(rename = "unknown")]
    Unknown,
}

impl Default for DatabaseActivityOperation {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseActivityStatus {
    #[serde(rename = "success")]
    Ok,
    #[serde(rename = "error")]
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseActivityEntry {
    #[serde(default)]
    pub id: String,
    pub source_id: String,
    pub source_name: String,
    pub engine: DatabaseSourceEngine,
    #[serde(default)]
    pub operation: DatabaseActivityOperation,
    #[serde(default)]
    pub query_preview: String,
    pub status: DatabaseActivityStatus,
    pub timestamp_ms: i64,
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows_returned: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows_affected: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl DatabaseActivityEntry {
    pub fn from_query(
        source: &DatabaseSourceConfig,
        query: &str,
        status: DatabaseActivityStatus,
        timestamp_ms: i64,
        duration_ms: u64,
        rows_returned: Option<u64>,
        rows_affected: Option<u64>,
        error: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            source_id: source.id.clone(),
            source_name: source.name.clone(),
            engine: source.engine,
            operation: classify_sql_operation(query),
            query_preview: sanitize_query_preview(query),
            status,
            timestamp_ms,
            duration_ms,
            rows_returned,
            rows_affected,
            error: error.map(|value| source.redacted_error_message(value)),
        }
        .normalized()
    }

    pub fn normalized(mut self) -> Self {
        self.id = self.id.trim().to_string();
        if self.id.is_empty() {
            self.id = Uuid::new_v4().to_string();
        }
        self.source_id = self.source_id.trim().to_string();
        self.source_name = self.source_name.trim().to_string();
        self.query_preview = clip_chars(self.query_preview.trim(), ACTIVITY_PREVIEW_LIMIT);
        self.error = self
            .error
            .map(|value| clip_chars(value.trim(), STATUS_MESSAGE_LIMIT))
            .filter(|value| !value.is_empty());
        self
    }
}

pub fn sanitize_query_preview(query: &str) -> String {
    clip_chars(collapse_whitespace(&mask_sql_literals(query)).trim(), ACTIVITY_PREVIEW_LIMIT)
}

pub fn classify_sql_operation(query: &str) -> DatabaseActivityOperation {
    let masked = mask_sql_literals(query).to_ascii_lowercase();
    let normalized = collapse_whitespace(&masked);
    let first = normalized.split_whitespace().next().unwrap_or_default();
    match first {
        "select" | "with" | "show" | "explain" | "describe" | "pragma" => {
            DatabaseActivityOperation::Read
        }
        "insert" | "update" | "merge" | "replace" | "call" => DatabaseActivityOperation::Write,
        "create" => DatabaseActivityOperation::Ddl,
        "alter" | "drop" | "truncate" => DatabaseActivityOperation::Destructive,
        "delete" => {
            if normalized.contains(" where ") {
                DatabaseActivityOperation::Write
            } else {
                DatabaseActivityOperation::Destructive
            }
        }
        _ => DatabaseActivityOperation::Unknown,
    }
}

fn mask_sql_literals(input: &str) -> String {
    let mut out = String::with_capacity(input.len().min(ACTIVITY_PREVIEW_LIMIT));
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\'' | '"' | '`' => {
                out.push('?');
                let quote = ch;
                while let Some(next) = chars.next() {
                    if next == quote {
                        if quote == '\'' && chars.peek() == Some(&'\'') {
                            let _ = chars.next();
                            continue;
                        }
                        break;
                    }
                    if next == '\\' {
                        let _ = chars.next();
                    }
                }
            }
            '-' if chars.peek() == Some(&'-') => {
                let _ = chars.next();
                out.push(' ');
                for next in chars.by_ref() {
                    if next == '\n' {
                        out.push(' ');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                let _ = chars.next();
                out.push(' ');
                let mut previous = '\0';
                for next in chars.by_ref() {
                    if previous == '*' && next == '/' {
                        break;
                    }
                    previous = next;
                }
            }
            '$' => {
                let mut tag = String::new();
                while let Some(next) = chars.peek().copied() {
                    if next == '$' {
                        let _ = chars.next();
                        break;
                    }
                    if next.is_ascii_alphanumeric() || next == '_' {
                        tag.push(next);
                        let _ = chars.next();
                    } else {
                        out.push('$');
                        out.push_str(&tag);
                        tag.clear();
                        break;
                    }
                }
                if tag.is_empty() {
                    continue;
                }
                out.push('?');
                let end = format!("${tag}$");
                let mut window = String::new();
                for next in chars.by_ref() {
                    window.push(next);
                    if window.ends_with(&end) {
                        break;
                    }
                    if window.len() > end.len() {
                        window.remove(0);
                    }
                }
            }
            _ => out.push(ch),
        }
    }
    out
}

fn collapse_whitespace(input: &str) -> String {
    let mut out = String::new();
    let mut last_was_space = true;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                out.push(' ');
            }
            last_was_space = true;
        } else {
            out.push(ch);
            last_was_space = false;
        }
    }
    out.trim().to_string()
}

fn clip_chars(input: &str, limit: usize) -> String {
    if input.chars().count() <= limit {
        return input.to_string();
    }
    let mut clipped = input.chars().take(limit.saturating_sub(1)).collect::<String>();
    clipped.push('…');
    clipped
}

fn default_true() -> bool {
    true
}

fn default_row_limit() -> u32 {
    DEFAULT_ROW_LIMIT
}

fn default_timeout_ms() -> u64 {
    DEFAULT_TIMEOUT_MS
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_preview_masks_literals_and_comments() {
        let preview = sanitize_query_preview(
            "select * from users where email = 'a@example.com' -- api key: secret\n and token = \"abc\"",
        );
        assert_eq!(preview, "select * from users where email = ? and token = ?");
    }

    #[test]
    fn destructive_sql_is_classified_without_string_literals() {
        assert_eq!(
            classify_sql_operation("select 'drop table users'"),
            DatabaseActivityOperation::Read
        );
        assert_eq!(
            classify_sql_operation("delete from users"),
            DatabaseActivityOperation::Destructive
        );
        assert_eq!(
            classify_sql_operation("delete from users where id = 1"),
            DatabaseActivityOperation::Write
        );
        assert_eq!(
            classify_sql_operation("alter table users add column note text"),
            DatabaseActivityOperation::Destructive
        );
    }

    #[test]
    fn enabled_summaries_do_not_include_secrets() {
        let settings = DatabaseSettings {
            sources: vec![DatabaseSourceConfig {
                id: "one".into(),
                name: "prod".into(),
                connection_string: "postgres://user:secret@example/db".into(),
                enabled: true,
                ..DatabaseSourceConfig::default()
            }],
        }
        .normalized_for_save()
        .expect("valid settings");

        let json = serde_json::to_string(&settings.enabled_summaries()).unwrap();
        assert!(!json.contains("secret"));
        assert!(!json.contains("postgres://"));
        assert!(json.contains("prod"));
    }

    #[test]
    fn build_sql_connection_url_from_separate_fields_percent_encodes_password() {
        let mut source = DatabaseSourceConfig::default();
        source.engine = DatabaseSourceEngine::Postgres;
        source.credential_mode = DatabaseCredentialMode::Fields;
        source.fields = DatabaseConnectionFields {
            host: "db.example.com".into(),
            port: Some(6432),
            user: "claake".into(),
            password: "p@ss/word:!".into(),
            database: "claakecode".into(),
            ssl_mode: DatabaseSslMode::Required,
            ssl_certificate: String::new(),
        };
        let url = build_sql_connection_url(&source, 5432, "postgresql").unwrap();
        assert!(url.starts_with("postgresql://claake:"));
        assert!(url.contains("@db.example.com:6432/claakecode"));
        assert!(!url.contains("p@ss/word:!"));
        assert!(url.contains("p%40ss%2Fword%3A%21"));
    }

    #[test]
    fn build_sql_connection_url_uses_connection_string_when_provided() {
        let mut source = DatabaseSourceConfig::default();
        source.engine = DatabaseSourceEngine::Postgres;
        source.credential_mode = DatabaseCredentialMode::ConnectionString;
        source.connection_string = "postgres://u:p@h/db".into();
        let url = build_sql_connection_url(&source, 5432, "postgresql").unwrap();
        assert_eq!(url, "postgres://u:p@h/db");
    }

    #[test]
    fn redacted_error_message_removes_known_secrets() {
        let mut source = DatabaseSourceConfig::default();
        source.engine = DatabaseSourceEngine::Postgres;
        source.credential_mode = DatabaseCredentialMode::Fields;
        source.fields.password = "super-secret-password".into();
        let redacted = source.redacted_error_message(
            "error connecting using super-secret-password to db".to_string(),
        );
        assert!(!redacted.contains("super-secret-password"));
    }
}
