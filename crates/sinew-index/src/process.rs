use std::{
    env,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::{
    background, ensure_workspace_index, index_stats, search_workspace, CodebaseHit, IndexStats,
};

const HELPER_ARG: &str = "--sinew-helper";
const REQUEST_HELPER: &str = "codebase-index";
const WATCH_HELPER: &str = "codebase-index-watch";
const CHILD_ENV: &str = "SINEW_INDEX_HELPER_CHILD";
const EMBEDDINGS_ENV: &str = "SINEW_INDEX_EMBEDDINGS";
const ISOLATION_ENV: &str = "SINEW_INDEX_PROCESS_ISOLATION";

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
enum HelperRequest {
    Ensure {
        workspace_root: String,
    },
    IndexAndSearch {
        workspace_root: String,
        query: String,
        path_prefix: Option<String>,
        limit: usize,
    },
    Stats {
        workspace_root: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct HelperResponse {
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stats: Option<IndexStats>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hits: Option<Vec<CodebaseHit>>,
}

impl HelperResponse {
    fn ok(stats: Option<IndexStats>, hits: Option<Vec<CodebaseHit>>) -> Self {
        Self {
            ok: true,
            error: None,
            stats,
            hits,
        }
    }

    fn err(error: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(error.into()),
            stats: None,
            hits: None,
        }
    }
}

pub fn run_helper_if_requested() -> bool {
    let mut args = env::args_os();
    let _exe = args.next();
    let Some(flag) = args.next() else {
        return false;
    };
    if flag != HELPER_ARG {
        return false;
    }

    env::set_var(CHILD_ENV, "1");

    let code = match args.next().and_then(|value| value.into_string().ok()) {
        Some(kind) if kind == REQUEST_HELPER => {
            env::set_var(EMBEDDINGS_ENV, "1");
            run_request_helper()
        }
        Some(kind) if kind == WATCH_HELPER => {
            env::remove_var(EMBEDDINGS_ENV);
            run_watch_helper(args.collect())
        }
        Some(other) => {
            eprintln!("unknown Sinew helper: {other}");
            2
        }
        None => {
            eprintln!("missing Sinew helper name");
            2
        }
    };
    std::process::exit(code);
}

pub fn process_isolation_enabled() -> bool {
    !matches!(
        env::var(ISOLATION_ENV)
            .unwrap_or_else(|_| "1".to_string())
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "0" | "false" | "off" | "no"
    )
}

pub fn helper_child() -> bool {
    env::var_os(CHILD_ENV).is_some()
}

pub fn ensure_workspace_index_isolated(workspace_root: &Path) -> Result<IndexStats> {
    if should_use_process_helper() {
        if let Ok(response) = request_helper(&HelperRequest::Ensure {
            workspace_root: workspace_root.display().to_string(),
        }) {
            return response_to_stats(response);
        }
    }
    ensure_workspace_index(workspace_root)
}

pub fn index_stats_isolated(workspace_root: &Path) -> Result<IndexStats> {
    index_stats(workspace_root)
}

pub fn index_and_search_workspace_isolated(
    workspace_root: &Path,
    query: &str,
    path_prefix: Option<&str>,
    limit: usize,
) -> Result<(IndexStats, Vec<CodebaseHit>)> {
    if should_use_process_helper() {
        if let Ok(response) = request_helper(&HelperRequest::IndexAndSearch {
            workspace_root: workspace_root.display().to_string(),
            query: query.to_string(),
            path_prefix: path_prefix.map(str::to_string),
            limit,
        }) {
            return response_to_search(response);
        }
    }

    let stats = ensure_workspace_index(workspace_root)?;
    let hits = search_workspace(workspace_root, query, path_prefix, limit)?;
    Ok((stats, hits))
}

pub(crate) fn parent_is_alive(parent_pid: Option<u32>) -> bool {
    match parent_pid {
        Some(pid) => process_is_alive(pid),
        None => true,
    }
}

fn should_use_process_helper() -> bool {
    process_isolation_enabled() && !helper_child()
}

fn request_helper(request: &HelperRequest) -> Result<HelperResponse> {
    let enable_embeddings = matches!(request, HelperRequest::IndexAndSearch { .. });
    let mut command = helper_command(enable_embeddings)?;
    command
        .arg(REQUEST_HELPER)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .context("unable to spawn Sinew codebase index helper")?;
    {
        let mut stdin = child
            .stdin
            .take()
            .context("Sinew codebase index helper stdin unavailable")?;
        let payload = serde_json::to_vec(request)?;
        stdin.write_all(&payload)?;
    }

    let output = child.wait_with_output()?;
    if output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        bail!("Sinew codebase index helper returned no output: {stderr}");
    }
    let response: HelperResponse = serde_json::from_slice(&output.stdout)
        .context("invalid Sinew codebase index helper response")?;
    if !output.status.success() && !response.ok {
        bail!(
            "Sinew codebase index helper failed: {}",
            response.error.unwrap_or_else(|| "unknown error".into())
        );
    }
    Ok(response)
}

fn helper_command(enable_embeddings: bool) -> Result<Command> {
    let exe = env::current_exe().context("unable to resolve current Sinew executable")?;
    let mut command = Command::new(exe);
    command.arg(HELPER_ARG).env(CHILD_ENV, "1");
    if enable_embeddings {
        command.env(EMBEDDINGS_ENV, "1");
    } else {
        command.env_remove(EMBEDDINGS_ENV);
    }
    hide_helper_window(&mut command);
    Ok(command)
}

fn run_request_helper() -> i32 {
    let mut input = String::new();
    if let Err(err) = std::io::stdin().read_to_string(&mut input) {
        return write_response(HelperResponse::err(format!(
            "unable to read helper request: {err}"
        )));
    }
    let request: HelperRequest = match serde_json::from_str(&input) {
        Ok(request) => request,
        Err(err) => {
            return write_response(HelperResponse::err(format!(
                "invalid helper request: {err}"
            )))
        }
    };
    let response = match handle_request(request) {
        Ok(response) => response,
        Err(err) => HelperResponse::err(err.to_string()),
    };
    write_response(response)
}

fn run_watch_helper(args: Vec<std::ffi::OsString>) -> i32 {
    let Some(workspace_root) = args.first().map(PathBuf::from) else {
        eprintln!("missing workspace root for codebase index helper");
        return 2;
    };
    let parent_pid = args
        .get(1)
        .and_then(|value| value.to_str())
        .and_then(|value| value.parse::<u32>().ok());
    background::run_background_indexing_loop(workspace_root, parent_pid);
    0
}

fn handle_request(request: HelperRequest) -> Result<HelperResponse> {
    match request {
        HelperRequest::Ensure { workspace_root } => {
            let stats = ensure_workspace_index(Path::new(&workspace_root))?;
            Ok(HelperResponse::ok(Some(stats), None))
        }
        HelperRequest::IndexAndSearch {
            workspace_root,
            query,
            path_prefix,
            limit,
        } => {
            let root = Path::new(&workspace_root);
            let stats = ensure_workspace_index(root)?;
            let hits = search_workspace(root, &query, path_prefix.as_deref(), limit)?;
            Ok(HelperResponse::ok(Some(stats), Some(hits)))
        }
        HelperRequest::Stats { workspace_root } => {
            let stats = index_stats(Path::new(&workspace_root))?;
            Ok(HelperResponse::ok(Some(stats), None))
        }
    }
}

fn response_to_stats(response: HelperResponse) -> Result<IndexStats> {
    if !response.ok {
        bail!(response
            .error
            .unwrap_or_else(|| "index helper failed".into()));
    }
    response.stats.context("index helper returned no stats")
}

fn response_to_search(response: HelperResponse) -> Result<(IndexStats, Vec<CodebaseHit>)> {
    if !response.ok {
        bail!(response
            .error
            .unwrap_or_else(|| "index helper failed".into()));
    }
    let stats = response.stats.context("index helper returned no stats")?;
    Ok((stats, response.hits.unwrap_or_default()))
}

fn write_response(response: HelperResponse) -> i32 {
    let is_ok = response.ok;
    match serde_json::to_vec(&response) {
        Ok(bytes) => {
            let _ = std::io::stdout().write_all(&bytes);
            let _ = std::io::stdout().write_all(b"\n");
        }
        Err(err) => {
            let _ = writeln!(std::io::stderr(), "unable to write helper response: {err}");
            return 1;
        }
    }
    if is_ok {
        0
    } else {
        1
    }
}

#[cfg(windows)]
fn hide_helper_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x08000000);
}

#[cfg(not(windows))]
fn hide_helper_window(_command: &mut Command) {}

#[cfg(windows)]
fn process_is_alive(pid: u32) -> bool {
    use std::ffi::c_void;

    type Handle = *mut c_void;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x0000_1000;
    const WAIT_TIMEOUT: u32 = 0x0000_0102;

    extern "system" {
        fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> Handle;
        fn WaitForSingleObject(hHandle: Handle, dwMilliseconds: u32) -> u32;
        fn CloseHandle(hObject: Handle) -> i32;
    }

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            return false;
        }
        let status = WaitForSingleObject(handle, 0);
        let _ = CloseHandle(handle);
        status == WAIT_TIMEOUT
    }
}

#[cfg(all(unix, not(windows)))]
fn process_is_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(not(any(windows, unix)))]
fn process_is_alive(_pid: u32) -> bool {
    true
}
