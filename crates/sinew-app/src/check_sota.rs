use serde_json::{json, Value};
use sinew_core::ToolDescriptor;
use std::path::PathBuf;
use std::process::Command as StdCommand;

use crate::{tool_names, tool_run::ToolRunResult};

#[derive(Debug, Clone, Default)]
pub struct CheckSotaTool;

impl CheckSotaTool {
    pub fn new() -> Self {
        Self
    }

    pub fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: tool_names::CHECK_SOTA.into(),
            description: "Check the status of SOTA (State-of-the-Art) development tools and system dependencies on this machine (e.g. ripgrep/rg, git, python, cargo, node, npm). Use this to verify if everything is installed and working correctly.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        }
    }

    pub async fn run(&self, _input: Value) -> ToolRunResult {
        let mut results = serde_json::Map::new();

        // 1. Ripgrep (rg)
        let rg_status = check_binary("rg");
        results.insert("ripgrep".into(), rg_status);

        // 2. Git
        let git_status = check_binary("git");
        results.insert("git".into(), git_status);

        // 3. Python
        let python_status = check_binary("python");
        results.insert("python".into(), python_status);

        // 3b. Pip (pip / pip3)
        let mut pip_status = check_binary("pip");
        if !pip_status
            .get("available")
            .and_then(|a| a.as_bool())
            .unwrap_or(false)
        {
            let pip3_status = check_binary("pip3");
            if pip3_status
                .get("available")
                .and_then(|a| a.as_bool())
                .unwrap_or(false)
            {
                pip_status = pip3_status;
            }
        }
        results.insert("pip".into(), pip_status);

        // 4. Cargo / Rust
        let cargo_status = check_binary("cargo");
        results.insert("cargo".into(), cargo_status);

        // 4b. Rust Compiler (rustc)
        let rustc_status = check_binary("rustc");
        results.insert("rustc".into(), rustc_status);

        // 5. Node
        let node_status = check_binary("node");
        results.insert("node".into(), node_status);

        // 6. Npm
        let npm_status = check_binary("npm");
        results.insert("npm".into(), npm_status);

        // Compute overall SOTA status
        let mut overall_ok = true;
        for (_, val) in results.iter() {
            if let Some(available) = val.get("available").and_then(|a| a.as_bool()) {
                if !available {
                    overall_ok = false;
                }
            }
        }

        let output = json!({
            "status": if overall_ok { "ok" } else { "warning" },
            "message": if overall_ok {
                "All SOTA development tools are fully installed and configured."
            } else {
                "Some SOTA development tools or dependencies are missing or could not be run."
            },
            "tools": results
        });

        ToolRunResult::ok(
            serde_json::to_string_pretty(&output).unwrap_or_default(),
            Vec::new(),
        )
    }
}

fn check_binary(name: &str) -> Value {
    let path = find_in_path(name);
    let available = path.is_some();

    let mut version = None;
    let mut error = None;

    if let Some(ref p) = path {
        let mut cmd = StdCommand::new(p);
        cmd.arg("--version");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        match cmd.output() {
            Ok(output) => {
                if output.status.success() {
                    version = Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
                } else {
                    let err_msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    error = Some(format!(
                        "Exit status: {}. Error: {}",
                        output.status, err_msg
                    ));
                }
            }
            Err(e) => {
                error = Some(e.to_string());
            }
        }
    } else {
        error = Some("Executable not found in system PATH".to_string());
    }

    json!({
        "available": available,
        "path": path.map(|p| p.display().to_string()),
        "version": version,
        "error": error
    })
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    let mut names = vec![name.to_string()];
    #[cfg(windows)]
    {
        if !name.ends_with(".exe") {
            names.push(format!("{name}.exe"));
            names.push(format!("{name}.cmd"));
            names.push(format!("{name}.bat"));
        }
    }
    std::env::split_paths(&paths)
        .flat_map(|path| names.iter().map(move |n| path.join(n)))
        .find(|path| path.is_file())
}
