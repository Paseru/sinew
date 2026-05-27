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
pub const CHECK_SOTA: &str = "check_sota";

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
        "CheckSota" | "check_sota" => CHECK_SOTA,
        _ => name,
    }
}

pub fn is_tool_name(name: &str, canonical: &str) -> bool {
    canonical_tool_name(name) == canonical
}

pub fn is_cursor_compatible_tool(name: &str) -> bool {
    let canonical = canonical_tool_name(name);
    matches!(
        canonical,
        BASH
            | GLOB
            | GREP
            | READ
            | EDIT_FILE
            | WRITE_FILE
            | WEB_SEARCH
            | WEB_FETCH
            | TODO_LIST
            | QUESTION
            | LOAD_MCP_TOOL
    ) || name.starts_with("mcp__")
}
