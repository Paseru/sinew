use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use sinew_core::{AppError, Result};
use tokio::process::Command;
use tokio::sync::Mutex;

static PREPARE_LOCK: Mutex<()> = Mutex::const_new(());
static BRIDGE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Directory containing `run-stream.mjs` (set by Sinew at startup or dev tree).
pub fn bridge_directory() -> Option<PathBuf> {
    if let Some(dir) = BRIDGE_DIR.get() {
        return Some(dir.clone());
    }
    resolve_bridge_directory().inspect(|dir| {
        let _ = BRIDGE_DIR.set(dir.clone());
    })
}

pub fn set_bridge_directory(dir: PathBuf) {
    let _ = BRIDGE_DIR.set(dir);
}

fn resolve_bridge_directory() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("SINEW_CURSOR_AGENT_BRIDGE_DIR") {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            let path = PathBuf::from(trimmed);
            if path.join("run-stream.mjs").is_file() {
                return Some(path);
            }
        }
    }
    if let Ok(script) = std::env::var("SINEW_CURSOR_AGENT_BRIDGE") {
        let trimmed = script.trim();
        if !trimmed.is_empty() {
            let path = PathBuf::from(trimmed);
            if let Some(parent) = path.parent() {
                if parent.join("run-stream.mjs").is_file() {
                    return Some(parent.to_path_buf());
                }
            }
        }
    }
    dev_bridge_directory()
}

fn dev_bridge_directory() -> Option<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidate = manifest
        .join("..")
        .join("..")
        .join("scripts")
        .join("agent-bridge");
    if candidate.join("run-stream.mjs").is_file() {
        candidate.canonicalize().ok().or(Some(candidate))
    } else {
        None
    }
}

fn tsx_binary(dir: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        dir.join("node_modules").join(".bin").join("tsx.cmd")
    }
    #[cfg(not(windows))]
    {
        dir.join("node_modules").join(".bin").join("tsx")
    }
}

fn bridge_ready(dir: &Path) -> bool {
    tsx_binary(dir).is_file() && dir.join("vendor").join("agent_pb.ts").is_file()
}

/// Install `agent-bridge` npm deps if missing (no-op when bundled `node_modules` exists).
pub async fn ensure_agent_bridge_ready() -> Result<PathBuf> {
    let dir = bridge_directory().ok_or_else(|| {
        AppError::Provider(
            "agent bridge introuvable (réinstallez Sinew ou définissez SINEW_CURSOR_AGENT_BRIDGE_DIR)".into(),
        )
    })?;
    if bridge_ready(&dir) {
        return Ok(dir);
    }

    let _guard = PREPARE_LOCK.lock().await;
    if bridge_ready(&dir) {
        return Ok(dir);
    }

    tracing::info!(path = %dir.display(), "installation automatique agent-bridge (npm ci)");
    run_npm_ci(&dir).await?;
    if !bridge_ready(&dir) {
        return Err(AppError::Provider(
            "agent bridge: npm ci terminé mais tsx/vendor manquant (Node/npm requis)".into(),
        ));
    }
    Ok(dir)
}

async fn run_npm_ci(dir: &Path) -> Result<()> {
    #[cfg(windows)]
    let npm = "npm.cmd";
    #[cfg(not(windows))]
    let npm = "npm";

    let mut cmd = Command::new(npm);
    cmd.arg("ci")
        .arg("--omit=dev")
        .current_dir(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped());

    #[cfg(windows)]
    {
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }

    let output = cmd
        .output()
        .await
        .map_err(|err| AppError::Provider(format!("agent bridge npm ci: {err}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Provider(format!(
            "agent bridge npm ci failed ({}): {}",
            output.status,
            stderr.trim()
        )));
    }
    Ok(())
}

pub fn run_stream_script(dir: &Path) -> PathBuf {
    dir.join("run-stream.mjs")
}

pub fn tsx_executable(dir: &Path) -> PathBuf {
    tsx_binary(dir)
}
