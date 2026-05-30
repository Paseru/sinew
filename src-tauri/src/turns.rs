use crate::*;

const COMPACT_DISPLAY_PROMPT: &str = "\
Display mode: Compact. Keep visible assistant text concise. Do not narrate routine tool use or internal reasoning. Prefer a short final answer with outcome, changed files, validation, and next step.";

const VERY_COMPACT_DISPLAY_PROMPT: &str = "\
Display mode: Very compact. Before the final answer, avoid progress narration and reasoning prose. Use tools silently unless you are blocked or must ask the user a required question. If a long operation truly needs a status update, use one short sentence. Keep the final answer ultra-concise (1-4 bullets/sentences), action-oriented, and mention only the outcome, key changed files, validation, and next step if useful. Never output empty paragraphs or blank lines in your visible responses.";

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OptimizePromptInput {
    pub text: String,
    pub model: Option<crate::models::ModelInput>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OptimizePromptOutput {
    pub mode: String,
    pub rewritten_prompt: String,
}

#[tauri::command]
pub(super) async fn optimize_prompt(
    state: State<'_, DesktopState>,
    input: OptimizePromptInput,
) -> std::result::Result<OptimizePromptOutput, String> {
    let text = input.text.trim();
    if text.is_empty() {
        return Err("prompt cannot be empty".into());
    }

    let model = match input.model {
        Some(m) => sinew_core::ModelRef {
            provider: m.provider.clone(),
            name: m.name.clone(),
            effort: None,
        },
        None => state.default_model.clone(),
    };

    let provider_ref = {
        let providers_guard = state.providers.lock().unwrap();
        providers_guard
            .get(&model.provider)
            .ok_or_else(|| format!("provider {} not found", model.provider))?
            .clone()
    };

    let system_prompt = "Vous êtes un Prompt Engineer expert pour un agent de développement SOTA.
Votre tâche est d'analyser le brouillon de l'utilisateur, de déterminer le mode d'exécution idéal, et de réécrire son brouillon en une consigne claire, structurée et exhaustive.

Modes disponibles:
- act (Action) : pour une tâche simple, un correctif, ou une question qui peut être réglée dans l'immédiat en 1 ou 2 requêtes.
- plan (Plan) : UNIQUEMENT pour concevoir une architecture complexe, rédiger un document de conception avant de coder, ou planifier un projet entier en plusieurs étapes.
- goal (Objectif) : pour un chantier autonome massif, le développement complet d'une fonctionnalité complexe de A à Z nécessitant plusieurs sessions de code.

Règles de réécriture:
- Soyez concis, professionnel et direct.
- Conservez un langage naturel et métier.
- Formulez sous forme d'instructions claires ou de description factuelle du besoin.
- N'inventez pas de choix techniques non suggérés par l'utilisateur, mais structurez ceux présents.

Répondez EXACTEMENT dans ce format texte (sans JSON, sans Markdown, sans commentaire) :
MODE: act
===PROMPT===
Votre texte réécrit ici (il peut tenir sur plusieurs lignes).

La première ligne commence par 'MODE:' suivi de act, plan ou goal.
La ligne suivante est exactement '===PROMPT==='.
Tout ce qui suit ce marqueur est le prompt réécrit, en texte brut.";

    let messages = vec![sinew_core::message::ChatMessage::user_text(text.to_string())];
    
    let request = sinew_core::provider::ProviderRequest::new(model, messages)
        .with_system(system_prompt.to_string());

    let mut stream = provider_ref.stream(request).await.map_err(error_to_string)?;
    let mut response_text = String::new();

    use futures::StreamExt;
    while let Some(event) = stream.next().await {
        match event.map_err(error_to_string)? {
            sinew_core::stream::StreamEvent::TextDelta { delta, .. } => {
                response_text.push_str(&delta);
            }
            _ => {}
        }
    }

    parse_optimize_response(&response_text, text)
}

/// Extrait le mode et le prompt réécrit de la réponse du modèle.
///
/// On tolère plusieurs formats car les LLM produisent rarement un JSON
/// strictement valide quand le texte réécrit est multi-lignes :
/// 1. Format délimité `MODE:` + `===PROMPT===` (format demandé, robuste).
/// 2. JSON `{ "mode": ..., "rewritten_prompt": ... }` (compatibilité).
/// 3. Texte brut exploitable en dernier recours, pour ne jamais échouer
///    silencieusement et renvoyer le brouillon original tel quel.
fn parse_optimize_response(
    raw: &str,
    original: &str,
) -> std::result::Result<OptimizePromptOutput, String> {
    let normalize_mode = |value: &str| -> Option<String> {
        let lower = value.trim().to_ascii_lowercase();
        let token = lower
            .split(|c: char| !c.is_ascii_alphabetic())
            .find(|s| matches!(*s, "act" | "plan" | "goal"))?;
        Some(token.to_string())
    };

    // 1. Format délimité.
    if let Some(marker) = raw.find("===PROMPT===") {
        let head = &raw[..marker];
        let body = raw[marker + "===PROMPT===".len()..].trim_matches(|c| c == '\r' || c == '\n');
        let mode = head
            .lines()
            .find_map(|line| {
                let trimmed = line.trim();
                if trimmed.to_ascii_uppercase().starts_with("MODE:") {
                    normalize_mode(&trimmed[5..])
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "act".to_string());
        let rewritten = body.trim();
        if !rewritten.is_empty() {
            return Ok(OptimizePromptOutput {
                mode,
                rewritten_prompt: rewritten.to_string(),
            });
        }
    }

    // 2. JSON (compatibilité ascendante).
    let mut json_text = raw.trim();
    if let Some(start) = json_text.find('{') {
        if let Some(end) = json_text.rfind('}') {
            json_text = &json_text[start..=end];
        }
    }
    if let Ok(output) = serde_json::from_str::<OptimizePromptOutput>(json_text) {
        if !output.rewritten_prompt.trim().is_empty() {
            return Ok(OptimizePromptOutput {
                mode: normalize_mode(&output.mode).unwrap_or_else(|| "act".to_string()),
                rewritten_prompt: output.rewritten_prompt,
            });
        }
    }

    // 3. Dernier recours : texte brut non vide et différent du brouillon.
    let fallback = raw.trim();
    if !fallback.is_empty() && fallback != original.trim() {
        return Ok(OptimizePromptOutput {
            mode: normalize_mode(fallback).unwrap_or_else(|| "act".to_string()),
            rewritten_prompt: fallback.to_string(),
        });
    }

    Err(format!(
        "Échec de l'optimisation : réponse du modèle inexploitable.\nRaw: {}",
        raw.trim()
    ))
}

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
        system_prompt_for_workspace(
            &workspace_root,
            &state.system_prompt,
            input.git_automation,
            input.concise_answers,
            input.agent_autonomy,
            input.force_changelog,
            input.git_french_messages,
            input.auto_mockups,
            input.strict_problem_solving,
            input.full_implementation,
            input.client_formatted_date_time.as_deref(),
        )
        .map_err(error_to_string)?;
    if !wait_for_conversation_turn_slot(&state.active_turns, &input.conversation_id).await {
        return Err("a turn is already running for this conversation".into());
    }

    let project_id = crate::workspace::resolve_project_id_str(&workspace_id);
    let mut conversation = state
        .store
        .load_conversation(&project_id, &input.conversation_id)
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
    let turn_system_prompt = with_display_mode_prompt(
        &with_turn_plan_reminder(&effective_system_prompt, turn_plan_reminder),
        input.display_mode,
    );
    let mut mode_model_settings = conversation.mode_model_settings.clone();
    let selected_model = model_with_optional_selection(
        mode_model_settings.get(policy.mode),
        input.model,
        input.thinking,
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
    let (steering_tx, steering_rx) = mpsc::unbounded_channel();
    let cancel = TurnCancel::with_steering(cmd_tx, steering_tx);
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
            let active_turn_inputs = state.active_turn_inputs.clone();
            let active_turn_details = state.active_turn_details.clone();
            let app = app.clone();
            let conversation_id = input.conversation_id.clone();
            tauri::async_runtime::spawn(async move {
                active_turns.lock().await.remove(&conversation_id);
                active_turn_inputs.lock().await.remove(&conversation_id);
                active_turn_details
                    .lock()
                    .map(|mut active| active.remove(&conversation_id))
                    .ok();
                emit_active_turns_changed(&app, &active_turn_details).await;
            });
            error_to_string(err)
        })?;

    let providers = provider_registry_snapshot(&state)?;
    let context = TurnContext {
        provider,
        workspace_root: workspace_root.clone(),
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
        list_dir: Arc::new(ListDirTool::new(workspace_root.clone())),
        grep: Arc::new(GrepTool::new(workspace_root.clone())),
        codebase_search: Arc::new(CodebaseSearchTool::new(workspace_root.clone())),
        check_sota: Arc::new(CheckSotaTool::new()),
        computer_use: Arc::new(ComputerUseTool::new()),
        read: Arc::new(ReadTool::new(workspace_root.clone())),
        edit_file: Arc::new(EditFileTool::new(workspace_root.clone())),
        write_file: Arc::new(WriteFileTool::new(workspace_root.clone())),
        delete_file: Arc::new(DeleteFileTool::new(workspace_root.clone())),
        read_lints: Arc::new(ReadLintsTool::new(
            workspace_root.clone(),
            state.editor_diagnostics.clone(),
        )),
        create_image: Arc::new(CreateImageTool::with_settings(
            workspace_root.clone(),
            tool_settings.image_provider,
            tool_settings.openai_image_use_subscription,
            tool_settings.gemini_image_use_subscription,
            Some(tool_settings.openai_image_model.clone()),
            Some(tool_settings.gemini_image_model.clone()),
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
        mcp: Arc::new(McpToolRegistry::new(mcp_settings.clone())),
        subagents: Some(Arc::new(SubAgentTool::new(
            workspace_root.clone(),
            turn_system_prompt.clone(),
            providers.clone(),
            sub_agent_settings.clone(),
            mcp_settings.clone(),
            tool_settings.clone(),
            skill_settings.clone(),
            state.max_tool_rounds,
            service_tier,
            cancel.clone(),
            state.editor_diagnostics.clone(),
        ))),
        teams: Some(Arc::new(TeamTool::new(
            conversation.id.clone(),
            workspace_root.clone(),
            turn_system_prompt.clone(),
            providers,
            sub_agent_settings.clone(),
            mcp_settings.clone(),
            tool_settings.clone(),
            skill_settings.clone(),
            conversation.model.clone(),
            state.max_tool_rounds,
            service_tier,
            state.team_runtime.clone(),
            state.editor_diagnostics.clone(),
            cancel.clone(),
        ))),
        tool_settings: tool_settings.clone(),
        event_scope: None,
        max_tool_rounds: state.max_tool_rounds,
        event_tx,
        cancel,
        cmd_rx,
        steering_rx: Some(steering_rx),
    };

    let store = state.store.clone();
    let active_turns = state.active_turns.clone();
    let active_turn_inputs = state.active_turn_inputs.clone();
    let active_turn_details = state.active_turn_details.clone();
    state.active_turn_inputs.lock().await.insert(
        conversation.id.clone(),
        ActiveTurnInputRecord {
            workspace_id: workspace_id.clone(),
            conversation_id: conversation.id.clone(),
            workspace_root: workspace_root.clone(),
        },
    );
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

    let mcp_settings_clone = mcp_settings.clone();
    let tool_settings_clone = tool_settings.clone();
    let skill_settings_clone = skill_settings.clone();
    let sub_agent_settings_clone = sub_agent_settings.clone();

    tauri::async_runtime::spawn(async move {
        let conversation_id_clone = conversation_id.clone();
        let workspace_id_clone = workspace_id.clone();
        let model_name_clone = conversation_model.name.clone();
        let provider_clone = conversation_model.provider.clone();
        let event_tx_clone = context.event_tx.clone();

        let mut engine = Box::pin(tauri::async_runtime::spawn(async move {
            #[cfg(windows)]
            {
                let run_res = run_turn_via_daemon(
                    &context,
                    &conversation_id_clone,
                    &workspace_id_clone,
                    &model_name_clone,
                    &provider_clone,
                    &mcp_settings_clone,
                    &tool_settings_clone,
                    &skill_settings_clone,
                    &sub_agent_settings_clone,
                    event_tx_clone,
                ).await;
                match run_res {
                    Ok(output) => output,
                    Err(e) => {
                        tracing::debug!("Failed to run turn via daemon, falling back to local runner: {:?}", e);
                        run_turn(context).await
                    }
                }
            }
            #[cfg(not(windows))]
            {
                run_turn(context).await
            }
        }));
        let mut engine_done = false;
        let mut events_done = false;

        loop {
            tokio::select! {
                event = event_rx.recv(), if !events_done => {
                    match event {
                        Some(event) => {
                            if matches!(event, AgentEvent::TurnFinished { .. }) {
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
                            let turn_duration_ms = goal_workflow_duration_ms(&goal_workflow);
                            let saved = SavedConversation {
                                id: conversation_id.clone(),
                                workspace_id: workspace_id.clone(),
                                git_remote_url: crate::git::get_git_remote_url(std::path::Path::new(&workspace_id)),
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
                            let turn_finished_event = AgentEvent::TurnFinished {
                                duration_ms: turn_duration_ms,
                            };
                            let _ = emit_agent_event(
                                &app,
                                &workspace_id,
                                &conversation_id,
                                &turn_finished_event,
                            );
                            active_turns.lock().await.remove(&conversation_id);
                            active_turn_inputs.lock().await.remove(&conversation_id);
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
                                &AgentEvent::TurnFinished { duration_ms: None },
                            );
                            active_turns.lock().await.remove(&conversation_id);
                            active_turn_inputs.lock().await.remove(&conversation_id);
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
        system_prompt_for_workspace(&workspace_root, &state.system_prompt, true, true, true, false, false, false, false, false, None)
            .map_err(error_to_string)?;
    if !wait_for_conversation_turn_slot(&state.active_turns, &input.conversation_id).await {
        return Err("a turn is already running for this conversation".into());
    }

    let project_id = crate::workspace::resolve_project_id_str(&workspace_id);
    let mut conversation = state
        .store
        .load_conversation(&project_id, &input.conversation_id)
        .map_err(error_to_string)?
        .ok_or_else(|| "conversation not found".to_string())?;
    if conversation.history.is_empty() {
        return Err("conversation has no history to compact".into());
    }

    let selected_model =
        model_with_optional_selection(&conversation.model, input.model, input.thinking);
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
        &AgentEvent::TurnFinished {
            duration_ms: goal_workflow_duration_ms(&conversation.goal_workflow),
        },
    );
    state.active_turns.lock().await.remove(&conversation_id);
    state
        .active_turn_inputs
        .lock()
        .await
        .remove(&conversation_id);
    state
        .active_turn_details
        .lock()
        .map(|mut active| active.remove(&conversation_id))
        .ok();
    emit_active_turns_changed(&app, &state.active_turn_details).await;

    command_result
}

#[tauri::command]
pub(super) async fn steer_turn(
    state: State<'_, DesktopState>,
    input: SteeringInput,
) -> std::result::Result<bool, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let workspace_id = workspace_root.display().to_string();
    let turn_input = state
        .active_turn_inputs
        .lock()
        .await
        .get(&input.conversation_id)
        .cloned();
    let Some(turn_input) = turn_input else {
        return Ok(false);
    };
    if turn_input.workspace_id != workspace_id
        || turn_input.workspace_root != workspace_root
        || turn_input.conversation_id != input.conversation_id
    {
        return Ok(false);
    }
    let sender = state
        .active_turns
        .lock()
        .await
        .get(&input.conversation_id)
        .cloned();
    let Some(sender) = sender else {
        return Ok(false);
    };
    let text = input.text.trim();
    if text.is_empty() {
        return Err("steering message cannot be empty".into());
    }
    Ok(sender.steer(
        input.id,
        build_user_message(
            text,
            &input.attachments,
            &workspace_root,
            None,
            MessageVisibilityInput::Normal,
        ),
    ))
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
        events: if after_sequence == 0 {
            record.replay_events.clone()
        } else {
            record
                .events
                .iter()
                .filter(|entry| entry.sequence > after_sequence)
                .cloned()
                .collect()
        },
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
                replay_events: Vec::new(),
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
        record.events.push(SequencedAgentEvent {
            sequence,
            event: event.clone(),
        });
        remember_active_turn_replay_event(record, sequence, event);
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

fn remember_active_turn_replay_event(
    record: &mut ActiveTurnRecord,
    sequence: u64,
    event: AgentEvent,
) {
    if let Some(previous) = record.replay_events.last_mut() {
        if merge_replay_event(&mut previous.event, &event) {
            previous.sequence = sequence;
            return;
        }
    }
    record
        .replay_events
        .push(SequencedAgentEvent { sequence, event });
}

fn merge_replay_event(existing: &mut AgentEvent, incoming: &AgentEvent) -> bool {
    match (existing, incoming) {
        (AgentEvent::TextChunk { delta: existing }, AgentEvent::TextChunk { delta })
        | (AgentEvent::ThinkingChunk { delta: existing }, AgentEvent::ThinkingChunk { delta }) => {
            existing.push_str(delta);
            true
        }
        (
            AgentEvent::ToolArgsDelta {
                id: existing_id,
                delta: existing,
            },
            AgentEvent::ToolArgsDelta { id, delta },
        )
        | (
            AgentEvent::ToolOutputDelta {
                id: existing_id,
                delta: existing,
            },
            AgentEvent::ToolOutputDelta { id, delta },
        ) if existing_id == id => {
            existing.push_str(delta);
            true
        }
        (
            AgentEvent::SubAgentEvent {
                id: existing_id,
                event: existing_event,
                ..
            },
            AgentEvent::SubAgentEvent { id, event, .. },
        ) if existing_id == id => merge_replay_event(existing_event.as_mut(), event.as_ref()),
        _ => false,
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

pub(super) fn with_display_mode_prompt(base: &str, display_mode: DisplayModeInput) -> String {
    match display_mode {
        DisplayModeInput::Disabled => base.to_string(),
        DisplayModeInput::Compact => {
            format!("{base}\n\n<display_mode>\n{COMPACT_DISPLAY_PROMPT}\n</display_mode>")
        }
        DisplayModeInput::VeryCompact => {
            format!("{base}\n\n<display_mode>\n{VERY_COMPACT_DISPLAY_PROMPT}\n</display_mode>")
        }
    }
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
        || path.contains(".sinew/plans/")
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
    if !path.starts_with(".sinew/plans/") || !path.ends_with(".md") {
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
    format!(".sinew/plans/{}-{}-{}.md", now_ms(), short_id, slug)
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
        CodebaseSearchTool::new(workspace_root).descriptor(),
        ReadTool::new(workspace_root).descriptor(),
        ReadLintsTool::new(workspace_root, new_editor_diagnostics_store()).descriptor(),
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

pub(super) fn system_prompt_for_workspace(
    workspace_root: &Path,
    base: &str,
    git_automation: bool,
    concise_answers: bool,
    agent_autonomy: bool,
    force_changelog: bool,
    git_french_messages: bool,
    auto_mockups: bool,
    strict_problem_solving: bool,
    full_implementation: bool,
    client_formatted_date_time: Option<&str>,
) -> Result<String> {
    let mut sections = vec![format!("# Shell environment\n\n{}", shell_system_prompt())];

    if git_automation {
        sections.push(format!(
            "# Git & Background Automation\n\nGit Automation is enabled. Please follow these rules strictly:\n\n{}",
            crate::state::DEFAULT_GIT_AUTOMATION_PROMPT
        ));
    }

    if concise_answers {
        sections.push(format!(
            "# Concise & Simplified Answers\n\nConcise Answers Mode is enabled. Please follow these rules strictly:\n\n{}",
            crate::state::DEFAULT_CONCISE_ANSWERS_PROMPT
        ));
    }

    if agent_autonomy {
        sections.push(format!(
            "# Agent Autonomy Instructions\n\nAgent Autonomy is enabled. Please follow these rules strictly:\n\n{}",
            crate::state::DEFAULT_AGENT_AUTONOMY_PROMPT
        ));
    }

    if force_changelog {
        let date_time_str = client_formatted_date_time.unwrap_or("current local time");
        sections.push(format!(
            "# Mandatory Changelog Rule\n\n\
            IMPORTANT: The 'Mandatory Changelog' option is enabled for this project. \
            Every single time you modify one or more files in this workspace (using edit_file, write_file, bash, or any other tool):\n\
            1. You MUST update (or create if it does not exist) the `CHANGELOG.md` file located at the root of the project.\n\
            2. Every file modification must be logged in `CHANGELOG.md` under the correct precise date and time. The current local system time is: {}.\n\
            3. Clearly document what was changed and why for each file.\n\
            4. Updating `CHANGELOG.md` is a strict requirement and must be done in the exact same turn/action as the file modification. Do not omit this step or wait for the end of the conversation!",
            date_time_str
        ));
    }

    if git_french_messages {
        sections.push(format!(
            "# Simple French Git Commit Messages\n\n\
            IMPORTANT: Every single time you make a Git commit in this project, you MUST write the commit message in clear, jargon-free, simple French. \
            Describe the change or feature in plain business/user terms (for example, `git commit -m \"Ajout du bouton de changelog dans les options\"` instead of `git commit -m \"feat(options): add changelog button\"`). \
            Never use English, technical abbreviations, or pure developer jargon."
        ));
    }

    if auto_mockups {
        sections.push(format!(
            "# Automatic Visual Mockups\n\n\
            IMPORTANT: Proactively generate Mermaid diagrams and visual flowcharts to help the user visualize your logic, architectural changes, or complex processes. You should default to illustrating your explanations whenever it adds value for a power user. However, do not block the actual file editing to ask for validation on UI mockups every single time you edit a simple frontend file."
        ));
    }

    if strict_problem_solving {
        sections.push(crate::state::DEFAULT_STRICT_PROBLEM_SOLVING_PROMPT.to_string());
    }

    if full_implementation {
        sections.push(crate::state::DEFAULT_FULL_IMPLEMENTATION_PROMPT.to_string());
    }

    // Always inject SSH Optimization Strategy. The agent will ignore it if not on SSH.
    sections.push(crate::state::DEFAULT_SSH_OPTIMIZATION_PROMPT.to_string());

    // Inject machine-wide global consolidated rules if present
    let mut global_rules_content = None;
    
    // 1. Try reading directly from OneDrive for real-time Multi-PC updates without restart
    let onedrive = std::env::var("ONEDRIVE").unwrap_or_else(|_| {
        std::env::var("USERPROFILE")
            .map(|u| format!("{}\\OneDrive", u))
            .unwrap_or_default()
    });
    
    if !onedrive.is_empty() {
        let onedrive_rules = std::path::PathBuf::from(&onedrive)
            .join("Documents")
            .join("Sinew")
            .join("instructions_consolidated.md");
            
        if let Ok(content) = std::fs::read_to_string(&onedrive_rules) {
            global_rules_content = Some(content);
        }
    }
    
    // 2. Fallback to LocalAppData if OneDrive is not configured or file missing
    if global_rules_content.is_none() {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let global_rules_path = std::path::PathBuf::from(local_app_data)
                .join("Sinew")
                .join("instructions_consolidated.md");
            if let Ok(content) = std::fs::read_to_string(&global_rules_path) {
                global_rules_content = Some(content);
            }
        }
    }

    if let Some(global_rules) = global_rules_content {
        sections.push(format!(
            "# Global Consolidated Instructions (Machine-wide Rules)\n\n\
            IMPORTANT: The following machine-wide rules are consolidated from previous agent sessions on this PC. \
            You MUST respect and follow these instructions strictly:\n\n{global_rules}"
        ));
    }

    if let Some(instructions) =
        read_workspace_prompt_file(workspace_root, WORKSPACE_INSTRUCTIONS_FILE)?
    {
        sections.push(format!(
            "# Workspace instructions\n\nThe following instructions come from the current workspace and should be treated as the project source of truth.\n\n{instructions}"
        ));
    }

    if let Some(memory) = read_workspace_prompt_file(workspace_root, WORKSPACE_MEMORY_FILE)? {
        sections.push(format!(
            "# Project memory (persistent across sessions)\n\n\
            IMPORTANT: This is YOUR working memory for this project, written by you in previous sessions. \
            It records key decisions, the current state, what is done and what remains, and gotchas to avoid. \
            Read it first so you do not re-discover the project from scratch. \
            Keep it current: edit `.sinew/memory.md` (concise, dated bullet points) whenever you make a decision, \
            complete a task, or learn something that future sessions must know.\n\n{memory}"
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

#[tauri::command]
pub(super) async fn check_sota_diagnostics() -> std::result::Result<String, String> {
    let tool = CheckSotaTool::new();
    let result = tool.run(serde_json::Value::Null).await;
    if result.is_error {
        Err(result.content)
    } else {
        Ok(result.content)
    }
}

#[cfg(windows)]
async fn run_turn_via_daemon(
    context: &sinew_app::TurnContext,
    conversation_id: &str,
    workspace_path: &str,
    model_name: &str,
    provider: &str,
    mcp_settings: &sinew_app::McpSettings,
    tool_settings: &sinew_app::ToolSettings,
    skill_settings: &sinew_app::SkillSettings,
    sub_agent_settings: &sinew_app::SubAgentSettings,
    event_tx: tokio::sync::mpsc::UnboundedSender<sinew_app::AgentEvent>,
) -> anyhow::Result<sinew_app::TurnOutput> {
    use tokio::io::{AsyncWriteExt, BufReader};
    use tokio::net::windows::named_pipe::ClientOptions;
    use anyhow::Context;

    let pipe_name = r"\\.\pipe\sinew-agent-ipc";

    // Connect or spawn daemon
    let mut client = match ClientOptions::new().open(pipe_name) {
        Ok(c) => c,
        Err(_) => {
            let _ = spawn_daemon();
            tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
            ClientOptions::new().open(pipe_name).context("Failed to connect to agent daemon after spawn")?
        }
    };

    let request = serde_json::json!({
        "type": "start_turn",
        "conversation_id": conversation_id,
        "workspace_path": workspace_path,
        "system_prompt": context.system_prompt,
        "model_name": model_name,
        "provider": provider,
        "history": context.history,
        "todo_list": context.todo_list,
        "goal_workflow": context.goal_workflow,
        "mcp_settings": mcp_settings.clone(),
        "tool_settings": tool_settings.clone(),
        "skill_settings": skill_settings.clone(),
        "sub_agent_settings": sub_agent_settings.clone(),
    });

    let mut req_bytes = serde_json::to_vec(&request)?;
    req_bytes.push(b'\n');
    
    if workspace_path.starts_with("super-ssh://") {
        let mut client = tokio::net::TcpStream::connect("127.0.0.1:47990").await.context("Failed to connect to remote daemon over TCP")?;
        client.write_all(&req_bytes).await?;
        let (reader, _) = tokio::io::split(client);
        return handle_daemon_stream(BufReader::new(reader), event_tx).await;
    }

    client.write_all(&req_bytes).await?;

    let (reader, _) = tokio::io::split(client);
    handle_daemon_stream(BufReader::new(reader), event_tx).await
}

async fn handle_daemon_stream<R: tokio::io::AsyncRead + Unpin>(
    mut reader: tokio::io::BufReader<R>,
    event_tx: tokio::sync::mpsc::UnboundedSender<sinew_app::AgentEvent>,
) -> anyhow::Result<sinew_app::TurnOutput> {
    use tokio::io::AsyncBufReadExt;
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            anyhow::bail!("Connection closed prematurely by daemon");
        }

        let response: serde_json::Value = serde_json::from_str(&line)?;
        let resp_type = response.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match resp_type {
            "event" => {
                if let Some(event_val) = response.get("event") {
                    if let Ok(event) = serde_json::from_value::<sinew_app::AgentEvent>(event_val.clone()) {
                        let _ = event_tx.send(event);
                    }
                }
            }
            "turn_finished" => {
                let success = response.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
                if !success {
                    let err = response.get("error").and_then(|v| v.as_str()).unwrap_or("Daemon execution failed");
                    anyhow::bail!("{}", err);
                }
                if let Some(output_val) = response.get("output") {
                    let history = serde_json::from_value::<Vec<sinew_core::ChatMessage>>(
                        output_val.get("history").cloned().unwrap_or(serde_json::Value::Null)
                    ).unwrap_or_default();
                    let todo_list = serde_json::from_value::<sinew_app::TodoListState>(
                        output_val.get("todo_list").cloned().unwrap_or(serde_json::Value::Null)
                    ).unwrap_or_default();
                    let goal_workflow = serde_json::from_value::<sinew_app::GoalWorkflowState>(
                        output_val.get("goal_workflow").cloned().unwrap_or(serde_json::Value::Null)
                    ).unwrap_or_default();
                    let interrupted = output_val.get("interrupted").and_then(|v| v.as_bool()).unwrap_or(false);
                    let compacted = output_val.get("compacted").and_then(|v| v.as_bool()).unwrap_or(false);

                    return Ok(sinew_app::TurnOutput {
                        history,
                        todo_list,
                        goal_workflow,
                        interrupted,
                        compacted,
                    });
                }
                anyhow::bail!("Daemon finished turn but did not return valid output");
            }
            "error" => {
                let msg = response.get("message").and_then(|v| v.as_str()).unwrap_or("Unknown daemon error");
                anyhow::bail!("Daemon error: {}", msg);
            }
            _ => {}
        }
    }
}

#[cfg(windows)]
fn spawn_daemon() -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let current_exe = std::env::current_exe()?;
    let exe_dir = current_exe.parent().unwrap_or(&current_exe);
    let daemon_exe = exe_dir.join("sinew-agent-daemon.exe");

    let daemon_exe = if daemon_exe.exists() {
        daemon_exe
    } else {
        std::path::PathBuf::from("target/debug/sinew-agent-daemon.exe")
    };

    if !daemon_exe.exists() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "daemon not found"));
    }

    std::process::Command::new(daemon_exe)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}
