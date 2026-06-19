use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};
use sinew_core::ToolDescriptor;
use sinew_search::{search::search, SearchIndex};
use tokio::sync::Mutex;

use crate::tool_names;
use crate::tool_run::ToolRunResult;

pub struct SemanticSearchTool {
    workspace_root: PathBuf,
    index: Arc<Mutex<Option<SearchIndex>>>,
}

impl SemanticSearchTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root, index: Arc::new(Mutex::new(None)) }
    }

    pub fn descriptors(&self) -> Vec<ToolDescriptor> {
        vec![
            ToolDescriptor {
                name: tool_names::INDEX_WORKSPACE.into(),
                description: "Index the workspace codebase for semantic search. Run once before using semantic_search, or after major changes. Respects .gitignore. Incremental by default — only re-indexes changed files.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "force": {
                            "type": "boolean",
                            "description": "Force re-index all files, ignoring the cache. Default: false."
                        }
                    },
                    "additionalProperties": false
                }),
            },
            ToolDescriptor {
                name: tool_names::SEMANTIC_SEARCH.into(),
                description: "Search the workspace codebase by meaning — finds relevant code even without exact keyword matches. Use this to locate implementations, understand patterns, or find related code. Run index_workspace first.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Natural language or code description of what you're looking for."
                        },
                        "max_results": {
                            "type": "integer",
                            "description": "Max results to return. Default: 8.",
                            "minimum": 1,
                            "maximum": 30
                        }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }),
            },
        ]
    }

    pub async fn run_index(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize, Default)]
        struct Input {
            #[serde(default)]
            force: bool,
        }
        let args: Input = serde_json::from_value(input).unwrap_or_default();
        let workspace = self.workspace_root.clone();
        let force = args.force;

        let result = tokio::task::spawn_blocking(move || {
            let idx = SearchIndex::new(&workspace)?;
            idx.index_workspace(&workspace, force)
        })
        .await;

        match result {
            Ok(Ok(stats)) => {
                // Store index for subsequent searches
                let mut guard = self.index.lock().await;
                if guard.is_none() {
                    if let Ok(idx) = SearchIndex::new(&self.workspace_root) {
                        *guard = Some(idx);
                    }
                }
                ToolRunResult::ok(
                    format!(
                        "Indexed {} files ({} skipped, unchanged). Total: {} chunks indexed.",
                        stats.indexed, stats.skipped, stats.total_chunks
                    ),
                    vec![],
                )
            }
            Ok(Err(e)) => ToolRunResult::err(format!("indexing failed: {e}"), vec![]),
            Err(e) => ToolRunResult::err(format!("task panicked: {e}"), vec![]),
        }
    }

    pub async fn run_search(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize)]
        struct Input {
            query: String,
            #[serde(default = "default_max")]
            max_results: usize,
        }
        fn default_max() -> usize { 8 }

        let args: Input = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => return ToolRunResult::err(format!("invalid input: {e}"), vec![]),
        };

        // Ensure index is initialized
        {
            let mut guard = self.index.lock().await;
            if guard.is_none() {
                match SearchIndex::new(&self.workspace_root) {
                    Ok(idx) => *guard = Some(idx),
                    Err(e) => {
                        return ToolRunResult::err(
                            format!("index not available — run index_workspace first. Error: {e}"),
                            vec![],
                        )
                    }
                }
            }
            if guard.as_ref().map(|i| i.total_chunks()).unwrap_or(0) == 0 {
                return ToolRunResult::err(
                    "index is empty — run index_workspace first",
                    vec![],
                );
            }
        }

        let workspace = self.workspace_root.clone();
        let workspace2 = workspace.clone();
        let query = args.query.clone();
        let max_results = args.max_results.clamp(1, 30);

        let result = tokio::task::spawn_blocking(move || {
            let idx = SearchIndex::new(&workspace)?;
            search(&idx, &query, max_results)
        })
        .await;

        match result {
            Ok(Ok(results)) if results.is_empty() => {
                ToolRunResult::ok("No results found. Try running index_workspace or rephrasing.", vec![])
            }
            Ok(Ok(results)) => {
                let mut output = format!("{} results for \"{}\":\n\n", results.len(), args.query);
                for (i, r) in results.iter().enumerate() {
                    output.push_str(&format!(
                        "### {} — {}:{}-{} (score: {:.3})\n```\n{}\n```\n\n",
                        i + 1,
                        shorten_path(&r.file_path, &workspace2),
                        r.start_line + 1,
                        r.end_line + 1,
                        r.score,
                        truncate(&r.content, 800),
                    ));
                }
                ToolRunResult::ok(output, vec![])
            }
            Ok(Err(e)) => ToolRunResult::err(format!("search failed: {e}"), vec![]),
            Err(e) => ToolRunResult::err(format!("task panicked: {e}"), vec![]),
        }
    }
}

fn shorten_path(path: &str, workspace: &PathBuf) -> String {
    let ws = workspace.to_string_lossy();
    path.strip_prefix(ws.as_ref())
        .unwrap_or(path)
        .trim_start_matches(['/', '\\'])
        .to_string()
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut boundary = max;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &s[..boundary]
}
