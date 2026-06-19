use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{json, Value};
use sinew_core::ToolDescriptor;

use crate::tool_names;
use crate::tool_run::ToolRunResult;

const MEMORY_FILE: &str = ".sinew/memory.md";

pub struct WorkspaceMemoryTool {
    workspace_root: PathBuf,
}

impl WorkspaceMemoryTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    pub fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: tool_names::WORKSPACE_MEMORY.into(),
            description: "Read or write persistent workspace memory stored in .sinew/memory.md. Use this to remember important facts across sessions: architecture decisions, recurring issues, key contacts, project conventions. Call read at the start of a session to recall what you know.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["read", "append", "update", "clear"],
                        "description": "read: return current memory. append: add text at the end. update: replace entire memory with new content. clear: erase all memory."
                    },
                    "content": {
                        "type": "string",
                        "description": "Text to append or to replace the memory with (required for append/update)."
                    }
                },
                "required": ["action"],
                "additionalProperties": false
            }),
        }
    }

    pub async fn run(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize)]
        struct Input {
            action: String,
            content: Option<String>,
        }
        let args: Input = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => return ToolRunResult::err(format!("invalid input: {e}"), vec![]),
        };
        let memory_path = self.workspace_root.join(MEMORY_FILE);
        match args.action.as_str() {
            "read" => {
                let content = if memory_path.exists() {
                    std::fs::read_to_string(&memory_path)
                        .unwrap_or_else(|e| format!("error reading memory: {e}"))
                } else {
                    String::new()
                };
                if content.trim().is_empty() {
                    ToolRunResult::ok("(workspace memory is empty)", vec![])
                } else {
                    ToolRunResult::ok(content, vec![])
                }
            }
            "append" => {
                let Some(content) = args.content else {
                    return ToolRunResult::err("append requires content", vec![]);
                };
                if let Some(parent) = memory_path.parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        return ToolRunResult::err(format!("cannot create .sinew dir: {e}"), vec![]);
                    }
                }
                let existing = if memory_path.exists() {
                    std::fs::read_to_string(&memory_path).unwrap_or_default()
                } else {
                    String::new()
                };
                let new_content = if existing.is_empty() {
                    content
                } else if existing.ends_with('\n') {
                    format!("{existing}{content}")
                } else {
                    format!("{existing}\n{content}")
                };
                match std::fs::write(&memory_path, &new_content) {
                    Ok(_) => ToolRunResult::ok("memory updated", vec![]),
                    Err(e) => ToolRunResult::err(format!("write failed: {e}"), vec![]),
                }
            }
            "update" => {
                let Some(content) = args.content else {
                    return ToolRunResult::err("update requires content", vec![]);
                };
                if let Some(parent) = memory_path.parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        return ToolRunResult::err(format!("cannot create .sinew dir: {e}"), vec![]);
                    }
                }
                match std::fs::write(&memory_path, &content) {
                    Ok(_) => ToolRunResult::ok("memory replaced", vec![]),
                    Err(e) => ToolRunResult::err(format!("write failed: {e}"), vec![]),
                }
            }
            "clear" => {
                if memory_path.exists() {
                    if let Err(e) = std::fs::write(&memory_path, "") {
                        return ToolRunResult::err(format!("clear failed: {e}"), vec![]);
                    }
                }
                ToolRunResult::ok("memory cleared", vec![])
            }
            _ => ToolRunResult::err(format!("unknown action: {}", args.action), vec![]),
        }
    }
}
