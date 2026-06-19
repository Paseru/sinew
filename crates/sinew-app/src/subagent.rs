use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, bail};
use directories::BaseDirs;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sinew_core::{ChatMessage, ModelRef, Part, Provider, Role, ServiceTier, ToolDescriptor};
use tokio::sync::mpsc;

use crate::tool_run::FileChange;
use crate::{
    run_turn, AgentEvent, AgentEventScope, AgentMode, BashTool, BrowserTools, CreateImageTool,
    EditFileTool, GlobTool, GoalWorkflowState, GrepTool, McpSettings, McpToolRegistry, QuestionTool,
    DocTool, ReadTool, SemanticSearchTool, SkillSettings, SkillTool, ToDoListTool, TodoListState,
    ToolRunResult, ToolSettings, TurnCancel, TurnContext, WebFetchTool, WebSearchTool,
    WorkspaceMemoryTool, WriteFileTool,
};

const TOOL_PREFIX: &str = "subagent_";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentConfig {
    pub id: String,
    pub name: String,
    pub description: String,
    pub prompt: String,
    pub model: ModelRef,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SubAgentSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubAgentSource {
    Workspace,
    Global,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentSettings {
    #[serde(default)]
    pub agents: Vec<SubAgentConfig>,
}

impl SubAgentSettings {
    pub fn normalized(mut self) -> Self {
        let mut seen = HashMap::<String, usize>::new();
        for (index, agent) in self.agents.iter_mut().enumerate() {
            agent.id = clean_id(&agent.id).unwrap_or_else(|| format!("agent-{}", index + 1));
            let count = seen.entry(agent.id.clone()).or_insert(0);
            if *count > 0 {
                agent.id = format!("{}-{}", agent.id, *count + 1);
            }
            *count += 1;

            agent.name = agent.name.trim().to_string();
            if agent.name.is_empty() {
                agent.name = format!("Sub-agent {}", index + 1);
            }
            agent.description = agent.description.trim().to_string();
            agent.prompt = agent.prompt.trim().to_string();
        }
        self
    }
}

#[derive(Clone)]
pub struct SubAgentTool {
    workspace_root: PathBuf,
    system_prompt: String,
    providers: HashMap<String, Arc<dyn Provider>>,
    settings: SubAgentSettings,
    mcp_settings: McpSettings,
    tool_settings: ToolSettings,
    skill_settings: SkillSettings,
    max_tool_rounds: usize,
    service_tier: Option<ServiceTier>,
    cancel: TurnCancel,
}

impl SubAgentTool {
    pub fn new(
        workspace_root: PathBuf,
        system_prompt: String,
        providers: HashMap<String, Arc<dyn Provider>>,
        settings: SubAgentSettings,
        mcp_settings: McpSettings,
        tool_settings: ToolSettings,
        skill_settings: SkillSettings,
        max_tool_rounds: usize,
        service_tier: Option<ServiceTier>,
        cancel: TurnCancel,
    ) -> Self {
        Self {
            workspace_root,
            system_prompt,
            providers,
            settings: settings.normalized(),
            mcp_settings,
            tool_settings,
            skill_settings,
            max_tool_rounds,
            service_tier,
            cancel,
        }
    }

    pub fn descriptors(&self) -> Vec<ToolDescriptor> {
        self.settings
            .agents
            .iter()
            .filter(|agent| agent.enabled)
            .map(|agent| ToolDescriptor {
                name: tool_name_for_agent(agent),
                description: descriptor_description(agent),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "The full free-form message to send to the sub-agent."
                        }
                    },
                    "required": ["prompt"],
                    "additionalProperties": false
                }),
            })
            .collect()
    }

    pub fn summary_for_tool_name(&self, name: &str) -> Option<String> {
        self.settings
            .agents
            .iter()
            .find(|agent| agent.enabled && tool_name_for_agent(agent) == name)
            .map(|agent| format!("Sub-agent · {}", agent.name))
    }

    pub async fn run(
        &self,
        tool_call_id: &str,
        name: &str,
        input: Value,
        mode: AgentMode,
        parent_event_tx: mpsc::UnboundedSender<AgentEvent>,
    ) -> Option<ToolRunResult> {
        let agent = self
            .settings
            .agents
            .iter()
            .find(|agent| agent.enabled && tool_name_for_agent(agent) == name)?
            .clone();

        Some(
            self.run_agent(tool_call_id, agent, input, mode, parent_event_tx)
                .await,
        )
    }

    async fn run_agent(
        &self,
        tool_call_id: &str,
        agent: SubAgentConfig,
        input: Value,
        mode: AgentMode,
        parent_event_tx: mpsc::UnboundedSender<AgentEvent>,
    ) -> ToolRunResult {
        let parsed: SubAgentInput = match serde_json::from_value(input) {
            Ok(value) => value,
            Err(err) => {
                return ToolRunResult::err(format!("invalid sub-agent input: {err}"), Vec::new())
            }
        };
        let prompt = parsed.prompt.trim();
        if prompt.is_empty() {
            return ToolRunResult::err("prompt is required", Vec::new());
        }

        let Some(provider) = self.providers.get(&agent.model.provider).cloned() else {
            return ToolRunResult::err(
                format!(
                    "provider `{}` is not configured or missing credentials",
                    agent.model.provider
                ),
                Vec::new(),
            );
        };
        if provider.capabilities(&agent.model).is_none() {
            return ToolRunResult::err(
                format!("model `{}` is not supported", agent.model.name),
                Vec::new(),
            );
        }

        let initial_message = prompt.to_string();
        let (child_cmd_tx, child_cmd_rx) = mpsc::unbounded_channel();
        self.cancel.register(child_cmd_tx);
        let child_mode = if mode == AgentMode::Goal {
            AgentMode::Act
        } else {
            mode
        };
        let child_context = TurnContext {
            provider,
            model: agent.model.clone(),
            cache_key: Some(format!("subagent:{}:{}", agent.id, tool_call_id)),
            cache_stable_message_count: 0,
            service_tier: self.service_tier,
            auto_compact: true,
            mode: child_mode,
            stop_questions: false,
            system_prompt: subagent_system_prompt(&self.system_prompt, &agent),
            history: vec![ChatMessage {
                role: Role::User,
                parts: vec![Part::Text {
                    text: initial_message.clone(),
                    meta: None,
                }],
            }],
            todo_list: TodoListState::default(),
            goal_workflow: GoalWorkflowState::Idle,
            bash: Arc::new(BashTool::new(self.workspace_root.clone())),
            glob: Arc::new(GlobTool::new(self.workspace_root.clone())),
            grep: Arc::new(GrepTool::new(self.workspace_root.clone())),
            read: Arc::new(ReadTool::new(self.workspace_root.clone())),
            edit_file: Arc::new(EditFileTool::new(self.workspace_root.clone())),
            write_file: Arc::new(WriteFileTool::new(self.workspace_root.clone())),
            create_image: Arc::new(CreateImageTool::with_settings(
                self.workspace_root.clone(),
                self.tool_settings.image_provider,
                self.tool_settings.openai_image_use_subscription,
                self.tool_settings.openai_image_api_key(),
                self.tool_settings.nano_banana_api_key(),
            )),
            todo_list_tool: Some(Arc::new(ToDoListTool::new())),
            question: Some(Arc::new(QuestionTool::new())),
            web_search: Arc::new(WebSearchTool::with_settings(
                self.tool_settings.web_search_provider,
                self.tool_settings.linkup_api_key(),
            )),
            web_fetch: Arc::new(WebFetchTool::new()),
            skill: Arc::new(SkillTool::with_settings(
                self.workspace_root.clone(),
                self.skill_settings.clone(),
            )),
            mcp: Arc::new(McpToolRegistry::new(self.mcp_settings.clone())),
            browser: Arc::new(BrowserTools::new(
                self.workspace_root.to_string_lossy().to_string(),
                sinew_browser::BrowserSessions::new(),
            )),
            workspace_memory: Arc::new(WorkspaceMemoryTool::new(self.workspace_root.clone())),
            semantic_search: Arc::new(SemanticSearchTool::new(self.workspace_root.clone())),
            doc_tool: Arc::new(DocTool::new(self.workspace_root.clone())),
            subagents: None,
            teams: None,
            tool_settings: self.tool_settings.clone(),
            event_scope: Some(AgentEventScope {
                id: tool_call_id.to_string(),
                agent_id: agent.id.clone(),
                agent_name: agent.name.clone(),
                team_name: None,
                model: agent.model.clone(),
                initial_message,
            }),
            max_tool_rounds: self.max_tool_rounds,
            event_tx: parent_event_tx,
            cancel: self.cancel.clone(),
            cmd_rx: child_cmd_rx,
        };

        let output = Box::pin(run_turn(child_context)).await;
        let file_changes = file_changes_from_history(&output.history);

        let final_answer = final_assistant_text(&output.history)
            .unwrap_or_else(|| "Sub-agent finished without a final answer.".to_string());
        ToolRunResult::ok_with_meta(
            final_answer,
            file_changes,
            json!({
                "subagent": {
                    "id": agent.id,
                    "name": agent.name,
                    "model": agent.model,
                    "history": output.history,
                }
            }),
        )
    }
}

#[derive(Debug, Deserialize)]
struct SubAgentInput {
    prompt: String,
}

pub fn subagent_system_prompt(base: &str, agent: &SubAgentConfig) -> String {
    let prompt = agent.prompt.trim();
    let profile = if prompt.is_empty() {
        "No extra profile prompt was provided.".to_string()
    } else {
        prompt.to_string()
    };
    format!(
        "{base}\n\n<sub_agent_profile name=\"{}\">\nYou are a delegated sub-agent. Work independently in your own context window. Use the normal workspace tools when useful. Do not ask the user questions; if you are blocked, explain the blocker in your final answer. When finished, return a concise final report for the main agent.\n\n{profile}\n</sub_agent_profile>",
        escape_attr(&agent.name)
    )
}

fn final_assistant_text(history: &[ChatMessage]) -> Option<String> {
    history.iter().rev().find_map(|message| {
        if !matches!(message.role, Role::Assistant) {
            return None;
        }
        let text = message
            .parts
            .iter()
            .filter_map(|part| match part {
                Part::Text { text, .. } if !text.trim().is_empty() => Some(text.trim()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        (!text.trim().is_empty()).then_some(text)
    })
}

fn file_changes_from_history(history: &[ChatMessage]) -> Vec<FileChange> {
    history
        .iter()
        .flat_map(|message| message.parts.iter())
        .filter_map(|part| match part {
            Part::ToolResult { meta, .. } => meta
                .as_ref()
                .and_then(|meta| meta.get("file_changes"))
                .and_then(|value| serde_json::from_value::<Vec<FileChange>>(value.clone()).ok()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn descriptor_description(agent: &SubAgentConfig) -> String {
    let desc = agent.description.trim();
    if desc.is_empty() {
        format!("Delegate a focused task to the {} sub-agent.", agent.name)
    } else {
        desc.to_string()
    }
}

pub fn is_subagent_tool_name(name: &str) -> bool {
    name.starts_with(TOOL_PREFIX)
}

pub fn subagent_summary(name: &str, settings: &SubAgentSettings) -> Option<String> {
    settings
        .agents
        .iter()
        .find(|agent| tool_name_for_agent(agent) == name)
        .map(|agent| format!("Sub-agent · {}", agent.name))
}

fn tool_name_for_agent(agent: &SubAgentConfig) -> String {
    format!("{TOOL_PREFIX}{}", slug(&agent.id))
}

fn clean_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn slug(value: &str) -> String {
    let slug = value
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    if slug.is_empty() {
        "agent".to_string()
    } else {
        slug
    }
}

fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedSubAgentImport {
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSubAgentsResult {
    pub imported: Vec<String>,
    pub skipped: Vec<SkippedSubAgentImport>,
}

struct AgentMarkdownFile {
    name: String,
    description: String,
    prompt: String,
    model: ModelRef,
    source: SubAgentSource,
    source_path: PathBuf,
    id: String,
}

/// Merge Claude Code sub-agent definitions from `.claude/agents/` into settings.
pub fn import_sub_agents_from_provider(
    workspace_root: impl AsRef<Path>,
    provider: &str,
    current: &SubAgentSettings,
    default_model: &ModelRef,
) -> Result<(SubAgentSettings, ImportSubAgentsResult)> {
    match provider.trim().to_ascii_lowercase().as_str() {
        "claude" => import_claude_sub_agents(workspace_root, current, default_model),
        other => bail!("unknown sub-agent import provider `{other}` (expected claude)"),
    }
}

fn import_claude_sub_agents(
    workspace_root: impl AsRef<Path>,
    current: &SubAgentSettings,
    default_model: &ModelRef,
) -> Result<(SubAgentSettings, ImportSubAgentsResult)> {
    let workspace_root = workspace_root.as_ref();
    let home = BaseDirs::new()
        .map(|base| base.home_dir().to_path_buf())
        .context("unable to locate home directory")?;

    let mut discovered = Vec::new();
    collect_claude_agent_files(
        &workspace_root.join(".claude/agents"),
        SubAgentSource::Workspace,
        default_model,
        &mut discovered,
    )?;
    collect_claude_agent_files(
        &home.join(".claude/agents"),
        SubAgentSource::Global,
        default_model,
        &mut discovered,
    )?;

    let mut settings = current.clone();
    let mut imported = Vec::new();
    let mut skipped = Vec::new();
    let mut seen_ids = settings
        .agents
        .iter()
        .map(|agent| agent.id.clone())
        .collect::<HashSet<_>>();
    let mut seen_names = settings
        .agents
        .iter()
        .map(|agent| agent.name.to_ascii_lowercase())
        .collect::<HashSet<_>>();

    for file in discovered {
        if seen_ids.contains(&file.id) || seen_names.contains(&file.name.to_ascii_lowercase()) {
            skipped.push(SkippedSubAgentImport {
                name: file.name.clone(),
                reason: "already configured in Sinew".into(),
            });
            continue;
        }
        seen_ids.insert(file.id.clone());
        seen_names.insert(file.name.to_ascii_lowercase());
        imported.push(file.name.clone());
        settings.agents.push(SubAgentConfig {
            id: file.id,
            name: file.name,
            description: file.description,
            prompt: file.prompt,
            model: file.model,
            enabled: true,
            source: Some(file.source),
            source_path: Some(display_path_string(&file.source_path)),
        });
    }

    imported.sort_unstable();
    Ok((settings.normalized(), ImportSubAgentsResult { imported, skipped }))
}

fn collect_claude_agent_files(
    root: &Path,
    source: SubAgentSource,
    default_model: &ModelRef,
    out: &mut Vec<AgentMarkdownFile>,
) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    collect_claude_agent_files_recursive(root, source, default_model, out)
}

fn collect_claude_agent_files_recursive(
    dir: &Path,
    source: SubAgentSource,
    default_model: &ModelRef,
    out: &mut Vec<AgentMarkdownFile>,
) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("unable to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_claude_agent_files_recursive(&path, source, default_model, out)?;
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let Some(parsed) = parse_claude_agent_file(&path, source, default_model) else {
            continue;
        };
        out.push(parsed);
    }
    Ok(())
}

fn parse_claude_agent_file(
    path: &Path,
    source: SubAgentSource,
    fallback_model: &ModelRef,
) -> Option<AgentMarkdownFile> {
    let content = fs::read_to_string(path).ok()?;
    let frontmatter = parse_frontmatter(&content);
    let fallback_name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("agent")
        .to_string();
    let name = frontmatter
        .get("name")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_name);
    let description = frontmatter
        .get("description")
        .map(|value| value.trim().to_string())
        .unwrap_or_default();
    let prompt = strip_frontmatter(&content).trim().to_string();
    if prompt.is_empty() && description.is_empty() {
        return None;
    }
    let model = frontmatter
        .get("model")
        .map(|value| map_claude_model_alias(value, &fallback_model))
        .unwrap_or_else(|| fallback_model.clone());
    let id = claude_agent_id(&name);
    Some(AgentMarkdownFile {
        name,
        description,
        prompt,
        model,
        source,
        source_path: path.to_path_buf(),
        id,
    })
}

fn map_claude_model_alias(alias: &str, default_model: &ModelRef) -> ModelRef {
    let trimmed = alias.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("inherit") {
        return default_model.clone();
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("opus") {
        return ModelRef::new("anthropic", "claude-opus-4-8");
    }
    if lower.contains("sonnet") {
        return ModelRef::new("anthropic", "claude-sonnet-4-6");
    }
    if lower.contains("haiku") {
        return ModelRef::new("anthropic", "claude-haiku-4-5");
    }
    if lower.contains("gpt") || lower.contains("codex") {
        return ModelRef::new("openai", "gpt-5.5");
    }
    default_model.clone()
}

fn claude_agent_id(name: &str) -> String {
    let mut slug = String::new();
    for ch in name.trim().to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
        } else if (ch == '-' || ch == '_') && !slug.ends_with('-') {
            slug.push('-');
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "agent".to_string()
    } else {
        slug
    }
}

fn strip_frontmatter(content: &str) -> String {
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return content.to_string();
    }
    for line in lines.by_ref() {
        if line.trim() == "---" {
            return lines.collect::<Vec<_>>().join("\n");
        }
    }
    content.to_string()
}

fn parse_frontmatter(content: &str) -> HashMap<String, String> {
    let mut fields = HashMap::new();
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return fields;
    }
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        fields.insert(
            key.trim().to_string(),
            clean_yaml_string(value.trim()).to_string(),
        );
    }
    fields
}

fn clean_yaml_string(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
}

fn display_path_string(path: &Path) -> String {
    let raw = path.display().to_string();
    if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = raw.strip_prefix(r"\\?\") {
        return rest.to_string();
    }
    raw
}

#[cfg(test)]
mod import_tests {
    use super::*;
    use std::fs;

    #[test]
    fn maps_claude_model_aliases() {
        let default = ModelRef::new("anthropic", "claude-sonnet-4-6");
        assert_eq!(
            map_claude_model_alias("opus", &default).name,
            "claude-opus-4-8"
        );
        assert_eq!(
            map_claude_model_alias("sonnet", &default).name,
            "claude-sonnet-4-6"
        );
        assert_eq!(
            map_claude_model_alias("inherit", &default).name,
            default.name
        );
    }

    #[test]
    fn parses_claude_agent_markdown() {
        let dir = std::env::temp_dir().join(format!(
            "sinew-agent-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("reviewer.md");
        fs::write(
            &path,
            "---\nname: code-reviewer\ndescription: Reviews diffs\nmodel: opus\n---\n\nYou review code.\n",
        )
        .unwrap();
        let default = ModelRef::new("anthropic", "claude-sonnet-4-6");
        let parsed =
            parse_claude_agent_file(&path, SubAgentSource::Workspace, &default).unwrap();
        assert_eq!(parsed.name, "code-reviewer");
        assert_eq!(parsed.description, "Reviews diffs");
        assert!(parsed.prompt.contains("You review code"));
        assert_eq!(parsed.id, "code-reviewer");
        let _ = fs::remove_dir_all(dir);
    }
}
