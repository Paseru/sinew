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

#[derive(Debug, Clone)]
pub struct DeleteFileTool {
    workspace_root: PathBuf,
}

impl DeleteFileTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    pub fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: tool_names::DELETE_FILE.into(),
            description: "Delete a file or empty directory inside the workspace.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file or empty directory to delete."
                    }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        }
    }

    pub async fn run(&self, input: Value) -> ToolRunResult {
        match self.delete(input) {
            Ok(output) => ToolRunResult::ok(output, Vec::new()),
            Err(err) => ToolRunResult::err(err.to_string(), Vec::new()),
        }
    }

    fn delete(&self, input: Value) -> Result<String> {
        let parsed: DeleteFileInput = serde_json::from_value(input)
            .map_err(|err| anyhow::anyhow!("invalid delete_file input: {err}"))?;
        let relative = parsed.path.trim();
        if relative.is_empty() {
            bail!("path is required");
        }
        let relative = normalize_workspace_relative_path(relative)?;
        let target = resolve_workspace_path(&self.workspace_root, &relative)?;
        if !target.exists() {
            bail!("path not found: {relative}");
        }
        if target.is_dir() {
            std::fs::remove_dir(&target)
                .map_err(|err| anyhow::anyhow!("unable to delete directory `{relative}`: {err}"))?;
        } else {
            std::fs::remove_file(&target)
                .map_err(|err| anyhow::anyhow!("unable to delete file `{relative}`: {err}"))?;
        }
        Ok(format!("deleted {relative}"))
    }
}

#[derive(Debug, Deserialize)]
struct DeleteFileInput {
    path: String,
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[tokio::test]
    async fn deletes_workspace_file() {
        let root = std::env::temp_dir().join(format!(
            "sinew-delete-file-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("tmp.txt"), "x").unwrap();
        let tool = DeleteFileTool::new(&root);
        let result = tool.run(json!({ "path": "tmp.txt" })).await;
        assert!(!result.is_error);
        assert!(!root.join("tmp.txt").exists());
        let _ = fs::remove_dir_all(root);
    }
}
