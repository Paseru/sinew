use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet},
    env,
    error::Error as StdError,
    ffi::OsStr,
    fmt, fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Stdio,
    sync::OnceLock,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    StatusCode, Url,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sinew_core::{ChatMessage, Part, ToolDescriptor};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::RwLock,
    time::timeout,
};
use tracing::warn;

use crate::tool_names;
use crate::tool_run::{ToolRunImage, ToolRunResult};

const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(45);
const CALL_TIMEOUT: Duration = Duration::from_secs(120);
const TOOL_OUTPUT_LIMIT: usize = 128 * 1024;
const TOOL_NAME_LIMIT: usize = 64;
const LOAD_MCP_TOOL_NAME: &str = tool_names::LOAD_MCP_TOOL;
const MCP_HTTP_ACCEPT: &str = "application/json, text/event-stream";
const MCP_SESSION_ID_HEADER: &str = "mcp-session-id";
pub const MCP_OAUTH_REDIRECT_PORT: u16 = 1458;
pub const MCP_OAUTH_CALLBACK_PATH: &str = "/mcp/oauth/callback";
const OAUTH_REDIRECT_URI: &str = "http://localhost:1458/mcp/oauth/callback";
const OAUTH_AUTH_FILE: &str = "mcp-auth.json";
const OAUTH_REFRESH_SKEW_MS: i64 = 60_000;
const OAUTH_CLIENT_NAME: &str = "Sinew";
const OAUTH_CLIENT_URI: &str = "https://github.com/Paseru/sinew";
const OAUTH_CLIENT_ID: &str = "sinew-mcp-client";
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpSettings {
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: Vec<McpEnvVar>,
    #[serde(default)]
    pub headers: Vec<McpHeader>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<McpAuthConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpEnvVar {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpHeader {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpAuthConfig {
    #[serde(default = "default_auth_type", rename = "type")]
    pub auth_type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub token: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolInfo {
    pub server_id: String,
    pub server_name: String,
    pub name: String,
    pub tool_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerProbe {
    pub server_id: String,
    pub server_name: String,
    pub enabled: bool,
    pub ok: bool,
    pub tools: Vec<McpToolInfo>,
    pub transport: String,
    #[serde(default)]
    pub auth_required: bool,
    #[serde(default)]
    pub authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartMcpOAuthLoginOutput {
    pub login_id: String,
    pub auth_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpOAuthStatus {
    pub connected: bool,
    pub connection_state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpOAuthDiscovery {
    pub auth_required: bool,
    pub authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct McpOAuthLoginPlan {
    pub login_id: String,
    pub auth_url: String,
    pub server: McpServerConfig,
    pub metadata: OAuthServerMetadata,
    pub resource: String,
    pub pkce: PkceCodes,
    pub state: String,
    pub client_id: String,
}

#[derive(Debug, Clone)]
pub struct McpOAuthOutcome {
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StartMcpOAuthLoginResult {
    pub output: StartMcpOAuthLoginOutput,
    pub plan: McpOAuthLoginPlan,
}

#[derive(Debug, Clone)]
struct McpToolBinding {
    server: McpServerConfig,
    original_name: String,
    display_name: String,
    description: String,
    input_schema: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolLabel {
    pub server_name: String,
    pub tool_name: String,
}

#[derive(Debug)]
pub struct McpToolRegistry {
    settings: McpSettings,
    bindings: RwLock<HashMap<String, McpToolBinding>>,
    loaded: RwLock<HashSet<String>>,
}

impl McpToolRegistry {
    pub fn new(settings: McpSettings) -> Self {
        Self {
            settings,
            bindings: RwLock::new(HashMap::new()),
            loaded: RwLock::new(HashSet::new()),
        }
    }

    pub async fn refresh_catalog(&self, history: &[ChatMessage]) -> Vec<ToolDescriptor> {
        let mut next_bindings = HashMap::new();

        for server in enabled_servers(&self.settings) {
            let mut client = match McpClient::connect(server).await {
                Ok(client) => client,
                Err(err) => {
                    warn!("unable to connect MCP server {}: {err}", server.name);
                    continue;
                }
            };

            let tools = match client.list_tools().await {
                Ok(tools) => tools,
                Err(err) => {
                    warn!("unable to list MCP tools for {}: {err}", server.name);
                    continue;
                }
            };

            for tool in tools {
                let generated_name = unique_tool_name(server, &tool.name, &next_bindings);
                let display_name = mcp_tool_display_name(&tool);
                let description = mcp_tool_description(server, &tool);
                next_bindings.insert(
                    generated_name,
                    McpToolBinding {
                        server: server.clone(),
                        original_name: tool.name,
                        display_name,
                        description,
                        input_schema: normalize_input_schema(tool.input_schema),
                    },
                );
            }
        }

        let history_requests = history_loaded_mcp_tools(history);
        let mut loaded = self.loaded.read().await.clone();
        for request in history_requests {
            if let Ok(name) = resolve_mcp_tool(&next_bindings, &request) {
                loaded.insert(name);
            }
        }
        loaded.retain(|name| next_bindings.contains_key(name));

        *self.bindings.write().await = next_bindings;
        *self.loaded.write().await = loaded;
        self.descriptors().await
    }

    pub async fn descriptors(&self) -> Vec<ToolDescriptor> {
        let bindings = self.bindings.read().await;
        if bindings.is_empty() {
            return Vec::new();
        }

        let loaded = self.loaded.read().await;
        let mut descriptors = vec![load_mcp_tool_descriptor(&bindings)];
        let mut names = bindings.keys().cloned().collect::<Vec<_>>();
        names.sort();

        for name in names {
            if !loaded.contains(&name) {
                continue;
            }
            if let Some(binding) = bindings.get(&name) {
                descriptors.push(ToolDescriptor {
                    name,
                    description: binding.description.clone(),
                    input_schema: binding.input_schema.clone(),
                });
            }
        }

        descriptors
    }

    pub async fn run_tool(&self, name: &str, input: Value) -> Option<ToolRunResult> {
        if tool_names::is_tool_name(name, LOAD_MCP_TOOL_NAME) {
            return Some(self.load_tool(input).await);
        }

        let binding = self.bindings.read().await.get(name).cloned()?;
        if !self.loaded.read().await.contains(name) {
            return Some(ToolRunResult::err(
                format!("MCP tool `{name}` is not loaded yet. Use {LOAD_MCP_TOOL_NAME} first."),
                Vec::new(),
            ));
        }
        Some(call_mcp_tool(binding, input).await)
    }

    pub async fn tool_label(&self, name: &str) -> Option<McpToolLabel> {
        let binding = self.bindings.read().await.get(name).cloned()?;
        Some(McpToolLabel {
            server_name: binding.server.name,
            tool_name: binding.original_name,
        })
    }

    async fn load_tool(&self, input: Value) -> ToolRunResult {
        let request = match mcp_tool_request_from_input(&input) {
            Ok(request) => request,
            Err(err) => return ToolRunResult::err(err.to_string(), Vec::new()),
        };

        let bindings = self.bindings.read().await;
        let name = match resolve_mcp_tool(&bindings, &request) {
            Ok(name) => name,
            Err(err) => return ToolRunResult::err(err.to_string(), Vec::new()),
        };
        let Some(binding) = bindings.get(&name).cloned() else {
            return ToolRunResult::err(format!("MCP tool `{name}` is unavailable"), Vec::new());
        };
        drop(bindings);

        self.loaded.write().await.insert(name.clone());
        ToolRunResult::ok(
            format!(
                "Loaded {} / {}.\nTool name: `{}`\nUse this tool on the next step; its full description and input schema are now available.",
                display_mcp_server_name(&binding.server.name),
                binding.original_name,
                name
            ),
            Vec::new(),
        )
    }
}

#[derive(Debug, Clone)]
struct McpToolRequest {
    generated_name: Option<String>,
    server: Option<String>,
    tool: Option<String>,
}

fn load_mcp_tool_descriptor(bindings: &HashMap<String, McpToolBinding>) -> ToolDescriptor {
    let mut entries = bindings
        .values()
        .map(|binding| {
            format!(
                "- {} / {}",
                display_mcp_server_name(&binding.server.name),
                binding.original_name
            )
        })
        .collect::<Vec<_>>();
    entries.sort();

    ToolDescriptor {
        name: LOAD_MCP_TOOL_NAME.to_string(),
        description: format!(
            "Load one MCP tool before calling it. Available MCP tools:\n{}\nCall with the exact `server` and `tool` strings shown around `/`. Tools not loaded yet do not expose their full description or input schema.",
            entries.join("\n")
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "server": {
                    "type": "string",
                    "description": "MCP server name as shown before `/` in the catalog."
                },
                "tool": {
                    "type": "string",
                    "description": "MCP tool name as shown after `/` in the catalog."
                },
                "name": {
                    "type": "string",
                    "description": "Optional generated tool name if a previous load result provided one."
                }
            },
            "required": ["server", "tool"],
            "additionalProperties": false
        }),
    }
}

fn history_loaded_mcp_tools(history: &[ChatMessage]) -> Vec<McpToolRequest> {
    let mut requests = Vec::new();
    for message in history {
        for part in &message.parts {
            let Part::ToolCall { name, input, .. } = part else {
                continue;
            };

            if tool_names::is_tool_name(name, LOAD_MCP_TOOL_NAME) {
                if let Ok(request) = mcp_tool_request_from_input(input) {
                    requests.push(request);
                }
            } else if is_mcp_generated_name(name) {
                requests.push(McpToolRequest {
                    generated_name: Some(name.clone()),
                    server: None,
                    tool: None,
                });
            }
        }
    }
    requests
}

fn mcp_tool_request_from_input(input: &Value) -> Result<McpToolRequest> {
    let generated_name = input_string(input, &["name", "toolName", "tool_name"])
        .filter(|value| is_mcp_generated_name(value));
    if generated_name.is_some() {
        return Ok(McpToolRequest {
            generated_name,
            server: input_string(input, &["server", "serverName", "server_name"]),
            tool: input_string(input, &["tool", "toolName", "tool_name"]),
        });
    }

    let server = input_string(input, &["server", "serverName", "server_name", "mcp"]);
    let tool = input_string(input, &["tool", "toolName", "tool_name", "name"]);
    if server.is_none() || tool.is_none() {
        bail!("load_mcp_tool needs `server` and `tool` from the MCP catalog");
    }

    Ok(McpToolRequest {
        generated_name: None,
        server,
        tool,
    })
}

fn input_string(input: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| input.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn resolve_mcp_tool(
    bindings: &HashMap<String, McpToolBinding>,
    request: &McpToolRequest,
) -> Result<String> {
    if let Some(name) = request.generated_name.as_deref() {
        if bindings.contains_key(name) {
            return Ok(name.to_string());
        }
        bail!("MCP tool `{name}` is unavailable");
    }

    let server = request
        .server
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("load_mcp_tool needs `server` from the MCP catalog"))?;
    let tool = request
        .tool
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("load_mcp_tool needs `tool` from the MCP catalog"))?;

    let matches = bindings
        .iter()
        .filter(|(_, binding)| {
            mcp_server_matches(binding, server) && mcp_tool_matches(binding, tool)
        })
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();

    match matches.as_slice() {
        [name] => Ok(name.clone()),
        [] => bail!("No MCP tool found for `{server} / {tool}`"),
        _ => bail!("Several MCP tools match `{server} / {tool}`"),
    }
}

fn mcp_server_matches(binding: &McpToolBinding, value: &str) -> bool {
    loose_label_eq(&binding.server.name, value)
        || loose_label_eq(&display_mcp_server_name(&binding.server.name), value)
        || loose_label_eq(&binding.server.id, value)
}

fn mcp_tool_matches(binding: &McpToolBinding, value: &str) -> bool {
    loose_label_eq(&binding.original_name, value) || loose_label_eq(&binding.display_name, value)
}

fn loose_label_eq(left: &str, right: &str) -> bool {
    compact_label(left) == compact_label(right)
}

fn compact_label(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn is_mcp_generated_name(name: &str) -> bool {
    name.starts_with("mcp__")
}

pub async fn probe_mcp_servers(settings: &McpSettings) -> Vec<McpServerProbe> {
    let mut probes = Vec::new();
    let mut known_names: HashMap<String, McpToolBinding> = HashMap::new();

    for server in &settings.servers {
        let transport = mcp_transport_label(server).to_string();
        let authenticated = mcp_server_has_auth(server);
        if !server.enabled {
            probes.push(McpServerProbe {
                server_id: server.id.clone(),
                server_name: server.name.clone(),
                enabled: false,
                ok: true,
                tools: Vec::new(),
                transport,
                auth_required: false,
                authenticated,
                error: None,
            });
            continue;
        }

        let mut client = match McpClient::connect(server).await {
            Ok(client) => client,
            Err(err) => {
                let auth_required = err.downcast_ref::<McpAuthRequired>().is_some();
                probes.push(McpServerProbe {
                    server_id: server.id.clone(),
                    server_name: server.name.clone(),
                    enabled: true,
                    ok: false,
                    tools: Vec::new(),
                    transport,
                    auth_required,
                    authenticated,
                    error: Some(err.to_string()),
                });
                continue;
            }
        };

        match client.list_tools().await {
            Ok(tools) => {
                let mut infos = Vec::with_capacity(tools.len());
                for tool in tools {
                    let tool_name = unique_tool_name(server, &tool.name, &known_names);
                    let display_name = mcp_tool_display_name(&tool);
                    known_names.insert(
                        tool_name.clone(),
                        McpToolBinding {
                            server: server.clone(),
                            original_name: tool.name.clone(),
                            display_name,
                            description: mcp_tool_description(server, &tool),
                            input_schema: normalize_input_schema(tool.input_schema.clone()),
                        },
                    );
                    infos.push(McpToolInfo {
                        server_id: server.id.clone(),
                        server_name: server.name.clone(),
                        name: tool.name,
                        tool_name,
                        title: tool.title,
                        description: tool.description,
                    });
                }
                probes.push(McpServerProbe {
                    server_id: server.id.clone(),
                    server_name: server.name.clone(),
                    enabled: true,
                    ok: true,
                    tools: infos,
                    transport,
                    auth_required: false,
                    authenticated: mcp_server_has_auth(server),
                    error: None,
                });
            }
            Err(err) => {
                let auth_required = err.downcast_ref::<McpAuthRequired>().is_some();
                probes.push(McpServerProbe {
                    server_id: server.id.clone(),
                    server_name: server.name.clone(),
                    enabled: true,
                    ok: false,
                    tools: Vec::new(),
                    transport,
                    auth_required,
                    authenticated: mcp_server_has_auth(server),
                    error: Some(err.to_string()),
                })
            }
        }
    }

    probes
}

fn enabled_servers(settings: &McpSettings) -> impl Iterator<Item = &McpServerConfig> {
    settings
        .servers
        .iter()
        .filter(|server| server.enabled && mcp_server_has_transport(server))
}

fn mcp_server_has_transport(server: &McpServerConfig) -> bool {
    !server.command.trim().is_empty() || mcp_server_url(server).is_some()
}

fn mcp_server_url(server: &McpServerConfig) -> Option<&str> {
    server
        .url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn mcp_transport_label(server: &McpServerConfig) -> &'static str {
    if mcp_server_url(server).is_some() {
        "http"
    } else {
        "stdio"
    }
}

fn mcp_server_has_auth(server: &McpServerConfig) -> bool {
    bearer_token_from_server(server).is_some()
        || !server
            .headers
            .iter()
            .all(|header| header.key.trim().is_empty())
}

async fn call_mcp_tool(binding: McpToolBinding, input: Value) -> ToolRunResult {
    match call_mcp_tool_inner(binding, input).await {
        Ok(result) => result,
        Err(err) => ToolRunResult::err(format!("MCP tool failed: {err}"), Vec::new()),
    }
}

async fn call_mcp_tool_inner(binding: McpToolBinding, input: Value) -> Result<ToolRunResult> {
    let mut client = McpClient::connect_with_timeout(&binding.server, CALL_TIMEOUT).await?;
    let result = client.call_tool(&binding.original_name, input).await?;
    Ok(format_call_result(result))
}

fn format_call_result(result: McpCallToolResult) -> ToolRunResult {
    let mut text = Vec::new();
    let mut images = Vec::new();

    for block in result.content {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(value) = block.get("text").and_then(Value::as_str) {
                    text.push(value.to_string());
                }
            }
            Some("image") => {
                let data = block
                    .get("data")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let media_type = block
                    .get("mimeType")
                    .or_else(|| block.get("mime_type"))
                    .and_then(Value::as_str)
                    .unwrap_or("image/png")
                    .to_string();
                if !data.is_empty() {
                    images.push(ToolRunImage {
                        media_type: media_type.clone(),
                        data,
                        path: None,
                    });
                }
                text.push(format!("[image: {media_type}]"));
            }
            Some("audio") => {
                let media_type = block
                    .get("mimeType")
                    .or_else(|| block.get("mime_type"))
                    .and_then(Value::as_str)
                    .unwrap_or("audio/*");
                text.push(format!("[audio: {media_type}]"));
            }
            _ => text.push(pretty_json(&block)),
        }
    }

    if let Some(structured) = result.structured_content {
        text.push(format!("Structured content:\n{}", pretty_json(&structured)));
    }

    let content = clip_output(text.join("\n\n"));
    if result.is_error {
        ToolRunResult::err(content, Vec::new())
    } else if images.is_empty() {
        ToolRunResult::ok(content, Vec::new())
    } else {
        ToolRunResult::ok_with_images(content, images, Vec::new())
    }
}

fn mcp_tool_description(server: &McpServerConfig, tool: &McpServerTool) -> String {
    let mut pieces = vec![format!(
        "MCP server `{}` tool `{}`.",
        server.name, tool.name
    )];
    if let Some(title) = tool
        .title
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        pieces.push(format!("Title: {title}."));
    }
    if let Some(description) = tool
        .description
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        pieces.push(description.to_string());
    }
    pieces.join(" ")
}

fn mcp_tool_display_name(tool: &McpServerTool) -> String {
    tool.title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| display_mcp_tool_name(&tool.name))
}

fn display_mcp_server_name(value: &str) -> String {
    let trimmed = value.trim();
    let Some(rest) = trimmed.get(3..) else {
        return trimmed.to_string();
    };
    if !trimmed[..3].eq_ignore_ascii_case("mcp") {
        return trimmed.to_string();
    }

    let stripped = rest
        .trim_start_matches(|ch: char| ch == '-' || ch == '_' || ch == '.' || ch.is_whitespace())
        .trim();
    if stripped.is_empty() {
        trimmed.to_string()
    } else {
        stripped.to_string()
    }
}

fn display_mcp_tool_name(value: &str) -> String {
    let mut spaced = String::new();
    let mut previous: Option<char> = None;

    for ch in value.trim().chars() {
        if matches!(ch, '_' | '-' | '.') {
            if !spaced.ends_with(' ') {
                spaced.push(' ');
            }
        } else {
            if let Some(prev) = previous {
                if ch.is_ascii_uppercase() && (prev.is_ascii_lowercase() || prev.is_ascii_digit()) {
                    spaced.push(' ');
                }
            }
            spaced.push(ch);
        }
        previous = Some(ch);
    }

    let words = spaced
        .split_whitespace()
        .map(display_mcp_word)
        .collect::<Vec<_>>();

    if words.is_empty() {
        "Tool".to_string()
    } else {
        words.join(" ")
    }
}

fn display_mcp_word(word: &str) -> String {
    if word
        .chars()
        .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit())
    {
        return word.to_string();
    }

    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };

    format!(
        "{}{}",
        first.to_uppercase().collect::<String>(),
        chars.as_str().to_ascii_lowercase()
    )
}

fn normalize_input_schema(value: Value) -> Value {
    if value.is_object() {
        value
    } else {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": true
        })
    }
}

fn unique_tool_name(
    server: &McpServerConfig,
    original_name: &str,
    known: &HashMap<String, McpToolBinding>,
) -> String {
    let server_slug = slug(&server.name)
        .or_else(|| slug(&server.id))
        .unwrap_or_else(|| "server".into());
    let tool_slug = slug(original_name).unwrap_or_else(|| "tool".into());
    let hash = short_hash(&(server.id.as_str(), original_name));
    let mut base = format!("mcp__{server_slug}__{tool_slug}");
    if base.len() > TOOL_NAME_LIMIT {
        let budget = TOOL_NAME_LIMIT.saturating_sub(7 + hash.len());
        base = format!("{}__{}", truncate_chars(&base, budget), hash);
    }

    if !known.contains_key(&base) {
        return base;
    }

    for idx in 2..1000 {
        let suffix = format!("__{idx}");
        let candidate = if base.len() + suffix.len() > TOOL_NAME_LIMIT {
            format!(
                "{}{}",
                truncate_chars(&base, TOOL_NAME_LIMIT - suffix.len()),
                suffix
            )
        } else {
            format!("{base}{suffix}")
        };
        if !known.contains_key(&candidate) {
            return candidate;
        }
    }

    format!("mcp__tool__{hash}")
}

fn slug(value: &str) -> Option<String> {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch == '-' || ch == '_' || ch == ' ') && !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_matches('_').to_string();
    (!out.is_empty()).then_some(out)
}

fn truncate_chars(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

fn short_hash<T: Hash>(value: &T) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:x}", hasher.finish())[..8].to_string()
}

fn default_enabled() -> bool {
    true
}

fn default_auth_type() -> String {
    "bearer".to_string()
}

#[derive(Debug, Clone)]
pub struct PkceCodes {
    pub code_verifier: String,
    pub code_challenge: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthServerMetadata {
    pub issuer: String,
    #[serde(alias = "authorizationEndpoint")]
    pub authorization_endpoint: String,
    #[serde(alias = "tokenEndpoint")]
    pub token_endpoint: String,
    #[serde(
        default,
        alias = "registrationEndpoint",
        skip_serializing_if = "Option::is_none"
    )]
    pub registration_endpoint: Option<String>,
    #[serde(default, alias = "scopesSupported")]
    pub scopes_supported: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OAuthProtectedResourceMetadata {
    resource: String,
    #[serde(default, alias = "authorizationServers")]
    authorization_servers: Vec<String>,
}

#[derive(Debug, Clone)]
struct OAuthDiscoveryDetails {
    resource: String,
    metadata: OAuthServerMetadata,
}

#[derive(Debug, Deserialize)]
struct OAuthClientRegistrationResponse {
    #[serde(alias = "clientId")]
    client_id: String,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    scope: Option<String>,
}

#[derive(Debug, Clone)]
struct McpAuthRequired {
    server_name: String,
    metadata_url: Option<String>,
    body: Option<String>,
}

impl McpAuthRequired {
    fn new(
        server_name: impl Into<String>,
        metadata_url: Option<String>,
        body: Option<String>,
    ) -> Self {
        Self {
            server_name: server_name.into(),
            metadata_url,
            body,
        }
    }
}

impl fmt::Display for McpAuthRequired {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MCP server `{}` requires authentication. Connect this MCP server in Settings or provide a bearer token/header.",
            self.server_name
        )?;
        if let Some(url) = &self.metadata_url {
            write!(f, " OAuth metadata: {url}.")?;
        }
        if let Some(body) = &self.body {
            let body = body.trim();
            if !body.is_empty() {
                write!(f, " {body}")?;
            }
        }
        Ok(())
    }
}

impl StdError for McpAuthRequired {}

enum McpClient {
    Stdio(McpStdioClient),
    Http(McpHttpClient),
}

impl McpClient {
    async fn connect(config: &McpServerConfig) -> Result<Self> {
        Self::connect_with_timeout(config, REQUEST_TIMEOUT).await
    }

    async fn connect_with_timeout(
        config: &McpServerConfig,
        request_timeout: Duration,
    ) -> Result<Self> {
        if mcp_server_url(config).is_some() {
            Ok(Self::Http(
                McpHttpClient::connect_with_timeout(config, request_timeout).await?,
            ))
        } else {
            Ok(Self::Stdio(
                McpStdioClient::connect_with_timeout(config, request_timeout).await?,
            ))
        }
    }

    async fn list_tools(&mut self) -> Result<Vec<McpServerTool>> {
        match self {
            Self::Stdio(client) => client.list_tools().await,
            Self::Http(client) => client.list_tools().await,
        }
    }

    async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<McpCallToolResult> {
        match self {
            Self::Stdio(client) => client.call_tool(name, arguments).await,
            Self::Http(client) => client.call_tool(name, arguments).await,
        }
    }
}

struct McpHttpClient {
    config: McpServerConfig,
    url: Url,
    http: reqwest::Client,
    session_id: Option<String>,
    next_id: u64,
    request_timeout: Duration,
}

impl McpHttpClient {
    async fn connect_with_timeout(
        config: &McpServerConfig,
        request_timeout: Duration,
    ) -> Result<Self> {
        let raw_url =
            mcp_server_url(config).ok_or_else(|| anyhow!("missing MCP URL for {}", config.name))?;
        let url = parse_mcp_http_url(raw_url)?;
        let http = reqwest::Client::builder()
            .user_agent(format!("sinew/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .context("unable to build MCP HTTP client")?;
        let mut client = Self {
            config: config.clone(),
            url,
            http,
            session_id: None,
            next_id: 1,
            request_timeout,
        };
        client.initialize().await?;
        Ok(client)
    }

    async fn initialize(&mut self) -> Result<()> {
        self.request(
            "initialize",
            json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {
                    "name": "sinew",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )
        .await?;
        self.notify("notifications/initialized", None).await?;
        Ok(())
    }

    async fn list_tools(&mut self) -> Result<Vec<McpServerTool>> {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let params = match cursor.as_deref() {
                Some(cursor) => json!({ "cursor": cursor }),
                None => json!({}),
            };
            let value = self.request("tools/list", params).await?;
            let page: McpListToolsResult =
                serde_json::from_value(value).context("invalid MCP tools/list response")?;
            tools.extend(page.tools);
            cursor = page.next_cursor;
            if cursor.as_deref().unwrap_or_default().is_empty() {
                break;
            }
        }

        Ok(tools)
    }

    async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<McpCallToolResult> {
        let params = json!({
            "name": name,
            "arguments": match arguments {
                Value::Object(_) => arguments,
                _ => json!({}),
            }
        });
        let value = self.request("tools/call", params).await?;
        serde_json::from_value(value).context("invalid MCP tools/call response")
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let message = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        let value = timeout(
            self.request_timeout,
            self.send_message(message, Some(id), method),
        )
        .await
        .map_err(|_| anyhow!("MCP request `{method}` timed out"))??;
        value.ok_or_else(|| anyhow!("MCP response missing result"))
    }

    async fn notify(&mut self, method: &str, params: Option<Value>) -> Result<()> {
        let mut message = json!({
            "jsonrpc": "2.0",
            "method": method
        });
        if let Some(params) = params {
            message["params"] = params;
        }
        timeout(
            self.request_timeout,
            self.send_message(message, None, method),
        )
        .await
        .map_err(|_| anyhow!("MCP notification `{method}` timed out"))??;
        Ok(())
    }

    async fn send_message(
        &mut self,
        message: Value,
        expected_id: Option<u64>,
        method: &str,
    ) -> Result<Option<Value>> {
        let mut headers = HeaderMap::new();
        apply_mcp_config_headers(&mut headers, &self.config)?;
        if let Some(session_id) = self.session_id.as_deref() {
            headers.insert(
                HeaderName::from_static(MCP_SESSION_ID_HEADER),
                HeaderValue::from_str(session_id).context("invalid MCP session id")?,
            );
        }
        if !headers.contains_key(AUTHORIZATION) {
            if let Some(token) = bearer_token_for_server(&self.config, &self.http).await? {
                let value = HeaderValue::from_str(&format!("Bearer {token}"))
                    .context("invalid MCP bearer token")?;
                headers.insert(AUTHORIZATION, value);
            }
        }

        let response = self
            .http
            .post(self.url.clone())
            .headers(headers)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, MCP_HTTP_ACCEPT)
            .json(&message)
            .send()
            .await
            .with_context(|| format!("MCP HTTP request `{method}` failed"))?;

        if let Some(session_id) = response
            .headers()
            .get(MCP_SESSION_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            self.session_id = Some(session_id.to_string());
        }

        if response.status() == StatusCode::UNAUTHORIZED {
            let metadata_url = www_authenticate_resource_metadata(response.headers());
            let body = response.text().await.unwrap_or_default();
            return Err(
                McpAuthRequired::new(self.config.name.clone(), metadata_url, Some(body)).into(),
            );
        }

        let status = response.status();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            bail!(
                "MCP HTTP request `{method}` failed with {status}: {}",
                clip_output(body)
            );
        }

        if expected_id.is_none() && body.trim().is_empty() {
            return Ok(None);
        }
        if status == StatusCode::ACCEPTED && body.trim().is_empty() {
            return Ok(None);
        }

        if content_type.contains("text/event-stream") {
            let id = expected_id.ok_or_else(|| anyhow!("MCP notification returned SSE"))?;
            return parse_sse_json_rpc_response(&body, id);
        }

        let value: Value = serde_json::from_str(body.trim())
            .with_context(|| "MCP HTTP server returned invalid JSON")?;
        match expected_id {
            Some(id) => Ok(Some(json_rpc_result(value, id)?)),
            None => Ok(None),
        }
    }
}

fn parse_mcp_http_url(raw_url: &str) -> Result<Url> {
    let url = Url::parse(raw_url.trim()).context("invalid MCP URL")?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        _ => bail!("MCP URL must use http or https"),
    }
}

fn apply_mcp_config_headers(headers: &mut HeaderMap, config: &McpServerConfig) -> Result<()> {
    for header in &config.headers {
        let key = header.key.trim();
        if key.is_empty() {
            continue;
        }
        let name = HeaderName::from_bytes(key.as_bytes())
            .with_context(|| format!("invalid MCP header `{key}`"))?;
        let value = HeaderValue::from_str(&header.value)
            .with_context(|| format!("invalid value for MCP header `{key}`"))?;
        headers.insert(name, value);
    }
    Ok(())
}

async fn bearer_token_for_server(
    config: &McpServerConfig,
    http: &reqwest::Client,
) -> Result<Option<String>> {
    if mcp_auth_type(config).as_deref() == Some("none") {
        return Ok(None);
    }
    if let Some(token) = explicit_bearer_token(config) {
        return Ok(Some(token));
    }
    stored_mcp_oauth_bearer(config, http).await
}

fn bearer_token_from_server(config: &McpServerConfig) -> Option<String> {
    explicit_bearer_token(config).or_else(|| stored_mcp_oauth_access_token(config).ok().flatten())
}

fn explicit_bearer_token(config: &McpServerConfig) -> Option<String> {
    let auth = config.auth.as_ref()?;
    let auth_type = auth.auth_type.trim().to_ascii_lowercase();
    if auth_type == "none" || auth.token.trim().is_empty() {
        return None;
    }
    if auth_type.is_empty()
        || matches!(
            auth_type.as_str(),
            "bearer" | "token" | "api_key" | "apikey"
        )
    {
        return Some(auth.token.trim().to_string());
    }
    None
}

fn mcp_auth_type(config: &McpServerConfig) -> Option<String> {
    config
        .auth
        .as_ref()
        .map(|auth| auth.auth_type.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

fn json_rpc_result(value: Value, expected_id: u64) -> Result<Value> {
    if value.get("id") != Some(&json!(expected_id)) {
        bail!("MCP response id mismatch");
    }
    if let Some(error) = value.get("error") {
        bail!("{}", format_json_rpc_error(error));
    }
    value
        .get("result")
        .cloned()
        .ok_or_else(|| anyhow!("MCP response missing result"))
}

fn parse_sse_json_rpc_response(body: &str, expected_id: u64) -> Result<Option<Value>> {
    let mut current = String::new();
    let mut events = Vec::new();
    for raw_line in body.lines() {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() {
            if !current.trim().is_empty() {
                events.push(std::mem::take(&mut current));
            }
            continue;
        }
        if let Some(data) = line.strip_prefix("data:") {
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(data.trim_start());
        }
    }
    if !current.trim().is_empty() {
        events.push(current);
    }

    for event in events {
        let event = event.trim();
        if event.is_empty() || event == "[DONE]" {
            continue;
        }
        let value: Value =
            serde_json::from_str(event).with_context(|| "MCP SSE event contained invalid JSON")?;
        if value.get("id") == Some(&json!(expected_id)) {
            return Ok(Some(json_rpc_result(value, expected_id)?));
        }
    }

    bail!("MCP SSE stream did not contain a response for request {expected_id}")
}

fn www_authenticate_resource_metadata(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all("www-authenticate")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(resource_metadata_from_challenge)
}

fn resource_metadata_from_challenge(challenge: &str) -> Option<String> {
    let needle = "resource_metadata=";
    let idx = challenge.to_ascii_lowercase().find(needle)?;
    let rest = &challenge[idx + needle.len()..];
    if let Some(quoted) = rest.strip_prefix('"') {
        return quoted
            .split_once('"')
            .map(|(value, _)| value.to_string())
            .filter(|value| !value.trim().is_empty());
    }
    rest.split(|ch: char| ch == ',' || ch.is_whitespace())
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub async fn discover_mcp_oauth(
    settings: &McpSettings,
    server_id: &str,
) -> Result<McpOAuthDiscovery> {
    let server = find_mcp_server(settings, server_id)?;
    if mcp_server_url(server).is_none() {
        return Ok(McpOAuthDiscovery {
            auth_required: false,
            authenticated: false,
            auth_url: None,
            error: Some("OAuth is only available for URL-based MCP servers".into()),
        });
    }
    match discover_oauth_server(server).await {
        Ok(details) => Ok(McpOAuthDiscovery {
            auth_required: true,
            authenticated: mcp_server_has_auth(server),
            auth_url: Some(details.metadata.authorization_endpoint),
            error: None,
        }),
        Err(err) => Ok(McpOAuthDiscovery {
            auth_required: false,
            authenticated: mcp_server_has_auth(server),
            auth_url: None,
            error: Some(err.to_string()),
        }),
    }
}

pub async fn start_mcp_oauth_login(
    settings: &McpSettings,
    server_id: &str,
) -> Result<StartMcpOAuthLoginResult> {
    let server = find_mcp_server(settings, server_id)?.clone();
    if mcp_server_url(&server).is_none() {
        bail!("OAuth is only available for URL-based MCP servers");
    }
    let details = discover_oauth_server(&server).await?;
    let pkce = generate_pkce();
    let state = generate_state();
    let scopes = oauth_scopes_for_server(&server, &details.metadata);
    let client_id = register_oauth_client(&details.metadata, &scopes).await?;
    let auth_url = oauth_authorize_url(
        &details.metadata,
        &client_id,
        &details.resource,
        &scopes,
        &pkce,
        &state,
    )?;
    let login_id = generate_state();
    let output = StartMcpOAuthLoginOutput {
        login_id: login_id.clone(),
        auth_url: auth_url.clone(),
    };
    Ok(StartMcpOAuthLoginResult {
        output,
        plan: McpOAuthLoginPlan {
            login_id,
            auth_url,
            server,
            metadata: details.metadata,
            resource: details.resource,
            pkce,
            state,
            client_id,
        },
    })
}

pub async fn exchange_mcp_oauth_code(plan: &McpOAuthLoginPlan, code: &str) -> Result<()> {
    let http = reqwest::Client::builder()
        .user_agent(format!("sinew/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .context("unable to build MCP OAuth client")?;
    let mut form = vec![
        ("grant_type".to_string(), "authorization_code".to_string()),
        ("code".to_string(), code.to_string()),
        ("redirect_uri".to_string(), OAUTH_REDIRECT_URI.to_string()),
        ("client_id".to_string(), plan.client_id.clone()),
        ("code_verifier".to_string(), plan.pkce.code_verifier.clone()),
    ];
    if !plan.resource.trim().is_empty() {
        form.push(("resource".to_string(), plan.resource.clone()));
    }

    let response = http
        .post(&plan.metadata.token_endpoint)
        .header(ACCEPT, "application/json")
        .form(&form)
        .send()
        .await
        .context("MCP OAuth token exchange failed")?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        bail!("MCP OAuth token exchange failed with {status}: {body}");
    }
    let token: OAuthTokenResponse = response
        .json()
        .await
        .context("invalid MCP OAuth token response")?;
    save_mcp_oauth_token(plan, token)
}

pub fn mcp_oauth_connected(settings: &McpSettings, server_id: &str) -> Result<bool> {
    Ok(mcp_server_oauth_connected(find_mcp_server(
        settings, server_id,
    )?))
}

pub fn mcp_server_oauth_connected(server: &McpServerConfig) -> bool {
    stored_mcp_oauth_access_token(server)
        .ok()
        .flatten()
        .is_some()
}

pub fn delete_mcp_oauth(settings: &McpSettings, server_id: &str) -> Result<()> {
    let server = find_mcp_server(settings, server_id)?;
    let Some(key) = mcp_auth_key(server) else {
        return Ok(());
    };
    let mut file = load_mcp_auth_file()?;
    file.servers.remove(&key);
    save_mcp_auth_file(&file)
}

fn find_mcp_server<'a>(settings: &'a McpSettings, server_id: &str) -> Result<&'a McpServerConfig> {
    settings
        .servers
        .iter()
        .find(|server| server.id == server_id || server.name == server_id)
        .ok_or_else(|| anyhow!("MCP server `{server_id}` not found"))
}

async fn discover_oauth_server(server: &McpServerConfig) -> Result<OAuthDiscoveryDetails> {
    let raw_url = mcp_server_url(server).ok_or_else(|| anyhow!("missing MCP URL"))?;
    let server_url = parse_mcp_http_url(raw_url)?;
    let http = reqwest::Client::builder()
        .user_agent(format!("sinew/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .context("unable to build MCP OAuth discovery client")?;
    let metadata_url = discover_resource_metadata_url(&http, server, &server_url).await?;
    let prm = fetch_protected_resource_metadata(&http, &[metadata_url]).await?;
    let metadata = fetch_oauth_server_metadata(&http, &prm.authorization_servers).await?;
    Ok(OAuthDiscoveryDetails {
        resource: prm.resource,
        metadata,
    })
}

async fn discover_resource_metadata_url(
    http: &reqwest::Client,
    server: &McpServerConfig,
    server_url: &Url,
) -> Result<String> {
    let initialize = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "sinew", "version": env!("CARGO_PKG_VERSION") }
        }
    });
    let response = http
        .post(server_url.clone())
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, MCP_HTTP_ACCEPT)
        .json(&initialize)
        .send()
        .await
        .context("MCP OAuth discovery request failed")?;
    if response.status() == StatusCode::UNAUTHORIZED {
        if let Some(metadata_url) = www_authenticate_resource_metadata(response.headers()) {
            return Ok(metadata_url);
        }
    }

    let candidates = default_protected_resource_metadata_urls(server_url);
    for candidate in &candidates {
        if fetch_protected_resource_metadata(http, std::slice::from_ref(candidate))
            .await
            .is_ok()
        {
            return Ok(candidate.clone());
        }
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if status.is_success() {
        bail!(
            "MCP server `{}` did not request OAuth authentication",
            server.name
        );
    }
    bail!(
        "MCP server `{}` did not advertise OAuth metadata ({status}): {}",
        server.name,
        clip_output(body)
    )
}

fn default_protected_resource_metadata_urls(server_url: &Url) -> Vec<String> {
    let mut urls = Vec::new();
    let path = server_url.path().trim_end_matches('/');
    if !path.is_empty() {
        let mut path_url = server_url.clone();
        path_url.set_path(&format!("/.well-known/oauth-protected-resource{path}"));
        path_url.set_query(None);
        path_url.set_fragment(None);
        urls.push(path_url.to_string());
    }

    let mut root = server_url.clone();
    root.set_path("/.well-known/oauth-protected-resource");
    root.set_query(None);
    root.set_fragment(None);
    let value = root.to_string();
    if !urls.contains(&value) {
        urls.push(value);
    }
    urls
}

async fn fetch_protected_resource_metadata(
    http: &reqwest::Client,
    urls: &[String],
) -> Result<OAuthProtectedResourceMetadata> {
    let mut errors = Vec::new();
    for url in urls {
        match http
            .get(url)
            .header(ACCEPT, "application/json")
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                let value: Value = response.json().await.with_context(|| {
                    format!("invalid OAuth protected resource metadata at {url}")
                })?;
                return serde_json::from_value(value).with_context(|| {
                    format!("invalid OAuth protected resource metadata at {url}")
                });
            }
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                errors.push(format!("{url}: {status} {body}"));
            }
            Err(err) => errors.push(format!("{url}: {err}")),
        }
    }
    bail!(
        "unable to fetch OAuth protected resource metadata: {}",
        errors.join("; ")
    )
}

async fn fetch_oauth_server_metadata(
    http: &reqwest::Client,
    authorization_servers: &[String],
) -> Result<OAuthServerMetadata> {
    let mut errors = Vec::new();
    for issuer in authorization_servers {
        for url in oauth_server_metadata_urls(issuer) {
            match http
                .get(&url)
                .header(ACCEPT, "application/json")
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    let value: Value = response
                        .json()
                        .await
                        .with_context(|| format!("invalid OAuth server metadata at {url}"))?;
                    return parse_oauth_server_metadata(value)
                        .with_context(|| format!("invalid OAuth server metadata at {url}"));
                }
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    errors.push(format!("{url}: {status} {body}"));
                }
                Err(err) => errors.push(format!("{url}: {err}")),
            }
        }
    }
    bail!(
        "unable to fetch OAuth server metadata: {}",
        errors.join("; ")
    )
}

fn parse_oauth_server_metadata(value: Value) -> Result<OAuthServerMetadata> {
    let metadata: OAuthServerMetadata = serde_json::from_value(value)?;
    if metadata.authorization_endpoint.trim().is_empty() {
        bail!("OAuth metadata is missing authorization_endpoint");
    }
    if metadata.token_endpoint.trim().is_empty() {
        bail!("OAuth metadata is missing token_endpoint");
    }
    Ok(metadata)
}

fn oauth_server_metadata_urls(issuer: &str) -> Vec<String> {
    let Ok(url) = Url::parse(issuer) else {
        return Vec::new();
    };
    let mut urls = Vec::new();
    let path = url.path().trim_end_matches('/');
    if !path.is_empty() {
        let mut with_path = url.clone();
        with_path.set_path(&format!("/.well-known/oauth-authorization-server{path}"));
        with_path.set_query(None);
        with_path.set_fragment(None);
        urls.push(with_path.to_string());
    }
    let mut root = url.clone();
    root.set_path("/.well-known/oauth-authorization-server");
    root.set_query(None);
    root.set_fragment(None);
    if !urls.contains(&root.to_string()) {
        urls.push(root.to_string());
    }
    if !path.is_empty() {
        let mut openid = url.clone();
        openid.set_path(&format!("/.well-known/openid-configuration{path}"));
        openid.set_query(None);
        openid.set_fragment(None);
        if !urls.contains(&openid.to_string()) {
            urls.push(openid.to_string());
        }
    }
    urls
}

async fn register_oauth_client(
    metadata: &OAuthServerMetadata,
    scopes: &[String],
) -> Result<String> {
    let Some(endpoint) = metadata.registration_endpoint.as_deref() else {
        return Ok(OAUTH_CLIENT_ID.to_string());
    };
    let http = reqwest::Client::builder()
        .user_agent(format!("sinew/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .context("unable to build MCP OAuth registration client")?;
    let mut payload = json!({
        "client_name": OAUTH_CLIENT_NAME,
        "client_uri": OAUTH_CLIENT_URI,
        "redirect_uris": [OAUTH_REDIRECT_URI],
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none"
    });
    if !scopes.is_empty() {
        payload["scope"] = json!(scopes.join(" "));
    }
    let response = http
        .post(endpoint)
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json")
        .json(&payload)
        .send()
        .await
        .context("MCP OAuth client registration failed")?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        bail!("MCP OAuth client registration failed with {status}: {body}");
    }
    let registration: OAuthClientRegistrationResponse = response
        .json()
        .await
        .context("invalid MCP OAuth client registration response")?;
    if registration.client_id.trim().is_empty() {
        bail!("MCP OAuth registration response is missing client_id");
    }
    Ok(registration.client_id)
}

fn oauth_scopes_for_server(
    server: &McpServerConfig,
    metadata: &OAuthServerMetadata,
) -> Vec<String> {
    if let Some(auth) = &server.auth {
        let scopes = auth
            .scopes
            .iter()
            .map(|scope| scope.trim())
            .filter(|scope| !scope.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if !scopes.is_empty() {
            return scopes;
        }
    }
    if metadata.scopes_supported.iter().any(|scope| scope == "mcp") {
        return vec!["mcp".to_string()];
    }
    Vec::new()
}

fn oauth_authorize_url(
    metadata: &OAuthServerMetadata,
    client_id: &str,
    resource: &str,
    scopes: &[String],
    pkce: &PkceCodes,
    state: &str,
) -> Result<String> {
    let mut url = Url::parse(&metadata.authorization_endpoint)
        .context("invalid MCP OAuth authorization endpoint")?;
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("response_type", "code")
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", OAUTH_REDIRECT_URI)
            .append_pair("code_challenge", &pkce.code_challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", state);
        if !scopes.is_empty() {
            query.append_pair("scope", &scopes.join(" "));
        }
        if !resource.trim().is_empty() {
            query.append_pair("resource", resource);
        }
    }
    Ok(url.to_string())
}

fn generate_pkce() -> PkceCodes {
    let verifier = generate_state();
    let digest = Sha256::digest(verifier.as_bytes());
    PkceCodes {
        code_verifier: verifier,
        code_challenge: URL_SAFE_NO_PAD.encode(digest),
    }
}

fn generate_state() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn oauth_expires_at(expires_in_seconds: Option<u64>) -> Option<i64> {
    expires_in_seconds.map(|seconds| now_ms() + (seconds as i64 * 1000) - OAUTH_REFRESH_SKEW_MS)
}

fn oauth_expired(expires_at_ms: Option<i64>) -> bool {
    expires_at_ms
        .map(|expires_at_ms| now_ms() + OAUTH_REFRESH_SKEW_MS >= expires_at_ms)
        .unwrap_or(false)
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredMcpAuthFile {
    #[serde(default)]
    servers: HashMap<String, StoredMcpAuth>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredMcpAuth {
    server_name: String,
    url: String,
    auth_mode: String,
    client_id: String,
    resource: String,
    metadata: OAuthServerMetadata,
    tokens: StoredMcpOAuthTokens,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_refresh_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredMcpOAuthTokens {
    access_token: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    refresh_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    token_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at_ms: Option<i64>,
}

fn save_mcp_oauth_token(plan: &McpOAuthLoginPlan, token: OAuthTokenResponse) -> Result<()> {
    let key = mcp_auth_key(&plan.server).ok_or_else(|| anyhow!("MCP OAuth server has no URL"))?;
    let mut file = load_mcp_auth_file()?;
    file.servers.insert(
        key,
        StoredMcpAuth {
            server_name: plan.server.name.clone(),
            url: mcp_server_url(&plan.server).unwrap_or_default().to_string(),
            auth_mode: "oauth".into(),
            client_id: plan.client_id.clone(),
            resource: plan.resource.clone(),
            metadata: plan.metadata.clone(),
            tokens: StoredMcpOAuthTokens {
                access_token: token.access_token,
                refresh_token: token.refresh_token.unwrap_or_default(),
                token_type: token.token_type,
                scope: token.scope,
                expires_at_ms: oauth_expires_at(token.expires_in),
            },
            last_refresh_ms: Some(now_ms()),
        },
    );
    save_mcp_auth_file(&file)
}

async fn stored_mcp_oauth_bearer(
    config: &McpServerConfig,
    http: &reqwest::Client,
) -> Result<Option<String>> {
    let Some(key) = mcp_auth_key(config) else {
        return Ok(None);
    };
    let mut file = load_mcp_auth_file()?;
    let Some(entry) = file.servers.get(&key).cloned() else {
        return Ok(None);
    };
    if !oauth_expired(entry.tokens.expires_at_ms) && !entry.tokens.access_token.trim().is_empty() {
        return Ok(Some(entry.tokens.access_token));
    }
    if entry.tokens.refresh_token.trim().is_empty() {
        bail!(
            "MCP OAuth token for `{}` is expired; reconnect the server",
            config.name
        );
    }

    let mut form = vec![
        ("grant_type".to_string(), "refresh_token".to_string()),
        (
            "refresh_token".to_string(),
            entry.tokens.refresh_token.clone(),
        ),
        ("client_id".to_string(), entry.client_id.clone()),
    ];
    if !entry.resource.trim().is_empty() {
        form.push(("resource".to_string(), entry.resource.clone()));
    }
    let response = http
        .post(&entry.metadata.token_endpoint)
        .header(ACCEPT, "application/json")
        .form(&form)
        .send()
        .await
        .context("MCP OAuth refresh failed")?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        bail!("MCP OAuth refresh failed with {status}: {body}");
    }
    let token: OAuthTokenResponse = response
        .json()
        .await
        .context("invalid MCP OAuth refresh response")?;
    let access = token.access_token.clone();
    file.servers.insert(
        key,
        StoredMcpAuth {
            tokens: StoredMcpOAuthTokens {
                access_token: token.access_token,
                refresh_token: token
                    .refresh_token
                    .unwrap_or_else(|| entry.tokens.refresh_token.clone()),
                token_type: token.token_type.or(entry.tokens.token_type),
                scope: token.scope.or(entry.tokens.scope),
                expires_at_ms: oauth_expires_at(token.expires_in).or(entry.tokens.expires_at_ms),
            },
            last_refresh_ms: Some(now_ms()),
            ..entry
        },
    );
    save_mcp_auth_file(&file)?;
    Ok(Some(access))
}

fn stored_mcp_oauth_access_token(config: &McpServerConfig) -> Result<Option<String>> {
    let Some(key) = mcp_auth_key(config) else {
        return Ok(None);
    };
    let file = load_mcp_auth_file()?;
    let Some(entry) = file.servers.get(&key) else {
        return Ok(None);
    };
    let token = entry.tokens.access_token.trim();
    if token.is_empty() {
        Ok(None)
    } else {
        Ok(Some(token.to_string()))
    }
}

fn mcp_auth_key(config: &McpServerConfig) -> Option<String> {
    let raw_url = mcp_server_url(config)?;
    let mut url = Url::parse(raw_url).ok()?;
    url.set_fragment(None);
    Some(url.to_string())
}

fn mcp_auth_file_path() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("dev", "hyrak", "sinew")
        .ok_or_else(|| anyhow!("unable to resolve local data directory"))?;
    Ok(dirs.data_local_dir().join(OAUTH_AUTH_FILE))
}

fn load_mcp_auth_file() -> Result<StoredMcpAuthFile> {
    let path = mcp_auth_file_path()?;
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(StoredMcpAuthFile::default())
        }
        Err(err) => return Err(anyhow!("unable to read MCP auth file: {err}")),
    };
    serde_json::from_slice(&bytes).context("invalid MCP auth file")
}

fn save_mcp_auth_file(file: &StoredMcpAuthFile) -> Result<()> {
    let path = mcp_auth_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("unable to create MCP auth directory")?;
    }
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, serde_json::to_vec_pretty(file)?).context("unable to write MCP auth file")?;
    apply_mcp_auth_permissions(&temp)?;
    fs::rename(&temp, &path).context("unable to replace MCP auth file")?;
    Ok(())
}

#[cfg(unix)]
fn apply_mcp_auth_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .context("unable to chmod MCP auth file")?;
    Ok(())
}

#[cfg(not(unix))]
fn apply_mcp_auth_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

struct McpStdioClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    request_timeout: Duration,
}

impl McpStdioClient {
    async fn connect_with_timeout(
        config: &McpServerConfig,
        request_timeout: Duration,
    ) -> Result<Self> {
        let command_name = config.command.trim();
        if command_name.is_empty() {
            bail!("missing MCP command for {}", config.name);
        }

        let search_paths = mcp_search_paths(config);
        let program = resolve_mcp_command(command_name, &search_paths)
            .unwrap_or_else(|| PathBuf::from(command_name));
        let path_env = env::join_paths(&search_paths).ok();
        let mut command = Command::new(program);
        command
            .args(config.args.iter().filter(|arg| !arg.is_empty()))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        if let Some(path_env) = path_env {
            command.env("PATH", path_env);
        }

        if let Some(cwd) = config
            .cwd
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            command.current_dir(cwd);
        }
        for env in &config.env {
            let key = env.key.trim();
            if !key.is_empty() {
                if is_path_env_key(key) {
                    continue;
                }
                command.env(key, &env.value);
            }
        }

        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);

        let mut child = command
            .spawn()
            .with_context(|| format!("unable to spawn `{}`", config.command))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("MCP server stdin unavailable"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("MCP server stdout unavailable"))?;

        if let Some(mut stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut sink = Vec::new();
                let _ = stderr.read_to_end(&mut sink).await;
            });
        }

        let mut client = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
            request_timeout,
        };
        client.initialize().await?;
        Ok(client)
    }

    async fn initialize(&mut self) -> Result<()> {
        self.request(
            "initialize",
            json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {
                    "name": "sinew",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )
        .await?;
        self.notify("notifications/initialized", None).await?;
        Ok(())
    }

    async fn list_tools(&mut self) -> Result<Vec<McpServerTool>> {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let params = match cursor.as_deref() {
                Some(cursor) => json!({ "cursor": cursor }),
                None => json!({}),
            };
            let value = self.request("tools/list", params).await?;
            let page: McpListToolsResult =
                serde_json::from_value(value).context("invalid MCP tools/list response")?;
            tools.extend(page.tools);
            cursor = page.next_cursor;
            if cursor.as_deref().unwrap_or_default().is_empty() {
                break;
            }
        }

        Ok(tools)
    }

    async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<McpCallToolResult> {
        let params = json!({
            "name": name,
            "arguments": match arguments {
                Value::Object(_) => arguments,
                _ => json!({}),
            }
        });
        let value = self.request("tools/call", params).await?;
        serde_json::from_value(value).context("invalid MCP tools/call response")
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.write_message(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        }))
        .await?;

        timeout(self.request_timeout, self.read_response(id))
            .await
            .map_err(|_| anyhow!("MCP request `{method}` timed out"))?
    }

    async fn notify(&mut self, method: &str, params: Option<Value>) -> Result<()> {
        let mut message = json!({
            "jsonrpc": "2.0",
            "method": method
        });
        if let Some(params) = params {
            message["params"] = params;
        }
        self.write_message(message).await
    }

    async fn read_response(&mut self, id: u64) -> Result<Value> {
        loop {
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line).await?;
            if read == 0 {
                let status = self.child.try_wait().ok().flatten();
                bail!("MCP server closed stdout ({status:?})");
            }

            let value: Value = serde_json::from_str(line.trim())
                .with_context(|| "MCP server emitted invalid JSON")?;
            if value.get("id") == Some(&json!(id)) {
                if let Some(error) = value.get("error") {
                    bail!("{}", format_json_rpc_error(error));
                }
                return value
                    .get("result")
                    .cloned()
                    .ok_or_else(|| anyhow!("MCP response missing result"));
            }

            if let Some(request_id) = value.get("id").cloned() {
                if value.get("method").is_some() {
                    self.write_message(json!({
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "error": {
                            "code": -32601,
                            "message": "Method not supported by Sinew MCP client"
                        }
                    }))
                    .await?;
                }
            }
        }
    }

    async fn write_message(&mut self, value: Value) -> Result<()> {
        let mut line = serde_json::to_vec(&value)?;
        line.push(b'\n');
        self.stdin.write_all(&line).await?;
        self.stdin.flush().await?;
        Ok(())
    }
}

static DEFAULT_MCP_SEARCH_PATHS: OnceLock<Vec<PathBuf>> = OnceLock::new();

fn mcp_search_paths(config: &McpServerConfig) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();

    if let Some(path) = config.env.iter().rev().find_map(|env| {
        let key = env.key.trim();
        is_path_env_key(key).then_some(env.value.as_str())
    }) {
        push_split_paths(&mut paths, &mut seen, OsStr::new(path));
    }

    for path in default_mcp_search_paths() {
        push_path(&mut paths, &mut seen, path.clone());
    }

    paths
}

fn default_mcp_search_paths() -> &'static [PathBuf] {
    DEFAULT_MCP_SEARCH_PATHS
        .get_or_init(build_default_mcp_search_paths)
        .as_slice()
}

fn build_default_mcp_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();

    if let Some(path) = env::var_os("PATH") {
        push_split_paths(&mut paths, &mut seen, &path);
    }

    push_common_node_paths(&mut paths, &mut seen);

    paths
}

fn push_common_node_paths(paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) {
    #[cfg(target_os = "macos")]
    {
        push_dir(paths, seen, "/opt/homebrew/bin");
        push_dir(paths, seen, "/usr/local/bin");
        push_dir(paths, seen, "/usr/bin");
        push_dir(paths, seen, "/bin");
        push_dir(paths, seen, "/usr/sbin");
        push_dir(paths, seen, "/sbin");
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        push_dir(paths, seen, "/usr/local/bin");
        push_dir(paths, seen, "/usr/bin");
        push_dir(paths, seen, "/bin");
        push_dir(paths, seen, "/snap/bin");
        push_dir(paths, seen, "/home/linuxbrew/.linuxbrew/bin");
    }

    #[cfg(windows)]
    {
        if let Some(app_data) = env::var_os("APPDATA") {
            push_dir(paths, seen, PathBuf::from(app_data).join("npm"));
        }
        if let Some(program_files) = env::var_os("ProgramFiles") {
            push_dir(paths, seen, PathBuf::from(program_files).join("nodejs"));
        }
        if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
            push_dir(paths, seen, PathBuf::from(program_files_x86).join("nodejs"));
        }
    }

    let Some(home) = home_dir() else {
        return;
    };

    push_dir(paths, seen, home.join(".local/bin"));
    push_dir(paths, seen, home.join(".volta/bin"));
    push_dir(paths, seen, home.join(".asdf/shims"));
    push_dir(paths, seen, home.join(".nodenv/shims"));
    push_dir(paths, seen, home.join(".local/share/mise/shims"));
    push_dir(paths, seen, home.join(".mise/shims"));

    push_versioned_dir(paths, seen, home.join(".nvm/versions/node"), &["bin"]);
    push_versioned_dir(paths, seen, home.join(".asdf/installs/nodejs"), &["bin"]);
    push_versioned_dir(paths, seen, home.join(".nodenv/versions"), &["bin"]);
    push_versioned_dir(
        paths,
        seen,
        home.join(".local/share/mise/installs/node"),
        &["bin"],
    );
    push_versioned_dir(
        paths,
        seen,
        home.join(".local/share/fnm/node-versions"),
        &["installation", "bin"],
    );
}

fn push_split_paths(paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, value: &OsStr) {
    for path in env::split_paths(value) {
        push_path(paths, seen, path);
    }
}

fn push_dir(paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: impl Into<PathBuf>) {
    let path = path.into();
    if path.is_dir() {
        push_path(paths, seen, path);
    }
}

fn push_versioned_dir(
    paths: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
    root: PathBuf,
    suffix: &[&str],
) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let mut entries = entries.filter_map(|entry| entry.ok()).collect::<Vec<_>>();
    entries.sort_by(|a, b| {
        version_key(&b.file_name())
            .cmp(&version_key(&a.file_name()))
            .then_with(|| b.file_name().cmp(&a.file_name()))
    });

    for entry in entries {
        let mut path = entry.path();
        for segment in suffix {
            path.push(segment);
        }
        push_dir(paths, seen, path);
    }
}

fn push_path(paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: PathBuf) {
    if path.as_os_str().is_empty() || !seen.insert(path.clone()) {
        return;
    }
    paths.push(path);
}

fn resolve_mcp_command(command: &str, paths: &[PathBuf]) -> Option<PathBuf> {
    if command_has_path_separator(command) {
        return None;
    }

    for dir in paths {
        let candidate = dir.join(command);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }

        #[cfg(windows)]
        if Path::new(command).extension().is_none() {
            for extension in windows_path_extensions() {
                let candidate = dir.join(format!("{command}{extension}"));
                if is_executable_file(&candidate) {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_executable_file(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
}

fn command_has_path_separator(command: &str) -> bool {
    command.contains('/') || command.contains('\\')
}

#[cfg(windows)]
fn windows_path_extensions() -> Vec<String> {
    env::var_os("PATHEXT")
        .map(|value| {
            value
                .to_string_lossy()
                .split(';')
                .filter(|extension| !extension.is_empty())
                .map(|extension| {
                    if extension.starts_with('.') {
                        extension.to_string()
                    } else {
                        format!(".{extension}")
                    }
                })
                .collect()
        })
        .unwrap_or_else(|| vec![".com".into(), ".exe".into(), ".bat".into(), ".cmd".into()])
}

#[cfg(windows)]
fn is_path_env_key(key: &str) -> bool {
    key.eq_ignore_ascii_case("PATH")
}

#[cfg(not(windows))]
fn is_path_env_key(key: &str) -> bool {
    key == "PATH"
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn version_key(value: &OsStr) -> Vec<u64> {
    let numbers = value
        .to_string_lossy()
        .trim_start_matches('v')
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u64>().ok())
        .collect::<Vec<_>>();

    if numbers.is_empty() {
        vec![0]
    } else {
        numbers
    }
}

#[derive(Debug, Deserialize)]
struct McpListToolsResult {
    #[serde(default)]
    tools: Vec<McpServerTool>,
    #[serde(default, rename = "nextCursor")]
    next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct McpServerTool {
    name: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "inputSchema")]
    input_schema: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpCallToolResult {
    #[serde(default)]
    content: Vec<Value>,
    #[serde(default)]
    structured_content: Option<Value>,
    #[serde(default)]
    is_error: bool,
}

fn format_json_rpc_error(value: &Value) -> String {
    let message = value
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("MCP JSON-RPC error");
    let code = value.get("code").and_then(Value::as_i64);
    match code {
        Some(code) => format!("{message} ({code})"),
        None => message.to_string(),
    }
}

fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn clip_output(value: String) -> String {
    if value.len() <= TOOL_OUTPUT_LIMIT {
        return value;
    }
    let mut clipped = value.chars().take(TOOL_OUTPUT_LIMIT).collect::<String>();
    clipped.push_str("\n\n[Output truncated]");
    clipped
}
