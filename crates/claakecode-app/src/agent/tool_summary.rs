use serde_json::Value;

use crate::bash::active_shell_display_name;

pub(super) fn summarize_tool(name: &str, input: &Value) -> String {
    if name == "bash" {
        if let Some(desc) = input
            .get("description")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return desc.to_string();
        }
        if let Some(command) = input.get("command").and_then(|value| value.as_str()) {
            return command.to_string();
        }
    }
    if name == "bash_input" {
        if let Some(session_id) = input.get("session_id").and_then(|value| value.as_u64()) {
            let shell = active_shell_display_name();
            if input
                .get("kill")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
            {
                return format!("Stop {shell} session {session_id}");
            }
            if input
                .get("input")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some()
            {
                return format!("Send input to {shell} session {session_id}");
            }
            return format!("Poll {shell} session {session_id}");
        }
    }
    if name == "read" {
        if let Some(path) = input.get("path").and_then(|value| value.as_str()) {
            return format!("Read {path}");
        }
    }
    if name == "Grep" {
        let scope = input
            .get("path")
            .and_then(grep_path_scope)
            .or_else(|| {
                input
                    .get("include")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
            })
            .filter(|value| value != ".")
            .unwrap_or_else(|| "workspace".to_string());
        return format!("Grep in {scope}");
    }
    if name == "Glob" {
        let pattern = input
            .get("pattern")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("*");
        let scope = input
            .get("path")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != ".")
            .unwrap_or("workspace");
        return format!("Glob {pattern} in {scope}");
    }
    if name == "edit_file" {
        if let Some(groups) = edit_file_groups(input) {
            let file_count = groups.len();
            let replacement_count = groups
                .iter()
                .map(|group| {
                    group
                        .get("edits")
                        .and_then(Value::as_array)
                        .map(Vec::len)
                        .unwrap_or_default()
                })
                .sum::<usize>();
            if file_count == 1 {
                if let Some(path) = groups[0].get("path").and_then(Value::as_str) {
                    if replacement_count > 1 {
                        return format!("Edit {path} · {replacement_count} replacements");
                    }
                    return format!("Edit {path}");
                }
                return if replacement_count > 1 {
                    format!("Edit file · {replacement_count} replacements")
                } else {
                    "Edit file".to_string()
                };
            }
            return if replacement_count > 0 {
                format!("Edit files · {file_count} files · {replacement_count} replacements")
            } else {
                format!("Edit files · {file_count} files")
            };
        }
        return "Edit file".to_string();
    }
    if name == "write_file" {
        if let Some(path) = input.get("path").and_then(Value::as_str) {
            return format!("Write {path}");
        }
        return "Write file".to_string();
    }
    if name == "clean_context" {
        let count = input
            .get("tool_call_ids")
            .or_else(|| input.get("ids"))
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or_default();
        return if count == 0 {
            "Clean context".to_string()
        } else {
            format!("Clean context · {count} results")
        };
    }
    if name == "update_goal" {
        return "Goal finished".to_string();
    }
    if name == "CreateImage" {
        if let Some(prompt) = input
            .get("prompt")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let mut clipped = prompt.chars().take(64).collect::<String>();
            if prompt.chars().count() > 64 {
                clipped.push_str("...");
            }
            return format!("Create image: {clipped}");
        }
        return "Create image".to_string();
    }
    if name == "ToDoList" {
        if let Some(changes) = input
            .get("changes")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if changes.eq_ignore_ascii_case("close") || changes.eq_ignore_ascii_case("clear") {
                return "Close ToDoList".to_string();
            }
        }
        return "Update ToDoList".to_string();
    }
    if name == "Question" {
        if let Some(count) = input
            .get("questions")
            .and_then(|value| value.as_array())
            .map(Vec::len)
        {
            return if count == 1 {
                "Question".to_string()
            } else {
                format!("{count} questions")
            };
        }
        if let Some(question) = input
            .get("question")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return question.to_string();
        }
        return "Question".to_string();
    }
    if name == "LoadMcpTool" {
        let server = input
            .get("server")
            .or_else(|| input.get("serverName"))
            .or_else(|| input.get("server_name"))
            .and_then(|value| value.as_str())
            .map(display_mcp_server_name)
            .filter(|value| !value.is_empty());
        let tool = input
            .get("tool")
            .or_else(|| input.get("toolName"))
            .or_else(|| input.get("tool_name"))
            .or_else(|| input.get("name"))
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let (Some(server), Some(tool)) = (server, tool) {
            return format!("Load {server} · {tool}");
        }
        return "Load MCP tool".to_string();
    }
    if name == "skill" {
        if let Some(skill) = input
            .get("name")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return format!("Load skill · {skill}");
        }
        return "Load skill".to_string();
    }
    if name.starts_with("subagent_") {
        if let Some(task) = input
            .get("task")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return format!("Sub-agent · {task}");
        }
        return "Sub-agent".to_string();
    }
    if name == "TeamRun" {
        let team = input
            .get("team_name")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let agent = input
            .get("agent")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let objective = input
            .get("objective")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty());
        return match (team, agent, objective) {
            (Some(team), Some(agent), _) => format!("Agent Swarm · restart @{agent} · {team}"),
            (None, Some(agent), _) => format!("Agent Swarm · restart @{agent}"),
            (Some(team), None, Some(objective)) => format!("Agent Swarm · {team} · {objective}"),
            (Some(team), None, None) => format!("Agent Swarm · {team}"),
            (None, None, Some(objective)) => format!("Agent Swarm · {objective}"),
            _ => "Agent Swarm".to_string(),
        };
    }
    if name == "TeamCreate" {
        if let Some(team) = input
            .get("team_name")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return format!("Agent Swarm · {team}");
        }
        return "Create Agent Swarm".to_string();
    }
    if name == "Agent" {
        let teammate = input
            .get("name")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let task = input
            .get("description")
            .or_else(|| input.get("prompt"))
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty());
        return match (teammate, task) {
            (Some(teammate), Some(task)) => format!("Agent · @{teammate} · {task}"),
            (Some(teammate), None) => format!("Agent · @{teammate}"),
            _ => "Agent teammate".to_string(),
        };
    }
    if name == "SendMessage" {
        if let Some(to) = input
            .get("to")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return format!("Message · {to}");
        }
        return "Send Agent Swarm message".to_string();
    }
    if name == "TaskCreate" {
        if let Some(subject) = input
            .get("subject")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return format!("Task · create · {subject}");
        }
        return "Create task".to_string();
    }
    if name == "TaskList" {
        let action = input
            .get("action")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let task_id = input
            .get("taskId")
            .or_else(|| input.get("id"))
            .and_then(|value| {
                value
                    .as_str()
                    .map(str::to_string)
                    .or_else(|| value.as_u64().map(|value| value.to_string()))
            });
        let subject = input
            .get("subject")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty());
        return match (action, task_id, subject) {
            (Some("create"), _, Some(subject)) => format!("Task · create · {subject}"),
            (Some(action @ ("update" | "claim" | "delete")), Some(task_id), _) => {
                format!("Task · {action} · #{task_id}")
            }
            (Some(action), _, _) => format!("Task · {action}"),
            _ => "Task list".to_string(),
        };
    }
    if name == "TaskUpdate" {
        let task_id = input
            .get("taskId")
            .or_else(|| input.get("id"))
            .and_then(|value| {
                value
                    .as_str()
                    .map(str::to_string)
                    .or_else(|| value.as_u64().map(|value| value.to_string()))
            });
        let status = input
            .get("status")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty());
        return match (task_id, status) {
            (Some(task_id), Some(status)) => format!("Task · #{task_id} · {status}"),
            (Some(task_id), None) => format!("Task · #{task_id}"),
            _ => "Update task".to_string(),
        };
    }
    if name == "TeamStatus" {
        return "Agent Swarm status".to_string();
    }
    if name == "TeamStop" {
        return "Stop Agent Swarm".to_string();
    }
    if name == "WebSearch" {
        if let Some(q) = input
            .get("q")
            .or_else(|| input.get("query"))
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return format!("Search web: {q}");
        }
        return "Search web".to_string();
    }
    if name == "WebFetch" {
        if let Some(url) = input
            .get("url")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return format!("Fetch {url}");
        }
        return "Fetch URL".to_string();
    }

    if let Ok(pretty) = serde_json::to_string(input) {
        if pretty.len() <= 72 {
            return pretty;
        }
        let mut clipped = pretty.chars().take(69).collect::<String>();
        clipped.push_str("...");
        return clipped;
    }

    name.to_string()
}

pub(super) fn should_stream_tool_args(name: &str) -> bool {
    matches!(name, "edit_file" | "write_file" | "read")
}

fn edit_file_groups(input: &Value) -> Option<&Vec<Value>> {
    input.get("files").and_then(Value::as_array)
}

fn grep_path_scope(value: &Value) -> Option<String> {
    if let Some(path) = value
        .as_str()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        return Some(path.to_string());
    }

    let paths = value.as_array()?;
    let scope = paths
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if scope.is_empty() {
        None
    } else {
        Some(scope)
    }
}

pub(super) fn display_mcp_server_name(value: &str) -> String {
    let trimmed = value.trim();
    let Some(rest) = trimmed.get(3..) else {
        return trimmed.to_string();
    };
    if !trimmed[..3].eq_ignore_ascii_case("mcp") {
        return trimmed.to_string();
    }

    let stripped = rest
        .trim_start_matches(|ch: char| ch == '-' || ch == '_' || ch == '.' || ch.is_whitespace())
        .trim();
    if stripped.is_empty() {
        trimmed.to_string()
    } else {
        stripped.to_string()
    }
}

pub(super) fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}
