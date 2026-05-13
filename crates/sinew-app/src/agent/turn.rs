use std::collections::BTreeSet;

use futures_util::StreamExt;
use serde_json::{json, Map, Value};
use tokio::sync::mpsc;
use uuid::Uuid;

use sinew_core::{
    ChatMessage, Part, PartKind, ProviderRequest, Role, StopReason, StreamEvent, ToolResultImage,
};

use super::{
    assistant_message::AssistantMessageBuilder,
    cancel::EngineCommand,
    clean_context::{clean_context_descriptor, run_clean_context},
    compaction::{
        can_auto_compact_history, is_context_length_error, maybe_auto_compact_history,
        run_auto_compaction,
    },
    context::{AgentMode, TurnContext, TurnOutput},
    events::{attach_token_usage, send_event, send_token_usage_event, AgentEvent, AgentEventScope},
    history::{
        append_interrupted_tool_results, history_with_current_tool_result_ids,
        normalize_tool_call_inputs, repair_missing_tool_results, strip_all_visible_tool_result_ids,
        successful_read_paths,
    },
    mode::{run_update_goal, system_prompt_for_turn, update_goal_descriptor},
    tool_dispatch::{run_tool, should_wait_for_cooperative_cancel},
    tool_summary::{
        display_mcp_server_name, pretty_json, should_stream_tool_args, summarize_tool,
    },
};

use crate::{system_prompt_with_todo, ToolRunResult};

pub async fn run_turn(ctx: TurnContext) -> TurnOutput {
    let TurnContext {
        provider,
        model,
        cache_key,
        mut cache_stable_message_count,
        auto_compact,
        mode,
        stop_questions,
        system_prompt,
        mut history,
        mut todo_list,
        mut goal_workflow,
        bash,
        glob,
        grep,
        read,
        apply_patch,
        create_image,
        todo_list_tool,
        question,
        web_search,
        web_fetch,
        skill,
        mcp,
        subagents,
        teams,
        tool_settings,
        event_scope,
        max_tool_rounds,
        event_tx,
        cancel: _cancel,
        mut cmd_rx,
    } = ctx;

    send_event(&event_tx, event_scope.as_ref(), AgentEvent::TurnStarted);
    strip_all_visible_tool_result_ids(&mut history);
    normalize_tool_call_inputs(&mut history);
    repair_missing_tool_results(&mut history);
    mcp.refresh_catalog(&history).await;

    let mut cancelled = false;
    let mut loops = 0usize;
    let mut auto_compaction_attempts = 0usize;
    let mut current_turn_tool_result_ids = BTreeSet::new();
    let mut read_paths = successful_read_paths(&history, &read);
    todo_list.normalize();

    'conversation: loop {
        if let Some(teams) = &teams {
            if let Some(messages_prompt) = teams.drain_current_agent_messages_prompt().await {
                history.push(ChatMessage {
                    role: Role::User,
                    parts: vec![Part::Text {
                        text: messages_prompt,
                        meta: Some(json!({ "agent_team_messages": true })),
                    }],
                });
            }
        }

        let mut tool_descriptors = vec![
            bash.descriptor(),
            bash.input_descriptor(),
            glob.descriptor(),
            grep.descriptor(),
            read.descriptor(),
            clean_context_descriptor(),
            web_search.descriptor(),
            web_fetch.descriptor(),
        ];
        if let Some(question) = &question {
            tool_descriptors.insert(6, question.descriptor());
        }
        if let Some(todo_list_tool) = &todo_list_tool {
            tool_descriptors.insert(6, todo_list_tool.descriptor());
        }
        if let Some(descriptor) = skill.descriptor() {
            tool_descriptors.push(descriptor);
        }
        if mode != AgentMode::Plan {
            tool_descriptors.insert(4, apply_patch.descriptor());
            tool_descriptors.push(create_image.descriptor());
        }
        if mode == AgentMode::Goal {
            tool_descriptors.push(update_goal_descriptor());
        }
        tool_descriptors.extend(mcp.descriptors().await);
        if let Some(subagents) = &subagents {
            tool_descriptors.extend(subagents.descriptors());
        }
        if let Some(teams) = &teams {
            tool_descriptors.extend(teams.descriptors());
        }
        let tool_descriptors = tool_settings.apply_to_descriptors(tool_descriptors);
        let question_enabled = question.is_some() && tool_settings.is_enabled("Question");

        let mut current_system_prompt = system_prompt_with_todo(&system_prompt, &todo_list);
        if let Some(teams) = &teams {
            if let Some(team_reminder) = teams.current_agent_system_reminder().await {
                current_system_prompt.push_str("\n\n");
                current_system_prompt.push_str(&team_reminder);
            }
        }
        let current_system_prompt =
            system_prompt_for_turn(&current_system_prompt, mode, &goal_workflow);

        if auto_compact {
            match maybe_auto_compact_history(
                &provider,
                &model,
                cache_key.as_ref(),
                &mut cache_stable_message_count,
                &mut history,
                &mut current_turn_tool_result_ids,
                &current_system_prompt,
                &tool_descriptors,
                &event_tx,
                event_scope.as_ref(),
                &mut cmd_rx,
                &mut auto_compaction_attempts,
            )
            .await
            {
                Ok(true) => continue,
                Ok(false) => {}
                Err(err) => {
                    send_event(
                        &event_tx,
                        event_scope.as_ref(),
                        AgentEvent::Error { message: err },
                    );
                    break;
                }
            }
        }

        let request_history =
            history_with_current_tool_result_ids(&history, &current_turn_tool_result_ids);
        let request = ProviderRequest::new(model.clone(), request_history)
            .with_system(current_system_prompt.clone())
            .with_tools(tool_descriptors.clone())
            .with_cache_stable_message_count(cache_stable_message_count);
        let request = match &cache_key {
            Some(cache_key) => request.with_cache_key(cache_key.clone()),
            None => request,
        };

        let mut stream = match provider.stream(request).await {
            Ok(stream) => stream,
            Err(err) => {
                if auto_compact
                    && is_context_length_error(&err)
                    && can_auto_compact_history(&history, auto_compaction_attempts)
                {
                    match run_auto_compaction(
                        &provider,
                        &model,
                        cache_key.as_ref(),
                        &mut cache_stable_message_count,
                        &mut history,
                        &mut current_turn_tool_result_ids,
                        &current_system_prompt,
                        &event_tx,
                        event_scope.as_ref(),
                        &mut cmd_rx,
                        &mut auto_compaction_attempts,
                    )
                    .await
                    {
                        Ok(()) => continue,
                        Err(compaction_err) => {
                            send_event(
                                &event_tx,
                                event_scope.as_ref(),
                                AgentEvent::Error {
                                    message: format!(
                                        "provider error: {err}; context compaction failed: {compaction_err}"
                                    ),
                                },
                            );
                            break;
                        }
                    }
                }
                send_event(
                    &event_tx,
                    event_scope.as_ref(),
                    AgentEvent::Error {
                        message: format!("provider error: {err}"),
                    },
                );
                break;
            }
        };

        let mut message_builder = AssistantMessageBuilder::default();
        let mut stop_reason = StopReason::EndTurn;
        let mut response_usage = None;
        let mut stream_error = None;

        loop {
            tokio::select! {
                biased;

                command = cmd_rx.recv() => {
                    if matches!(command, Some(EngineCommand::Cancel)) {
                        cancelled = true;
                        break;
                    }
                }
                event = stream.next() => {
                    let Some(event) = event else { break; };
                    let event = match event {
                        Ok(event) => event,
                        Err(err) => {
                            stream_error = Some(err);
                            break;
                        }
                    };

                    match event {
                        StreamEvent::MessageStart { .. } => {}
                        StreamEvent::PartStart { index, kind, tool } => {
                            message_builder.open(index, kind);
                            match kind {
                                PartKind::Text => { send_event(&event_tx, event_scope.as_ref(), AgentEvent::TextStarted); }
                                PartKind::Thinking => { send_event(&event_tx, event_scope.as_ref(), AgentEvent::ThinkingStarted); }
                                PartKind::ToolCall => {
                                    if let Some(tool) = tool {
                                        message_builder.register_tool(index, tool.id.clone(), tool.name.clone());
                                        send_event(&event_tx, event_scope.as_ref(), AgentEvent::ToolStarted { id: tool.id, name: tool.name });
                                    }
                                }
                            }
                        }
                        StreamEvent::TextDelta { index, delta } => {
                            message_builder.push_text(index, &delta);
                            send_event(&event_tx, event_scope.as_ref(), AgentEvent::TextChunk { delta });
                        }
                        StreamEvent::ThinkingDelta { index, delta } => {
                            message_builder.push_text(index, &delta);
                            send_event(&event_tx, event_scope.as_ref(), AgentEvent::ThinkingChunk { delta });
                        }
                        StreamEvent::ToolJsonDelta { index, chunk } => {
                            message_builder.push_tool_json(index, &chunk);
                            if let Some((id, name)) = message_builder.tool_head(index) {
                                if should_stream_tool_args(&name) {
                                    send_event(&event_tx, event_scope.as_ref(), AgentEvent::ToolArgsDelta { id, delta: chunk });
                                }
                            }
                        }
                        StreamEvent::PartMeta { index, meta } => {
                            message_builder.push_meta(index, meta);
                        }
                        StreamEvent::PartStop { index } => {
                            match message_builder.kind(index) {
                                Some(PartKind::Text) => { send_event(&event_tx, event_scope.as_ref(), AgentEvent::TextFinished); }
                                Some(PartKind::Thinking) => {
                                    if let Some(ms) = message_builder.thinking_duration_ms(index) {
                                        message_builder.insert_meta_field(index, "duration_ms", json!(ms));
                                    }
                                    send_event(&event_tx, event_scope.as_ref(), AgentEvent::ThinkingFinished);
                                }
                                Some(PartKind::ToolCall) => {
                                    let (id, name, args) = message_builder.finalize_tool(index);
                                    let mcp_label = mcp.tool_label(&name).await;
                                    let summary = mcp_label
                                        .as_ref()
                                        .map(|label| {
                                            format!(
                                                "{} · {}",
                                                display_mcp_server_name(&label.server_name),
                                                label.tool_name
                                            )
                                        })
                                        .or_else(|| {
                                            subagents
                                                .as_ref()
                                                .and_then(|tool| tool.summary_for_tool_name(&name))
                                        })
                                        .or_else(|| {
                                            teams
                                                .as_ref()
                                                .and_then(|tool| tool.summary_for_tool_name(&name))
                                        })
                                        .unwrap_or_else(|| summarize_tool(&name, &args));
                                    if let Some(label) = mcp_label {
                                        message_builder.insert_meta_field(index, "mcp", json!(label));
                                    }
                                    send_event(&event_tx, event_scope.as_ref(), AgentEvent::ToolReady {
                                        id,
                                        summary,
                                        args_pretty: pretty_json(&args),
                                    });
                                }
                                None => {}
                            }
                        }
                        StreamEvent::Usage { usage } => {
                            response_usage = Some(usage);
                            send_token_usage_event(&event_tx, event_scope.as_ref(), &provider, &model, usage);
                        }
                        StreamEvent::MessageStop { stop_reason: reason, usage } => {
                            stop_reason = reason;
                            response_usage = Some(usage);
                            send_token_usage_event(&event_tx, event_scope.as_ref(), &provider, &model, usage);
                            break;
                        }
                    }
                }
            }
        }

        if let Some(err) = stream_error {
            if auto_compact
                && message_builder.is_empty()
                && is_context_length_error(&err)
                && can_auto_compact_history(&history, auto_compaction_attempts)
            {
                match run_auto_compaction(
                    &provider,
                    &model,
                    cache_key.as_ref(),
                    &mut cache_stable_message_count,
                    &mut history,
                    &mut current_turn_tool_result_ids,
                    &current_system_prompt,
                    &event_tx,
                    event_scope.as_ref(),
                    &mut cmd_rx,
                    &mut auto_compaction_attempts,
                )
                .await
                {
                    Ok(()) => continue,
                    Err(compaction_err) => {
                        send_event(
                            &event_tx,
                            event_scope.as_ref(),
                            AgentEvent::Error {
                                message: format!(
                                    "stream error: {err}; context compaction failed: {compaction_err}"
                                ),
                            },
                        );
                        break 'conversation;
                    }
                }
            }

            send_event(
                &event_tx,
                event_scope.as_ref(),
                AgentEvent::Error {
                    message: format!("stream error: {err}"),
                },
            );
            break 'conversation;
        }

        let mut assistant = message_builder.finish();
        if cancelled {
            retain_cancelled_visible_parts(&mut assistant);
            if !assistant.parts.is_empty() {
                history.push(assistant);
            }
            break 'conversation;
        }
        if mode == AgentMode::Plan && !stop_questions && question_enabled {
            if !assistant_has_question_tool(&assistant)
                && !matches!(stop_reason, StopReason::ToolUse)
            {
                append_plan_fallback_question(&mut assistant, &event_tx, event_scope.as_ref());
                stop_reason = StopReason::ToolUse;
            } else if assistant_has_question_tool(&assistant) {
                stop_reason = StopReason::ToolUse;
            }
        }
        if let Some(usage) = response_usage {
            attach_token_usage(&mut assistant, provider.name(), &model.name, usage);
        }
        if !assistant.parts.is_empty() {
            history.push(assistant.clone());
        }

        if !matches!(stop_reason, StopReason::ToolUse) {
            break;
        }

        if loops >= max_tool_rounds {
            send_event(
                &event_tx,
                event_scope.as_ref(),
                AgentEvent::Error {
                    message: format!("tool loop limit reached ({max_tool_rounds})"),
                },
            );
            break;
        }
        loops += 1;

        let mut tool_results = Vec::new();
        for part in &assistant.parts {
            if let Part::ToolCall {
                id, name, input, ..
            } = part
            {
                let result = if name == "clean_context" {
                    run_clean_context(&mut history, input.clone(), &current_turn_tool_result_ids)
                } else if name == "update_goal" {
                    run_update_goal(&mut goal_workflow, input.clone())
                } else if should_wait_for_cooperative_cancel(
                    name,
                    subagents.as_ref(),
                    teams.as_ref(),
                ) {
                    let result = run_tool(
                        &bash,
                        &glob,
                        &grep,
                        &read,
                        &apply_patch,
                        &create_image,
                        todo_list_tool.as_deref(),
                        question.as_deref(),
                        &web_search,
                        &web_fetch,
                        &skill,
                        &mcp,
                        subagents.as_deref(),
                        teams.as_deref(),
                        &tool_settings,
                        &read_paths,
                        &mut todo_list,
                        mode,
                        &event_tx,
                        id,
                        name,
                        input.clone(),
                    )
                    .await;
                    if matches!(cmd_rx.try_recv(), Ok(EngineCommand::Cancel)) {
                        cancelled = true;
                    }
                    result
                } else {
                    tokio::select! {
                    biased;
                        command = cmd_rx.recv() => {
                            if matches!(command, Some(EngineCommand::Cancel)) {
                                cancelled = true;
                                ToolRunResult::err("tool call interrupted by user", Vec::new())
                            } else {
                                continue;
                            }
                        }
                        result = run_tool(
                            &bash,
                            &glob,
                            &grep,
                            &read,
                            &apply_patch,
                            &create_image,
                            todo_list_tool.as_deref(),
                            question.as_deref(),
                            &web_search,
                            &web_fetch,
                            &skill,
                            &mcp,
                            subagents.as_deref(),
                            teams.as_deref(),
                            &tool_settings,
                            &read_paths,
                            &mut todo_list,
                            mode,
                            &event_tx,
                            id,
                            name,
                            input.clone(),
                        ) => result,
                    }
                };
                if name == "read" && !result.is_error {
                    if let Some(path) = input.get("path").and_then(|value| value.as_str()) {
                        if let Ok(normalized) = read.normalize_path(path) {
                            read_paths.insert(normalized);
                        }
                    }
                }
                let result_images = result.images.clone();
                let result_content = result.content.clone();
                if let Some(teams) = &teams {
                    teams
                        .record_current_agent_file_changes(name, &result.file_changes)
                        .await;
                }
                let mut meta = Map::new();
                if !result.file_changes.is_empty() {
                    meta.insert("file_changes".into(), json!(result.file_changes.clone()));
                }
                if name == "ToDoList" && !result.is_error {
                    meta.insert("todo_list".into(), json!(&todo_list));
                }
                if let Some(Value::Object(result_meta)) = result.meta.clone() {
                    for (key, value) in result_meta {
                        meta.insert(key, value);
                    }
                }
                let result_meta = (!meta.is_empty()).then_some(Value::Object(meta));
                send_event(
                    &event_tx,
                    event_scope.as_ref(),
                    AgentEvent::ToolFinished {
                        id: id.clone(),
                        output: result_content.clone(),
                        is_error: result.is_error,
                        file_changes: result.file_changes.clone(),
                        images: result_images.clone(),
                        meta: result_meta.clone(),
                    },
                );
                if name != "clean_context" {
                    current_turn_tool_result_ids.insert(id.clone());
                }
                tool_results.push(Part::ToolResult {
                    tool_call_id: id.clone(),
                    content: result_content,
                    images: result_images
                        .into_iter()
                        .map(|image| ToolResultImage {
                            media_type: image.media_type,
                            data: if name == "CreateImage" {
                                String::new()
                            } else {
                                image.data
                            },
                            path: image.path,
                        })
                        .collect(),
                    is_error: result.is_error,
                    meta: result_meta,
                });
                if cancelled {
                    break;
                }
            }
        }

        if cancelled {
            append_interrupted_tool_results(&assistant, &mut tool_results);
        }

        if tool_results.is_empty() {
            break;
        }

        history.push(ChatMessage {
            role: Role::User,
            parts: tool_results,
        });
        if cancelled {
            break 'conversation;
        }
        if mode == AgentMode::Plan && !stop_questions && assistant_has_question_tool(&assistant) {
            break;
        }
    }

    if cancelled {
        send_event(&event_tx, event_scope.as_ref(), AgentEvent::Interrupted);
    }
    send_event(&event_tx, event_scope.as_ref(), AgentEvent::TurnFinished);
    todo_list.normalize();
    TurnOutput {
        history,
        todo_list,
        goal_workflow,
        interrupted: cancelled,
    }
}

pub(super) fn retain_cancelled_visible_parts(message: &mut ChatMessage) {
    message.parts.retain(|part| match part {
        Part::Text { text, .. } | Part::Thinking { text, .. } => !text.is_empty(),
        _ => false,
    });
}

fn append_plan_fallback_question(
    message: &mut ChatMessage,
    event_tx: &mpsc::UnboundedSender<AgentEvent>,
    event_scope: Option<&AgentEventScope>,
) {
    let id = format!("plan-question-{}", Uuid::new_v4());
    let name = "Question".to_string();
    let input = json!({
        "question": "Je peux continuer a preparer le plan. Tu veux ajouter une contrainte avant que je le cree ?",
        "type": "single_choice",
        "options": [
            {
                "label": "Ajouter une contrainte",
                "description": "Je precise le scope, le gameplay, le style ou les priorites."
            },
            {
                "label": "Creer le plan maintenant",
                "description": "Je suis pret a generer le plan."
            }
        ]
    });

    send_event(
        event_tx,
        event_scope,
        AgentEvent::ToolStarted {
            id: id.clone(),
            name: name.clone(),
        },
    );
    send_event(
        event_tx,
        event_scope,
        AgentEvent::ToolReady {
            id: id.clone(),
            summary: summarize_tool(&name, &input),
            args_pretty: pretty_json(&input),
        },
    );

    message.parts.push(Part::ToolCall {
        id,
        name,
        input,
        meta: None,
    });
}

fn assistant_has_question_tool(message: &ChatMessage) -> bool {
    message.parts.iter().any(|part| {
        matches!(
            part,
            Part::ToolCall { name, .. } if name == "Question"
        )
    })
}
