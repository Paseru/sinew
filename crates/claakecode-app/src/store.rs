use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use claakecode_core::{ChatMessage, ModelRef, Part, Role, ToolDescriptor};
use uuid::Uuid;

use crate::agent::AgentMode;
use crate::bash::active_shell_display_name;
use crate::database::{DatabaseActivityEntry, DatabaseSettings};
use crate::mcp::McpSettings;
use crate::skill::SkillSettings;
use crate::subagent::SubAgentSettings;
use crate::todo::TodoListState;
use crate::tool_run::TurnCheckpoint;
use crate::workspace::{workspace_info, WorkspaceInfo};

const MODE_MODEL_SETTINGS_KEY: &str = "mode_model_settings";
const MCP_SETTINGS_KEY: &str = "mcp_settings";
const SUB_AGENT_SETTINGS_KEY: &str = "sub_agent_settings";
const TOOL_SETTINGS_KEY: &str = "tool_settings";
const SKILL_SETTINGS_KEY: &str = "skill_settings";
const DATABASE_SETTINGS_KEY: &str = "database_settings";
const DATABASE_ACTIVITY_KEY: &str = "database_activity";
const OPENROUTER_MODELS_KEY: &str = "openrouter_models";
const HIDDEN_TOOL_SETTING_NAMES: &[&str] = &["skill"];

pub const DEFAULT_PLAN_MODE_PROMPT: &str = r#"You are in Plan mode.

Rules:
- Build understanding by reading/searching/running diagnostic shell commands as needed.
- Do not edit workspace files.
- You must keep the user in a Question loop until the user explicitly clicks "Send and stop questions".
- If the user message does not contain <plan_mode_control action="stop_questions">, your turn must end by calling the Question tool. Do not write the final plan yet.
- After each normal answer to a Question, inspect/explore more if needed, then ask the next Question.
- If you have no remaining substantive question, ask the user to confirm that you should create the plan now. Still use the Question tool.
- Only when the user message contains <plan_mode_control action="stop_questions">, stop asking questions and write the complete plan now.
- When the plan is ready, respond with only the Markdown plan. Do not implement it.

STRICTLY FORBIDDEN in the plan (unless the user explicitly requests it):
- Code snippets, pseudo-code, or inline code
- File paths, directory structures, or tree views
- Function, class, variable, or module names
- Shell commands or CLI instructions
- Technical configuration details
- Any implementation-specific notation

The plan should read as a clear description of intent and expected behavior that anyone could understand without technical background. Bullet points and paragraphs are both acceptable. The focus is on WHAT the system should do, not HOW the code should be written.

If technical specifics become necessary to avoid ambiguity, the AI may include them at its discretion, integrated naturally into the plan - but this should remain the exception, not the default.

You may include Mermaid diagrams (in ```mermaid fenced blocks) when a flow, decision tree, sequence, or set of relationships would be clearer as a picture than as prose. Keep diagram labels at the same level of abstraction as the rest of the plan: describe intent and behavior, not files, functions, or implementation details."#;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct TurnCheckpointRecord {
    pub history_index: usize,
    pub checkpoint: TurnCheckpoint,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedConversation {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub model: ModelRef,
    pub mode_model_settings: ModeModelSettings,
    pub system_prompt: String,
    pub todo_list: TodoListState,
    pub plan_workflow: PlanWorkflowState,
    pub goal_workflow: GoalWorkflowState,
    pub history: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeModelSettings {
    pub act: ModelRef,
    pub plan: ModelRef,
    pub goal: ModelRef,
}

impl ModeModelSettings {
    pub fn new(default_model: &ModelRef) -> Self {
        Self {
            act: default_model.clone(),
            plan: default_model.clone(),
            goal: default_model.clone(),
        }
    }

    pub fn get(&self, mode: AgentMode) -> &ModelRef {
        match mode {
            AgentMode::Act => &self.act,
            AgentMode::Plan => &self.plan,
            AgentMode::Goal => &self.goal,
        }
    }

    pub fn set(&mut self, mode: AgentMode, model: ModelRef) {
        match mode {
            AgentMode::Act => self.act = model,
            AgentMode::Plan => self.plan = model,
            AgentMode::Goal => self.goal = model,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawModeModelSettings {
    act: ModelRef,
    plan: ModelRef,
    #[serde(default)]
    goal: Option<ModelRef>,
}

impl<'de> Deserialize<'de> for ModeModelSettings {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawModeModelSettings::deserialize(deserializer)?;
        Ok(Self {
            goal: raw.goal.unwrap_or_else(|| raw.act.clone()),
            act: raw.act,
            plan: raw.plan,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlanArtifactState {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub absolute_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "camelCase")]
#[derive(Default)]
pub enum PlanWorkflowState {
    #[default]
    Idle,
    PlanningQuestions,
    PlanReady {
        artifact: PlanArtifactState,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "camelCase")]
#[derive(Default)]
pub enum GoalWorkflowState {
    #[default]
    Idle,
    Active {
        objective: String,
        started_at_ms: i64,
        updated_at_ms: i64,
    },
    Paused {
        objective: String,
        started_at_ms: i64,
        updated_at_ms: i64,
    },
    Complete {
        objective: String,
        started_at_ms: i64,
        completed_at_ms: i64,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceBootstrap {
    pub workspace: WorkspaceInfo,
    pub conversations: Vec<ConversationSummary>,
    pub active_conversation: SavedConversation,
    pub mode_model_settings: ModeModelSettings,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSettings {
    #[serde(default)]
    pub tools: Vec<ToolConfig>,
    #[serde(default)]
    pub plan_mode_prompt: String,
    #[serde(default)]
    pub image_provider: ImageProvider,
    #[serde(default)]
    pub openai_image_use_subscription: bool,
    #[serde(default)]
    pub openai_image_api_key: String,
    #[serde(default)]
    pub nano_banana_api_key: String,
    #[serde(default)]
    pub web_search_provider: WebSearchProvider,
    #[serde(default)]
    pub linkup_api_key: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageProvider {
    #[default]
    #[serde(rename = "gptImage2")]
    GptImage2,
    #[serde(rename = "nanoBanana2")]
    NanoBanana2,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebSearchProvider {
    #[serde(rename = "linkup")]
    LinkUp,
    #[default]
    #[serde(rename = "classic")]
    Classic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolConfig {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub description_override: bool,
    #[serde(default, skip_serializing)]
    pub default_description: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSettingsView {
    pub tools: Vec<ToolConfigView>,
    pub plan_mode_prompt: String,
    pub default_plan_mode_prompt: String,
    pub image_provider: ImageProvider,
    pub openai_image_use_subscription: bool,
    pub openai_image_api_key: String,
    pub nano_banana_api_key: String,
    pub web_search_provider: WebSearchProvider,
    pub linkup_api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OpenRouterModelRecord {
    pub id: String,
    pub name: String,
    pub context_window: u32,
    pub max_output_tokens: u32,
    #[serde(default)]
    pub supports_images: bool,
    #[serde(default)]
    pub supports_thinking: bool,
    #[serde(default = "default_enabled")]
    pub supports_tools: bool,
    #[serde(default)]
    pub added_at_ms: i64,
}

impl OpenRouterModelRecord {
    pub fn normalized(mut self) -> Option<Self> {
        self.id = self.id.trim().to_string();
        self.name = self.name.trim().to_string();
        if self.id.is_empty() {
            return None;
        }
        if self.name.is_empty() {
            self.name = self.id.clone();
        }
        self.context_window = self.context_window.max(1);
        self.max_output_tokens = self.max_output_tokens.max(1).min(self.context_window);
        if self.added_at_ms <= 0 {
            self.added_at_ms = now_ms();
        }
        Some(self)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolConfigView {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub default_description: String,
    pub enabled: bool,
}

impl ToolSettings {
    pub fn normalized(mut self) -> Self {
        let mut seen = HashSet::new();
        self.plan_mode_prompt = normalize_plan_mode_prompt(&self.plan_mode_prompt);
        self.openai_image_api_key = self.openai_image_api_key.trim().to_string();
        self.nano_banana_api_key = self.nano_banana_api_key.trim().to_string();
        self.linkup_api_key = self.linkup_api_key.trim().to_string();
        self.tools = self
            .tools
            .into_iter()
            .filter_map(|mut tool| {
                tool.name = tool.name.trim().to_string();
                if tool.name.is_empty()
                    || HIDDEN_TOOL_SETTING_NAMES.contains(&tool.name.as_str())
                    || !seen.insert(tool.name.clone())
                {
                    return None;
                }
                tool.default_description.clear();
                if !tool.description_override {
                    tool.description.clear();
                }
                Some(tool)
            })
            .collect();
        self
    }

    pub fn normalized_for_catalog(mut self, catalog: &[ToolDescriptor]) -> Self {
        let defaults = catalog
            .iter()
            .map(|descriptor| (descriptor.name.as_str(), descriptor.description.as_str()))
            .collect::<HashMap<_, _>>();

        for tool in &mut self.tools {
            let name = tool.name.trim();
            if let Some(default_description) = defaults.get(name).copied().or_else(|| {
                (!tool.default_description.is_empty()).then_some(tool.default_description.as_str())
            }) {
                tool.description_override = tool.description != default_description;
            }
        }

        self.normalized()
    }

    pub fn apply_to_descriptors(&self, descriptors: Vec<ToolDescriptor>) -> Vec<ToolDescriptor> {
        let by_name = self
            .tools
            .iter()
            .map(|tool| (tool.name.as_str(), tool))
            .collect::<HashMap<_, _>>();

        descriptors
            .into_iter()
            .filter_map(|mut descriptor| {
                let setting = by_name.get(descriptor.name.as_str());
                let enabled = setting
                    .map(|tool| tool.enabled)
                    .unwrap_or_else(|| default_tool_enabled(&descriptor.name));
                if !enabled {
                    return None;
                }
                if let Some(setting) = setting.filter(|tool| tool.description_override) {
                    descriptor.description = setting.description.clone();
                }
                Some(descriptor)
            })
            .collect()
    }

    pub fn plan_mode_prompt(&self) -> &str {
        let prompt = self.plan_mode_prompt.trim();
        if prompt.is_empty() {
            DEFAULT_PLAN_MODE_PROMPT
        } else {
            prompt
        }
    }

    pub fn is_enabled(&self, name: &str) -> bool {
        self.tools
            .iter()
            .find(|tool| tool.name == name)
            .map(|tool| tool.enabled)
            .unwrap_or_else(|| default_tool_enabled(name))
    }

    pub fn openai_image_api_key(&self) -> Option<String> {
        let key = self.openai_image_api_key.trim();
        if key.is_empty() {
            None
        } else {
            Some(key.to_string())
        }
    }

    pub fn nano_banana_api_key(&self) -> Option<String> {
        let key = self.nano_banana_api_key.trim();
        if key.is_empty() {
            None
        } else {
            Some(key.to_string())
        }
    }

    pub fn linkup_api_key(&self) -> Option<String> {
        let key = self.linkup_api_key.trim();
        if key.is_empty() {
            None
        } else {
            Some(key.to_string())
        }
    }
}

fn normalize_plan_mode_prompt(value: &str) -> String {
    let prompt = value.trim();
    if prompt.is_empty() || prompt == DEFAULT_PLAN_MODE_PROMPT.trim() {
        String::new()
    } else {
        prompt.to_string()
    }
}

pub fn tool_settings_view(settings: &ToolSettings, catalog: &[ToolDescriptor]) -> ToolSettingsView {
    let by_name = settings
        .tools
        .iter()
        .map(|tool| (tool.name.as_str(), tool))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();

    ToolSettingsView {
        plan_mode_prompt: settings.plan_mode_prompt().to_string(),
        default_plan_mode_prompt: DEFAULT_PLAN_MODE_PROMPT.to_string(),
        image_provider: settings.image_provider,
        openai_image_use_subscription: settings.openai_image_use_subscription,
        openai_image_api_key: settings.openai_image_api_key.clone(),
        nano_banana_api_key: settings.nano_banana_api_key.clone(),
        web_search_provider: settings.web_search_provider,
        linkup_api_key: settings.linkup_api_key.clone(),
        tools: catalog
            .iter()
            .filter_map(|descriptor| {
                if !seen.insert(descriptor.name.clone()) {
                    return None;
                }
                let setting = by_name.get(descriptor.name.as_str());
                Some(ToolConfigView {
                    name: descriptor.name.clone(),
                    display_name: tool_display_name(&descriptor.name),
                    description: setting
                        .filter(|tool| tool.description_override)
                        .map(|tool| tool.description.clone())
                        .unwrap_or_else(|| descriptor.description.clone()),
                    default_description: descriptor.description.clone(),
                    enabled: setting
                        .map(|tool| tool.enabled)
                        .unwrap_or_else(|| default_tool_enabled(&descriptor.name)),
                })
            })
            .collect(),
    }
}

fn tool_display_name(name: &str) -> String {
    match name {
        "bash" => active_shell_display_name().to_string(),
        "bash_input" => format!("{} input", active_shell_display_name()),
        _ => default_tool_display_name(name),
    }
}

fn default_tool_display_name(name: &str) -> String {
    match name {
        "read" => "Read".to_string(),
        "edit_file" => "Edit file".to_string(),
        "write_file" => "Write file".to_string(),
        "Glob" => "Glob".to_string(),
        "Grep" => "Grep".to_string(),
        "WebSearch" => "Web search".to_string(),
        "WebFetch" => "Web fetch".to_string(),
        "CreateImage" => "Create image".to_string(),
        "database_list_sources" => "List database sources".to_string(),
        "database_describe_schema" => "Describe database schema".to_string(),
        "database_execute_query" => "Execute database query".to_string(),
        "Question" => "Question".to_string(),
        "ToDoList" => "To-do list".to_string(),
        "LoadMcpTool" => "Load MCP tool".to_string(),
        "LoadSkill" => "Load skill".to_string(),
        "TeamRun" => "Team run".to_string(),
        "TeamStatus" => "Team status".to_string(),
        "TeamStop" => "Team stop".to_string(),
        "SendMessage" => "Send message".to_string(),
        "clean_context" => "Clean context".to_string(),
        "update_goal" => "Update goal".to_string(),
        "context_compaction" => "Compact context".to_string(),
        _ => humanize_tool_name(name),
    }
}

fn humanize_tool_name(name: &str) -> String {
    let mut out = String::new();
    let mut previous_was_separator = true;
    let mut previous_was_lowercase = false;

    for ch in name.chars() {
        if ch == '_' || ch == '-' || ch.is_whitespace() {
            if !out.ends_with(' ') && !out.is_empty() {
                out.push(' ');
            }
            previous_was_separator = true;
            previous_was_lowercase = false;
            continue;
        }
        if ch.is_uppercase() && previous_was_lowercase && !out.ends_with(' ') {
            out.push(' ');
        }
        if previous_was_separator {
            out.extend(ch.to_uppercase());
        } else {
            out.extend(ch.to_lowercase());
        }
        previous_was_separator = false;
        previous_was_lowercase = ch.is_lowercase();
    }

    let trimmed = out.trim();
    if trimmed.is_empty() {
        name.to_string()
    } else {
        trimmed.to_string()
    }
}

#[derive(Debug, Clone)]
pub struct AppStore {
    path: PathBuf,
}

impl AppStore {
    pub fn open_default() -> Result<Self> {
        let dirs = ProjectDirs::from("dev", "williampeynichou", "claakecode")
            .context("unable to resolve local data directory")?;
        std::fs::create_dir_all(dirs.data_local_dir())
            .context("unable to create local data directory")?;

        let store = Self {
            path: dirs.data_local_dir().join("desktop-state.sqlite3"),
        };
        store.migrate()?;
        Ok(store)
    }

    /// Build an in-memory store backed by a unique temporary SQLite file.
    /// Intended for tests only.
    pub fn in_memory() -> Result<Self> {
        let dir = std::env::temp_dir();
        let unique = format!(
            "claakecode-test-{}-{}.sqlite3",
            std::process::id(),
            now_ms()
        );
        let store = Self {
            path: dir.join(unique),
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn bootstrap_workspace(
        &self,
        workspace_root: &Path,
        default_model: &ModelRef,
        default_system: &str,
    ) -> Result<WorkspaceBootstrap> {
        let workspace_id = workspace_root.display().to_string();
        let mode_model_settings = self.load_mode_model_settings(default_model)?;
        let mut conversations = self.list_conversations(&workspace_id)?;
        let active_conversation = if let Some(first) = conversations.first() {
            self.load_conversation(&workspace_id, &first.id)?
                .context("conversation listed in index but missing from store")?
        } else {
            let created = self.create_conversation(&workspace_id, default_model, default_system)?;
            conversations = self.list_conversations(&workspace_id)?;
            created
        };

        Ok(WorkspaceBootstrap {
            workspace: workspace_info(workspace_root),
            conversations,
            active_conversation,
            mode_model_settings,
        })
    }

    pub fn create_conversation(
        &self,
        workspace_id: &str,
        default_model: &ModelRef,
        default_system: &str,
    ) -> Result<SavedConversation> {
        let id = Uuid::new_v4().to_string();
        let now = now_ms();
        let title = "New conversation".to_string();
        let todo_list = TodoListState::default();
        let todo_list_json = serde_json::to_string(&todo_list)?;
        let plan_workflow = PlanWorkflowState::default();
        let plan_workflow_json = serde_json::to_string(&plan_workflow)?;
        let goal_workflow = GoalWorkflowState::default();
        let goal_workflow_json = serde_json::to_string(&goal_workflow)?;
        let mode_model_settings = self.load_mode_model_settings(default_model)?;
        let conversation_model = mode_model_settings.act.clone();
        let mode_model_settings_json = serde_json::to_string(&mode_model_settings)?;
        let conn = self.connection()?;
        conn.execute(
            "insert into conversations (id, workspace_id, title, model_json, mode_model_settings_json, system_prompt, todo_list_json, plan_workflow_json, goal_workflow_json, created_at_ms, updated_at_ms)
             values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                &id,
                workspace_id,
                &title,
                serde_json::to_string(&conversation_model)?,
                mode_model_settings_json,
                default_system,
                todo_list_json,
                plan_workflow_json,
                goal_workflow_json,
                now,
                now,
            ],
        )
        .context("unable to insert conversation")?;

        Ok(SavedConversation {
            id,
            workspace_id: workspace_id.to_string(),
            title,
            model: conversation_model,
            mode_model_settings,
            system_prompt: default_system.to_string(),
            todo_list,
            plan_workflow,
            goal_workflow,
            history: Vec::new(),
        })
    }

    pub fn list_conversations(&self, workspace_id: &str) -> Result<Vec<ConversationSummary>> {
        let conn = self.connection()?;
        let mut statement = conn
            .prepare(
                "select id, title, updated_at_ms from conversations
                 where workspace_id = ?1
                 order by updated_at_ms desc",
            )
            .context("unable to prepare conversation list query")?;

        let rows = statement
            .query_map(params![workspace_id], |row| {
                Ok(ConversationSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    updated_at_ms: row.get(2)?,
                })
            })
            .context("unable to read conversation list")?;

        let mut conversations = Vec::new();
        for row in rows {
            conversations.push(row.context("bad conversation row")?);
        }
        Ok(conversations)
    }

    pub fn load_conversation(
        &self,
        workspace_id: &str,
        id: &str,
    ) -> Result<Option<SavedConversation>> {
        let conn = self.connection()?;
        let conversation = conn
            .query_row(
                "select title, model_json, system_prompt, todo_list_json, plan_workflow_json, mode_model_settings_json, goal_workflow_json from conversations where workspace_id = ?1 and id = ?2",
                params![workspace_id, id],
                |row| {
                    let model_json: String = row.get(1)?;
                    let todo_list_json: String = row.get(3)?;
                    let plan_workflow_json: String = row.get(4)?;
                    let mode_model_settings_json: Option<String> = row.get(5)?;
                    let goal_workflow_json: String = row.get(6)?;
                    let model = serde_json::from_str::<ModelRef>(&model_json).map_err(|err| {
                        rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Text,
                            Box::new(err),
                        )
                    })?;
                    let mode_model_settings = mode_model_settings_json
                        .and_then(|json| serde_json::from_str::<ModeModelSettings>(&json).ok())
                        .unwrap_or_else(|| ModeModelSettings::new(&model));
                    let mut todo_list = serde_json::from_str::<TodoListState>(&todo_list_json)
                        .unwrap_or_default();
                    todo_list.normalize();
                    Ok((
                        row.get::<_, String>(0)?,
                        model,
                        mode_model_settings,
                        row.get::<_, String>(2)?,
                        todo_list,
                        serde_json::from_str::<PlanWorkflowState>(&plan_workflow_json)
                            .unwrap_or_default(),
                        serde_json::from_str::<GoalWorkflowState>(&goal_workflow_json)
                            .unwrap_or_default(),
                    ))
                },
            )
            .optional()
            .context("unable to load conversation metadata")?;

        let Some((
            title,
            model,
            mode_model_settings,
            system_prompt,
            todo_list,
            plan_workflow,
            goal_workflow,
        )) = conversation
        else {
            return Ok(None);
        };

        let mut statement = conn
            .prepare(
                "select message_json from messages
                 where conversation_id = ?1
                 order by ordinal asc",
            )
            .context("unable to prepare message query")?;
        let rows = statement
            .query_map(params![id], |row| {
                let message_json: String = row.get(0)?;
                serde_json::from_str::<ChatMessage>(&message_json).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })
            })
            .context("unable to read stored messages")?;

        let mut history = Vec::new();
        for row in rows {
            history.push(row.context("bad stored message")?);
        }

        Ok(Some(SavedConversation {
            id: id.to_string(),
            workspace_id: workspace_id.to_string(),
            title,
            model,
            mode_model_settings,
            system_prompt,
            todo_list,
            plan_workflow,
            goal_workflow,
            history,
        }))
    }

    pub fn save_conversation(&self, conversation: &SavedConversation) -> Result<()> {
        let now = now_ms();
        let title =
            title_from_history(&conversation.history).unwrap_or_else(|| conversation.title.clone());
        let mut todo_list = conversation.todo_list.clone();
        todo_list.normalize();
        let todo_list_json = serde_json::to_string(&todo_list)?;
        let plan_workflow_json = serde_json::to_string(&conversation.plan_workflow)?;
        let goal_workflow_json = serde_json::to_string(&conversation.goal_workflow)?;
        let mode_model_settings_json = serde_json::to_string(&conversation.mode_model_settings)?;
        let mut conn = self.connection()?;
        let tx = conn
            .transaction()
            .context("unable to open sqlite transaction")?;

        tx.execute(
            "update conversations
             set title = ?2, model_json = ?3, system_prompt = ?4, updated_at_ms = ?5, todo_list_json = ?6, plan_workflow_json = ?7, mode_model_settings_json = ?8, goal_workflow_json = ?9
             where id = ?1 and workspace_id = ?10",
            params![
                &conversation.id,
                &title,
                serde_json::to_string(&conversation.model)?,
                &conversation.system_prompt,
                now,
                todo_list_json,
                plan_workflow_json,
                mode_model_settings_json,
                goal_workflow_json,
                &conversation.workspace_id,
            ],
        )
        .context("unable to update conversation")?;

        tx.execute(
            "delete from messages where conversation_id = ?1",
            params![&conversation.id],
        )
        .context("unable to clear previous conversation messages")?;

        for (ordinal, message) in conversation.history.iter().enumerate() {
            tx.execute(
                "insert into messages (conversation_id, ordinal, message_json) values (?1, ?2, ?3)",
                params![
                    &conversation.id,
                    ordinal as i64,
                    serde_json::to_string(message)?
                ],
            )
            .context("unable to write conversation message")?;
        }

        tx.commit()
            .context("unable to commit conversation transaction")?;
        Ok(())
    }

    pub fn save_conversation_and_mode_model_settings(
        &self,
        conversation: &SavedConversation,
        settings: &ModeModelSettings,
    ) -> Result<()> {
        let now = now_ms();
        let title =
            title_from_history(&conversation.history).unwrap_or_else(|| conversation.title.clone());
        let mut todo_list = conversation.todo_list.clone();
        todo_list.normalize();
        let todo_list_json = serde_json::to_string(&todo_list)?;
        let plan_workflow_json = serde_json::to_string(&conversation.plan_workflow)?;
        let goal_workflow_json = serde_json::to_string(&conversation.goal_workflow)?;
        let mode_model_settings_json = serde_json::to_string(&conversation.mode_model_settings)?;
        let default_settings_json = serde_json::to_string(settings)?;
        let mut conn = self.connection()?;
        let tx = conn
            .transaction()
            .context("unable to open sqlite transaction")?;

        tx.execute(
            "update conversations
             set title = ?2, model_json = ?3, system_prompt = ?4, updated_at_ms = ?5, todo_list_json = ?6, plan_workflow_json = ?7, mode_model_settings_json = ?8, goal_workflow_json = ?9
             where id = ?1 and workspace_id = ?10",
            params![
                &conversation.id,
                &title,
                serde_json::to_string(&conversation.model)?,
                &conversation.system_prompt,
                now,
                todo_list_json,
                plan_workflow_json,
                mode_model_settings_json,
                goal_workflow_json,
                &conversation.workspace_id,
            ],
        )
        .context("unable to update conversation")?;

        tx.execute(
            "delete from messages where conversation_id = ?1",
            params![&conversation.id],
        )
        .context("unable to clear previous conversation messages")?;

        for (ordinal, message) in conversation.history.iter().enumerate() {
            tx.execute(
                "insert into messages (conversation_id, ordinal, message_json) values (?1, ?2, ?3)",
                params![
                    &conversation.id,
                    ordinal as i64,
                    serde_json::to_string(message)?
                ],
            )
            .context("unable to write conversation message")?;
        }

        tx.execute(
            "insert into app_settings (key, value_json, updated_at_ms)
             values (?1, ?2, ?3)
             on conflict(key) do update set
                value_json = excluded.value_json,
                updated_at_ms = excluded.updated_at_ms",
            params![MODE_MODEL_SETTINGS_KEY, default_settings_json, now],
        )
        .context("unable to save mode model settings")?;

        tx.commit()
            .context("unable to commit conversation/settings transaction")?;
        Ok(())
    }

    pub fn append_conversation_message(
        &self,
        workspace_id: &str,
        conversation_id: &str,
        message: &ChatMessage,
    ) -> Result<()> {
        let now = now_ms();
        let mut conn = self.connection()?;
        let tx = conn
            .transaction()
            .context("unable to open sqlite transaction")?;
        let next_ordinal: i64 = tx
            .query_row(
                "select coalesce(max(ordinal) + 1, 0) from messages where conversation_id = ?1",
                params![conversation_id],
                |row| row.get(0),
            )
            .context("unable to read next message ordinal")?;
        tx.execute(
            "insert into messages (conversation_id, ordinal, message_json) values (?1, ?2, ?3)",
            params![
                conversation_id,
                next_ordinal,
                serde_json::to_string(message)?
            ],
        )
        .context("unable to append conversation message")?;
        tx.execute(
            "update conversations set updated_at_ms = ?3 where workspace_id = ?1 and id = ?2",
            params![workspace_id, conversation_id, now],
        )
        .context("unable to update conversation timestamp")?;
        tx.commit()
            .context("unable to commit append message transaction")?;
        Ok(())
    }

    pub fn save_turn_checkpoint(
        &self,
        conversation_id: &str,
        history_index: usize,
        checkpoint: &TurnCheckpoint,
    ) -> Result<()> {
        let conn = self.connection()?;
        if checkpoint.files.is_empty() {
            conn.execute(
                "delete from turn_checkpoints where conversation_id = ?1 and history_index = ?2",
                params![conversation_id, history_index as i64],
            )
            .context("unable to clear empty turn checkpoint")?;
            return Ok(());
        }

        conn.execute(
            "insert into turn_checkpoints (conversation_id, history_index, checkpoint_json)
             values (?1, ?2, ?3)
             on conflict(conversation_id, history_index) do update set
                checkpoint_json = excluded.checkpoint_json",
            params![
                conversation_id,
                history_index as i64,
                serde_json::to_string(checkpoint)?,
            ],
        )
        .context("unable to save turn checkpoint")?;
        Ok(())
    }

    pub fn load_turn_checkpoints_from(
        &self,
        conversation_id: &str,
        history_index: usize,
    ) -> Result<Vec<TurnCheckpointRecord>> {
        let conn = self.connection()?;
        let mut statement = conn
            .prepare(
                "select history_index, checkpoint_json from turn_checkpoints
                 where conversation_id = ?1 and history_index >= ?2
                 order by history_index asc",
            )
            .context("unable to prepare turn checkpoint query")?;
        let rows = statement
            .query_map(params![conversation_id, history_index as i64], |row| {
                let checkpoint_json: String = row.get(1)?;
                let checkpoint =
                    serde_json::from_str::<TurnCheckpoint>(&checkpoint_json).map_err(|err| {
                        rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Text,
                            Box::new(err),
                        )
                    })?;
                let stored_index: i64 = row.get(0)?;
                Ok(TurnCheckpointRecord {
                    history_index: stored_index.max(0) as usize,
                    checkpoint,
                })
            })
            .context("unable to read turn checkpoints")?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row.context("bad turn checkpoint row")?);
        }
        Ok(records)
    }

    pub fn delete_turn_checkpoints_from(
        &self,
        conversation_id: &str,
        history_index: usize,
    ) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "delete from turn_checkpoints where conversation_id = ?1 and history_index >= ?2",
            params![conversation_id, history_index as i64],
        )
        .context("unable to delete turn checkpoints")?;
        Ok(())
    }

    pub fn load_mode_model_settings(&self, default_model: &ModelRef) -> Result<ModeModelSettings> {
        let conn = self.connection()?;
        load_mode_model_settings_from_conn(&conn, default_model)
    }

    pub fn save_mode_model_settings(&self, settings: &ModeModelSettings) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "insert into app_settings (key, value_json, updated_at_ms)
             values (?1, ?2, ?3)
             on conflict(key) do update set
                value_json = excluded.value_json,
                updated_at_ms = excluded.updated_at_ms",
            params![
                MODE_MODEL_SETTINGS_KEY,
                serde_json::to_string(settings)?,
                now_ms(),
            ],
        )
        .context("unable to save mode model settings")?;
        Ok(())
    }

    pub fn load_mcp_settings(&self) -> Result<McpSettings> {
        let conn = self.connection()?;
        let stored = conn
            .query_row(
                "select value_json from app_settings where key = ?1",
                params![MCP_SETTINGS_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("unable to read MCP settings")?;

        if let Some(json) = stored {
            if let Ok(settings) = serde_json::from_str::<McpSettings>(&json) {
                return Ok(settings);
            }
        }

        Ok(McpSettings::default())
    }

    pub fn save_mcp_settings(&self, settings: &McpSettings) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "insert into app_settings (key, value_json, updated_at_ms)
             values (?1, ?2, ?3)
             on conflict(key) do update set
                value_json = excluded.value_json,
                updated_at_ms = excluded.updated_at_ms",
            params![MCP_SETTINGS_KEY, serde_json::to_string(settings)?, now_ms()],
        )
        .context("unable to save MCP settings")?;
        Ok(())
    }

    pub fn load_tool_settings(&self) -> Result<ToolSettings> {
        let conn = self.connection()?;
        let stored = conn
            .query_row(
                "select value_json from app_settings where key = ?1",
                params![TOOL_SETTINGS_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("unable to read tool settings")?;

        if let Some(json) = stored {
            if let Ok(settings) = serde_json::from_str::<ToolSettings>(&json) {
                return Ok(settings.normalized());
            }
        }

        Ok(ToolSettings::default())
    }

    pub fn save_tool_settings(&self, settings: &ToolSettings) -> Result<ToolSettings> {
        self.save_tool_settings_for_catalog(settings, &[])
    }

    pub fn save_tool_settings_for_catalog(
        &self,
        settings: &ToolSettings,
        catalog: &[ToolDescriptor],
    ) -> Result<ToolSettings> {
        let normalized = settings.clone().normalized_for_catalog(catalog);
        let conn = self.connection()?;
        conn.execute(
            "insert into app_settings (key, value_json, updated_at_ms)
             values (?1, ?2, ?3)
             on conflict(key) do update set
                value_json = excluded.value_json,
                updated_at_ms = excluded.updated_at_ms",
            params![
                TOOL_SETTINGS_KEY,
                serde_json::to_string(&normalized)?,
                now_ms()
            ],
        )
        .context("unable to save tool settings")?;
        Ok(normalized)
    }

    pub fn load_database_settings(&self) -> Result<DatabaseSettings> {
        let conn = self.connection()?;
        let stored = conn
            .query_row(
                "select value_json from app_settings where key = ?1",
                params![DATABASE_SETTINGS_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("unable to read database settings")?;

        if let Some(json) = stored {
            if let Ok(settings) = serde_json::from_str::<DatabaseSettings>(&json) {
                return Ok(settings.normalized());
            }
        }

        Ok(DatabaseSettings::default())
    }

    pub fn save_database_settings(&self, settings: &DatabaseSettings) -> Result<DatabaseSettings> {
        let normalized = settings.clone().normalized_for_save()?;
        let conn = self.connection()?;
        conn.execute(
            "insert into app_settings (key, value_json, updated_at_ms)
             values (?1, ?2, ?3)
             on conflict(key) do update set
                value_json = excluded.value_json,
                updated_at_ms = excluded.updated_at_ms",
            params![
                DATABASE_SETTINGS_KEY,
                serde_json::to_string(&normalized)?,
                now_ms(),
            ],
        )
        .context("unable to save database settings")?;
        Ok(normalized)
    }

    pub fn list_database_source_activity(
        &self,
        source_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<DatabaseActivityEntry>> {
        let mut entries = self.load_database_activity_entries()?;
        let source_id = source_id.trim();
        if !source_id.is_empty() {
            entries.retain(|entry| entry.source_id == source_id);
        }
        entries.sort_by(|left, right| right.timestamp_ms.cmp(&left.timestamp_ms));
        if let Some(limit) = limit.filter(|limit| *limit > 0) {
            entries.truncate(limit);
        }
        Ok(entries)
    }

    pub fn append_database_source_activity(&self, entry: &DatabaseActivityEntry) -> Result<()> {
        let mut entries = self.load_database_activity_entries()?;
        entries.push(entry.clone().normalized());
        entries.sort_by(|left, right| right.timestamp_ms.cmp(&left.timestamp_ms));
        entries.truncate(500);
        self.save_database_activity_entries(&entries)
    }

    pub fn clear_database_source_activity(
        &self,
        source_id: &str,
    ) -> Result<Vec<DatabaseActivityEntry>> {
        let source_id = source_id.trim();
        let entries = self
            .load_database_activity_entries()?
            .into_iter()
            .filter(|entry| entry.source_id != source_id)
            .collect::<Vec<_>>();
        self.save_database_activity_entries(&entries)?;
        Ok(Vec::new())
    }

    fn load_database_activity_entries(&self) -> Result<Vec<DatabaseActivityEntry>> {
        let conn = self.connection()?;
        let stored = conn
            .query_row(
                "select value_json from app_settings where key = ?1",
                params![DATABASE_ACTIVITY_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("unable to read database activity")?;

        if let Some(json) = stored {
            if let Ok(entries) = serde_json::from_str::<Vec<DatabaseActivityEntry>>(&json) {
                return Ok(entries
                    .into_iter()
                    .map(DatabaseActivityEntry::normalized)
                    .collect());
            }
        }

        Ok(Vec::new())
    }

    fn save_database_activity_entries(&self, entries: &[DatabaseActivityEntry]) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "insert into app_settings (key, value_json, updated_at_ms)
             values (?1, ?2, ?3)
             on conflict(key) do update set
                value_json = excluded.value_json,
                updated_at_ms = excluded.updated_at_ms",
            params![
                DATABASE_ACTIVITY_KEY,
                serde_json::to_string(entries)?,
                now_ms()
            ],
        )
        .context("unable to save database activity")?;
        Ok(())
    }

    pub fn load_skill_settings(&self) -> Result<SkillSettings> {
        let conn = self.connection()?;
        let stored = conn
            .query_row(
                "select value_json from app_settings where key = ?1",
                params![SKILL_SETTINGS_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("unable to read skill settings")?;

        if let Some(json) = stored {
            if let Ok(settings) = serde_json::from_str::<SkillSettings>(&json) {
                return Ok(settings.normalized());
            }
        }

        Ok(SkillSettings::default())
    }

    pub fn save_skill_settings(&self, settings: &SkillSettings) -> Result<SkillSettings> {
        let normalized = settings.clone().normalized();
        let conn = self.connection()?;
        conn.execute(
            "insert into app_settings (key, value_json, updated_at_ms)
             values (?1, ?2, ?3)
             on conflict(key) do update set
                value_json = excluded.value_json,
                updated_at_ms = excluded.updated_at_ms",
            params![
                SKILL_SETTINGS_KEY,
                serde_json::to_string(&normalized)?,
                now_ms()
            ],
        )
        .context("unable to save skill settings")?;
        Ok(normalized)
    }

    pub fn load_sub_agent_settings(&self) -> Result<SubAgentSettings> {
        let conn = self.connection()?;
        let stored = conn
            .query_row(
                "select value_json from app_settings where key = ?1",
                params![SUB_AGENT_SETTINGS_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("unable to read sub-agent settings")?;

        if let Some(json) = stored {
            if let Ok(settings) = serde_json::from_str::<SubAgentSettings>(&json) {
                return Ok(settings.normalized());
            }
        }

        Ok(SubAgentSettings::default())
    }

    pub fn load_openrouter_models(&self) -> Result<Vec<OpenRouterModelRecord>> {
        let conn = self.connection()?;
        let stored = conn
            .query_row(
                "select value_json from app_settings where key = ?1",
                params![OPENROUTER_MODELS_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("unable to read OpenRouter model list")?;

        if let Some(json) = stored {
            if let Ok(models) = serde_json::from_str::<Vec<OpenRouterModelRecord>>(&json) {
                return Ok(normalize_openrouter_models(models));
            }
        }

        Ok(Vec::new())
    }

    pub fn save_openrouter_models(
        &self,
        models: &[OpenRouterModelRecord],
    ) -> Result<Vec<OpenRouterModelRecord>> {
        let normalized = normalize_openrouter_models(models.to_vec());
        let conn = self.connection()?;
        conn.execute(
            "insert into app_settings (key, value_json, updated_at_ms)
             values (?1, ?2, ?3)
             on conflict(key) do update set
                value_json = excluded.value_json,
                updated_at_ms = excluded.updated_at_ms",
            params![
                OPENROUTER_MODELS_KEY,
                serde_json::to_string(&normalized)?,
                now_ms(),
            ],
        )
        .context("unable to save OpenRouter model list")?;
        Ok(normalized)
    }

    pub fn add_openrouter_model(
        &self,
        model: OpenRouterModelRecord,
    ) -> Result<Vec<OpenRouterModelRecord>> {
        let Some(model) = model.normalized() else {
            anyhow::bail!("OpenRouter model id cannot be empty");
        };
        let mut models = self.load_openrouter_models()?;
        if !models.iter().any(|existing| existing.id == model.id) {
            models.push(model);
        }
        self.save_openrouter_models(&models)
    }

    pub fn remove_openrouter_model(&self, id: &str) -> Result<Vec<OpenRouterModelRecord>> {
        let id = id.trim();
        let models = self
            .load_openrouter_models()?
            .into_iter()
            .filter(|model| model.id != id)
            .collect::<Vec<_>>();
        self.save_openrouter_models(&models)
    }

    pub fn save_sub_agent_settings(&self, settings: &SubAgentSettings) -> Result<SubAgentSettings> {
        let normalized = settings.clone().normalized();
        let conn = self.connection()?;
        conn.execute(
            "insert into app_settings (key, value_json, updated_at_ms)
             values (?1, ?2, ?3)
             on conflict(key) do update set
                value_json = excluded.value_json,
                updated_at_ms = excluded.updated_at_ms",
            params![
                SUB_AGENT_SETTINGS_KEY,
                serde_json::to_string(&normalized)?,
                now_ms(),
            ],
        )
        .context("unable to save sub-agent settings")?;
        Ok(normalized)
    }

    pub fn rename_conversation(&self, workspace_id: &str, id: &str, title: &str) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "update conversations set title = ?3, updated_at_ms = ?4 where workspace_id = ?1 and id = ?2",
            params![workspace_id, id, title.trim(), now_ms()],
        )
        .context("unable to rename conversation")?;
        Ok(())
    }

    pub fn delete_conversation(&self, workspace_id: &str, id: &str) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "delete from conversations where workspace_id = ?1 and id = ?2",
            params![workspace_id, id],
        )
        .context("unable to delete conversation")?;
        Ok(())
    }

    pub fn load_conversation_model_by_id(&self, id: &str) -> Result<Option<ModelRef>> {
        let conn = self.connection()?;
        conn.query_row(
            "select model_json from conversations where id = ?1",
            params![id],
            |row| {
                let model_json: String = row.get(0)?;
                serde_json::from_str::<ModelRef>(&model_json).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })
            },
        )
        .optional()
        .context("unable to load conversation model")
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.connection()?;
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap_or(0);

        if version >= 8 {
            return Ok(());
        }

        if version < 2 {
            conn.execute_batch(
                "
            create table if not exists conversations (
                id text primary key,
                workspace_id text not null,
                title text not null,
                model_json text not null,
                mode_model_settings_json text,
                system_prompt text not null,
                todo_list_json text not null default '{\"active\":false,\"tasks\":[],\"nextId\":1}',
                plan_workflow_json text not null default '{\"status\":\"idle\"}',
                goal_workflow_json text not null default '{\"status\":\"idle\"}',
                created_at_ms integer not null,
                updated_at_ms integer not null
            );

            create table if not exists messages (
                conversation_id text not null,
                ordinal integer not null,
                message_json text not null,
                primary key (conversation_id, ordinal),
                foreign key (conversation_id) references conversations(id) on delete cascade
            );

            create index if not exists idx_conversations_workspace_updated
                on conversations(workspace_id, updated_at_ms desc);

            create table if not exists app_settings (
                key text primary key,
                value_json text not null,
                updated_at_ms integer not null
            );
            ",
            )
            .context("unable to migrate sqlite schema")?;
        }
        ensure_conversations_todo_column(&conn)?;
        ensure_conversations_plan_workflow_column(&conn)?;
        ensure_conversations_goal_workflow_column(&conn)?;
        ensure_conversations_mode_model_settings_column(&conn)?;
        ensure_app_settings_table(&conn)?;
        ensure_turn_checkpoints_table(&conn)?;
        if version < 8 {
            conn.execute("delete from turn_checkpoints", [])
                .context("unable to clear legacy turn checkpoints")?;
        }
        conn.pragma_update(None, "user_version", 8)
            .context("unable to set sqlite schema version")?;
        Ok(())
    }

    fn connection(&self) -> Result<Connection> {
        let conn = Connection::open(&self.path).context("unable to open sqlite database")?;
        conn.execute_batch("pragma foreign_keys = on;")
            .context("unable to enable foreign keys")?;
        Ok(conn)
    }
}

fn ensure_conversations_todo_column(conn: &Connection) -> Result<()> {
    if conversation_has_column(conn, "todo_list_json")? {
        return Ok(());
    }
    conn.execute_batch(
        r#"
        alter table conversations
            add column todo_list_json text not null
            default '{"active":false,"tasks":[],"nextId":1}';
        "#,
    )
    .context("unable to add todo list state column")?;
    Ok(())
}

fn ensure_conversations_plan_workflow_column(conn: &Connection) -> Result<()> {
    if conversation_has_column(conn, "plan_workflow_json")? {
        return Ok(());
    }
    conn.execute_batch(
        r#"
        alter table conversations
            add column plan_workflow_json text not null
            default '{"status":"idle"}';
        "#,
    )
    .context("unable to add plan workflow state column")?;
    Ok(())
}

fn ensure_conversations_goal_workflow_column(conn: &Connection) -> Result<()> {
    if conversation_has_column(conn, "goal_workflow_json")? {
        return Ok(());
    }
    conn.execute_batch(
        r#"
        alter table conversations
            add column goal_workflow_json text not null
            default '{"status":"idle"}';
        "#,
    )
    .context("unable to add goal workflow state column")?;
    Ok(())
}

fn ensure_conversations_mode_model_settings_column(conn: &Connection) -> Result<()> {
    if conversation_has_column(conn, "mode_model_settings_json")? {
        return Ok(());
    }
    conn.execute_batch(
        r#"
        alter table conversations
            add column mode_model_settings_json text;
        "#,
    )
    .context("unable to add mode model settings column")?;
    Ok(())
}

fn ensure_app_settings_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        create table if not exists app_settings (
            key text primary key,
            value_json text not null,
            updated_at_ms integer not null
        );
        "#,
    )
    .context("unable to create app settings table")?;
    Ok(())
}

fn ensure_turn_checkpoints_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        create table if not exists turn_checkpoints (
            conversation_id text not null,
            history_index integer not null,
            checkpoint_json text not null,
            primary key (conversation_id, history_index),
            foreign key (conversation_id) references conversations(id) on delete cascade
        );
        "#,
    )
    .context("unable to create turn checkpoint table")?;
    Ok(())
}

fn default_enabled() -> bool {
    true
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn default_tool_enabled(name: &str) -> bool {
    !matches!(name, "CreateImage" | "WebSearch")
}

fn normalize_openrouter_models(models: Vec<OpenRouterModelRecord>) -> Vec<OpenRouterModelRecord> {
    let mut seen = HashSet::new();
    models
        .into_iter()
        .filter_map(OpenRouterModelRecord::normalized)
        .filter(|model| seen.insert(model.id.clone()))
        .collect()
}

fn load_mode_model_settings_from_conn(
    conn: &Connection,
    default_model: &ModelRef,
) -> Result<ModeModelSettings> {
    let stored = conn
        .query_row(
            "select value_json from app_settings where key = ?1",
            params![MODE_MODEL_SETTINGS_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .context("unable to read mode model settings")?;

    if let Some(json) = stored {
        if let Ok(settings) = serde_json::from_str::<ModeModelSettings>(&json) {
            return Ok(settings);
        }
    }

    let latest_conversation_settings = conn
        .query_row(
            "select mode_model_settings_json from conversations
             where mode_model_settings_json is not null
             order by updated_at_ms desc
             limit 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .context("unable to read latest conversation model settings")?;

    if let Some(json) = latest_conversation_settings {
        if let Ok(settings) = serde_json::from_str::<ModeModelSettings>(&json) {
            return Ok(settings);
        }
    }

    Ok(ModeModelSettings::new(default_model))
}

fn conversation_has_column(conn: &Connection, name: &str) -> Result<bool> {
    let mut statement = conn
        .prepare("pragma table_info(conversations)")
        .context("unable to inspect conversations table")?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .context("unable to read conversations columns")?;

    for row in rows {
        if row.context("bad conversations column row")? == name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn title_from_history(history: &[ChatMessage]) -> Option<String> {
    history
        .iter()
        .filter(|message| matches!(message.role, Role::User))
        .find_map(|message| {
            message.parts.iter().find_map(|part| match part {
                Part::Text { text, meta }
                    if !title_hidden_text(meta) && !text.trim().is_empty() =>
                {
                    Some(text.trim().to_string())
                }
                _ => None,
            })
        })
        .map(|title| {
            if title.chars().count() <= 48 {
                title
            } else {
                let mut shortened = title.chars().take(45).collect::<String>();
                shortened.push_str("...");
                shortened
            }
        })
}

fn title_hidden_text(meta: &Option<Value>) -> bool {
    let Some(Value::Object(meta)) = meta else {
        return false;
    };
    meta.get("ui_only").and_then(Value::as_bool) == Some(true)
        || meta
            .get("compaction_retained_user")
            .and_then(Value::as_bool)
            == Some(true)
        || meta.get("compaction_summary").and_then(Value::as_bool) == Some(true)
        || meta.get("system_reminder").and_then(Value::as_bool) == Some(true)
        || meta.get("attachment_context").and_then(Value::as_bool) == Some(true)
        || meta.get("plan_control").and_then(Value::as_str).is_some()
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
    use serde_json::json;

    fn message(role: Role, text: &str, meta: Option<Value>) -> ChatMessage {
        ChatMessage {
            role,
            parts: vec![Part::Text {
                text: text.to_string(),
                meta,
            }],
        }
    }

    #[test]
    fn title_from_history_uses_first_visible_user_text() {
        let history = vec![
            message(Role::Assistant, "Assistant text", None),
            message(
                Role::User,
                "Hidden system reminder",
                Some(json!({ "system_reminder": true })),
            ),
            message(Role::User, "Real user request", None),
        ];

        assert_eq!(
            title_from_history(&history).as_deref(),
            Some("Real user request")
        );
    }

    #[test]
    fn title_from_history_ignores_assistant_when_no_visible_user_text() {
        let history = vec![
            message(
                Role::User,
                "Implement completely this plan",
                Some(json!({ "system_reminder": true })),
            ),
            message(Role::Assistant, "I'll start implementing the plan.", None),
        ];

        assert_eq!(title_from_history(&history), None);
    }

    fn descriptor(name: &str, description: &str) -> ToolDescriptor {
        ToolDescriptor {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: json!({ "type": "object" }),
        }
    }

    #[test]
    fn tool_settings_ignore_legacy_saved_descriptions_without_user_override() {
        let settings = ToolSettings {
            tools: vec![ToolConfig {
                name: "edit_file".to_string(),
                description: "old default from database".to_string(),
                enabled: true,
                description_override: false,
                default_description: String::new(),
            }],
            ..ToolSettings::default()
        }
        .normalized();

        let tools =
            settings.apply_to_descriptors(vec![descriptor("edit_file", "new code default")]);

        assert_eq!(tools[0].description, "new code default");
    }

    #[test]
    fn tool_settings_persist_only_descriptions_that_differ_from_catalog_default() {
        let settings = ToolSettings {
            tools: vec![
                ToolConfig {
                    name: "read".to_string(),
                    description: "read default".to_string(),
                    enabled: true,
                    description_override: false,
                    default_description: "read default".to_string(),
                },
                ToolConfig {
                    name: "edit_file".to_string(),
                    description: "custom edit instructions".to_string(),
                    enabled: true,
                    description_override: false,
                    default_description: "edit default".to_string(),
                },
            ],
            ..ToolSettings::default()
        }
        .normalized_for_catalog(&[
            descriptor("read", "read default"),
            descriptor("edit_file", "edit default"),
        ]);

        assert_eq!(settings.tools[0].description, "");
        assert!(!settings.tools[0].description_override);
        assert_eq!(settings.tools[1].description, "custom edit instructions");
        assert!(settings.tools[1].description_override);
    }

    #[test]
    fn tool_settings_apply_user_description_override() {
        let settings = ToolSettings {
            tools: vec![ToolConfig {
                name: "edit_file".to_string(),
                description: "custom edit instructions".to_string(),
                enabled: true,
                description_override: true,
                default_description: String::new(),
            }],
            ..ToolSettings::default()
        }
        .normalized();

        let tools =
            settings.apply_to_descriptors(vec![descriptor("edit_file", "new code default")]);

        assert_eq!(tools[0].description, "custom edit instructions");
    }

    #[test]
    fn database_settings_round_trip_and_activity_cap() {
        use crate::database::{
            DatabaseActivityEntry, DatabaseActivityOperation, DatabaseActivityStatus,
            DatabaseSettings, DatabaseSourceConfig, DatabaseSourceEngine,
        };

        let store = AppStore::in_memory().expect("in-memory store");
        let settings = DatabaseSettings {
            sources: vec![DatabaseSourceConfig {
                id: "src-1".into(),
                name: "primary".into(),
                engine: DatabaseSourceEngine::Sqlite,
                enabled: true,
                sqlite: crate::database::DatabaseSqliteConfig {
                    file_path: "/tmp/claake-test.sqlite".into(),
                    create_if_missing: true,
                },
                ..DatabaseSourceConfig::default()
            }],
        };
        let saved = store
            .save_database_settings(&settings)
            .expect("save database settings");
        assert_eq!(saved.sources.len(), 1);
        let loaded = store.load_database_settings().expect("load database settings");
        assert_eq!(loaded.sources.len(), 1);
        assert_eq!(loaded.sources[0].name, "primary");

        for i in 0..600 {
            let entry = DatabaseActivityEntry {
                id: format!("e-{i}"),
                source_id: "src-1".into(),
                source_name: "primary".into(),
                engine: DatabaseSourceEngine::Sqlite,
                operation: DatabaseActivityOperation::Read,
                query_preview: String::new(),
                status: DatabaseActivityStatus::Ok,
                timestamp_ms: i as i64,
                duration_ms: 1,
                rows_returned: None,
                rows_affected: None,
                error: None,
            };
            store
                .append_database_source_activity(&entry)
                .expect("append activity");
        }
        let listed = store
            .list_database_source_activity("src-1", Some(1000))
            .expect("list activity");
        assert!(listed.len() <= 500, "activity cap 500 (got {})", listed.len());
        // Most recent first.
        assert_eq!(listed.first().unwrap().id, "e-599");

        store
            .clear_database_source_activity("src-1")
            .expect("clear activity");
        let listed = store
            .list_database_source_activity("src-1", None)
            .expect("relist activity");
        assert!(listed.is_empty());
    }

    #[test]
    fn database_settings_normalize_and_validate_unique_names() {
        use crate::database::{
            DatabaseSettings, DatabaseSourceConfig, DatabaseSourceEngine, DatabaseSqliteConfig,
        };
        let settings = DatabaseSettings {
            sources: vec![DatabaseSourceConfig {
                id: "".to_string(),
                name: " local ".to_string(),
                engine: DatabaseSourceEngine::Sqlite,
                sqlite: DatabaseSqliteConfig {
                    file_path: "/tmp/example.sqlite".to_string(),
                    create_if_missing: true,
                },
                default_row_limit: 50_000,
                default_timeout_ms: 10,
                ..DatabaseSourceConfig::default()
            }],
        }
        .normalized_for_save()
        .expect("valid database settings");

        assert_eq!(settings.sources[0].name, "local");
        assert!(!settings.sources[0].id.is_empty());
        assert_eq!(settings.sources[0].default_row_limit, 10_000);
        assert_eq!(settings.sources[0].default_timeout_ms, 1_000);

        let duplicate = DatabaseSettings {
            sources: vec![
                DatabaseSourceConfig {
                    id: "a".to_string(),
                    name: "Prod".to_string(),
                    engine: DatabaseSourceEngine::Sqlite,
                    sqlite: DatabaseSqliteConfig {
                        file_path: "/tmp/a.sqlite".to_string(),
                        create_if_missing: true,
                    },
                    ..DatabaseSourceConfig::default()
                },
                DatabaseSourceConfig {
                    id: "b".to_string(),
                    name: "prod".to_string(),
                    engine: DatabaseSourceEngine::Sqlite,
                    sqlite: DatabaseSqliteConfig {
                        file_path: "/tmp/b.sqlite".to_string(),
                        create_if_missing: true,
                    },
                    ..DatabaseSourceConfig::default()
                },
            ],
        };
        assert!(duplicate.normalized_for_save().is_err());
    }
}
