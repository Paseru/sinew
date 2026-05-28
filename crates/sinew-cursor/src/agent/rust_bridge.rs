//! Native Rust `agent.v1` Run client (HTTP/2 + prost-reflect).

use async_stream::try_stream;
use sinew_core::{
    AppError, PartKind, ProviderRequest, ProviderStream, Result, StopReason, StreamEvent,
    ToolCallIntro, Usage,
};

use crate::identity::CursorIdeIdentity;
use crate::workspace;

use super::bridge::{stream_via_node_bridge, tools_json};
use super::conversation_id::stable_agent_conversation_id;
use super::run_h2::{run_agent_stream, AgentRunConfig, ToolResponse};
use super::server_decode::BridgeEvent;
use super::state::AgentConversationStore;
use super::tools::execute_tool;
use super::transcript::split_transcript;

/// Stream via native Rust HTTP/2 + prost (no Node subprocess).
pub async fn stream_via_rust_bridge(
    identity: &CursorIdeIdentity,
    token: String,
    request: ProviderRequest,
) -> Result<ProviderStream> {
    match stream_via_rust_bridge_inner(identity, token.clone(), request.clone()).await {
        Ok(stream) => Ok(stream),
        Err(err) if super::transport::allow_node_fallback() => {
            tracing::warn!(
                "Rust agent bridge failed ({err}), falling back to Node (SINEW_CURSOR_BRIDGE_FALLBACK)"
            );
            stream_via_node_bridge(identity, token, request).await
        }
        Err(err) => Err(err),
    }
}

async fn stream_via_rust_bridge_inner(
    identity: &CursorIdeIdentity,
    token: String,
    request: ProviderRequest,
) -> Result<ProviderStream> {
    let model = request.model.name.clone();
    let system = request
        .system_prompt
        .clone()
        .unwrap_or_else(|| "You are Composer in Cursor IDE.".to_string());
    let (history_turns, current_user) = split_transcript(&request.transcript);
    let user = if current_user.is_empty() {
        request
            .transcript
            .last()
            .map(|m| m.text())
            .unwrap_or_default()
    } else {
        current_user
    };
    let workspace = request.workspace_root.clone().unwrap_or_default();
    let cache_key = request.cache_key.clone().unwrap_or_default();
    let conversation_id = stable_agent_conversation_id(request.cache_key.as_deref());
    let persisted = AgentConversationStore::load().get(&cache_key);
    let trimmed = workspace.trim();
    let workspace_snapshot = if !trimmed.is_empty() {
        workspace::snapshot(trimmed).map(|snap| {
            serde_json::json!({
                "uri": snap.uri,
                "name": snap.name,
                "branch": snap.branch,
                "gitStatus": snap.git_status,
                "projectLayout": snap.project_layout,
            })
        })
    } else {
        None
    };

    let config = AgentRunConfig {
        token,
        model_id: model.clone(),
        system_prompt: system,
        user_text: user,
        conversation_id,
        history_turns,
        persisted,
        workspace_root: workspace.clone(),
        tools: tools_json(&request),
        workspace_snapshot,
    };

    let handle = run_agent_stream(identity, config).await?;
    let mut events_rx = handle.events;
    let tool_tx = handle.tool_responses;

    let model_name = model;
    let workspace_for_tools = workspace.clone();
    let cache_key_for_save = cache_key;

    let events = try_stream! {
        yield StreamEvent::MessageStart { model: model_name.clone() };
        let text_index = 0usize;
        let thinking_index = 1usize;
        let mut next_tool_index = 2usize;
        let mut open_part: Option<(usize, PartKind)> = None;
        let mut started_text = false;
        let mut tools_executed = 0u32;
        let mut usage = Usage::default();
        let mut total_output_tokens = 0u32;

        while let Some(item) = events_rx.recv().await {
            let ev = item?;
            match ev {
                BridgeEvent::Checkpoint { checkpoint_b64, blobs } => {
                    if !cache_key_for_save.trim().is_empty() {
                        let mut store = AgentConversationStore::load();
                        let _ = store.save_checkpoint(
                            &cache_key_for_save,
                            checkpoint_b64,
                            blobs,
                        );
                    }
                }
                BridgeEvent::Usage { output_tokens, total_tokens } => {
                    total_output_tokens = total_output_tokens.saturating_add(output_tokens);
                    usage.output_tokens = total_output_tokens;
                    usage.total_tokens = total_tokens.max(total_output_tokens);
                    yield StreamEvent::Usage { usage };
                }
                BridgeEvent::ToolRequest {
                    exec_id: _,
                    exec_msg_id: _,
                    tool_name,
                    tool_call_id,
                    args,
                } => {
                    if let Some((idx, _)) = open_part.take() {
                        yield StreamEvent::PartStop { index: idx };
                    }
                    let tool_index = next_tool_index;
                    next_tool_index += 1;
                    let args_json = serde_json::to_string(&args)
                        .map_err(|err| AppError::Provider(format!("tool args json: {err}")))?;
                    yield StreamEvent::PartStart {
                        index: tool_index,
                        kind: PartKind::ToolCall,
                        tool: Some(ToolCallIntro {
                            id: tool_call_id.clone(),
                            name: tool_name.clone(),
                        }),
                    };
                    yield StreamEvent::ToolJsonDelta {
                        index: tool_index,
                        chunk: args_json,
                    };
                    let content = execute_tool(&tool_name, &args, &workspace_for_tools);
                    let is_error = content.starts_with("Error:");
                    yield StreamEvent::PartMeta {
                        index: tool_index,
                        meta: serde_json::json!({
                            "composer_bridge": { "content": content, "is_error": is_error }
                        }),
                    };
                    yield StreamEvent::PartStop { index: tool_index };
                    tools_executed += 1;
                    let _ = tool_tx
                        .send(ToolResponse {
                            content,
                            is_error,
                        })
                        .await;
                }
                BridgeEvent::Text(delta) => {
                    let index = text_index;
                    let kind = PartKind::Text;
                    if open_part.map(|(_, k)| k) != Some(kind) {
                        if let Some((idx, _)) = open_part.take() {
                            yield StreamEvent::PartStop { index: idx };
                        }
                        open_part = Some((index, kind));
                        yield StreamEvent::PartStart { index, kind, tool: None };
                    }
                    started_text = true;
                    yield StreamEvent::TextDelta { index, delta };
                }
                BridgeEvent::Thinking(delta) => {
                    let index = thinking_index;
                    let kind = PartKind::Thinking;
                    if open_part.map(|(_, k)| k) != Some(kind) {
                        if let Some((idx, _)) = open_part.take() {
                            yield StreamEvent::PartStop { index: idx };
                        }
                        open_part = Some((index, kind));
                        yield StreamEvent::PartStart { index, kind, tool: None };
                    }
                    yield StreamEvent::ThinkingDelta { index, delta };
                }
                BridgeEvent::StepCompleted | BridgeEvent::TurnEnded => break,
            }
        }

        if let Some((idx, _)) = open_part.take() {
            yield StreamEvent::PartStop { index: idx };
        }

        if !started_text && tools_executed == 0 {
            Err(AppError::Network(
                "Rust agent bridge returned no text (OAuth Composer connecté ?)".into(),
            ))?;
        }

        let stop_reason = if tools_executed > 0 {
            StopReason::ToolUse
        } else {
            StopReason::EndTurn
        };
        yield StreamEvent::MessageStop { stop_reason, usage };
    };

    Ok(Box::pin(events))
}
