use crate::*;

#[tauri::command]
pub(super) async fn list_conversations(
    state: State<'_, DesktopState>,
    input: WorkspaceInput,
) -> std::result::Result<Vec<ConversationSummary>, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    state
        .store
        .list_conversations(&workspace_root.display().to_string())
        .map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn create_conversation(
    state: State<'_, DesktopState>,
    input: WorkspaceInput,
) -> std::result::Result<WorkspaceBootstrap, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    state
        .store
        .create_conversation(
            &workspace_root.display().to_string(),
            &state.default_model,
            &state.system_prompt,
        )
        .map_err(error_to_string)?;
    state
        .store
        .bootstrap_workspace(&workspace_root, &state.default_model, &state.system_prompt)
        .map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn load_conversation(
    state: State<'_, DesktopState>,
    input: ConversationInput,
) -> std::result::Result<SavedConversation, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    state
        .store
        .load_conversation(
            &workspace_root.display().to_string(),
            &input.conversation_id,
        )
        .map_err(error_to_string)?
        .ok_or_else(|| "conversation not found".to_string())
}

#[tauri::command]
pub(super) async fn rename_conversation(
    state: State<'_, DesktopState>,
    input: RenameConversationInput,
) -> std::result::Result<Vec<ConversationSummary>, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let title = input.title.trim();
    if title.is_empty() {
        return Err("title cannot be empty".into());
    }
    let workspace_id = workspace_root.display().to_string();
    state
        .store
        .rename_conversation(&workspace_id, &input.conversation_id, title)
        .map_err(error_to_string)?;
    state
        .store
        .list_conversations(&workspace_id)
        .map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn delete_conversation(
    state: State<'_, DesktopState>,
    input: ConversationInput,
) -> std::result::Result<WorkspaceBootstrap, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let workspace_id = workspace_root.display().to_string();
    {
        let active_turns = state.active_turns.lock().await;
        if active_turns.contains_key(&input.conversation_id) {
            return Err("a turn is already running for this conversation".into());
        }
    }
    state
        .store
        .delete_conversation(&workspace_id, &input.conversation_id)
        .map_err(error_to_string)?;
    state
        .store
        .bootstrap_workspace(&workspace_root, &state.default_model, &state.system_prompt)
        .map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn set_conversation_mode(
    state: State<'_, DesktopState>,
    input: ConversationModeInput,
) -> std::result::Result<SavedConversation, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let workspace_id = workspace_root.display().to_string();
    {
        let active_turns = state.active_turns.lock().await;
        if active_turns.contains_key(&input.conversation_id) {
            return Err("a turn is already running for this conversation".into());
        }
    }

    let mut conversation = state
        .store
        .load_conversation(&workspace_id, &input.conversation_id)
        .map_err(error_to_string)?
        .ok_or_else(|| "conversation not found".to_string())?;

    let mode = AgentMode::from(input.mode);
    let current_plan_workflow = std::mem::take(&mut conversation.plan_workflow);
    conversation.plan_workflow = match mode {
        AgentMode::Act => PlanWorkflowState::Idle,
        AgentMode::Plan => match current_plan_workflow {
            PlanWorkflowState::Idle => PlanWorkflowState::PlanningQuestions,
            current => current,
        },
        AgentMode::Goal => PlanWorkflowState::Idle,
    };
    conversation.goal_workflow = match mode {
        AgentMode::Goal => resume_goal_workflow(std::mem::take(&mut conversation.goal_workflow)),
        AgentMode::Act | AgentMode::Plan => {
            pause_goal_workflow(std::mem::take(&mut conversation.goal_workflow))
        }
    };
    conversation.model = conversation.mode_model_settings.get(mode).clone();

    state
        .store
        .save_conversation(&conversation)
        .map_err(error_to_string)?;
    Ok(conversation)
}

#[tauri::command]
pub(super) async fn set_conversation_model_preference(
    state: State<'_, DesktopState>,
    input: ConversationModelPreferenceInput,
) -> std::result::Result<ModeModelSettings, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let workspace_id = workspace_root.display().to_string();
    let conversation_id = input.conversation_id;
    let mode = AgentMode::from(input.mode);

    {
        let active_turns = state.active_turns.lock().await;
        if active_turns.contains_key(&conversation_id) {
            return Err("a turn is already running for this conversation".into());
        }
    }

    let mut conversation = state
        .store
        .load_conversation(&workspace_id, &conversation_id)
        .map_err(error_to_string)?
        .ok_or_else(|| "conversation not found".to_string())?;
    let selected = model_with_optional_selection(
        conversation.mode_model_settings.get(mode),
        input.model,
        input.thinking,
    );
    let provider = provider_from_registry(&state, &selected.provider)?;
    provider
        .capabilities(&selected)
        .ok_or_else(|| format!("model `{}` is not supported", selected.name))?;

    conversation.mode_model_settings.set(mode, selected.clone());
    if conversation_active_mode(&conversation) == mode {
        conversation.model = selected.clone();
    }

    let mut default_settings = state
        .store
        .load_mode_model_settings(&state.default_model)
        .map_err(error_to_string)?;
    default_settings.set(mode, selected);

    state
        .store
        .save_conversation_and_mode_model_settings(&conversation, &default_settings)
        .map_err(error_to_string)?;
    Ok(conversation.mode_model_settings)
}

#[tauri::command]
pub(super) async fn list_mcp_settings(
    state: State<'_, DesktopState>,
) -> std::result::Result<McpSettings, String> {
    state.store.load_mcp_settings().map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn save_mcp_settings(
    state: State<'_, DesktopState>,
    input: SaveMcpSettingsInput,
) -> std::result::Result<McpSettings, String> {
    state
        .store
        .save_mcp_settings(&input.settings)
        .map_err(error_to_string)?;
    Ok(input.settings)
}

#[tauri::command]
pub(super) async fn list_tool_settings(
    state: State<'_, DesktopState>,
    input: WorkspaceInput,
) -> std::result::Result<ToolSettingsView, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let settings = state.store.load_tool_settings().map_err(error_to_string)?;
    Ok(tool_settings_view(
        &settings,
        &configurable_tool_catalog(&workspace_root),
    ))
}

#[tauri::command]
pub(super) async fn save_tool_settings(
    state: State<'_, DesktopState>,
    input: SaveToolSettingsInput,
) -> std::result::Result<ToolSettingsView, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let catalog = configurable_tool_catalog(&workspace_root);
    let saved = state
        .store
        .save_tool_settings_for_catalog(&input.settings, &catalog)
        .map_err(error_to_string)?;
    Ok(tool_settings_view(&saved, &catalog))
}

#[tauri::command]
pub(super) async fn list_sub_agent_settings(
    state: State<'_, DesktopState>,
) -> std::result::Result<SubAgentSettings, String> {
    state
        .store
        .load_sub_agent_settings()
        .map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn save_sub_agent_settings(
    state: State<'_, DesktopState>,
    input: SaveSubAgentSettingsInput,
) -> std::result::Result<SubAgentSettings, String> {
    for agent in input.settings.agents.iter().filter(|agent| agent.enabled) {
        let provider = provider_from_registry(&state, &agent.model.provider)?;
        provider
            .capabilities(&agent.model)
            .ok_or_else(|| format!("model `{}` is not supported", agent.model.name))?;
    }
    state
        .store
        .save_sub_agent_settings(&input.settings)
        .map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn probe_mcp_tools(
    state: State<'_, DesktopState>,
) -> std::result::Result<Vec<sinew_app::McpServerProbe>, String> {
    let settings = state.store.load_mcp_settings().map_err(error_to_string)?;
    Ok(probe_mcp_servers(&settings).await)
}

#[tauri::command]
pub(super) async fn get_mcp_oauth_status(
    state: State<'_, DesktopState>,
    input: McpOAuthInput,
) -> std::result::Result<McpOAuthStatus, String> {
    let settings = state.store.load_mcp_settings().map_err(error_to_string)?;
    let mut active_login = state.mcp_login.lock().await;
    let attempt = active_login.clone();
    if let Some(attempt) = attempt.filter(|attempt| attempt.server_id == input.server_id) {
        let outcome = attempt
            .outcome
            .lock()
            .map_err(|_| "MCP login state is unavailable".to_string())?
            .clone();
        if let Some(outcome) = outcome {
            *active_login = None;
            let connected = mcp_oauth_connected(&settings, &input.server_id).unwrap_or(false);
            return Ok(McpOAuthStatus {
                connected,
                connection_state: if outcome.success && connected {
                    "connected"
                } else {
                    "error"
                }
                .into(),
                login_id: None,
                error: outcome.error,
            });
        }
        return Ok(McpOAuthStatus {
            connected: false,
            connection_state: "connecting".into(),
            login_id: Some(attempt.id),
            error: None,
        });
    }
    drop(active_login);

    let settings = state.store.load_mcp_settings().map_err(error_to_string)?;
    let connected = mcp_oauth_connected(&settings, &input.server_id).unwrap_or(false);
    let discovery_error = if connected {
        None
    } else {
        discover_mcp_oauth(&settings, &input.server_id)
            .await
            .ok()
            .and_then(|discovery| discovery.error)
    };
    Ok(McpOAuthStatus {
        connected,
        connection_state: if connected {
            "connected"
        } else {
            "disconnected"
        }
        .into(),
        login_id: None,
        error: discovery_error,
    })
}

#[tauri::command]
pub(super) async fn start_mcp_oauth_login_command(
    state: State<'_, DesktopState>,
    input: McpOAuthInput,
) -> std::result::Result<StartMcpOAuthLoginOutput, String> {
    if let Some(existing) = state.mcp_login.lock().await.take() {
        existing.cancel.notify_one();
    }

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", MCP_OAUTH_REDIRECT_PORT))
        .await
        .with_context(|| {
            format!("unable to bind MCP OAuth callback port {MCP_OAUTH_REDIRECT_PORT}")
        })
        .map_err(error_to_string)?;
    let settings = state.store.load_mcp_settings().map_err(error_to_string)?;
    let started = start_mcp_oauth_login(&settings, &input.server_id)
        .await
        .map_err(error_to_string)?;
    let login_id = started.output.login_id.clone();
    let output = started.output.clone();
    let plan = started.plan;
    let cancel = Arc::new(Notify::new());
    let outcome = Arc::new(StdMutex::new(None));

    {
        let mut active_login = state.mcp_login.lock().await;
        *active_login = Some(McpLoginAttempt {
            id: login_id,
            server_id: input.server_id,
            cancel: cancel.clone(),
            outcome: outcome.clone(),
        });
    }

    tauri::async_runtime::spawn(async move {
        let result = run_mcp_oauth_server(listener, plan, cancel).await;
        let login_outcome = match result {
            Ok(()) => sinew_app::McpOAuthOutcome {
                success: true,
                error: None,
            },
            Err(err) => sinew_app::McpOAuthOutcome {
                success: false,
                error: Some(err.to_string()),
            },
        };
        if let Ok(mut slot) = outcome.lock() {
            *slot = Some(login_outcome);
        }
    });

    Ok(output)
}

#[tauri::command]
pub(super) async fn cancel_mcp_oauth_login(
    state: State<'_, DesktopState>,
    input: McpOAuthInput,
) -> std::result::Result<McpOAuthStatus, String> {
    {
        let mut active_login = state.mcp_login.lock().await;
        if let Some(attempt) = active_login.take() {
            if attempt.server_id == input.server_id {
                attempt.cancel.notify_one();
            } else {
                *active_login = Some(attempt);
            }
        }
    }
    let settings = state.store.load_mcp_settings().map_err(error_to_string)?;
    Ok(McpOAuthStatus {
        connected: mcp_oauth_connected(&settings, &input.server_id).unwrap_or(false),
        connection_state: "disconnected".into(),
        login_id: None,
        error: None,
    })
}

#[tauri::command]
pub(super) async fn disconnect_mcp_oauth(
    state: State<'_, DesktopState>,
    input: McpOAuthInput,
) -> std::result::Result<McpOAuthStatus, String> {
    {
        let mut active_login = state.mcp_login.lock().await;
        if let Some(attempt) = active_login.take() {
            if attempt.server_id == input.server_id {
                attempt.cancel.notify_one();
            } else {
                *active_login = Some(attempt);
            }
        }
    }
    let settings = state.store.load_mcp_settings().map_err(error_to_string)?;
    delete_mcp_oauth(&settings, &input.server_id).map_err(error_to_string)?;
    Ok(McpOAuthStatus {
        connected: false,
        connection_state: "disconnected".into(),
        login_id: None,
        error: None,
    })
}

async fn run_mcp_oauth_server(
    listener: tokio::net::TcpListener,
    plan: McpOAuthLoginPlan,
    cancel: Arc<Notify>,
) -> Result<()> {
    loop {
        tokio::select! {
            _ = cancel.notified() => {
                anyhow::bail!("Login canceled");
            }
            accepted = listener.accept() => {
                let (mut stream, _) = accepted.context("MCP OAuth callback accept failed")?;
                if let Some(result) = handle_mcp_oauth_request(&mut stream, &plan).await? {
                    return result;
                }
            }
        }
    }
}

async fn handle_mcp_oauth_request(
    stream: &mut tokio::net::TcpStream,
    plan: &McpOAuthLoginPlan,
) -> Result<Option<Result<()>>> {
    let mut buffer = [0u8; 8192];
    let read = stream
        .read(&mut buffer)
        .await
        .context("MCP OAuth callback read failed")?;
    if read == 0 {
        return Ok(None);
    }
    let request = String::from_utf8_lossy(&buffer[..read]);
    let Some(first_line) = request.lines().next() else {
        write_http_response(stream, 400, "Bad Request", "Bad Request").await?;
        return Ok(None);
    };
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();
    if method != "GET" {
        write_http_response(stream, 405, "Method Not Allowed", "Method Not Allowed").await?;
        return Ok(None);
    }
    let parsed = parse_local_oauth_url(target)?;
    match parsed.path() {
        MCP_OAUTH_CALLBACK_PATH => {
            let params = parsed
                .query_pairs()
                .into_owned()
                .collect::<HashMap<String, String>>();
            if params.get("state").map(String::as_str) != Some(plan.state.as_str()) {
                write_html_response(stream, 400, mcp_login_error_html("State mismatch")).await?;
                return Ok(Some(Err(anyhow::anyhow!("State mismatch"))));
            }
            if let Some(error) = params.get("error") {
                let message = params
                    .get("error_description")
                    .filter(|value| !value.trim().is_empty())
                    .cloned()
                    .unwrap_or_else(|| error.clone());
                write_html_response(stream, 400, mcp_login_error_html(&message)).await?;
                return Ok(Some(Err(anyhow::anyhow!(message))));
            }
            let Some(code) = params.get("code").filter(|value| !value.is_empty()) else {
                write_html_response(
                    stream,
                    400,
                    mcp_login_error_html("Missing authorization code"),
                )
                .await?;
                return Ok(Some(Err(anyhow::anyhow!("Missing authorization code"))));
            };
            match exchange_mcp_oauth_code(plan, code).await {
                Ok(()) => {
                    write_html_response(stream, 200, mcp_login_success_html(&plan.server.name))
                        .await?;
                    Ok(Some(Ok(())))
                }
                Err(err) => {
                    let message = err.to_string();
                    write_html_response(stream, 500, mcp_login_error_html(&message)).await?;
                    Ok(Some(Err(anyhow::anyhow!(message))))
                }
            }
        }
        "/cancel" => {
            write_http_response(stream, 200, "OK", "Login canceled").await?;
            Ok(Some(Err(anyhow::anyhow!("Login canceled"))))
        }
        _ => {
            write_http_response(stream, 404, "Not Found", "Not Found").await?;
            Ok(None)
        }
    }
}

fn mcp_login_success_html(server_name: &str) -> String {
    let server_name = html_escape(server_name);
    format!(
        r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <title>Sinew MCP connected</title>
    <style>
      body{{margin:0;min-height:100vh;display:grid;place-items:center;background:#0a0b0d;color:#f4f4f5;font:15px -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}}
      main{{max-width:420px;padding:32px;text-align:center}}
      h1{{font-size:22px;margin:0 0 10px}}
      p{{margin:0;color:#a1a1aa;line-height:1.5}}
    </style>
  </head>
  <body><main><h1>{server_name} is connected</h1><p>You can close this tab and return to Sinew.</p></main></body>
</html>"#
    )
}

fn mcp_login_error_html(message: &str) -> String {
    openai_login_error_html(message)
}

#[tauri::command]
pub(super) async fn list_installed_skills_command(
    state: State<'_, DesktopState>,
    input: WorkspaceInput,
) -> std::result::Result<Vec<InstalledSkill>, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let settings = state.store.load_skill_settings().map_err(error_to_string)?;
    Ok(list_installed_skills(workspace_root, &settings))
}

#[tauri::command]
pub(super) async fn save_skill_settings(
    state: State<'_, DesktopState>,
    input: SaveSkillSettingsInput,
) -> std::result::Result<Vec<InstalledSkill>, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let saved = state
        .store
        .save_skill_settings(&input.settings)
        .map_err(error_to_string)?;
    Ok(list_installed_skills(workspace_root, &saved))
}
