use crate::*;

pub(super) const DEFAULT_SYSTEM_PROMPT: &str = "You are Claake Code, a coding assistant. You build context by examining the codebase first without making assumptions or jumping to conclusions. When exploring, you provide user updates frequently, every 30s. ALWAYS check for a dedicated tool that fits the task before falling back to the shell/bash tool. You keep your responses concise without repeating yourself.";
pub(super) const WORKSPACE_INSTRUCTIONS_FILE: &str = "AGENTS.md";
pub(super) const WORKSPACE_DESIGN_FILE: &str = "DESIGN.md";
pub(super) const AGENT_EVENT_NAME: &str = "agent-event";
pub(super) const FILE_CHANGE_EVENT_NAME: &str = "workspace-file-changed";
pub(super) const TERMINAL_DATA_EVENT_NAME: &str = "terminal-data";
pub(super) const TERMINAL_EXIT_EVENT_NAME: &str = "terminal-exit";
pub(super) const TERMINAL_OPEN_EVENT_NAME: &str = "terminal-open-requested";
pub(super) const ACTIVE_TURNS_EVENT_NAME: &str = "active-turns-changed";
pub(super) const DATABASE_SOURCES_EVENT_NAME: &str = "database-sources-changed";
pub(super) const TERMINAL_OPEN_MENU_ID: &str = "terminal-open";
pub(super) const NEW_WINDOW_MENU_ID: &str = "new-window";
pub(super) const NEW_WINDOW_LABEL_PREFIX: &str = "claakecode-window";
pub(super) const NEW_WINDOW_URL: &str = "index.html?newWindow=1";
pub(super) const MAX_ATTACHMENT_BYTES: usize = 128 * 1024;
pub(super) const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
pub(super) const TURN_SLOT_WAIT_ATTEMPTS: usize = 30;
pub(super) const TURN_SLOT_WAIT_INTERVAL_MS: u64 = 50;
pub(super) const SWARM_WAKE_TURN_SLOT_WAIT_ATTEMPTS: usize = 600;
pub(super) const ACTIVE_TURN_EVENT_BUFFER_MAX: usize = 2_000;

#[cfg(target_os = "macos")]
pub(super) static MACOS_APP_HANDLE: std::sync::OnceLock<AppHandle> = std::sync::OnceLock::new();

pub(super) struct TerminalProcess {
    pub(super) token: String,
    pub(super) master: Box<dyn MasterPty + Send>,
    pub(super) writer: Arc<StdMutex<Box<dyn Write + Send>>>,
    pub(super) killer: Arc<StdMutex<Box<dyn ChildKiller + Send + Sync>>>,
}

#[derive(Clone)]
pub(super) struct DesktopState {
    pub(super) providers: Arc<StdMutex<HashMap<String, Arc<dyn Provider>>>>,
    pub(super) store: AppStore,
    pub(super) default_model: ModelRef,
    pub(super) system_prompt: String,
    pub(super) max_tool_rounds: usize,
    pub(super) active_turns: Arc<Mutex<HashMap<String, TurnCancel>>>,
    pub(super) active_turn_details: Arc<StdMutex<HashMap<String, ActiveTurnRecord>>>,
    pub(super) team_runtime: Arc<RwLock<TeamRuntime>>,
    pub(super) file_watchers: Arc<Mutex<HashMap<String, RecommendedWatcher>>>,
    pub(super) terminal_sessions: Arc<Mutex<HashMap<String, TerminalProcess>>>,
    pub(super) openai_login: Arc<Mutex<Option<OpenAiLoginAttempt>>>,
    pub(super) anthropic_login: Arc<Mutex<Option<AnthropicLoginAttempt>>>,
    pub(super) google_login: Arc<Mutex<Option<GoogleLoginAttempt>>>,
    pub(super) kimi_login: Arc<Mutex<Option<KimiLoginAttempt>>>,
}

#[derive(Clone)]
pub(super) struct ActiveTurnRecord {
    pub(super) workspace_id: String,
    pub(super) conversation_id: String,
    pub(super) started_at_ms: i64,
    pub(super) events: Vec<SequencedAgentEvent>,
    pub(super) next_sequence: u64,
}

impl ActiveTurnRecord {
    pub(super) fn latest_sequence(&self) -> u64 {
        self.next_sequence.saturating_sub(1)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SequencedAgentEvent {
    pub(super) sequence: u64,
    pub(super) event: AgentEvent,
}

#[derive(Clone)]
pub(super) struct OpenAiLoginAttempt {
    pub(super) id: String,
    pub(super) cancel: Arc<Notify>,
    pub(super) outcome: Arc<StdMutex<Option<OpenAiLoginOutcome>>>,
}

#[derive(Clone)]
pub(super) struct OpenAiLoginOutcome {
    pub(super) success: bool,
    pub(super) error: Option<String>,
}

#[derive(Clone)]
pub(super) struct AnthropicLoginAttempt {
    pub(super) id: String,
    pub(super) cancel: Arc<Notify>,
    pub(super) outcome: Arc<StdMutex<Option<AnthropicLoginOutcome>>>,
}

#[derive(Clone)]
pub(super) struct AnthropicLoginOutcome {
    pub(super) success: bool,
    pub(super) error: Option<String>,
}

#[derive(Clone)]
pub(super) struct GoogleLoginAttempt {
    pub(super) id: String,
    pub(super) cancel: Arc<Notify>,
    pub(super) outcome: Arc<StdMutex<Option<GoogleLoginOutcome>>>,
}

#[derive(Clone)]
pub(super) struct GoogleLoginOutcome {
    pub(super) success: bool,
    pub(super) error: Option<String>,
}

#[derive(Clone)]
pub(super) struct KimiLoginAttempt {
    pub(super) id: String,
    pub(super) cancel: Arc<Notify>,
    pub(super) outcome: Arc<StdMutex<Option<KimiLoginOutcome>>>,
}

#[derive(Clone)]
pub(super) struct KimiLoginOutcome {
    pub(super) success: bool,
    pub(super) error: Option<String>,
}
