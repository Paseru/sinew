use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use sinew_core::ToolDescriptor;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{
    tool_run::{diff_snapshots, snapshot_workspace_paths, FileChange, ToolRunResult},
    workspace::normalize_workspace_relative_path,
};

const MAX_PATCH_BYTES: usize = 2 * 1024 * 1024;
const BEGIN_PATCH_MARKER: &str = "*** Begin Patch";
const END_PATCH_MARKER: &str = "*** End Patch";
const ADD_FILE_MARKER: &str = "*** Add File: ";
const DELETE_FILE_MARKER: &str = "*** Delete File: ";
const UPDATE_FILE_MARKER: &str = "*** Update File: ";
const MOVE_TO_MARKER: &str = "*** Move to: ";
const EOF_MARKER: &str = "*** End of File";

const APPLY_PATCH_DESCRIPTION: &str = r#"MANDATORY tool for ALL file creation, modification, deletion, and renaming. This is the ONLY approved way to write to the filesystem.

Use this exact patch format:

*** Begin Patch
*** Add File: path/to/new_file
+new line
+another line

*** Update File: path/to/existing_file
@@ optional code anchor (e.g. `def greet():` or `class Foo:`)
 context line above
-old line
+new line
 context line below

*** Delete File: path/to/old_file
*** End Patch

A single patch can combine several operations atomically:

*** Begin Patch
*** Add File: hello.txt
+Hello world
*** Update File: src/app.py
*** Move to: src/main.py
@@ def greet():
-print("Hi")
+print("Hello, world!")
*** Delete File: obsolete.txt
*** End Patch

Rules:
- Always start with `*** Begin Patch` and end with `*** End Patch`.
- The `@@` header takes a code-level anchor (function/class declaration line), not a unified-diff line range like `-N,M +N,M`.
- For Update File hunks, include ~3 lines of unchanged context above and below your -/+ changes so the patch can be anchored unambiguously.
- New file content lines must start with `+`.
- Update hunk lines start with ' ' (context), '-' (removal), or '+' (addition).
- Paths must be relative, never absolute.
- Operations apply sequentially. If one fails, earlier ops stay on disk; the error lists applied/failed/not-attempted. Re-send only failed and not-attempted operations.
"#;

#[derive(Debug, Clone)]
pub struct ApplyPatchTool {
    workspace_root: PathBuf,
    write_lock: Option<Arc<Semaphore>>,
}

impl ApplyPatchTool {
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
            name: "apply_patch".into(),
            description: APPLY_PATCH_DESCRIPTION.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "patch": {
                        "type": "string",
                        "description": "The entire Codex apply_patch payload, starting with *** Begin Patch and ending with *** End Patch."
                    }
                },
                "required": ["patch"],
                "additionalProperties": false
            }),
        }
    }

    pub async fn run(&self, input: Value) -> ToolRunResult {
        self.run_with_read_paths(input).await
    }

    pub async fn run_with_read_paths(&self, input: Value) -> ToolRunResult {
        match self.apply(input).await {
            Ok(output) => output,
            Err(ApplyPatchError::Fatal(err)) => ToolRunResult::err(err.to_string(), Vec::new()),
            Err(ApplyPatchError::Partial {
                message,
                file_changes,
            }) => ToolRunResult::err(message, file_changes),
        }
    }

    async fn apply(&self, input: Value) -> std::result::Result<ToolRunResult, ApplyPatchError> {
        let parsed: ApplyPatchInput = serde_json::from_value(input)
            .map_err(|err| anyhow::anyhow!("invalid apply_patch input: {err}"))?;

        if parsed.patch.trim().is_empty() {
            return Err(anyhow::anyhow!("patch is required").into());
        }
        if parsed.patch.len() > MAX_PATCH_BYTES {
            return Err(anyhow::anyhow!("patch is too large to apply safely").into());
        }

        let operations = parse_patch(&parsed.patch)?;

        let affected_paths = affected_patch_paths(&operations);
        let _write_permit = self.acquire_write_permit().await?;
        let before = snapshot_workspace_paths(&self.workspace_root, &affected_paths);
        let summary = match self.apply_operations(&operations) {
            Ok(summary) => summary,
            Err(partial) => {
                let after = snapshot_workspace_paths(&self.workspace_root, &affected_paths);
                let file_changes = diff_snapshots(before, after);
                return Err(ApplyPatchError::Partial {
                    message: partial.format_message(operations.len()),
                    file_changes,
                });
            }
        };
        let after = snapshot_workspace_paths(&self.workspace_root, &affected_paths);
        let file_changes = diff_snapshots(before, after);

        let mut content = if file_changes.is_empty() {
            "Patch applied; no file changes detected.".to_string()
        } else {
            format!(
                "Patch applied. {} file{} changed.",
                file_changes.len(),
                if file_changes.len() == 1 { "" } else { "s" }
            )
        };
        if !summary.trim().is_empty() {
            content.push_str("\n\n");
            content.push_str(summary.trim());
        }

        Ok(ToolRunResult::ok(content, file_changes))
    }

    fn apply_operations(&self, operations: &[PatchOperation]) -> Result<String, PartialPatchError> {
        let mut added = Vec::new();
        let mut modified = Vec::new();
        let mut deleted = Vec::new();

        for (index, operation) in operations.iter().enumerate() {
            let applied = AppliedPatchOperations {
                added: added.clone(),
                modified: modified.clone(),
                deleted: deleted.clone(),
            };
            match self.apply_operation(operation) {
                Ok(kind) => match kind {
                    AppliedPatchKind::Added => added.push(operation.path().to_string()),
                    AppliedPatchKind::Modified => modified.push(operation.path().to_string()),
                    AppliedPatchKind::Deleted => deleted.push(operation.path().to_string()),
                },
                Err(err) => {
                    return Err(PartialPatchError {
                        failed_index: index,
                        failed_operation: operation.clone(),
                        error: err,
                        applied,
                        not_attempted: operations[index + 1..].to_vec(),
                    });
                }
            }
        }

        Ok(format_summary(&added, &modified, &deleted))
    }

    fn apply_operation(&self, operation: &PatchOperation) -> Result<AppliedPatchKind> {
        match operation {
            PatchOperation::AddFile { path, lines } => {
                let target = self.target_path(path)?;
                if target.exists() {
                    bail!("file already exists: {path}");
                }
                write_text_file(&target, &join_patch_lines(lines))?;
                Ok(AppliedPatchKind::Added)
            }
            PatchOperation::DeleteFile { path } => {
                let target = self.existing_file_path(path)?;
                fs::remove_file(&target)
                    .with_context(|| format!("unable to delete file {path}"))?;
                Ok(AppliedPatchKind::Deleted)
            }
            PatchOperation::UpdateFile {
                path,
                move_path,
                chunks,
            } => {
                let source = self.existing_file_path(path)?;
                let original = fs::read_to_string(&source)
                    .with_context(|| format!("unable to read file {path}"))?;
                let updated = if chunks.is_empty() {
                    original
                } else {
                    apply_chunks(&original, chunks, path)?
                };

                if let Some(destination_path) = move_path {
                    let destination = self.target_path(destination_path)?;
                    if destination.exists() && destination != source {
                        bail!("destination already exists: {destination_path}");
                    }
                    write_text_file(&destination, &updated)?;
                    fs::remove_file(&source)
                        .with_context(|| format!("unable to remove original file {path}"))?;
                    Ok(AppliedPatchKind::Modified)
                } else {
                    write_text_file(&source, &updated)?;
                    Ok(AppliedPatchKind::Modified)
                }
            }
        }
    }

    fn target_path(&self, path: &str) -> Result<PathBuf> {
        let normalized = normalize_patch_path(path)?;
        Ok(self.workspace_root.join(normalized))
    }

    fn existing_file_path(&self, path: &str) -> Result<PathBuf> {
        let target = self.target_path(path)?;
        let metadata = fs::metadata(&target)
            .with_context(|| format!("unable to read file metadata {path}"))?;
        if !metadata.is_file() {
            bail!("path is not a file: {path}");
        }
        Ok(target)
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
struct ApplyPatchInput {
    patch: String,
}

#[derive(Debug)]
enum ApplyPatchError {
    Fatal(anyhow::Error),
    Partial {
        message: String,
        file_changes: Vec<FileChange>,
    },
}

impl From<anyhow::Error> for ApplyPatchError {
    fn from(err: anyhow::Error) -> Self {
        Self::Fatal(err)
    }
}

#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names)]
enum PatchOperation {
    AddFile {
        path: String,
        lines: Vec<String>,
    },
    DeleteFile {
        path: String,
    },
    UpdateFile {
        path: String,
        move_path: Option<String>,
        chunks: Vec<PatchChunk>,
    },
}

impl PatchOperation {
    fn path(&self) -> &str {
        match self {
            PatchOperation::AddFile { path, .. }
            | PatchOperation::DeleteFile { path }
            | PatchOperation::UpdateFile { path, .. } => path,
        }
    }

    fn label(&self) -> String {
        match self {
            PatchOperation::AddFile { path, .. } => format!("A {path}"),
            PatchOperation::DeleteFile { path } => format!("D {path}"),
            PatchOperation::UpdateFile {
                path,
                move_path: Some(move_path),
                ..
            } => format!("M {path} -> {move_path}"),
            PatchOperation::UpdateFile { path, .. } => format!("M {path}"),
        }
    }
}

#[derive(Debug)]
enum AppliedPatchKind {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone)]
struct AppliedPatchOperations {
    added: Vec<String>,
    modified: Vec<String>,
    deleted: Vec<String>,
}

#[derive(Debug)]
struct PartialPatchError {
    failed_index: usize,
    failed_operation: PatchOperation,
    error: anyhow::Error,
    applied: AppliedPatchOperations,
    not_attempted: Vec<PatchOperation>,
}

impl PartialPatchError {
    fn format_message(&self, total_operations: usize) -> String {
        let mut output = format!(
            "Patch partially applied. Stopped at operation {}/{}.",
            self.failed_index + 1,
            total_operations
        );

        output.push_str("\n\nApplied (kept on disk):");
        let mut applied_any = false;
        for path in &self.applied.added {
            applied_any = true;
            output.push_str(&format!("\n  A {path}"));
        }
        for path in &self.applied.modified {
            applied_any = true;
            output.push_str(&format!("\n  M {path}"));
        }
        for path in &self.applied.deleted {
            applied_any = true;
            output.push_str(&format!("\n  D {path}"));
        }
        if !applied_any {
            output.push_str("\n  none");
        }

        output.push_str("\n\nFailed:");
        output.push_str(&format!("\n  {}", self.failed_operation.label()));
        output.push_str(&format!("\n  Reason: {}", self.error));

        output.push_str("\n\nNot attempted:");
        if self.not_attempted.is_empty() {
            output.push_str("\n  none");
        } else {
            for operation in &self.not_attempted {
                output.push_str(&format!("\n  {}", operation.label()));
            }
        }

        output.push_str("\n\nRe-send only the failed and not-attempted operations.");
        output
    }
}

fn affected_patch_paths(operations: &[PatchOperation]) -> Vec<String> {
    let mut paths = BTreeSet::new();
    for operation in operations {
        match operation {
            PatchOperation::AddFile { path, .. } | PatchOperation::DeleteFile { path } => {
                paths.insert(path.clone());
            }
            PatchOperation::UpdateFile {
                path, move_path, ..
            } => {
                paths.insert(path.clone());
                if let Some(move_path) = move_path {
                    paths.insert(move_path.clone());
                }
            }
        }
    }
    paths.into_iter().collect()
}

#[derive(Debug, Clone)]
struct PatchChunk {
    change_context: Option<String>,
    old_lines: Vec<String>,
    new_lines: Vec<String>,
    is_end_of_file: bool,
}

fn parse_patch(patch: &str) -> Result<Vec<PatchOperation>> {
    let patch = patch.trim_matches(|char| matches!(char, '\n' | '\r'));
    let mut lines = patch
        .lines()
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect::<Vec<_>>();

    // Silent-fix the patch envelope: '+*** Begin Patch' / '+*** End Patch' (or
    // '-' variants, or several signs stacked). These framing markers are
    // positional and reserved: there is no legitimate use case for them to
    // carry a hunk-style prefix, so we strip it before validation rather than
    // forcing the model to retry on a deterministic mistake.
    sanitize_envelope_markers(&mut lines);

    if lines.first().map(|line| line.trim()) != Some(BEGIN_PATCH_MARKER) {
        let received = lines.first().map(String::as_str).unwrap_or("");
        bail!("{}", boundary_error("first", BEGIN_PATCH_MARKER, received));
    }
    if lines.last().map(|line| line.trim()) != Some(END_PATCH_MARKER) {
        let received = lines.last().map(String::as_str).unwrap_or("");
        bail!("{}", boundary_error("last", END_PATCH_MARKER, received));
    }
    if lines.len() < 3 {
        bail!("invalid patch: at least one file operation is required");
    }

    let mut operations = Vec::new();
    let mut index = 1;
    let end_index = lines.len() - 1;

    while index < end_index {
        let line = &lines[index];
        if let Some(path) = line.strip_prefix(ADD_FILE_MARKER) {
            let path = normalize_patch_path(path)?;
            index += 1;
            let mut added_lines = Vec::new();
            while index < end_index && !is_file_operation_header(&lines[index]) {
                // Lenient mode: accept the line as content whether or not it
                // carries the documented '+' prefix. The outer loop already
                // exits on file-operation headers, and the envelope markers
                // are sanitized upstream, so any line that lands here is
                // unambiguously body content the model wanted to add.
                let line = &lines[index];
                let content = line.strip_prefix('+').unwrap_or(line.as_str());
                added_lines.push(content.to_string());
                index += 1;
            }
            if added_lines.is_empty() {
                bail!("invalid add file hunk: new file must contain at least one line");
            }
            operations.push(PatchOperation::AddFile {
                path,
                lines: added_lines,
            });
            continue;
        }

        if let Some(path) = line.strip_prefix(DELETE_FILE_MARKER) {
            operations.push(PatchOperation::DeleteFile {
                path: normalize_patch_path(path)?,
            });
            index += 1;
            continue;
        }

        if let Some(path) = line.strip_prefix(UPDATE_FILE_MARKER) {
            let path = normalize_patch_path(path)?;
            index += 1;
            let mut move_path = None;
            if index < end_index {
                if let Some(raw_move_path) = lines[index].strip_prefix(MOVE_TO_MARKER) {
                    move_path = Some(normalize_patch_path(raw_move_path)?);
                    index += 1;
                }
            }

            let mut chunks = Vec::new();
            while index < end_index && !is_file_operation_header(&lines[index]) {
                chunks.push(parse_chunk(&lines, &mut index, end_index)?);
            }

            if chunks.is_empty() && move_path.is_none() {
                bail!("invalid update hunk: expected at least one @@ section");
            }
            operations.push(PatchOperation::UpdateFile {
                path,
                move_path,
                chunks,
            });
            continue;
        }

        bail!("invalid patch header: {line}");
    }

    if operations.is_empty() {
        bail!("invalid patch: at least one file operation is required");
    }

    Ok(operations)
}

fn parse_chunk(lines: &[String], index: &mut usize, end_index: usize) -> Result<PatchChunk> {
    let header = &lines[*index];
    let change_context = if header == "@@" {
        None
    } else if let Some(context) = header.strip_prefix("@@ ") {
        Some(context.to_string())
    } else {
        bail!("invalid update hunk: expected '@@'");
    };
    *index += 1;

    let mut old_lines = Vec::new();
    let mut new_lines = Vec::new();
    let mut is_end_of_file = false;

    while *index < end_index
        && !is_file_operation_header(&lines[*index])
        && !is_chunk_header(&lines[*index])
    {
        let line = &lines[*index];
        if line == EOF_MARKER {
            is_end_of_file = true;
            *index += 1;
            break;
        }

        let Some((prefix, content)) = line.split_at_checked(1) else {
            bail!("invalid update hunk line");
        };
        match prefix {
            " " => {
                old_lines.push(content.to_string());
                new_lines.push(content.to_string());
            }
            "-" => old_lines.push(content.to_string()),
            "+" => new_lines.push(content.to_string()),
            _ => bail!("invalid update hunk line: lines must start with ' ', '-', or '+'"),
        }
        *index += 1;
    }

    // Reject hunks that contain only context lines. Without at least one
    // '+' or '-', the chunk is a no-op: it would silently round-trip the
    // file on disk and report "Patch applied" with zero added/removed
    // lines, leaving callers convinced they made a change when they did
    // not. Failing loudly here forces the agent to send a real diff.
    if old_lines == new_lines {
        bail!(
            "invalid update hunk: must contain at least one '+' or '-' line (context-only hunks have no effect)"
        );
    }

    Ok(PatchChunk {
        change_context,
        old_lines,
        new_lines,
        is_end_of_file,
    })
}

fn apply_chunks(original: &str, chunks: &[PatchChunk], path: &str) -> Result<String> {
    let mut original_lines = split_logical_lines(original);
    let replacements = compute_replacements(&original_lines, chunks, path)?;

    for (start_index, old_len, new_lines) in replacements.iter().rev() {
        original_lines.splice(
            *start_index..start_index.saturating_add(*old_len),
            new_lines.clone(),
        );
    }

    if !original_lines.last().is_some_and(String::is_empty) {
        original_lines.push(String::new());
    }

    Ok(original_lines.join("\n"))
}

fn compute_replacements(
    original_lines: &[String],
    chunks: &[PatchChunk],
    path: &str,
) -> Result<Vec<(usize, usize, Vec<String>)>> {
    let mut replacements = Vec::new();
    let mut line_index = 0usize;

    for chunk in chunks {
        if let Some(context) = &chunk.change_context {
            if let Some(index) = seek_sequence(
                original_lines,
                std::slice::from_ref(context),
                line_index,
                false,
            ) {
                line_index = index + 1;
            } else {
                let hint = if looks_like_unified_diff_range(context) {
                    " Hint: '@@' takes an optional source-code anchor (e.g. '@@ def greet():', '@@ class Foo:'), not a unified-diff line range. Use a bare '@@' for the first hunk, or anchor on a real code line above the change."
                } else {
                    ""
                };
                bail!("failed to find context '{context}' in {path}.{hint}");
            }
        }

        if chunk.old_lines.is_empty() {
            let insertion_index = if chunk.is_end_of_file {
                original_lines.len()
            } else {
                line_index.min(original_lines.len())
            };
            replacements.push((insertion_index, 0, chunk.new_lines.clone()));
            line_index = insertion_index + chunk.new_lines.len();
            continue;
        }

        let mut pattern = chunk.old_lines.as_slice();
        let mut new_slice = chunk.new_lines.as_slice();
        let mut found = seek_sequence(original_lines, pattern, line_index, chunk.is_end_of_file);

        if found.is_none() && pattern.last().is_some_and(String::is_empty) {
            pattern = &pattern[..pattern.len() - 1];
            if new_slice.last().is_some_and(String::is_empty) {
                new_slice = &new_slice[..new_slice.len() - 1];
            }
            found = seek_sequence(original_lines, pattern, line_index, chunk.is_end_of_file);
        }

        let Some(start_index) = found else {
            bail!(
                "failed to find expected lines in {path}:\n{}",
                chunk.old_lines.join("\n")
            );
        };
        replacements.push((start_index, pattern.len(), new_slice.to_vec()));
        line_index = start_index + pattern.len();
    }

    replacements.sort_by_key(|(start_index, _, _)| *start_index);
    Ok(replacements)
}

fn seek_sequence(lines: &[String], pattern: &[String], start: usize, eof: bool) -> Option<usize> {
    if pattern.is_empty() {
        return Some(start.min(lines.len()));
    }
    if pattern.len() > lines.len() {
        return None;
    }

    let search_start = if eof && lines.len() >= pattern.len() {
        lines.len() - pattern.len()
    } else {
        start.min(lines.len().saturating_sub(pattern.len()))
    };

    for mode in 0..3 {
        for index in search_start..=lines.len().saturating_sub(pattern.len()) {
            let matched = pattern.iter().enumerate().all(|(offset, expected)| {
                let actual = &lines[index + offset];
                match mode {
                    0 => actual == expected,
                    1 => actual.trim_end() == expected.trim_end(),
                    _ => actual.trim() == expected.trim(),
                }
            });
            if matched {
                return Some(index);
            }
        }
    }

    // 4th pass: normalise common Unicode punctuation (em-dashes, curly
    // quotes, non-breaking spaces, ideographic spaces, etc.) to their ASCII
    // equivalents before comparing. This mirrors `git apply`'s tolerance
    // and rescues the (frequent) case where the model emits ASCII context
    // for a source file that contains typographic characters.
    for index in search_start..=lines.len().saturating_sub(pattern.len()) {
        let matched = pattern.iter().enumerate().all(|(offset, expected)| {
            normalize_for_match(&lines[index + offset]) == normalize_for_match(expected)
        });
        if matched {
            return Some(index);
        }
    }

    None
}

fn split_logical_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut lines = text
        .split('\n')
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

fn join_patch_lines(lines: &[String]) -> String {
    if lines.is_empty() {
        String::new()
    } else {
        let mut content = lines.join("\n");
        content.push('\n');
        content
    }
}

fn write_text_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("unable to create directory {}", parent.display()))?;
    }
    fs::write(path, content).with_context(|| format!("unable to write file {}", path.display()))
}

fn format_summary(added: &[String], modified: &[String], deleted: &[String]) -> String {
    let mut output = String::from("Success. Updated the following files:");
    for path in added {
        output.push_str(&format!("\nA {path}"));
    }
    for path in modified {
        output.push_str(&format!("\nM {path}"));
    }
    for path in deleted {
        output.push_str(&format!("\nD {path}"));
    }
    output
}

fn normalize_patch_path(path: &str) -> Result<String> {
    let normalized = normalize_workspace_relative_path(path.trim())?;
    if normalized.is_empty() {
        bail!("path cannot be empty");
    }
    Ok(normalized)
}

fn is_file_operation_header(line: &str) -> bool {
    line.starts_with(ADD_FILE_MARKER)
        || line.starts_with(DELETE_FILE_MARKER)
        || line.starts_with(UPDATE_FILE_MARKER)
}

fn is_chunk_header(line: &str) -> bool {
    line == "@@" || line.starts_with("@@ ")
}

/// Build a helpful error message when a Begin/End Patch boundary line is wrong.
/// Echoes the received line and adds a targeted hint when the marker was
/// prefixed with '+' or '-' (the most common LLM mistake on this format).
fn boundary_error(position: &str, expected: &str, received: &str) -> String {
    if received.is_empty() {
        return format!(
            "invalid patch: {position} line must be '{expected}' (patch is empty or whitespace-only)"
        );
    }
    let trimmed = received.trim();
    let after_strip = trimmed
        .strip_prefix('+')
        .or_else(|| trimmed.strip_prefix('-'))
        .map(str::trim_start)
        .unwrap_or(trimmed);
    let hint = if after_strip == expected {
        format!(
            " Hint: '{expected}' was prefixed with '+' or '-'. Framing markers (*** Begin Patch, *** End Patch, *** Add/Update/Delete File:, *** Move to:) appear verbatim \u{2014} only file content lines inside a hunk carry '+' or '-'."
        )
    } else {
        String::new()
    };
    format!(
        "invalid patch: {position} line must be exactly '{expected}'. Received: '{received}'.{hint}"
    )
}

/// Strip leading '+' or '-' signs (any number of them) from the patch envelope
/// markers when they are the only thing wrong with the line. We intentionally
/// limit this silent fix to the very first and very last line because those
/// are the *positional* framing of the patch: there is no legitimate world
/// where the model wants the literal text `*** Begin Patch` or `*** End Patch`
/// as the first/last line of its payload. For any other occurrence of these
/// markers (e.g. inside an Add File body) the lenient body parser will keep
/// the `+` as content, which is the desired behaviour.
fn sanitize_envelope_markers(lines: &mut [String]) {
    fn strip_all_signs(mut s: &str) -> &str {
        while let Some(rest) = s.strip_prefix('+').or_else(|| s.strip_prefix('-')) {
            s = rest;
        }
        s
    }
    if let Some(first) = lines.first_mut() {
        if strip_all_signs(first).trim() == BEGIN_PATCH_MARKER {
            *first = BEGIN_PATCH_MARKER.to_string();
        }
    }
    if let Some(last) = lines.last_mut() {
        if strip_all_signs(last).trim() == END_PATCH_MARKER {
            *last = END_PATCH_MARKER.to_string();
        }
    }
}

/// Normalise common typographic Unicode characters to their ASCII equivalents
/// for the most permissive pass of `seek_sequence`. This rescues diffs whose
/// `-`/context lines were authored with ASCII but target source files that
/// contain em-dashes, smart quotes, or non-breaking spaces (very common in
/// Markdown, prose, and even minified JS bundles).
fn normalize_for_match(s: &str) -> String {
    s.trim()
        .chars()
        .map(|c| match c {
            // Various dash / hyphen code-points -> ASCII '-'
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}'
            | '\u{2212}' => '-',
            // Curly single quotes -> ASCII '
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => '\'',
            // Curly double quotes -> ASCII "
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => '"',
            // Non-breaking and other odd spaces -> normal space
            '\u{00A0}' | '\u{2002}' | '\u{2003}' | '\u{2004}' | '\u{2005}' | '\u{2006}'
            | '\u{2007}' | '\u{2008}' | '\u{2009}' | '\u{200A}' | '\u{202F}' | '\u{205F}'
            | '\u{3000}' => ' ',
            other => other,
        })
        .collect()
}

/// Heuristic that recognizes unified-diff hunk headers like `-10,7 +10,8` or
/// `-1 +1`. Used to detect when the model wrote a git-style `@@ -N,M +N,M @@`
/// header instead of our source-anchor `@@ <code line>`.
fn looks_like_unified_diff_range(context: &str) -> bool {
    let mut chars = context.trim().chars().peekable();
    if chars.next() != Some('-') {
        return false;
    }
    if !chars.peek().is_some_and(|c| c.is_ascii_digit()) {
        return false;
    }
    while chars
        .peek()
        .is_some_and(|c| c.is_ascii_digit() || *c == ',')
    {
        chars.next();
    }
    if chars.next() != Some(' ') {
        return false;
    }
    if chars.next() != Some('+') {
        return false;
    }
    chars.next().is_some_and(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    #[tokio::test]
    async fn applies_patch_to_existing_file() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        fs::write(root.join("note.txt"), "old\n").expect("write file");

        let tool = ApplyPatchTool::new(&root);
        let result = tool
            .apply(json!({
                "patch": "*** Begin Patch\n*** Update File: note.txt\n@@\n-old\n+new\n*** End Patch\n"
            }))
            .await
            .expect("patch should apply");

        assert!(!result.is_error);
        assert_eq!(
            fs::read_to_string(root.join("note.txt")).expect("read file"),
            "new\n"
        );
        assert_eq!(result.file_changes.len(), 1);
        assert_eq!(result.file_changes[0].added_lines, 1);
        assert_eq!(result.file_changes[0].removed_lines, 1);
        assert!(!result.file_changes[0].lines.is_empty());
        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn patch_diff_details_ignore_unrelated_workspace_size() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        for index in 0..18 {
            fs::write(
                root.join(format!("large-{index}.txt")),
                "unrelated\n".repeat(32_000),
            )
            .expect("write large file");
        }
        fs::write(root.join("note.txt"), "old\n").expect("write file");

        let tool = ApplyPatchTool::new(&root);
        let result = tool
            .apply(json!({
                "patch": "*** Begin Patch\n*** Update File: note.txt\n@@\n-old\n+new\n*** End Patch\n"
            }))
            .await
            .expect("patch should apply");

        assert!(!result.is_error);
        assert_eq!(result.file_changes.len(), 1);
        assert_eq!(result.file_changes[0].added_lines, 1);
        assert_eq!(result.file_changes[0].removed_lines, 1);
        assert!(!result.file_changes[0].lines.is_empty());
        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn applies_patch_to_create_and_delete_files() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        fs::write(root.join("old.txt"), "bye\n").expect("write file");

        let tool = ApplyPatchTool::new(&root);
        let result = tool
            .apply(json!({
                "patch": concat!(
                    "*** Begin Patch\n",
                    "*** Add File: new.txt\n",
                    "+hi\n",
                    "*** Delete File: old.txt\n",
                    "*** End Patch\n"
                )
            }))
            .await
            .expect("patch should apply");

        assert!(!result.is_error);
        assert_eq!(
            fs::read_to_string(root.join("new.txt")).expect("read file"),
            "hi\n"
        );
        assert!(!root.join("old.txt").exists());
        assert_eq!(result.file_changes.len(), 2);
        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn allows_existing_file_patch_without_read() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        fs::write(root.join("note.txt"), "old\n").expect("write file");

        let tool = ApplyPatchTool::new(&root);
        let result = tool
            .apply(
                json!({
                    "patch": "*** Begin Patch\n*** Update File: note.txt\n@@\n-old\n+new\n*** End Patch\n"
                }),
            )
            .await
            .expect("patch should apply");

        assert!(!result.is_error);
        assert_eq!(
            fs::read_to_string(root.join("note.txt")).expect("read file"),
            "new\n"
        );
        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn allows_new_file_patch_without_read() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");

        let tool = ApplyPatchTool::new(&root);
        let result = tool
            .apply(json!({
                "patch": concat!(
                    "*** Begin Patch\n",
                    "*** Add File: new.txt\n",
                    "+hi\n",
                    "*** End Patch\n"
                )
            }))
            .await
            .expect("new file patch should apply");

        assert!(!result.is_error);
        assert_eq!(
            fs::read_to_string(root.join("new.txt")).expect("read file"),
            "hi\n"
        );
        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn applies_patch_to_move_file() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        fs::write(root.join("from.txt"), "old\n").expect("write file");

        let tool = ApplyPatchTool::new(&root);
        let result = tool
            .apply(json!({
                "patch": concat!(
                    "*** Begin Patch\n",
                    "*** Update File: from.txt\n",
                    "*** Move to: to.txt\n",
                    "@@\n",
                    "-old\n",
                    "+new\n",
                    "*** End Patch\n"
                )
            }))
            .await
            .expect("move patch should apply");

        assert!(!result.is_error);
        assert!(!root.join("from.txt").exists());
        assert_eq!(
            fs::read_to_string(root.join("to.txt")).expect("read moved file"),
            "new\n"
        );
        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn applies_patch_to_rename_file_without_content_change() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        fs::write(root.join("from.txt"), "same").expect("write file");

        let tool = ApplyPatchTool::new(&root);
        let result = tool
            .apply(json!({
                "patch": concat!(
                    "*** Begin Patch\n",
                    "*** Update File: from.txt\n",
                    "*** Move to: to.txt\n",
                    "*** End Patch\n"
                )
            }))
            .await
            .expect("rename patch should apply");

        assert!(!result.is_error);
        assert!(!root.join("from.txt").exists());
        assert_eq!(
            fs::read_to_string(root.join("to.txt")).expect("read renamed file"),
            "same"
        );
        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn partial_patch_error_reports_applied_failed_and_not_attempted_operations() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        fs::write(root.join("first.txt"), "old first\n").expect("write first file");
        fs::write(root.join("second.txt"), "actual second\n").expect("write second file");
        fs::write(root.join("third.txt"), "old third\n").expect("write third file");

        let tool = ApplyPatchTool::new(&root);
        let result = tool
            .run(json!({
                "patch": concat!(
                    "*** Begin Patch\n",
                    "*** Update File: first.txt\n",
                    "@@\n",
                    "-old first\n",
                    "+new first\n",
                    "*** Update File: second.txt\n",
                    "@@\n",
                    "-missing second\n",
                    "+new second\n",
                    "*** Update File: third.txt\n",
                    "@@\n",
                    "-old third\n",
                    "+new third\n",
                    "*** End Patch\n"
                )
            }))
            .await;

        assert!(result.is_error);
        assert_eq!(
            fs::read_to_string(root.join("first.txt")).expect("read first file"),
            "new first\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("second.txt")).expect("read second file"),
            "actual second\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("third.txt")).expect("read third file"),
            "old third\n"
        );
        assert!(result
            .content
            .contains("Patch partially applied. Stopped at operation 2/3."));
        assert!(result
            .content
            .contains("Applied (kept on disk):\n  M first.txt"));
        assert!(result.content.contains("Failed:\n  M second.txt"));
        assert!(result
            .content
            .contains("Reason: failed to find expected lines in second.txt"));
        assert!(result.content.contains("Not attempted:\n  M third.txt"));
        assert!(result
            .content
            .contains("Re-send only the failed and not-attempted operations."));
        assert_eq!(result.file_changes.len(), 1);
        assert_eq!(result.file_changes[0].relative_path, "first.txt");

        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn silently_accepts_plus_prefix_on_end_patch_marker() {
        // The model's most common mistake: prefixing the End Patch terminator
        // with '+' because it forgot to break out of the Add File "+" mode.
        // We absorb this rather than forcing a retry on a 20 KB patch.
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");

        let tool = ApplyPatchTool::new(&root);
        let result = tool
            .run(json!({
                "patch": "*** Begin Patch\n*** Add File: hi.txt\n+hello\n+*** End Patch\n"
            }))
            .await;

        assert!(
            !result.is_error,
            "should silent-fix, got: {}",
            result.content
        );
        let content = fs::read_to_string(root.join("hi.txt")).expect("read hi.txt");
        assert_eq!(content, "hello\n");
        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn silently_accepts_plus_prefix_on_begin_patch_marker() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");

        let tool = ApplyPatchTool::new(&root);
        let result = tool
            .run(json!({
                "patch": "+*** Begin Patch\n*** Add File: hi.txt\n+hello\n*** End Patch\n"
            }))
            .await;

        assert!(
            !result.is_error,
            "should silent-fix, got: {}",
            result.content
        );
        let content = fs::read_to_string(root.join("hi.txt")).expect("read hi.txt");
        assert_eq!(content, "hello\n");
        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn silently_accepts_unprefixed_lines_in_add_file_body() {
        // Reproduces the conv-n°2 bug: the model interleaves '+' lines with
        // bare (whitespace-prefixed) lines inside an Add File body. The
        // intent is unambiguous, so we treat every body line as content.
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");

        let tool = ApplyPatchTool::new(&root);
        let patch = "*** Begin Patch\n*** Add File: index.html\n+<!DOCTYPE html>\n+<html>\n   <body>\n   </body>\n+</html>\n*** End Patch\n";
        let result = tool.run(json!({ "patch": patch })).await;

        assert!(
            !result.is_error,
            "should silent-fix, got: {}",
            result.content
        );
        let content = fs::read_to_string(root.join("index.html")).expect("read index.html");
        assert_eq!(
            content,
            "<!DOCTYPE html>\n<html>\n   <body>\n   </body>\n</html>\n"
        );
        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn fuzzy_match_normalizes_smart_quotes_and_em_dashes() {
        // The source file uses typographic punctuation (em-dash, smart
        // quote, non-breaking space). The model writes its diff with plain
        // ASCII. The 4th seek_sequence pass normalises both sides before
        // comparing so the patch still applies.
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        fs::write(
            root.join("doc.md"),
            "## Title \u{2014} with em-dash and \u{2018}smart\u{2019} quote\n",
        )
        .expect("write file");

        let tool = ApplyPatchTool::new(&root);
        let patch = "*** Begin Patch\n*** Update File: doc.md\n@@\n-## Title - with em-dash and 'smart' quote\n+## Title (rewritten)\n*** End Patch\n";
        let result = tool.run(json!({ "patch": patch })).await;

        assert!(
            !result.is_error,
            "should match across Unicode/ASCII boundary, got: {}",
            result.content
        );
        let updated = fs::read_to_string(root.join("doc.md")).expect("read doc.md");
        assert!(
            updated.contains("## Title (rewritten)"),
            "file should be updated, got: {updated}"
        );
        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn error_when_at_at_uses_unified_diff_range_gives_hint() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        fs::write(root.join("a.txt"), "line one\nline two\nline three\n").expect("write file");

        let tool = ApplyPatchTool::new(&root);
        let result = tool
            .run(json!({
                "patch": "*** Begin Patch\n*** Update File: a.txt\n@@ -1,3 +1,3 @@\n-line two\n+LINE TWO\n*** End Patch\n"
            }))
            .await;

        assert!(result.is_error);
        assert!(
            result
                .content
                .contains("failed to find context '-1,3 +1,3 @@'"),
            "error should echo the bogus context, got: {}",
            result.content
        );
        assert!(
            result.content.contains("unified-diff line range"),
            "error should hint at the unified-diff confusion, got: {}",
            result.content
        );
        fs::remove_dir_all(root).ok();
    }

    fn unique_temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("sinew-patch-test-{}", Uuid::new_v4()))
    }
}
