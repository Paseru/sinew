use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use claakecode_core::{ChatMessage, ModelRef, Part, Provider, Role, ServiceTier, ToolDescriptor};
use tokio::sync::{mpsc, Notify, RwLock, Semaphore};
use uuid::Uuid;

use crate::tool_run::{DiffLineKind, FileChange, FileChangeKind, ToolRunImage};
use crate::{
    run_turn, subagent_system_prompt, AgentEvent, AgentEventScope, AgentMode, BashTool,
    CreateImageTool, DatabaseTool, EditFileTool, GlobTool, GoalWorkflowState, GrepTool, McpSettings,
    McpToolRegistry, ReadTool, SkillSettings, SkillTool, SubAgentConfig, SubAgentSettings,
    TodoListState, ToolRunResult, ToolSettings, TurnCancel, TurnContext, WebFetchTool,
    WebSearchTool, WriteFileTool,
};

const TEAM_RUN_TOOL: &str = "TeamRun";
const TEAM_CREATE_TOOL: &str = "TeamCreate";
const AGENT_TOOL: &str = "Agent";
const SEND_MESSAGE_TOOL: &str = "SendMessage";
const TEAM_STATUS_TOOL: &str = "TeamStatus";
const TEAM_STOP_TOOL: &str = "TeamStop";
const TASK_CREATE_TOOL: &str = "TaskCreate";
const TASK_LIST_TOOL: &str = "TaskList";
const TASK_UPDATE_TOOL: &str = "TaskUpdate";
const TEAM_SETTLE_GRACE_MS: u64 = 100;
const TEAM_RECENT_FILE_CHANGE_LIMIT: usize = 20;

mod agent_turns;
mod context;
mod descriptors;
mod launch;
mod live;
mod messaging;
mod model;
mod render;
mod session;
mod status_stop;
mod task_board;

#[cfg(test)]
mod tests;

pub use self::descriptors::is_team_tool_name;
pub use self::model::{
    TeamAgent, TeamAgentStatus, TeamQueuedMessage, TeamRecentFileChange, TeamRuntime, TeamSession,
    TeamTask, TeamTaskStatus, TeamTaskWake, TeamTool,
};

use self::model::{
    LiveAgentReport, PreparedTeamAgentConfig, PreparedTeamRunTask, SendMessageInput,
    TaskCreateInput, TaskIdInput, TaskListAction, TaskListInput, TaskUpdateInput,
    TeamAgentFinalResponse, TeamIdentity, TeamNameInput, TeamRunInput, TeamRunTaskInput,
    TeamSnapshot, TeamStopInput, TeamTaskSnapshot, TeamTurn,
};
use self::render::*;
use self::task_board::*;
