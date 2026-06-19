use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};
use sinew_core::ToolDescriptor;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::tool_names;
use crate::tool_run::ToolRunResult;

pub struct DocTool {
    workspace_root: PathBuf,
}

impl DocTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    pub fn descriptors(&self) -> Vec<ToolDescriptor> {
        vec![
            ToolDescriptor {
                name: tool_names::DOC_READ.into(),
                description: "Read the text content of a document (PDF, DOCX, XLSX, PPTX). Returns structured text extraction.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Absolute or workspace-relative path to the document."
                        }
                    },
                    "required": ["path"],
                    "additionalProperties": false
                }),
            },
            ToolDescriptor {
                name: tool_names::DOC_EDIT.into(),
                description: "Surgically edit a document (DOCX, PDF, XLSX, PPTX) without regenerating it from scratch. Preserves styles, images, and structure. Supported operations: find_replace, insert_after (DOCX only), delete_paragraph (DOCX only).".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Absolute or workspace-relative path to the document."
                        },
                        "op": {
                            "type": "string",
                            "enum": ["find_replace", "insert_after", "delete_paragraph"],
                            "description": "Operation to perform."
                        },
                        "find": {
                            "type": "string",
                            "description": "Text to search for (find_replace)."
                        },
                        "replace": {
                            "type": "string",
                            "description": "Replacement text (find_replace)."
                        },
                        "all": {
                            "type": "boolean",
                            "description": "Replace all occurrences. Default: true (find_replace)."
                        },
                        "after": {
                            "type": "string",
                            "description": "Anchor paragraph text — insert new content after this paragraph (insert_after)."
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to insert (insert_after)."
                        },
                        "paragraph": {
                            "type": "string",
                            "description": "Paragraph text (or substring) to delete (delete_paragraph)."
                        }
                    },
                    "required": ["path", "op"],
                    "additionalProperties": false
                }),
            },
        ]
    }

    pub async fn run_read(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize)]
        struct Input { path: String }
        let args: Input = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => return ToolRunResult::err(format!("invalid input: {e}"), vec![]),
        };
        let path = self.resolve_path(&args.path);
        self.call_python(json!({ "op": "read", "path": path })).await
    }

    pub async fn run_edit(&self, input: Value) -> ToolRunResult {
        let path = match input.get("path").and_then(|v| v.as_str()) {
            Some(p) => self.resolve_path(p),
            None => return ToolRunResult::err("missing path", vec![]),
        };
        let mut cmd = input.clone();
        cmd["path"] = Value::String(path);
        self.call_python(cmd).await
    }

    fn resolve_path(&self, path: &str) -> String {
        let p = PathBuf::from(path);
        if p.is_absolute() {
            path.to_string()
        } else {
            self.workspace_root.join(p).to_string_lossy().to_string()
        }
    }

    async fn call_python(&self, cmd: Value) -> ToolRunResult {
        let python = match find_python() {
            Ok(p) => p,
            Err(e) => return ToolRunResult::err(e.to_string(), vec![]),
        };
        let script = find_script();
        if !script.exists() {
            return ToolRunResult::err(
                format!(
                    "sinew_docs.py not found at {}. Expected next to the binary or in sinew-sidecar/.",
                    script.display()
                ),
                vec![],
            );
        }

        let input_bytes = match serde_json::to_vec(&cmd) {
            Ok(b) => b,
            Err(e) => return ToolRunResult::err(format!("serialize error: {e}"), vec![]),
        };

        let result = tokio::time::timeout(Duration::from_secs(60), async {
            let mut child = Command::new(&python)
                .arg(&script)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;

            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(&input_bytes).await?;
                // Drop stdin to signal EOF
            }

            child.wait_with_output().await
        })
        .await;

        match result {
            Err(_) => ToolRunResult::err("doc operation timed out (60s)", vec![]),
            Ok(Err(e)) => ToolRunResult::err(format!("spawn error: {e}"), vec![]),
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                if stdout.trim().is_empty() {
                    let err = if stderr.is_empty() {
                        "no output from Python sidecar".to_string()
                    } else {
                        format!("Python error: {stderr}")
                    };
                    return ToolRunResult::err(err, vec![]);
                }

                match serde_json::from_str::<Value>(stdout.trim()) {
                    Err(e) => ToolRunResult::err(
                        format!("invalid JSON from sidecar: {e}\nOutput: {stdout}"),
                        vec![],
                    ),
                    Ok(v) => {
                        let ok = v.get("ok").and_then(|b| b.as_bool()).unwrap_or(false);
                        if !ok {
                            let err = v
                                .get("error")
                                .and_then(|e| e.as_str())
                                .unwrap_or("unknown error")
                                .to_string();
                            ToolRunResult::err(err, vec![])
                        } else {
                            // Build human-readable result
                            let text = if let Some(content) = v.get("content").and_then(|c| c.as_str()) {
                                content.to_string()
                            } else if let Some(n) = v.get("replacements").and_then(|n| n.as_i64()) {
                                format!("{n} replacement(s) applied and saved.")
                            } else {
                                "Operation completed successfully.".to_string()
                            };
                            ToolRunResult::ok(text, vec![])
                        }
                    }
                }
            }
        }
    }
}

fn find_python() -> anyhow::Result<String> {
    for candidate in ["python3", "python"] {
        let ok = std::process::Command::new(candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return Ok(candidate.to_string());
        }
    }
    Err(anyhow::anyhow!(
        "Python not found. Install Python 3.8+ and run: pip install python-docx PyMuPDF openpyxl python-pptx"
    ))
}

fn find_script() -> PathBuf {
    if let Ok(p) = std::env::var("SINEW_DOCS_SCRIPT") {
        return PathBuf::from(p);
    }
    // Next to the binary (production bundle)
    if let Ok(exe) = std::env::current_exe() {
        let candidate = exe.parent().unwrap_or(exe.as_path()).join("sinew_docs.py");
        if candidate.exists() {
            return candidate;
        }
    }
    // Dev: relative to CWD (project root when running `tauri dev`)
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join("sinew-sidecar").join("sinew_docs.py");
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from("sinew-sidecar/sinew_docs.py")
}
