use crate::*;

#[tauri::command]
pub(super) async fn send_message(
    app: AppHandle,
    state: State<'_, DesktopState>,
    input: SendMessageInput,
) -> std::result::Result<(), String> {
    let text = input.text.trim();
    if text.is_empty() {
        return Err("message cannot be empty".into());
    }
    let requested_mode = input.mode.map(AgentMode::from).unwrap_or_default();
    let plan_control = input.plan_control;
    let message_visibility = input
        .message_visibility
        .unwrap_or(MessageVisibilityInput::Normal);
    let service_tier = input.service_tier.map(ServiceTier::from);

    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let workspace_id = workspace_root.display().to_string();
    let effective_system_prompt =
        system_prompt_for_workspace(&workspace_root, &state.system_prompt)
            .map_err(error_to_string)?;
    if !wait_for_conversation_turn_slot(&state.active_turns, &input.conversation_id).await {
        return Err("a turn is already running for this conversation".into());
    }

    let mut conversation = state
        .store
        .load_conversation(&workspace_id, &input.conversation_id)
        .map_err(error_to_string)?
        .ok_or_else(|| "conversation not found".to_string())?;

    if let Some(index) = input.rewrite_from_history_index {
        if index >= conversation.history.len() {
            return Err("rewrite index out of bounds".into());
        }
        let message = &conversation.history[index];
        if !is_rewritable_user_message(message) {
            return Err("rewrite index must point to a rewritable user message".into());
        }
        if input.revert_workspace_changes {
            restore_workspace_for_rewrite(
                &app,
                &state.store,
                &workspace_root,
                &input.conversation_id,
                index,
            )
            .map_err(error_to_string)?;
        } else {
            state
                .store
                .delete_turn_checkpoints_from(&input.conversation_id, index)
                .map_err(error_to_string)?;
        }
        conversation.history.truncate(index);
        conversation.todo_list = todo_list_from_history(&conversation.history);
        conversation.plan_workflow = PlanWorkflowState::Idle;
    }

    let policy = plan_turn_policy(&conversation.plan_workflow, requested_mode, plan_control)?;
    let turn_plan_reminder = plan_implementation_turn_reminder(
        &workspace_root,
        &conversation.plan_workflow,
        &input.attachments,
        plan_control,
    )?;
    let turn_system_prompt = with_turn_plan_reminder(&effective_system_prompt, turn_plan_reminder);
    let mut mode_model_settings = conversation.mode_model_settings.clone();
    let selected_model = model_with_optional_selection(
        mode_model_settings.get(policy.mode),
        input.model,
        input.thinking,
        input.use_1m_context,
    );
    mode_model_settings.set(policy.mode, selected_model.clone());
    conversation.mode_model_settings = mode_model_settings.clone();
    conversation.model = selected_model;
    let provider = provider_from_registry(&state, &conversation.model.provider)?;
    provider
        .capabilities(&conversation.model)
        .ok_or_else(|| format!("model `{}` is not supported", conversation.model.name))?;
    let mcp_settings = state.store.load_mcp_settings().map_err(error_to_string)?;
    let sub_agent_settings = state
        .store
        .load_sub_agent_settings()
        .map_err(error_to_string)?;
    let tool_settings = state.store.load_tool_settings().map_err(error_to_string)?;
    let skill_settings = state.store.load_skill_settings().map_err(error_to_string)?;
    let next_plan_workflow = policy.next_workflow.clone();
    conversation.plan_workflow = next_plan_workflow.clone();
    conversation.goal_workflow = if policy.mode == AgentMode::Goal {
        match message_visibility {
            MessageVisibilityInput::Normal => start_goal_workflow(text),
            MessageVisibilityInput::SystemReminder => {
                resume_goal_workflow(std::mem::take(&mut conversation.goal_workflow))
            }
        }
    } else {
        pause_goal_workflow(std::mem::take(&mut conversation.goal_workflow))
    };

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let cancel = TurnCancel::new(cmd_tx);
    {
        let mut active_turns = state.active_turns.lock().await;
        if active_turns.contains_key(&input.conversation_id) {
            return Err("a turn is already running for this conversation".into());
        }
        active_turns.insert(input.conversation_id.clone(), cancel.clone());
    }
    register_active_turn(&app, &state, &workspace_id, &input.conversation_id).await;

    let turn_user_history_index = conversation.history.len();
    let before_turn_snapshot = snapshot_workspace_for_checkpoint(&workspace_root);
    conversation.history.push(build_user_message(
        text,
        &input.attachments,
        &workspace_root,
        plan_control,
        message_visibility,
    ));
    state
        .store
        .save_conversation(&conversation)
        .map_err(|err| {
            let active_turns = state.active_turns.clone();
            let active_turn_details = state.active_turn_details.clone();
            let app = app.clone();
            let conversation_id = input.conversation_id.clone();
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

    let providers = provider_registry_snapshot(&state)?;
    let database_tool = Arc::new(DatabaseTool::new(state.store.clone()));
    let context = TurnContext {
        provider,
        model: conversation.model.clone(),
        cache_key: Some(conversation.id.clone()),
        cache_stable_message_count: turn_user_history_index,
        service_tier,
        auto_compact: true,
        mode: policy.mode,
        stop_questions: policy.stop_questions,
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
        database: database_tool.clone(),
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
            service_tier,
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
            service_tier,
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
    let state_for_wake = state.inner().clone();
    let conversation_id = conversation.id.clone();
    let conversation_title = conversation.title.clone();
    let conversation_model = conversation.model.clone();
    let conversation_mode_model_settings = conversation.mode_model_settings.clone();
    let conversation_system_prompt = conversation.system_prompt.clone();
    let workspace_root_for_output = workspace_root.clone();
    let workspace_root_for_wake = workspace_root.clone();
    let plan_requested = policy.attach_plan;
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
                                &state_for_wake,
                                &workspace_root_for_wake,
                                &conversation_id,
                                &event,
                            );
                            let _ = emit_agent_event(&app, &workspace_id, &conversation_id, &event);
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
                            let mut history = output.history;
                            let mut plan_workflow = next_plan_workflow.clone();
                            let mut goal_workflow = output.goal_workflow;
                            if output.interrupted {
                                goal_workflow = pause_goal_workflow(goal_workflow);
                            }
                            let question_stop_requested = latest_question_stop_requested(
                                &history,
                                turn_user_history_index,
                            );
                            if plan_requested || question_stop_requested {
                                match attach_latest_plan_artifact(
                                    &workspace_root_for_output,
                                    &conversation_id,
                                    &mut history,
                                    turn_user_history_index,
                                    !question_stop_requested,
                                ) {
                                    Ok(Some(artifact)) => {
                                        emit_workspace_file_change(
                                            &app,
                                            &workspace_root_for_output,
                                            &artifact.path,
                                        );
                                        plan_workflow = PlanWorkflowState::PlanReady { artifact };
                                    }
                                    Ok(None) => {
                                        if question_stop_requested {
                                            plan_workflow = PlanWorkflowState::PlanningQuestions;
                                        }
                                    }
                                    Err(err) => {
                                        let _ = emit_agent_event(
                                            &app,
                                            &workspace_id,
                                            &conversation_id,
                                            &AgentEvent::Error {
                                                message: format!("plan save failed: {err}"),
                                            },
                                        );
                                    }
                                }
                            }
                            let saved = SavedConversation {
                                id: conversation_id.clone(),
                                workspace_id: workspace_id.clone(),
                                title: conversation_title.clone(),
                                model: conversation_model.clone(),
                                mode_model_settings: conversation_mode_model_settings.clone(),
                                system_prompt: conversation_system_prompt.clone(),
                                todo_list: output.todo_list,
                                plan_workflow,
                                goal_workflow,
                                history,
                            };
                            let saved_ok = match store.save_conversation(&saved) {
                                Ok(()) => true,
                                Err(err) => {
                                    let _ = emit_agent_event(
                                        &app,
                                        &workspace_id,
                                        &conversation_id,
                                        &AgentEvent::Error {
                                            message: format!("save failed: {err}"),
                                        },
                                    );
                                    false
                                }
                            };
                            if saved_ok {
                                if output.compacted {
                                    if let Err(err) =
                                        store.delete_turn_checkpoints_from(&conversation_id, 0)
                                    {
                                        let _ = emit_agent_event(
                                            &app,
                                            &workspace_id,
                                            &conversation_id,
                                            &AgentEvent::Error {
                                                message: format!(
                                                    "checkpoint cleanup failed: {err}"
                                                ),
                                            },
                                        );
                                    }
                                } else {
                                    let after_turn_snapshot = snapshot_workspace_for_checkpoint(
                                        &workspace_root_for_output,
                                    );
                                    let checkpoint = checkpoint_from_snapshots(
                                        &before_turn_snapshot_for_checkpoint,
                                        &after_turn_snapshot,
                                    );
                                    if let Err(err) = store.save_turn_checkpoint(
                                        &conversation_id,
                                        turn_user_history_index,
                                        &checkpoint,
                                    ) {
                                        let _ = emit_agent_event(
                                            &app,
                                            &workspace_id,
                                            &conversation_id,
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
                                &conversation_id,
                                &AgentEvent::TurnFinished,
                            );
                            active_turns.lock().await.remove(&conversation_id);
                            active_turn_details
                                .lock()
                                .map(|mut active| active.remove(&conversation_id))
                                .ok();
                            emit_active_turns_changed(&app, &active_turn_details).await;
                        }
                        Err(err) => {
                            let _ = emit_agent_event(
                                &app,
                                &workspace_id,
                                &conversation_id,
                                &AgentEvent::Error {
                                    message: format!("turn task failed: {err}"),
                                },
                            );
                            let _ = emit_agent_event(
                                &app,
                                &workspace_id,
                                &conversation_id,
                                &AgentEvent::TurnFinished,
                            );
                            active_turns.lock().await.remove(&conversation_id);
                            active_turn_details
                                .lock()
                                .map(|mut active| active.remove(&conversation_id))
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

#[tauri::command]
pub(super) async fn compact_conversation(
    app: AppHandle,
    state: State<'_, DesktopState>,
    input: CompactConversationInput,
) -> std::result::Result<(), String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let workspace_id = workspace_root.display().to_string();
    let effective_system_prompt =
        system_prompt_for_workspace(&workspace_root, &state.system_prompt)
            .map_err(error_to_string)?;
    if !wait_for_conversation_turn_slot(&state.active_turns, &input.conversation_id).await {
        return Err("a turn is already running for this conversation".into());
    }

    let mut conversation = state
        .store
        .load_conversation(&workspace_id, &input.conversation_id)
        .map_err(error_to_string)?
        .ok_or_else(|| "conversation not found".to_string())?;
    if conversation.history.is_empty() {
        return Err("conversation has no history to compact".into());
    }

    let selected_model = model_with_optional_selection(
        &conversation.model,
        input.model,
        input.thinking,
        input.use_1m_context,
    );
    let service_tier = input.service_tier.map(ServiceTier::from);
    let compaction_instruction = input
        .instruction
        .as_deref()
        .map(str::trim)
        .filter(|instruction| !instruction.is_empty())
        .map(str::to_string);
    let provider = provider_from_registry(&state, &selected_model.provider)?;
    provider
        .capabilities(&selected_model)
        .ok_or_else(|| format!("model `{}` is not supported", selected_model.name))?;
    let compact_mode = conversation_active_mode(&conversation);

    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel();
    let cancel = TurnCancel::new(cmd_tx);
    {
        let mut active_turns = state.active_turns.lock().await;
        if active_turns.contains_key(&input.conversation_id) {
            return Err("a turn is already running for this conversation".into());
        }
        active_turns.insert(input.conversation_id.clone(), cancel);
    }
    register_active_turn(&app, &state, &workspace_id, &input.conversation_id).await;

    let conversation_id = conversation.id.clone();
    let source_history = conversation.history.clone();
    let compaction_id = format!("context-compaction-{}", now_ms());

    let _ = emit_agent_event(
        &app,
        &workspace_id,
        &conversation_id,
        &AgentEvent::TurnStarted,
    );
    let _ = emit_agent_event(
        &app,
        &workspace_id,
        &conversation_id,
        &AgentEvent::ToolStarted {
            id: compaction_id.clone(),
            name: "context_compaction".to_string(),
        },
    );
    let args_pretty = compaction_instruction
        .as_ref()
        .map(|instruction| json!({ "instruction": instruction }).to_string())
        .unwrap_or_else(|| "{}".to_string());
    let _ = emit_agent_event(
        &app,
        &workspace_id,
        &conversation_id,
        &AgentEvent::ToolReady {
            id: compaction_id.clone(),
            summary: "Compact context".to_string(),
            args_pretty,
        },
    );

    let (summary_delta_tx, mut summary_delta_rx) = mpsc::unbounded_channel();
    let app_for_deltas = app.clone();
    let workspace_id_for_deltas = workspace_id.clone();
    let conversation_id_for_deltas = conversation_id.clone();
    let compaction_id_for_deltas = compaction_id.clone();
    let delta_forwarder = tauri::async_runtime::spawn(async move {
        while let Some(delta) = summary_delta_rx.recv().await {
            let _ = emit_agent_event(
                &app_for_deltas,
                &workspace_id_for_deltas,
                &conversation_id_for_deltas,
                &AgentEvent::ToolOutputDelta {
                    id: compaction_id_for_deltas.clone(),
                    delta,
                },
            );
        }
    });

    let result = compact_conversation_history(
        provider,
        selected_model.clone(),
        effective_system_prompt,
        source_history.clone(),
        Some(conversation_id.clone()),
        source_history.len(),
        service_tier,
        compaction_instruction,
        &mut cmd_rx,
        Some(summary_delta_tx),
    )
    .await;
    let _ = delta_forwarder.await;

    let command_result = match result {
        Ok(output) => {
            let retained = output.retained_user_messages;
            let summary = output.summary;
            conversation.model = selected_model.clone();
            conversation
                .mode_model_settings
                .set(compact_mode, selected_model);
            conversation.history = output.history;
            conversation.todo_list = todo_list_from_history(&conversation.history);
            match state.store.save_conversation(&conversation) {
                Ok(()) => {
                    if let Err(err) = state
                        .store
                        .delete_turn_checkpoints_from(&conversation_id, 0)
                    {
                        let message = format!("checkpoint cleanup failed: {err}");
                        let _ = emit_agent_event(
                            &app,
                            &workspace_id,
                            &conversation_id,
                            &AgentEvent::ToolFinished {
                                id: compaction_id.clone(),
                                output: message.clone(),
                                is_error: true,
                                file_changes: Vec::new(),
                                images: Vec::new(),
                                meta: None,
                            },
                        );
                        let _ = emit_agent_event(
                            &app,
                            &workspace_id,
                            &conversation_id,
                            &AgentEvent::Error {
                                message: message.clone(),
                            },
                        );
                        Err(message)
                    } else {
                        let label = match retained {
                            0 => "No raw user messages retained".to_string(),
                            1 => "Retained 1 recent user message".to_string(),
                            count => format!("Retained {count} recent user messages"),
                        };
                        let _ = emit_agent_event(
                            &app,
                            &workspace_id,
                            &conversation_id,
                            &AgentEvent::ToolFinished {
                                id: compaction_id.clone(),
                                output: label,
                                is_error: false,
                                file_changes: Vec::new(),
                                images: Vec::new(),
                                meta: Some(json!({
                                    "retainedUserMessages": retained,
                                    "compactionSummary": summary,
                                })),
                            },
                        );
                        Ok(())
                    }
                }
                Err(err) => {
                    let message = format!("save failed: {err}");
                    let _ = emit_agent_event(
                        &app,
                        &workspace_id,
                        &conversation_id,
                        &AgentEvent::ToolFinished {
                            id: compaction_id.clone(),
                            output: message.clone(),
                            is_error: true,
                            file_changes: Vec::new(),
                            images: Vec::new(),
                            meta: None,
                        },
                    );
                    let _ = emit_agent_event(
                        &app,
                        &workspace_id,
                        &conversation_id,
                        &AgentEvent::Error {
                            message: message.clone(),
                        },
                    );
                    Err(message)
                }
            }
        }
        Err(err) => {
            let message = err.to_string();
            let _ = emit_agent_event(
                &app,
                &workspace_id,
                &conversation_id,
                &AgentEvent::ToolFinished {
                    id: compaction_id.clone(),
                    output: message.clone(),
                    is_error: true,
                    file_changes: Vec::new(),
                    images: Vec::new(),
                    meta: None,
                },
            );
            let _ = emit_agent_event(
                &app,
                &workspace_id,
                &conversation_id,
                &AgentEvent::Error {
                    message: message.clone(),
                },
            );
            Err(message)
        }
    };

    let _ = emit_agent_event(
        &app,
        &workspace_id,
        &conversation_id,
        &AgentEvent::TurnFinished,
    );
    state.active_turns.lock().await.remove(&conversation_id);
    state
        .active_turn_details
        .lock()
        .map(|mut active| active.remove(&conversation_id))
        .ok();
    emit_active_turns_changed(&app, &state.active_turn_details).await;

    command_result
}

#[tauri::command]
pub(super) async fn cancel_turn(
    state: State<'_, DesktopState>,
    input: ConversationInput,
) -> std::result::Result<bool, String> {
    let sender = state
        .active_turns
        .lock()
        .await
        .get(&input.conversation_id)
        .cloned();

    Ok(match sender {
        Some(sender) => sender.cancel_all(),
        None => false,
    })
}

#[tauri::command]
pub(super) async fn answer_question(
    state: State<'_, DesktopState>,
    input: AnswerQuestionInput,
) -> std::result::Result<bool, String> {
    let _workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let sender = state
        .active_turns
        .lock()
        .await
        .get(&input.conversation_id)
        .cloned();

    Ok(sender
        .map(|sender| {
            sender.answer_question(&input.tool_call_id, input.answers, input.stop_questions)
        })
        .unwrap_or(false))
}

#[tauri::command]
pub(super) async fn reject_question(
    state: State<'_, DesktopState>,
    input: RejectQuestionInput,
) -> std::result::Result<bool, String> {
    let _workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let sender = state
        .active_turns
        .lock()
        .await
        .get(&input.conversation_id)
        .cloned();

    Ok(sender
        .map(|sender| sender.reject_question(&input.tool_call_id))
        .unwrap_or(false))
}

#[tauri::command]
pub(super) async fn list_active_turns(
    state: State<'_, DesktopState>,
) -> std::result::Result<Vec<ActiveTurnSummary>, String> {
    let active = state
        .active_turn_details
        .lock()
        .map_err(|_| "active turn state is unavailable".to_string())?;
    Ok(active_turn_summaries_from_map(&active))
}

#[tauri::command]
pub(super) async fn replay_active_turn_events(
    state: State<'_, DesktopState>,
    input: ActiveTurnReplayInput,
) -> std::result::Result<ActiveTurnReplay, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let workspace_id = workspace_root.display().to_string();
    let after_sequence = input.after_sequence.unwrap_or(0);
    let active = state
        .active_turn_details
        .lock()
        .map_err(|_| "active turn state is unavailable".to_string())?;
    let Some(record) = active.get(&input.conversation_id) else {
        return Ok(ActiveTurnReplay {
            active: false,
            workspace_id,
            conversation_id: input.conversation_id,
            started_at_ms: None,
            latest_sequence: 0,
            events: Vec::new(),
        });
    };

    if record.workspace_id != workspace_id {
        return Ok(ActiveTurnReplay {
            active: false,
            workspace_id,
            conversation_id: input.conversation_id,
            started_at_ms: None,
            latest_sequence: 0,
            events: Vec::new(),
        });
    }

    Ok(ActiveTurnReplay {
        active: true,
        workspace_id: record.workspace_id.clone(),
        conversation_id: record.conversation_id.clone(),
        started_at_ms: Some(record.started_at_ms),
        latest_sequence: record.latest_sequence(),
        events: record
            .events
            .iter()
            .filter(|entry| entry.sequence > after_sequence)
            .cloned()
            .collect(),
    })
}

pub(super) async fn wait_for_conversation_turn_slot(
    active_turns: &Arc<Mutex<HashMap<String, TurnCancel>>>,
    conversation_id: &str,
) -> bool {
    wait_for_conversation_turn_slot_with_attempts(
        active_turns,
        conversation_id,
        TURN_SLOT_WAIT_ATTEMPTS,
    )
    .await
}

pub(super) async fn wait_for_conversation_turn_slot_with_attempts(
    active_turns: &Arc<Mutex<HashMap<String, TurnCancel>>>,
    conversation_id: &str,
    attempts: usize,
) -> bool {
    for attempt in 0..attempts {
        let is_busy = active_turns.lock().await.contains_key(conversation_id);
        if !is_busy {
            return true;
        }
        if attempt + 1 < attempts {
            tokio::time::sleep(Duration::from_millis(TURN_SLOT_WAIT_INTERVAL_MS)).await;
        }
    }
    false
}

pub(super) fn emit_agent_event(
    app: &AppHandle,
    workspace_id: &str,
    conversation_id: &str,
    event: &AgentEvent,
) -> Result<()> {
    let sequence = remember_active_turn_event(app, conversation_id, event.clone());
    app.emit(
        AGENT_EVENT_NAME,
        ConversationEvent {
            workspace_id: workspace_id.to_string(),
            conversation_id: conversation_id.to_string(),
            sequence,
            event: event.clone(),
        },
    )
    .context("unable to emit agent event")?;
    Ok(())
}

pub(super) async fn register_active_turn(
    app: &AppHandle,
    state: &DesktopState,
    workspace_id: &str,
    conversation_id: &str,
) {
    {
        let mut active = match state.active_turn_details.lock() {
            Ok(active) => active,
            Err(_) => return,
        };
        active.insert(
            conversation_id.to_string(),
            ActiveTurnRecord {
                workspace_id: workspace_id.to_string(),
                conversation_id: conversation_id.to_string(),
                started_at_ms: now_ms(),
                events: Vec::new(),
                next_sequence: 1,
            },
        );
    }
    emit_active_turns_changed(app, &state.active_turn_details).await;
}

pub(super) async fn emit_active_turns_changed(
    app: &AppHandle,
    active_turn_details: &Arc<StdMutex<HashMap<String, ActiveTurnRecord>>>,
) {
    let active_turns = {
        let active = match active_turn_details.lock() {
            Ok(active) => active,
            Err(_) => return,
        };
        active_turn_summaries_from_map(&active)
    };
    let _ = app.emit(
        ACTIVE_TURNS_EVENT_NAME,
        ActiveTurnsChangedPayload { active_turns },
    );
}

pub(super) fn emit_database_sources_changed(app: &AppHandle, settings: &DatabaseSettings) {
    let payload = DatabaseSourcesChangedPayload {
        active_count: settings.active_count(),
        source_count: settings.sources.len(),
    };
    let _ = app.emit(DATABASE_SOURCES_EVENT_NAME, payload);
}

pub(super) fn active_turn_summaries_from_map(
    active: &HashMap<String, ActiveTurnRecord>,
) -> Vec<ActiveTurnSummary> {
    let mut summaries = active
        .values()
        .map(|record| ActiveTurnSummary {
            workspace_id: record.workspace_id.clone(),
            conversation_id: record.conversation_id.clone(),
            started_at_ms: record.started_at_ms,
            latest_sequence: record.latest_sequence(),
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|a, b| {
        a.workspace_id
            .cmp(&b.workspace_id)
            .then_with(|| b.started_at_ms.cmp(&a.started_at_ms))
            .then_with(|| a.conversation_id.cmp(&b.conversation_id))
    });
    summaries
}

fn remember_active_turn_event(
    app: &AppHandle,
    conversation_id: &str,
    event: AgentEvent,
) -> Option<u64> {
    let state = app.try_state::<DesktopState>()?;
    let active_turn_details = state.active_turn_details.clone();
    {
        let mut active = active_turn_details.lock().ok()?;
        let record = active.get_mut(conversation_id)?;
        let sequence = record.next_sequence;
        record.next_sequence = record.next_sequence.saturating_add(1);
        record.events.push(SequencedAgentEvent { sequence, event });
        let overflow = record
            .events
            .len()
            .saturating_sub(ACTIVE_TURN_EVENT_BUFFER_MAX);
        if overflow > 0 {
            record.events.drain(0..overflow);
        }
        Some(sequence)
    }
}

pub(super) fn emit_agent_file_changes(app: &AppHandle, workspace_id: &str, event: &AgentEvent) {
    match event {
        AgentEvent::ToolFinished { file_changes, .. } => {
            for change in file_changes {
                let _ = app.emit(
                    FILE_CHANGE_EVENT_NAME,
                    WorkspaceFileChangeEvent {
                        workspace_path: workspace_id.to_string(),
                        relative_path: change.relative_path.clone(),
                    },
                );
            }
        }
        AgentEvent::SubAgentEvent { event, .. } => {
            emit_agent_file_changes(app, workspace_id, event);
        }
        _ => {}
    }
}

pub(super) fn build_user_message(
    text: &str,
    attachments: &[AttachmentInput],
    workspace_root: &Path,
    plan_control: Option<PlanControlInput>,
    message_visibility: MessageVisibilityInput,
) -> ChatMessage {
    let mut parts = Vec::new();
    let mut context_blocks = Vec::new();
    let mut context_attachments = Vec::new();

    for attachment in attachments.iter().take(8) {
        let path = resolve_attachment_path(workspace_root, &attachment.path);
        let label = attachment_label(attachment, &path);
        let attachment_meta = json!({
            "path": path.display().to_string(),
            "name": label.clone(),
        });
        match prepare_attachment(&path, &label) {
            PreparedAttachment::Image(mut image) => {
                if let Part::Image { meta, .. } = &mut image {
                    *meta = Some(json!({ "attachment": attachment_meta }));
                }
                parts.push(image);
            }
            PreparedAttachment::Context(block) => {
                context_blocks.push(block);
                context_attachments.push(attachment_meta);
            }
        }
    }

    parts.push(Part::Text {
        text: text.to_string(),
        meta: match message_visibility {
            MessageVisibilityInput::Normal => None,
            MessageVisibilityInput::SystemReminder => Some(json!({ "system_reminder": true })),
        },
    });

    if matches!(plan_control, Some(PlanControlInput::StopQuestions)) {
        parts.push(Part::Text {
            text: "\n\n<plan_mode_control action=\"stop_questions\">\nThe user clicked Send and stop questions. Do not ask more questions in this turn. Produce the complete Markdown plan now and do not implement it.\n</plan_mode_control>".to_string(),
            meta: Some(json!({ "plan_control": "stop_questions" })),
        });
    }

    if !context_blocks.is_empty() {
        parts.push(Part::Text {
            text: format!(
                "\n\nAttached file context:\n\n{}",
                context_blocks.join("\n\n")
            ),
            meta: Some(json!({
                "attachment_context": true,
                "attachments": context_attachments,
            })),
        });
    }

    ChatMessage {
        role: Role::User,
        parts,
    }
}

pub(super) fn is_rewritable_user_message(message: &ChatMessage) -> bool {
    message.role == Role::User
        && message.parts.iter().any(|part| match part {
            Part::Text { text, meta } => {
                !text.trim().is_empty() && !is_hidden_rewrite_text(meta.as_ref())
            }
            _ => false,
        })
}

fn is_hidden_rewrite_text(meta: Option<&Value>) -> bool {
    let Some(Value::Object(meta)) = meta else {
        return false;
    };
    meta.get("attachment_context").and_then(Value::as_bool) == Some(true)
        || meta.get("system_reminder").and_then(Value::as_bool) == Some(true)
        || meta.get("ui_only").and_then(Value::as_bool) == Some(true)
        || meta
            .get("compaction_retained_user")
            .and_then(Value::as_bool)
            == Some(true)
        || meta.get("compaction_summary").and_then(Value::as_bool) == Some(true)
        || meta.get("compaction_marker").and_then(Value::as_bool) == Some(true)
        || meta.get("plan_control").and_then(Value::as_str).is_some()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PlanReference {
    pub(super) path: String,
    pub(super) title: Option<String>,
}

pub(super) fn plan_implementation_turn_reminder(
    workspace_root: &Path,
    workflow: &PlanWorkflowState,
    attachments: &[AttachmentInput],
    control: Option<PlanControlInput>,
) -> std::result::Result<Option<String>, String> {
    if !matches!(control, Some(PlanControlInput::ImplementPlan)) {
        return Ok(None);
    }

    let plan = match workflow {
        PlanWorkflowState::PlanReady { artifact } => {
            Some(plan_reference_from_artifact(workspace_root, artifact))
        }
        _ => plan_reference_from_attachments(workspace_root, attachments),
    }
    .ok_or_else(|| "plan implementation requires an attached plan".to_string())?;

    let mut lines = vec![
        "You are implementing this plan for the current turn.".to_string(),
        format!("Plan path: {}", plan.path),
    ];
    if let Some(title) = plan.title.filter(|title| !title.trim().is_empty()) {
        lines.push(format!("Plan title: {}", title.trim()));
    }
    lines.extend([
        "Treat the plan as the source of truth for this implementation run.".to_string(),
        "Use the ToDoList tool to track implementation progress when the plan has multiple steps, and keep it updated until the plan is complete.".to_string(),
        "Read the plan file when you need details, keep changes aligned with it, and complete the implementation before your final response.".to_string(),
    ]);

    Ok(Some(lines.join("\n")))
}

pub(super) fn with_turn_plan_reminder(base: &str, reminder: Option<String>) -> String {
    let Some(reminder) = reminder else {
        return base.to_string();
    };
    format!("{base}\n\n<plan_implementation_turn>\n{reminder}\n</plan_implementation_turn>")
}

pub(super) fn plan_reference_from_artifact(
    workspace_root: &Path,
    artifact: &PlanArtifactState,
) -> PlanReference {
    let path = if !artifact.path.trim().is_empty() {
        artifact.path.clone()
    } else {
        artifact
            .absolute_path
            .as_deref()
            .map(|path| plan_display_path(workspace_root, path))
            .unwrap_or_else(|| "attached plan".to_string())
    };
    PlanReference {
        path,
        title: artifact.title.clone(),
    }
}

pub(super) fn plan_reference_from_attachments(
    workspace_root: &Path,
    attachments: &[AttachmentInput],
) -> Option<PlanReference> {
    let attachment = attachments
        .iter()
        .find(|attachment| attachment_looks_like_plan(attachment))
        .or_else(|| attachments.first())?;
    Some(PlanReference {
        path: plan_display_path(workspace_root, &attachment.path),
        title: attachment.name.clone(),
    })
}

pub(super) fn attachment_looks_like_plan(attachment: &AttachmentInput) -> bool {
    let path = attachment.path.to_ascii_lowercase();
    let name = attachment
        .name
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    path.ends_with(".md")
        || path.contains(".claakecode/plans/")
        || name.ends_with(".md")
        || name.contains("plan")
}

pub(super) fn plan_display_path(workspace_root: &Path, raw: &str) -> String {
    let resolved = resolve_attachment_path(workspace_root, raw);
    resolved
        .strip_prefix(workspace_root)
        .ok()
        .filter(|relative| !relative.as_os_str().is_empty())
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|| {
            if raw.trim().is_empty() {
                resolved.display().to_string()
            } else {
                raw.to_string()
            }
        })
}

pub(super) fn attach_latest_plan_artifact(
    workspace_root: &Path,
    conversation_id: &str,
    history: &mut [ChatMessage],
    turn_user_history_index: usize,
    skip_if_question_tool: bool,
) -> Result<Option<PlanArtifactState>> {
    if skip_if_question_tool && turn_has_question_tool(history, turn_user_history_index) {
        return Ok(None);
    }

    let Some(assistant_index) = latest_assistant_index_after(history, turn_user_history_index)
    else {
        return Ok(None);
    };
    let plan_text = assistant_plan_text(&history[assistant_index]);
    if plan_text.trim().is_empty() {
        return Ok(None);
    }

    let relative_path = latest_plan_artifact_path(history)
        .filter(|path| is_safe_plan_path(path))
        .unwrap_or_else(|| new_plan_relative_path(conversation_id, &plan_text));
    let plan_path = workspace_root.join(&relative_path);
    if let Some(parent) = plan_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("unable to create plan directory {}", parent.display()))?;
    }

    let plan_text = ensure_trailing_newline(plan_text.trim());
    fs::write(&plan_path, &plan_text)
        .with_context(|| format!("unable to write plan {}", plan_path.display()))?;

    mark_plan_source(&mut history[assistant_index]);

    let title = plan_title(&plan_text).unwrap_or_else(|| "Plan created".to_string());
    let updated_at_ms = now_ms();
    let artifact = PlanArtifactState {
        path: relative_path,
        absolute_path: Some(plan_path.display().to_string()),
        title: Some(title),
        updated_at_ms: Some(updated_at_ms),
    };
    history[assistant_index].parts.push(Part::Text {
        text: String::new(),
        meta: Some(json!({
            "plan_artifact": {
                "path": artifact.path.clone(),
                "absolutePath": artifact.absolute_path.clone(),
                "title": artifact.title.clone(),
                "updatedAtMs": artifact.updated_at_ms,
            }
        })),
    });

    Ok(Some(artifact))
}

pub(super) fn turn_has_question_tool(
    history: &[ChatMessage],
    turn_user_history_index: usize,
) -> bool {
    history
        .iter()
        .skip(turn_user_history_index.saturating_add(1))
        .flat_map(|message| &message.parts)
        .any(|part| {
            matches!(
                part,
                Part::ToolCall { name, .. } if name == "Question"
            )
        })
}

pub(super) fn latest_question_stop_requested(
    history: &[ChatMessage],
    turn_user_history_index: usize,
) -> bool {
    history
        .iter()
        .skip(turn_user_history_index.saturating_add(1))
        .flat_map(|message| &message.parts)
        .filter_map(|part| match part {
            Part::ToolResult { meta, .. } => meta.as_ref(),
            _ => None,
        })
        .any(|meta| {
            meta.get("question_stop_requested")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
}

pub(super) fn latest_assistant_index_after(
    history: &[ChatMessage],
    turn_user_history_index: usize,
) -> Option<usize> {
    let start = turn_user_history_index.saturating_add(1);
    (start..history.len())
        .rev()
        .find(|index| matches!(history[*index].role, Role::Assistant))
}

pub(super) fn assistant_plan_text(message: &ChatMessage) -> String {
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            Part::Text { text, .. } if !text.trim().is_empty() => Some(text.trim()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(super) fn mark_plan_source(message: &mut ChatMessage) {
    for part in &mut message.parts {
        let Part::Text { text, meta } = part else {
            continue;
        };
        if text.trim().is_empty() {
            continue;
        }
        insert_meta(meta, "plan_source", Value::Bool(true));
    }
}

pub(super) fn latest_plan_artifact_path(history: &[ChatMessage]) -> Option<String> {
    for message in history.iter().rev() {
        for part in message.parts.iter().rev() {
            let Some(path) = part_meta(part)
                .and_then(|meta| meta.get("plan_artifact"))
                .and_then(|artifact| artifact.get("path"))
                .and_then(Value::as_str)
            else {
                continue;
            };
            return Some(path.to_string());
        }
    }
    None
}

pub(super) fn is_safe_plan_path(path: &str) -> bool {
    if !path.starts_with(".claakecode/plans/") || !path.ends_with(".md") {
        return false;
    }
    Path::new(path)
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
}

pub(super) fn new_plan_relative_path(conversation_id: &str, plan_text: &str) -> String {
    let title = plan_title(plan_text).unwrap_or_else(|| "plan".to_string());
    let slug = slugify(&title);
    let short_id = conversation_id.chars().take(8).collect::<String>();
    format!(".claakecode/plans/{}-{}-{}.md", now_ms(), short_id, slug)
}

pub(super) fn plan_title(plan_text: &str) -> Option<String> {
    plan_text.lines().find_map(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }
        let title = trimmed.trim_start_matches('#').trim();
        (!title.is_empty()).then(|| {
            if title.chars().count() > 80 {
                let mut shortened = title.chars().take(77).collect::<String>();
                shortened.push_str("...");
                shortened
            } else {
                title.to_string()
            }
        })
    })
}

pub(super) fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in value.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            slug.push(lower);
            last_dash = false;
        } else if !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
        if slug.len() >= 48 {
            break;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "plan".to_string()
    } else {
        slug
    }
}

pub(super) fn ensure_trailing_newline(mut value: &str) -> String {
    value = value.trim_end();
    let mut output = value.to_string();
    output.push('\n');
    output
}

pub(super) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

pub(super) fn insert_meta(meta: &mut Option<Value>, key: &str, value: Value) {
    let mut map = match meta.take() {
        Some(Value::Object(map)) => map,
        Some(previous) => {
            let mut map = serde_json::Map::new();
            map.insert("previous_meta".into(), previous);
            map
        }
        None => serde_json::Map::new(),
    };
    map.insert(key.to_string(), value);
    *meta = Some(Value::Object(map));
}

pub(super) fn tool_descriptors_for_workspace(
    workspace_root: &Path,
    mode: AgentMode,
    skill_settings: &SkillSettings,
) -> Vec<ToolDescriptor> {
    let bash = BashTool::new(workspace_root);
    let mut tools = vec![
        bash.descriptor(),
        bash.input_descriptor(),
        GlobTool::new(workspace_root).descriptor(),
        GrepTool::new(workspace_root).descriptor(),
        ReadTool::new(workspace_root).descriptor(),
        clean_context_descriptor(),
        ToDoListTool::new().descriptor(),
        QuestionTool::new().descriptor(),
        WebSearchTool::new().descriptor(),
        WebFetchTool::new().descriptor(),
    ];
    if let Some(descriptor) =
        SkillTool::with_settings(workspace_root, skill_settings.clone()).descriptor()
    {
        tools.push(descriptor);
    }
    if mode != AgentMode::Plan {
        tools.insert(4, EditFileTool::new(workspace_root).descriptor());
        tools.insert(5, WriteFileTool::new(workspace_root).descriptor());
        tools.push(CreateImageTool::new(workspace_root).descriptor());
    }
    tools
}

pub(super) fn configurable_tool_catalog(workspace_root: &Path) -> Vec<ToolDescriptor> {
    let mut tools =
        tool_descriptors_for_workspace(workspace_root, AgentMode::Act, &SkillSettings::default());
    tools.retain(|tool| tool.name != "skill");
    tools.extend(TeamTool::descriptors_static());
    tools.extend(TeamTool::agent_descriptors_static());
    tools
}

pub(super) fn system_prompt_for_workspace(workspace_root: &Path, base: &str) -> Result<String> {
    let mut sections = vec![format!("# Shell environment\n\n{}", shell_system_prompt())];

    if let Some(instructions) =
        read_workspace_prompt_file(workspace_root, WORKSPACE_INSTRUCTIONS_FILE)?
    {
        sections.push(format!(
            "# Workspace instructions\n\nThe following instructions come from the current workspace and should be treated as the project source of truth.\n\n{instructions}"
        ));
    }

    if let Some(design) = read_workspace_prompt_file(workspace_root, WORKSPACE_DESIGN_FILE)? {
        sections.push(format!(
            "# Workspace design context\n\nThe following design guidance comes from the current workspace and should guide product, UX, visual, and frontend decisions.\n\n{design}"
        ));
    }

    Ok(format!("{base}\n\n{}", sections.join("\n\n")))
}

pub(super) fn read_workspace_prompt_file(
    workspace_root: &Path,
    file_name: &str,
) -> Result<Option<String>> {
    let path = workspace_root.join(file_name);
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(err) => {
            return Err(err).with_context(|| {
                format!("unable to read workspace prompt file at {}", path.display())
            });
        }
    };

    let contents = contents.trim();
    if contents.is_empty() {
        return Ok(None);
    }

    Ok(Some(contents.to_string()))
}

pub(super) fn resolve_attachment_path(workspace_root: &Path, raw: &str) -> std::path::PathBuf {
    let path = Path::new(raw);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    }
}

pub(super) fn attachment_label(attachment: &AttachmentInput, path: &Path) -> String {
    attachment
        .name
        .clone()
        .or_else(|| {
            path.file_name()
                .and_then(|value| value.to_str())
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| attachment.path.clone())
}

pub(super) enum PreparedAttachment {
    Image(Part),
    Context(String),
}

pub(super) fn prepare_attachment(path: &Path, label: &str) -> PreparedAttachment {
    let Some(media_type) = supported_image_media_type(path) else {
        return PreparedAttachment::Context(read_attachment_block(path, label));
    };

    let intro = format!("<attachment path=\"{}\">", path.display());
    match fs::read(path) {
        Ok(bytes) => {
            if bytes.len() > MAX_IMAGE_BYTES {
                return PreparedAttachment::Context(format!(
                    "{intro}\n[Image too large to send visually: {label}]\n</attachment>"
                ));
            }

            PreparedAttachment::Image(Part::Image {
                media_type: media_type.to_string(),
                data: BASE64_STANDARD.encode(bytes),
                meta: None,
            })
        }
        Err(err) => PreparedAttachment::Context(format!(
            "{intro}\n[Unable to read image {label}: {err}]\n</attachment>"
        )),
    }
}

pub(super) fn supported_image_media_type(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

pub(super) fn clipboard_image_type(
    media_type: &str,
    name: Option<&str>,
) -> Option<(&'static str, &'static str)> {
    let normalized = media_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match normalized.as_str() {
        "image/png" => return Some(("image/png", "png")),
        "image/jpeg" | "image/jpg" => return Some(("image/jpeg", "jpg")),
        "image/gif" => return Some(("image/gif", "gif")),
        "image/webp" => return Some(("image/webp", "webp")),
        _ => {}
    }

    let ext = Path::new(name?).extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some(("image/png", "png")),
        "jpg" | "jpeg" => Some(("image/jpeg", "jpg")),
        "gif" => Some(("image/gif", "gif")),
        "webp" => Some(("image/webp", "webp")),
        _ => None,
    }
}

pub(super) fn clipboard_image_display_name(name: Option<&str>, extension: &str) -> String {
    let raw = name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("pasted-image");
    let stem = Path::new(raw)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("pasted-image");
    format!("{}.{}", safe_temp_file_stem(stem), extension)
}

pub(super) fn safe_temp_file_stem(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if out.len() >= 72 {
            break;
        }
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            out.push(ch);
        } else if ch.is_whitespace() && !out.ends_with('-') {
            out.push('-');
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "pasted-image".to_string()
    } else {
        out
    }
}

pub(super) fn read_attachment_block(path: &Path, label: &str) -> String {
    let intro = format!("<attachment path=\"{}\">", path.display());

    match fs::read(path) {
        Ok(bytes) => {
            if bytes.contains(&0) || std::str::from_utf8(&bytes).is_err() {
                return format!("{intro}\n[Binary file attached: {label}]\n</attachment>");
            }

            let truncated = bytes.len() > MAX_ATTACHMENT_BYTES;
            let slice = &bytes[..bytes.len().min(MAX_ATTACHMENT_BYTES)];
            let mut content = String::from_utf8_lossy(slice).into_owned();
            if truncated {
                content.push_str("\n\n[truncated]");
            }

            format!("{intro}\n{content}\n</attachment>")
        }
        Err(err) => format!("{intro}\n[Unable to read {label}: {err}]\n</attachment>"),
    }
}

pub(super) fn restore_workspace_for_rewrite(
    app: &AppHandle,
    store: &AppStore,
    workspace_root: &Path,
    conversation_id: &str,
    history_index: usize,
) -> Result<()> {
    let checkpoint_records = store
        .load_turn_checkpoints_from(conversation_id, history_index)
        .context("unable to load turn checkpoints")?;
    let checkpoints = checkpoint_records
        .into_iter()
        .map(|record| record.checkpoint)
        .collect::<Vec<_>>();
    let restored_paths = restore_turn_checkpoints(workspace_root, &checkpoints)
        .context("unable to restore workspace checkpoint")?;
    store
        .delete_turn_checkpoints_from(conversation_id, history_index)
        .context("unable to delete old turn checkpoints")?;
    for relative_path in restored_paths {
        emit_workspace_file_change(app, workspace_root, &relative_path);
    }
    Ok(())
}
