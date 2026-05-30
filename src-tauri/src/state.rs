use crate::*;

pub(super) const DEFAULT_SYSTEM_PROMPT: &str = "You are Sinew, a coding assistant. You build context by examining the codebase first without making assumptions or jumping to conclusions. ALWAYS check for a dedicated tool that fits the task before falling back to the shell/bash tool. You keep your responses concise without repeating yourself.";
pub(super) const DEFAULT_GIT_AUTOMATION_PROMPT: &str = "\
When possible, automate Git maintenance: check whether the opened project is up to date, pull if it is behind, \
and push after successful modifications so the user mostly manages ideas, not Git.";
pub(super) const DEFAULT_CONCISE_ANSWERS_PROMPT: &str = "\
User preference: the user is a power user, not a coder. Keep answers simple, concise, and action-oriented. \
Strictly avoid technical developer jargon (such as API REST, State Hook, Mutex, Serde, IPC, etc.). \
Instead, explain concepts using real-world analogies and everyday metaphors (e.g., comparing a database to a filing cabinet). \
Keep your explanations simple, direct, and ultra-concise without repeating yourself.";
pub(super) const DEFAULT_AGENT_AUTONOMY_PROMPT: &str = "\
Agent Autonomy Mode is enabled. Always follow these rules strictly: \
- If you can perform a task, run a tool, inspect a file, search the workspace, check diagnostics, or run a test YOURSELF, DO IT DIRECTLY. \
- DO NOT ask the user for permission, clarification, or help to do something that is within your capability or toolset. \
- NEVER write textual instructions or command lines telling the user how to run a command, compile code, edit a file, or configure their system if you have a tool (like bash, edit_file, write_file) capable of doing it yourself. ALWAYS run the tools first yourself. Act, do not explain how to act. \
- Never ask the user to find, read, edit, or check logs/files manually if you can access them with your tools (such as read, grep, glob, bash, etc.). \
- Proactively use all available tools to resolve the user's objective without requiring manual user intervention.";
pub(super) const DEFAULT_STRICT_PROBLEM_SOLVING_PROMPT: &str = "\
Strict Problem Solving Mode is enabled. Never bypass, hide, or ignore errors and warnings. Always dig down to the root cause and implement the real, correct solution, even if it requires more effort or reading more files.";
pub(super) const DEFAULT_FULL_IMPLEMENTATION_PROMPT: &str = "\
Full Implementation Mode is enabled. Never leave TODOs, placeholders, or fake/mock code. Everything you write must be completely wired up, functional, and ready for production use immediately.";

pub(super) const WORKSPACE_INSTRUCTIONS_FILE: &str = "AGENTS.md";
pub(super) const WORKSPACE_DESIGN_FILE: &str = "DESIGN.md";
pub(super) const AGENT_EVENT_NAME: &str = "agent-event";
pub(super) const FILE_CHANGE_EVENT_NAME: &str = "workspace-file-changed";
pub(super) const TERMINAL_DATA_EVENT_NAME: &str = "terminal-data";
pub(super) const TERMINAL_EXIT_EVENT_NAME: &str = "terminal-exit";
pub(super) const TERMINAL_OPEN_EVENT_NAME: &str = "terminal-open-requested";
pub(super) const ACTIVE_TURNS_EVENT_NAME: &str = "active-turns-changed";
pub(super) const TERMINAL_OPEN_MENU_ID: &str = "terminal-open";
pub(super) const CLOSE_ACTIVE_TAB_MENU_ID: &str = "close-active-tab";
pub(super) const NEW_WINDOW_MENU_ID: &str = "new-window";
pub(super) const CLOSE_ACTIVE_TAB_EVENT_NAME: &str = "editor-close-active-tab-requested";
pub(super) const NEW_WINDOW_LABEL_PREFIX: &str = "sinew-window";
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
    pub(super) active_turn_inputs: Arc<Mutex<HashMap<String, ActiveTurnInputRecord>>>,
    pub(super) active_turn_details: Arc<StdMutex<HashMap<String, ActiveTurnRecord>>>,
    pub(super) team_runtime: Arc<RwLock<TeamRuntime>>,
    pub(super) file_watchers: Arc<Mutex<HashMap<String, RecommendedWatcher>>>,
    pub(super) terminal_sessions: Arc<Mutex<HashMap<String, TerminalProcess>>>,
    pub(super) openai_login: Arc<Mutex<Option<OpenAiLoginAttempt>>>,
    pub(super) anthropic_login: Arc<Mutex<Option<AnthropicLoginAttempt>>>,
    pub(super) google_login: Arc<Mutex<Option<GoogleLoginAttempt>>>,
    pub(super) kimi_login: Arc<Mutex<Option<KimiLoginAttempt>>>,
    pub(super) cursor_login: Arc<Mutex<Option<CursorLoginAttempt>>>,
    pub(super) editor_diagnostics: SharedEditorDiagnosticsStore,
}

#[derive(Clone)]
pub(super) struct ActiveTurnInputRecord {
    pub(super) workspace_id: String,
    pub(super) conversation_id: String,
    pub(super) workspace_root: PathBuf,
}

#[derive(Clone)]
pub(super) struct ActiveTurnRecord {
    pub(super) workspace_id: String,
    pub(super) conversation_id: String,
    pub(super) started_at_ms: i64,
    pub(super) events: Vec<SequencedAgentEvent>,
    pub(super) replay_events: Vec<SequencedAgentEvent>,
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
    #[allow(dead_code)]
    pub(super) target_key: Option<String>,
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
    #[allow(dead_code)]
    pub(super) target_key: Option<String>,
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

#[derive(Clone)]
pub(super) struct CursorLoginAttempt {
    pub(super) id: String,
    pub(super) cancel: Arc<Notify>,
    pub(super) outcome: Arc<StdMutex<Option<CursorLoginOutcome>>>,
}

#[derive(Clone)]
pub(super) struct CursorLoginOutcome {
    pub(super) success: bool,
    pub(super) error: Option<String>,
}
