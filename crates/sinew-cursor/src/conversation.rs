use std::collections::HashMap;

use sha2::{Digest, Sha256};
use serde_json::{json, Value};
use sinew_core::{Effort, Part, ProviderRequest, Role, ServiceTier, ToolDescriptor};

use crate::{
    context_injection::append_local_index_excerpts,
    encryption::BlobEncryptionKey,
    identity::CursorIdeIdentity,
    images::message_images,
    sanitize::{sanitize_outbound_json, sanitize_outbound_text},
    tools::{build_client_tool_result, cursor_tool_name, is_mappable_sinew_tool, SUPPORTED_TOOLS},
    workspace::{snapshot, WorkspaceSnapshot},
};

const HEADER_ONLY_THRESHOLD: usize = 24;

pub fn build_stream_request(
    request: &ProviderRequest,
    conversation_id: &str,
    idempotency_key: &str,
    seqno: u32,
    identity: &CursorIdeIdentity,
    encryption_key: &BlobEncryptionKey,
) -> (Vec<u8>, u32) {
    if is_tool_result_continuation(request) {
        let (mut framed, seqno) = build_tool_result_frames(request, idempotency_key, seqno);
        crate::connect::append_end_stream_frame(&mut framed);
        return (framed, seqno);
    }
    let body = build_full_request(request, conversation_id, identity, encryption_key);
    let chunk = json!({
        "clientChunk": body,
        "idempotencyKey": idempotency_key,
        "seqno": seqno,
    });
    let payload = serde_json::to_vec(&chunk).unwrap_or_default();
    let mut framed = crate::connect::frame_connect_json(&payload, 0);
    crate::connect::append_end_stream_frame(&mut framed);
    (framed, seqno + 1)
}

fn build_full_request(
    request: &ProviderRequest,
    conversation_id: &str,
    identity: &CursorIdeIdentity,
    encryption_key: &BlobEncryptionKey,
) -> Value {
    let body_key = encryption_key.body_json_string();
    let workspace = request
        .workspace_root
        .as_deref()
        .and_then(snapshot);
    let (model_name, enable_slow_pool, max_mode, thinking_level) =
        model_details(&request.model.name, request.effective_effort(), request.service_tier);
    let (conversation, headers) = build_conversation(request, conversation_id);
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
            "enableYoloMode": false,
            "supportedTools": SUPPORTED_TOOLS,
            "conversationId": conversation_id,
            "environmentInfo": build_environment_info(identity, workspace.as_ref()),
            "shouldDisableTools": false,
            "allowModelFallbacks": false,
            "mcpTools": build_mcp_tools(&request.tools),
            "blobEncryptionKey": body_key,
            "speculativeSummarizationEncryptionKey": body_key,
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
        json!([{
            "uri": sanitize_outbound_text(&workspace.uri),
            "name": sanitize_outbound_text(&workspace.name),
        }]),
    );
    req.insert(
        "projectLayouts".into(),
        json!([{
            "rootPath": sanitize_outbound_text(&workspace.name),
            "content": sanitize_outbound_json(workspace.project_layout.clone()),
        }]),
    );
    if let Some(branch) = workspace.branch.as_ref() {
        req.insert(
            "repositoryInfo".into(),
            json!({
                "repoName": sanitize_outbound_text(&workspace.name),
                "branchName": sanitize_outbound_text(branch),
                "workspaceUri": sanitize_outbound_text(&workspace.uri),
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
    append_local_index_excerpts(request, &mut context);
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
                "name": sanitize_outbound_text(&tool.name),
                "providerIdentifier": "cursor-mcp",
                "toolName": sanitize_outbound_text(&tool.name),
                "description": sanitize_outbound_text(&tool.description),
                "inputSchema": sanitize_outbound_json(tool.input_schema.clone()),
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

fn build_conversation(request: &ProviderRequest, conversation_id: &str) -> (Vec<Value>, Vec<Value>) {
    let mut messages = Vec::new();
    let mut headers = Vec::new();
    let mut pending_calls: HashMap<String, (String, String, Value)> = HashMap::new();
    let mut message_index = 0usize;

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
                    let bubble_id =
                        stable_bubble_id(conversation_id, message_index, Role::Assistant, message);
                    headers.push(json!({
                        "bubbleId": bubble_id,
                        "type": "MESSAGE_TYPE_AI",
                    }));
                    let mut entry = json!({
                        "type": "MESSAGE_TYPE_AI",
                        "text": text,
                        "bubbleId": bubble_id,
                        "isAgentic": true,
                    });
                    if !tool_calls.is_empty() {
                        entry["clientSideToolV2Calls"] =
                            Value::Array(assistant_tool_calls_payload(&tool_calls));
                    }
                    messages.push(entry);
                    message_index += 1;
                }
            }
            Role::User => {
                let text = message_text(message);
                let images = message_images(message);
                let tool_results = tool_results_from_message(message, &pending_calls);
                if text.is_empty() && tool_results.is_empty() && images.is_empty() {
                    continue;
                }
                let bubble_id =
                    stable_bubble_id(conversation_id, message_index, Role::User, message);
                let request_id = stable_request_id(conversation_id, message_index, message);
                headers.push(json!({
                    "bubbleId": bubble_id,
                    "type": "MESSAGE_TYPE_HUMAN",
                }));
                let mut entry = json!({
                    "type": "MESSAGE_TYPE_HUMAN",
                    "text": text,
                    "bubbleId": bubble_id,
                    "requestId": request_id,
                });
                if !images.is_empty() {
                    entry["images"] = Value::Array(images);
                }
                if !tool_results.is_empty() {
                    entry["toolResults"] = Value::Array(tool_results);
                }
                if let Some(status) = request
                    .workspace_root
                    .as_deref()
                    .and_then(snapshot)
                    .and_then(|snapshot| snapshot.git_status)
                {
                    entry["gitStatusRaw"] = json!(sanitize_outbound_text(&status));
                }
                messages.push(entry);
                message_index += 1;
            }
        }
    }

    if messages.is_empty() {
        let bubble_id = stable_bubble_id(
            conversation_id,
            0,
            Role::User,
            &sinew_core::ChatMessage::user_text("Continue."),
        );
        let request_id = stable_request_id(
            conversation_id,
            0,
            &sinew_core::ChatMessage::user_text("Continue."),
        );
        messages.push(json!({
            "type": "MESSAGE_TYPE_HUMAN",
            "text": "Continue.",
            "bubbleId": bubble_id,
            "requestId": request_id,
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
            } => {
                let cursor_tool = resolve_cursor_tool(name, meta)?;
                Some((
                    id.clone(),
                    name.clone(),
                    cursor_tool,
                    sanitize_outbound_json(input.clone()),
                ))
            }
            _ => None,
        })
        .collect()
}

fn resolve_cursor_tool(name: &str, meta: &Option<Value>) -> Option<String> {
    if name.starts_with("mcp__") {
        return Some("CLIENT_SIDE_TOOL_V2_CALL_MCP_TOOL".into());
    }
    if !is_mappable_sinew_tool(name) {
        return None;
    }
    meta.as_ref()
        .and_then(|value| value.get("cursor_tool"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            let mapped = cursor_tool_name(name);
            (mapped != "CLIENT_SIDE_TOOL_V2_UNSPECIFIED").then(|| mapped.to_string())
        })
}

fn assistant_tool_calls_payload(
    tool_calls: &[(String, String, String, Value)],
) -> Vec<Value> {
    tool_calls
        .iter()
        .map(|(id, _name, cursor_tool, input)| {
            let raw_args = serde_json::to_string(&input).unwrap_or_else(|_| "{}".into());
            json!({
                "tool": cursor_tool,
                "toolCallId": id,
                "rawArgs": sanitize_outbound_text(&raw_args),
            })
        })
        .collect()
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
                images,
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
                    "args": sanitize_outbound_text(&args.to_string()),
                    "rawArgs": sanitize_outbound_text(&args.to_string()),
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
                        Some(images),
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
            images,
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
                    Some(images),
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
    let is_fast = match service_tier {
        Some(ServiceTier::Fast) => true,
        _ => model == "composer-2.5-fast",
    };

    let (model_name, default_slow_pool, max_mode, default_thinking) = if is_fast {
        (
            "composer-2.5-fast".into(),
            false,
            false,
            Some("THINKING_LEVEL_MEDIUM"),
        )
    } else {
        (
            "composer-2.5".into(),
            false,
            true,
            Some("THINKING_LEVEL_HIGH"),
        )
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

fn stable_bubble_id(
    conversation_id: &str,
    message_index: usize,
    role: Role,
    message: &sinew_core::ChatMessage,
) -> String {
    if let Some(id) = message
        .parts
        .iter()
        .find_map(|part| part_meta(part).and_then(|meta| meta.get("cursor_bubble_id")))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        return id.to_string();
    }
    deterministic_uuid(
        "bubble",
        conversation_id,
        message_index,
        role,
        &message_fingerprint(message),
    )
}

fn stable_request_id(
    conversation_id: &str,
    message_index: usize,
    message: &sinew_core::ChatMessage,
) -> String {
    if let Some(id) = message
        .parts
        .iter()
        .find_map(|part| part_meta(part).and_then(|meta| meta.get("cursor_request_id")))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        return id.to_string();
    }
    deterministic_uuid(
        "request",
        conversation_id,
        message_index,
        Role::User,
        &message_fingerprint(message),
    )
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

fn message_fingerprint(message: &sinew_core::ChatMessage) -> String {
    let mut hasher = Sha256::new();
    for part in &message.parts {
        match part {
            Part::Text { text, .. } => {
                hasher.update(b"text:");
                hasher.update(text.as_bytes());
            }
            Part::Image { media_type, data, .. } => {
                hasher.update(b"image:");
                hasher.update(media_type.as_bytes());
                hasher.update(data.as_bytes());
            }
            Part::Thinking { text, .. } => {
                hasher.update(b"thinking:");
                hasher.update(text.as_bytes());
            }
            Part::ToolCall {
                id,
                name,
                input,
                ..
            } => {
                hasher.update(b"tool_call:");
                hasher.update(id.as_bytes());
                hasher.update(name.as_bytes());
                if let Ok(encoded) = serde_json::to_vec(input) {
                    hasher.update(&encoded);
                }
            }
            Part::ToolResult {
                tool_call_id,
                content,
                is_error,
                ..
            } => {
                hasher.update(b"tool_result:");
                hasher.update(tool_call_id.as_bytes());
                hasher.update(content.as_bytes());
                hasher.update([u8::from(*is_error)]);
            }
        }
    }
    hex_digest(hasher.finalize())
}

fn deterministic_uuid(
    kind: &str,
    conversation_id: &str,
    message_index: usize,
    role: Role,
    fingerprint: &str,
) -> String {
    let role_tag = match role {
        Role::User => "human",
        Role::Assistant => "ai",
    };
    let seed = format!("{kind}:{conversation_id}:{message_index}:{role_tag}:{fingerprint}");
    let digest = Sha256::digest(seed.as_bytes());
    let bytes: [u8; 16] = digest[..16]
        .try_into()
        .unwrap_or([0u8; 16]);
    uuid::Uuid::from_bytes(bytes).to_string()
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use sinew_core::{ChatMessage, ModelRef, Part, Role};

    #[test]
    fn human_message_includes_attached_images() {
        let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
        let request = ProviderRequest::new(
            ModelRef::new("cursor", "composer-2.5-fast"),
            vec![ChatMessage {
                role: Role::User,
                parts: vec![
                    Part::Image {
                        media_type: "image/png".into(),
                        data: png.into(),
                        meta: None,
                    },
                    Part::Text {
                        text: "Describe this screenshot.".into(),
                        meta: None,
                    },
                ],
            }],
        )
        .with_cache_key("conv-image-test");
        let (messages, _) = build_conversation(&request, "conv-image-test");
        assert_eq!(messages.len(), 1);
        let images = messages[0]["images"].as_array().expect("images array");
        assert_eq!(images.len(), 1);
        assert_eq!(images[0]["data"].as_str(), Some(png));
        assert_eq!(images[0]["dimension"]["width"], 1);
    }

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

    #[test]
    fn bubble_ids_stay_stable_for_same_transcript() {
        let request = ProviderRequest::new(
            ModelRef::new("cursor", "composer-2.5-fast"),
            vec![
                ChatMessage::user_text("Find auth code"),
                ChatMessage::assistant_text("I'll search the codebase."),
            ],
        )
        .with_cache_key("conv-stable-test");
        let (first, _) = build_conversation(&request, "conv-stable-test");
        let (second, _) = build_conversation(&request, "conv-stable-test");
        assert_eq!(
            first[0]["bubbleId"].as_str(),
            second[0]["bubbleId"].as_str()
        );
        assert_eq!(
            first[1]["bubbleId"].as_str(),
            second[1]["bubbleId"].as_str()
        );
    }
}
