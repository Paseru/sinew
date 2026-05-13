use super::*;

#[derive(Debug, Default)]
pub struct TeamRuntime {
    pub(super) scopes: HashMap<String, TeamScope>,
    pub(super) agent_notifiers: HashMap<String, Arc<Notify>>,
    pub(super) workspace_write_locks: HashMap<String, Arc<Semaphore>>,
}

#[derive(Debug, Default)]
pub(super) struct TeamScope {
    pub(super) active_team: Option<String>,
    pub(super) teams: HashMap<String, TeamSession>,
    pub(super) team_cancels: HashMap<String, Vec<TurnCancel>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamSession {
    pub name: String,
    pub description: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub agents: HashMap<String, TeamAgent>,
    pub tasks: Vec<TeamTask>,
    pub next_task_id: u64,
    pub queued_messages: Vec<TeamQueuedMessage>,
    pub next_message_id: u64,
    #[serde(default)]
    pub pending_task_wakes: Vec<TeamTaskWake>,
    #[serde(default)]
    pub recent_file_changes: Vec<TeamRecentFileChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamAgent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub prompt: String,
    pub model: ModelRef,
    pub status: TeamAgentStatus,
    pub history: Vec<ChatMessage>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub last_summary: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TeamAgentStatus {
    Idle,
    Running,
    Stopped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamTask {
    pub id: u64,
    pub subject: String,
    pub description: Option<String>,
    pub status: TeamTaskStatus,
    pub owner: Option<String>,
    pub blocked_by: Vec<u64>,
    pub created_by: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub completed_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamQueuedMessage {
    pub id: u64,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub target: Option<String>,
    pub message: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamTaskWake {
    pub task_id: u64,
    pub owner: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamRecentFileChange {
    pub agent: String,
    pub tool: String,
    pub relative_path: String,
    pub kind: FileChangeKind,
    pub added: usize,
    pub removed: usize,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TeamTaskStatus {
    #[serde(alias = "todo")]
    Pending,
    InProgress,
    Blocked,
    #[serde(alias = "done", alias = "resolved")]
    Completed,
}

#[derive(Clone)]
pub struct TeamTool {
    pub(super) scope_id: String,
    pub(super) workspace_root: PathBuf,
    pub(super) system_prompt: String,
    pub(super) providers: HashMap<String, Arc<dyn Provider>>,
    pub(super) sub_agent_settings: SubAgentSettings,
    pub(super) mcp_settings: McpSettings,
    pub(super) tool_settings: ToolSettings,
    pub(super) skill_settings: SkillSettings,
    pub(super) default_model: ModelRef,
    pub(super) max_tool_rounds: usize,
    pub(super) runtime: Arc<RwLock<TeamRuntime>>,
    pub(super) cancel: TurnCancel,
    pub(super) current_agent: Option<TeamIdentity>,
}

#[derive(Clone)]
pub(super) struct TeamIdentity {
    pub(super) team_name: String,
    pub(super) agent_name: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct TeamRunInput {
    pub(super) objective: Option<String>,
    pub(super) agent: Option<String>,
    pub(super) agent_names: Option<Vec<String>>,
    #[serde(default, alias = "agentProfiles")]
    pub(super) agent_profiles: Option<AgentProfilesInput>,
    #[serde(default, alias = "agentPrompts")]
    pub(super) agent_prompts: Option<AgentPromptsInput>,
    pub(super) tasks: Option<Vec<TeamRunTaskInput>>,
    #[serde(flatten)]
    pub(super) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(super) enum AgentProfilesInput {
    Assignments(Vec<AgentProfileAssignmentInput>),
    Map(HashMap<String, String>),
}

impl AgentProfilesInput {
    pub(super) fn to_profile_map(&self) -> std::result::Result<HashMap<String, String>, String> {
        match self {
            Self::Map(map) => Ok(map.clone()),
            Self::Assignments(assignments) => {
                let mut map = HashMap::new();
                for assignment in assignments {
                    let agent = assignment.agent.trim();
                    let profile = assignment.profile.trim();
                    if agent.is_empty() || profile.is_empty() {
                        return Err("agent_profiles entries require non-empty agent and profile"
                            .to_string());
                    }
                    if map.insert(agent.to_string(), profile.to_string()).is_some() {
                        return Err(format!(
                            "agent_profiles contains duplicate teammate `{agent}`"
                        ));
                    }
                }
                Ok(map)
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AgentProfileAssignmentInput {
    pub(super) agent: String,
    pub(super) profile: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(super) enum AgentPromptsInput {
    Assignments(Vec<AgentPromptAssignmentInput>),
    Map(HashMap<String, String>),
}

impl AgentPromptsInput {
    pub(super) fn to_prompt_map(&self) -> std::result::Result<HashMap<String, String>, String> {
        match self {
            Self::Map(map) => Ok(map.clone()),
            Self::Assignments(assignments) => {
                let mut map = HashMap::new();
                let mut seen = BTreeSet::new();
                for assignment in assignments {
                    let agent = assignment.agent.trim();
                    let prompt = assignment.prompt.trim();
                    if agent.is_empty() || prompt.is_empty() {
                        return Err(
                            "agent_prompts entries require non-empty agent and prompt".to_string()
                        );
                    }
                    if !seen.insert(agent_key(agent)) {
                        return Err(format!(
                            "agent_prompts contains duplicate teammate `{agent}`"
                        ));
                    }
                    map.insert(agent.to_string(), prompt.to_string());
                }
                Ok(map)
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AgentPromptAssignmentInput {
    pub(super) agent: String,
    pub(super) prompt: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TeamRunTaskInput {
    pub(super) subject: String,
    pub(super) description: Option<String>,
    pub(super) owner: Option<String>,
    #[serde(default, rename = "blockedBy")]
    pub(super) blocked_by: Option<Vec<TaskIdInput>>,
    #[serde(default, rename = "blocked_by")]
    pub(super) blocked_by_snake: Option<Vec<TaskIdInput>>,
}

#[derive(Debug, Clone)]
pub(super) struct PreparedTeamRunTask {
    pub(super) subject: String,
    pub(super) description: Option<String>,
    pub(super) owner: Option<String>,
    pub(super) blocked_by: Vec<u64>,
}

#[derive(Debug, Clone)]
pub(super) struct PreparedTeamAgentConfig {
    pub(super) name: String,
    pub(super) description: String,
    pub(super) prompt: String,
    pub(super) model: ModelRef,
}

#[derive(Debug, Deserialize)]
pub(super) struct SendMessageInput {
    pub(super) to: String,
    pub(super) message: String,
    pub(super) team_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TeamNameInput {
    pub(super) team_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TeamStopInput {
    pub(super) team_name: Option<String>,
    pub(super) agent: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TaskCreateInput {
    pub(super) subject: String,
    pub(super) description: Option<String>,
    pub(super) owner: Option<String>,
    #[serde(default, rename = "blockedBy")]
    pub(super) blocked_by: Option<Vec<TaskIdInput>>,
    #[serde(default, rename = "blocked_by")]
    pub(super) blocked_by_snake: Option<Vec<TaskIdInput>>,
    pub(super) team_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TaskListInput {
    pub(super) team_name: Option<String>,
    pub(super) action: Option<TaskListAction>,
    pub(super) status: Option<String>,
    #[serde(rename = "taskId", alias = "id", alias = "task_id")]
    pub(super) task_id: Option<TaskIdInput>,
    pub(super) subject: Option<String>,
    pub(super) description: Option<String>,
    pub(super) owner: Option<String>,
    #[serde(default, rename = "blockedBy")]
    pub(super) blocked_by: Option<Vec<TaskIdInput>>,
    #[serde(default, rename = "blocked_by")]
    pub(super) blocked_by_snake: Option<Vec<TaskIdInput>>,
    #[serde(default, rename = "addBlockedBy")]
    pub(super) add_blocked_by: Option<Vec<TaskIdInput>>,
    #[serde(default, rename = "add_blocked_by")]
    pub(super) add_blocked_by_snake: Option<Vec<TaskIdInput>>,
    pub(super) clear_owner: Option<bool>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum TaskListAction {
    List,
    Create,
    Update,
    Delete,
    Claim,
}

#[derive(Debug, Deserialize)]
pub(super) struct TaskUpdateInput {
    #[serde(rename = "taskId", alias = "id", alias = "task_id")]
    pub(super) task_id: TaskIdInput,
    pub(super) team_name: Option<String>,
    pub(super) status: Option<TeamTaskStatus>,
    pub(super) owner: Option<String>,
    pub(super) subject: Option<String>,
    pub(super) description: Option<String>,
    #[serde(default, rename = "blockedBy")]
    pub(super) blocked_by: Option<Vec<TaskIdInput>>,
    #[serde(default, rename = "blocked_by")]
    pub(super) blocked_by_snake: Option<Vec<TaskIdInput>>,
    #[serde(default, rename = "addBlockedBy")]
    pub(super) add_blocked_by: Option<Vec<TaskIdInput>>,
    #[serde(default, rename = "add_blocked_by")]
    pub(super) add_blocked_by_snake: Option<Vec<TaskIdInput>>,
    pub(super) clear_owner: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(super) enum TaskIdInput {
    Number(u64),
    String(String),
}

impl TaskIdInput {
    pub(super) fn to_u64(&self) -> std::result::Result<u64, String> {
        match self {
            Self::Number(value) if *value > 0 => Ok(*value),
            Self::Number(_) => Err("task id must be greater than zero".to_string()),
            Self::String(value) => {
                let trimmed = value.trim().trim_start_matches('#');
                trimmed.parse::<u64>().map_err(|_| {
                    format!(
                        "invalid task id `{}`; expected a positive integer",
                        value.trim()
                    )
                })
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct TeamTurn {
    pub(super) agent_name: String,
    pub(super) message: String,
    pub(super) task_id: Option<u64>,
    pub(super) label: String,
}

#[derive(Debug, Default)]
pub(super) struct LiveAgentReport {
    pub(super) reports: Vec<String>,
    pub(super) file_changes: Vec<FileChange>,
    pub(super) images: Vec<ToolRunImage>,
    pub(super) last_meta: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TeamSnapshot {
    pub(super) name: String,
    pub(super) description: Option<String>,
    pub(super) agents: Vec<TeamAgentSnapshot>,
    pub(super) tasks: Vec<TeamTaskSnapshot>,
    pub(super) queued_messages: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TeamAgentSnapshot {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) status: TeamAgentStatus,
    pub(super) model: ModelRef,
    pub(super) last_summary: Option<String>,
    pub(super) last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TeamTaskSnapshot {
    pub(super) id: u64,
    pub(super) subject: String,
    pub(super) description: Option<String>,
    pub(super) status: TeamTaskStatus,
    pub(super) owner: Option<String>,
    pub(super) blocked_by: Vec<u64>,
    pub(super) updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TeamAgentFinalResponse {
    pub(super) agent: String,
    pub(super) status: String,
    pub(super) last_response: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) last_error: Option<String>,
}

impl TeamSnapshot {
    pub(super) fn from_session(session: &TeamSession) -> Self {
        let mut agents = session
            .agents
            .values()
            .map(TeamAgentSnapshot::from_agent)
            .collect::<Vec<_>>();
        agents.sort_by(|left, right| left.name.cmp(&right.name));
        let mut tasks = session
            .tasks
            .iter()
            .map(TeamTaskSnapshot::from_task)
            .collect::<Vec<_>>();
        tasks.sort_by_key(|task| task.id);
        Self {
            name: session.name.clone(),
            description: session.description.clone(),
            agents,
            tasks,
            queued_messages: session.queued_messages.len(),
        }
    }
}

impl TeamAgentSnapshot {
    pub(super) fn from_agent(agent: &TeamAgent) -> Self {
        Self {
            id: agent.id.clone(),
            name: agent.name.clone(),
            description: agent.description.clone(),
            status: agent.status,
            model: agent.model.clone(),
            last_summary: agent.last_summary.clone(),
            last_error: agent.last_error.clone(),
        }
    }
}

impl TeamTaskSnapshot {
    pub(super) fn from_task(task: &TeamTask) -> Self {
        Self {
            id: task.id,
            subject: task.subject.clone(),
            description: task.description.clone(),
            status: task.status,
            owner: task.owner.clone(),
            blocked_by: task.blocked_by.clone(),
            updated_at_ms: task.updated_at_ms,
        }
    }
}
