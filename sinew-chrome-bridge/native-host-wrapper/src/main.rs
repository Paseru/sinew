use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn main() {
    let exe_dir = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let server_path = exe_dir.join("server.js");
    let node_path = env::var("SINEW_NODE_PATH")
        .unwrap_or_else(|_| String::from("C:\\Program Files\\nodejs\\node.exe"));

    let mut child = Command::new(node_path)
        .arg(server_path)
        .arg("--native")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to start Sinew Chrome native host");

    let status = child
        .wait()
        .expect("Failed to wait on Sinew Chrome native host");
    std::process::exit(status.code().unwrap_or(0));
}
