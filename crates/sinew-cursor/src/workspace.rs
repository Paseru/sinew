use std::{
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::{json, Value};
use sinew_index::IndexStats;

const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".turbo",
    "__pycache__",
];

pub struct WorkspaceSnapshot {
    pub uri: String,
    pub name: String,
    pub branch: Option<String>,
    pub git_status: Option<String>,
    pub project_layout: Value,
}

pub fn snapshot(workspace_root: &str) -> Option<WorkspaceSnapshot> {
    let path = PathBuf::from(workspace_root);
    if !path.is_dir() {
        return None;
    }
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("workspace")
        .to_string();
    let index_stats = sinew_index::index_stats(&path).unwrap_or_else(|_| IndexStats {
        files_indexed: 0,
        chunks_indexed: 0,
        files_updated: 0,
    });
    let mut project_layout = build_project_layout(&path, 3, 120);
    if let Some(object) = project_layout.as_object_mut() {
        object.insert(
            "localIndex".into(),
            json!({
                "filesIndexed": index_stats.files_indexed,
                "chunksIndexed": index_stats.chunks_indexed,
                "engine": "sinew-fts5+embeddings"
            }),
        );
    }
    Some(WorkspaceSnapshot {
        uri: path_to_file_uri(&path),
        name,
        branch: git_branch(&path),
        git_status: git_status_porcelain(&path),
        project_layout,
    })
}

pub fn path_to_file_uri(path: &Path) -> String {
    let normalized = path.display().to_string().replace('\\', "/");
    if normalized.starts_with("//") {
        return format!("file:{normalized}");
    }
    format!("file:///{normalized}")
}

fn git_branch(root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["-C", &root.display().to_string(), "branch", "--show-current"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!branch.is_empty()).then_some(branch)
}

fn git_status_porcelain(root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["-C", &root.display().to_string(), "status", "--porcelain"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!status.is_empty()).then_some(status)
}

fn build_project_layout(root: &Path, max_depth: usize, max_entries: usize) -> Value {
    let mut directories = Vec::new();
    let mut files = Vec::new();
    let mut remaining = max_entries;
    collect_layout(
        root,
        root,
        max_depth,
        &mut directories,
        &mut files,
        &mut remaining,
    );
    json!({
        "directories": directories,
        "files": files,
        "totalFiles": files.len(),
        "totalSubfolders": directories.len(),
    })
}

fn collect_layout(
    root: &Path,
    current: &Path,
    depth: usize,
    directories: &mut Vec<Value>,
    files: &mut Vec<Value>,
    remaining: &mut usize,
) {
    if depth == 0 || *remaining == 0 {
        return;
    }
    let read_dir = match std::fs::read_dir(current) {
        Ok(read_dir) => read_dir,
        Err(_) => return,
    };
    let mut entries = read_dir
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if *remaining == 0 {
            break;
        }
        let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
        if file_name.is_empty() || should_skip(file_name) {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map(|value| value.display().to_string().replace('\\', "/"))
            .unwrap_or_else(|_| file_name.to_string());
        if path.is_dir() {
            directories.push(json!({ "name": relative }));
            *remaining = remaining.saturating_sub(1);
            collect_layout(root, &path, depth - 1, directories, files, remaining);
        } else {
            files.push(json!({ "name": relative }));
            *remaining = remaining.saturating_sub(1);
        }
    }
}

fn should_skip(name: &str) -> bool {
    SKIP_DIRS.contains(&name) || name.starts_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uri_uses_forward_slashes() {
        let uri = path_to_file_uri(Path::new(r"C:\Dev\sinew"));
        assert!(uri.starts_with("file:///C:/Dev/sinew"));
    }
}
