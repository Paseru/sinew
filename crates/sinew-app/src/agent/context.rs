use std::sync::Arc;

use tokio::sync::mpsc;

use sinew_core::{ChatMessage, Provider, ServiceTier};

use crate::{
    BashTool, BrowserTools, CreateImageTool, DocTool, EditFileTool, GlobTool, GoalWorkflowState,
    GrepTool, McpToolRegistry, QuestionTool, ReadTool, SemanticSearchTool, SkillTool, SubAgentTool,
    TeamTool, ToDoListTool, TodoListState, ToolSettings, WebFetchTool, WebSearchTool,
    WorkspaceMemoryTool, WriteFileTool,
};

use super::{
    cancel::{EngineCommand, TurnCancel},
    events::{AgentEvent, AgentEventScope},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentMode {
    #[default]
    Act,
    Plan,
    Goal,
}

pub struct TurnContext {
    pub provider: Arc<dyn Provider>,
    pub model: sinew_core::ModelRef,
    pub cache_key: Option<String>,
    pub cache_stable_message_count: usize,
    pub service_tier: Option<ServiceTier>,
    pub auto_compact: bool,
    pub mode: AgentMode,
    pub stop_questions: bool,
    pub system_prompt: String,
    pub history: Vec<ChatMessage>,
    pub todo_list: TodoListState,
    pub goal_workflow: GoalWorkflowState,
    pub bash: Arc<BashTool>,
    pub glob: Arc<GlobTool>,
    pub grep: Arc<GrepTool>,
    pub read: Arc<ReadTool>,
    pub edit_file: Arc<EditFileTool>,
    pub write_file: Arc<WriteFileTool>,
    pub create_image: Arc<CreateImageTool>,
    pub todo_list_tool: Option<Arc<ToDoListTool>>,
    pub question: Option<Arc<QuestionTool>>,
    pub web_search: Arc<WebSearchTool>,
    pub web_fetch: Arc<WebFetchTool>,
    pub browser: Arc<BrowserTools>,
    pub workspace_memory: Arc<WorkspaceMemoryTool>,
    pub semantic_search: Arc<SemanticSearchTool>,
    pub doc_tool: Arc<DocTool>,
    pub skill: Arc<SkillTool>,
    pub mcp: Arc<McpToolRegistry>,
    pub subagents: Option<Arc<SubAgentTool>>,
    pub teams: Option<Arc<TeamTool>>,
    pub tool_settings: ToolSettings,
    pub event_scope: Option<AgentEventScope>,
    pub max_tool_rounds: usize,
    pub event_tx: mpsc::UnboundedSender<AgentEvent>,
    pub cancel: TurnCancel,
    pub cmd_rx: mpsc::UnboundedReceiver<EngineCommand>,
}

pub struct TurnOutput {
    pub history: Vec<ChatMessage>,
    pub todo_list: TodoListState,
    pub goal_workflow: GoalWorkflowState,
    pub interrupted: bool,
    pub compacted: bool,
}
