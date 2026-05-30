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

        // 7. Sinew Browser Extension
        let extension_status = check_sinew_chrome_bridge();
        results.insert("sinew-extension".into(), extension_status);

        // Compute overall SOTA status
        let mut overall_ok = true;
        for (key, val) in results.iter() {
            if key == "sinew-extension" {
                continue;
            }
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

fn check_sinew_chrome_bridge() -> Value {
    let mut available = false;
    let mut path = None;
    let mut version = None;
    let mut error = None;

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = StdCommand::new("reg");
        cmd.args(["query", "HKCU\\Software\\Google\\Chrome\\NativeMessagingHosts\\com.sinew.chrome_bridge", "/ve"]);
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        match cmd.output() {
            Ok(output) => {
                if output.status.success() {
                    let out_str = String::from_utf8_lossy(&output.stdout);
                    if let Some(pos) = out_str.find("REG_SZ") {
                        let p = out_str[pos + 6..].trim().to_string();
                        let p_buf = PathBuf::from(&p);
                        path = Some(p.clone());
                        if p_buf.is_file() {
                            available = true;
                            // Check package.json in the same directory for version
                            let pkg_json_path = p_buf.parent().map(|parent| parent.join("package.json"));
                            if let Some(pkg_path) = pkg_json_path {
                                if pkg_path.is_file() {
                                    if let Ok(content) = std::fs::read_to_string(pkg_path) {
                                        if let Ok(pkg) = serde_json::from_str::<Value>(&content) {
                                            if let Some(v) = pkg.get("version").and_then(|v| v.as_str()) {
                                                version = Some(v.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                            if version.is_none() {
                                version = Some("1.0.0".into());
                            }
                        } else {
                            error = Some("Fichier manifeste de l'extension introuvable".to_string());
                        }
                    } else {
                        error = Some("Impossible de lire le chemin dans la base de registre".to_string());
                    }
                } else {
                    error = Some("Extension Sinew non enregistrée ou non installée".to_string());
                }
            }
            Err(e) => {
                error = Some(e.to_string());
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        let p_buf = PathBuf::from(home).join("Library/Application Support/Google/Chrome/NativeMessagingHosts/com.sinew.chrome_bridge.json");
        if p_buf.is_file() {
            available = true;
            path = Some(p_buf.display().to_string());
            // Try package.json in same directory
            let pkg_json_path = p_buf.parent().map(|parent| parent.join("package.json"));
            if let Some(pkg_path) = pkg_json_path {
                if pkg_path.is_file() {
                    if let Ok(content) = std::fs::read_to_string(pkg_path) {
                        if let Ok(pkg) = serde_json::from_str::<Value>(&content) {
                            if let Some(v) = pkg.get("version").and_then(|v| v.as_str()) {
                                version = Some(v.to_string());
                            }
                        }
                    }
                }
            }
            if version.is_none() {
                version = Some("1.0.0".into());
            }
        } else {
            error = Some("Extension Sinew non configurée ou non installée".to_string());
        }
    }

    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        let p_buf = PathBuf::from(home).join(".config/google-chrome/NativeMessagingHosts/com.sinew.chrome_bridge.json");
        if p_buf.is_file() {
            available = true;
            path = Some(p_buf.display().to_string());
            // Try package.json in same directory
            let pkg_json_path = p_buf.parent().map(|parent| parent.join("package.json"));
            if let Some(pkg_path) = pkg_json_path {
                if pkg_path.is_file() {
                    if let Ok(content) = std::fs::read_to_string(pkg_path) {
                        if let Ok(pkg) = serde_json::from_str::<Value>(&content) {
                            if let Some(v) = pkg.get("version").and_then(|v| v.as_str()) {
                                version = Some(v.to_string());
                            }
                        }
                    }
                }
            }
            if version.is_none() {
                version = Some("1.0.0".into());
            }
        } else {
            error = Some("Extension Sinew non configurée ou non installée".to_string());
        }
    }

    json!({
        "available": available,
        "path": path,
        "version": version,
        "error": error
    })
}

fn check_binary(name: &str) -> Value {
    let path = find_in_path(name);
    let available = path.is_some();

    let mut version = None;
    let mut error = None;

    if let Some(ref p) = path {
        #[cfg(windows)]
        let mut cmd = {
            let is_batch = p.extension().is_some_and(|ext| ext == "cmd" || ext == "bat");
            if is_batch {
                let mut c = StdCommand::new("cmd");
                c.arg("/C").arg(p);
                c
            } else {
                StdCommand::new(p)
            }
        };

        #[cfg(not(windows))]
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
    let mut names = vec![];
    #[cfg(windows)]
    {
        if !name.ends_with(".exe") && !name.ends_with(".cmd") && !name.ends_with(".bat") {
            names.push(format!("{name}.exe"));
            names.push(format!("{name}.cmd"));
            names.push(format!("{name}.bat"));
        }
        names.push(name.to_string());
    }
    #[cfg(not(windows))]
    {
        names.push(name.to_string());
    }
    std::env::split_paths(&paths)
        .flat_map(|path| names.iter().map(move |n| path.join(n)))
        .find(|path| path.is_file())
}
