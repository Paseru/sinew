use std::path::PathBuf;
use std::process::Stdio;

use async_stream::try_stream;
use sinew_core::{
    AppError, PartKind, ProviderRequest, ProviderStream, Result, StopReason, StreamEvent, Usage,
};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

fn bridge_script_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("SINEW_CURSOR_AGENT_BRIDGE") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidate = manifest
        .join("..")
        .join("..")
        .join("scripts")
        .join("agent-bridge")
        .join("run-stream.mjs");
    if candidate.exists() {
        return Some(candidate);
    }
    None
}

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

/// Stream Composer via Node `agent-bridge` (protobuf Run over HTTP/2).
pub async fn stream_via_node_bridge(
    token: String,
    request: ProviderRequest,
) -> Result<ProviderStream> {
    let script = bridge_script_path().ok_or_else(|| {
        AppError::Provider(
            "agent bridge missing: run `npm install` in scripts/agent-bridge or set SINEW_CURSOR_AGENT_BRIDGE".into(),
        )
    })?;

    let model = request.model.name.clone();
    let system = request.system_prompt.clone().unwrap_or_default();
    let user = user_text(&request);
    let workspace = request.workspace_root.clone().unwrap_or_default();

    let payload = serde_json::json!({
        "accessToken": token,
        "modelId": model,
        "systemPrompt": system,
        "userText": user,
        "workspaceRoot": workspace,
    });

    let bridge_dir = script.parent().ok_or_else(|| {
        AppError::Provider("agent bridge script has no parent directory".into())
    })?;

    let mut child = Command::new("npx")
        .args(["tsx", script.to_string_lossy().as_ref()])
        .current_dir(bridge_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| AppError::Provider(format!("failed to spawn agent bridge: {err}")))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Provider("agent bridge stdin unavailable".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Provider("agent bridge stdout unavailable".into()))?;

    let json = serde_json::to_string(&payload)
        .map_err(|err| AppError::Provider(format!("agent bridge payload: {err}")))?;
    use tokio::io::AsyncWriteExt;
    stdin
        .write_all(json.as_bytes())
        .await
        .map_err(|err| AppError::Provider(format!("agent bridge write: {err}")))?;
    stdin
        .write_all(b"\n")
        .await
        .map_err(|err| AppError::Provider(format!("agent bridge write: {err}")))?;
    drop(stdin);

    let model_name = request.model.name.clone();
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();

    let events = try_stream! {
        yield StreamEvent::MessageStart { model: model_name.clone() };
        let text_index = 0usize;
        let thinking_index = 1usize;
        let mut open_part: Option<(usize, PartKind)> = None;
        let mut started_text = false;

        while let Some(line) = lines.next_line().await.map_err(|err| AppError::Network(err.to_string()))? {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(line)
                .map_err(|err| AppError::Decode(format!("agent bridge line: {err} ({line})")))?;

            if let Some(err) = value.get("error").and_then(|v| v.as_str()) {
                Err(AppError::Network(err.to_string()))?;
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

        if let Some((idx, _)) = open_part.take() {
            yield StreamEvent::PartStop { index: idx };
        }

        if !started_text {
            Err(AppError::Network(
                "agent bridge returned no text (install deps: cd scripts/agent-bridge && npm install)".into(),
            ))?;
        }

        yield StreamEvent::MessageStop {
            stop_reason: StopReason::EndTurn,
            usage: Usage::default(),
        };
    };

    Ok(Box::pin(events))
}
