use std::collections::HashMap;

use serde_json::{json, Value};
use sinew_core::{Effort, Part, ProviderRequest, Role, ServiceTier, ToolDescriptor};

use crate::{
    identity::CursorIdeIdentity,
    tools::{build_client_tool_result, cursor_tool_name, SUPPORTED_TOOLS},
    workspace::{snapshot, WorkspaceSnapshot},
};

const HEADER_ONLY_THRESHOLD: usize = 24;

pub fn build_stream_request(
    request: &ProviderRequest,
    conversation_id: &str,
    idempotency_key: &str,
    seqno: u32,
    identity: &CursorIdeIdentity,
) -> (Vec<u8>, u32) {
    if is_tool_result_continuation(request) {
        return build_tool_result_frames(request, idempotency_key, seqno);
    }
    let body = build_full_request(request, conversation_id, identity);
    let chunk = json!({
        "clientChunk": body,
        "idempotencyKey": idempotency_key,
        "seqno": seqno,
    });
    let payload = serde_json::to_vec(&chunk).unwrap_or_default();
    (crate::connect::frame_connect_json(&payload, 0), seqno + 1)
}

fn build_full_request(
    request: &ProviderRequest,
    conversation_id: &str,
    identity: &CursorIdeIdentity,
) -> Value {
    let workspace = request
        .workspace_root
        .as_deref()
        .and_then(snapshot);
    let (model_name, enable_slow_pool, max_mode, thinking_level) =
        model_details(&request.model.name, request.effective_effort(), request.service_tier);
    let (conversation, headers) = build_conversation(request);
    let explicit_context = build_explicit_context(request, &request.tools);
    let mut stream_request = json!({
        "streamUnifiedChatRequest": {
            "conversation": conversation,
            "explicitContext": explicit_context,
            "modelDetails": {
                "modelName": model_name,
                "enableSlowPool": enable_slow_pool,
                "maxMode": max_mode
            },
            "isAgentic": true,
            "isChat": false,
            "unifiedMode": "UNIFIED_MODE_AGENT",
            "useUnifiedChatPrompt": true,
            "enableYoloMode": true,
            "supportedTools": SUPPORTED_TOOLS,
            "conversationId": conversation_id,
            "environmentInfo": build_environment_info(identity, workspace.as_ref()),
            "shouldDisableTools": false,
            "allowModelFallbacks": false,
            "mcpTools": build_mcp_tools(&request.tools),
        }
    });
    if let Some(level) = thinking_level {
        stream_request["streamUnifiedChatRequest"]["thinkingLevel"] = json!(level);
    }
    if let Some(workspace) = workspace.as_ref() {
        attach_workspace_context(&mut stream_request, workspace);
    }
    if !headers.is_empty() {
        stream_request["streamUnifiedChatRequest"]["fullConversationHeadersOnly"] = Value::Array(headers);
    }
    stream_request
}

fn attach_workspace_context(stream_request: &mut Value, workspace: &WorkspaceSnapshot) {
    let req = stream_request
        .get_mut("streamUnifiedChatRequest")
        .and_then(Value::as_object_mut);
    let Some(req) = req else {
        return;
    };
    req.insert(
        "workspaceFolders".into(),
        json!([{ "uri": workspace.uri, "name": workspace.name }]),
    );
    req.insert(
        "projectLayouts".into(),
        json!([{
            "rootPath": workspace.name,
            "content": workspace.project_layout,
        }]),
    );
    if let Some(branch) = workspace.branch.as_ref() {
        req.insert(
            "repositoryInfo".into(),
            json!({
                "repoName": workspace.name,
                "branchName": branch,
                "workspaceUri": workspace.uri,
            }),
        );
    }
}

fn build_explicit_context(request: &ProviderRequest, tools: &[ToolDescriptor]) -> Value {
    let mut context = request
        .system_prompt
        .as_deref()
        .map(sanitize_outbound_text)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            "You are Composer in Cursor IDE, a coding assistant. Respond in French when the user writes in French.".into()
        });
    let mcp_summary = mcp_instructions(tools);
    if !mcp_summary.is_empty() {
        context.push_str("\n\n");
        context.push_str(&mcp_summary);
    }
    json!({ "context": context })
}

fn mcp_instructions(tools: &[ToolDescriptor]) -> String {
    let mut lines = Vec::new();
    for tool in tools {
        if tool.name.starts_with("mcp__") {
            lines.push(format!(
                "- {}: {}",
                tool.name,
                sanitize_outbound_text(&tool.description)
            ));
        }
    }
    if lines.is_empty() {
        return String::new();
    }
    format!("Available MCP tools:\n{}", lines.join("\n"))
}

fn build_mcp_tools(tools: &[ToolDescriptor]) -> Vec<Value> {
    tools
        .iter()
        .filter(|tool| tool.name.starts_with("mcp__"))
        .map(|tool| {
            json!({
                "name": tool.name,
                "providerIdentifier": "cursor-mcp",
                "toolName": tool.name,
                "description": sanitize_outbound_text(&tool.description),
                "inputSchema": tool.input_schema,
            })
        })
        .collect()
}

fn build_environment_info(identity: &CursorIdeIdentity, workspace: Option<&WorkspaceSnapshot>) -> Value {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let mut info = json!({
        "exthostPlatform": identity.platform,
        "exthostArch": identity.arch,
        "exthostShell": identity.shell,
        "cursorVersion": identity.client_version,
        "localOsType": identity.platform,
        "localTimezone": identity.timezone,
        "homeDirectory": home,
        "isRemote": false,
    });
    if let Some(workspace) = workspace {
        if let Some(obj) = info.as_object_mut() {
            obj.insert(
                "workspaceUris".into(),
                Value::Array(vec![Value::String(workspace.uri.clone())]),
            );
        }
    }
    info
}

fn build_conversation(request: &ProviderRequest) -> (Vec<Value>, Vec<Value>) {
    let mut messages = Vec::new();
    let mut headers = Vec::new();
    let mut pending_calls: HashMap<String, (String, String, Value)> = HashMap::new();

    for message in &request.transcript {
        match message.role {
            Role::Assistant => {
                let text = message_text(message);
                let tool_calls = tool_calls_from_message(message);
                for (id, name, cursor_tool, input) in &tool_calls {
                    pending_calls.insert(
                        id.clone(),
                        (name.clone(), cursor_tool.clone(), input.clone()),
                    );
                }
                if !text.is_empty() || !tool_calls.is_empty() {
                    let bubble_id = uuid::Uuid::new_v4().to_string();
                    headers.push(json!({
                        "bubbleId": bubble_id,
                        "type": "MESSAGE_TYPE_AI",
                    }));
                    messages.push(json!({
                        "type": "MESSAGE_TYPE_AI",
                        "text": text,
                        "bubbleId": bubble_id,
                        "isAgentic": true,
                    }));
                }
            }
            Role::User => {
                let text = message_text(message);
                let tool_results = tool_results_from_message(message, &pending_calls);
                if text.is_empty() && tool_results.is_empty() {
                    continue;
                }
                let bubble_id = uuid::Uuid::new_v4().to_string();
                headers.push(json!({
                    "bubbleId": bubble_id,
                    "type": "MESSAGE_TYPE_HUMAN",
                }));
                let mut entry = json!({
                    "type": "MESSAGE_TYPE_HUMAN",
                    "text": text,
                    "bubbleId": bubble_id,
                    "requestId": uuid::Uuid::new_v4().to_string(),
                });
                if !tool_results.is_empty() {
                    entry["toolResults"] = Value::Array(tool_results);
                }
                if let Some(status) = request
                    .workspace_root
                    .as_deref()
                    .and_then(snapshot)
                    .and_then(|snapshot| snapshot.git_status)
                {
                    entry["gitStatusRaw"] = json!(status);
                }
                messages.push(entry);
            }
        }
    }

    if messages.is_empty() {
        messages.push(json!({
            "type": "MESSAGE_TYPE_HUMAN",
            "text": "Continue.",
            "bubbleId": uuid::Uuid::new_v4().to_string(),
            "requestId": uuid::Uuid::new_v4().to_string(),
        }));
    }

    let header_only = if messages.len() >= HEADER_ONLY_THRESHOLD {
        headers
    } else {
        Vec::new()
    };
    (messages, header_only)
}

fn tool_calls_from_message(
    message: &sinew_core::ChatMessage,
) -> Vec<(String, String, String, Value)> {
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            Part::ToolCall {
                id,
                name,
                input,
                meta,
            } => Some((
                id.clone(),
                name.clone(),
                resolve_cursor_tool(name, meta),
                input.clone(),
            )),
            _ => None,
        })
        .collect()
}

fn resolve_cursor_tool(name: &str, meta: &Option<Value>) -> String {
    meta.as_ref()
        .and_then(|value| value.get("cursor_tool"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| cursor_tool_name(name).to_string())
}

fn tool_results_from_message(
    message: &sinew_core::ChatMessage,
    pending_calls: &HashMap<String, (String, String, Value)>,
) -> Vec<Value> {
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            Part::ToolResult {
                tool_call_id,
                content,
                is_error,
                ..
            } => {
                let (sinew_name, cursor_tool, args) = pending_calls
                    .get(tool_call_id)
                    .cloned()
                    .unwrap_or_else(|| {
                        (
                            "bash".into(),
                            "CLIENT_SIDE_TOOL_V2_RUN_TERMINAL_COMMAND_V2".into(),
                            json!({}),
                        )
                    });
                Some(json!({
                    "toolCallId": tool_call_id,
                    "toolName": cursor_tool,
                    "toolIndex": 0,
                    "content": sanitize_outbound_text(content),
                    "args": args.to_string(),
                    "rawArgs": args.to_string(),
                    "error": if *is_error {
                        json!({ "message": sanitize_outbound_text(content) })
                    } else {
                        Value::Null
                    },
                    "result": build_client_tool_result(
                        tool_call_id,
                        &sinew_name,
                        &cursor_tool,
                        content,
                        *is_error,
                    ),
                }))
            }
            _ => None,
        })
        .collect()
}

fn is_tool_result_continuation(request: &ProviderRequest) -> bool {
    let Some(last) = request.transcript.last() else {
        return false;
    };
    matches!(last.role, Role::User)
        && last.parts.iter().all(|part| matches!(part, Part::ToolResult { .. }))
        && !last.parts.is_empty()
}

fn build_tool_result_frames(
    request: &ProviderRequest,
    idempotency_key: &str,
    mut seqno: u32,
) -> (Vec<u8>, u32) {
    let mut pending_calls: HashMap<String, (String, String, Value)> = HashMap::new();
    for message in &request.transcript {
        if matches!(message.role, Role::Assistant) {
            for (id, name, cursor_tool, input) in tool_calls_from_message(message) {
                pending_calls.insert(id, (name, cursor_tool, input));
            }
        }
    }
    let Some(last) = request.transcript.last() else {
        return (Vec::new(), seqno);
    };
    let mut framed = Vec::new();
    for part in &last.parts {
        let Part::ToolResult {
            tool_call_id,
            content,
            is_error,
            ..
        } = part
        else {
            continue;
        };
        let (sinew_name, cursor_tool, _) = pending_calls.get(tool_call_id).cloned().unwrap_or_else(|| {
            (
                "bash".into(),
                "CLIENT_SIDE_TOOL_V2_RUN_TERMINAL_COMMAND_V2".into(),
                json!({}),
            )
        });
        let chunk = json!({
            "clientChunk": {
                "clientSideToolV2Result": build_client_tool_result(
                    tool_call_id,
                    &sinew_name,
                    &cursor_tool,
                    content,
                    *is_error,
                )
            },
            "idempotencyKey": idempotency_key,
            "seqno": seqno,
        });
        if let Ok(payload) = serde_json::to_vec(&chunk) {
            framed.extend(crate::connect::frame_connect_json(&payload, 0));
            seqno += 1;
        }
    }
    (framed, seqno)
}

fn model_details(
    model: &str,
    effort: Option<Effort>,
    service_tier: Option<ServiceTier>,
) -> (String, bool, bool, Option<&'static str>) {
    let (model_name, default_slow_pool, max_mode, default_thinking) = match model {
        "composer-2.5" => (
            "composer-2.5".into(),
            false,
            true,
            Some("THINKING_LEVEL_HIGH"),
        ),
        _ => (
            "composer-2.5-fast".into(),
            false,
            false,
            Some("THINKING_LEVEL_MEDIUM"),
        ),
    };
    let enable_slow_pool = match service_tier {
        Some(ServiceTier::Flex) => true,
        Some(ServiceTier::Fast) => false,
        None => default_slow_pool,
    };
    let thinking_level = effort_to_thinking_level(effort).or(default_thinking);
    (model_name, enable_slow_pool, max_mode, thinking_level)
}

fn effort_to_thinking_level(effort: Option<Effort>) -> Option<&'static str> {
    match effort {
        None => None,
        Some(Effort::None) => Some("THINKING_LEVEL_NONE"),
        Some(Effort::Low) => Some("THINKING_LEVEL_LOW"),
        Some(Effort::Medium) => Some("THINKING_LEVEL_MEDIUM"),
        Some(Effort::High) => Some("THINKING_LEVEL_HIGH"),
        Some(Effort::Xhigh) | Some(Effort::Max) => Some("THINKING_LEVEL_XHIGH"),
    }
}

fn message_text(message: &sinew_core::ChatMessage) -> String {
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            Part::Text { text, .. } if !text.trim().is_empty() => {
                Some(sanitize_outbound_text(text))
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn sanitize_outbound_text(text: &str) -> String {
    text.replace("Sinew", "Cursor")
        .replace("sinew", "cursor")
        .replace("HYRAK", "Cursor")
        .replace("Hyrak", "Cursor")
        .replace("hyrak", "cursor")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sinew_core::{ChatMessage, ModelRef};

    #[test]
    fn tool_result_continuation_detected() {
        let request = ProviderRequest::new(
            ModelRef::new("cursor", "composer-2.5-fast"),
            vec![ChatMessage {
                role: Role::User,
                parts: vec![Part::ToolResult {
                    tool_call_id: "call_1".into(),
                    content: "ok".into(),
                    images: Vec::new(),
                    is_error: false,
                    meta: None,
                }],
            }],
        );
        assert!(is_tool_result_continuation(&request));
    }

    #[test]
    fn effort_maps_to_thinking_level() {
        use sinew_core::Effort;

        let (_, _, _, level) = super::model_details("composer-2.5-fast", Some(Effort::High), None);
        assert_eq!(level, Some("THINKING_LEVEL_HIGH"));
    }
}
