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
    },
    CancelTurn {
        conversation_id: String,
    },
    GetStatus,
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
    },
    Error {
        message: String,
    },
}
