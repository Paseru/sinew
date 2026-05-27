use std::path::PathBuf;

use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use sinew_core::ToolDescriptor;

use crate::{
    tool_names,
    tool_run::ToolRunResult,
    workspace::{normalize_workspace_relative_path, resolve_workspace_path},
};

const MAX_ENTRIES: usize = 500;

#[derive(Debug, Clone)]
pub struct ListDirTool {
    workspace_root: PathBuf,
}

impl ListDirTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    pub fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: tool_names::LIST_DIR.into(),
            description: "List files and directories in a single workspace directory (non-recursive).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory to list. Relative to the workspace root. Defaults to the workspace root."
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_ENTRIES,
                        "description": "Maximum entries to return."
                    }
                },
                "additionalProperties": false
            }),
        }
    }

    pub async fn run(&self, input: Value) -> ToolRunResult {
        match self.list(input) {
            Ok(output) => ToolRunResult::ok(output, Vec::new()),
            Err(err) => ToolRunResult::err(err.to_string(), Vec::new()),
        }
    }

    fn list(&self, input: Value) -> Result<String> {
        let parsed: ListDirInput = serde_json::from_value(input)
            .map_err(|err| anyhow::anyhow!("invalid list_dir input: {err}"))?;
        let limit = parsed.limit.unwrap_or(MAX_ENTRIES).clamp(1, MAX_ENTRIES);
        let relative = parsed
            .path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(".");
        let relative = normalize_workspace_relative_path(relative)?;
        let target = resolve_workspace_path(&self.workspace_root, &relative)?;
        if !target.is_dir() {
            bail!("path is not a directory: {relative}");
        }

        let mut dirs = Vec::new();
        let mut files = Vec::new();
        for entry in std::fs::read_dir(&target)? {
            let entry = entry?;
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if file_name.starts_with('.') && file_name != ".git" {
                continue;
            }
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                dirs.push(format!("{file_name}/"));
            } else if file_type.is_file() || file_type.is_symlink() {
                files.push(file_name);
            }
        }
        dirs.sort();
        files.sort();

        let mut lines = Vec::new();
        for name in dirs.into_iter().chain(files).take(limit) {
            lines.push(name);
        }
        if lines.is_empty() {
            return Ok(format!("{relative} is empty"));
        }
        Ok(lines.join("\n"))
    }
}

#[derive(Debug, Deserialize)]
struct ListDirInput {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[tokio::test]
    async fn lists_single_directory_level() {
        let root = std::env::temp_dir().join(format!(
            "sinew-list-dir-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("Cargo.toml"), "").unwrap();
        let tool = ListDirTool::new(&root);
        let result = tool
            .run(json!({ "path": "." }))
            .await;
        assert!(!result.is_error);
        assert!(result.content.contains("src/"));
        assert!(result.content.contains("Cargo.toml"));
        let _ = fs::remove_dir_all(root);
    }
}
