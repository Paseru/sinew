use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::Path,
    time::UNIX_EPOCH,
};

use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use tracing::warn;
use walkdir::{DirEntry, WalkDir};

use crate::text::decode_text;
use crate::workspace::normalize_workspace_relative_path;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const SNAPSHOT_FILE_LIMIT: u64 = 256 * 1024;
const SNAPSHOT_TOTAL_LIMIT: usize = 4 * 1024 * 1024;
const DIFF_CONTEXT_LINES: usize = 3;
const DIFF_LINE_LIMIT: usize = 400;
const CHECKPOINT_FILE_LIMIT: u64 = 2 * 1024 * 1024;
const CHECKPOINT_TOTAL_LIMIT: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeKind {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub relative_path: String,
    pub kind: FileChangeKind,
    pub summary: String,
    pub binary: bool,
    pub added_lines: usize,
    pub removed_lines: usize,
    pub truncated: bool,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRunImage {
    pub media_type: String,
    pub data: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ToolRunResult {
    pub content: String,
    pub is_error: bool,
    pub file_changes: Vec<FileChange>,
    pub images: Vec<ToolRunImage>,
    pub meta: Option<serde_json::Value>,
}

impl ToolRunResult {
    pub fn ok(content: impl Into<String>, file_changes: Vec<FileChange>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
            file_changes,
            images: Vec::new(),
            meta: None,
        }
    }

    pub fn ok_with_images(
        content: impl Into<String>,
        images: Vec<ToolRunImage>,
        file_changes: Vec<FileChange>,
    ) -> Self {
        Self {
            content: content.into(),
            is_error: false,
            file_changes,
            images,
            meta: None,
        }
    }

    pub fn ok_with_meta(
        content: impl Into<String>,
        file_changes: Vec<FileChange>,
        meta: serde_json::Value,
    ) -> Self {
        Self {
            content: content.into(),
            is_error: false,
            file_changes,
            images: Vec::new(),
            meta: Some(meta),
        }
    }

    pub fn err(content: impl Into<String>, file_changes: Vec<FileChange>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
            file_changes,
            images: Vec::new(),
            meta: None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceSnapshot {
    entries: HashMap<String, SnapshotEntry>,
}

#[derive(Debug, Clone)]
pub struct TurnSnapshot {
    entries: HashMap<String, TurnSnapshotEntry>,
}

#[derive(Debug, Clone)]
struct SnapshotEntry {
    size: u64,
    modified_ms: i64,
    text: Option<String>,
    binary: bool,
}

#[derive(Debug, Clone)]
struct TurnSnapshotEntry {
    size: u64,
    modified_ms: i64,
    bytes: Option<Vec<u8>>,
    mode: Option<u32>,
    unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnCheckpoint {
    pub files: Vec<TurnFileCheckpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnFileCheckpoint {
    pub relative_path: String,
    pub before: TurnFileState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<TurnFileState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnFileState {
    pub exists: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
}

pub(crate) fn snapshot_workspace(root: &Path) -> WorkspaceSnapshot {
    let mut entries = HashMap::new();
    let mut consumed_bytes = 0usize;

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| should_visit(entry, root))
    {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                warn!("skipping workspace entry while diffing: {err}");
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };

        let relative_path = match path.strip_prefix(root) {
            Ok(relative) => relative
                .components()
                .filter_map(|component| {
                    component
                        .as_os_str()
                        .to_str()
                        .map(|value| value.to_string())
                })
                .collect::<Vec<_>>()
                .join("/"),
            Err(_) => continue,
        };

        let modified_ms = metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_millis() as i64)
            .unwrap_or_default();

        let mut snapshot_entry = SnapshotEntry {
            size: metadata.len(),
            modified_ms,
            text: None,
            binary: false,
        };

        if metadata.len() <= SNAPSHOT_FILE_LIMIT && consumed_bytes < SNAPSHOT_TOTAL_LIMIT {
            match fs::read(path) {
                Ok(bytes) => {
                    if let Some(decoded) = decode_text(&bytes) {
                        consumed_bytes += bytes.len();
                        snapshot_entry.text = Some(decoded.content);
                    } else {
                        snapshot_entry.binary = true;
                    }
                }
                Err(_) => continue,
            }
        }

        entries.insert(relative_path, snapshot_entry);
    }

    WorkspaceSnapshot { entries }
}

pub(crate) fn snapshot_workspace_paths<I, P>(root: &Path, relative_paths: I) -> WorkspaceSnapshot
where
    I: IntoIterator<Item = P>,
    P: AsRef<str>,
{
    let mut entries = HashMap::new();

    for relative_path in relative_paths {
        let Ok(normalized) = normalize_workspace_relative_path(relative_path.as_ref()) else {
            continue;
        };
        if normalized.is_empty() {
            continue;
        }
        if let Some(entry) = snapshot_entry_for_path(root, &normalized) {
            entries.insert(normalized, entry);
        }
    }

    WorkspaceSnapshot { entries }
}

fn snapshot_entry_for_path(root: &Path, relative_path: &str) -> Option<SnapshotEntry> {
    let path = root.join(relative_path);
    let metadata = fs::metadata(&path).ok()?;
    if !metadata.is_file() {
        return None;
    }

    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_millis() as i64)
        .unwrap_or_default();

    let mut snapshot_entry = SnapshotEntry {
        size: metadata.len(),
        modified_ms,
        text: None,
        binary: false,
    };

    if metadata.len() <= SNAPSHOT_FILE_LIMIT {
        if let Ok(bytes) = fs::read(path) {
            if let Some(decoded) = decode_text(&bytes) {
                snapshot_entry.text = Some(decoded.content);
            } else {
                snapshot_entry.binary = true;
            }
        }
    }

    Some(snapshot_entry)
}

pub fn snapshot_workspace_for_checkpoint(root: &Path) -> TurnSnapshot {
    let mut entries = HashMap::new();
    let mut consumed_bytes = 0usize;

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| should_visit(entry, root))
    {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                warn!("skipping workspace entry while checkpointing: {err}");
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };

        let relative_path = match path.strip_prefix(root) {
            Ok(relative) => relative
                .components()
                .filter_map(|component| {
                    component
                        .as_os_str()
                        .to_str()
                        .map(|value| value.to_string())
                })
                .collect::<Vec<_>>()
                .join("/"),
            Err(_) => continue,
        };

        let modified_ms = metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_millis() as i64)
            .unwrap_or_default();

        let mode = file_mode(&metadata);
        let mut snapshot_entry = TurnSnapshotEntry {
            size: metadata.len(),
            modified_ms,
            bytes: None,
            mode,
            unavailable_reason: None,
        };

        if metadata.len() > CHECKPOINT_FILE_LIMIT {
            snapshot_entry.unavailable_reason = Some(format!(
                "file is larger than {} bytes",
                CHECKPOINT_FILE_LIMIT
            ));
        } else if consumed_bytes >= CHECKPOINT_TOTAL_LIMIT
            || consumed_bytes.saturating_add(metadata.len() as usize) > CHECKPOINT_TOTAL_LIMIT
        {
            snapshot_entry.unavailable_reason = Some("checkpoint size limit reached".into());
        } else {
            match fs::read(path) {
                Ok(bytes) => {
                    consumed_bytes += bytes.len();
                    snapshot_entry.bytes = Some(bytes);
                }
                Err(err) => {
                    snapshot_entry.unavailable_reason = Some(err.to_string());
                }
            }
        }

        entries.insert(relative_path, snapshot_entry);
    }

    TurnSnapshot { entries }
}

pub(crate) fn diff_snapshots(
    before: WorkspaceSnapshot,
    after: WorkspaceSnapshot,
) -> Vec<FileChange> {
    let mut paths = BTreeSet::new();
    paths.extend(before.entries.keys().cloned());
    paths.extend(after.entries.keys().cloned());

    let mut changes = Vec::new();
    for path in paths {
        match (before.entries.get(&path), after.entries.get(&path)) {
            (None, Some(after_entry)) => changes.push(build_change(&path, None, Some(after_entry))),
            (Some(before_entry), None) => {
                changes.push(build_change(&path, Some(before_entry), None))
            }
            (Some(before_entry), Some(after_entry)) => {
                let content_unchanged = before_entry.text == after_entry.text;
                let metadata_unchanged = before_entry.size == after_entry.size
                    && before_entry.modified_ms == after_entry.modified_ms
                    && before_entry.binary == after_entry.binary;
                if content_unchanged && metadata_unchanged {
                    continue;
                }
                changes.push(build_change(&path, Some(before_entry), Some(after_entry)));
            }
            (None, None) => {}
        }
    }

    changes
}

pub fn checkpoint_from_snapshots(before: &TurnSnapshot, after: &TurnSnapshot) -> TurnCheckpoint {
    let mut paths = BTreeSet::new();
    paths.extend(before.entries.keys().cloned());
    paths.extend(after.entries.keys().cloned());

    let mut files = Vec::new();
    for path in paths {
        let before_entry = before.entries.get(&path);
        let after_entry = after.entries.get(&path);
        if !turn_entry_changed(before_entry, after_entry) {
            continue;
        }
        files.push(TurnFileCheckpoint {
            relative_path: path,
            before: turn_file_state(before_entry),
            after: Some(turn_file_state(after_entry)),
        });
    }

    TurnCheckpoint { files }
}

pub fn restore_turn_checkpoints(
    root: &Path,
    checkpoints: &[TurnCheckpoint],
) -> Result<Vec<String>> {
    let mut states = HashMap::<String, (TurnFileState, Option<TurnFileState>)>::new();
    let mut ordered_paths = Vec::<String>::new();
    let mut expected_after_by_path = BTreeMap::<String, TurnFileState>::new();

    for checkpoint in checkpoints {
        for file in &checkpoint.files {
            let normalized = normalize_workspace_relative_path(&file.relative_path)?;
            if states.contains_key(&normalized) {
                continue;
            }
            ordered_paths.push(normalized.clone());
            states.insert(normalized, (file.before.clone(), file.after.clone()));
        }
    }

    for checkpoint in checkpoints {
        for file in &checkpoint.files {
            if let Some(after) = &file.after {
                let normalized = normalize_workspace_relative_path(&file.relative_path)?;
                expected_after_by_path.insert(normalized, after.clone());
            }
        }
    }
    ordered_paths.sort_by(|left, right| {
        let left_exists = states
            .get(left)
            .map(|(state, _)| state.exists)
            .unwrap_or(true);
        let right_exists = states
            .get(right)
            .map(|(state, _)| state.exists)
            .unwrap_or(true);
        match (left_exists, right_exists) {
            (false, true) => std::cmp::Ordering::Less,
            (true, false) => std::cmp::Ordering::Greater,
            (false, false) => path_depth(right)
                .cmp(&path_depth(left))
                .then_with(|| left.cmp(right)),
            (true, true) => path_depth(left)
                .cmp(&path_depth(right))
                .then_with(|| left.cmp(right)),
        }
    });

    let mut restored = Vec::new();
    for relative_path in &ordered_paths {
        let Some((before, _)) = states.get(relative_path) else {
            continue;
        };
        let Some(expected) = expected_after_by_path.get(relative_path) else {
            continue;
        };
        ensure_current_file_state(
            root,
            relative_path,
            before,
            expected,
            &expected_after_by_path,
        )?;
    }

    for relative_path in ordered_paths {
        let (state, _) = states
            .get(&relative_path)
            .ok_or_else(|| anyhow!("missing restore state"))?;
        restore_file_state(root, &relative_path, state)?;
        restored.push(relative_path);
    }

    Ok(restored)
}

fn path_depth(path: &str) -> usize {
    path.split('/').count()
}

fn path_to_slash_string(path: &Path) -> String {
    path.components()
        .filter_map(|component| {
            component
                .as_os_str()
                .to_str()
                .map(|value| value.to_string())
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn ensure_current_file_state(
    root: &Path,
    relative_path: &str,
    before: &TurnFileState,
    expected: &TurnFileState,
    expected_after_by_path: &BTreeMap<String, TurnFileState>,
) -> Result<()> {
    let current = current_file_state(root, relative_path)?;
    if file_state_matches(&current, expected)
        || added_file_already_absent(before, expected, &current)
        || expected_tree_absent(root, relative_path, expected, expected_after_by_path)?
        || expected_checkpoint_tree_present(
            root,
            relative_path,
            expected,
            &current,
            expected_after_by_path,
        )?
    {
        return Ok(());
    }

    bail!("unable to restore {relative_path}: file changed since checkpoint was created");
}

fn added_file_already_absent(
    before: &TurnFileState,
    expected: &TurnFileState,
    current: &TurnFileState,
) -> bool {
    !before.exists && expected.exists && !current.exists
}

fn expected_tree_absent(
    root: &Path,
    relative_path: &str,
    expected: &TurnFileState,
    expected_after_by_path: &BTreeMap<String, TurnFileState>,
) -> Result<bool> {
    if expected.exists {
        return Ok(false);
    }
    let prefix = format!("{relative_path}/");
    if !expected_after_by_path
        .iter()
        .any(|(path, state)| path.starts_with(&prefix) && state.exists)
    {
        return Ok(false);
    }
    let target = restore_target_path(root, relative_path)?;
    Ok(!target.exists())
}

fn expected_checkpoint_tree_present(
    root: &Path,
    relative_path: &str,
    expected: &TurnFileState,
    current: &TurnFileState,
    expected_after_by_path: &BTreeMap<String, TurnFileState>,
) -> Result<bool> {
    if expected.exists
        || !current.exists
        || current.content_base64.is_some()
        || current.unavailable_reason.as_deref() != Some("current path is not a regular file")
    {
        return Ok(false);
    }
    let prefix = format!("{relative_path}/");
    let expected_files = expected_after_by_path
        .iter()
        .filter_map(|(path, state)| (path.starts_with(&prefix) && state.exists).then_some(path))
        .cloned()
        .collect::<BTreeSet<_>>();
    if expected_files.is_empty() {
        return Ok(false);
    }

    let target = restore_target_path(root, relative_path)?;
    let Ok(metadata) = fs::symlink_metadata(&target) else {
        return Ok(false);
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Ok(false);
    }

    let mut current_files = BTreeSet::new();
    for entry in WalkDir::new(&target).follow_links(false) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                warn!("skipping restore validation entry: {err}");
                return Ok(false);
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(relative) = entry.path().strip_prefix(root) else {
            return Ok(false);
        };
        current_files.insert(path_to_slash_string(relative));
    }

    Ok(current_files == expected_files)
}

fn current_file_state(root: &Path, relative_path: &str) -> Result<TurnFileState> {
    let target = restore_target_path(root, relative_path)?;
    let Ok(metadata) = fs::symlink_metadata(&target) else {
        return Ok(TurnFileState {
            exists: false,
            content_base64: None,
            mode: None,
            unavailable_reason: None,
        });
    };

    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Ok(TurnFileState {
            exists: true,
            content_base64: None,
            mode: file_mode(&metadata),
            unavailable_reason: Some("current path is not a regular file".into()),
        });
    }

    if metadata.len() > CHECKPOINT_FILE_LIMIT {
        return Ok(TurnFileState {
            exists: true,
            content_base64: None,
            mode: file_mode(&metadata),
            unavailable_reason: Some(format!(
                "file is larger than {} bytes",
                CHECKPOINT_FILE_LIMIT
            )),
        });
    }

    let bytes = fs::read(&target)
        .with_context(|| format!("unable to read current state for {relative_path}"))?;
    Ok(TurnFileState {
        exists: true,
        content_base64: Some(BASE64_STANDARD.encode(bytes)),
        mode: file_mode(&metadata),
        unavailable_reason: None,
    })
}

fn file_state_matches(current: &TurnFileState, expected: &TurnFileState) -> bool {
    if current.exists != expected.exists {
        return false;
    }
    if !current.exists {
        return true;
    }
    if current.mode != expected.mode {
        return false;
    }
    match (&current.content_base64, &expected.content_base64) {
        (Some(current), Some(expected)) => current == expected,
        _ => false,
    }
}

fn should_visit(entry: &DirEntry, root: &Path) -> bool {
    if entry.path() == root {
        return true;
    }

    let Some(name) = entry.file_name().to_str() else {
        return false;
    };

    if !entry.file_type().is_dir() {
        return true;
    }

    !matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | "build" | ".next" | "coverage"
    )
}

fn build_change(
    relative_path: &str,
    before: Option<&SnapshotEntry>,
    after: Option<&SnapshotEntry>,
) -> FileChange {
    let kind = match (before, after) {
        (None, Some(_)) => FileChangeKind::Added,
        (Some(_), None) => FileChangeKind::Deleted,
        _ => FileChangeKind::Modified,
    };

    let summary = match kind {
        FileChangeKind::Added => format!("Added {relative_path}"),
        FileChangeKind::Deleted => format!("Deleted {relative_path}"),
        FileChangeKind::Modified => format!("Updated {relative_path}"),
    };

    let binary = before.map(|entry| entry.binary).unwrap_or(false)
        || after.map(|entry| entry.binary).unwrap_or(false)
        || before.and_then(|entry| entry.text.as_ref()).is_none()
            && before.is_some()
            && before
                .map(|entry| entry.size > SNAPSHOT_FILE_LIMIT)
                .unwrap_or(false)
        || after.and_then(|entry| entry.text.as_ref()).is_none()
            && after.is_some()
            && after
                .map(|entry| entry.size > SNAPSHOT_FILE_LIMIT)
                .unwrap_or(false);

    let mut truncated = false;
    let mut added_lines = 0;
    let mut removed_lines = 0;
    let lines = match (
        before.and_then(|entry| entry.text.as_deref()),
        after.and_then(|entry| entry.text.as_deref()),
    ) {
        (Some(old_text), Some(new_text)) => diff_lines(
            old_text,
            new_text,
            &mut truncated,
            &mut added_lines,
            &mut removed_lines,
        ),
        (None, Some(new_text)) => diff_lines(
            "",
            new_text,
            &mut truncated,
            &mut added_lines,
            &mut removed_lines,
        ),
        (Some(old_text), None) => diff_lines(
            old_text,
            "",
            &mut truncated,
            &mut added_lines,
            &mut removed_lines,
        ),
        (None, None) => Vec::new(),
    };

    FileChange {
        relative_path: relative_path.to_string(),
        kind,
        summary,
        binary,
        added_lines,
        removed_lines,
        truncated,
        lines,
    }
}

fn diff_lines(
    old_text: &str,
    new_text: &str,
    truncated: &mut bool,
    added_lines: &mut usize,
    removed_lines: &mut usize,
) -> Vec<DiffLine> {
    let diff = TextDiff::from_lines(old_text, new_text);
    let mut lines = Vec::new();

    for change in diff.iter_all_changes() {
        let kind = match change.tag() {
            ChangeTag::Delete => DiffLineKind::Removed,
            ChangeTag::Insert => DiffLineKind::Added,
            ChangeTag::Equal => DiffLineKind::Context,
        };

        match kind {
            DiffLineKind::Added => *added_lines += 1,
            DiffLineKind::Removed => *removed_lines += 1,
            DiffLineKind::Context => {}
        }
    }

    for (group_index, group) in diff.grouped_ops(DIFF_CONTEXT_LINES).iter().enumerate() {
        if group_index > 0 {
            push_diff_line(&mut lines, truncated, DiffLineKind::Context, "...\n".into());
        }

        for op in group {
            for change in diff.iter_changes(op) {
                let kind = match change.tag() {
                    ChangeTag::Delete => DiffLineKind::Removed,
                    ChangeTag::Insert => DiffLineKind::Added,
                    ChangeTag::Equal => DiffLineKind::Context,
                };

                push_diff_line(&mut lines, truncated, kind, change.to_string());
            }
        }
    }

    lines
}

fn push_diff_line(
    lines: &mut Vec<DiffLine>,
    truncated: &mut bool,
    kind: DiffLineKind,
    text: String,
) {
    if lines.len() >= DIFF_LINE_LIMIT {
        *truncated = true;
        return;
    }

    lines.push(DiffLine { kind, text });
}

fn turn_entry_changed(
    before: Option<&TurnSnapshotEntry>,
    after: Option<&TurnSnapshotEntry>,
) -> bool {
    match (before, after) {
        (None, None) => false,
        (None, Some(_)) | (Some(_), None) => true,
        (Some(before), Some(after)) => {
            if before.mode != after.mode {
                return true;
            }
            match (&before.bytes, &after.bytes) {
                (Some(before_bytes), Some(after_bytes)) => before_bytes != after_bytes,
                _ => before.size != after.size || before.modified_ms != after.modified_ms,
            }
        }
    }
}

fn turn_file_state(entry: Option<&TurnSnapshotEntry>) -> TurnFileState {
    match entry {
        None => TurnFileState {
            exists: false,
            content_base64: None,
            mode: None,
            unavailable_reason: None,
        },
        Some(entry) => TurnFileState {
            exists: true,
            content_base64: entry
                .bytes
                .as_ref()
                .map(|bytes| BASE64_STANDARD.encode(bytes)),
            mode: entry.mode,
            unavailable_reason: entry.unavailable_reason.clone(),
        },
    }
}

fn restore_file_state(root: &Path, relative_path: &str, state: &TurnFileState) -> Result<()> {
    let target = restore_target_path(root, relative_path)?;

    if !state.exists {
        if let Ok(metadata) = fs::symlink_metadata(&target) {
            if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
                fs::remove_dir_all(&target)
                    .with_context(|| format!("unable to remove {}", target.display()))?;
            } else {
                fs::remove_file(&target)
                    .with_context(|| format!("unable to remove {}", target.display()))?;
            }
        }
        return Ok(());
    }

    let Some(content_base64) = &state.content_base64 else {
        let reason = state
            .unavailable_reason
            .as_deref()
            .unwrap_or("checkpoint content is unavailable");
        bail!("unable to restore {relative_path}: {reason}");
    };
    let bytes = BASE64_STANDARD
        .decode(content_base64)
        .with_context(|| format!("invalid checkpoint data for {relative_path}"))?;

    if let Ok(metadata) = fs::symlink_metadata(&target) {
        if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
            fs::remove_dir_all(&target)
                .with_context(|| format!("unable to replace {}", target.display()))?;
        } else if metadata.file_type().is_symlink() {
            fs::remove_file(&target)
                .with_context(|| format!("unable to replace {}", target.display()))?;
        }
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("unable to create {}", parent.display()))?;
    }
    fs::write(&target, bytes).with_context(|| format!("unable to restore {}", target.display()))?;
    set_file_mode(&target, state.mode)?;
    Ok(())
}

fn restore_target_path(root: &Path, relative_path: &str) -> Result<std::path::PathBuf> {
    let normalized = normalize_workspace_relative_path(relative_path)?;
    let target = root.join(&normalized);
    let mut ancestor = target
        .parent()
        .ok_or_else(|| anyhow!("invalid restore path"))?
        .to_path_buf();
    while !ancestor.exists() {
        let Some(parent) = ancestor.parent() else {
            bail!("path escapes workspace");
        };
        ancestor = parent.to_path_buf();
    }
    let canonical_ancestor = ancestor
        .canonicalize()
        .with_context(|| format!("unable to resolve {}", ancestor.display()))?;
    if !canonical_ancestor.starts_with(root) {
        bail!("path escapes workspace");
    }
    Ok(target)
}

fn file_mode(metadata: &fs::Metadata) -> Option<u32> {
    #[cfg(unix)]
    {
        Some(metadata.permissions().mode())
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        None
    }
}

fn set_file_mode(path: &Path, mode: Option<u32>) -> Result<()> {
    #[cfg(unix)]
    {
        if let Some(mode) = mode {
            fs::set_permissions(path, fs::Permissions::from_mode(mode))
                .with_context(|| format!("unable to restore permissions for {}", path.display()))?;
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn diff_lines_focuses_on_changes_after_long_unchanged_prefix() {
        let before = (0..700)
            .map(|index| format!("line {index}\n"))
            .collect::<String>();
        let mut after_lines = (0..700)
            .map(|index| format!("line {index}\n"))
            .collect::<Vec<_>>();
        after_lines[650] = "changed line\n".to_string();
        let after = after_lines.concat();
        let mut truncated = false;
        let mut added_lines = 0;
        let mut removed_lines = 0;

        let lines = diff_lines(
            &before,
            &after,
            &mut truncated,
            &mut added_lines,
            &mut removed_lines,
        );

        assert_eq!(added_lines, 1);
        assert_eq!(removed_lines, 1);
        assert!(lines.iter().any(|line| {
            matches!(line.kind, DiffLineKind::Removed) && line.text == "line 650\n"
        }));
        assert!(lines.iter().any(|line| {
            matches!(line.kind, DiffLineKind::Added) && line.text == "changed line\n"
        }));
        assert!(lines.len() < 20);
        assert!(!truncated);
    }

    #[test]
    fn restore_checkpoint_removes_added_files_and_restores_original_content() {
        let root = test_root();
        fs::write(root.join("note.txt"), "one\n").expect("write original file");
        let before = snapshot_workspace_for_checkpoint(&root);

        fs::write(root.join("note.txt"), "two\n").expect("modify file");
        fs::write(root.join("new.txt"), "created\n").expect("create file");
        let after = snapshot_workspace_for_checkpoint(&root);
        let checkpoint = checkpoint_from_snapshots(&before, &after);

        let restored = restore_turn_checkpoints(&root, &[checkpoint]).expect("restore checkpoint");

        assert!(restored.iter().any(|path| path == "note.txt"));
        assert!(restored.iter().any(|path| path == "new.txt"));
        assert_eq!(
            fs::read_to_string(root.join("note.txt")).expect("read restored file"),
            "one\n"
        );
        assert!(!root.join("new.txt").exists());
        fs::remove_dir_all(root).expect("remove temp workspace");
    }

    #[test]
    fn restore_checkpoint_handles_file_directory_conflicts() {
        let root = test_root();
        fs::write(root.join("thing"), "file\n").expect("write original file");
        let before = snapshot_workspace_for_checkpoint(&root);

        fs::remove_file(root.join("thing")).expect("remove original file");
        fs::create_dir(root.join("thing")).expect("create replacement directory");
        fs::write(root.join("thing").join("child.txt"), "child\n").expect("write child");
        let after = snapshot_workspace_for_checkpoint(&root);
        let checkpoint = checkpoint_from_snapshots(&before, &after);

        restore_turn_checkpoints(&root, &[checkpoint]).expect("restore checkpoint");

        assert!(root.join("thing").is_file());
        assert_eq!(
            fs::read_to_string(root.join("thing")).expect("read restored file"),
            "file\n"
        );
        fs::remove_dir_all(root).expect("remove temp workspace");
    }

    #[test]
    fn restore_checkpoint_ignores_missing_added_file_parent() {
        let root = test_root();
        let before = snapshot_workspace_for_checkpoint(&root);

        fs::create_dir(root.join("nested")).expect("create nested dir");
        fs::write(root.join("nested").join("new.txt"), "created\n").expect("write file");
        let after = snapshot_workspace_for_checkpoint(&root);
        let checkpoint = checkpoint_from_snapshots(&before, &after);
        fs::remove_dir_all(root.join("nested")).expect("remove added tree");

        restore_turn_checkpoints(&root, &[checkpoint]).expect("restore checkpoint");

        assert!(!root.join("nested").exists());
        fs::remove_dir_all(root).expect("remove temp workspace");
    }

    #[test]
    fn restore_checkpoint_refuses_changed_modified_file() {
        let root = test_root();
        fs::write(root.join("note.txt"), "one\n").expect("write original file");
        let before = snapshot_workspace_for_checkpoint(&root);

        fs::write(root.join("note.txt"), "two\n").expect("modify file");
        let after = snapshot_workspace_for_checkpoint(&root);
        let checkpoint = checkpoint_from_snapshots(&before, &after);

        fs::write(root.join("note.txt"), "three\n").expect("external edit");
        let err = restore_turn_checkpoints(&root, &[checkpoint]).expect_err("restore should fail");

        assert!(err.to_string().contains("file changed since checkpoint"));
        assert_eq!(
            fs::read_to_string(root.join("note.txt")).expect("read current file"),
            "three\n"
        );
        fs::remove_dir_all(root).expect("remove temp workspace");
    }

    #[test]
    fn restore_checkpoint_refuses_recreated_added_file() {
        let root = test_root();
        let before = snapshot_workspace_for_checkpoint(&root);

        fs::write(root.join("new.txt"), "created\n").expect("create file");
        let after = snapshot_workspace_for_checkpoint(&root);
        let checkpoint = checkpoint_from_snapshots(&before, &after);

        fs::write(root.join("new.txt"), "other work\n").expect("external edit");
        let err = restore_turn_checkpoints(&root, &[checkpoint]).expect_err("restore should fail");

        assert!(err.to_string().contains("file changed since checkpoint"));
        assert_eq!(
            fs::read_to_string(root.join("new.txt")).expect("read current file"),
            "other work\n"
        );
        fs::remove_dir_all(root).expect("remove temp workspace");
    }

    fn test_root() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("claakecode-checkpoint-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create temp workspace");
        root.canonicalize().expect("canonical temp workspace")
    }
}
