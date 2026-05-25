use std::{
    path::{Component, Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use claakecode_core::ToolDescriptor;
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

const MAX_LIMIT: usize = 1000;
const STDERR_LIMIT: usize = 8 * 1024;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone)]
pub struct GlobTool {
    workspace_root: PathBuf,
    timeout: Duration,
}

impl GlobTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "Glob".into(),
            description: "Find workspace files by glob pattern.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "File glob pattern, e.g. \"**/*.rs\", \"src/**/*.tsx\" or \"*.{json,toml}\"."
                    },
                    "path": {
                        "type": "string",
                        "description": "Optional file or directory to search. Relative paths are resolved from the workspace root; absolute paths are allowed. Defaults to the workspace root."
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_LIMIT,
                        "description": "Required maximum number of file paths to show. Hard-capped at 1000."
                    }
                },
                "required": ["pattern", "limit"],
                "additionalProperties": false
            }),
        }
    }

    pub async fn run(&self, input: Value) -> ToolRunResult {
        match self.find(input).await {
            Ok(output) => ToolRunResult::ok(output, Vec::new()),
            Err(err) => ToolRunResult::err(err.to_string(), Vec::new()),
        }
    }

    async fn find(&self, input: Value) -> Result<String> {
        let parsed: GlobInput = serde_json::from_value(input)
            .map_err(|err| anyhow::anyhow!("invalid Glob input: {err}"))?;
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
        let result = timeout(
            self.timeout,
            self.run_ripgrep_files(pattern, &target.arg, limit),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Glob timed out after {}s", self.timeout.as_secs()))??;

        Ok(format_output(result))
    }

    fn resolve_target(&self, raw_path: Option<&str>) -> Result<GlobTarget> {
        let Some(raw_path) = raw_path.map(str::trim).filter(|value| !value.is_empty()) else {
            return Ok(GlobTarget { arg: ".".into() });
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

            return Ok(GlobTarget {
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

        Ok(GlobTarget {
            arg: if normalized.is_empty() {
                ".".into()
            } else {
                normalized.clone()
            },
        })
    }

    async fn run_ripgrep_files(
        &self,
        pattern: &str,
        target: &str,
        limit: usize,
    ) -> Result<GlobSearchResult> {
        let mut command = Command::new(ripgrep_executable());
        command
            .arg("--files")
            .arg("--hidden")
            .arg("--color")
            .arg("never")
            .arg("--no-messages")
            .arg("--sort")
            .arg("path")
            .arg("-g")
            .arg("!.git/**")
            .arg("-g")
            .arg(pattern)
            .arg("--")
            .arg(target)
            .current_dir(&self.workspace_root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);

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
        let mut paths = Vec::new();
        let mut total_matches = 0usize;

        while let Some(line) = reader
            .next_line()
            .await
            .context("unable to read ripgrep output")?
        {
            let relative_path = workspace_relative_path(&self.workspace_root, &line)?;
            if relative_path.is_empty() {
                continue;
            }

            total_matches += 1;
            if paths.len() < limit {
                paths.push(relative_path);
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

        Ok(GlobSearchResult {
            paths,
            total_matches,
        })
    }
}

#[derive(Debug, Deserialize)]
struct GlobInput {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug)]
struct GlobTarget {
    arg: String,
}

#[derive(Debug)]
struct GlobSearchResult {
    paths: Vec<String>,
    total_matches: usize,
}

fn workspace_relative_path(root: &Path, raw_path: &str) -> Result<String> {
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

fn format_output(result: GlobSearchResult) -> String {
    let shown = result.paths.len();
    let mut output = String::new();
    output.push_str(&format!("matches: {}\n", result.total_matches));
    if shown < result.total_matches {
        output.push_str(&format!("shown: {shown}\n"));
    }

    if result.paths.is_empty() {
        output.push_str("\nNo files matched.");
        return output;
    }

    output.push('\n');
    for path in result.paths {
        output.push_str(&path);
        output.push('\n');
    }

    output.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        process::Command as StdCommand,
        sync::atomic::{AtomicUsize, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn glob_returns_matching_files_with_limit() {
        if !ripgrep_available() {
            return;
        }

        let root = unique_temp_dir();
        fs::create_dir_all(root.join("src").join("nested")).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(root.join("src").join("one.rs"), "").expect("write first file");
        fs::write(root.join("src").join("nested").join("two.rs"), "").expect("write nested file");
        fs::write(root.join("src").join("three.ts"), "").expect("write ignored file");

        let tool = GlobTool::new(&root);
        let result = tool
            .find(json!({
                "pattern": "**/*.rs",
                "path": "src",
                "limit": 1
            }))
            .await
            .expect("glob should succeed");

        assert!(!result.contains("pattern:"));
        assert!(!result.contains("path:"));
        assert!(result.contains("matches: 2"));
        assert!(result.contains("shown: 1"));
        assert!(result.contains("src/nested/two.rs") || result.contains("src/one.rs"));
        assert!(!result.contains("src/three.ts"));

        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn glob_reports_no_matches() {
        if !ripgrep_available() {
            return;
        }

        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(root.join("app.ts"), "").expect("write file");

        let tool = GlobTool::new(&root);
        let result = tool
            .find(json!({ "pattern": "**/*.rs", "limit": 10 }))
            .await
            .expect("glob should succeed");

        assert!(result.contains("matches: 0"));
        assert!(result.contains("No files matched."));

        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn glob_finds_explicit_hidden_paths() {
        if !ripgrep_available() {
            return;
        }

        let root = unique_temp_dir();
        fs::create_dir_all(root.join(".github").join("workflows")).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(root.join(".github").join("workflows").join("ci.yml"), "")
            .expect("write hidden file");

        let tool = GlobTool::new(&root);
        let result = tool
            .find(json!({ "pattern": ".github/**/*.yml", "limit": 10 }))
            .await
            .expect("glob should succeed");

        assert!(result.contains("matches: 1"));
        assert!(result.contains(".github/workflows/ci.yml"));

        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn glob_accepts_absolute_workspace_path() {
        if !ripgrep_available() {
            return;
        }

        let root = unique_temp_dir();
        fs::create_dir_all(root.join("src")).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(root.join("src").join("one.rs"), "").expect("write rust file");
        fs::write(root.join("src").join("two.ts"), "").expect("write ts file");
        let absolute_src = root.join("src").canonicalize().expect("canonical src");

        let tool = GlobTool::new(&root);
        let result = tool
            .find(json!({
                "pattern": "**/*.rs",
                "path": absolute_src.display().to_string(),
                "limit": 10
            }))
            .await
            .expect("glob should accept absolute workspace paths");

        assert!(result.contains("matches: 1"));
        assert!(result.contains("src/one.rs"));
        assert!(!result.contains("src/two.ts"));

        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn glob_accepts_absolute_path_outside_workspace() {
        if !ripgrep_available() {
            return;
        }

        let base = unique_temp_dir();
        let workspace = base.join("workspace");
        let external = base.join("external");
        fs::create_dir_all(&workspace).expect("create temp workspace");
        fs::create_dir_all(&external).expect("create external directory");
        let workspace = workspace.canonicalize().expect("canonical temp workspace");
        fs::write(external.join("outside.rs"), "").expect("write external rust file");
        fs::write(external.join("outside.ts"), "").expect("write external ts file");
        let external = external
            .canonicalize()
            .expect("canonical external directory");
        let external_file = external.join("outside.rs");

        let tool = GlobTool::new(&workspace);
        let result = tool
            .find(json!({
                "pattern": "**/*.rs",
                "path": external.display().to_string(),
                "limit": 10
            }))
            .await
            .expect("glob should accept absolute paths outside the workspace");

        assert!(result.contains("matches: 1"));
        assert!(result.contains(&external_file.display().to_string()));
        assert!(!result.contains("outside.ts"));

        fs::remove_dir_all(base).ok();
    }

    #[tokio::test]
    async fn glob_requires_limit() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");

        let tool = GlobTool::new(&root);
        let error = tool
            .find(json!({ "pattern": "**/*.rs" }))
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
        static NEXT_TEMP_ID: AtomicUsize = AtomicUsize::new(0);

        let counter = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "claakecode-glob-test-{}-{counter}-{nanos}",
            std::process::id()
        ))
    }
}
