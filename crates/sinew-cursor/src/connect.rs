use bytes::Bytes;
use serde_json::Value;
use sinew_core::Result;

use crate::tools::{parse_tool_call, ParsedToolCall};

pub fn frame_connect_json(payload: &[u8], flags: u8) -> Vec<u8> {
    let mut frame = Vec::with_capacity(5 + payload.len());
    frame.push(flags);
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(payload);
    frame
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
            if let Some(parsed) = parse_tool_call(nested) {
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
            if let Some(parsed) = parse_tool_call(call) {
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
            if let Some(parsed) = parse_tool_call(call) {
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
}
