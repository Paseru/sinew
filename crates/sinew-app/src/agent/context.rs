use std::{path::PathBuf, sync::Arc};

use tokio::sync::mpsc;

use sinew_core::{ChatMessage, Provider, ServiceTier};

use crate::{
    BashTool, CheckSotaTool, CreateImageTool, EditFileTool, GlobTool, GoalWorkflowState, GrepTool,
    McpToolRegistry, QuestionTool, ReadTool, SkillTool, SubAgentTool, TeamTool, ToDoListTool,
    TodoListState, ToolSettings, WebFetchTool, WebSearchTool, WriteFileTool,
};

use super::{
    cancel::{EngineCommand, SteeringCommand, TurnCancel},
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
    pub workspace_root: PathBuf,
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
    pub check_sota: Arc<CheckSotaTool>,
    pub read: Arc<ReadTool>,
    pub edit_file: Arc<EditFileTool>,
    pub write_file: Arc<WriteFileTool>,
    pub create_image: Arc<CreateImageTool>,
    pub todo_list_tool: Option<Arc<ToDoListTool>>,
    pub question: Option<Arc<QuestionTool>>,
    pub web_search: Arc<WebSearchTool>,
    pub web_fetch: Arc<WebFetchTool>,
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
    pub steering_rx: Option<mpsc::UnboundedReceiver<SteeringCommand>>,
}

pub struct TurnOutput {
    pub history: Vec<ChatMessage>,
    pub todo_list: TodoListState,
    pub goal_workflow: GoalWorkflowState,
    pub interrupted: bool,
    pub compacted: bool,
}
