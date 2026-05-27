use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use sinew_core::ToolDescriptor;
use tokio::{process::Command, time::timeout};

use crate::{
    editor_diagnostics::{EditorDiagnostic, SharedEditorDiagnosticsStore},
    tool_names,
    tool_run::ToolRunResult,
    workspace::{normalize_workspace_relative_path, resolve_workspace_path},
};

const MAX_DIAGNOSTICS: usize = 200;
const PROJECT_LINT_TIMEOUT: Duration = Duration::from_secs(45);
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DiagnosticKey {
    path: String,
    line: u32,
    column: u32,
    severity: String,
    message: String,
}

#[derive(Debug, Clone)]
pub struct ReadLintsTool {
    workspace_root: PathBuf,
    editor_store: SharedEditorDiagnosticsStore,
}

impl ReadLintsTool {
    pub fn new(
        workspace_root: impl Into<PathBuf>,
        editor_store: SharedEditorDiagnosticsStore,
    ) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            editor_store,
        }
    }

    pub fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: tool_names::READ_LINTS.into(),
            description: "Read linter/compiler diagnostics for workspace files. Uses live editor diagnostics when available, plus project linters (cargo, eslint, ruff) for supported languages.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "paths": {
                        "oneOf": [
                            { "type": "string" },
                            { "type": "array", "items": { "type": "string" } }
                        ],
                        "description": "Optional file or directory paths (relative to the workspace root). When omitted, returns diagnostics for all known open editor files plus workspace-wide checks where available."
                    }
                },
                "additionalProperties": false
            }),
        }
    }

    pub async fn run(&self, input: Value) -> ToolRunResult {
        match self.read(input).await {
            Ok(output) => ToolRunResult::ok(output, Vec::new()),
            Err(err) => ToolRunResult::err(err.to_string(), Vec::new()),
        }
    }

    async fn read(&self, input: Value) -> Result<String> {
        let parsed: ReadLintsInput = serde_json::from_value(input)
            .map_err(|err| anyhow::anyhow!("invalid read_lints input: {err}"))?;
        let requested_paths = normalize_requested_paths(parsed.paths)?;
        let path_filter = if requested_paths.is_empty() {
            None
        } else {
            Some(requested_paths.as_slice())
        };
        let diagnostics = self.collect_editor_diagnostics(path_filter);
        let project_paths = if requested_paths.is_empty() {
            self.default_project_paths(&diagnostics)
        } else {
            requested_paths
        };
        let mut diagnostics = diagnostics;
        diagnostics.extend(
            self.collect_project_diagnostics(&project_paths)
                .await
                .context("project linter failed")?,
        );
        let mut diagnostics = dedupe_diagnostics(diagnostics);
        diagnostics.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then(left.line.cmp(&right.line))
                .then(left.column.cmp(&right.column))
        });
        diagnostics.truncate(MAX_DIAGNOSTICS);
        Ok(format_diagnostics(&diagnostics))
    }

    fn collect_editor_diagnostics(&self, paths: Option<&[String]>) -> Vec<EditorDiagnostic> {
        let store = self
            .editor_store
            .read()
            .unwrap_or_else(|err| err.into_inner());
        store.matching(paths).cloned().collect()
    }

    fn default_project_paths(&self, editor: &[EditorDiagnostic]) -> Vec<String> {
        let mut paths = HashSet::new();
        for diag in editor {
            paths.insert(diag.path.clone());
        }
        if paths.is_empty() {
            if self.workspace_root.join("Cargo.toml").is_file() {
                paths.insert(".".to_string());
            }
        }
        paths.into_iter().collect()
    }

    async fn collect_project_diagnostics(
        &self,
        paths: &[String],
    ) -> Result<Vec<EditorDiagnostic>> {
        let mut out = Vec::new();
        let mut rust_paths = Vec::new();
        let mut js_paths = Vec::new();
        let mut py_paths = Vec::new();

        for path in paths {
            let normalized = normalize_workspace_relative_path(path)?;
            if normalized.is_empty() {
                if self.workspace_root.join("Cargo.toml").is_file() {
                    rust_paths.push(".".to_string());
                }
                continue;
            }
            let absolute = resolve_workspace_path(&self.workspace_root, &normalized)?;
            if absolute.is_dir() {
                if normalized == "." || self.workspace_root.join("Cargo.toml").is_file() {
                    rust_paths.push(normalized);
                }
                continue;
            }
            if is_rust_path(&absolute) {
                rust_paths.push(normalized);
            } else if is_js_path(&absolute) {
                js_paths.push(normalized);
            } else if is_py_path(&absolute) {
                py_paths.push(normalized);
            }
        }

        if !rust_paths.is_empty() && self.workspace_root.join("Cargo.toml").is_file() {
            out.extend(run_cargo_check(&self.workspace_root, &rust_paths).await?);
        }
        if !js_paths.is_empty() && eslint_config_exists(&self.workspace_root) {
            out.extend(run_eslint(&self.workspace_root, &js_paths).await?);
        }
        if !py_paths.is_empty() {
            out.extend(run_ruff(&self.workspace_root, &py_paths).await?);
        }
        Ok(out)
    }
}

#[derive(Debug, Deserialize)]
struct ReadLintsInput {
    #[serde(default)]
    paths: Option<Value>,
}

fn normalize_requested_paths(paths: Option<Value>) -> Result<Vec<String>> {
    let Some(paths) = paths else {
        return Ok(Vec::new());
    };
    let values = match paths {
        Value::String(path) => vec![path],
        Value::Array(items) => items
            .into_iter()
            .filter_map(|value| value.as_str().map(str::to_string))
            .collect(),
        _ => {
            return Err(anyhow::anyhow!(
                "paths must be a string or an array of strings"
            ))
        }
    };
    let mut out = Vec::new();
    for path in values {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            continue;
        }
        out.push(normalize_workspace_relative_path(trimmed)?.to_string());
    }
    Ok(out)
}

fn format_diagnostics(diagnostics: &[EditorDiagnostic]) -> String {
    if diagnostics.is_empty() {
        return "No linter errors found.".to_string();
    }
    diagnostics
        .iter()
        .map(|diag| {
            let source = if diag.source.trim().is_empty() {
                String::new()
            } else {
                format!(" ({})", diag.source.trim())
            };
            format!(
                "{}:{}:{}-{}: {}{}",
                diag.path,
                diag.line,
                diag.column,
                diag.severity,
                diag.message.trim(),
                source
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_rust_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
}

fn is_js_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase()),
        Some(ext) if matches!(
            ext.as_str(),
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs"
        )
    )
}

fn is_py_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("py"))
}

fn dedupe_diagnostics(diagnostics: Vec<EditorDiagnostic>) -> Vec<EditorDiagnostic> {
    let mut seen = HashSet::new();
    diagnostics
        .into_iter()
        .filter(|diag| seen.insert(diag_key(diag)))
        .collect()
}

fn eslint_config_exists(workspace_root: &Path) -> bool {
    [
        "eslint.config.js",
        "eslint.config.mjs",
        "eslint.config.cjs",
        "eslint.config.ts",
        ".eslintrc",
        ".eslintrc.js",
        ".eslintrc.cjs",
        ".eslintrc.json",
        ".eslintrc.yaml",
        ".eslintrc.yml",
    ]
    .iter()
    .any(|name| workspace_root.join(name).is_file())
}

async fn run_cargo_check(workspace_root: &Path, paths: &[String]) -> Result<Vec<EditorDiagnostic>> {
    let mut command = Command::new("cargo");
    command
        .arg("check")
        .arg("--message-format=json")
        .current_dir(workspace_root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    let output = timeout(PROJECT_LINT_TIMEOUT, command.output())
        .await
        .context("cargo check timed out")?
        .context("unable to run cargo check")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let allowed = path_prefixes(workspace_root, paths)?;
    parse_cargo_messages(&stdout, &allowed)
}

fn path_prefixes(workspace_root: &Path, paths: &[String]) -> Result<HashSet<String>> {
    let mut allowed = HashSet::new();
    for path in paths {
        if path == "." {
            return Ok(HashSet::new());
        }
        let absolute = resolve_workspace_path(workspace_root, path)?;
        allowed.insert(normalize_path_string(&absolute));
    }
    Ok(allowed)
}

fn normalize_path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn parse_cargo_messages(stdout: &str, allowed: &HashSet<String>) -> Result<Vec<EditorDiagnostic>> {
    let filter_all = allowed.is_empty();
    let mut diagnostics = Vec::new();
    let mut seen = HashSet::new();

    for line in stdout.lines() {
        let value: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if value.get("reason").and_then(Value::as_str) != Some("compiler-message") {
            continue;
        }
        let message = value
            .pointer("/message")
            .ok_or_else(|| anyhow::anyhow!("missing cargo message payload"))?;
        let level = message
            .get("level")
            .and_then(Value::as_str)
            .unwrap_or("warning");
        if !matches!(level, "error" | "warning") {
            continue;
        }
        let spans = message
            .get("spans")
            .and_then(Value::as_array)
            .filter(|spans| !spans.is_empty())
            .ok_or_else(|| anyhow::anyhow!("missing cargo span"))?;
        let primary = spans
            .iter()
            .find(|span| span.get("is_primary").and_then(Value::as_bool).unwrap_or(false))
            .or_else(|| spans.first())
            .context("missing primary cargo span")?;
        let file_name = primary
            .get("file_name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if file_name.is_empty() {
            continue;
        }
        let absolute = normalize_path_string(Path::new(file_name));
        if !filter_all && !allowed.iter().any(|prefix| absolute.starts_with(prefix)) {
            continue;
        }
        let relative = workspace_relative_from_absolute(file_name);
        let rendered = message
            .get("rendered")
            .and_then(Value::as_str)
            .or_else(|| message.get("message").and_then(Value::as_str))
            .unwrap_or("compiler diagnostic");
        let line = primary.get("line_start").and_then(Value::as_u64).unwrap_or(1) as u32;
        let column = primary.get("column_start").and_then(Value::as_u64).unwrap_or(1) as u32;
        let end_line = primary.get("line_end").and_then(Value::as_u64).unwrap_or(line as u64) as u32;
        let end_column =
            primary.get("column_end").and_then(Value::as_u64).unwrap_or(column as u64) as u32;
        let diag = EditorDiagnostic {
            path: relative,
            line,
            column,
            end_line,
            end_column,
            severity: level.to_string(),
            message: rendered.trim().to_string(),
            source: "rustc".to_string(),
        };
        if seen.insert(diag_key(&diag)) {
            diagnostics.push(diag);
        }
    }
    Ok(diagnostics)
}

fn workspace_relative_from_absolute(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

async fn run_eslint(workspace_root: &Path, paths: &[String]) -> Result<Vec<EditorDiagnostic>> {
    let mut command = Command::new("npx");
    command
        .arg("--yes")
        .arg("eslint")
        .arg("--format")
        .arg("json")
        .current_dir(workspace_root);
    for path in paths {
        command.arg(path);
    }
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    let output = timeout(PROJECT_LINT_TIMEOUT, command.output())
        .await
        .context("eslint timed out")?
        .context("unable to run eslint")?;
    if output.stdout.is_empty() {
        return Ok(Vec::new());
    }
    parse_eslint_json(&String::from_utf8_lossy(&output.stdout))
}

fn parse_eslint_json(stdout: &str) -> Result<Vec<EditorDiagnostic>> {
    let files: Value = serde_json::from_str(stdout).context("invalid eslint json output")?;
    let Some(files) = files.as_array() else {
        return Ok(Vec::new());
    };
    let mut diagnostics = Vec::new();
    for file in files {
        let Some(path) = file.get("filePath").and_then(Value::as_str) else {
            continue;
        };
        let relative = workspace_relative_from_absolute(path);
        let Some(messages) = file.get("messages").and_then(Value::as_array) else {
            continue;
        };
        for message in messages {
            let severity = match message.get("severity").and_then(Value::as_u64) {
                Some(2) => "error",
                Some(1) => "warning",
                _ => "info",
            };
            diagnostics.push(EditorDiagnostic {
                path: relative.clone(),
                line: message.get("line").and_then(Value::as_u64).unwrap_or(1) as u32,
                column: message.get("column").and_then(Value::as_u64).unwrap_or(1) as u32,
                end_line: message.get("endLine").and_then(Value::as_u64).unwrap_or(1) as u32,
                end_column: message.get("endColumn").and_then(Value::as_u64).unwrap_or(1) as u32,
                severity: severity.to_string(),
                message: message
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("eslint diagnostic")
                    .to_string(),
                source: message
                    .get("ruleId")
                    .and_then(Value::as_str)
                    .unwrap_or("eslint")
                    .to_string(),
            });
        }
    }
    Ok(diagnostics)
}

async fn run_ruff(workspace_root: &Path, paths: &[String]) -> Result<Vec<EditorDiagnostic>> {
    let mut command = Command::new("ruff");
    command
        .arg("check")
        .arg("--output-format")
        .arg("json")
        .current_dir(workspace_root);
    for path in paths {
        command.arg(path);
    }
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    let output = timeout(PROJECT_LINT_TIMEOUT, command.output())
        .await
        .context("ruff timed out")?
        .context("unable to run ruff")?;
    if output.stdout.is_empty() {
        return Ok(Vec::new());
    }
    parse_ruff_json(&String::from_utf8_lossy(&output.stdout))
}

fn parse_ruff_json(stdout: &str) -> Result<Vec<EditorDiagnostic>> {
    let items: Value = serde_json::from_str(stdout).context("invalid ruff json output")?;
    let Some(items) = items.as_array() else {
        return Ok(Vec::new());
    };
    let mut diagnostics = Vec::new();
    for item in items {
        let filename = item
            .get("filename")
            .and_then(Value::as_str)
            .unwrap_or_default();
        diagnostics.push(EditorDiagnostic {
            path: workspace_relative_from_absolute(filename),
            line: item.get("location").and_then(|v| v.get("row")).and_then(Value::as_u64).unwrap_or(1) as u32,
            column: item.get("location").and_then(|v| v.get("column")).and_then(Value::as_u64).unwrap_or(1) as u32,
            end_line: item.get("end_location").and_then(|v| v.get("row")).and_then(Value::as_u64).unwrap_or(1) as u32,
            end_column: item.get("end_location").and_then(|v| v.get("column")).and_then(Value::as_u64).unwrap_or(1) as u32,
            severity: "warning".to_string(),
            message: item
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("ruff diagnostic")
                .to_string(),
            source: item
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("ruff")
                .to_string(),
        });
    }
    Ok(diagnostics)
}

fn diag_key(diag: &EditorDiagnostic) -> DiagnosticKey {
    DiagnosticKey {
        path: diag.path.clone(),
        line: diag.line,
        column: diag.column,
        severity: diag.severity.clone(),
        message: diag.message.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_diagnostics_for_model() {
        let text = format_diagnostics(&[EditorDiagnostic {
            path: "src/main.rs".into(),
            line: 4,
            column: 12,
            end_line: 4,
            end_column: 18,
            severity: "error".into(),
            message: "expected `;`".into(),
            source: "rustc".into(),
        }]);
        assert!(text.contains("src/main.rs:4:12-error: expected `;` (rustc)"));
    }

    #[test]
    fn parses_cargo_json_diagnostics() {
        let stdout = r#"{"reason":"compiler-message","message":{"level":"error","message":"cannot find value `x`","rendered":"error: cannot find value `x`","spans":[{"file_name":"src/lib.rs","line_start":2,"column_start":5,"line_end":2,"column_end":6,"is_primary":true}]}}"#;
        let allowed = HashSet::from([normalize_path_string(Path::new("src/lib.rs"))]);
        let diagnostics = parse_cargo_messages(stdout, &allowed).expect("parse cargo");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].path, "src/lib.rs");
        assert_eq!(diagnostics[0].source, "rustc");
    }
}
