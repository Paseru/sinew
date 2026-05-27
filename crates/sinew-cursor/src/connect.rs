use bytes::Bytes;
use serde_json::Value;
use sinew_core::{Result, Usage};

use crate::tools::{resolve_tool_call, ParsedToolCall};

pub fn frame_connect_json(payload: &[u8], flags: u8) -> Vec<u8> {
    let mut frame = Vec::with_capacity(5 + payload.len());
    frame.push(flags);
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}

pub fn append_end_stream_frame(buffer: &mut Vec<u8>) {
    buffer.extend(frame_connect_json(&[], 0x02));
}

pub fn decode_connect_frames(buffer: &mut Vec<u8>) -> Result<Vec<Bytes>> {
    let mut frames = Vec::new();
    loop {
        if buffer.len() < 5 {
            break;
        }
        let flags = buffer[0];
        let length = u32::from_be_bytes([buffer[1], buffer[2], buffer[3], buffer[4]]) as usize;
        if buffer.len() < 5 + length {
            break;
        }
        let payload = buffer[5..5 + length].to_vec();
        buffer.drain(..5 + length);
        if flags & 0x02 != 0 && payload.is_empty() {
            continue;
        }
        frames.push(Bytes::from(payload));
    }
    Ok(frames)
}

#[derive(Debug, Clone)]
pub enum ComposerEvent {
    Text(String),
    Thinking(String),
    ToolCall(ParsedToolCall),
    Usage(Usage),
}

pub fn parse_connect_events(payload: &[u8]) -> Result<Vec<ComposerEvent>> {
    let value: Value = match serde_json::from_slice(payload) {
        Ok(value) => value,
        Err(_) => return Ok(Vec::new()),
    };
    Ok(collect_events(&value))
}

fn collect_events(value: &Value) -> Vec<ComposerEvent> {
    let mut events = Vec::new();
    if let Some(server_chunk) = value
        .get("serverChunk")
        .or_else(|| value.get("server_chunk"))
    {
        push_events_from_value(server_chunk, &mut events);
        if let Some(nested) = server_chunk.get("streamUnifiedChatResponse") {
            push_events_from_value(nested, &mut events);
        }
        if let Some(nested) = server_chunk.get("stream_unified_chat_response") {
            push_events_from_value(nested, &mut events);
        }
        if let Some(nested) = server_chunk.get("clientSideToolV2Call") {
            if let Some(parsed) = resolve_tool_call(nested) {
                events.push(ComposerEvent::ToolCall(parsed));
            }
        }
    }
    push_events_from_value(value, &mut events);
    if let Some(nested) = value.get("streamUnifiedChatResponse") {
        push_events_from_value(nested, &mut events);
    }
    if let Some(nested) = value.get("stream_unified_chat_response") {
        push_events_from_value(nested, &mut events);
    }
    for key in [
        "clientSideToolV2Call",
        "client_side_tool_v2_call",
        "toolCallV2",
        "tool_call_v2",
        "partialToolCall",
        "partial_tool_call",
    ] {
        if let Some(call) = value.get(key) {
            if let Some(parsed) = resolve_tool_call(call) {
                events.push(ComposerEvent::ToolCall(parsed));
            }
        }
    }
    events
}

fn push_events_from_value(value: &Value, events: &mut Vec<ComposerEvent>) {
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        if !text.is_empty() {
            events.push(ComposerEvent::Text(text.to_string()));
        }
    }
    if let Some(thinking) = value.get("thinking") {
        if let Some(text) = thinking.as_str() {
            if !text.is_empty() {
                events.push(ComposerEvent::Thinking(text.to_string()));
            }
        } else if let Some(text) = thinking.get("text").and_then(Value::as_str) {
            if !text.is_empty() {
                events.push(ComposerEvent::Thinking(text.to_string()));
            }
        }
    }
    for key in [
        "clientSideToolV2Call",
        "client_side_tool_v2_call",
        "toolCallV2",
        "tool_call_v2",
        "partialToolCall",
        "partial_tool_call",
        "toolCall",
        "tool_call",
    ] {
        if let Some(call) = value.get(key) {
            if let Some(parsed) = resolve_tool_call(call) {
                events.push(ComposerEvent::ToolCall(parsed));
            }
        }
    }
    if let Some(error) = value.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Composer error");
        events.push(ComposerEvent::Text(format!("[error] {message}")));
    }
    if let Some(usage) = parse_usage_value(value) {
        events.push(ComposerEvent::Usage(usage));
    }
}

fn parse_usage_value(value: &Value) -> Option<Usage> {
    let usage = value
        .get("usage")
        .or_else(|| value.get("tokenUsage"))
        .or_else(|| value.get("token_usage"))
        .or_else(|| value.get("usageInfo"))
        .or_else(|| value.get("usage_info"))?;
    let input_tokens = usage_field_u32(usage, &["inputTokens", "input_tokens", "promptTokens", "prompt_tokens"]);
    let output_tokens = usage_field_u32(
        usage,
        &[
            "outputTokens",
            "output_tokens",
            "completionTokens",
            "completion_tokens",
        ],
    );
    let total_tokens = usage_field_u32(usage, &["totalTokens", "total_tokens"]).max(input_tokens + output_tokens);
    let reasoning_tokens = usage_field_u32(
        usage,
        &["reasoningTokens", "reasoning_tokens", "thinkingTokens", "thinking_tokens"],
    );
    let cache_read_tokens = usage_field_u32(
        usage,
        &["cacheReadTokens", "cache_read_tokens", "cachedInputTokens", "cached_input_tokens"],
    );
    let cache_creation_tokens = usage_field_u32(
        usage,
        &[
            "cacheCreationTokens",
            "cache_creation_tokens",
            "cacheWriteTokens",
            "cache_write_tokens",
        ],
    );
    if input_tokens == 0
        && output_tokens == 0
        && total_tokens == 0
        && reasoning_tokens == 0
        && cache_read_tokens == 0
        && cache_creation_tokens == 0
    {
        return None;
    }
    Some(Usage {
        input_tokens,
        output_tokens,
        total_tokens,
        reasoning_tokens,
        cache_read_tokens,
        cache_creation_tokens,
    })
}

fn usage_field_u32(value: &Value, keys: &[&str]) -> u32 {
    for key in keys {
        if let Some(number) = value.get(*key).and_then(Value::as_u64) {
            return number.min(u32::MAX as u64) as u32;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn end_stream_frame_uses_connect_end_flag() {
        let mut body = frame_connect_json(b"{}", 0);
        append_end_stream_frame(&mut body);
        assert_eq!(body.len(), 10);
        assert_eq!(body[5], 0x02);
    }

    #[test]
    fn parses_nested_usage_payload() {
        let events = collect_events(&json!({
            "streamUnifiedChatResponse": {
                "usage": {
                    "inputTokens": 120,
                    "outputTokens": 45,
                    "reasoningTokens": 10
                }
            }
        }));
        assert!(events.iter().any(|event| matches!(
            event,
            ComposerEvent::Usage(usage)
                if usage.input_tokens == 120
                    && usage.output_tokens == 45
                    && usage.reasoning_tokens == 10
        )));
    }

    #[test]
    fn resolves_unsupported_tool_calls_from_stream() {
        let events = collect_events(&json!({
            "clientSideToolV2Call": {
                "tool": "CLIENT_SIDE_TOOL_V2_APPLY_PATCH",
                "toolCallId": "call_unknown"
            }
        }));
        assert!(events.iter().any(|event| matches!(
            event,
            ComposerEvent::ToolCall(call)
                if call.sinew_name == crate::tools::COMPOSER_UNSUPPORTED_TOOL
                    && call.cursor_tool == "CLIENT_SIDE_TOOL_V2_APPLY_PATCH"
        )));
    }
}
