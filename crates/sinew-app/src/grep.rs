use std::{
    collections::{HashMap, HashSet},
    path::{Component, Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use anyhow::{bail, Context, Result};
use regex::Regex;
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
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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
                        "description": "Regex pattern to search for."
                    },
                    "path": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional files or directories to search. Relative paths are resolved from the workspace root; absolute paths are allowed. Defaults to the workspace root."
                    },
                    "include": {
                        "type": "string",
                        "description": "Optional file glob."
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_LIMIT,
                        "description": "Required maximum number of matches to show. Hard-capped at 500."
                    },
                    "output_mode": {
                        "type": "string",
                        "enum": ["context", "matches", "files", "count"],
                        "default": "context",
                        "description": "Output mode. context (default) groups file + line + content by file; matches prints only matched strings; files prints only paths with at least one match; count prints match counts per file."
                    },
                    "unique": {
                        "type": "boolean",
                        "default": false,
                        "description": "Deduplicate output rows. Most useful with output_mode=matches."
                    },
                    "exclude_pattern": {
                        "type": "string",
                        "description": "Optional regex; matches whose full line contains this regex are excluded."
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
        let pattern = parsed.pattern.trim().to_string();
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
        let targets = self.resolve_targets(parsed.path)?;
        let include = parsed
            .include
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let exclude_pattern = parsed
            .exclude_pattern
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let exclude_regex = exclude_pattern
            .map(|pattern| {
                Regex::new(pattern).with_context(|| format!("invalid exclude_pattern `{pattern}`"))
            })
            .transpose()?;

        let result = timeout(
            self.timeout,
            self.run_ripgrep(
                &pattern,
                &targets.args,
                include.as_deref(),
                limit,
                parsed.output_mode,
                parsed.unique,
                exclude_regex.as_ref(),
            ),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Grep timed out after {}s", self.timeout.as_secs()))??;

        Ok(format_output(limit, parsed.output_mode, result))
    }

    fn resolve_targets(&self, raw_path: Option<GrepPathInput>) -> Result<GrepTargets> {
        let mut args = Vec::new();
        let mut seen = HashSet::new();

        for raw_path in raw_path
            .map(GrepPathInput::into_vec)
            .unwrap_or_default()
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            let target = self.resolve_single_target(&raw_path)?;
            if seen.insert(target.clone()) {
                args.push(target);
            }
        }

        if args.is_empty() {
            args.push(".".into());
        }

        Ok(GrepTargets { args })
    }

    fn resolve_single_target(&self, raw_path: &str) -> Result<String> {
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

            return Ok(path.display().to_string());
        }

        let normalized = normalize_workspace_relative_path(raw_path)?;
        let path = resolve_workspace_path(&self.workspace_root, &normalized)?;
        let metadata = path
            .metadata()
            .with_context(|| format!("unable to read metadata for {normalized}"))?;
        if !metadata.is_file() && !metadata.is_dir() {
            bail!("path must be a file or directory");
        }

        Ok(if normalized.is_empty() {
            ".".into()
        } else {
            normalized.clone()
        })
    }

    async fn run_ripgrep(
        &self,
        pattern: &str,
        targets: &[String],
        include: Option<&str>,
        limit: usize,
        output_mode: GrepOutputMode,
        unique: bool,
        exclude_pattern: Option<&Regex>,
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
            .args(targets)
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
        let mut context_matches = Vec::new();
        let mut match_texts = Vec::new();
        let mut seen_match_texts =
            (output_mode == GrepOutputMode::Matches && unique).then(HashSet::<String>::new);
        let mut files = Vec::new();
        let mut counts = Vec::<GrepFileCount>::new();
        let mut count_indexes = HashMap::<String, usize>::new();
        let mut total_line_matches = 0usize;
        let mut total_match_occurrences = 0usize;

        while let Some(line) = reader
            .next_line()
            .await
            .context("unable to read ripgrep output")?
        {
            let Some(entry) = parse_match_line(&self.workspace_root, &line)? else {
                continue;
            };
            if exclude_pattern
                .map(|pattern| pattern.is_match(&entry.line_text))
                .unwrap_or(false)
            {
                continue;
            }

            total_line_matches += 1;
            let occurrence_count = entry.match_count();
            total_match_occurrences += occurrence_count;

            if let Some(index) = count_indexes.get(&entry.relative_path).copied() {
                counts[index].count += occurrence_count;
            } else {
                let index = counts.len();
                count_indexes.insert(entry.relative_path.clone(), index);
                counts.push(GrepFileCount {
                    relative_path: entry.relative_path.clone(),
                    count: occurrence_count,
                });
                if files.len() < limit {
                    files.push(entry.relative_path.clone());
                }
            }

            if output_mode == GrepOutputMode::Matches {
                for matched_text in &entry.matched_texts {
                    if let Some(seen) = &mut seen_match_texts {
                        if seen.insert(matched_text.clone()) && match_texts.len() < limit {
                            match_texts.push(matched_text.clone());
                        }
                    } else if match_texts.len() < limit {
                        match_texts.push(matched_text.clone());
                    }
                }
            }

            if output_mode == GrepOutputMode::Context && context_matches.len() < limit {
                context_matches.push(entry);
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
            context_matches,
            match_texts,
            files,
            counts,
            total_line_matches,
            total_match_occurrences,
            unique_match_occurrences: seen_match_texts.map(|seen| seen.len()),
        })
    }
}

#[derive(Debug, Deserialize)]
struct GrepInput {
    pattern: String,
    #[serde(default)]
    path: Option<GrepPathInput>,
    #[serde(default)]
    include: Option<String>,
    limit: Option<usize>,
    #[serde(default)]
    output_mode: GrepOutputMode,
    #[serde(default)]
    unique: bool,
    #[serde(default)]
    exclude_pattern: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum GrepPathInput {
    One(String),
    Many(Vec<String>),
}

impl GrepPathInput {
    fn into_vec(self) -> Vec<String> {
        match self {
            GrepPathInput::One(path) => vec![path],
            GrepPathInput::Many(paths) => paths,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum GrepOutputMode {
    #[default]
    Context,
    Matches,
    Files,
    Count,
}

#[derive(Debug)]
struct GrepTargets {
    args: Vec<String>,
}

#[derive(Debug)]
struct GrepSearchResult {
    context_matches: Vec<GrepMatch>,
    match_texts: Vec<String>,
    files: Vec<String>,
    counts: Vec<GrepFileCount>,
    total_line_matches: usize,
    total_match_occurrences: usize,
    unique_match_occurrences: Option<usize>,
}

#[derive(Debug)]
struct GrepFileCount {
    relative_path: String,
    count: usize,
}

#[derive(Debug)]
struct GrepMatch {
    relative_path: String,
    line_number: u64,
    line_text: String,
    matched_texts: Vec<String>,
}

impl GrepMatch {
    fn match_count(&self) -> usize {
        self.matched_texts.len().max(1)
    }
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
    let line_text = raw_line.trim_end_matches(['\r', '\n']).to_string();
    let mut matched_texts = data
        .get("submatches")
        .and_then(Value::as_array)
        .map(|submatches| {
            submatches
                .iter()
                .filter_map(|submatch| {
                    submatch
                        .get("match")
                        .and_then(|value| value.get("text"))
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if matched_texts.is_empty() {
        matched_texts.push(line_text.clone());
    }

    Ok(Some(GrepMatch {
        relative_path: display_match_path(root, raw_path)?,
        line_number,
        line_text,
        matched_texts,
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

fn format_output(limit: usize, output_mode: GrepOutputMode, result: GrepSearchResult) -> String {
    let shown = shown_items(limit, output_mode, &result);
    let total_items = total_items(output_mode, &result);
    let total_matches = total_matches(output_mode, &result);

    let mut output = String::new();
    output.push_str(&format!(
        "matches: {}\nfiles: {}\n",
        total_matches,
        result.counts.len()
    ));
    if shown < total_items {
        output.push_str(&format!("shown: {shown}\n"));
    }

    if total_items == 0 {
        output.push_str("\nNo matches.");
        return output;
    }

    output.push('\n');
    match output_mode {
        GrepOutputMode::Context => {
            let groups = group_matches(result.context_matches);
            for group in groups {
                output.push_str(&format!("{}\n", group.relative_path));
                for item in group.matches {
                    output.push_str(&format!(
                        "  {} | {}\n",
                        item.line_number,
                        clip_line(&item.line_text)
                    ));
                }
                output.push('\n');
            }
        }
        GrepOutputMode::Matches => {
            for matched_text in result.match_texts {
                output.push_str(&clip_line(&matched_text));
                output.push('\n');
            }
        }
        GrepOutputMode::Files => {
            for file in result.files {
                output.push_str(&file);
                output.push('\n');
            }
        }
        GrepOutputMode::Count => {
            for item in result.counts.into_iter().take(limit) {
                output.push_str(&format!("{}: {}\n", item.relative_path, item.count));
            }
        }
    }

    output.trim_end().to_string()
}

fn shown_items(limit: usize, output_mode: GrepOutputMode, result: &GrepSearchResult) -> usize {
    match output_mode {
        GrepOutputMode::Context => result.context_matches.len(),
        GrepOutputMode::Matches => result.match_texts.len(),
        GrepOutputMode::Files => result.files.len(),
        GrepOutputMode::Count => result.counts.len().min(limit),
    }
}

fn total_items(output_mode: GrepOutputMode, result: &GrepSearchResult) -> usize {
    match output_mode {
        GrepOutputMode::Context => result.total_line_matches,
        GrepOutputMode::Matches => result
            .unique_match_occurrences
            .unwrap_or(result.total_match_occurrences),
        GrepOutputMode::Files | GrepOutputMode::Count => result.counts.len(),
    }
}

fn total_matches(output_mode: GrepOutputMode, result: &GrepSearchResult) -> usize {
    match output_mode {
        GrepOutputMode::Context => result.total_line_matches,
        GrepOutputMode::Matches | GrepOutputMode::Files | GrepOutputMode::Count => {
            result.total_match_occurrences
        }
    }
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
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;

    use super::*;

    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

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
    async fn grep_accepts_multiple_paths() {
        if !ripgrep_available() {
            return;
        }

        let root = unique_temp_dir();
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::create_dir_all(root.join("tests")).expect("create tests");
        fs::create_dir_all(root.join("docs")).expect("create docs");
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(root.join("src").join("app.rs"), "needle in src\n").expect("write src");
        fs::write(root.join("tests").join("app.rs"), "needle in tests\n").expect("write tests");
        fs::write(root.join("docs").join("app.rs"), "needle in docs\n").expect("write docs");

        let tool = GrepTool::new(&root);
        let result = tool
            .search(json!({
                "pattern": "needle",
                "path": ["src", "tests"],
                "include": "*.rs",
                "limit": 10
            }))
            .await
            .expect("grep should search multiple paths");

        assert!(result.contains("matches: 2"));
        assert!(result.contains("files: 2"));
        assert!(result.contains("src/app.rs"));
        assert!(result.contains("tests/app.rs"));
        assert!(!result.contains("docs/app.rs"));

        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn grep_deduplicates_multiple_paths() {
        if !ripgrep_available() {
            return;
        }

        let root = unique_temp_dir();
        fs::create_dir_all(root.join("src")).expect("create src");
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(root.join("src").join("app.rs"), "needle\n").expect("write file");

        let tool = GrepTool::new(&root);
        let result = tool
            .search(json!({
                "pattern": "needle",
                "path": ["src", "src"],
                "limit": 10
            }))
            .await
            .expect("grep should deduplicate repeated paths");

        assert!(result.contains("matches: 1"));
        assert!(result.contains("files: 1"));
        assert_eq!(result.matches("src/app.rs").count(), 1);

        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn grep_matches_mode_returns_only_match_texts() {
        if !ripgrep_available() {
            return;
        }

        let root = unique_temp_dir();
        fs::create_dir_all(root.join("src")).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(
            root.join("src").join("app.txt"),
            "id_123 and id_456\nno match\nid_789\n",
        )
        .expect("write file");

        let tool = GrepTool::new(&root);
        let result = tool
            .search(json!({
                "pattern": "id_[0-9]+",
                "output_mode": "matches",
                "limit": 10
            }))
            .await
            .expect("grep should return matched strings");

        assert!(result.contains("matches: 3"));
        assert!(result.contains("files: 1"));
        assert_eq!(payload_lines(&result), vec!["id_123", "id_456", "id_789"]);
        assert!(!payload(&result).contains("src/app.txt"));
        assert!(!payload(&result).contains("no match"));

        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn grep_matches_mode_supports_unique() {
        if !ripgrep_available() {
            return;
        }

        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(root.join("tokens.txt"), "alpha beta alpha\nbeta alpha\n").expect("write file");

        let tool = GrepTool::new(&root);
        let result = tool
            .search(json!({
                "pattern": "alpha|beta",
                "output_mode": "matches",
                "unique": true,
                "limit": 10
            }))
            .await
            .expect("grep should deduplicate matched strings");

        assert!(result.contains("matches: 5"));
        assert!(result.contains("files: 1"));
        assert_eq!(payload_lines(&result), vec!["alpha", "beta"]);

        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn grep_files_mode_returns_only_paths() {
        if !ripgrep_available() {
            return;
        }

        let root = unique_temp_dir();
        fs::create_dir_all(root.join("src")).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(root.join("src").join("one.txt"), "needle\n").expect("write first file");
        fs::write(root.join("src").join("two.txt"), "needle\n").expect("write second file");
        fs::write(root.join("src").join("three.txt"), "haystack\n").expect("write third file");

        let tool = GrepTool::new(&root);
        let result = tool
            .search(json!({
                "pattern": "needle",
                "path": "src",
                "output_mode": "files",
                "limit": 10
            }))
            .await
            .expect("grep should return matching files");

        assert!(result.contains("matches: 2"));
        assert!(result.contains("files: 2"));
        let lines = payload_lines(&result);
        assert_eq!(lines.len(), 2);
        assert!(lines.contains(&"src/one.txt"));
        assert!(lines.contains(&"src/two.txt"));
        assert!(!lines.contains(&"src/three.txt"));

        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn grep_count_mode_counts_matches_per_file() {
        if !ripgrep_available() {
            return;
        }

        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(root.join("one.txt"), "needle needle\n").expect("write first file");
        fs::write(root.join("two.txt"), "needle\nneedle\n").expect("write second file");

        let tool = GrepTool::new(&root);
        let result = tool
            .search(json!({
                "pattern": "needle",
                "output_mode": "count",
                "limit": 10
            }))
            .await
            .expect("grep should count matches by file");

        assert!(result.contains("matches: 4"));
        assert!(result.contains("files: 2"));
        let lines = payload_lines(&result);
        assert_eq!(lines.len(), 2);
        assert!(lines.contains(&"one.txt: 2"));
        assert!(lines.contains(&"two.txt: 2"));

        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn grep_exclude_pattern_filters_matching_lines() {
        if !ripgrep_available() {
            return;
        }

        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(
            root.join("app.txt"),
            "keep needle\nskip needle TODO\nneedle keep\n",
        )
        .expect("write file");

        let tool = GrepTool::new(&root);
        let result = tool
            .search(json!({
                "pattern": "needle",
                "exclude_pattern": "TODO",
                "limit": 10
            }))
            .await
            .expect("grep should exclude matching lines");

        assert!(result.contains("matches: 2"));
        assert!(result.contains("files: 1"));
        assert!(result.contains("keep needle"));
        assert!(result.contains("needle keep"));
        assert!(!result.contains("skip needle TODO"));

        fs::remove_dir_all(root).ok();
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

    fn payload(output: &str) -> &str {
        output
            .split_once("\n\n")
            .map(|(_, value)| value)
            .unwrap_or("")
    }

    fn payload_lines(output: &str) -> Vec<&str> {
        payload(output).lines().collect()
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
        let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "sinew-grep-test-{}-{nanos}-{counter}",
            std::process::id()
        ))
    }
}
