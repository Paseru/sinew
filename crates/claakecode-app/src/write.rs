use std::{
    collections::HashMap,
    fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use claakecode_core::ToolDescriptor;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{
    read::{fingerprint_path, ReadFingerprint},
    tool_run::{diff_snapshots, snapshot_workspace_paths, ToolRunResult},
    workspace::normalize_workspace_relative_path,
};

const MAX_WRITE_FILE_BYTES: usize = 2 * 1024 * 1024;

const WRITE_FILE_DESCRIPTION: &str = r#"Use this tool to write files. Put the path before the content"#;

#[derive(Debug, Clone)]
pub struct WriteFileTool {
    workspace_root: PathBuf,
    write_lock: Option<Arc<Semaphore>>,
}

impl WriteFileTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            write_lock: None,
        }
    }

    pub fn with_workspace_write_lock(mut self, write_lock: Arc<Semaphore>) -> Self {
        self.write_lock = Some(write_lock);
        self
    }

    pub fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "write_file".into(),
            description: WRITE_FILE_DESCRIPTION.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path to write. Relative paths are resolved from the workspace root; absolute paths must be inside the workspace."
                    },
                    "content": {
                        "type": "string",
                        "description": "Full file content to write."
                    }
                },
                "required": ["path", "content"],
                "additionalProperties": false
            }),
        }
    }

    pub async fn run(
        &self,
        input: Value,
        read_fingerprints: &HashMap<String, ReadFingerprint>,
    ) -> ToolRunResult {
        match self.write(input, read_fingerprints).await {
            Ok(output) => output,
            Err(err) => ToolRunResult::err(err.to_string(), Vec::new()),
        }
    }

    async fn write(
        &self,
        input: Value,
        read_fingerprints: &HashMap<String, ReadFingerprint>,
    ) -> Result<ToolRunResult> {
        let parsed: WriteFileInput = serde_json::from_value(input)
            .map_err(|err| anyhow::anyhow!("invalid write_file input: {err}"))?;
        if parsed.path.trim().is_empty() {
            bail!("path is required");
        }
        if parsed.content.len() > MAX_WRITE_FILE_BYTES {
            bail!("content is too large to write safely");
        }

        let target = resolve_workspace_file_target(&self.workspace_root, &parsed.path)?;
        let _write_permit = self.acquire_write_permit().await?;

        let existed = target.absolute_path.exists();
        if existed {
            let metadata = fs::metadata(&target.absolute_path).with_context(|| {
                format!("unable to read file metadata {}", target.relative_path)
            })?;
            if !metadata.is_file() {
                bail!("path is not a file: {}", target.relative_path);
            }
            let expected = read_fingerprints
                .get(&target.relative_path)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "write_file requires a successful read of {} before overwriting it",
                        target.relative_path
                    )
                })?;
            let current = fingerprint_path(&self.workspace_root, &target.absolute_path)?;
            if !fingerprints_match(expected, &current) {
                bail!(
                    "{} changed since the last successful read; run read on this file before write_file",
                    target.relative_path
                );
            }
        }

        let before = snapshot_workspace_paths(&self.workspace_root, [&target.relative_path]);
        write_text_file(&target.absolute_path, &parsed.content)
            .with_context(|| format!("unable to write file {}", target.relative_path))?;
        let after = snapshot_workspace_paths(&self.workspace_root, [&target.relative_path]);
        let file_changes = diff_snapshots(before, after);
        let fingerprint = fingerprint_path(&self.workspace_root, &target.absolute_path)?;

        let content = format!(
            "Wrote {} ({}).",
            target.relative_path,
            if existed {
                "overwrote existing file"
            } else {
                "created new file"
            }
        );
        Ok(ToolRunResult::ok_with_meta(
            content,
            file_changes,
            json!({
                "read_fingerprint": fingerprint.clone(),
                "read_fingerprints": [fingerprint],
            }),
        ))
    }

    async fn acquire_write_permit(&self) -> Result<Option<OwnedSemaphorePermit>> {
        let Some(write_lock) = &self.write_lock else {
            return Ok(None);
        };
        write_lock
            .clone()
            .acquire_owned()
            .await
            .map(Some)
            .map_err(|_| anyhow::anyhow!("workspace write lock is closed"))
    }
}

#[derive(Debug, Deserialize)]
struct WriteFileInput {
    path: String,
    content: String,
}

#[derive(Debug)]
struct ResolvedWriteTarget {
    relative_path: String,
    absolute_path: PathBuf,
}

fn resolve_workspace_file_target(root: &Path, raw: &str) -> Result<ResolvedWriteTarget> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("path cannot be empty");
    }

    let root = root
        .canonicalize()
        .with_context(|| format!("unable to resolve workspace root {}", root.display()))?;
    let candidate = Path::new(trimmed);
    let absolute_path = if candidate.is_absolute() {
        resolve_absolute_target(&root, candidate)?
    } else {
        let normalized = normalize_workspace_relative_path(trimmed)?;
        if normalized.is_empty() {
            bail!("path cannot be empty");
        }
        resolve_relative_target(&root, &normalized)?
    };

    ensure_target_in_workspace(&root, &absolute_path)?;
    let relative_path = relative_path(&root, &absolute_path)?;
    if relative_path.is_empty() {
        bail!("path cannot be the workspace root");
    }
    Ok(ResolvedWriteTarget {
        relative_path,
        absolute_path,
    })
}

fn resolve_absolute_target(root: &Path, path: &Path) -> Result<PathBuf> {
    if path_has_entry(path) {
        return path
            .canonicalize()
            .with_context(|| format!("unable to resolve path {}", path.display()));
    }
    ensure_new_absolute_path_is_under_root(root, path)?;
    Ok(path.to_path_buf())
}

fn resolve_relative_target(root: &Path, normalized: &str) -> Result<PathBuf> {
    let path = root.join(normalized);
    if path_has_entry(&path) {
        return path
            .canonicalize()
            .with_context(|| format!("unable to resolve path {normalized}"));
    }
    Ok(path)
}

fn ensure_target_in_workspace(root: &Path, path: &Path) -> Result<()> {
    if path_has_entry(path) {
        if path.starts_with(root) {
            return Ok(());
        }
        bail!("{} is outside the workspace", path.display());
    }
    if path.starts_with(root) {
        ensure_existing_ancestor_in_workspace(root, path)
    } else {
        bail!("{} is outside the workspace", path.display())
    }
}

fn path_has_entry(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn ensure_existing_ancestor_in_workspace(root: &Path, path: &Path) -> Result<()> {
    let mut ancestor = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("invalid path"))?;
    loop {
        if path_has_entry(ancestor) {
            let canonical = ancestor
                .canonicalize()
                .with_context(|| format!("unable to resolve path {}", ancestor.display()))?;
            if canonical.starts_with(root) {
                return Ok(());
            }
            bail!("{} is outside the workspace", path.display());
        }
        ancestor = ancestor
            .parent()
            .ok_or_else(|| anyhow::anyhow!("unable to resolve path {}", path.display()))?;
    }
}

fn ensure_new_absolute_path_is_under_root(root: &Path, path: &Path) -> Result<()> {
    let relative = path
        .strip_prefix(root)
        .with_context(|| format!("{} is outside the workspace", path.display()))?;
    if relative.as_os_str().is_empty() {
        bail!("path cannot be the workspace root");
    }
    for component in relative.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("{} is outside the workspace", path.display())
            }
        }
    }
    Ok(())
}

fn relative_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .with_context(|| format!("{} is outside the workspace", path.display()))?;
    Ok(relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/"))
}

fn write_text_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("unable to create directory {}", parent.display()))?;
    }
    fs::write(path, content).with_context(|| format!("unable to write file {}", path.display()))
}

fn fingerprints_match(expected: &ReadFingerprint, current: &ReadFingerprint) -> bool {
    expected.size == current.size
        && expected.modified_ms == current.modified_ms
        && expected.sha256 == current.sha256
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, fs, path::Path};

    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    #[tokio::test]
    async fn creates_new_file_without_prior_read() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let tool = WriteFileTool::new(&root);

        let result = tool
            .write(
                json!({ "path": "src/app.rs", "content": "fn main() {}\n" }),
                &HashMap::new(),
            )
            .await
            .expect("write should create file");

        assert!(!result.is_error);
        assert_eq!(
            fs::read_to_string(root.join("src/app.rs")).unwrap(),
            "fn main() {}\n"
        );
        assert!(result.content.contains("created new file"));
        assert_eq!(result.file_changes.len(), 1);
        assert!(result
            .meta
            .as_ref()
            .unwrap()
            .get("read_fingerprint")
            .is_some());
        assert!(result
            .meta
            .as_ref()
            .unwrap()
            .get("read_fingerprints")
            .is_some());
        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn overwrites_existing_file_after_read() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        fs::write(root.join("notes.txt"), "old\n").expect("write file");
        let tool = WriteFileTool::new(&root);
        let fingerprints = fingerprints(&root, &["notes.txt"]);

        let result = tool
            .write(
                json!({ "path": "notes.txt", "content": "new\ncontent\n" }),
                &fingerprints,
            )
            .await
            .expect("write should overwrite file");

        assert!(!result.is_error);
        assert_eq!(
            fs::read_to_string(root.join("notes.txt")).unwrap(),
            "new\ncontent\n"
        );
        assert!(result.content.contains("overwrote existing file"));
        assert_eq!(result.file_changes.len(), 1);
        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn refuses_existing_file_without_read() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        fs::write(root.join("notes.txt"), "old\n").expect("write file");
        let tool = WriteFileTool::new(&root);

        let error = tool
            .write(
                json!({ "path": "notes.txt", "content": "new\n" }),
                &HashMap::new(),
            )
            .await
            .expect_err("existing file should require read");

        assert!(error.to_string().contains("requires a successful read"));
        assert_eq!(fs::read_to_string(root.join("notes.txt")).unwrap(), "old\n");
        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn refuses_stale_existing_file() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        fs::write(root.join("notes.txt"), "old\n").expect("write file");
        let tool = WriteFileTool::new(&root);
        let fingerprints = fingerprints(&root, &["notes.txt"]);
        fs::write(root.join("notes.txt"), "changed\n").expect("modify file");

        let error = tool
            .write(
                json!({ "path": "notes.txt", "content": "new\n" }),
                &fingerprints,
            )
            .await
            .expect_err("stale fingerprint should fail");

        assert!(error
            .to_string()
            .contains("changed since the last successful read"));
        assert_eq!(
            fs::read_to_string(root.join("notes.txt")).unwrap(),
            "changed\n"
        );
        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn accepts_absolute_workspace_path() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        let tool = WriteFileTool::new(&root);
        let path = root.join("abs.txt");

        tool.write(
            json!({ "path": path.display().to_string(), "content": "absolute\n" }),
            &HashMap::new(),
        )
        .await
        .expect("absolute workspace path should write");

        assert_eq!(fs::read_to_string(path).unwrap(), "absolute\n");
        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn rejects_absolute_path_outside_workspace() {
        let base = unique_temp_dir();
        let root = base.join("workspace");
        let outside = base.join("outside.txt");
        fs::create_dir_all(&root).expect("create temp workspace");
        fs::create_dir_all(&base).expect("create base");
        let root = root.canonicalize().expect("canonical temp workspace");
        let tool = WriteFileTool::new(&root);

        let error = tool
            .write(
                json!({ "path": outside.display().to_string(), "content": "nope\n" }),
                &HashMap::new(),
            )
            .await
            .expect_err("outside path should fail");

        assert!(error.to_string().contains("outside the workspace"));
        assert!(!outside.exists());
        fs::remove_dir_all(base).ok();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_relative_path_through_symlink_directory() {
        use std::os::unix::fs as unix_fs;

        let base = unique_temp_dir();
        let root = base.join("workspace");
        let outside = base.join("outside");
        fs::create_dir_all(&root).expect("create temp workspace");
        fs::create_dir_all(&outside).expect("create outside dir");
        unix_fs::symlink(&outside, root.join("link")).expect("create symlink");
        let root = root.canonicalize().expect("canonical temp workspace");
        let tool = WriteFileTool::new(&root);

        let error = tool
            .write(
                json!({ "path": "link/escape.txt", "content": "nope\n" }),
                &HashMap::new(),
            )
            .await
            .expect_err("symlink escape should fail");

        assert!(error.to_string().contains("outside the workspace"));
        assert!(!outside.join("escape.txt").exists());
        fs::remove_dir_all(base).ok();
    }

    fn fingerprints(root: &Path, paths: &[&str]) -> HashMap<String, ReadFingerprint> {
        paths
            .iter()
            .map(|path| {
                let fingerprint =
                    fingerprint_path(root, &root.join(path)).expect("fingerprint file");
                ((*path).to_string(), fingerprint)
            })
            .collect()
    }

    fn unique_temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("claakecode-write-test-{}", Uuid::new_v4()))
    }
}
