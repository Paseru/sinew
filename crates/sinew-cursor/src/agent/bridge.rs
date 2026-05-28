use std::process::Stdio;
use std::sync::Arc;

use async_stream::try_stream;
use sinew_core::{
    AppError, PartKind, ProviderRequest, ProviderStream, Result, StopReason, StreamEvent, Usage,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex};

use crate::identity::CursorIdeIdentity;
use crate::workspace;

use super::conversation_id::stable_agent_conversation_id;
use super::setup::{ensure_agent_bridge_ready, run_stream_script, tsx_executable};
use super::tools::execute_tool;

fn user_text(request: &ProviderRequest) -> String {
    let mut parts = Vec::new();
    for message in &request.transcript {
        for part in &message.parts {
            if let sinew_core::Part::Text { text, .. } = part {
                if !text.trim().is_empty() {
                    parts.push(text.clone());
                }
            }
        }
    }
    parts.join("\n")
}

fn tools_json(request: &ProviderRequest) -> Vec<serde_json::Value> {
    request
        .tools
        .iter()
        .map(|tool| {
            serde_json::json!({
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.input_schema,
            })
        })
        .collect()
}

/// Stream Composer via Node `agent-bridge` (protobuf Run over HTTP/2).
pub async fn stream_via_node_bridge(
    identity: &CursorIdeIdentity,
    token: String,
    request: ProviderRequest,
) -> Result<ProviderStream> {
    let bridge_dir = ensure_agent_bridge_ready().await?;
    let script = run_stream_script(&bridge_dir);
    let tsx = tsx_executable(&bridge_dir);

    let model = request.model.name.clone();
    let system = request.system_prompt.clone().unwrap_or_default();
    let user = user_text(&request);
    let workspace = request
        .workspace_root
        .clone()
        .unwrap_or_default();
    let conversation_id = stable_agent_conversation_id(request.cache_key.as_deref());
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

    let api_headers = identity.agent_bridge_headers(&token);
    let payload = serde_json::json!({
        "accessToken": token,
        "modelId": model,
        "systemPrompt": system,
        "userText": user,
        "workspaceRoot": workspace,
        "conversationId": conversation_id,
        "tools": tools_json(&request),
        "workspaceSnapshot": workspace_snapshot,
        "apiHeaders": api_headers,
    });

    let mut child = Command::new(&tsx)
        .arg(&script)
        .current_dir(&bridge_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            AppError::Provider(format!(
                "failed to spawn agent bridge (tsx): {err}. Vérifiez Node/npm (diagnostic SOTA)."
            ))
        })?;

    let stdin = Arc::new(Mutex::new(
        child
            .stdin
            .take()
            .ok_or_else(|| AppError::Provider("agent bridge stdin unavailable".into()))?,
    ));
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Provider("agent bridge stdout unavailable".into()))?;

    let json = serde_json::to_string(&payload)
        .map_err(|err| AppError::Provider(format!("agent bridge payload: {err}")))?;
    {
        let mut guard = stdin.lock().await;
        guard
            .write_all(json.as_bytes())
            .await
            .map_err(|err| AppError::Provider(format!("agent bridge write: {err}")))?;
        guard
            .write_all(b"\n")
            .await
            .map_err(|err| AppError::Provider(format!("agent bridge write: {err}")))?;
    }

    let (line_tx, mut line_rx) = mpsc::channel::<String>(128);
    let reader_task = tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if line_tx.send(line).await.is_err() {
                break;
            }
        }
    });

    let model_name = request.model.name.clone();
    let workspace_for_tools = workspace;
    let events = try_stream! {
        yield StreamEvent::MessageStart { model: model_name.clone() };
        let text_index = 0usize;
        let thinking_index = 1usize;
        let mut open_part: Option<(usize, PartKind)> = None;
        let mut started_text = false;

        while let Some(line) = line_rx.recv().await {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(&line)
                .map_err(|err| AppError::Decode(format!("agent bridge line: {err} ({line})")))?;

            if let Some(err) = value.get("error").and_then(|v| v.as_str()) {
                Err(AppError::Network(err.to_string()))?;
            }

            if value.get("type").and_then(|v| v.as_str()) == Some("tool_request") {
                let tool_name = value
                    .get("toolName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let args = value.get("args").cloned().unwrap_or(serde_json::Value::Null);
                let content = execute_tool(tool_name, &args, &workspace_for_tools);
                let is_error = content.starts_with("Error:");
                let response = serde_json::json!({
                    "type": "tool_response",
                    "execId": value.get("execId"),
                    "execMsgId": value.get("execMsgId"),
                    "toolCallId": value.get("toolCallId"),
                    "content": content,
                    "isError": is_error,
                });
                let response_line = serde_json::to_string(&response)
                    .map_err(|err| AppError::Provider(format!("tool response json: {err}")))?;
                let mut guard = stdin.lock().await;
                guard
                    .write_all(response_line.as_bytes())
                    .await
                    .map_err(|err| AppError::Provider(format!("agent bridge tool write: {err}")))?;
                guard
                    .write_all(b"\n")
                    .await
                    .map_err(|err| AppError::Provider(format!("agent bridge tool write: {err}")))?;
                continue;
            }

            let event_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let delta = value.get("delta").and_then(|v| v.as_str()).unwrap_or("");
            if delta.is_empty() {
                continue;
            }

            let (index, kind) = if event_type == "thinking" {
                (thinking_index, PartKind::Thinking)
            } else {
                (text_index, PartKind::Text)
            };

            if open_part.map(|(_, k)| k) != Some(kind) {
                if let Some((idx, _)) = open_part.take() {
                    yield StreamEvent::PartStop { index: idx };
                }
                open_part = Some((index, kind));
                yield StreamEvent::PartStart { index, kind, tool: None };
            }
            started_text = true;
            match kind {
                PartKind::Text => yield StreamEvent::TextDelta { index, delta: delta.to_string() },
                PartKind::Thinking => yield StreamEvent::ThinkingDelta { index, delta: delta.to_string() },
                _ => {}
            }
        }

        let _ = reader_task.await;

        if let Some((idx, _)) = open_part.take() {
            yield StreamEvent::PartStop { index: idx };
        }

        if !started_text {
            Err(AppError::Network(
                "agent bridge returned no text (OAuth Composer connecté ?)".into(),
            ))?;
        }

        yield StreamEvent::MessageStop {
            stop_reason: StopReason::EndTurn,
            usage: Usage::default(),
        };
    };

    Ok(Box::pin(events))
}
