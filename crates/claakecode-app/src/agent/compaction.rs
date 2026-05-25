use std::{collections::BTreeSet, sync::Arc};

use serde_json::{json, Value};
use tokio::sync::mpsc;
use uuid::Uuid;

use claakecode_core::{
    AppError, ChatMessage, ModelRef, Part, Provider, ProviderRequest, ServiceTier, ToolDescriptor,
};

use crate::compact_conversation_history;

use super::{
    cancel::EngineCommand,
    events::{send_event, AgentEvent, AgentEventScope},
    history::history_with_current_tool_result_ids,
};

const AUTO_COMPACT_OUTPUT_TOKEN_MAX: u32 = 32_000;
const MAX_AUTO_COMPACTIONS_PER_TURN: usize = 3;
const AUTO_COMPACTION_TOOL_NAME: &str = "context_compaction";

pub(super) async fn maybe_auto_compact_history(
    provider: &Arc<dyn Provider>,
    model: &ModelRef,
    cache_key: Option<&String>,
    cache_stable_message_count: &mut usize,
    service_tier: Option<ServiceTier>,
    history: &mut Vec<ChatMessage>,
    current_turn_tool_result_ids: &mut BTreeSet<String>,
    system_prompt: &str,
    tool_descriptors: &[ToolDescriptor],
    event_tx: &mpsc::UnboundedSender<AgentEvent>,
    event_scope: Option<&AgentEventScope>,
    cmd_rx: &mut mpsc::UnboundedReceiver<EngineCommand>,
    auto_compaction_attempts: &mut usize,
) -> std::result::Result<bool, String> {
    if !can_auto_compact_history(history, *auto_compaction_attempts) {
        return Ok(false);
    }

    let Some(caps) = provider.capabilities(model) else {
        return Ok(false);
    };
    if caps.context_window == 0 {
        return Ok(false);
    }

    let request_history =
        history_with_current_tool_result_ids(history, current_turn_tool_result_ids);
    let mut request = ProviderRequest::new(model.clone(), request_history)
        .with_system(system_prompt.to_string())
        .with_tools(tool_descriptors.to_vec())
        .with_cache_stable_message_count(*cache_stable_message_count);
    if let Some(cache_key) = cache_key {
        request = request.with_cache_key(cache_key.clone());
    }
    if let Some(service_tier) = service_tier {
        request = request.with_service_tier(service_tier);
    }

    let should_compact = match provider.estimate_tokens(request).await {
        Ok(estimate) => {
            let threshold = auto_compact_threshold(caps.context_window, caps.max_output_tokens);
            estimate.input_tokens >= threshold
        }
        Err(err) if is_context_length_error(&err) => true,
        Err(_) => false,
    };

    if !should_compact {
        return Ok(false);
    }

    run_auto_compaction(
        provider,
        model,
        cache_key,
        cache_stable_message_count,
        service_tier,
        history,
        current_turn_tool_result_ids,
        system_prompt,
        event_tx,
        event_scope,
        cmd_rx,
        auto_compaction_attempts,
    )
    .await?;
    Ok(true)
}

pub(super) async fn run_auto_compaction(
    provider: &Arc<dyn Provider>,
    model: &ModelRef,
    cache_key: Option<&String>,
    cache_stable_message_count: &mut usize,
    service_tier: Option<ServiceTier>,
    history: &mut Vec<ChatMessage>,
    current_turn_tool_result_ids: &mut BTreeSet<String>,
    system_prompt: &str,
    event_tx: &mpsc::UnboundedSender<AgentEvent>,
    event_scope: Option<&AgentEventScope>,
    cmd_rx: &mut mpsc::UnboundedReceiver<EngineCommand>,
    auto_compaction_attempts: &mut usize,
) -> std::result::Result<(), String> {
    if !can_auto_compact_history(history, *auto_compaction_attempts) {
        return Err("context is still too large, but there is no new content to compact".into());
    }

    let compaction_id = format!("auto-context-compaction-{}", Uuid::new_v4());
    send_event(
        event_tx,
        event_scope,
        AgentEvent::ToolStarted {
            id: compaction_id.clone(),
            name: AUTO_COMPACTION_TOOL_NAME.to_string(),
        },
    );
    send_event(
        event_tx,
        event_scope,
        AgentEvent::ToolReady {
            id: compaction_id.clone(),
            summary: "Compact context".to_string(),
            args_pretty: "{}".to_string(),
        },
    );

    let before_len = history.len();
    let (summary_delta_tx, mut summary_delta_rx) = mpsc::unbounded_channel();
    let delta_event_tx = event_tx.clone();
    let delta_event_scope = event_scope.cloned();
    let delta_compaction_id = compaction_id.clone();
    let delta_forwarder = tokio::spawn(async move {
        while let Some(delta) = summary_delta_rx.recv().await {
            send_event(
                &delta_event_tx,
                delta_event_scope.as_ref(),
                AgentEvent::ToolOutputDelta {
                    id: delta_compaction_id.clone(),
                    delta,
                },
            );
        }
    });
    let result = compact_conversation_history(
        provider.clone(),
        model.clone(),
        system_prompt.to_string(),
        history.clone(),
        cache_key.cloned(),
        *cache_stable_message_count,
        service_tier,
        None,
        cmd_rx,
        Some(summary_delta_tx),
    )
    .await;
    let _ = delta_forwarder.await;

    match result {
        Ok(output) => {
            let retained = output.retained_user_messages;
            let summary = output.summary;
            *history = output.history;
            current_turn_tool_result_ids.clear();
            *cache_stable_message_count = 0;
            *auto_compaction_attempts += 1;
            let label = match retained {
                0 => "Context compacted. No raw user messages retained".to_string(),
                1 => format!(
                    "Context compacted from {before_len} messages. Retained 1 recent user message"
                ),
                count => format!(
                    "Context compacted from {before_len} messages. Retained {count} recent user messages"
                ),
            };
            send_event(
                event_tx,
                event_scope,
                AgentEvent::ToolFinished {
                    id: compaction_id,
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
        Err(err) => {
            let message = err.to_string();
            send_event(
                event_tx,
                event_scope,
                AgentEvent::ToolFinished {
                    id: compaction_id,
                    output: message.clone(),
                    is_error: true,
                    file_changes: Vec::new(),
                    images: Vec::new(),
                    meta: None,
                },
            );
            Err(message)
        }
    }
}

pub(super) fn can_auto_compact_history(history: &[ChatMessage], attempts: usize) -> bool {
    attempts < MAX_AUTO_COMPACTIONS_PER_TURN
        && (attempts > 0 || has_content_after_latest_compaction(history))
}

fn has_content_after_latest_compaction(history: &[ChatMessage]) -> bool {
    let latest_boundary = history
        .iter()
        .rposition(|message| message.parts.iter().any(is_auto_compaction_boundary_part));
    history
        .iter()
        .skip(latest_boundary.map(|index| index + 1).unwrap_or(0))
        .any(|message| message.parts.iter().any(is_auto_compaction_meaningful_part))
}

fn is_auto_compaction_boundary_part(part: &Part) -> bool {
    let Some(meta) = part_meta(part) else {
        return false;
    };
    meta.get("compaction_summary").and_then(Value::as_bool) == Some(true)
        || meta.get("compaction_marker").and_then(Value::as_bool) == Some(true)
}

fn is_auto_compaction_meaningful_part(part: &Part) -> bool {
    match part {
        Part::Text { text, meta } => {
            !text.trim().is_empty() && !is_auto_compaction_hidden_text(meta)
        }
        _ => true,
    }
}

fn is_auto_compaction_hidden_text(meta: &Option<Value>) -> bool {
    let Some(Value::Object(meta)) = meta else {
        return false;
    };
    meta.get("attachment_context").and_then(Value::as_bool) == Some(true)
        || meta.get("ui_only").and_then(Value::as_bool) == Some(true)
        || meta.get("system_reminder").and_then(Value::as_bool) == Some(true)
        || meta
            .get("compaction_retained_user")
            .and_then(Value::as_bool)
            == Some(true)
        || meta.get("compaction_summary").and_then(Value::as_bool) == Some(true)
        || meta.get("plan_control").and_then(Value::as_str).is_some()
}

pub(super) fn is_context_length_error(err: &AppError) -> bool {
    matches!(err, AppError::ContextLength(_))
}

fn auto_compact_threshold(context_window: u32, max_output_tokens: u32) -> u32 {
    if context_window == 0 {
        return 0;
    }
    let reserved_output = if max_output_tokens == 0 {
        AUTO_COMPACT_OUTPUT_TOKEN_MAX
    } else {
        max_output_tokens.min(AUTO_COMPACT_OUTPUT_TOKEN_MAX)
    };
    context_window.saturating_sub(reserved_output)
}

fn part_meta(part: &Part) -> Option<&Value> {
    match part {
        Part::Text { meta, .. }
        | Part::Image { meta, .. }
        | Part::Thinking { meta, .. }
        | Part::ToolCall { meta, .. }
        | Part::ToolResult { meta, .. } => meta.as_ref(),
    }
}
