use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet},
    env,
    ffi::OsStr,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Stdio,
    sync::OnceLock,
    time::Duration,
};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use claakecode_core::{ChatMessage, Part, ToolDescriptor};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::RwLock,
    time::timeout,
};
use tracing::warn;

use crate::tool_run::{ToolRunImage, ToolRunResult};

const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(45);
const CALL_TIMEOUT: Duration = Duration::from_secs(120);
const TOOL_OUTPUT_LIMIT: usize = 128 * 1024;
const TOOL_NAME_LIMIT: usize = 64;
const LOAD_MCP_TOOL_NAME: &str = "LoadMcpTool";
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
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: Vec<McpEnvVar>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
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
            let mut client = match McpStdioClient::connect(server).await {
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
        if name == LOAD_MCP_TOOL_NAME {
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

            if name == LOAD_MCP_TOOL_NAME {
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
        bail!("LoadMcpTool needs `server` and `tool` from the MCP catalog");
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
        .ok_or_else(|| anyhow!("LoadMcpTool needs `server` from the MCP catalog"))?;
    let tool = request
        .tool
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("LoadMcpTool needs `tool` from the MCP catalog"))?;

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
        if !server.enabled {
            probes.push(McpServerProbe {
                server_id: server.id.clone(),
                server_name: server.name.clone(),
                enabled: false,
                ok: true,
                tools: Vec::new(),
                error: None,
            });
            continue;
        }

        let mut client = match McpStdioClient::connect(server).await {
            Ok(client) => client,
            Err(err) => {
                probes.push(McpServerProbe {
                    server_id: server.id.clone(),
                    server_name: server.name.clone(),
                    enabled: true,
                    ok: false,
                    tools: Vec::new(),
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
                    error: None,
                });
            }
            Err(err) => probes.push(McpServerProbe {
                server_id: server.id.clone(),
                server_name: server.name.clone(),
                enabled: true,
                ok: false,
                tools: Vec::new(),
                error: Some(err.to_string()),
            }),
        }
    }

    probes
}

fn enabled_servers(settings: &McpSettings) -> impl Iterator<Item = &McpServerConfig> {
    settings
        .servers
        .iter()
        .filter(|server| server.enabled && !server.command.trim().is_empty())
}

async fn call_mcp_tool(binding: McpToolBinding, input: Value) -> ToolRunResult {
    match call_mcp_tool_inner(binding, input).await {
        Ok(result) => result,
        Err(err) => ToolRunResult::err(format!("MCP tool failed: {err}"), Vec::new()),
    }
}

async fn call_mcp_tool_inner(binding: McpToolBinding, input: Value) -> Result<ToolRunResult> {
    let mut client = McpStdioClient::connect_with_timeout(&binding.server, CALL_TIMEOUT).await?;
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

struct McpStdioClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    request_timeout: Duration,
}

impl McpStdioClient {
    async fn connect(config: &McpServerConfig) -> Result<Self> {
        Self::connect_with_timeout(config, REQUEST_TIMEOUT).await
    }

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
                    "name": "claakecode",
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
                            "message": "Method not supported by Claake Code MCP client"
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
