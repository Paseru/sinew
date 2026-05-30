use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn find_executable(name: &str) -> Option<PathBuf> {
    if let Ok(paths) = env::var("PATH") {
        for path in env::split_paths(&paths) {
            let exe_path = path.join(name);
            let exe_path_win = path.join(format!("{}.exe", name));
            if exe_path.is_file() {
                return Some(exe_path);
            }
            if exe_path_win.is_file() {
                return Some(exe_path_win);
            }
        }
    }
    None
}

pub fn handle_args() -> bool {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        return false;
    }

    match args[1].as_str() {
        "--sync" | "sync" => {
            println!("Starting standalone synchronization...");
            if let Err(e) = run_sync_cli() {
                eprintln!("Error during synchronization: {:?}", e);
                std::process::exit(1);
            }
            println!("Synchronization completed successfully!");
            true
        }
        "--register-chrome" | "register-chrome" => {
            println!("Registering Sinew Chrome MCP Server...");
            if let Err(e) = run_register_chrome_cli() {
                eprintln!("Error during registration: {:?}", e);
                std::process::exit(1);
            }
            println!("Registration completed successfully!");
            true
        }
        _ => false,
    }
}

fn run_sync_cli() -> Result<(), anyhow::Error> {
    // 1. Consolidate rules
    println!("1/4 Consolidating rules...");
    crate::rules::consolidate_rules();

    // 2. Locate databases
    let localappdata = env::var("LOCALAPPDATA").map_err(|_| anyhow::anyhow!("LOCALAPPDATA not found"))?;
    let local_db = PathBuf::from(&localappdata)
        .join("hyrak")
        .join("sinew")
        .join("data")
        .join("desktop-state.sqlite3");

    let onedrive = env::var("ONEDRIVE").unwrap_or_else(|_| {
        env::var("USERPROFILE")
            .map(|u| format!("{}\\OneDrive", u))
            .unwrap_or_default()
    });

    if onedrive.is_empty() {
        return Err(anyhow::anyhow!("OneDrive folder path not found"));
    }

    let onedrive_db_dir = PathBuf::from(&onedrive).join("Documents").join("Sinew");
    let onedrive_db = onedrive_db_dir.join("desktop-state.sqlite3");

    // 3. Database & File Sync
    println!("2/4 Merging databases...");
    if local_db.exists() {
        let _ = fs::create_dir_all(&onedrive_db_dir);
        if onedrive_db.exists() {
            if let Err(e) = crate::merge_databases(&onedrive_db, &local_db) {
                eprintln!("Warning: Differential merge failed, copying directly: {:?}", e);
                fs::copy(&local_db, &onedrive_db)?;
            }
        } else {
            fs::copy(&local_db, &onedrive_db)?;
        }

        // Copy auth files
        crate::sync_auth_files(&localappdata, &onedrive_db_dir, true);

        // Sync rules/errors
        let local_learning_dir = PathBuf::from(&localappdata).join("Sinew");
        let errors_local = local_learning_dir.join("errors_raw.json");
        let rules_local = local_learning_dir.join("instructions_consolidated.md");

        let errors_onedrive = onedrive_db_dir.join("errors_raw.json");
        let rules_onedrive = onedrive_db_dir.join("instructions_consolidated.md");

        if errors_local.exists() {
            let _ = fs::copy(&errors_local, &errors_onedrive);
        }
        if rules_local.exists() {
            let _ = fs::copy(&rules_local, &rules_onedrive);
        }
    } else {
        println!("Local database does not exist, skipping database sync.");
    }

    // 4. Git Push current workspace
    println!("3/4 Verifying Git status for the workspace...");
    let status_out = Command::new("git")
        .args(&["status", "--porcelain"])
        .stdout(Stdio::piped())
        .output()?;
    let status_str = String::from_utf8_lossy(&status_out.stdout);

    if !status_str.trim().is_empty() {
        println!("Changes detected. Committing and pushing to git...");
        Command::new("git").args(&["add", "-A"]).status()?;
        Command::new("git").args(&["commit", "-m", "Sauvegarde automatique via Sinew Sync"]).status()?;
        Command::new("git").args(&["push"]).status()?;
        println!("Git changes successfully pushed!");
    } else {
        println!("Workspace is clean. Checking for unpushed commits...");
        let cherry_out = Command::new("git")
            .args(&["cherry", "-v"])
            .stdout(Stdio::piped())
            .output()?;
        let cherry_str = String::from_utf8_lossy(&cherry_out.stdout);
        if !cherry_str.trim().is_empty() {
            println!("Unpushed commits found. Pushing...");
            Command::new("git").args(&["push"]).status()?;
            println!("Commits successfully pushed!");
        } else {
            println!("Everything is up to date.");
        }
    }

    Ok(())
}

fn run_register_chrome_cli() -> Result<(), anyhow::Error> {
    let local_app_data = env::var("LOCALAPPDATA").map_err(|_| anyhow::anyhow!("LOCALAPPDATA not found"))?;
    let db_path = env::var("SINEW_DESKTOP_DB").map(PathBuf::from).unwrap_or_else(|_| {
        PathBuf::from(&local_app_data)
            .join("hyrak")
            .join("sinew")
            .join("data")
            .join("desktop-state.sqlite3")
    });

    if !db_path.exists() {
        return Err(anyhow::anyhow!("Database not found at {:?}", db_path));
    }

    let current_dir = env::current_dir().unwrap_or_default();
    let source_dir = current_dir.join("sinew-chrome-bridge");
    let installed_dir = env::var("SINEW_CHROME_BRIDGE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(&local_app_data)
                .join("Sinew")
                .join("ChromeBridge")
        });

    let script_dir = if installed_dir.join("mcp_server.js").exists() {
        installed_dir
    } else {
        source_dir
    };

    let node_path = env::var("SINEW_NODE_PATH")
        .ok()
        .or_else(|| {
            find_executable("node").map(|p| p.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "C:\\Program Files\\nodejs\\node.exe".to_string());

    let new_server = serde_json::json!({
        "id": "sinew-chrome",
        "name": "Sinew Chrome",
        "command": node_path,
        "args": [script_dir.join("mcp_server.js").to_string_lossy().to_string()],
        "env": [
            {"key": "MCP_BROWSER_CDP_URL", "value": "http://127.0.0.1:29002"},
            {"key": "SINEW_CHROME_BRIDGE_DIR", "value": script_dir.to_string_lossy().to_string()},
        ],
        "cwd": script_dir.to_string_lossy().to_string(),
        "enabled": true,
        "autoLoad": true,
    });

    let conn = rusqlite::Connection::open(&db_path)?;
    let mut stmt = conn.prepare("SELECT value_json FROM app_settings WHERE key = 'mcp_settings';")?;
    let row_opt: Option<String> = stmt.query_row([], |r| r.get(0)).ok();

    let mut settings = if let Some(ref val_str) = row_opt {
        serde_json::from_str::<serde_json::Value>(val_str).unwrap_or_else(|_| serde_json::json!({"servers": []}))
    } else {
        serde_json::json!({"servers": []})
    };

    let servers = settings.get_mut("servers").and_then(|s| s.as_array_mut());
    if let Some(servers_arr) = servers {
        let mut updated = false;
        for s in servers_arr.iter_mut() {
            let id = s.get("id").and_then(|id| id.as_str()).unwrap_or("");
            let name = s.get("name").and_then(|n| n.as_str()).unwrap_or("");
            if id == "sinew-chrome" || id == "browser-use" || name == "Sinew Chrome" {
                *s = new_server.clone();
                updated = true;
                break;
            }
        }
        if !updated {
            servers_arr.push(new_server);
        }
    } else {
        settings = serde_json::json!({
            "servers": [new_server]
        });
    }

    let value_json = serde_json::to_string(&settings)?;
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNISO_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute(
        "INSERT INTO app_settings (key, value_json, updated_at_ms) \
         VALUES ('mcp_settings', ?, ?) \
         ON CONFLICT(key) DO UPDATE SET \
             value_json = excluded.value_json, \
             updated_at_ms = excluded.updated_at_ms;",
        rusqlite::params![value_json, now_ms],
    )?;

    println!("MCP server 'Sinew Chrome' registered at {:?}", script_dir);
    Ok(())
}
