use crate::*;

#[tauri::command]
pub(super) async fn run_terminal_command(
    app: AppHandle,
    input: TerminalCommandInput,
) -> std::result::Result<TerminalCommandOutput, String> {
    let command = input.command.trim();
    if command.is_empty() {
        return Err("command cannot be empty".into());
    }

    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let result = BashTool::new(workspace_root.clone())
        .run(json!({
            "command": command,
            "timeout_secs": 120,
        }))
        .await;

    for change in &result.file_changes {
        emit_workspace_file_change(&app, &workspace_root, &change.relative_path);
    }

    Ok(TerminalCommandOutput {
        content: result.content,
        is_error: result.is_error,
    })
}

#[tauri::command]
pub(super) async fn spawn_terminal(
    app: AppHandle,
    state: State<'_, DesktopState>,
    input: TerminalSpawnInput,
) -> std::result::Result<TerminalSpawnOutput, String> {
    let session_id = validate_terminal_value(&input.session_id, "session id")?.to_string();
    let token = validate_terminal_value(&input.token, "session token")?.to_string();
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(terminal_size(
            input.cols,
            input.rows,
            input.pixel_width,
            input.pixel_height,
        ))
        .map_err(error_to_string)?;

    let mut command = default_terminal_command();
    command.cwd(workspace_root.as_os_str());
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    command.env("CLAAKECODE_WORKSPACE", workspace_root.as_os_str());

    let child = pair.slave.spawn_command(command).map_err(error_to_string)?;
    drop(pair.slave);

    let reader = pair.master.try_clone_reader().map_err(error_to_string)?;
    let writer = Arc::new(StdMutex::new(
        pair.master.take_writer().map_err(error_to_string)?,
    ));
    let killer = Arc::new(StdMutex::new(child.clone_killer()));

    if let Some(previous) = state.terminal_sessions.lock().await.remove(&session_id) {
        terminate_terminal_process(previous);
    }

    state.terminal_sessions.lock().await.insert(
        session_id.clone(),
        TerminalProcess {
            token: token.clone(),
            master: pair.master,
            writer,
            killer,
        },
    );

    spawn_terminal_reader(app.clone(), session_id.clone(), token.clone(), reader);
    spawn_terminal_waiter(
        app,
        state.terminal_sessions.clone(),
        session_id.clone(),
        token,
        child,
    );

    Ok(TerminalSpawnOutput { session_id })
}

fn default_terminal_command() -> CommandBuilder {
    #[cfg(windows)]
    {
        let mut command = CommandBuilder::new("powershell.exe");
        command.arg("-NoLogo");
        command.arg("-NoProfile");
        command.arg("-ExecutionPolicy");
        command.arg("Bypass");
        command
    }
    #[cfg(not(windows))]
    {
        CommandBuilder::new_default_prog()
    }
}

#[tauri::command]
pub(super) async fn write_terminal(
    state: State<'_, DesktopState>,
    input: TerminalWriteInput,
) -> std::result::Result<(), String> {
    let writer = {
        let sessions = state.terminal_sessions.lock().await;
        let Some(process) = sessions.get(&input.session_id) else {
            return Ok(());
        };
        if process.token != input.token {
            return Ok(());
        }
        process.writer.clone()
    };

    let mut writer = writer
        .lock()
        .map_err(|_| "terminal writer unavailable".to_string())?;
    writer
        .write_all(input.data.as_bytes())
        .map_err(error_to_string)?;
    writer.flush().map_err(error_to_string)?;
    Ok(())
}

#[tauri::command]
pub(super) async fn resize_terminal(
    state: State<'_, DesktopState>,
    input: TerminalResizeInput,
) -> std::result::Result<(), String> {
    let sessions = state.terminal_sessions.lock().await;
    let Some(process) = sessions.get(&input.session_id) else {
        return Ok(());
    };
    if process.token != input.token {
        return Ok(());
    }
    process
        .master
        .resize(terminal_size(
            input.cols,
            input.rows,
            input.pixel_width,
            input.pixel_height,
        ))
        .map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn kill_terminal(
    state: State<'_, DesktopState>,
    input: TerminalControlInput,
) -> std::result::Result<bool, String> {
    let process = {
        let mut sessions = state.terminal_sessions.lock().await;
        match sessions.get(&input.session_id) {
            Some(process) if process.token == input.token => sessions.remove(&input.session_id),
            _ => None,
        }
    };

    if let Some(process) = process {
        terminate_terminal_process(process);
        Ok(true)
    } else {
        Ok(false)
    }
}

pub(super) fn validate_terminal_value<'a>(
    value: &'a str,
    label: &str,
) -> std::result::Result<&'a str, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} cannot be empty"));
    }
    if trimmed.len() > 256 {
        return Err(format!("{label} is too long"));
    }
    Ok(trimmed)
}

pub(super) fn terminal_size(cols: u16, rows: u16, pixel_width: u16, pixel_height: u16) -> PtySize {
    PtySize {
        rows: rows.clamp(4, 200),
        cols: cols.clamp(20, 500),
        pixel_width,
        pixel_height,
    }
}

pub(super) fn spawn_terminal_reader(
    app: AppHandle,
    session_id: String,
    token: String,
    mut reader: Box<dyn Read + Send>,
) {
    std::thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        let mut pending = Vec::<u8>::new();
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    pending.extend_from_slice(&buffer[..n]);
                    emit_terminal_utf8_chunks(&app, &session_id, &token, &mut pending);
                }
                Err(_) => break,
            }
        }

        if !pending.is_empty() {
            emit_terminal_data(
                &app,
                &session_id,
                &token,
                String::from_utf8_lossy(&pending).to_string(),
            );
        }
    });
}

pub(super) fn emit_terminal_utf8_chunks(
    app: &AppHandle,
    session_id: &str,
    token: &str,
    pending: &mut Vec<u8>,
) {
    loop {
        match std::str::from_utf8(pending) {
            Ok(valid) => {
                if !valid.is_empty() {
                    emit_terminal_data(app, session_id, token, valid.to_string());
                }
                pending.clear();
                break;
            }
            Err(err) => {
                let valid_up_to = err.valid_up_to();
                if valid_up_to > 0 {
                    let data = String::from_utf8_lossy(&pending[..valid_up_to]).to_string();
                    emit_terminal_data(app, session_id, token, data);
                    pending.drain(..valid_up_to);
                    continue;
                }

                if let Some(error_len) = err.error_len() {
                    let data = String::from_utf8_lossy(&pending[..error_len]).to_string();
                    emit_terminal_data(app, session_id, token, data);
                    pending.drain(..error_len);
                    continue;
                }

                break;
            }
        }
    }
}

pub(super) fn emit_terminal_data(app: &AppHandle, session_id: &str, token: &str, data: String) {
    if data.is_empty() {
        return;
    }

    let _ = app.emit(
        TERMINAL_DATA_EVENT_NAME,
        TerminalDataEvent {
            session_id: session_id.to_string(),
            token: token.to_string(),
            data,
        },
    );
}

pub(super) fn spawn_terminal_waiter(
    app: AppHandle,
    terminal_sessions: Arc<Mutex<HashMap<String, TerminalProcess>>>,
    session_id: String,
    token: String,
    mut child: Box<dyn Child + Send + Sync>,
) {
    std::thread::spawn(move || {
        let status = child.wait();
        let (exit_code, signal) = match status {
            Ok(status) => (
                Some(status.exit_code()),
                status.signal().map(std::string::ToString::to_string),
            ),
            Err(err) => (None, Some(err.to_string())),
        };

        tauri::async_runtime::spawn(async move {
            let mut sessions = terminal_sessions.lock().await;
            let is_current = sessions
                .get(&session_id)
                .map(|process| process.token == token)
                .unwrap_or(false);
            if !is_current {
                return;
            }

            sessions.remove(&session_id);
            let _ = app.emit(
                TERMINAL_EXIT_EVENT_NAME,
                TerminalExitEvent {
                    session_id,
                    token,
                    exit_code,
                    signal,
                },
            );
        });
    });
}

pub(super) fn terminate_terminal_process(process: TerminalProcess) {
    if let Ok(mut killer) = process.killer.lock() {
        let _ = killer.kill();
    }
}
