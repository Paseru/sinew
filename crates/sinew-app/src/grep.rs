use std::{
    collections::{HashMap, HashSet},
    path::{Component, Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use sinew_core::ToolDescriptor;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::Command,
    time::timeout,
};

use crate::{
    ripgrep::ripgrep_executable,
    tool_run::ToolRunResult,
    workspace::{normalize_workspace_relative_path, resolve_workspace_path},
};

const MAX_LIMIT: usize = 500;
const MAX_LINE_CHARS: usize = 240;
const STDERR_LIMIT: usize = 8 * 1024;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct GrepTool {
    workspace_root: PathBuf,
    timeout: Duration,
}

impl GrepTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "Grep".into(),
            description: "Search files for text or regex patterns using ripgrep".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Text or ripgrep-compatible regex pattern to search for."
                    },
                    "path": {
                        "type": "string",
                        "description": "Optional file or directory to search. Relative paths are resolved from the workspace root; absolute paths are allowed. Defaults to the workspace root."
                    },
                    "include": {
                        "type": "string",
                        "description": "Optional file glob, e.g. \"*.rs\", \"*.{ts,tsx}\" or \"src/**/*.tsx\"."
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_LIMIT,
                        "description": "Required maximum number of matches to show. Hard-capped at 500."
                    }
                },
                "required": ["pattern", "limit"],
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
        let parsed: GrepInput = serde_json::from_value(input)
            .map_err(|err| anyhow::anyhow!("invalid Grep input: {err}"))?;
        let pattern = parsed.pattern.trim();
        if pattern.is_empty() {
            bail!("pattern is required");
        }

        let requested_limit = parsed
            .limit
            .ok_or_else(|| anyhow::anyhow!("limit is required"))?;
        if requested_limit == 0 {
            bail!("limit must be greater than 0");
        }
        let limit = requested_limit.min(MAX_LIMIT);
        let target = self.resolve_target(parsed.path.as_deref())?;
        let include = parsed
            .include
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);

        let result = timeout(
            self.timeout,
            self.run_ripgrep(pattern, &target.arg, include.as_deref(), limit),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Grep timed out after {}s", self.timeout.as_secs()))??;

        Ok(format_output(limit, requested_limit, result))
    }

    fn resolve_target(&self, raw_path: Option<&str>) -> Result<GrepTarget> {
        let Some(raw_path) = raw_path.map(str::trim).filter(|value| !value.is_empty()) else {
            return Ok(GrepTarget { arg: ".".into() });
        };

        let raw_candidate = Path::new(raw_path);
        if raw_candidate.is_absolute() {
            let path = raw_candidate
                .canonicalize()
                .with_context(|| format!("unable to resolve path {raw_path}"))?;
            let metadata = path
                .metadata()
                .with_context(|| format!("unable to read metadata for {}", path.display()))?;
            if !metadata.is_file() && !metadata.is_dir() {
                bail!("path must be a file or directory");
            }

            return Ok(GrepTarget {
                arg: path.display().to_string(),
            });
        }

        let normalized = normalize_workspace_relative_path(raw_path)?;
        let path = resolve_workspace_path(&self.workspace_root, &normalized)?;
        let metadata = path
            .metadata()
            .with_context(|| format!("unable to read metadata for {normalized}"))?;
        if !metadata.is_file() && !metadata.is_dir() {
            bail!("path must be a file or directory");
        }

        Ok(GrepTarget {
            arg: if normalized.is_empty() {
                ".".into()
            } else {
                normalized.clone()
            },
        })
    }

    async fn run_ripgrep(
        &self,
        pattern: &str,
        target: &str,
        include: Option<&str>,
        limit: usize,
    ) -> Result<GrepSearchResult> {
        let mut command = Command::new(ripgrep_executable());
        command
            .arg("--json")
            .arg("--line-number")
            .arg("--color")
            .arg("never")
            .arg("--no-messages")
            .arg("--with-filename");
        if let Some(include) = include {
            command.arg("-g").arg(include);
        }
        command
            .arg("--")
            .arg(pattern)
            .arg(target)
            .current_dir(&self.workspace_root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = command
            .spawn()
            .context("unable to spawn ripgrep (`rg` was not found in the app bundle or PATH)")?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("ripgrep stdout pipe missing"))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("ripgrep stderr pipe missing"))?;
        let stderr_task = tokio::spawn(async move { read_stderr(&mut stderr).await });

        let mut reader = BufReader::new(stdout).lines();
        let mut matches = Vec::new();
        let mut seen_files = HashSet::new();
        let mut total_matches = 0usize;

        while let Some(line) = reader
            .next_line()
            .await
            .context("unable to read ripgrep output")?
        {
            let Some(entry) = parse_match_line(&self.workspace_root, &line)? else {
                continue;
            };
            total_matches += 1;
            seen_files.insert(entry.relative_path.clone());
            if matches.len() < limit {
                matches.push(entry);
            }
        }

        let status = child.wait().await.context("ripgrep failed to exit")?;
        let stderr = stderr_task
            .await
            .unwrap_or_else(|err| Err(std::io::Error::other(err.to_string())))
            .context("unable to read ripgrep stderr")?;

        if !status.success() && status.code() != Some(1) {
            let message = stderr.trim();
            if message.is_empty() {
                bail!("ripgrep failed with status {status}");
            }
            bail!("ripgrep failed with status {status}: {message}");
        }

        Ok(GrepSearchResult {
            matches,
            seen_files: seen_files.len(),
            total_matches,
        })
    }
}

#[derive(Debug, Deserialize)]
struct GrepInput {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    include: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug)]
struct GrepTarget {
    arg: String,
}

#[derive(Debug)]
struct GrepSearchResult {
    matches: Vec<GrepMatch>,
    seen_files: usize,
    total_matches: usize,
}

#[derive(Debug)]
struct GrepMatch {
    relative_path: String,
    line_number: u64,
    line_text: String,
}

fn parse_match_line(root: &Path, line: &str) -> Result<Option<GrepMatch>> {
    let value: Value = serde_json::from_str(line)
        .with_context(|| format!("unable to parse ripgrep JSON line: {line}"))?;
    if value.get("type").and_then(Value::as_str) != Some("match") {
        return Ok(None);
    }

    let data = value
        .get("data")
        .ok_or_else(|| anyhow::anyhow!("ripgrep match missing data"))?;
    let raw_path = data
        .get("path")
        .and_then(|path| path.get("text"))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("ripgrep match missing path"))?;
    let line_number = data
        .get("line_number")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow::anyhow!("ripgrep match missing line number"))?;
    let raw_line = data
        .get("lines")
        .and_then(|lines| lines.get("text"))
        .and_then(Value::as_str)
        .unwrap_or_default();

    Ok(Some(GrepMatch {
        relative_path: display_match_path(root, raw_path)?,
        line_number,
        line_text: clip_line(raw_line),
    }))
}

fn display_match_path(root: &Path, raw_path: &str) -> Result<String> {
    let path = Path::new(raw_path);
    if path.is_absolute() {
        if let Ok(relative) = path.strip_prefix(root) {
            return Ok(path_to_slash_string(relative));
        }

        return Ok(path.display().to_string());
    }

    normalize_workspace_relative_path(raw_path)
}

fn path_to_slash_string(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

async fn read_stderr<R: AsyncReadExt + Unpin>(reader: &mut R) -> std::io::Result<String> {
    let mut bytes = Vec::with_capacity(STDERR_LIMIT.min(1024));
    let mut buffer = [0u8; 1024];
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let remaining = STDERR_LIMIT.saturating_sub(bytes.len());
        if remaining == 0 {
            continue;
        }
        bytes.extend_from_slice(&buffer[..remaining.min(read)]);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn clip_line(raw: &str) -> String {
    let line = raw.trim_end_matches(['\r', '\n']);
    let mut clipped = line.chars().take(MAX_LINE_CHARS).collect::<String>();
    if line.chars().count() > MAX_LINE_CHARS {
        clipped.push_str("...");
    }
    clipped
}

fn format_output(_limit: usize, _requested_limit: usize, result: GrepSearchResult) -> String {
    let shown = result.matches.len();

    let mut output = String::new();
    output.push_str(&format!(
        "matches: {}\nfiles: {}\n",
        result.total_matches, result.seen_files
    ));
    if shown < result.total_matches {
        output.push_str(&format!("shown: {shown}\n"));
    }

    if result.matches.is_empty() {
        output.push_str("\nNo matches.");
        return output;
    }

    let groups = group_matches(result.matches);
    for group in groups {
        output.push_str(&format!("\n{}\n", group.relative_path));
        for item in group.matches {
            output.push_str(&format!("  {} | {}\n", item.line_number, item.line_text));
        }
    }

    output.trim_end().to_string()
}

struct GrepGroup {
    relative_path: String,
    matches: Vec<GrepMatch>,
}

fn group_matches(matches: Vec<GrepMatch>) -> Vec<GrepGroup> {
    let mut groups = Vec::<GrepGroup>::new();
    let mut indexes = HashMap::<String, usize>::new();

    for entry in matches {
        if let Some(index) = indexes.get(&entry.relative_path).copied() {
            groups[index].matches.push(entry);
            continue;
        }

        let index = groups.len();
        indexes.insert(entry.relative_path.clone(), index);
        groups.push(GrepGroup {
            relative_path: entry.relative_path.clone(),
            matches: vec![entry],
        });
    }

    groups
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        process::Command as StdCommand,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn grep_returns_grouped_capped_output() {
        if !ripgrep_available() {
            return;
        }

        let root = unique_temp_dir();
        fs::create_dir_all(root.join("src")).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(
            root.join("src").join("one.rs"),
            "fn alpha() {}\nfn beta() {}\nfn alphabet() {}\n",
        )
        .expect("write first file");
        fs::write(root.join("src").join("two.txt"), "alpha\n").expect("write second file");

        let tool = GrepTool::new(&root);
        let result = tool
            .search(json!({
                "pattern": "alpha",
                "path": "src",
                "include": "*.rs",
                "limit": 1
            }))
            .await
            .expect("grep should succeed");

        assert!(!result.contains("path:"));
        assert!(!result.contains("include:"));
        assert!(result.contains("matches: 2"));
        assert!(result.contains("files: 1"));
        assert!(result.contains("shown: 1"));
        assert!(!result.contains("truncated:"));
        assert!(!result.contains("limit:"));
        assert!(result.contains("src/one.rs"));
        assert!(!result.contains("src/two.txt"));

        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn grep_reports_no_matches() {
        if !ripgrep_available() {
            return;
        }

        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(root.join("app.ts"), "const value = 1;\n").expect("write file");

        let tool = GrepTool::new(&root);
        let result = tool
            .search(json!({ "pattern": "missing", "limit": 10 }))
            .await
            .expect("grep should succeed");

        assert!(result.contains("matches: 0"));
        assert!(result.contains("files: 0"));
        assert!(result.contains("No matches."));

        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn grep_accepts_absolute_path_outside_workspace() {
        if !ripgrep_available() {
            return;
        }

        let base = unique_temp_dir();
        let workspace = base.join("workspace");
        let external = base.join("external");
        fs::create_dir_all(&workspace).expect("create temp workspace");
        fs::create_dir_all(&external).expect("create external directory");
        let workspace = workspace.canonicalize().expect("canonical temp workspace");
        let external = external
            .canonicalize()
            .expect("canonical external directory");
        let external_file = external.join("outside.txt");
        fs::write(&external_file, "needle\n").expect("write external file");

        let tool = GrepTool::new(&workspace);
        let result = tool
            .search(json!({
                "pattern": "needle",
                "path": external.display().to_string(),
                "limit": 10
            }))
            .await
            .expect("grep should search absolute paths outside the workspace");

        assert!(result.contains("matches: 1"));
        assert!(result.contains("files: 1"));
        assert!(result.contains(&external_file.display().to_string()));

        fs::remove_dir_all(base).ok();
    }

    #[tokio::test]
    async fn grep_requires_limit() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");

        let tool = GrepTool::new(&root);
        let error = tool
            .search(json!({ "pattern": "anything" }))
            .await
            .expect_err("missing limit should fail");

        assert!(error.to_string().contains("limit is required"));

        fs::remove_dir_all(root).ok();
    }

    fn ripgrep_available() -> bool {
        StdCommand::new(ripgrep_executable())
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("sinew-grep-test-{}-{nanos}", std::process::id()))
    }
}
