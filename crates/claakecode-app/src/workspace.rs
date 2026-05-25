use std::{
    cmp::Ordering,
    fs,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::{Deserialize, Serialize};

use crate::text::{decode_text, encode_text, TextEncoding};

const MAX_EDITABLE_BYTES: u64 = 1024 * 1024;
const MAX_PREVIEW_IMAGE_BYTES: u64 = 25 * 1024 * 1024;
const MAX_INDEXED_FILES: usize = 10_000;
const MAX_SEARCH_FILES: usize = 80;
const MAX_SEARCH_MATCHES: usize = 400;
const MAX_SEARCH_MATCHES_PER_FILE: usize = 8;
const SEARCH_LINE_CONTEXT_BEFORE_CHARS: usize = 18;
const SEARCH_LINE_CONTEXT_AFTER_CHARS: usize = 64;
const DESIGN_SYSTEM_FILE_NAME: &str = "design.md";
const DESIGN_SYSTEM_TEMPLATE: &str =
    "This is the design system of our project :\n\n'Paste design system here'";
const CLAUDE_FILE_NAME: &str = "claude.md";
const CLAUDE_FILE_TEMPLATE: &str =
    "Note: Claake Code does not use this CLAUDE.md file as its reference instructions. Use AGENTS.md instead.\n";
const IGNORED_DIRS: &[&str] = &[
    ".git",
    ".history",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".turbo",
    ".cache",
    ".idea",
    "__pycache__",
    ".pytest_cache",
    ".venv",
    "venv",
    ".mypy_cache",
    "out",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceEntryKind {
    File,
    Directory,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntry {
    pub name: String,
    pub relative_path: String,
    pub absolute_path: String,
    pub kind: WorkspaceEntryKind,
    pub has_children: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDocument {
    pub name: String,
    pub relative_path: String,
    pub absolute_path: String,
    pub editable: bool,
    pub content: Option<String>,
    pub reason: Option<String>,
    pub size: u64,
    pub last_modified_ms: Option<i64>,
    pub image_media_type: Option<String>,
    pub image_data: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchResult {
    pub query: String,
    pub files_scanned: usize,
    pub total_matches: usize,
    pub files: Vec<WorkspaceSearchFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchFile {
    pub name: String,
    pub relative_path: String,
    pub absolute_path: String,
    pub path_match: bool,
    pub match_count: usize,
    pub matches: Vec<WorkspaceSearchMatch>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchMatch {
    pub line_number: usize,
    pub column_start: usize,
    pub column_end: usize,
    pub line_text: String,
    pub match_start: usize,
    pub match_end: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFileChangeEvent {
    pub workspace_path: String,
    pub relative_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDeletedEntry {
    pub name: String,
    pub relative_path: String,
    pub original_absolute_path: String,
    pub trash_path: String,
    pub kind: WorkspaceEntryKind,
}

pub fn normalize_workspace_root(path: impl AsRef<Path>) -> Result<PathBuf> {
    let canonical = path
        .as_ref()
        .canonicalize()
        .with_context(|| format!("unable to open workspace {}", path.as_ref().display()))?;

    if !canonical.is_dir() {
        bail!("workspace must be a directory");
    }

    Ok(canonical)
}

pub fn workspace_info(root: &Path) -> WorkspaceInfo {
    WorkspaceInfo {
        path: root.display().to_string(),
        name: root
            .file_name()
            .and_then(|value| value.to_str())
            .map(|value| value.to_string())
            .unwrap_or_else(|| root.display().to_string()),
    }
}

pub fn list_workspace_entries(
    root: &Path,
    relative_path: Option<&str>,
) -> Result<Vec<WorkspaceEntry>> {
    let directory = match relative_path.filter(|value| !value.is_empty()) {
        Some(value) => resolve_workspace_path(root, value)?,
        None => root.to_path_buf(),
    };

    if !directory.is_dir() {
        bail!("path is not a directory");
    }

    let mut entries = Vec::new();
    for item in fs::read_dir(&directory)
        .with_context(|| format!("unable to list directory {}", directory.display()))?
    {
        let item = match item {
            Ok(item) => item,
            Err(_) => continue,
        };
        let path = item.path();
        let metadata = match item.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        let kind = if metadata.is_dir() {
            WorkspaceEntryKind::Directory
        } else {
            WorkspaceEntryKind::File
        };
        let relative_path = relative_from_root(root, &path)?;
        let has_children =
            matches!(kind, WorkspaceEntryKind::Directory) && directory_has_children(&path);
        entries.push(WorkspaceEntry {
            name: item.file_name().to_string_lossy().into_owned(),
            relative_path,
            absolute_path: path.display().to_string(),
            kind,
            has_children,
        });
    }

    entries.sort_by(|left, right| match (left.kind, right.kind) {
        (WorkspaceEntryKind::Directory, WorkspaceEntryKind::File) => Ordering::Less,
        (WorkspaceEntryKind::File, WorkspaceEntryKind::Directory) => Ordering::Greater,
        _ => left
            .name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase()),
    });

    Ok(entries)
}

pub fn list_workspace_files(root: &Path) -> Result<Vec<WorkspaceEntry>> {
    use walkdir::WalkDir;

    let mut entries = Vec::new();
    let walker = WalkDir::new(root).follow_links(false).into_iter();
    for item in walker.filter_entry(|entry| !is_walk_ignored(entry)) {
        if entries.len() >= MAX_INDEXED_FILES {
            break;
        }
        let item = match item {
            Ok(item) => item,
            Err(_) => continue,
        };
        if !item.file_type().is_file() {
            continue;
        }
        let path = item.path();
        let relative_path = match relative_from_root(root, path) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if relative_path.is_empty() {
            continue;
        }
        entries.push(WorkspaceEntry {
            name: item.file_name().to_string_lossy().into_owned(),
            relative_path,
            absolute_path: path.display().to_string(),
            kind: WorkspaceEntryKind::File,
            has_children: false,
        });
    }
    entries.sort_by(|left, right| {
        left.relative_path
            .to_ascii_lowercase()
            .cmp(&right.relative_path.to_ascii_lowercase())
    });
    Ok(entries)
}

pub fn search_workspace_files(root: &Path, query: &str) -> Result<WorkspaceSearchResult> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(WorkspaceSearchResult {
            query: String::new(),
            files_scanned: 0,
            total_matches: 0,
            files: Vec::new(),
        });
    }

    let query_lower = query.to_lowercase();
    let terms = query_lower
        .split_whitespace()
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();
    let mut files_scanned = 0usize;
    let mut total_matches = 0usize;
    let mut results = Vec::<ScoredSearchFile>::new();

    for entry in list_workspace_files(root)? {
        if results.len() >= MAX_SEARCH_FILES && total_matches >= MAX_SEARCH_MATCHES {
            break;
        }

        let path_lower = entry.relative_path.to_lowercase();
        let path_score = path_match_score(&query_lower, &terms, &path_lower);
        let mut match_count = 0usize;
        let mut matches = Vec::new();

        if total_matches < MAX_SEARCH_MATCHES {
            if let Ok(doc) = read_workspace_file(root, &entry.relative_path) {
                if let Some(content) = doc.content {
                    files_scanned += 1;
                    for (index, raw_line) in content.lines().enumerate() {
                        let Some(line_match) = line_match(&query_lower, &terms, raw_line) else {
                            continue;
                        };
                        match_count += 1;
                        total_matches += 1;
                        if matches.len() < MAX_SEARCH_MATCHES_PER_FILE {
                            matches.push(search_match_from_line(index + 1, raw_line, line_match));
                        }
                        if total_matches >= MAX_SEARCH_MATCHES {
                            break;
                        }
                    }
                }
            }
        }

        if path_score.is_none() && matches.is_empty() {
            continue;
        }

        let score = search_file_score(path_score, match_count, &entry.relative_path);
        results.push(ScoredSearchFile {
            score,
            file: WorkspaceSearchFile {
                name: entry.name,
                relative_path: entry.relative_path,
                absolute_path: entry.absolute_path,
                path_match: path_score.is_some(),
                match_count,
                matches,
            },
        });
    }

    results.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.file.relative_path.cmp(&right.file.relative_path))
    });

    Ok(WorkspaceSearchResult {
        query: query.to_string(),
        files_scanned,
        total_matches,
        files: results.into_iter().map(|result| result.file).collect(),
    })
}

fn is_walk_ignored(entry: &walkdir::DirEntry) -> bool {
    if entry.depth() == 0 {
        return false;
    }
    if !entry.file_type().is_dir() {
        return false;
    }
    let name = entry.file_name().to_string_lossy();
    IGNORED_DIRS.iter().any(|dir| name == *dir)
}

struct ScoredSearchFile {
    score: i64,
    file: WorkspaceSearchFile,
}

fn search_file_score(path_score: Option<i64>, match_count: usize, relative_path: &str) -> i64 {
    path_score.unwrap_or(0) + (match_count.min(MAX_SEARCH_MATCHES_PER_FILE) as i64 * 1_500)
        - relative_path.len() as i64
}

#[derive(Debug, Clone, Copy)]
struct SearchLineMatch {
    start_char: usize,
    end_char: usize,
}

fn line_match(query: &str, terms: &[&str], raw_line: &str) -> Option<SearchLineMatch> {
    let line = raw_line.to_lowercase();
    if let Some(start) = line.find(query) {
        let start_char = byte_to_char_index(&line, start);
        let end_char = byte_to_char_index(&line, start + query.len());
        return Some(SearchLineMatch {
            start_char,
            end_char,
        });
    }
    if terms.len() > 1 && terms.iter().all(|term| line.contains(term)) {
        let first = terms
            .iter()
            .filter_map(|term| line.find(term).map(|start| (term, start)))
            .min_by_key(|(_, start)| *start)?;
        let start_char = byte_to_char_index(&line, first.1);
        let end_char = byte_to_char_index(&line, first.1 + first.0.len());
        return Some(SearchLineMatch {
            start_char,
            end_char,
        });
    }
    None
}

fn path_match_score(query: &str, terms: &[&str], path: &str) -> Option<i64> {
    if path.contains(query) {
        return Some(3_000 - path.len() as i64);
    }
    if terms.len() > 1 && terms.iter().all(|term| path.contains(term)) {
        return Some(2_400 - path.len() as i64);
    }
    fuzzy_score(query, path).map(|score| score + 1_000 - path.len() as i64)
}

fn fuzzy_score(query: &str, text: &str) -> Option<i64> {
    let needle = query
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<Vec<_>>();
    if needle.is_empty() {
        return None;
    }

    let haystack = text.chars().collect::<Vec<_>>();
    let mut cursor = 0usize;
    let mut last_match = None::<usize>;
    let mut score = 0i64;

    for wanted in needle {
        let offset = haystack[cursor..].iter().position(|ch| *ch == wanted)?;
        let index = cursor + offset;
        score += 12;
        if last_match == Some(index.saturating_sub(1)) {
            score += 24;
        }
        if index == 0 || is_path_separator(haystack[index.saturating_sub(1)]) {
            score += 18;
        }
        last_match = Some(index);
        cursor = index + 1;
    }

    Some(score)
}

fn is_path_separator(ch: char) -> bool {
    matches!(ch, '/' | '-' | '_' | '.' | ' ')
}

fn search_match_from_line(
    line_number: usize,
    raw_line: &str,
    line_match: SearchLineMatch,
) -> WorkspaceSearchMatch {
    let (line_text, match_start, match_end) =
        search_line_snippet(raw_line, line_match.start_char, line_match.end_char);
    WorkspaceSearchMatch {
        line_number,
        column_start: line_match.start_char + 1,
        column_end: line_match.end_char + 1,
        line_text,
        match_start,
        match_end,
    }
}

fn search_line_snippet(raw: &str, match_start: usize, match_end: usize) -> (String, usize, usize) {
    let chars = raw.chars().collect::<Vec<_>>();
    let len = chars.len();
    let start = match_start.saturating_sub(SEARCH_LINE_CONTEXT_BEFORE_CHARS);
    let end = (match_end + SEARCH_LINE_CONTEXT_AFTER_CHARS).min(len);
    let has_prefix = start > 0;
    let has_suffix = end < len;

    let mut snippet = String::new();
    if has_prefix {
        snippet.push_str("...");
    }
    snippet.extend(chars[start..end].iter());
    if has_suffix {
        snippet.push_str("...");
    }

    let offset = if has_prefix { 3 } else { 0 };
    (
        snippet,
        match_start.saturating_sub(start) + offset,
        match_end.saturating_sub(start) + offset,
    )
}

fn byte_to_char_index(value: &str, byte_index: usize) -> usize {
    value
        .char_indices()
        .take_while(|(index, _)| *index < byte_index)
        .count()
}

pub fn read_workspace_file(root: &Path, relative_path: &str) -> Result<FileDocument> {
    let path = resolve_workspace_path(root, relative_path)?;
    read_document(root, &path)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalPathResolution {
    /// "file" | "directory" | "missing"
    pub kind: String,
    pub absolute_path: String,
    pub relative_path: Option<String>,
    pub is_outside_workspace: bool,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

/// Parse a path emitted by the terminal (typically by an AI/CLI tool) and
/// figure out where it lives. Supports `path`, `path:line`, `path:line:col`,
/// `~` expansion, relative paths against the workspace and absolute paths.
pub fn resolve_terminal_path(
    workspace_root: &Path,
    raw_path: &str,
) -> Result<TerminalPathResolution> {
    let cleaned = clean_terminal_token(raw_path);
    if cleaned.is_empty() {
        bail!("empty path");
    }

    let (path_part, line, column) = split_path_and_position(&cleaned);

    // Try the (stripped, line, col) variant first, fall back to using the
    // full raw input when nothing matches on disk. This handles paths that
    // legitimately contain a `:` (Windows drives, weird filenames, ...).
    let mut attempts: Vec<(String, Option<u32>, Option<u32>)> = Vec::new();
    attempts.push((path_part.clone(), line, column));
    if path_part != cleaned {
        attempts.push((cleaned.clone(), None, None));
    }

    for (candidate, candidate_line, candidate_col) in attempts {
        let Some(absolute) = expand_to_absolute(workspace_root, &candidate) else {
            continue;
        };
        let metadata = match fs::metadata(&absolute) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        let kind = if metadata.is_dir() {
            "directory".to_string()
        } else {
            "file".to_string()
        };
        let (relative_path, is_outside) = match absolute.strip_prefix(workspace_root) {
            Ok(rel) => {
                let value = rel.to_string_lossy().to_string();
                (Some(value), false)
            }
            Err(_) => (None, true),
        };
        return Ok(TerminalPathResolution {
            kind,
            absolute_path: absolute.display().to_string(),
            relative_path,
            is_outside_workspace: is_outside,
            line: candidate_line,
            column: candidate_col,
        });
    }

    // Nothing existed on disk: still return a best-effort absolute path so
    // the frontend can decide what to do (typically just ignore it).
    let fallback_absolute =
        expand_to_absolute(workspace_root, &path_part).map(|p| p.display().to_string());
    Ok(TerminalPathResolution {
        kind: "missing".to_string(),
        absolute_path: fallback_absolute.unwrap_or_else(|| path_part.clone()),
        relative_path: None,
        is_outside_workspace: true,
        line,
        column,
    })
}

/// Read a file from anywhere on disk using its absolute path. Used by the
/// terminal cmd+click handler to display files outside of the active
/// workspace in a read-only Monaco tab.
pub fn read_external_file(path: &Path) -> Result<FileDocument> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("unable to read file metadata {}", path.display()))?;
    if !metadata.is_file() {
        bail!("path is not a file");
    }

    let size = metadata.len();
    let last_modified_ms = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_millis() as i64);

    let bytes =
        fs::read(path).with_context(|| format!("unable to read file {}", path.display()))?;
    let image_media_type = preview_image_media_type(path).map(str::to_string);
    let image_data = if image_media_type.is_some() && size <= MAX_PREVIEW_IMAGE_BYTES {
        Some(BASE64_STANDARD.encode(&bytes))
    } else {
        None
    };
    // External files are always returned with `editable: false`. The
    // frontend uses this flag together with its own `external` marker to
    // decide whether to render Monaco in read-only mode or fall back to the
    // non-editable preview view.
    let (content, reason) = if size > MAX_EDITABLE_BYTES {
        (None, Some("File too large to display.".to_string()))
    } else if let Some(decoded) = decode_text(&bytes) {
        (Some(decoded.content), None)
    } else {
        (None, Some("Binary file is not displayable.".to_string()))
    };

    Ok(FileDocument {
        name: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string(),
        // For external files we use the absolute path as the identifier
        // because there is no meaningful workspace-relative path.
        relative_path: path.display().to_string(),
        absolute_path: path.display().to_string(),
        editable: false,
        content,
        reason,
        size,
        last_modified_ms,
        image_media_type,
        image_data,
    })
}

fn clean_terminal_token(raw: &str) -> String {
    let mut value = raw.trim();
    // Strip wrapping quotes
    if ((value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\'')))
        && value.len() >= 2
    {
        value = &value[1..value.len() - 1];
    }
    // Strip trailing punctuation often present in prose ("see foo.rs.").
    let trimmed = value.trim_end_matches([',', '.', ';', ')', ']', '>']);
    let trimmed = trimmed.trim_start_matches(['(', '[', '<']);
    trimmed.trim().to_string()
}

fn split_path_and_position(raw: &str) -> (String, Option<u32>, Option<u32>) {
    // path:line:col first (greedy)
    if let Some((rest, last)) = raw.rsplit_once(':') {
        if let Ok(last_num) = last.parse::<u32>() {
            if let Some((path, middle)) = rest.rsplit_once(':') {
                if let Ok(middle_num) = middle.parse::<u32>() {
                    // path:line:col
                    if !path.is_empty() && !looks_like_windows_drive(path) {
                        return (path.to_string(), Some(middle_num), Some(last_num));
                    }
                }
            }
            // path:line
            if !rest.is_empty() && !looks_like_windows_drive(rest) {
                return (rest.to_string(), Some(last_num), None);
            }
        }
    }
    (raw.to_string(), None, None)
}

fn looks_like_windows_drive(value: &str) -> bool {
    // "C", "D", etc. — a single ASCII letter is most likely a drive letter
    // that the splitter consumed by accident.
    let mut chars = value.chars();
    let first = chars.next();
    let rest = chars.next();
    matches!(first, Some(c) if c.is_ascii_alphabetic()) && rest.is_none()
}

fn expand_to_absolute(workspace_root: &Path, candidate: &str) -> Option<PathBuf> {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return None;
    }

    // ~ or ~/foo
    if trimmed == "~" || trimmed.starts_with("~/") {
        if let Some(home) = home_dir() {
            let suffix = trimmed.trim_start_matches('~').trim_start_matches('/');
            return Some(if suffix.is_empty() {
                home
            } else {
                home.join(suffix)
            });
        }
    }

    let candidate_path = Path::new(trimmed);
    if candidate_path.is_absolute() {
        return Some(candidate_path.to_path_buf());
    }

    Some(workspace_root.join(candidate_path))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

pub fn write_workspace_file(
    root: &Path,
    relative_path: &str,
    content: &str,
) -> Result<FileDocument> {
    let path = resolve_workspace_path(root, relative_path)?;
    let parent = path.parent().ok_or_else(|| anyhow!("invalid file path"))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("unable to create directory {}", parent.display()))?;

    let encoding = fs::read(&path)
        .ok()
        .and_then(|bytes| decode_text(&bytes).map(|decoded| decoded.encoding))
        .unwrap_or(TextEncoding::Utf8);
    fs::write(&path, encode_text(content, encoding))
        .with_context(|| format!("unable to write file {}", path.display()))?;
    read_document(root, &path)
}

pub fn create_workspace_file(
    root: &Path,
    target_relative: Option<&str>,
    name: &str,
) -> Result<WorkspaceEntry> {
    let target_dir = resolve_workspace_directory(root, target_relative)?;
    let path = target_dir.join(clean_new_entry_path(name)?);
    ensure_path_stays_in_root(root, &path)?;
    if path.exists() {
        bail!("file already exists");
    }

    let parent = path.parent().ok_or_else(|| anyhow!("invalid file path"))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("unable to create directory {}", parent.display()))?;
    fs::write(&path, initial_workspace_file_contents(&path))
        .with_context(|| format!("unable to create file {}", path.display()))?;
    workspace_entry_from_path(root, &path)
}

pub fn create_workspace_directory(
    root: &Path,
    target_relative: Option<&str>,
    name: &str,
) -> Result<WorkspaceEntry> {
    let target_dir = resolve_workspace_directory(root, target_relative)?;
    let path = target_dir.join(clean_new_entry_path(name)?);
    ensure_path_stays_in_root(root, &path)?;
    if path.exists() {
        bail!("folder already exists");
    }

    fs::create_dir_all(&path)
        .with_context(|| format!("unable to create directory {}", path.display()))?;
    workspace_entry_from_path(root, &path)
}

pub fn rename_workspace_entry(
    root: &Path,
    relative_path: &str,
    new_name: &str,
) -> Result<WorkspaceEntry> {
    let source = resolve_workspace_path(root, relative_path)?;
    if normalize_workspace_relative_path(relative_path)?.is_empty() {
        bail!("cannot rename workspace root");
    }

    let name = clean_child_name(new_name)?;
    let destination = source
        .parent()
        .ok_or_else(|| anyhow!("invalid file path"))?
        .join(name);
    ensure_path_stays_in_root(root, &destination)?;
    if destination.exists() {
        bail!("an item with that name already exists");
    }

    fs::rename(&source, &destination).with_context(|| {
        format!(
            "unable to rename {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    workspace_entry_from_path(root, &destination)
}

pub fn delete_workspace_entry(root: &Path, relative_path: &str) -> Result<()> {
    let normalized = normalize_workspace_relative_path(relative_path)?;
    if normalized.is_empty() {
        bail!("cannot delete workspace root");
    }
    let path = resolve_workspace_path(root, &normalized)?;
    let metadata = fs::symlink_metadata(&path)
        .with_context(|| format!("unable to read metadata {}", path.display()))?;

    if metadata.file_type().is_dir() {
        fs::remove_dir_all(&path)
            .with_context(|| format!("unable to delete folder {}", path.display()))?;
    } else {
        fs::remove_file(&path)
            .with_context(|| format!("unable to delete file {}", path.display()))?;
    }

    Ok(())
}

pub fn trash_workspace_entry(root: &Path, relative_path: &str) -> Result<WorkspaceDeletedEntry> {
    let normalized = normalize_workspace_relative_path(relative_path)?;
    if normalized.is_empty() {
        bail!("cannot delete workspace root");
    }
    let path = resolve_workspace_path(root, &normalized)?;
    let metadata = fs::symlink_metadata(&path)
        .with_context(|| format!("unable to read metadata {}", path.display()))?;
    let kind = if metadata.file_type().is_dir() {
        WorkspaceEntryKind::Directory
    } else {
        WorkspaceEntryKind::File
    };
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("invalid file path"))?
        .to_string();

    let trash_root = workspace_trash_root();
    fs::create_dir_all(&trash_root)
        .with_context(|| format!("unable to create restore storage {}", trash_root.display()))?;
    let trash_path = unique_trash_path(&trash_root, &normalized);
    move_path(&path, &trash_path)?;

    Ok(WorkspaceDeletedEntry {
        name,
        relative_path: normalized,
        original_absolute_path: path.display().to_string(),
        trash_path: trash_path.display().to_string(),
        kind,
    })
}

pub fn restore_workspace_deleted_entries(
    root: &Path,
    entries: &[WorkspaceDeletedEntry],
) -> Result<Vec<WorkspaceEntry>> {
    let mut restored = Vec::new();

    for entry in entries {
        let normalized = normalize_workspace_relative_path(&entry.relative_path)?;
        if normalized.is_empty() {
            bail!("cannot restore workspace root");
        }

        let target = root.join(Path::new(&normalized));
        ensure_path_stays_in_root(root, &target)?;
        if target.exists() {
            bail!("an item already exists at {}", entry.relative_path);
        }

        let trash_path = PathBuf::from(&entry.trash_path);
        ensure_known_trash_path(&trash_path)?;
        if !trash_path.exists() {
            bail!("deleted item is no longer available to restore");
        }

        let parent = target
            .parent()
            .ok_or_else(|| anyhow!("invalid restore path"))?;
        fs::create_dir_all(parent)
            .with_context(|| format!("unable to create directory {}", parent.display()))?;
        move_path(&trash_path, &target)?;
        restored.push(workspace_entry_from_path(root, &target)?);
    }

    Ok(restored)
}

#[derive(Debug, Clone, Copy)]
pub enum WorkspaceCopyOperation {
    Copy,
    Move,
}

pub fn copy_workspace_entries(
    root: &Path,
    target_relative: Option<&str>,
    sources: &[String],
    operation: WorkspaceCopyOperation,
) -> Result<Vec<WorkspaceEntry>> {
    let target_dir = resolve_workspace_directory(root, target_relative)?;
    let mut copied = Vec::new();

    for relative_path in sources {
        let source = resolve_workspace_path(root, relative_path)?;
        if normalize_workspace_relative_path(relative_path)?.is_empty() {
            bail!("cannot copy workspace root");
        }
        let name = source
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or_else(|| anyhow!("source has no file name: {}", source.display()))?;

        if source.is_dir() && target_dir.starts_with(&source) {
            bail!("cannot paste a folder into itself");
        }

        if matches!(operation, WorkspaceCopyOperation::Move)
            && source.parent() == Some(target_dir.as_path())
        {
            copied.push(workspace_entry_from_path(root, &source)?);
            continue;
        }

        let destination = unique_destination(&target_dir, name);
        if matches!(operation, WorkspaceCopyOperation::Move) {
            if let Err(rename_err) = fs::rename(&source, &destination) {
                copy_path(&source, &destination).with_context(|| {
                    format!(
                        "unable to move {} to {} after rename failed: {}",
                        source.display(),
                        destination.display(),
                        rename_err
                    )
                })?;
                remove_path(&source)?;
            }
        } else {
            copy_path(&source, &destination)?;
        }

        copied.push(workspace_entry_from_path(root, &destination)?);
    }

    Ok(copied)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedEntry {
    pub source_path: String,
    pub relative_path: String,
}

pub fn import_workspace_paths(
    root: &Path,
    target_relative: Option<&str>,
    sources: &[String],
) -> Result<Vec<ImportedEntry>> {
    let target_dir = match target_relative.filter(|value| !value.is_empty()) {
        Some(value) => {
            let resolved = resolve_workspace_path(root, value)?;
            if !resolved.is_dir() {
                bail!("target is not a directory");
            }
            resolved
        }
        None => root.to_path_buf(),
    };

    let mut imported = Vec::new();
    for source in sources {
        let src_path = Path::new(source);
        if !src_path.exists() {
            continue;
        }
        let canonical_src = src_path
            .canonicalize()
            .with_context(|| format!("unable to resolve source {}", src_path.display()))?;
        let name = canonical_src
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or_else(|| anyhow!("source has no file name: {}", canonical_src.display()))?;
        if canonical_src.is_dir() && target_dir.starts_with(&canonical_src) {
            bail!("cannot copy a folder into itself");
        }
        let destination = unique_destination(&target_dir, name);
        copy_path(&canonical_src, &destination)?;
        imported.push(ImportedEntry {
            source_path: canonical_src.display().to_string(),
            relative_path: relative_from_root(root, &destination)?,
        });
    }

    Ok(imported)
}

fn unique_destination(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), format!(".{ext}")),
        _ => (name.to_string(), String::new()),
    };
    for n in 1..10_000 {
        let attempt = dir.join(format!("{stem} ({n}){ext}"));
        if !attempt.exists() {
            return attempt;
        }
    }
    dir.join(format!("{stem}-conflict{ext}"))
}

fn copy_path(src: &Path, dst: &Path) -> Result<()> {
    if src.is_dir() {
        copy_directory(src, dst)
    } else {
        fs::copy(src, dst)
            .with_context(|| format!("unable to copy {} to {}", src.display(), dst.display()))?;
        Ok(())
    }
}

fn move_path(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("unable to create directory {}", parent.display()))?;
    }

    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(rename_err) => {
            copy_path(src, dst).with_context(|| {
                format!(
                    "unable to copy {} to {} after rename failed: {}",
                    src.display(),
                    dst.display(),
                    rename_err
                )
            })?;
            remove_path(src)
        }
    }
}

fn remove_path(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("unable to read metadata {}", path.display()))?;
    if metadata.file_type().is_dir() {
        fs::remove_dir_all(path)
            .with_context(|| format!("unable to delete folder {}", path.display()))
    } else {
        fs::remove_file(path).with_context(|| format!("unable to delete file {}", path.display()))
    }
}

fn copy_directory(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)
        .with_context(|| format!("unable to create directory {}", dst.display()))?;
    for entry in
        fs::read_dir(src).with_context(|| format!("unable to list directory {}", src.display()))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_directory(&from, &to)?;
        } else if file_type.is_file() {
            fs::copy(&from, &to).with_context(|| {
                format!("unable to copy {} to {}", from.display(), to.display())
            })?;
        }
    }
    Ok(())
}

fn workspace_trash_root() -> PathBuf {
    std::env::temp_dir().join("claakecode-deleted-workspace-entries")
}

fn unique_trash_path(trash_root: &Path, relative_path: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let stem = safe_trash_stem(relative_path);
    for attempt in 0..10_000 {
        let suffix = if attempt == 0 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let candidate = trash_root.join(format!("{}-{nanos}-{stem}{suffix}", std::process::id()));
        if !candidate.exists() {
            return candidate;
        }
    }
    trash_root.join(format!("{}-{nanos}-entry", std::process::id()))
}

fn safe_trash_stem(value: &str) -> String {
    let mut stem = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    if stem.len() > 96 {
        stem.truncate(96);
    }
    if stem.is_empty() {
        "entry".to_string()
    } else {
        stem
    }
}

fn ensure_known_trash_path(path: &Path) -> Result<()> {
    let trash_root = workspace_trash_root();
    let canonical_root = trash_root
        .canonicalize()
        .with_context(|| format!("unable to resolve restore storage {}", trash_root.display()))?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("invalid restore storage path"))?;
    let canonical_parent = parent
        .canonicalize()
        .with_context(|| format!("unable to resolve restore path {}", parent.display()))?;
    if canonical_parent.starts_with(canonical_root) {
        Ok(())
    } else {
        bail!("restore path is outside restore storage")
    }
}

pub fn resolve_workspace_path(root: &Path, relative_path: &str) -> Result<PathBuf> {
    let cleaned = clean_relative_path(relative_path)?;
    let joined = root.join(cleaned);

    if joined.exists() {
        let canonical = joined
            .canonicalize()
            .with_context(|| format!("unable to resolve path {}", joined.display()))?;
        ensure_within_root(root, &canonical)?;
        Ok(canonical)
    } else {
        let parent = joined.parent().ok_or_else(|| anyhow!("invalid path"))?;
        let canonical_parent = parent
            .canonicalize()
            .with_context(|| format!("unable to resolve path {}", parent.display()))?;
        ensure_within_root(root, &canonical_parent)?;
        Ok(joined)
    }
}

pub fn normalize_workspace_relative_path(relative_path: &str) -> Result<String> {
    let cleaned = clean_relative_path(relative_path)?;
    Ok(cleaned
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/"))
}

fn resolve_workspace_directory(root: &Path, relative_path: Option<&str>) -> Result<PathBuf> {
    let directory = match relative_path.filter(|value| !value.is_empty()) {
        Some(value) => resolve_workspace_path(root, value)?,
        None => root.to_path_buf(),
    };
    if !directory.is_dir() {
        bail!("target is not a directory");
    }
    Ok(directory)
}

fn clean_new_entry_path(raw: &str) -> Result<PathBuf> {
    let cleaned = clean_relative_path(raw.trim())?;
    if cleaned.as_os_str().is_empty() {
        bail!("name cannot be empty");
    }
    Ok(cleaned)
}

fn clean_child_name(raw: &str) -> Result<PathBuf> {
    let cleaned = clean_new_entry_path(raw)?;
    if cleaned.components().count() != 1 {
        bail!("name cannot contain path separators");
    }
    Ok(cleaned)
}

fn initial_workspace_file_contents(path: &Path) -> &'static [u8] {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return b"";
    };

    if name.eq_ignore_ascii_case(DESIGN_SYSTEM_FILE_NAME) {
        DESIGN_SYSTEM_TEMPLATE.as_bytes()
    } else if name.eq_ignore_ascii_case(CLAUDE_FILE_NAME) {
        CLAUDE_FILE_TEMPLATE.as_bytes()
    } else {
        b""
    }
}

fn ensure_path_stays_in_root(root: &Path, path: &Path) -> Result<()> {
    if path.starts_with(root) {
        Ok(())
    } else {
        bail!("path escapes workspace")
    }
}

fn workspace_entry_from_path(root: &Path, path: &Path) -> Result<WorkspaceEntry> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("unable to read file metadata {}", path.display()))?;
    let kind = if metadata.is_dir() {
        WorkspaceEntryKind::Directory
    } else {
        WorkspaceEntryKind::File
    };
    Ok(WorkspaceEntry {
        name: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string(),
        relative_path: relative_from_root(root, path)?,
        absolute_path: path.display().to_string(),
        kind,
        has_children: matches!(kind, WorkspaceEntryKind::Directory) && directory_has_children(path),
    })
}

fn ensure_within_root(root: &Path, path: &Path) -> Result<()> {
    if path.starts_with(root) {
        Ok(())
    } else {
        bail!("path escapes workspace")
    }
}

fn clean_relative_path(raw: &str) -> Result<PathBuf> {
    let candidate = Path::new(raw);
    let mut cleaned = PathBuf::new();

    for component in candidate.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => cleaned.push(value),
            Component::ParentDir => bail!("parent segments are not allowed"),
            Component::RootDir | Component::Prefix(_) => {
                bail!("absolute paths are not allowed here")
            }
        }
    }

    Ok(cleaned)
}

fn read_document(root: &Path, path: &Path) -> Result<FileDocument> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("unable to read file metadata {}", path.display()))?;
    if !metadata.is_file() {
        bail!("path is not a file");
    }

    let relative_path = relative_from_root(root, path)?;
    let size = metadata.len();
    let last_modified_ms = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_millis() as i64);

    let bytes =
        fs::read(path).with_context(|| format!("unable to read file {}", path.display()))?;
    let image_media_type = preview_image_media_type(path).map(str::to_string);
    let image_data = if image_media_type.is_some() && size <= MAX_PREVIEW_IMAGE_BYTES {
        Some(BASE64_STANDARD.encode(&bytes))
    } else {
        None
    };
    let (editable, content, reason) = if size > MAX_EDITABLE_BYTES {
        (
            false,
            None,
            Some("File too large to edit in this view.".to_string()),
        )
    } else if let Some(decoded) = decode_text(&bytes) {
        (true, Some(decoded.content), None)
    } else {
        (
            false,
            None,
            Some("Binary file is not editable yet.".to_string()),
        )
    };

    Ok(FileDocument {
        name: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string(),
        relative_path,
        absolute_path: path.display().to_string(),
        editable,
        content,
        reason,
        size,
        last_modified_ms,
        image_media_type,
        image_data,
    })
}

fn preview_image_media_type(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => Some("image/png"),
        Some("jpg" | "jpeg") => Some("image/jpeg"),
        Some("gif") => Some("image/gif"),
        Some("webp") => Some("image/webp"),
        Some("svg") => Some("image/svg+xml"),
        Some("bmp") => Some("image/bmp"),
        Some("avif") => Some("image/avif"),
        Some("heic") => Some("image/heic"),
        Some("heif") => Some("image/heif"),
        _ => None,
    }
}

fn relative_from_root(root: &Path, path: &Path) -> Result<String> {
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

fn directory_has_children(path: &Path) -> bool {
    fs::read_dir(path)
        .ok()
        .and_then(|mut items| items.next())
        .is_some()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn search_workspace_files_returns_grouped_line_matches() {
        let root = unique_temp_dir();
        fs::create_dir_all(root.join("src")).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(
            root.join("src").join("alpha.ts"),
            "const title = 'Alpha';\nconst hidden = 'Beta';\n",
        )
        .expect("write file");
        fs::write(root.join("notes.md"), "Nothing here.\n").expect("write file");

        let result = search_workspace_files(&root, "alpha").expect("search should succeed");

        assert_eq!(result.total_matches, 1);
        assert_eq!(result.files[0].relative_path, "src/alpha.ts");
        assert_eq!(result.files[0].match_count, 1);
        assert_eq!(result.files[0].matches[0].line_number, 1);

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn search_workspace_files_supports_fuzzy_path_matches() {
        let root = unique_temp_dir();
        fs::create_dir_all(root.join("src").join("components")).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(
            root.join("src").join("components").join("SearchPane.tsx"),
            "export const value = 1;\n",
        )
        .expect("write file");

        let result = search_workspace_files(&root, "srchpn").expect("search should succeed");

        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].path_match);
        assert_eq!(result.files[0].match_count, 0);

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn create_workspace_file_seeds_design_doc() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");

        let entry = create_workspace_file(&root, None, "DESIGN.md").expect("create design file");
        let contents = fs::read_to_string(root.join(entry.relative_path)).expect("read file");

        assert_eq!(contents, DESIGN_SYSTEM_TEMPLATE);

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn create_workspace_file_seeds_claude_doc() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");

        let entry = create_workspace_file(&root, None, "CLAUDE.md").expect("create claude file");
        let contents = fs::read_to_string(root.join(entry.relative_path)).expect("read file");

        assert_eq!(contents, CLAUDE_FILE_TEMPLATE);

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn trash_workspace_entry_can_restore_file() {
        let root = unique_temp_dir();
        fs::create_dir_all(root.join("src")).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(root.join("src").join("note.txt"), "hello").expect("write file");

        let deleted = trash_workspace_entry(&root, "src/note.txt").expect("trash workspace entry");
        assert_eq!(deleted.relative_path, "src/note.txt");
        assert!(!root.join("src").join("note.txt").exists());

        let restored =
            restore_workspace_deleted_entries(&root, &[deleted]).expect("restore workspace entry");

        assert_eq!(restored[0].relative_path, "src/note.txt");
        assert_eq!(
            fs::read_to_string(root.join("src").join("note.txt")).expect("read restored file"),
            "hello",
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn restore_workspace_entry_refuses_to_overwrite_existing_item() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(root.join("note.txt"), "old").expect("write file");

        let deleted = trash_workspace_entry(&root, "note.txt").expect("trash workspace entry");
        fs::write(root.join("note.txt"), "new").expect("write replacement file");

        let result = restore_workspace_deleted_entries(&root, &[deleted]);

        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(root.join("note.txt")).expect("read replacement file"),
            "new",
        );

        fs::remove_dir_all(root).ok();
    }

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "claakecode-workspace-test-{}-{nanos}",
            std::process::id()
        ))
    }
}
