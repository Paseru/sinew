/// Canonical tool identifiers exposed to models.
///
/// Sinew used to expose a mixed casing surface (`Glob`, `WebSearch`,
/// `ToDoList`, ...). Keep those legacy spellings accepted at dispatch and
/// in saved settings/history, but expose and store the snake_case names below.
pub const BASH: &str = "bash";
pub const BASH_INPUT: &str = "bash_input";
pub const READ: &str = "read";
pub const GLOB: &str = "glob";
pub const GREP: &str = "grep";
pub const EDIT_FILE: &str = "edit_file";
pub const WRITE_FILE: &str = "write_file";
pub const WEB_SEARCH: &str = "web_search";
pub const WEB_FETCH: &str = "web_fetch";
pub const CREATE_IMAGE: &str = "create_image";
pub const QUESTION: &str = "question";
pub const TODO_LIST: &str = "todo_list";
pub const CLEAN_CONTEXT: &str = "clean_context";
pub const LOAD_MCP_TOOL: &str = "load_mcp_tool";
pub const SKILL: &str = "skill";
pub const UPDATE_GOAL: &str = "update_goal";
pub const CONTEXT_COMPACTION: &str = "context_compaction";

pub const BROWSER_OPEN: &str = "browser_open";
pub const BROWSER_SCREENSHOT: &str = "browser_screenshot";
pub const BROWSER_DOM: &str = "browser_dom";
pub const BROWSER_CLICK: &str = "browser_click";
pub const BROWSER_TYPE: &str = "browser_type";
pub const BROWSER_EVAL: &str = "browser_eval";
pub const BROWSER_CONSOLE: &str = "browser_console";
pub const BROWSER_NETWORK: &str = "browser_network";
pub const BROWSER_WAIT: &str = "browser_wait";
pub const BROWSER_SCROLL: &str = "browser_scroll";
pub const BROWSER_SELECT: &str = "browser_select";
pub const BROWSER_HOVER: &str = "browser_hover";
pub const BROWSER_CLOSE: &str = "browser_close";
pub const BROWSER_RECORD_START: &str = "browser_record_start";
pub const BROWSER_RECORD_STOP: &str = "browser_record_stop";
pub const BROWSER_RESIZE: &str = "browser_resize";
pub const BROWSER_BACK: &str = "browser_back";
pub const BROWSER_FORWARD: &str = "browser_forward";
pub const BROWSER_FIND: &str = "browser_find";
pub const BROWSER_PDF: &str = "browser_pdf";
pub const BROWSER_UPLOAD: &str = "browser_upload";
pub const BROWSER_COOKIES: &str = "browser_cookies";
pub const BROWSER_KEYS: &str = "browser_keys";
pub const BROWSER_IFRAME: &str = "browser_iframe";

pub const WORKSPACE_MEMORY: &str = "workspace_memory";
pub const INDEX_WORKSPACE: &str = "index_workspace";
pub const SEMANTIC_SEARCH: &str = "semantic_search";
pub const DOC_READ: &str = "doc_read";
pub const DOC_EDIT: &str = "doc_edit";

pub const TEAM_RUN: &str = "team_run";
pub const TEAM_CREATE: &str = "team_create";
pub const AGENT: &str = "agent";
pub const SEND_MESSAGE: &str = "send_message";
pub const TEAM_STATUS: &str = "team_status";
pub const TEAM_STOP: &str = "team_stop";
pub const TASK_CREATE: &str = "task_create";
pub const TASK_LIST: &str = "task_list";
pub const TASK_UPDATE: &str = "task_update";

pub fn canonical_tool_name(name: &str) -> &str {
    match name {
        "Glob" => GLOB,
        "Grep" => GREP,
        "WebSearch" => WEB_SEARCH,
        "WebFetch" => WEB_FETCH,
        "CreateImage" => CREATE_IMAGE,
        "Question" => QUESTION,
        "ToDoList" | "TodoList" => TODO_LIST,
        "LoadMcpTool" => LOAD_MCP_TOOL,
        "LoadSkill" => SKILL,
        "TeamRun" => TEAM_RUN,
        "TeamCreate" => TEAM_CREATE,
        "Agent" => AGENT,
        "SendMessage" => SEND_MESSAGE,
        "TeamStatus" => TEAM_STATUS,
        "TeamStop" => TEAM_STOP,
        "TaskCreate" => TASK_CREATE,
        "TaskList" => TASK_LIST,
        "TaskUpdate" => TASK_UPDATE,
        _ => name,
    }
}

pub fn is_tool_name(name: &str, canonical: &str) -> bool {
    canonical_tool_name(name) == canonical
}
