use std::path::PathBuf;

use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use sinew_core::ToolDescriptor;
use sinew_index::{ensure_workspace_index, search_workspace};

use crate::{tool_names, tool_run::ToolRunResult};

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 50;

#[derive(Debug, Clone)]
pub struct CodebaseSearchTool {
    workspace_root: PathBuf,
}

impl CodebaseSearchTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    pub fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: tool_names::CODEBASE_SEARCH.into(),
            description: "Search the local Sinew codebase index for relevant code chunks by meaning or keywords. Faster than grep for exploration; uses a local FTS index (no cloud).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Natural language or keyword query describing what to find in the codebase."
                    },
                    "path": {
                        "type": "string",
                        "description": "Optional path prefix within the workspace (e.g. src/auth)."
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_LIMIT,
                        "default": DEFAULT_LIMIT,
                        "description": "Maximum number of chunks to return."
                    }
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        }
    }

    pub async fn run(&self, input: Value) -> ToolRunResult {
        match self.search(input).await {
            Ok(output) => ToolRunResult::ok(output, Vec::new()),
            Err(err) => ToolRunResult::err(err.to_string(), Vec::new()),
        }
    }

    async fn search(&self, input: Value) -> Result<String> {
        let parsed: CodebaseSearchInput = serde_json::from_value(input)
            .map_err(|err| anyhow::anyhow!("invalid codebase_search input: {err}"))?;
        let query = parsed.query.trim();
        if query.is_empty() {
            bail!("query is required");
        }
        let limit = parsed.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        let path_prefix = parsed
            .path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());

        let stats = ensure_workspace_index(&self.workspace_root)?;
        let hits = search_workspace(&self.workspace_root, query, path_prefix, limit)?;

        let mut output = format!(
            "index: {} files, {} chunks (updated {} files this sync)\nresults: {}\n\n",
            stats.files_indexed,
            stats.chunks_indexed,
            stats.files_updated,
            hits.len()
        );
        if hits.is_empty() {
            output.push_str("No matches.");
            return Ok(output);
        }
        for hit in hits {
            output.push_str(&format!(
                "{}:{}-{} (score {:.2})\n{}\n\n",
                hit.path, hit.start_line, hit.end_line, hit.score, hit.snippet.trim()
            ));
        }
        Ok(output.trim_end().to_string())
    }
}

#[derive(Debug, Deserialize)]
struct CodebaseSearchInput {
    query: String,
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
    async fn codebase_search_finds_indexed_symbols() {
        let root = std::env::temp_dir().join(format!(
            "sinew-codebase-search-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(root.join("auth.rs"), "pub fn verify_session_token() {}\n").unwrap();

        let tool = CodebaseSearchTool::new(&root);
        let result = tool
            .search(json!({ "query": "verify" }))
            .await
            .expect("search should succeed");

        assert!(result.contains("auth.rs"));
        assert!(result.contains("verify"));
        let _ = fs::remove_dir_all(root);
    }
}
