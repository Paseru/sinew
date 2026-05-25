use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use futures_util::StreamExt;
use rand::Rng;
use serde_json::{json, Map, Value};
use tokio::{sync::mpsc, task::JoinHandle};
use uuid::Uuid;

use claakecode_core::{
    AppError, ChatMessage, Part, PartKind, ProviderRequest, Role, StopReason, StreamEvent,
    ToolResultImage,
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
        successful_read_fingerprints,
    },
    mode::{run_update_goal, system_prompt_for_turn, update_goal_descriptor},
    tool_dispatch::{run_tool, should_wait_for_cooperative_cancel},
    tool_summary::{display_mcp_server_name, pretty_json, should_stream_tool_args, summarize_tool},
};

use crate::{system_prompt_with_todo, ReadFingerprint, ToolRunResult};

const SAFE_STREAM_MAX_RETRIES: usize = 5;

pub async fn run_turn(ctx: TurnContext) -> TurnOutput {
    let TurnContext {
        provider,
        model,
        cache_key,
        mut cache_stable_message_count,
        service_tier,
        auto_compact,
        mode,
        mut stop_questions,
        system_prompt,
        mut history,
        mut todo_list,
        mut goal_workflow,
        bash,
        glob,
        grep,
        read,
        edit_file,
        write_file,
        create_image,
        todo_list_tool,
        question,
        web_search,
        web_fetch,
        skill,
        database,
        mcp,
        subagents,
        teams,
        tool_settings,
        event_scope,
        max_tool_rounds,
        event_tx,
        cancel,
        mut cmd_rx,
    } = ctx;

    send_event(&event_tx, event_scope.as_ref(), AgentEvent::TurnStarted);
    strip_all_visible_tool_result_ids(&mut history);
    normalize_tool_call_inputs(&mut history);
    repair_missing_tool_results(&mut history);
    mcp.refresh_catalog(&history).await;

    let mut cancelled = false;
    let mut compacted = false;
    let mut loops = 0usize;
    let mut auto_compaction_attempts = 0usize;
    let mut current_turn_tool_result_ids = BTreeSet::new();
    let mut eager_tool_results = BTreeMap::<String, JoinHandle<ToolRunResult>>::new();
    let mut read_fingerprints = successful_read_fingerprints(&history, &read);
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
        tool_descriptors.extend(database.descriptors());
        if mode != AgentMode::Plan {
            tool_descriptors.insert(4, edit_file.descriptor());
            tool_descriptors.insert(5, write_file.descriptor());
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
        let current_system_prompt = system_prompt_for_turn(
            &current_system_prompt,
            mode,
            &goal_workflow,
            tool_settings.plan_mode_prompt(),
        );

        if auto_compact {
            match maybe_auto_compact_history(
                &provider,
                &model,
                cache_key.as_ref(),
                &mut cache_stable_message_count,
                service_tier,
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
                Ok(true) => {
                    compacted = true;
                    continue;
                }
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
        let request = match service_tier {
            Some(service_tier) => request.with_service_tier(service_tier),
            None => request,
        };

        let mut stream_retry_attempts = 0usize;
        let (message_builder, mut stop_reason, response_usage) = 'stream_attempt: loop {
            let mut stream = match provider.stream(request.clone()).await {
                Ok(stream) => stream,
                Err(err) => {
                    if should_retry_stream(&err, stream_retry_attempts) {
                        stream_retry_attempts += 1;
                        tracing::warn!(
                            provider = provider.name(),
                            attempt = stream_retry_attempts,
                            max_attempts = SAFE_STREAM_MAX_RETRIES,
                            error = %err,
                            "retrying provider stream setup"
                        );
                        tokio::time::sleep(stream_retry_delay(stream_retry_attempts)).await;
                        continue 'stream_attempt;
                    }

                    if auto_compact
                        && is_context_length_error(&err)
                        && can_auto_compact_history(&history, auto_compaction_attempts)
                    {
                        match run_auto_compaction(
                            &provider,
                            &model,
                            cache_key.as_ref(),
                            &mut cache_stable_message_count,
                            service_tier,
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
                            Ok(()) => {
                                compacted = true;
                                continue 'conversation;
                            }
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
                                break 'conversation;
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
                    break 'conversation;
                }
            };

            let mut message_builder = AssistantMessageBuilder::default();
            let mut stop_reason = StopReason::EndTurn;
            let mut response_usage = None;
            let mut stream_error = None;
            let mut saw_message_stop = false;
            let mut finalized_tool_calls = 0usize;

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
                                            id: id.clone(),
                                            summary,
                                            args_pretty: pretty_json(&args),
                                        });
                                        if should_run_eager_write_file(&name, mode, &tool_settings)
                                            && finalized_tool_calls == 0
                                            && loops < max_tool_rounds
                                            && eager_tool_results.is_empty()
                                        {
                                            let eager_write_file = write_file.clone();
                                            let read_fingerprints = read_fingerprints.clone();
                                            let input = args.clone();
                                            eager_tool_results.insert(
                                                id,
                                                tokio::spawn(async move {
                                                    eager_write_file.run(input, &read_fingerprints).await
                                                }),
                                            );
                                        }
                                        finalized_tool_calls += 1;
                                    }
                                    None => {}
                                }
                            }
                            StreamEvent::Usage { usage } => {
                                response_usage = Some(usage);
                                send_token_usage_event(&event_tx, event_scope.as_ref(), &provider, &model, usage);
                            }
                            StreamEvent::MessageStop { stop_reason: reason, usage } => {
                                saw_message_stop = true;
                                stop_reason = reason;
                                response_usage = Some(usage);
                                send_token_usage_event(&event_tx, event_scope.as_ref(), &provider, &model, usage);
                                break;
                            }
                        }
                    }
                }
            }

            // Detect a silent stream close: the underlying SSE source returned `None` (or yielded
            // its last item) without ever emitting a `MessageStop`. This is the classic "OpenAI
            // just stops without an error" symptom — usually a connection drop on the provider /
            // edge proxy side. Surface it as an explicit stream error so the user gets feedback
            // and the normal recovery path (auto-compaction, etc.) is given a chance to run.
            if !cancelled && stream_error.is_none() && !saw_message_stop {
                stream_error = Some(AppError::Stream(format!(
                    "{} stream closed without sending a stop event (likely a connection drop)",
                    provider.name()
                )));
            }

            if let Some(err) = stream_error {
                if !eager_tool_results.is_empty() {
                    tracing::warn!(
                        provider = provider.name(),
                        error = %err,
                        "stream ended after eager tool execution; continuing with completed tool call"
                    );
                    stop_reason = StopReason::ToolUse;
                    break 'stream_attempt (message_builder, stop_reason, response_usage);
                }

                if should_retry_stream(&err, stream_retry_attempts) {
                    stream_retry_attempts += 1;
                    tracing::warn!(
                        provider = provider.name(),
                        attempt = stream_retry_attempts,
                        max_attempts = SAFE_STREAM_MAX_RETRIES,
                        error = %err,
                        "retrying provider stream"
                    );
                    tokio::time::sleep(stream_retry_delay(stream_retry_attempts)).await;
                    continue 'stream_attempt;
                }

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
                        service_tier,
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
                        Ok(()) => {
                            compacted = true;
                            continue 'conversation;
                        }
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

            break 'stream_attempt (message_builder, stop_reason, response_usage);
        };

        let mut assistant = message_builder.finish();
        if cancelled {
            if eager_tool_results.is_empty() {
                retain_cancelled_visible_parts(&mut assistant);
                if !assistant.parts.is_empty() {
                    history.push(assistant);
                }
                break 'conversation;
            }
            retain_cancelled_eager_parts(&mut assistant, &eager_tool_results);
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

        if !matches!(stop_reason, StopReason::ToolUse) && !eager_tool_results.is_empty() {
            stop_reason = StopReason::ToolUse;
        }
        if !matches!(stop_reason, StopReason::ToolUse) {
            break;
        }

        if loops >= max_tool_rounds {
            abort_eager_tool_results(&mut eager_tool_results);
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
                } else if let Some(handle) = eager_tool_results.remove(id) {
                    match handle.await {
                        Ok(result) => result,
                        Err(err) => {
                            ToolRunResult::err(format!("write_file task failed: {err}"), Vec::new())
                        }
                    }
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
                        &edit_file,
                        &write_file,
                        &create_image,
                        todo_list_tool.as_deref(),
                        question.as_deref(),
                        &web_search,
                        &web_fetch,
                        &skill,
                        &database,
                        &mcp,
                        subagents.as_deref(),
                        teams.as_deref(),
                        &tool_settings,
                        &read_fingerprints,
                        &mut todo_list,
                        mode,
                        &event_tx,
                        &cancel,
                        id,
                        name,
                        input.clone(),
                    )
                    .await;
                    if matches!(cmd_rx.try_recv(), Ok(EngineCommand::Cancel)) {
                        cancelled = true;
                        abort_eager_tool_results(&mut eager_tool_results);
                    }
                    result
                } else {
                    tokio::select! {
                    biased;
                        command = cmd_rx.recv() => {
                            if matches!(command, Some(EngineCommand::Cancel)) {
                                cancelled = true;
                                abort_eager_tool_results(&mut eager_tool_results);
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
                            &edit_file,
                            &write_file,
                            &create_image,
                            todo_list_tool.as_deref(),
                            question.as_deref(),
                            &web_search,
                            &web_fetch,
                            &skill,
                            &database,
                            &mcp,
                            subagents.as_deref(),
                            teams.as_deref(),
                            &tool_settings,
                            &read_fingerprints,
                            &mut todo_list,
                            mode,
                            &event_tx,
                            &cancel,
                            id,
                            name,
                            input.clone(),
                        ) => result,
                    }
                };
                if (name == "read" || name == "edit_file" || name == "write_file")
                    && !result.is_error
                {
                    update_read_fingerprint_cache(&mut read_fingerprints, result.meta.as_ref());
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
                let stop_after_question = name == "Question"
                    && meta
                        .get("question_stop_requested")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    && mode == AgentMode::Plan;
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
                if stop_after_question {
                    break;
                }
            }
        }

        if cancelled {
            abort_eager_tool_results(&mut eager_tool_results);
            append_interrupted_tool_results(&assistant, &mut tool_results);
        }

        if tool_results.is_empty() {
            break;
        }

        let stop_after_question_result = tool_results.iter().any(|part| {
            matches!(part, Part::ToolResult { meta: Some(Value::Object(meta)), .. }
                if meta
                    .get("question_stop_requested")
                    .and_then(Value::as_bool)
                    .unwrap_or(false))
        }) && mode == AgentMode::Plan;
        history.push(ChatMessage {
            role: Role::User,
            parts: tool_results,
        });
        if cancelled {
            break 'conversation;
        }
        if stop_after_question_result {
            stop_questions = true;
            history.push(ChatMessage {
                role: Role::User,
                parts: vec![Part::Text {
                    text: "\n\n<plan_mode_control action=\"stop_questions\">\nThe user clicked Send and stop questions. Do not ask more questions in this turn. Produce the complete Markdown plan now and do not implement it.\n</plan_mode_control>".to_string(),
                    meta: Some(json!({ "plan_control": "stop_questions" })),
                }],
            });
            continue 'conversation;
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
        compacted,
    }
}

pub(super) fn retain_cancelled_eager_parts(
    message: &mut ChatMessage,
    eager_tool_results: &BTreeMap<String, JoinHandle<ToolRunResult>>,
) {
    message.parts.retain(|part| match part {
        Part::Text { text, .. } | Part::Thinking { text, .. } => !text.is_empty(),
        Part::ToolCall { id, .. } => eager_tool_results.contains_key(id),
        _ => false,
    });
}

pub(super) fn retain_cancelled_visible_parts(message: &mut ChatMessage) {
    message.parts.retain(|part| match part {
        Part::Text { text, .. } | Part::Thinking { text, .. } => !text.is_empty(),
        _ => false,
    });
}

fn should_run_eager_write_file(
    name: &str,
    mode: AgentMode,
    tool_settings: &crate::ToolSettings,
) -> bool {
    name == "write_file" && mode != AgentMode::Plan && tool_settings.is_enabled(name)
}

fn abort_eager_tool_results(handles: &mut BTreeMap<String, JoinHandle<ToolRunResult>>) {
    for (_, handle) in std::mem::take(handles) {
        handle.abort();
    }
}

fn update_read_fingerprint_cache(
    cache: &mut std::collections::HashMap<String, ReadFingerprint>,
    meta: Option<&serde_json::Value>,
) {
    let Some(meta) = meta else {
        return;
    };
    if let Some(fingerprint) = meta
        .get("read_fingerprint")
        .cloned()
        .and_then(|value| serde_json::from_value::<ReadFingerprint>(value).ok())
    {
        cache.insert(fingerprint.relative_path.clone(), fingerprint);
    }
    if let Some(values) = meta.get("read_fingerprints").and_then(Value::as_array) {
        for fingerprint in values
            .iter()
            .filter_map(|value| serde_json::from_value::<ReadFingerprint>(value.clone()).ok())
        {
            cache.insert(fingerprint.relative_path.clone(), fingerprint);
        }
    }
}

fn should_retry_stream(err: &AppError, attempts: usize) -> bool {
    attempts < SAFE_STREAM_MAX_RETRIES
        && matches!(
            err,
            AppError::Network(_)
                | AppError::Stream(_)
                | AppError::Decode(_)
                | AppError::RetryableStream { .. }
        )
}

fn stream_retry_delay(attempt: usize) -> Duration {
    let exponent = attempt.saturating_sub(1).min(8) as u32;
    let base_ms = 200u64.saturating_mul(2u64.saturating_pow(exponent));
    let jitter = rand::rng().random_range(0.9..1.1);
    Duration::from_millis((base_ms as f64 * jitter) as u64)
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
