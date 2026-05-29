use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::Child,
    sync::{Mutex, OnceLock},
    thread,
    time::Duration,
};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::indexer::ensure_workspace_index;

static ACTIVE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static HELPERS: OnceLock<Mutex<Vec<Child>>> = OnceLock::new();

pub fn start_background_indexing(workspace_root: PathBuf) {
    let key = workspace_root.display().to_string();
    let active = ACTIVE.get_or_init(|| Mutex::new(HashSet::new()));
    let mut guard = match active.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    if !guard.insert(key.clone()) {
        return;
    }
    drop(guard);

    if crate::process::process_isolation_enabled() && !crate::process::helper_child() {
        match crate::process::spawn_background_indexer(&workspace_root) {
            Ok(child) => {
                HELPERS
                    .get_or_init(|| Mutex::new(Vec::new()))
                    .lock()
                    .map(|mut helpers| helpers.push(child))
                    .ok();
                return;
            }
            Err(err) => {
                tracing::warn!(error = %err, "unable to start isolated codebase index helper; falling back in-process");
            }
        }
    }

    thread::spawn(move || run_background_indexing_loop(workspace_root, None));
}

pub(crate) fn run_background_indexing_loop(workspace_root: PathBuf, parent_pid: Option<u32>) {
    let _ = ensure_workspace_index(&workspace_root);
    let (tx, rx) = std::sync::mpsc::channel();
    let watch_root = workspace_root.clone();
    let mut watcher = match RecommendedWatcher::new(
        move |result: notify::Result<notify::Event>| {
            if let Ok(event) = result {
                if is_indexable_event(&event.kind) {
                    let _ = tx.send(());
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
            Ok(()) => {
                while rx.recv_timeout(Duration::from_millis(400)).is_ok() {}
                let _ = ensure_workspace_index(&workspace_root);
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

pub fn warm_workspace_index(workspace_root: &Path) {
    start_background_indexing(workspace_root.to_path_buf());
}
