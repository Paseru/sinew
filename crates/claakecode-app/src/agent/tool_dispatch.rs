use std::{collections::HashMap, sync::Arc};

use serde_json::Value;
use tokio::sync::mpsc;

use crate::{
    BashTool, CreateImageTool, DatabaseTool, EditFileTool, GlobTool, GrepTool, McpToolRegistry,
    QuestionTool, ReadFingerprint, ReadTool, SkillTool, SubAgentTool, TeamTool, ToDoListTool,
    TodoListState, ToolRunResult, ToolSettings, WebFetchTool, WebSearchTool, WriteFileTool,
};

use super::{cancel::TurnCancel, context::AgentMode, events::AgentEvent};

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
    edit_file: &EditFileTool,
    write_file: &WriteFileTool,
    create_image: &CreateImageTool,
    todo_list_tool: Option<&ToDoListTool>,
    question: Option<&QuestionTool>,
    web_search: &WebSearchTool,
    web_fetch: &WebFetchTool,
    skill: &SkillTool,
    database: &DatabaseTool,
    mcp: &McpToolRegistry,
    subagents: Option<&SubAgentTool>,
    teams: Option<&TeamTool>,
    tool_settings: &ToolSettings,
    read_fingerprints: &HashMap<String, ReadFingerprint>,
    todo_list: &mut TodoListState,
    mode: AgentMode,
    event_tx: &mpsc::UnboundedSender<AgentEvent>,
    cancel: &TurnCancel,
    tool_call_id: &str,
    name: &str,
    input: Value,
) -> ToolRunResult {
    let name = match name {
        "TodoList" | "todo_list" => "ToDoList",
        _ => name,
    };
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
    } else if name == "edit_file" {
        if mode == AgentMode::Plan {
            return ToolRunResult::err("edit_file is unavailable in Plan mode", Vec::new());
        }
        edit_file.run(input, read_fingerprints).await
    } else if name == "write_file" {
        if mode == AgentMode::Plan {
            return ToolRunResult::err("write_file is unavailable in Plan mode", Vec::new());
        }
        write_file.run(input, read_fingerprints).await
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
        question.run(tool_call_id, input, cancel).await
    } else if name == "WebSearch" {
        web_search.run(input).await
    } else if name == "WebFetch" {
        web_fetch.run(input).await
    } else if name == "skill" {
        skill.run(input).await
    } else if let Some(result) = database.run(name, input.clone(), question.is_some()).await {
        result
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
