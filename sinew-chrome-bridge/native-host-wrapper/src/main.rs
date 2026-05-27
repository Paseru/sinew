use std::env;
use std::path::PathBuf;
use std::process::{exit, Command, Stdio};

fn find_node_path() -> String {
    if let Ok(path) = env::var("SINEW_NODE_PATH") {
        if !path.trim().is_empty() {
            return path;
        }
    }

    let candidates = [
        env::var("ProgramFiles")
            .ok()
            .map(|root| format!("{}\\nodejs\\node.exe", root)),
        env::var("ProgramFiles(x86)")
            .ok()
            .map(|root| format!("{}\\nodejs\\node.exe", root)),
        env::var("LOCALAPPDATA")
            .ok()
            .map(|root| format!("{}\\Programs\\nodejs\\node.exe", root)),
    ];

    for candidate in candidates.into_iter().flatten() {
        if PathBuf::from(&candidate).exists() {
            return candidate;
        }
    }

    String::from("node")
}

fn main() {
    let exe_dir = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let server_path = exe_dir.join("server.js");
    let node_path = find_node_path();

    let mut child = match Command::new(node_path)
        .arg(server_path)
        .arg("--native")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            eprintln!("Sinew Chrome native host failed to start server.js: {err}");
            exit(1);
        }
    };

    let status = match child.wait() {
        Ok(status) => status,
        Err(err) => {
            eprintln!("Sinew Chrome native host failed while waiting for server.js: {err}");
            exit(1);
        }
    };
    exit(status.code().unwrap_or(0));
}
