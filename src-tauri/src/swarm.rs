use crate::*;

pub(super) fn agent_swarm_error_from_event(event: &AgentEvent) -> Option<(String, String)> {
    let AgentEvent::SubAgentEvent {
        agent_name,
        team_name,
        event,
        ..
    } = event
    else {
        return None;
    };
    team_name.as_ref()?;
    let AgentEvent::Error { message } = event.as_ref() else {
        return None;
    };
    Some((agent_name.clone(), message.clone()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AgentSwarmCompletion {
    pub(super) team_name: String,
    pub(super) responses: Vec<AgentSwarmFinalResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AgentSwarmFinalResponse {
    pub(super) agent: String,
    pub(super) status: String,
    pub(super) last_response: String,
    pub(super) last_error: Option<String>,
}

pub(super) fn schedule_main_wake_for_swarm_event(
    app: &AppHandle,
    state: &DesktopState,
    workspace_root: &Path,
    conversation_id: &str,
    event: &AgentEvent,
) {
    schedule_main_wake_for_swarm_error(app, state, workspace_root, conversation_id, event);
    schedule_main_wake_for_swarm_completion(app, state, workspace_root, conversation_id, event);
}

pub(super) fn schedule_main_wake_for_swarm_error(
    app: &AppHandle,
    state: &DesktopState,
    workspace_root: &Path,
    conversation_id: &str,
    event: &AgentEvent,
) {
    let Some((agent_name, error)) = agent_swarm_error_from_event(event) else {
        return;
    };
    let app_for_wake = app.clone();
    let state_for_wake = state.clone();
    let workspace_root_for_wake = workspace_root.to_path_buf();
    let conversation_id_for_wake = conversation_id.to_string();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        if let Err(err) = wake_main_agent_for_swarm_error(
            app_for_wake,
            state_for_wake,
            workspace_root_for_wake,
            conversation_id_for_wake,
            agent_name,
            error,
        )
        .await
        {
            tracing::warn!(%err, "failed to wake main agent for swarm error");
        }
    });
}

pub(super) fn schedule_main_wake_for_swarm_completion(
    app: &AppHandle,
    state: &DesktopState,
    workspace_root: &Path,
    conversation_id: &str,
    event: &AgentEvent,
) {
    let Some(completion) = agent_swarm_completion_from_event(event) else {
        return;
    };
    let app_for_wake = app.clone();
    let state_for_wake = state.clone();
    let workspace_root_for_wake = workspace_root.to_path_buf();
    let conversation_id_for_wake = conversation_id.to_string();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        if let Err(err) = wake_main_agent_for_swarm_completion(
            app_for_wake,
            state_for_wake,
            workspace_root_for_wake,
            conversation_id_for_wake,
            completion,
        )
        .await
        {
            tracing::warn!(%err, "failed to wake main agent for swarm completion");
        }
    });
}

pub(super) fn agent_swarm_completion_from_event(
    event: &AgentEvent,
) -> Option<AgentSwarmCompletion> {
    let AgentEvent::ToolFinished {
        is_error,
        meta: Some(meta),
        ..
    } = event
    else {
        return None;
    };
    if *is_error {
        return None;
    }
    let meta = meta.as_object()?;
    let status = meta
        .get("teamRunStatus")
        .and_then(Value::as_str)
        .map(str::trim)?;
    if status != "completed" {
        return None;
    }
    let team = meta.get("team")?.as_object()?;
    let team_name = team.get("name")?.as_str()?.trim();
    if team_name.is_empty() {
        return None;
    }
    let mut responses = agent_swarm_final_responses_from_value(meta.get("agentFinalResponses"));
    if responses.is_empty() {
        responses = agent_swarm_final_responses_from_team(meta.get("team"));
    }
    Some(AgentSwarmCompletion {
        team_name: team_name.to_string(),
        responses,
    })
}

pub(super) fn agent_swarm_final_responses_from_value(
    value: Option<&Value>,
) -> Vec<AgentSwarmFinalResponse> {
    value
        .and_then(Value::as_array)
        .map(|responses| {
            responses
                .iter()
                .filter_map(|value| agent_swarm_final_response_from_record(value, false))
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn agent_swarm_final_responses_from_team(
    value: Option<&Value>,
) -> Vec<AgentSwarmFinalResponse> {
    value
        .and_then(Value::as_object)
        .and_then(|team| team.get("agents"))
        .and_then(Value::as_array)
        .map(|agents| {
            agents
                .iter()
                .filter_map(|value| agent_swarm_final_response_from_record(value, true))
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn agent_swarm_final_response_from_record(
    value: &Value,
    team_agent_snapshot: bool,
) -> Option<AgentSwarmFinalResponse> {
    let record = value.as_object()?;
    let agent = record
        .get("agent")
        .or_else(|| record.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let status = final_response_status_from_meta(record.get("status"));
    let last_response_key = if team_agent_snapshot {
        "lastSummary"
    } else {
        "lastResponse"
    };
    let last_response = record
        .get(last_response_key)
        .or_else(|| record.get("lastResponse"))
        .or_else(|| record.get("lastSummary"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("No final response recorded.");
    let last_error = record
        .get("lastError")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    Some(AgentSwarmFinalResponse {
        agent: agent.to_string(),
        status,
        last_response: last_response.to_string(),
        last_error,
    })
}

pub(super) fn final_response_status_from_meta(value: Option<&Value>) -> String {
    let status = value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("finished");
    if status == "idle" {
        "finished".to_string()
    } else {
        status.to_string()
    }
}

pub(super) async fn wake_main_agent_for_swarm_error(
    app: AppHandle,
    state: DesktopState,
    workspace_root: PathBuf,
    conversation_id: String,
    agent_name: String,
    error: String,
) -> std::result::Result<(), String> {
    let wake_text = agent_swarm_error_wake_text(&agent_name, &error);
    wake_main_agent_for_swarm_notice(app, state, workspace_root, conversation_id, wake_text).await
}

pub(super) async fn wake_main_agent_for_swarm_completion(
    app: AppHandle,
    state: DesktopState,
    workspace_root: PathBuf,
    conversation_id: String,
    completion: AgentSwarmCompletion,
) -> std::result::Result<(), String> {
    let wake_text = agent_swarm_completion_wake_text(&completion);
    wake_main_agent_for_swarm_notice(app, state, workspace_root, conversation_id, wake_text).await
}

pub(super) async fn wake_main_agent_for_swarm_notice(
    app: AppHandle,
    state: DesktopState,
    workspace_root: PathBuf,
    conversation_id: String,
    wake_text: String,
) -> std::result::Result<(), String> {
    if !wait_for_conversation_turn_slot_with_attempts(
        &state.active_turns,
        &conversation_id,
        SWARM_WAKE_TURN_SLOT_WAIT_ATTEMPTS,
    )
    .await
    {
        return Ok(());
    }

    let workspace_id = workspace_root.display().to_string();
    let effective_system_prompt =
        system_prompt_for_workspace(&workspace_root, &state.system_prompt)
            .map_err(error_to_string)?;
    let mut conversation = state
        .store
        .load_conversation(&workspace_id, &conversation_id)
        .map_err(error_to_string)?
        .ok_or_else(|| "conversation not found".to_string())?;

    let selected_model = conversation.mode_model_settings.get(AgentMode::Act).clone();
    conversation.model = selected_model;
    let provider = provider_from_registry(&state, &conversation.model.provider)?;
    provider
        .capabilities(&conversation.model)
        .ok_or_else(|| format!("model `{}` is not supported", conversation.model.name))?;

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let cancel = TurnCancel::new(cmd_tx);
    {
        let mut active_turns = state.active_turns.lock().await;
        if active_turns.contains_key(&conversation_id) {
            return Ok(());
        }
        active_turns.insert(conversation_id.clone(), cancel.clone());
    }
    register_active_turn(&app, &state, &workspace_id, &conversation_id).await;

    let turn_user_history_index = conversation.history.len();
    let before_turn_snapshot = snapshot_workspace_for_checkpoint(&workspace_root);
    conversation.history.push(build_user_message(
        &wake_text,
        &[],
        &workspace_root,
        None,
        MessageVisibilityInput::SystemReminder,
    ));
    state
        .store
        .save_conversation(&conversation)
        .map_err(|err| {
            let active_turns = state.active_turns.clone();
            let active_turn_details = state.active_turn_details.clone();
            let app = app.clone();
            let conversation_id = conversation_id.clone();
            tauri::async_runtime::spawn(async move {
                active_turns.lock().await.remove(&conversation_id);
                active_turn_details
                    .lock()
                    .map(|mut active| active.remove(&conversation_id))
                    .ok();
                emit_active_turns_changed(&app, &active_turn_details).await;
            });
            error_to_string(err)
        })?;

    let mcp_settings = state.store.load_mcp_settings().map_err(error_to_string)?;
    let sub_agent_settings = state
        .store
        .load_sub_agent_settings()
        .map_err(error_to_string)?;
    let tool_settings = state.store.load_tool_settings().map_err(error_to_string)?;
    let skill_settings = state.store.load_skill_settings().map_err(error_to_string)?;
    let turn_system_prompt = with_turn_plan_reminder(&effective_system_prompt, None);
    let providers = provider_registry_snapshot(&state)?;
    let context = TurnContext {
        provider,
        model: conversation.model.clone(),
        cache_key: Some(conversation.id.clone()),
        cache_stable_message_count: turn_user_history_index,
        service_tier: None,
        auto_compact: true,
        mode: AgentMode::Act,
        stop_questions: false,
        system_prompt: turn_system_prompt.clone(),
        history: conversation.history.clone(),
        todo_list: conversation.todo_list.clone(),
        goal_workflow: conversation.goal_workflow.clone(),
        bash: Arc::new(BashTool::new(workspace_root.clone())),
        glob: Arc::new(GlobTool::new(workspace_root.clone())),
        grep: Arc::new(GrepTool::new(workspace_root.clone())),
        read: Arc::new(ReadTool::new(workspace_root.clone())),
        edit_file: Arc::new(EditFileTool::new(workspace_root.clone())),
        write_file: Arc::new(WriteFileTool::new(workspace_root.clone())),
        create_image: Arc::new(CreateImageTool::with_settings(
            workspace_root.clone(),
            tool_settings.image_provider,
            tool_settings.openai_image_use_subscription,
            tool_settings.openai_image_api_key(),
            tool_settings.nano_banana_api_key(),
        )),
        todo_list_tool: Some(Arc::new(ToDoListTool::new())),
        question: Some(Arc::new(QuestionTool::new())),
        web_search: Arc::new(WebSearchTool::with_settings(
            tool_settings.web_search_provider,
            tool_settings.linkup_api_key(),
        )),
        web_fetch: Arc::new(WebFetchTool::new()),
        skill: Arc::new(SkillTool::with_settings(
            workspace_root.clone(),
            skill_settings.clone(),
        )),
        database: Arc::new(DatabaseTool::new(state.store.clone())),
        mcp: Arc::new(McpToolRegistry::new(mcp_settings.clone())),
        subagents: Some(Arc::new(SubAgentTool::new(
            workspace_root.clone(),
            turn_system_prompt.clone(),
            providers.clone(),
            sub_agent_settings.clone(),
            mcp_settings.clone(),
            tool_settings.clone(),
            skill_settings.clone(),
            DatabaseTool::new(state.store.clone()),
            state.max_tool_rounds,
            None,
            cancel.clone(),
        ))),
        teams: Some(Arc::new(TeamTool::new(
            conversation.id.clone(),
            workspace_root.clone(),
            turn_system_prompt.clone(),
            providers,
            sub_agent_settings,
            mcp_settings,
            tool_settings.clone(),
            skill_settings,
            DatabaseTool::new(state.store.clone()),
            conversation.model.clone(),
            state.max_tool_rounds,
            None,
            state.team_runtime.clone(),
            cancel.clone(),
        ))),
        tool_settings,
        event_scope: None,
        max_tool_rounds: state.max_tool_rounds,
        event_tx,
        cancel,
        cmd_rx,
    };

    let store = state.store.clone();
    let active_turns = state.active_turns.clone();
    let active_turn_details = state.active_turn_details.clone();
    let conversation_title = conversation.title.clone();
    let conversation_model = conversation.model.clone();
    let conversation_mode_model_settings = conversation.mode_model_settings.clone();
    let conversation_system_prompt = conversation.system_prompt.clone();
    let plan_workflow = conversation.plan_workflow.clone();
    let conversation_id_for_events = conversation_id.clone();
    let workspace_root_for_checkpoint = workspace_root.clone();
    let before_turn_snapshot_for_checkpoint = before_turn_snapshot;

    tauri::async_runtime::spawn(async move {
        let mut engine = Box::pin(tauri::async_runtime::spawn(async move {
            run_turn(context).await
        }));
        let mut engine_done = false;
        let mut events_done = false;

        loop {
            tokio::select! {
                event = event_rx.recv(), if !events_done => {
                    match event {
                        Some(event) => {
                            if matches!(event, AgentEvent::TurnFinished) {
                                continue;
                            }
                            schedule_main_wake_for_swarm_event(
                                &app,
                                &state,
                                &workspace_root,
                                &conversation_id_for_events,
                                &event,
                            );
                            let _ = emit_agent_event(
                                &app,
                                &workspace_id,
                                &conversation_id_for_events,
                                &event,
                            );
                            emit_agent_file_changes(&app, &workspace_id, &event);
                        }
                        None => {
                            events_done = true;
                        }
                    }
                }
                engine_result = &mut engine, if !engine_done => {
                    engine_done = true;
                    match engine_result {
                        Ok(output) => {
                            let saved = SavedConversation {
                                id: conversation_id_for_events.clone(),
                                workspace_id: workspace_id.clone(),
                                title: conversation_title.clone(),
                                model: conversation_model.clone(),
                                mode_model_settings: conversation_mode_model_settings.clone(),
                                system_prompt: conversation_system_prompt.clone(),
                                todo_list: output.todo_list,
                                plan_workflow: plan_workflow.clone(),
                                goal_workflow: output.goal_workflow,
                                history: output.history,
                            };
                            let saved_ok = match store.save_conversation(&saved) {
                                Ok(()) => true,
                                Err(err) => {
                                    let _ = emit_agent_event(
                                        &app,
                                        &workspace_id,
                                        &conversation_id_for_events,
                                        &AgentEvent::Error {
                                            message: format!("save failed: {err}"),
                                        },
                                    );
                                    false
                                }
                            };
                            if saved_ok {
                                if output.compacted {
                                    if let Err(err) = store
                                        .delete_turn_checkpoints_from(&conversation_id_for_events, 0)
                                    {
                                        let _ = emit_agent_event(
                                            &app,
                                            &workspace_id,
                                            &conversation_id_for_events,
                                            &AgentEvent::Error {
                                                message: format!(
                                                    "checkpoint cleanup failed: {err}"
                                                ),
                                            },
                                        );
                                    }
                                } else {
                                    let after_turn_snapshot = snapshot_workspace_for_checkpoint(
                                        &workspace_root_for_checkpoint,
                                    );
                                    let checkpoint = checkpoint_from_snapshots(
                                        &before_turn_snapshot_for_checkpoint,
                                        &after_turn_snapshot,
                                    );
                                    if let Err(err) = store.save_turn_checkpoint(
                                        &conversation_id_for_events,
                                        turn_user_history_index,
                                        &checkpoint,
                                    ) {
                                        let _ = emit_agent_event(
                                            &app,
                                            &workspace_id,
                                            &conversation_id_for_events,
                                            &AgentEvent::Error {
                                                message: format!("checkpoint save failed: {err}"),
                                            },
                                        );
                                    }
                                }
                            }
                            let _ = emit_agent_event(
                                &app,
                                &workspace_id,
                                &conversation_id_for_events,
                                &AgentEvent::TurnFinished,
                            );
                            active_turns.lock().await.remove(&conversation_id_for_events);
                            active_turn_details
                                .lock()
                                .map(|mut active| active.remove(&conversation_id_for_events))
                                .ok();
                            emit_active_turns_changed(&app, &active_turn_details).await;
                        }
                        Err(err) => {
                            let _ = emit_agent_event(
                                &app,
                                &workspace_id,
                                &conversation_id_for_events,
                                &AgentEvent::Error {
                                    message: format!("turn task failed: {err}"),
                                },
                            );
                            let _ = emit_agent_event(
                                &app,
                                &workspace_id,
                                &conversation_id_for_events,
                                &AgentEvent::TurnFinished,
                            );
                            active_turns.lock().await.remove(&conversation_id_for_events);
                            active_turn_details
                                .lock()
                                .map(|mut active| active.remove(&conversation_id_for_events))
                                .ok();
                            emit_active_turns_changed(&app, &active_turn_details).await;
                        }
                    }
                }
            }

            if engine_done && events_done {
                break;
            }
        }
    });

    Ok(())
}

pub(super) fn agent_swarm_error_wake_text(agent_name: &str, error: &str) -> String {
    format!(
        "<agent_swarm_error>\nagent: @{agent_name}\nerror: {}\n</agent_swarm_error>\n\nHandle this Agent Swarm failure now. Relaunch only the failed teammate when that is the right recovery. If it keeps failing, stop that teammate so their open work returns to pending.",
        truncate_hidden_turn_line(error, 1200)
    )
}

pub(super) fn agent_swarm_completion_wake_text(completion: &AgentSwarmCompletion) -> String {
    let mut lines = vec![
        "<agent_swarm_finished>".to_string(),
        format!("team: {}", completion.team_name),
        "agentResponses:".to_string(),
    ];
    if completion.responses.is_empty() {
        lines.push("- none".to_string());
    } else {
        for response in &completion.responses {
            lines.push(format!("- agent: @{}", response.agent));
            lines.push(format!("  status: {}", response.status));
            if let Some(error) = response
                .last_error
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                lines.push(format!(
                    "  error: {}",
                    truncate_hidden_turn_line(error.trim(), 1200)
                ));
            }
            lines.push("  lastResponse: |".to_string());
            lines.extend(indent_hidden_lines(
                &truncate_hidden_turn_line(&response.last_response, 4000),
                "    ",
            ));
        }
    }
    lines.push("</agent_swarm_finished>".to_string());
    lines.push(String::new());
    lines.push("L'Agent Swarm a terminé. Réponds maintenant à l'utilisateur pour lui dire que l'Agent Swarm a terminé, puis résume les dernières réponses structurées ci-dessus agent par agent. N'utilise pas TeamStatus, le shell, ni les fichiers juste pour vérifier que le swarm est terminé.".to_string());
    lines.join("\n")
}

pub(super) fn indent_hidden_lines(value: &str, indent: &str) -> Vec<String> {
    let lines = value.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return vec![indent.to_string()];
    }
    lines
        .into_iter()
        .map(|line| format!("{indent}{line}"))
        .collect()
}

pub(super) fn truncate_hidden_turn_line(value: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (count, ch) in value.chars().enumerate() {
        if count >= max_chars {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}

#[tauri::command]
pub(super) async fn stop_agent_swarm_command(
    app: AppHandle,
    state: State<'_, DesktopState>,
    input: StopAgentSwarmInput,
) -> std::result::Result<String, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let workspace_id = workspace_root.display().to_string();
    let conversation = state
        .store
        .load_conversation(&workspace_id, &input.conversation_id)
        .map_err(error_to_string)?
        .ok_or_else(|| "conversation not found".to_string())?;
    let effective_system_prompt =
        system_prompt_for_workspace(&workspace_root, &state.system_prompt)
            .map_err(error_to_string)?;
    let mcp_settings = state.store.load_mcp_settings().map_err(error_to_string)?;
    let sub_agent_settings = state
        .store
        .load_sub_agent_settings()
        .map_err(error_to_string)?;
    let tool_settings = state.store.load_tool_settings().map_err(error_to_string)?;
    let skill_settings = state.store.load_skill_settings().map_err(error_to_string)?;
    let team_tool = TeamTool::new(
        conversation.id.clone(),
        workspace_root.clone(),
        effective_system_prompt,
        provider_registry_snapshot(&state)?,
        sub_agent_settings,
        mcp_settings,
        tool_settings,
        skill_settings,
        DatabaseTool::new(state.store.clone()),
        conversation.model.clone(),
        state.max_tool_rounds,
        None,
        state.team_runtime.clone(),
        TurnCancel::empty(),
    );
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let mut payload = serde_json::Map::new();
    if let Some(team_name) = input
        .team_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        payload.insert("team_name".into(), json!(team_name));
    }
    let result = team_tool
        .run(
            "ui-agent-swarm-stop",
            "TeamStop",
            Value::Object(payload),
            AgentMode::Act,
            event_tx,
        )
        .await
        .ok_or_else(|| "TeamStop is unavailable".to_string())?;
    while let Ok(event) = event_rx.try_recv() {
        let _ = emit_agent_event(&app, &workspace_id, &conversation.id, &event);
        emit_agent_file_changes(&app, &workspace_id, &event);
    }
    if result.is_error {
        Err(result.content)
    } else {
        Ok(result.content)
    }
}
