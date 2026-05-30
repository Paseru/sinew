use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonRequest {
    StartTurn {
        conversation_id: String,
        workspace_path: String,
        system_prompt: String,
        model_name: String,
        provider: String,
        history: serde_json::Value,
        todo_list: serde_json::Value,
        goal_workflow: serde_json::Value,
        mcp_settings: Option<serde_json::Value>,
        tool_settings: Option<serde_json::Value>,
        skill_settings: Option<serde_json::Value>,
        sub_agent_settings: Option<serde_json::Value>,
    },
    CancelTurn {
        conversation_id: String,
    },
    GetStatus,
    ListEntries {
        workspace_path: String,
        relative_path: Option<String>,
    },
    ListAllFiles {
        workspace_path: String,
    },
    ReadFile {
        workspace_path: String,
        relative_path: String,
    },
    WriteFile {
        workspace_path: String,
        relative_path: String,
        content: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonResponse {
    Status {
        is_busy: bool,
        active_conversation_id: Option<String>,
    },
    TurnStarted {
        conversation_id: String,
    },
    Event {
        conversation_id: String,
        event: serde_json::Value,
    },
    TurnFinished {
        conversation_id: String,
        success: bool,
        error: Option<String>,
        output: Option<serde_json::Value>,
    },
    Error {
        message: String,
    },
    EntriesList {
        entries: serde_json::Value,
    },
    FileContent {
        content: String,
    },
    FileWritten,
}
