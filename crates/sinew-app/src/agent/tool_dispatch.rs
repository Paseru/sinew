use std::{collections::BTreeSet, sync::Arc};

use serde_json::Value;
use tokio::sync::mpsc;

use crate::{
    ApplyPatchTool, BashTool, CreateImageTool, GlobTool, GrepTool, McpToolRegistry, QuestionTool,
    ReadTool, SkillTool, SubAgentTool, TeamTool, ToDoListTool, TodoListState, ToolRunResult,
    ToolSettings, WebFetchTool, WebSearchTool,
};

use super::{context::AgentMode, events::AgentEvent};

pub(super) fn should_wait_for_cooperative_cancel(
    name: &str,
    subagents: Option<&Arc<SubAgentTool>>,
    teams: Option<&Arc<TeamTool>>,
) -> bool {
    name.starts_with("subagent_")
        || teams
            .and_then(|tool| tool.summary_for_tool_name(name))
            .is_some()
        || subagents
            .and_then(|tool| tool.summary_for_tool_name(name))
            .is_some()
}

pub(super) async fn run_tool(
    bash: &BashTool,
    glob: &GlobTool,
    grep: &GrepTool,
    read: &ReadTool,
    apply_patch: &ApplyPatchTool,
    create_image: &CreateImageTool,
    todo_list_tool: Option<&ToDoListTool>,
    question: Option<&QuestionTool>,
    web_search: &WebSearchTool,
    web_fetch: &WebFetchTool,
    skill: &SkillTool,
    mcp: &McpToolRegistry,
    subagents: Option<&SubAgentTool>,
    teams: Option<&TeamTool>,
    tool_settings: &ToolSettings,
    _read_paths: &BTreeSet<String>,
    todo_list: &mut TodoListState,
    mode: AgentMode,
    event_tx: &mpsc::UnboundedSender<AgentEvent>,
    tool_call_id: &str,
    name: &str,
    input: Value,
) -> ToolRunResult {
    if !tool_settings.is_enabled(name) {
        return ToolRunResult::err(format!("{name} is disabled in Settings"), Vec::new());
    }
    if name == "bash" {
        bash.run(input).await
    } else if name == "bash_input" {
        bash.run_input(input).await
    } else if name == "Glob" {
        glob.run(input).await
    } else if name == "Grep" {
        grep.run(input).await
    } else if name == "read" {
        read.run(input).await
    } else if name == "apply_patch" {
        if mode == AgentMode::Plan {
            return ToolRunResult::err("apply_patch is unavailable in Plan mode", Vec::new());
        }
        apply_patch.run_with_read_paths(input).await
    } else if name == "CreateImage" {
        if mode == AgentMode::Plan {
            return ToolRunResult::err("CreateImage is unavailable in Plan mode", Vec::new());
        }
        create_image.run(input).await
    } else if name == "ToDoList" {
        let Some(todo_list_tool) = todo_list_tool else {
            return ToolRunResult::err("ToDoList is unavailable in this context", Vec::new());
        };
        todo_list_tool.run(input, todo_list).await
    } else if name == "Question" {
        let Some(question) = question else {
            return ToolRunResult::err("Question is unavailable in this context", Vec::new());
        };
        question.run(input).await
    } else if name == "WebSearch" {
        web_search.run(input).await
    } else if name == "WebFetch" {
        web_fetch.run(input).await
    } else if name == "skill" {
        skill.run(input).await
    } else if name.starts_with("subagent_") {
        let Some(subagents) = subagents else {
            return ToolRunResult::err(format!("unknown tool: {name}"), Vec::new());
        };
        subagents
            .run(tool_call_id, name, input, mode, event_tx.clone())
            .await
            .unwrap_or_else(|| ToolRunResult::err(format!("unknown tool: {name}"), Vec::new()))
    } else if let Some(teams) = teams {
        if let Some(result) = teams
            .run(tool_call_id, name, input.clone(), mode, event_tx.clone())
            .await
        {
            result
        } else if let Some(result) = mcp.run_tool(name, input).await {
            result
        } else {
            ToolRunResult::err(format!("unknown tool: {name}"), Vec::new())
        }
    } else if let Some(result) = mcp.run_tool(name, input).await {
        result
    } else {
        ToolRunResult::err(format!("unknown tool: {name}"), Vec::new())
    }
}
