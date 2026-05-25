use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use claakecode_core::{ChatMessage, Part, ToolDescriptor};

use crate::tool_run::ToolRunResult;

const MAX_CHANGES_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Done,
}

impl TodoStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TodoStatus::Pending => "pending",
            TodoStatus::InProgress => "in_progress",
            TodoStatus::Done => "done",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoTask {
    pub id: String,
    pub text: String,
    pub status: TodoStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoListState {
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub tasks: Vec<TodoTask>,
    #[serde(default = "default_next_id")]
    pub next_id: u32,
}

impl Default for TodoListState {
    fn default() -> Self {
        Self {
            active: false,
            tasks: Vec::new(),
            next_id: default_next_id(),
        }
    }
}

impl TodoListState {
    pub fn normalize(&mut self) {
        let max_id = self
            .tasks
            .iter()
            .filter_map(|task| task.id.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        if self.next_id == 0 || self.next_id <= max_id {
            self.next_id = max_id.saturating_add(1).max(1);
        }
        enforce_single_in_progress(&mut self.tasks);
        if !self.active {
            self.tasks.clear();
            self.next_id = default_next_id();
        }
    }

    pub fn system_block(&self) -> Option<String> {
        if !self.active {
            return None;
        }

        let mut block = String::from("Current ToDoList:\n");
        if self.tasks.is_empty() {
            block.push_str("- none\n");
        } else {
            for task in &self.tasks {
                block.push_str(&format!(
                    "- {} [{}] {}\n",
                    task.id,
                    task.status.as_str(),
                    task.text
                ));
            }
        }
        block.push_str(
            "\nUse ToDoList to update this list. Keep at most one task in_progress. Call ToDoList with `close` when the list is finished.",
        );
        Some(block)
    }

    pub fn render_tool_output(&self, heading: &str) -> String {
        let mut output = String::new();
        output.push_str(heading);
        output.push('\n');
        output.push_str(if self.active {
            "state: active\n"
        } else {
            "state: closed\n"
        });
        output.push_str("tasks:\n");
        if self.tasks.is_empty() {
            output.push_str("none\n");
        } else {
            for task in &self.tasks {
                output.push_str(&format!(
                    "{}. [{}] {}\n",
                    task.id,
                    task.status.as_str(),
                    task.text
                ));
            }
        }
        output.trim_end().to_string()
    }

    fn add_task(&mut self, text: String, status: TodoStatus) {
        self.active = true;
        let id = self.next_id.max(1);
        self.next_id = id.saturating_add(1);
        self.tasks.push(TodoTask {
            id: id.to_string(),
            text,
            status,
        });
        if status == TodoStatus::InProgress {
            set_single_in_progress(&mut self.tasks, &id.to_string());
        }
    }

    fn update_task(
        &mut self,
        id: &str,
        status: Option<TodoStatus>,
        text: Option<String>,
    ) -> Result<()> {
        if !self.active {
            bail!("no active ToDoList");
        }

        let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) else {
            bail!("todo task `{id}` was not found");
        };
        if let Some(text) = text {
            task.text = text;
        }
        if let Some(status) = status {
            task.status = status;
        }
        if status == Some(TodoStatus::InProgress) {
            set_single_in_progress(&mut self.tasks, id);
        }
        Ok(())
    }

    fn delete_task(&mut self, id: &str) -> Result<()> {
        if !self.active {
            bail!("no active ToDoList");
        }
        let original_len = self.tasks.len();
        self.tasks.retain(|task| task.id != id);
        if self.tasks.len() == original_len {
            bail!("todo task `{id}` was not found");
        }
        Ok(())
    }

    fn close(&mut self) {
        self.active = false;
        self.tasks.clear();
        self.next_id = default_next_id();
    }
}

#[derive(Debug, Clone, Default)]
pub struct ToDoListTool;

impl ToDoListTool {
    pub fn new() -> Self {
        Self
    }

    pub fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "ToDoList".into(),
            description: "Update the current task list with a small line-based patch. Use one call to add, update, delete, or close tasks instead of rewriting the whole list.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "changes": {
                        "type": "string",
                        "description": "Line-based ToDoList patch. Examples: `+ Read the code`, `~ 1 in_progress`, `~ 1 done`, `- 2`, `close`. Use `pending`, `in_progress`, or `done` for task status."
                    }
                },
                "required": ["changes"],
                "additionalProperties": false
            }),
        }
    }

    pub async fn run(&self, input: Value, state: &mut TodoListState) -> ToolRunResult {
        let parsed: ToDoListInput = match serde_json::from_value(input) {
            Ok(value) => value,
            Err(err) => {
                return ToolRunResult::err(format!("invalid ToDoList input: {err}"), Vec::new())
            }
        };

        match apply_changes(state, &parsed.changes) {
            Ok(heading) => ToolRunResult::ok(state.render_tool_output(&heading), Vec::new()),
            Err(err) => ToolRunResult::err(err.to_string(), Vec::new()),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ToDoListInput {
    changes: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TodoOp {
    Add {
        text: String,
        status: TodoStatus,
    },
    Update {
        id: String,
        status: Option<TodoStatus>,
        text: Option<String>,
    },
    Delete {
        id: String,
    },
    Open,
    Close,
}

pub fn system_prompt_with_todo(base: &str, state: &TodoListState) -> String {
    let Some(block) = state.system_block() else {
        return base.to_string();
    };
    format!("{base}\n\n<todo_list_state>\n{block}\n</todo_list_state>")
}

pub fn todo_list_from_history(history: &[ChatMessage]) -> TodoListState {
    let mut state = TodoListState::default();
    for message in history {
        for part in &message.parts {
            let Part::ToolResult {
                meta: Some(meta), ..
            } = part
            else {
                continue;
            };
            let Some(value) = meta.get("todo_list") else {
                continue;
            };
            if let Ok(next) = serde_json::from_value::<TodoListState>(value.clone()) {
                state = next;
            }
        }
    }
    state.normalize();
    state
}

fn apply_changes(state: &mut TodoListState, changes: &str) -> Result<String> {
    if changes.trim().is_empty() {
        bail!("changes are required");
    }
    if changes.len() > MAX_CHANGES_BYTES {
        bail!("ToDoList changes are too large");
    }

    let ops = parse_changes(changes)?;
    let mut next = state.clone();
    next.normalize();

    let mut saw_close = false;
    let mut saw_open_or_update = false;
    for op in ops {
        match op {
            TodoOp::Add { text, status } => {
                next.add_task(text, status);
                saw_open_or_update = true;
            }
            TodoOp::Update { id, status, text } => {
                next.update_task(&id, status, text)?;
                saw_open_or_update = true;
            }
            TodoOp::Delete { id } => {
                next.delete_task(&id)?;
                saw_open_or_update = true;
            }
            TodoOp::Open => {
                next.active = true;
                saw_open_or_update = true;
            }
            TodoOp::Close => {
                next.close();
                saw_close = true;
            }
        }
    }

    next.normalize();
    *state = next;

    let heading = if !state.active && saw_close {
        "ToDoList closed."
    } else if state.active && saw_open_or_update {
        "ToDoList updated."
    } else if state.active {
        "ToDoList opened."
    } else {
        "ToDoList unchanged."
    };
    Ok(heading.to_string())
}

fn parse_changes(changes: &str) -> Result<Vec<TodoOp>> {
    let mut ops = Vec::new();
    for (index, raw) in changes.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("```") {
            continue;
        }
        let op = parse_line(line).ok_or_else(|| {
            anyhow::anyhow!("line {} is not a valid ToDoList change: {line}", index + 1)
        })?;
        ops.push(op);
    }
    if ops.is_empty() {
        bail!("changes must include at least one ToDoList operation");
    }
    Ok(ops)
}

fn parse_line(line: &str) -> Option<TodoOp> {
    let lowered = line.to_ascii_lowercase();
    match lowered.as_str() {
        "open" => return Some(TodoOp::Open),
        "close" | "closed" | "clear" | "reset" | "finish" | "finished" => {
            return Some(TodoOp::Close)
        }
        _ => {}
    }

    if let Some(rest) = line.strip_prefix('+') {
        return parse_add(rest);
    }
    if let Some(rest) = line.strip_prefix('~') {
        return parse_update(rest);
    }
    if let Some(rest) = line.strip_prefix('-') {
        let rest = rest.trim();
        if starts_with_id(rest) {
            return parse_delete(rest);
        }
    }

    for prefix in ["add:", "create:", "todo:"] {
        if let Some(rest) = strip_ascii_prefix(line, prefix) {
            return parse_add(rest);
        }
    }
    for prefix in ["update:", "set:", "change:"] {
        if let Some(rest) = strip_ascii_prefix(line, prefix) {
            return parse_update(rest);
        }
    }
    for prefix in ["delete:", "remove:", "drop:"] {
        if let Some(rest) = strip_ascii_prefix(line, prefix) {
            return parse_delete(rest);
        }
    }

    for (prefix, status) in [
        ("pending:", TodoStatus::Pending),
        ("todo:", TodoStatus::Pending),
        ("start:", TodoStatus::InProgress),
        ("started:", TodoStatus::InProgress),
        ("in_progress:", TodoStatus::InProgress),
        ("doing:", TodoStatus::InProgress),
        ("done:", TodoStatus::Done),
        ("complete:", TodoStatus::Done),
        ("completed:", TodoStatus::Done),
    ] {
        if let Some(rest) = strip_ascii_prefix(line, prefix) {
            return parse_status_command(rest, status);
        }
    }

    None
}

fn parse_add(rest: &str) -> Option<TodoOp> {
    let raw = strip_leading_separators(rest);
    if raw.is_empty() {
        return None;
    }

    let (status, text) = parse_add_status(raw).unwrap_or((TodoStatus::Pending, raw));
    let text = strip_leading_separators(text).trim();
    if text.is_empty() {
        return None;
    }

    Some(TodoOp::Add {
        text: text.to_string(),
        status,
    })
}

fn parse_update(rest: &str) -> Option<TodoOp> {
    let rest = strip_leading_separators(rest);
    let (id, remainder) = split_first_word(rest)?;
    if !is_id(id) {
        return None;
    }

    let mut status = None;
    let mut text = None;
    let mut remaining = remainder.trim();

    if let Some(after) = strip_ascii_prefix(remaining, "status:") {
        let (parsed_status, after_status) = parse_status_prefix(after)?;
        status = Some(parsed_status);
        remaining = after_status.trim();
    } else if let Some(after) = strip_ascii_prefix(remaining, "status=") {
        let (parsed_status, after_status) = parse_status_prefix(after)?;
        status = Some(parsed_status);
        remaining = after_status.trim();
    } else if let Some((parsed_status, after_status)) = parse_status_prefix(remaining) {
        status = Some(parsed_status);
        remaining = after_status.trim();
    }

    if let Some(after) = strip_ascii_prefix(remaining, "text=")
        .or_else(|| strip_ascii_prefix(remaining, "title="))
        .or_else(|| strip_ascii_prefix(remaining, "task="))
    {
        let value = strip_wrapping_quotes(after.trim());
        if !value.is_empty() {
            text = Some(value.to_string());
        }
    } else if status.is_none() && !remaining.is_empty() {
        text = Some(strip_wrapping_quotes(remaining).to_string());
    } else if status.is_some() {
        let value = strip_leading_separators(remaining);
        if !value.is_empty() {
            text = Some(strip_wrapping_quotes(value).to_string());
        }
    }

    if status.is_none() && text.is_none() {
        return None;
    }

    Some(TodoOp::Update {
        id: id.to_string(),
        status,
        text,
    })
}

fn parse_delete(rest: &str) -> Option<TodoOp> {
    let rest = strip_leading_separators(rest);
    let (id, _) = split_first_word(rest)?;
    if !is_id(id) {
        return None;
    }
    Some(TodoOp::Delete { id: id.to_string() })
}

fn parse_status_command(rest: &str, status: TodoStatus) -> Option<TodoOp> {
    let rest = strip_leading_separators(rest);
    if starts_with_id(rest) {
        let (id, remaining) = split_first_word(rest)?;
        let remaining = strip_leading_separators(remaining);
        return Some(TodoOp::Update {
            id: id.to_string(),
            status: Some(status),
            text: (!remaining.is_empty()).then(|| strip_wrapping_quotes(remaining).to_string()),
        });
    }
    parse_add(&format!("[{}] {rest}", status.as_str()))
}

fn parse_add_status(raw: &str) -> Option<(TodoStatus, &str)> {
    let raw = raw.trim();
    if let Some(stripped) = raw.strip_prefix('[') {
        let end = stripped.find(']')?;
        let status = parse_status_word(&stripped[..end])?;
        return Some((status, &stripped[end + 1..]));
    }
    parse_status_prefix(raw)
}

fn parse_status_prefix(raw: &str) -> Option<(TodoStatus, &str)> {
    let trimmed = raw.trim_start();
    let lowered = trimmed.to_ascii_lowercase();
    let candidates = [
        ("status=in_progress", TodoStatus::InProgress),
        ("status=in-progress", TodoStatus::InProgress),
        ("status=in progress", TodoStatus::InProgress),
        ("in_progress", TodoStatus::InProgress),
        ("in-progress", TodoStatus::InProgress),
        ("in progress", TodoStatus::InProgress),
        ("started", TodoStatus::InProgress),
        ("start", TodoStatus::InProgress),
        ("doing", TodoStatus::InProgress),
        ("pending", TodoStatus::Pending),
        ("todo", TodoStatus::Pending),
        ("open", TodoStatus::Pending),
        ("done", TodoStatus::Done),
        ("completed", TodoStatus::Done),
        ("complete", TodoStatus::Done),
        ("finished", TodoStatus::Done),
        ("finish", TodoStatus::Done),
    ];

    for (candidate, status) in candidates {
        if lowered == candidate {
            return Some((status, ""));
        }
        if lowered.starts_with(candidate) {
            let boundary = lowered.as_bytes().get(candidate.len()).copied();
            if matches!(boundary, Some(b' ' | b':' | b',' | b';' | b'-' | b'=')) {
                return Some((status, &trimmed[candidate.len()..]));
            }
        }
    }
    None
}

fn parse_status_word(raw: &str) -> Option<TodoStatus> {
    let normalized = raw.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    match normalized.as_str() {
        "pending" | "todo" | "open" => Some(TodoStatus::Pending),
        "in_progress" | "doing" | "started" | "start" => Some(TodoStatus::InProgress),
        "done" | "complete" | "completed" | "finished" | "finish" => Some(TodoStatus::Done),
        _ => None,
    }
}

fn strip_ascii_prefix<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    value
        .get(..prefix.len())
        .filter(|head| head.eq_ignore_ascii_case(prefix))
        .map(|_| &value[prefix.len()..])
}

fn strip_leading_separators(value: &str) -> &str {
    value
        .trim_start_matches(|c: char| c.is_whitespace() || matches!(c, ':' | '-' | '='))
        .trim()
}

fn strip_wrapping_quotes(value: &str) -> &str {
    let value = value.trim();
    if value.len() >= 2 {
        if let Some(stripped) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
            return stripped.trim();
        }
        if let Some(stripped) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
            return stripped.trim();
        }
    }
    value
}

fn split_first_word(value: &str) -> Option<(&str, &str)> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    for (index, ch) in value.char_indices() {
        if ch.is_whitespace() {
            return Some((&value[..index], &value[index..]));
        }
    }
    Some((value, ""))
}

fn starts_with_id(value: &str) -> bool {
    split_first_word(value)
        .map(|(word, _)| is_id(word))
        .unwrap_or(false)
}

fn is_id(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn set_single_in_progress(tasks: &mut [TodoTask], id: &str) {
    for task in tasks {
        if task.id == id {
            task.status = TodoStatus::InProgress;
        } else if task.status == TodoStatus::InProgress {
            task.status = TodoStatus::Pending;
        }
    }
}

fn enforce_single_in_progress(tasks: &mut [TodoTask]) {
    let mut seen = false;
    for task in tasks {
        if task.status != TodoStatus::InProgress {
            continue;
        }
        if seen {
            task.status = TodoStatus::Pending;
        } else {
            seen = true;
        }
    }
}

fn default_next_id() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn applies_add_update_delete_and_close() {
        let tool = ToDoListTool::new();
        let mut state = TodoListState::default();

        let result = tool
            .run(
                json!({
                    "changes": "+ Read code\n+ Implement tool\n~ 1 in_progress"
                }),
                &mut state,
            )
            .await;
        assert!(!result.is_error);
        assert!(state.active);
        assert_eq!(state.tasks.len(), 2);
        assert_eq!(state.tasks[0].status, TodoStatus::InProgress);

        let result = tool
            .run(
                json!({ "changes": "~ 1 done\n~ 2 in_progress" }),
                &mut state,
            )
            .await;
        assert!(!result.is_error);
        assert_eq!(state.tasks[0].status, TodoStatus::Done);
        assert_eq!(state.tasks[1].status, TodoStatus::InProgress);

        let result = tool
            .run(json!({ "changes": "- 2\nclose" }), &mut state)
            .await;
        assert!(!result.is_error);
        assert!(!state.active);
        assert!(state.tasks.is_empty());
    }

    #[tokio::test]
    async fn only_keeps_one_in_progress_task() {
        let tool = ToDoListTool::new();
        let mut state = TodoListState::default();
        let result = tool
            .run(
                json!({
                    "changes": "+ [in_progress] First\n+ [in_progress] Second"
                }),
                &mut state,
            )
            .await;
        assert!(!result.is_error);
        assert_eq!(state.tasks[0].status, TodoStatus::Pending);
        assert_eq!(state.tasks[1].status, TodoStatus::InProgress);
    }
}
