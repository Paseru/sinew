use std::path::{Path, PathBuf};
use std::process::Command;
use serde_json::Value;

const READ_LIMIT: usize = 512 * 1024;

/// Run a Sinew tool invoked via Cursor `mcpArgs` (best-effort parity).
pub fn execute_tool(name: &str, args: &Value, workspace_root: &str) -> String {
    let root = PathBuf::from(workspace_root);
    if !root.is_dir() {
        return "Error: workspace root is not a directory".into();
    }
    match normalize_tool_name(name).as_str() {
        "read" => exec_read(&root, args),
        "list_dir" => exec_list_dir(&root, args),
        "grep" => exec_grep(&root, args),
        "glob" => exec_glob(&root, args),
        "bash" => exec_shell(&root, args),
        "write" => exec_write(&root, args),
        "edit" => exec_edit(&root, args),
        "delete" => exec_delete(&root, args),
        _ => format!("Error: unsupported tool '{name}' in Composer bridge"),
    }
}

fn normalize_tool_name(name: &str) -> String {
    match name.trim().to_ascii_lowercase().as_str() {
        "read" | "readfile" | "read_file" => "read".into(),
        "listdir" | "list_dir" | "ls" => "list_dir".into(),
        "grep" | "rg" => "grep".into(),
        "glob" | "glob_file_search" => "glob".into(),
        "bash" | "shell" | "run_terminal_cmd" => "bash".into(),
        "write" | "writefile" | "write_file" => "write".into(),
        "strreplace" | "search_replace" | "edit" | "editfile" | "edit_file" => "edit".into(),
        "delete" | "deletefile" | "delete_file" => "delete".into(),
        other => other.to_string(),
    }
}

fn resolve_path(root: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn pick_string(args: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = args.get(*key).and_then(|v| v.as_str()) {
            if !value.trim().is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn exec_read(root: &Path, args: &Value) -> String {
    let Some(path) = pick_string(args, &["path", "filePath", "file_path", "target_file"]) else {
        return "Error: read requires path".into();
    };
    let full = resolve_path(root, &path);
    if !full.starts_with(root) {
        return "Error: path outside workspace".into();
    }
    match std::fs::read_to_string(&full) {
        Ok(content) => {
            if content.len() > READ_LIMIT {
                format!(
                    "{}\n\n[truncated: {} bytes total]",
                    &content[..READ_LIMIT],
                    content.len()
                )
            } else {
                content
            }
        }
        Err(err) => format!("Error reading {}: {err}", full.display()),
    }
}

fn exec_list_dir(root: &Path, args: &Value) -> String {
    let path = pick_string(args, &["path", "target_directory", "directory"]).unwrap_or_else(|| ".".into());
    let full = resolve_path(root, &path);
    if !full.starts_with(root) {
        return "Error: path outside workspace".into();
    }
    let mut entries = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(&full) else {
        return format!("Error: cannot read directory {}", full.display());
    };
    for entry in read_dir.flatten().take(500) {
        let name = entry.file_name().to_string_lossy().to_string();
        let kind = if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            "dir"
        } else {
            "file"
        };
        entries.push(format!("{kind}\t{name}"));
    }
    entries.sort();
    entries.join("\n")
}

fn exec_grep(root: &Path, args: &Value) -> String {
    let pattern = pick_string(args, &["pattern", "query", "regex"]).unwrap_or_default();
    if pattern.is_empty() {
        return "Error: grep requires pattern".into();
    }
    let path = pick_string(args, &["path", "glob", "target"]).unwrap_or_else(|| ".".into());
    let full = resolve_path(root, &path);
    let mut cmd = Command::new("rg");
    cmd.arg("--line-number")
        .arg("--max-count")
        .arg("200")
        .arg(&pattern)
        .arg(&full);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stdout.trim().is_empty() && !output.status.success() {
                if stderr.trim().is_empty() {
                    "No matches".into()
                } else {
                    stderr.to_string()
                }
            } else {
                stdout.to_string()
            }
        }
        Err(err) => format!("Error: rg failed ({err})"),
    }
}

fn exec_glob(root: &Path, args: &Value) -> String {
    let pattern = pick_string(args, &["glob_pattern", "pattern", "glob"]).unwrap_or_default();
    if pattern.is_empty() {
        return "Error: glob requires pattern".into();
    }
    let mut cmd = Command::new("rg");
    cmd.arg("--files")
        .arg("-g")
        .arg(&pattern)
        .arg(root);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    match cmd.output() {
        Ok(output) => String::from_utf8_lossy(&output.stdout).to_string(),
        Err(err) => format!("Error: glob via rg failed ({err})"),
    }
}

fn exec_shell(root: &Path, args: &Value) -> String {
    let command = pick_string(args, &["command", "cmd"]).unwrap_or_default();
    if command.is_empty() {
        return "Error: shell requires command".into();
    }
    let cwd = pick_string(args, &["working_directory", "cwd", "workdir"])
        .map(|p| resolve_path(root, &p))
        .unwrap_or_else(|| root.to_path_buf());
    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(&command);
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = Command::new("sh");
        c.arg("-lc").arg(&command);
        c
    };
    cmd.current_dir(&cwd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    // Best-effort timeout via thread is heavy; keep simple sync for bridge spike.
    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            format!("exit={}\n{stdout}{stderr}", output.status)
        }
        Err(err) => format!("Error: shell failed ({err})"),
    }
}

fn exec_edit(root: &Path, args: &Value) -> String {
    let Some(path) = pick_string(args, &["path", "filePath", "file_path", "target_file"]) else {
        return "Error: edit requires path".into();
    };
    let old_str = pick_string(args, &["old_string", "oldString"]).unwrap_or_default();
    let new_str = pick_string(args, &["new_string", "newString", "content", "text"])
        .unwrap_or_default();
    if old_str.is_empty() {
        return exec_write(root, args);
    }
    let full = resolve_path(root, &path);
    if !full.starts_with(root) {
        return "Error: path outside workspace".into();
    }
    match std::fs::read_to_string(&full) {
        Ok(prior) => {
            if !prior.contains(&old_str) {
                return format!("Error: old_string not found in {}", full.display());
            }
            let updated = prior.replace(&old_str, &new_str);
            match std::fs::write(&full, &updated) {
                Ok(()) => format!("Edited {}", full.display()),
                Err(err) => format!("Error writing {}: {err}", full.display()),
            }
        }
        Err(err) => format!("Error reading {}: {err}", full.display()),
    }
}

fn exec_write(root: &Path, args: &Value) -> String {
    let Some(path) = pick_string(args, &["path", "filePath", "file_path", "target_file"]) else {
        return "Error: write requires path".into();
    };
    let full = resolve_path(root, &path);
    if !full.starts_with(root) {
        return "Error: path outside workspace".into();
    }
    let content = pick_string(args, &["content", "contents", "text", "new_string", "replacement"])
        .unwrap_or_default();
    let len = content.len();
    if let Some(parent) = full.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&full, &content) {
        Ok(()) => format!("Wrote {len} bytes to {}", full.display()),
        Err(err) => format!("Error writing {}: {err}", full.display()),
    }
}

fn exec_delete(root: &Path, args: &Value) -> String {
    let Some(path) = pick_string(args, &["path", "filePath", "file_path", "target_file"]) else {
        return "Error: delete requires path".into();
    };
    let full = resolve_path(root, &path);
    if !full.starts_with(root) {
        return "Error: path outside workspace".into();
    }
    match std::fs::remove_file(&full) {
        Ok(()) => format!("Deleted {}", full.display()),
        Err(err) => format!("Error deleting {}: {err}", full.display()),
    }
}
