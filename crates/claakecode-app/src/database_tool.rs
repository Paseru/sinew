use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail, Context, Result};
use claakecode_core::ToolDescriptor;
use reqwest::{header::ACCEPT, Method, Url};
use rusqlite::{
    hooks::{AuthAction, AuthContext, Authorization},
    types::ValueRef,
    Connection, OpenFlags,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    database::{
        classify_sql_operation, sanitize_query_preview, DatabaseActivityEntry,
        DatabaseActivityOperation, DatabaseActivityStatus, DatabaseSourceConfig,
        DatabaseSourceEngine,
    },
    store::AppStore,
    tool_run::ToolRunResult,
};

const MAX_ROW_LIMIT: u32 = 10_000;
const MIN_TIMEOUT_MS: u64 = 1_000;
const MAX_TIMEOUT_MS: u64 = 300_000;
const SCHEMA_OBJECT_LIMIT: usize = 200;

pub const DATABASE_LIST_SOURCES_TOOL: &str = "database_list_sources";
pub const DATABASE_DESCRIBE_SCHEMA_TOOL: &str = "database_describe_schema";
pub const DATABASE_EXECUTE_QUERY_TOOL: &str = "database_execute_query";

#[derive(Clone)]
pub struct DatabaseTool {
    store: AppStore,
}

impl DatabaseTool {
    pub fn new(store: AppStore) -> Self {
        Self { store }
    }

    pub fn descriptors_static() -> Vec<ToolDescriptor> {
        vec![
            list_sources_descriptor(),
            describe_schema_descriptor(),
            execute_query_descriptor(),
        ]
    }

    pub fn descriptors(&self) -> Vec<ToolDescriptor> {
        Self::descriptors_static()
    }

    pub async fn run(
        &self,
        name: &str,
        input: Value,
        question_available: bool,
    ) -> Option<ToolRunResult> {
        match name {
            DATABASE_LIST_SOURCES_TOOL => Some(self.run_list_sources(input).await),
            DATABASE_DESCRIBE_SCHEMA_TOOL => Some(self.run_describe_schema(input).await),
            DATABASE_EXECUTE_QUERY_TOOL => {
                Some(self.run_execute_query(input, question_available).await)
            }
            _ => None,
        }
    }

    async fn run_list_sources(&self, input: Value) -> ToolRunResult {
        let parsed: ListDatabaseSourcesInput = match serde_json::from_value(input) {
            Ok(value) => value,
            Err(err) => {
                return ToolRunResult::err(
                    format!("invalid database_list_sources input: {err}"),
                    Vec::new(),
                )
            }
        };
        let settings = match self.store.load_database_settings() {
            Ok(settings) => settings,
            Err(err) => {
                return ToolRunResult::err(
                    format!("unable to load database sources: {err}"),
                    Vec::new(),
                )
            }
        };
        let mut sources = settings.enabled_summaries();
        if parsed.include_disabled {
            sources = settings
                .sources
                .iter()
                .map(DatabaseSourceConfig::agent_summary)
                .collect();
        }
        let disabled_count = settings.sources.iter().filter(|source| !source.enabled).count();
        let content = json!({
            "sources": sources,
            "activeCount": settings.active_count(),
            "sourceCount": settings.sources.len(),
            "disabledCount": disabled_count,
        });
        ToolRunResult::ok(pretty_json(&content), Vec::new())
    }

    async fn run_describe_schema(&self, input: Value) -> ToolRunResult {
        let parsed: DescribeDatabaseSchemaInput = match serde_json::from_value(input) {
            Ok(value) => value,
            Err(err) => {
                return ToolRunResult::err(
                    format!("invalid database_describe_schema input: {err}"),
                    Vec::new(),
                )
            }
        };
        let source = match self.load_enabled_source(&parsed.source) {
            Ok(source) => source,
            Err(err) => return ToolRunResult::err(err, Vec::new()),
        };
        let started = Instant::now();
        let result = match source.engine {
            DatabaseSourceEngine::Sqlite => describe_sqlite_schema(
                &source,
                parsed.schema.as_deref(),
                parsed.table.as_deref(),
            ),
            DatabaseSourceEngine::SupabaseRest => {
                describe_supabase_rest_schema(
                    &source,
                    parsed.schema.as_deref(),
                    parsed.table.as_deref(),
                )
                .await
            }
            DatabaseSourceEngine::Postgres => {
                describe_postgres_schema(
                    &source,
                    parsed.schema.as_deref(),
                    parsed.table.as_deref(),
                )
                .await
            }
            DatabaseSourceEngine::Mysql => {
                describe_mysql_schema(
                    &source,
                    parsed.schema.as_deref(),
                    parsed.table.as_deref(),
                )
                .await
            }
            DatabaseSourceEngine::Mssql => Err(anyhow!(
                "Microsoft SQL Server schema introspection is not bundled in this build"
            )),
        };
        match result {
            Ok(mut value) => {
                insert_duration(&mut value, started.elapsed());
                ToolRunResult::ok(pretty_json(&value), Vec::new())
            }
            Err(err) => ToolRunResult::err(source.redacted_error_message(err.to_string()), Vec::new()),
        }
    }

    async fn run_execute_query(&self, input: Value, question_available: bool) -> ToolRunResult {
        let parsed: ExecuteDatabaseQueryInput = match serde_json::from_value(input) {
            Ok(value) => value,
            Err(err) => {
                return ToolRunResult::err(
                    format!("invalid database_execute_query input: {err}"),
                    Vec::new(),
                )
            }
        };
        let query = parsed.query.trim();
        if query.is_empty() {
            return ToolRunResult::err("query is required", Vec::new());
        }
        let source = match self.load_enabled_source(&parsed.source) {
            Ok(source) => source,
            Err(err) => return ToolRunResult::err(err, Vec::new()),
        };
        let operation = classify_sql_operation(query);
        if source.read_only && !matches!(operation, DatabaseActivityOperation::Read) {
            return ToolRunResult::err(
                format!(
                    "database source `{}` is read-only; refusing {:?} operation",
                    source.name, operation
                ),
                Vec::new(),
            );
        }
        if source.require_confirmation_for_destructive
            && is_confirmation_required(operation)
            && !parsed.confirmed
        {
            return ToolRunResult::err(
                confirmation_required_message(&source, query, operation, question_available),
                Vec::new(),
            );
        }

        let started = Instant::now();
        let timestamp_ms = now_ms();
        let result = match source.engine {
            DatabaseSourceEngine::Sqlite => execute_sqlite_query(&source, query, parsed.row_limit),
            DatabaseSourceEngine::SupabaseRest => execute_supabase_rest_request(&source, &parsed).await,
            DatabaseSourceEngine::Postgres => {
                execute_postgres_query(&source, query, parsed.row_limit).await
            }
            DatabaseSourceEngine::Mysql => {
                execute_mysql_query(&source, query, parsed.row_limit).await
            }
            DatabaseSourceEngine::Mssql => Err(anyhow!(
                "Microsoft SQL Server query execution is not bundled in this build"
            )),
        };
        let duration_ms = started.elapsed().as_millis() as u64;
        match result {
            Ok(output) => {
                let activity = DatabaseActivityEntry::from_query(
                    &source,
                    query,
                    DatabaseActivityStatus::Ok,
                    timestamp_ms,
                    duration_ms,
                    output.rows_returned(),
                    output.rows_affected,
                    None,
                );
                if let Err(err) = self.store.append_database_source_activity(&activity) {
                    return ToolRunResult::err(
                        format!("query succeeded but activity logging failed: {err}"),
                        Vec::new(),
                    );
                }
                ToolRunResult::ok(pretty_json(&output.into_json(duration_ms)), Vec::new())
            }
            Err(err) => {
                let redacted = source.redacted_error_message(err.to_string());
                let activity = DatabaseActivityEntry::from_query(
                    &source,
                    query,
                    DatabaseActivityStatus::Error,
                    timestamp_ms,
                    duration_ms,
                    None,
                    None,
                    Some(redacted.clone()),
                );
                let _ = self.store.append_database_source_activity(&activity);
                ToolRunResult::err(redacted, Vec::new())
            }
        }
    }

    fn load_enabled_source(
        &self,
        source_name: &str,
    ) -> std::result::Result<DatabaseSourceConfig, String> {
        let source_name = source_name.trim();
        if source_name.is_empty() {
            return Err("source is required".to_string());
        }
        let settings = self
            .store
            .load_database_settings()
            .map_err(|err| format!("unable to load database sources: {err}"))?;
        if let Some(source) = settings.find_enabled_source(source_name) {
            return Ok(source);
        }
        if settings.sources.iter().any(|source| source.name == source_name) {
            Err(format!("database source `{source_name}` is disabled"))
        } else {
            Err(format!("database source `{source_name}` does not exist"))
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListDatabaseSourcesInput {
    #[serde(default)]
    include_disabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DescribeDatabaseSchemaInput {
    source: String,
    #[serde(default)]
    schema: Option<String>,
    #[serde(default)]
    table: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecuteDatabaseQueryInput {
    source: String,
    query: String,
    #[serde(default)]
    row_limit: Option<u32>,
    #[serde(default)]
    confirmed: bool,
    #[serde(default)]
    supabase: Option<SupabaseRestExecutionInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SupabaseRestExecutionInput {
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    body: Option<Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QueryExecutionOutput {
    source: String,
    engine: String,
    operation: DatabaseActivityOperation,
    columns: Vec<String>,
    rows: Vec<BTreeMap<String, Value>>,
    row_count: u64,
    rows_affected: Option<u64>,
    truncated: bool,
}

impl QueryExecutionOutput {
    fn rows_returned(&self) -> Option<u64> {
        Some(self.row_count)
    }

    fn into_json(self, duration_ms: u64) -> Value {
        json!({
            "source": self.source,
            "engine": self.engine,
            "operation": self.operation,
            "columns": self.columns,
            "rows": self.rows,
            "rowCount": self.row_count,
            "rowsAffected": self.rows_affected,
            "truncated": self.truncated,
            "durationMs": duration_ms,
        })
    }
}

fn list_sources_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        name: DATABASE_LIST_SOURCES_TOOL.into(),
        description: "List configured database sources that are enabled for the agent. Returns source names, engine IDs, defaults, read-only/destructive-confirmation flags, and last connection status; never returns credentials.".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "includeDisabled": {
                    "type": "boolean",
                    "description": "Optional diagnostic flag. Defaults to false; disabled sources cannot be used by other database tools."
                }
            },
            "additionalProperties": false
        }),
    }
}

fn describe_schema_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        name: DATABASE_DESCRIBE_SCHEMA_TOOL.into(),
        description: "Describe tables, views, columns, primary keys, foreign keys, and indexes for a configured database source. You may target a schema or table. Large schemas are truncated with a flag. SQLite is introspected locally and Supabase REST returns PostgREST/OpenAPI metadata when available; other direct SQL engines return a clear driver-unavailable error until their driver is installed.".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "description": "Database source name from database_list_sources." },
                "schema": { "type": "string", "description": "Optional schema name. Defaults to the source default schema when applicable." },
                "table": { "type": "string", "description": "Optional table or view name to narrow introspection." }
            },
            "required": ["source"],
            "additionalProperties": false
        }),
    }
}

fn execute_query_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        name: DATABASE_EXECUTE_QUERY_TOOL.into(),
        description: "Execute a SQL query (or, for Supabase REST, a REST request) against a configured database source with row-limit, timeout, read-only, destructive-operation, and activity-log safeguards. Disabled or missing sources are refused. Destructive writes/DDL require explicit user confirmation unless disabled for that source; after the user confirms, call again with confirmed=true. Results never include credentials.".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "description": "Database source name from database_list_sources." },
                "query": { "type": "string", "description": "SQL text. For Supabase REST, use a concise operation description unless supabase.path is provided." },
                "rowLimit": { "type": "integer", "minimum": 1, "description": "Optional read row limit. The source default still caps this value." },
                "confirmed": { "type": "boolean", "description": "Set true only after explicit user confirmation for destructive operations." },
                "supabase": {
                    "type": "object",
                    "description": "Optional Supabase REST request details.",
                    "properties": {
                        "method": { "type": "string", "enum": ["GET", "POST", "PATCH", "DELETE", "PUT"] },
                        "path": { "type": "string", "description": "REST path relative to /rest/v1/ or rpc/<function>." },
                        "body": { "description": "Optional JSON request body." }
                    },
                    "additionalProperties": false
                }
            },
            "required": ["source", "query"],
            "additionalProperties": false
        }),
    }
}

fn describe_sqlite_schema(
    source: &DatabaseSourceConfig,
    _schema: Option<&str>,
    table_filter: Option<&str>,
) -> Result<Value> {
    let conn = open_sqlite_connection(source, true)?;
    let mut statement = conn.prepare(
        "select name, type, sql from sqlite_master where type in ('table','view') and name not like 'sqlite_%' order by type, name",
    )?;
    let mut objects = Vec::new();
    let mut rows = statement.query([])?;
    let requested_table = table_filter.map(str::trim).filter(|value| !value.is_empty());
    let mut truncated = false;
    while let Some(row) = rows.next()? {
        let name: String = row.get(0)?;
        if requested_table.is_some_and(|wanted| wanted != name) {
            continue;
        }
        if objects.len() >= SCHEMA_OBJECT_LIMIT {
            truncated = true;
            break;
        }
        let kind: String = row.get(1)?;
        let sql: Option<String> = row.get(2)?;
        objects.push(json!({
            "name": name,
            "kind": kind,
            "columns": sqlite_columns(&conn, &name)?,
            "primaryKeys": sqlite_primary_keys(&conn, &name)?,
            "foreignKeys": sqlite_foreign_keys(&conn, &name)?,
            "indexes": sqlite_indexes(&conn, &name)?,
            "definition": sql.unwrap_or_default(),
        }));
    }
    Ok(json!({
        "source": source.name,
        "engine": source.engine.agent_id(),
        "schemas": [{ "name": "main", "objects": objects }],
        "truncated": truncated,
    }))
}

async fn describe_supabase_rest_schema(
    source: &DatabaseSourceConfig,
    schema: Option<&str>,
    table: Option<&str>,
) -> Result<Value> {
    let api_key = source
        .supabase_rest
        .selected_key()
        .ok_or_else(|| anyhow!("Supabase anon key or service role key is required"))?;
    let base = Url::parse(source.supabase_rest.project_url.trim())
        .context("Supabase project URL is invalid")?;
    let url = base
        .join("rest/v1/")
        .context("unable to build Supabase REST endpoint")?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(source.default_timeout_ms))
        .build()
        .context("unable to build Supabase REST client")?;
    let response = client
        .get(url)
        .header("apikey", api_key)
        .bearer_auth(api_key)
        .header(ACCEPT, "application/openapi+json, application/json")
        .send()
        .await
        .context("Supabase REST schema request failed")?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!(
            "Supabase REST schema request failed ({status}): {}",
            truncate_chars(text.trim(), 1_000)
        );
    }
    let parsed = serde_json::from_str::<Value>(&text)
        .unwrap_or_else(|_| json!({ "raw": truncate_chars(text.trim(), 10_000) }));
    Ok(json!({
        "source": source.name,
        "engine": source.engine.agent_id(),
        "schema": schema.unwrap_or_else(|| source.default_schema.as_str()),
        "table": table,
        "postgrest": parsed,
        "truncated": false,
    }))
}

fn execute_sqlite_query(
    source: &DatabaseSourceConfig,
    query: &str,
    requested_row_limit: Option<u32>,
) -> Result<QueryExecutionOutput> {
    let conn = open_sqlite_connection(source, source.read_only)?;
    install_sqlite_timeout(&conn, source.default_timeout_ms);
    if source.read_only {
        install_sqlite_read_only_authorizer(&conn);
    }
    let mut statement = conn.prepare(query).context("unable to prepare SQLite query")?;
    let is_read = statement.readonly() && statement.column_count() > 0;
    if source.read_only && !is_read {
        bail!("database source is read-only; refusing non-read SQLite statement");
    }
    if is_read {
        let source_limit = source.default_row_limit.max(1);
        let row_limit = requested_row_limit
            .unwrap_or(source_limit)
            .clamp(1, source_limit)
            .min(MAX_ROW_LIMIT);
        let columns = statement
            .column_names()
            .into_iter()
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let mut rows = statement.query([])?;
        let mut output_rows = Vec::new();
        let mut seen = 0u64;
        let mut truncated = false;
        while let Some(row) = rows.next()? {
            seen += 1;
            if seen > row_limit as u64 {
                truncated = true;
                break;
            }
            let mut object = BTreeMap::new();
            for (idx, column) in columns.iter().enumerate() {
                object.insert(column.clone(), sqlite_value_to_json(row.get_ref(idx)?));
            }
            output_rows.push(object);
        }
        Ok(QueryExecutionOutput {
            source: source.name.clone(),
            engine: source.engine.agent_id().to_string(),
            operation: classify_sql_operation(query),
            columns,
            row_count: output_rows.len() as u64,
            rows: output_rows,
            rows_affected: None,
            truncated,
        })
    } else {
        let rows_affected = statement.execute([])? as u64;
        Ok(QueryExecutionOutput {
            source: source.name.clone(),
            engine: source.engine.agent_id().to_string(),
            operation: classify_sql_operation(query),
            columns: Vec::new(),
            rows: Vec::new(),
            row_count: 0,
            rows_affected: Some(rows_affected),
            truncated: false,
        })
    }
}

async fn execute_supabase_rest_request(
    source: &DatabaseSourceConfig,
    input: &ExecuteDatabaseQueryInput,
) -> Result<QueryExecutionOutput> {
    let api_key = source
        .supabase_rest
        .selected_key()
        .ok_or_else(|| anyhow!("Supabase anon key or service role key is required"))?;
    let request = input.supabase.as_ref().ok_or_else(|| {
        anyhow!("Supabase REST execution requires supabase.path with an explicit REST path")
    })?;
    let method = request
        .method
        .as_deref()
        .unwrap_or("GET")
        .trim()
        .to_ascii_uppercase();
    let method = Method::from_bytes(method.as_bytes()).context("invalid Supabase REST method")?;
    let path = request
        .path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("Supabase REST path is required"))?;
    if path.starts_with("rpc/") && !source.supabase_rest.allow_rpc {
        bail!("Supabase RPC calls are disabled for this source");
    }
    let base = Url::parse(source.supabase_rest.project_url.trim())
        .context("Supabase project URL is invalid")?;
    let url = base
        .join(&format!("rest/v1/{}", path.trim_start_matches('/')))
        .context("unable to build Supabase REST endpoint")?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(source.default_timeout_ms))
        .build()
        .context("unable to build Supabase REST client")?;
    let mut builder = client
        .request(method.clone(), url)
        .header("apikey", api_key)
        .bearer_auth(api_key)
        .header(ACCEPT, "application/json");
    if let Some(body) = request.body.clone() {
        builder = builder.json(&body);
    }
    let response = builder.send().await.context("Supabase REST request failed")?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!(
            "Supabase REST request failed ({status}): {}",
            truncate_chars(text.trim(), 1_000)
        );
    }
    let value = serde_json::from_str::<Value>(&text).unwrap_or(Value::Null);
    let source_limit = source.default_row_limit.max(1) as usize;
    let row_limit = input
        .row_limit
        .unwrap_or(source.default_row_limit)
        .clamp(1, source.default_row_limit)
        .min(MAX_ROW_LIMIT) as usize;
    let (columns, rows, truncated) = json_value_to_rows(value, row_limit.min(source_limit));
    let rows_affected = if method == Method::GET {
        None
    } else {
        Some(rows.len() as u64)
    };
    Ok(QueryExecutionOutput {
        source: source.name.clone(),
        engine: source.engine.agent_id().to_string(),
        operation: classify_sql_operation(&input.query),
        row_count: rows.len() as u64,
        columns,
        rows,
        rows_affected,
        truncated,
    })
}

fn open_sqlite_connection(source: &DatabaseSourceConfig, force_read_only: bool) -> Result<Connection> {
    let path = source.sqlite.file_path.trim();
    if path.is_empty() {
        bail!("SQLite file path is required");
    }
    let flags = if force_read_only {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else if source.sqlite.create_if_missing {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    };
    Connection::open_with_flags(Path::new(path), flags)
        .with_context(|| "unable to open SQLite database")
}

fn install_sqlite_timeout(conn: &Connection, timeout_ms: u64) {
    let deadline =
        Instant::now() + Duration::from_millis(timeout_ms.clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS));
    conn.progress_handler(1_000, Some(move || Instant::now() >= deadline));
}

fn install_sqlite_read_only_authorizer(conn: &Connection) {
    conn.authorizer(Some(|ctx: AuthContext<'_>| match ctx.action {
        AuthAction::Select | AuthAction::Read { .. } | AuthAction::Function { .. } => {
            Authorization::Allow
        }
        AuthAction::Pragma {
            pragma_name,
            pragma_value,
        } => {
            if pragma_value.is_none() && is_safe_read_only_pragma(pragma_name) {
                Authorization::Allow
            } else {
                Authorization::Deny
            }
        }
        _ => Authorization::Deny,
    }));
}

fn is_safe_read_only_pragma(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "table_info"
            | "table_xinfo"
            | "index_list"
            | "index_info"
            | "index_xinfo"
            | "foreign_key_list"
            | "database_list"
            | "schema_version"
            | "user_version"
            | "integrity_check"
            | "quick_check"
    )
}

fn sqlite_columns(conn: &Connection, table: &str) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(&format!("pragma table_xinfo({})", quote_sqlite_literal(table)))?;
    let mut rows = stmt.query([])?;
    let mut columns = Vec::new();
    while let Some(row) = rows.next()? {
        let hidden: i64 = row.get(6).unwrap_or(0);
        columns.push(json!({
            "name": row.get::<_, String>(1)?,
            "type": row.get::<_, String>(2).unwrap_or_default(),
            "notNull": row.get::<_, i64>(3).unwrap_or(0) != 0,
            "defaultValue": row.get::<_, Option<String>>(4).unwrap_or(None),
            "primaryKeyOrdinal": row.get::<_, i64>(5).unwrap_or(0),
            "hidden": hidden != 0,
        }));
    }
    Ok(columns)
}

fn sqlite_primary_keys(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("pragma table_info({})", quote_sqlite_literal(table)))?;
    let mapped = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(1)?,
            row.get::<_, i64>(5).unwrap_or(0),
        ))
    })?;
    let mut keys = mapped
        .filter_map(|row| row.ok())
        .filter(|(_, pk)| *pk > 0)
        .collect::<Vec<_>>();
    keys.sort_by_key(|(_, pk)| *pk);
    Ok(keys.into_iter().map(|(name, _)| name).collect())
}

fn sqlite_foreign_keys(conn: &Connection, table: &str) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(&format!(
        "pragma foreign_key_list({})",
        quote_sqlite_literal(table)
    ))?;
    let mut rows = stmt.query([])?;
    let mut keys = Vec::new();
    while let Some(row) = rows.next()? {
        keys.push(json!({
            "id": row.get::<_, i64>(0).unwrap_or(0),
            "seq": row.get::<_, i64>(1).unwrap_or(0),
            "table": row.get::<_, String>(2).unwrap_or_default(),
            "from": row.get::<_, String>(3).unwrap_or_default(),
            "to": row.get::<_, String>(4).unwrap_or_default(),
            "onUpdate": row.get::<_, String>(5).unwrap_or_default(),
            "onDelete": row.get::<_, String>(6).unwrap_or_default(),
        }));
    }
    Ok(keys)
}

fn sqlite_indexes(conn: &Connection, table: &str) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(&format!("pragma index_list({})", quote_sqlite_literal(table)))?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2).unwrap_or(0) != 0,
            row.get::<_, String>(3).unwrap_or_default(),
        ))
    })?;
    let mut indexes = Vec::new();
    for row in rows {
        let (name, unique, origin) = row?;
        indexes.push(json!({
            "name": name,
            "unique": unique,
            "origin": origin,
            "columns": sqlite_index_columns(conn, &name)?,
        }));
    }
    Ok(indexes)
}

fn sqlite_index_columns(conn: &Connection, index: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("pragma index_info({})", quote_sqlite_literal(index)))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(2))?;
    let mut columns = Vec::new();
    for row in rows {
        columns.push(row?);
    }
    Ok(columns)
}

fn quote_sqlite_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn sqlite_value_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(value) => json!(value),
        ValueRef::Real(value) => json!(value),
        ValueRef::Text(value) => String::from_utf8_lossy(value).to_string().into(),
        ValueRef::Blob(value) => format!("[blob:{} bytes]", value.len()).into(),
    }
}

fn json_value_to_rows(
    value: Value,
    row_limit: usize,
) -> (Vec<String>, Vec<BTreeMap<String, Value>>, bool) {
    let values = match value {
        Value::Array(values) => values,
        Value::Null => Vec::new(),
        other => vec![other],
    };
    let truncated = values.len() > row_limit;
    let mut columns = Vec::<String>::new();
    let mut rows = Vec::new();
    for value in values.into_iter().take(row_limit) {
        let mut row = BTreeMap::new();
        match value {
            Value::Object(map) => {
                for (key, value) in map {
                    if !columns.contains(&key) {
                        columns.push(key.clone());
                    }
                    row.insert(key, value);
                }
            }
            other => {
                let key = "value".to_string();
                if columns.is_empty() {
                    columns.push(key.clone());
                }
                row.insert(key, other);
            }
        }
        rows.push(row);
    }
    (columns, rows, truncated)
}

fn insert_duration(value: &mut Value, duration: Duration) {
    if let Value::Object(map) = value {
        map.insert("durationMs".into(), json!(duration.as_millis() as u64));
    }
}

fn is_confirmation_required(operation: DatabaseActivityOperation) -> bool {
    matches!(
        operation,
        DatabaseActivityOperation::Write
            | DatabaseActivityOperation::Ddl
            | DatabaseActivityOperation::Destructive
            | DatabaseActivityOperation::Unknown
    )
}

fn confirmation_required_message(
    source: &DatabaseSourceConfig,
    query: &str,
    operation: DatabaseActivityOperation,
    question_available: bool,
) -> String {
    let preview = sanitize_query_preview(query);
    let instruction = if question_available {
        "Ask the user for explicit confirmation in the conversation before executing. If they approve, call database_execute_query again with confirmed=true."
    } else {
        "This context cannot ask the user for confirmation, so the operation was refused. Ask the main agent/user to perform it explicitly."
    };
    format!(
        "confirmation required for database source `{}` ({}) {:?} operation. Query preview: `{}`. {}",
        source.name,
        source.engine.agent_id(),
        operation,
        preview,
        instruction
    )
}

fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn truncate_chars(input: &str, limit: usize) -> String {
    if input.chars().count() <= limit {
        return input.to_string();
    }
    let mut clipped = input
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>();
    clipped.push('…');
    clipped
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_required_for_destructive_operations() {
        assert!(is_confirmation_required(DatabaseActivityOperation::Destructive));
        assert!(is_confirmation_required(DatabaseActivityOperation::Ddl));
        assert!(!is_confirmation_required(DatabaseActivityOperation::Read));
    }

    #[test]
    fn sqlite_blob_values_are_summarized() {
        assert_eq!(sqlite_value_to_json(ValueRef::Blob(&[1, 2, 3])), "[blob:3 bytes]");
    }
}

// --- Postgres / MySQL implementations via sqlx ---

use sqlx::{
    mysql::{MySqlConnectOptions, MySqlPoolOptions, MySqlRow, MySqlSslMode},
    postgres::{PgConnectOptions, PgPoolOptions, PgRow, PgSslMode},
    Column, ConnectOptions, Row, TypeInfo,
};
use tokio::time::timeout as tokio_timeout;

use crate::database::{build_sql_connection_url, DatabaseSslMode};

fn pg_connect_options(source: &DatabaseSourceConfig) -> Result<PgConnectOptions> {
    let url = build_sql_connection_url(source, 5432, "postgresql")?;
    let mut opts: PgConnectOptions = url
        .parse()
        .context("invalid Postgres connection string")?;
    opts = opts.application_name("claakecode");
    opts = match source.fields.ssl_mode {
        DatabaseSslMode::Disabled => opts.ssl_mode(PgSslMode::Disable),
        DatabaseSslMode::Required => opts.ssl_mode(PgSslMode::Require),
        DatabaseSslMode::Strict => opts.ssl_mode(PgSslMode::VerifyFull),
    };
    opts = opts.log_statements(tracing::log::LevelFilter::Off);
    Ok(opts)
}

fn my_connect_options(source: &DatabaseSourceConfig) -> Result<MySqlConnectOptions> {
    let url = build_sql_connection_url(source, 3306, "mysql")?;
    let mut opts: MySqlConnectOptions = url
        .parse()
        .context("invalid MySQL connection string")?;
    opts = match source.fields.ssl_mode {
        DatabaseSslMode::Disabled => opts.ssl_mode(MySqlSslMode::Disabled),
        DatabaseSslMode::Required => opts.ssl_mode(MySqlSslMode::Required),
        DatabaseSslMode::Strict => opts.ssl_mode(MySqlSslMode::VerifyIdentity),
    };
    opts = opts.log_statements(tracing::log::LevelFilter::Off);
    Ok(opts)
}

fn source_timeout(source: &DatabaseSourceConfig) -> Duration {
    Duration::from_millis(source.default_timeout_ms.max(MIN_TIMEOUT_MS).min(MAX_TIMEOUT_MS))
}

fn source_row_limit(source: &DatabaseSourceConfig, requested: Option<u32>) -> u32 {
    let source_limit = source.default_row_limit.max(1);
    requested
        .unwrap_or(source_limit)
        .clamp(1, source_limit)
        .min(MAX_ROW_LIMIT)
}

async fn describe_postgres_schema(
    source: &DatabaseSourceConfig,
    schema: Option<&str>,
    table: Option<&str>,
) -> Result<Value> {
    let opts = pg_connect_options(source)?;
    let timeout = source_timeout(source);
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(timeout)
        .connect_with(opts)
        .await
        .context("unable to connect to Postgres")?;
    let schema_filter = schema
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&source.default_schema)
        .to_string();
    let table_filter = table.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    let tables_query = "
        SELECT n.nspname AS schema_name,
               c.relname AS table_name,
               CASE c.relkind WHEN 'r' THEN 'table' WHEN 'v' THEN 'view'
                              WHEN 'm' THEN 'materialized_view' ELSE c.relkind::text END AS kind
          FROM pg_class c
          JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE c.relkind IN ('r','v','m')
           AND ($1 = '' OR n.nspname = $1)
           AND ($2::text IS NULL OR c.relname = $2)
         ORDER BY n.nspname, c.relname
         LIMIT $3";
    let table_rows = tokio_timeout(
        timeout,
        sqlx::query(tables_query)
            .bind(&schema_filter)
            .bind(table_filter.as_deref())
            .bind(SCHEMA_OBJECT_LIMIT as i64 + 1)
            .fetch_all(&pool),
    )
    .await
    .context("Postgres schema query timed out")?
    .context("Postgres schema query failed")?;
    let truncated = table_rows.len() > SCHEMA_OBJECT_LIMIT;
    let tables = table_rows
        .into_iter()
        .take(SCHEMA_OBJECT_LIMIT)
        .collect::<Vec<_>>();
    let mut entries = Vec::new();
    for trow in &tables {
        let table_schema: String = trow.try_get(0).unwrap_or_default();
        let table_name: String = trow.try_get(1).unwrap_or_default();
        let kind: String = trow.try_get(2).unwrap_or_default();
        let columns = sqlx::query(
            "SELECT column_name, data_type, is_nullable, column_default, ordinal_position
               FROM information_schema.columns
              WHERE table_schema = $1 AND table_name = $2
              ORDER BY ordinal_position",
        )
        .bind(&table_schema)
        .bind(&table_name)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
        let columns_json = columns
            .into_iter()
            .map(|row| {
                json!({
                    "name": row.try_get::<String, _>(0).unwrap_or_default(),
                    "type": row.try_get::<String, _>(1).unwrap_or_default(),
                    "nullable": row.try_get::<String, _>(2).unwrap_or_default() == "YES",
                    "default": row.try_get::<Option<String>, _>(3).unwrap_or(None),
                    "ordinalPosition": row.try_get::<i32, _>(4).unwrap_or(0),
                })
            })
            .collect::<Vec<_>>();
        entries.push(json!({
            "schema": table_schema,
            "name": table_name,
            "kind": kind,
            "columns": columns_json,
        }));
    }
    pool.close().await;
    Ok(json!({
        "engine": source.engine.agent_id(),
        "source": source.name,
        "schema": schema_filter,
        "tables": entries,
        "truncated": truncated,
    }))
}

async fn describe_mysql_schema(
    source: &DatabaseSourceConfig,
    schema: Option<&str>,
    table: Option<&str>,
) -> Result<Value> {
    let opts = my_connect_options(source)?;
    let timeout = source_timeout(source);
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(timeout)
        .connect_with(opts)
        .await
        .context("unable to connect to MySQL")?;
    let database_default = source.fields.database.trim().to_string();
    let schema_filter = schema
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or(database_default);
    let table_filter = table.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    let tables = tokio_timeout(
        timeout,
        sqlx::query(
            "SELECT table_schema, table_name, table_type
               FROM information_schema.tables
              WHERE (? = '' OR table_schema = ?)
                AND (? IS NULL OR table_name = ?)
              ORDER BY table_schema, table_name
              LIMIT ?",
        )
        .bind(&schema_filter)
        .bind(&schema_filter)
        .bind(table_filter.as_deref())
        .bind(table_filter.as_deref())
        .bind((SCHEMA_OBJECT_LIMIT + 1) as i64)
        .fetch_all(&pool),
    )
    .await
    .context("MySQL schema query timed out")?
    .context("MySQL schema query failed")?;
    let truncated = tables.len() > SCHEMA_OBJECT_LIMIT;
    let tables = tables.into_iter().take(SCHEMA_OBJECT_LIMIT).collect::<Vec<_>>();
    let mut entries = Vec::new();
    for trow in &tables {
        let table_schema: String = trow.try_get(0).unwrap_or_default();
        let table_name: String = trow.try_get(1).unwrap_or_default();
        let kind: String = trow.try_get(2).unwrap_or_default();
        let columns = sqlx::query(
            "SELECT column_name, column_type, is_nullable, column_default, ordinal_position
               FROM information_schema.columns
              WHERE table_schema = ? AND table_name = ?
              ORDER BY ordinal_position",
        )
        .bind(&table_schema)
        .bind(&table_name)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
        let columns_json = columns
            .into_iter()
            .map(|row| {
                json!({
                    "name": row.try_get::<String, _>(0).unwrap_or_default(),
                    "type": row.try_get::<String, _>(1).unwrap_or_default(),
                    "nullable": row.try_get::<String, _>(2).unwrap_or_default().eq_ignore_ascii_case("YES"),
                    "default": row.try_get::<Option<String>, _>(3).unwrap_or(None),
                    "ordinalPosition": row.try_get::<i64, _>(4).unwrap_or(0),
                })
            })
            .collect::<Vec<_>>();
        entries.push(json!({
            "schema": table_schema,
            "name": table_name,
            "kind": kind,
            "columns": columns_json,
        }));
    }
    pool.close().await;
    Ok(json!({
        "engine": source.engine.agent_id(),
        "source": source.name,
        "schema": schema_filter,
        "tables": entries,
        "truncated": truncated,
    }))
}

async fn execute_postgres_query(
    source: &DatabaseSourceConfig,
    query: &str,
    requested_row_limit: Option<u32>,
) -> Result<QueryExecutionOutput> {
    let opts = pg_connect_options(source)?;
    let timeout = source_timeout(source);
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(timeout)
        .connect_with(opts)
        .await
        .context("unable to connect to Postgres")?;
    let operation = classify_sql_operation(query);
    let is_read = matches!(operation, DatabaseActivityOperation::Read);
    let row_limit = source_row_limit(source, requested_row_limit) as usize;
    let outcome = if is_read {
        let rows = tokio_timeout(timeout, sqlx::query(query).fetch_all(&pool))
            .await
            .context("Postgres query timed out")?
            .context("Postgres query failed")?;
        let truncated = rows.len() > row_limit;
        let rows = rows.into_iter().take(row_limit).collect::<Vec<_>>();
        let columns = rows
            .first()
            .map(|row| {
                row.columns()
                    .iter()
                    .map(|c| c.name().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let json_rows = rows.iter().map(pg_row_to_json).collect::<Vec<_>>();
        QueryExecutionOutput {
            source: source.name.clone(),
            engine: source.engine.agent_id().to_string(),
            operation,
            row_count: json_rows.len() as u64,
            columns,
            rows: json_rows,
            rows_affected: None,
            truncated,
        }
    } else {
        let result = tokio_timeout(timeout, sqlx::query(query).execute(&pool))
            .await
            .context("Postgres query timed out")?
            .context("Postgres query failed")?;
        QueryExecutionOutput {
            source: source.name.clone(),
            engine: source.engine.agent_id().to_string(),
            operation,
            columns: Vec::new(),
            rows: Vec::new(),
            row_count: 0,
            rows_affected: Some(result.rows_affected()),
            truncated: false,
        }
    };
    pool.close().await;
    Ok(outcome)
}

async fn execute_mysql_query(
    source: &DatabaseSourceConfig,
    query: &str,
    requested_row_limit: Option<u32>,
) -> Result<QueryExecutionOutput> {
    let opts = my_connect_options(source)?;
    let timeout = source_timeout(source);
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(timeout)
        .connect_with(opts)
        .await
        .context("unable to connect to MySQL")?;
    let operation = classify_sql_operation(query);
    let is_read = matches!(operation, DatabaseActivityOperation::Read);
    let row_limit = source_row_limit(source, requested_row_limit) as usize;
    let outcome = if is_read {
        let rows = tokio_timeout(timeout, sqlx::query(query).fetch_all(&pool))
            .await
            .context("MySQL query timed out")?
            .context("MySQL query failed")?;
        let truncated = rows.len() > row_limit;
        let rows = rows.into_iter().take(row_limit).collect::<Vec<_>>();
        let columns = rows
            .first()
            .map(|row| {
                row.columns()
                    .iter()
                    .map(|c| c.name().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let json_rows = rows.iter().map(mysql_row_to_json).collect::<Vec<_>>();
        QueryExecutionOutput {
            source: source.name.clone(),
            engine: source.engine.agent_id().to_string(),
            operation,
            row_count: json_rows.len() as u64,
            columns,
            rows: json_rows,
            rows_affected: None,
            truncated,
        }
    } else {
        let result = tokio_timeout(timeout, sqlx::query(query).execute(&pool))
            .await
            .context("MySQL query timed out")?
            .context("MySQL query failed")?;
        QueryExecutionOutput {
            source: source.name.clone(),
            engine: source.engine.agent_id().to_string(),
            operation,
            columns: Vec::new(),
            rows: Vec::new(),
            row_count: 0,
            rows_affected: Some(result.rows_affected()),
            truncated: false,
        }
    };
    pool.close().await;
    Ok(outcome)
}

fn pg_row_to_json(row: &PgRow) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for column in row.columns() {
        let name = column.name().to_string();
        let type_name = column.type_info().name().to_string();
        let value = pg_value_to_json(row, column.ordinal(), &type_name);
        out.insert(name, value);
    }
    out
}

fn pg_value_to_json(row: &PgRow, index: usize, type_name: &str) -> Value {
    match type_name {
        "BOOL" => row
            .try_get::<Option<bool>, _>(index)
            .ok()
            .flatten()
            .map(Value::Bool)
            .unwrap_or(Value::Null),
        "INT2" | "INT4" => row
            .try_get::<Option<i32>, _>(index)
            .ok()
            .flatten()
            .map(|v| Value::Number(v.into()))
            .unwrap_or(Value::Null),
        "INT8" => row
            .try_get::<Option<i64>, _>(index)
            .ok()
            .flatten()
            .map(|v| Value::Number(v.into()))
            .unwrap_or(Value::Null),
        "FLOAT4" => row
            .try_get::<Option<f32>, _>(index)
            .ok()
            .flatten()
            .and_then(|v| serde_json::Number::from_f64(v as f64).map(Value::Number))
            .unwrap_or(Value::Null),
        "FLOAT8" | "NUMERIC" => row
            .try_get::<Option<f64>, _>(index)
            .ok()
            .flatten()
            .and_then(|v| serde_json::Number::from_f64(v).map(Value::Number))
            .unwrap_or(Value::Null),
        "JSON" | "JSONB" => row
            .try_get::<Option<Value>, _>(index)
            .ok()
            .flatten()
            .unwrap_or(Value::Null),
        _ => row
            .try_get::<Option<String>, _>(index)
            .ok()
            .flatten()
            .map(Value::String)
            .unwrap_or_else(|| Value::String(format!("[{type_name}]"))),
    }
}

fn mysql_row_to_json(row: &MySqlRow) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for column in row.columns() {
        let name = column.name().to_string();
        let type_name = column.type_info().name().to_string();
        let value = mysql_value_to_json(row, column.ordinal(), &type_name);
        out.insert(name, value);
    }
    out
}

fn mysql_value_to_json(row: &MySqlRow, index: usize, type_name: &str) -> Value {
    let upper = type_name.to_ascii_uppercase();
    if upper.contains("INT") {
        if let Ok(Some(value)) = row.try_get::<Option<i64>, _>(index) {
            return Value::Number(value.into());
        }
    }
    if upper.contains("DECIMAL") || upper.contains("DOUBLE") || upper.contains("FLOAT") {
        if let Ok(Some(value)) = row.try_get::<Option<f64>, _>(index) {
            if let Some(num) = serde_json::Number::from_f64(value) {
                return Value::Number(num);
            }
        }
    }
    if upper.contains("BOOL") || upper == "TINYINT(1)" {
        if let Ok(Some(value)) = row.try_get::<Option<bool>, _>(index) {
            return Value::Bool(value);
        }
    }
    if upper.contains("JSON") {
        if let Ok(Some(value)) = row.try_get::<Option<Value>, _>(index) {
            return value;
        }
    }
    if let Ok(Some(value)) = row.try_get::<Option<String>, _>(index) {
        return Value::String(value);
    }
    Value::Null
}
