use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
    thread,
    time::Duration,
};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::indexer::sync_changed_paths;

static ACTIVE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

pub fn start_background_indexing(workspace_root: PathBuf) {
    if !background_indexing_enabled() {
        return;
    }

    let key = workspace_root.display().to_string();
    let active = ACTIVE.get_or_init(|| Mutex::new(HashSet::new()));
    let mut guard = match active.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    if !guard.insert(key) {
        return;
    }
    drop(guard);

    if crate::process::process_isolation_enabled() && !crate::process::helper_child() {
        if spawn_watch_helper(&workspace_root).is_ok() {
            return;
        }
    }

    thread::spawn(move || run_background_indexing_loop(workspace_root, None));
}

pub(crate) fn run_background_indexing_loop(workspace_root: PathBuf, parent_pid: Option<u32>) {
    let (tx, rx) = std::sync::mpsc::channel::<Vec<PathBuf>>();
    let watch_root = workspace_root.clone();
    let mut watcher = match RecommendedWatcher::new(
        move |result: notify::Result<notify::Event>| {
            if let Ok(event) = result {
                if is_indexable_event(&event.kind) && !event.paths.is_empty() {
                    let _ = tx.send(event.paths);
                }
            }
        },
        notify::Config::default(),
    ) {
        Ok(watcher) => watcher,
        Err(_) => return,
    };
    if watcher
        .watch(&watch_root, RecursiveMode::Recursive)
        .is_err()
    {
        return;
    }

    loop {
        if !crate::process::parent_is_alive(parent_pid) {
            break;
        }
        match rx.recv_timeout(Duration::from_secs(3)) {
            Ok(paths) => {
                let mut changed = paths;
                while let Ok(paths) = rx.recv_timeout(Duration::from_millis(400)) {
                    changed.extend(paths);
                }
                let _ = sync_changed_paths(&workspace_root, changed);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn is_indexable_event(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) | EventKind::Any
    )
}

fn background_indexing_enabled() -> bool {
    !matches!(
        std::env::var("SINEW_INDEX_BACKGROUND")
            .unwrap_or_else(|_| "1".to_string())
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "0" | "false" | "off" | "no"
    )
}

fn spawn_watch_helper(workspace_root: &Path) -> std::io::Result<()> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("--sinew-helper")
        .arg("codebase-index-watch")
        .arg(workspace_root)
        .arg(std::process::id().to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .env("SINEW_INDEX_HELPER_CHILD", "1")
        .env_remove("SINEW_INDEX_EMBEDDINGS");
    hide_helper_window(&mut command);
    command.spawn().map(|_| ())
}

#[cfg(windows)]
fn hide_helper_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x08000000);
}

#[cfg(not(windows))]
fn hide_helper_window(_command: &mut Command) {}

pub fn warm_workspace_index(workspace_root: &Path) {
    start_background_indexing(workspace_root.to_path_buf());
}
