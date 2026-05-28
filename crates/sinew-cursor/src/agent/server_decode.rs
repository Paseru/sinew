//! Decode `AgentServerMessage` frames into bridge events.

use base64::Engine as _;
use prost::Message as _;
use prost_reflect::{DynamicMessage, Value};
use sinew_core::{AppError, Result};

use super::proto_dynamic::{get_i32_field, get_message_field, get_string_field};
use super::proto_pool::agent_pool;

#[derive(Debug, Clone)]
pub enum BridgeEvent {
    Text(String),
    Thinking(String),
    Usage { output_tokens: u32, total_tokens: u32 },
    ToolRequest {
        exec_id: String,
        exec_msg_id: String,
        tool_call_id: String,
        tool_name: String,
        args: serde_json::Value,
    },
    Checkpoint {
        checkpoint_b64: String,
        blobs: std::collections::HashMap<String, String>,
    },
    StepCompleted,
    TurnEnded,
}

pub fn decode_server_message(payload: &[u8]) -> Result<Vec<BridgeEvent>> {
    let desc = agent_pool()?
        .get_message_by_name("agent.v1.AgentServerMessage")
        .ok_or_else(|| AppError::Provider("AgentServerMessage descriptor missing".into()))?;
    let msg = DynamicMessage::decode(desc, payload)
        .map_err(|err| AppError::Decode(format!("AgentServerMessage: {err}")))?;
    Ok(collect_events(&msg))
}

fn collect_events(msg: &DynamicMessage) -> Vec<BridgeEvent> {
    let mut out = Vec::new();
    if let Some(update) = get_message_field(msg, "interaction_update") {
        out.extend(interaction_events(&update));
    }
    // Exec messages are handled inline in run_h2 (bidirectional loop).
    if let Some(checkpoint) = get_message_field(msg, "conversation_checkpoint_update") {
        if let Some(ev) = checkpoint_event(&checkpoint) {
            out.push(ev);
        }
    }
    out
}

fn interaction_events(update: &DynamicMessage) -> Vec<BridgeEvent> {
    let mut out = Vec::new();
    if let Some(inner) = get_message_field(update, "text_delta") {
        if let Some(text) = get_string_field(&inner, "text") {
            if !text.is_empty() {
                out.push(BridgeEvent::Text(text));
            }
        }
    }
    if let Some(inner) = get_message_field(update, "thinking_delta") {
        if let Some(text) = get_string_field(&inner, "text") {
            if !text.is_empty() {
                out.push(BridgeEvent::Thinking(text));
            }
        }
    }
    if let Some(inner) = get_message_field(update, "token_delta") {
        let tokens = get_i32_field(&inner, "tokens").unwrap_or(0).max(0) as u32;
        out.push(BridgeEvent::Usage {
            output_tokens: tokens,
            total_tokens: tokens,
        });
    }
    if get_message_field(update, "step_completed").is_some() {
        out.push(BridgeEvent::StepCompleted);
    }
    if get_message_field(update, "turn_ended").is_some() {
        out.push(BridgeEvent::TurnEnded);
    }
    out
}

pub fn decode_agent_server_message(payload: &[u8]) -> Result<DynamicMessage> {
    let desc = agent_pool()?
        .get_message_by_name("agent.v1.AgentServerMessage")
        .ok_or_else(|| AppError::Provider("AgentServerMessage descriptor missing".into()))?;
    DynamicMessage::decode(desc, payload)
        .map_err(|err| AppError::Decode(format!("AgentServerMessage: {err}")))
}

pub fn decode_mcp_args_from_message(mcp: &DynamicMessage) -> serde_json::Value {
    decode_mcp_args_map(mcp)
}

fn decode_mcp_args_map(mcp: &DynamicMessage) -> serde_json::Value {
    let Some(args_msg) = get_message_field(mcp, "args") else {
        return serde_json::Value::Null;
    };
    let mut map = serde_json::Map::new();
    for (key, value) in args_msg.fields() {
        map.insert(key.name().to_string(), value_to_json(value));
    }
    serde_json::Value::Object(map)
}

fn value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::I32(n) => serde_json::json!(*n),
        Value::I64(n) => serde_json::json!(*n),
        Value::U32(n) => serde_json::json!(*n),
        Value::U64(n) => serde_json::json!(*n),
        Value::F32(n) => serde_json::json!(*n),
        Value::F64(n) => serde_json::json!(*n),
        Value::Bytes(b) => {
            if let Ok(text) = std::str::from_utf8(b) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
                    return json;
                }
                return serde_json::Value::String(text.to_string());
            }
            serde_json::Value::String(base64::engine::general_purpose::STANDARD.encode(b))
        }
        Value::List(items) => {
            serde_json::Value::Array(items.iter().map(value_to_json).collect())
        }
        Value::Message(m) => {
            let mut obj = serde_json::Map::new();
            for (field, val) in m.fields() {
                obj.insert(field.name().to_string(), value_to_json(val));
            }
            serde_json::Value::Object(obj)
        }
        Value::EnumNumber(n) => serde_json::json!(*n),
        _ => serde_json::Value::Null,
    }
}

fn checkpoint_event(state: &DynamicMessage) -> Option<BridgeEvent> {
    let bytes = state.encode_to_vec();
    let checkpoint_b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(BridgeEvent::Checkpoint {
        checkpoint_b64,
        blobs: std::collections::HashMap::new(),
    })
}

pub fn parse_connect_end(payload: &[u8]) -> Option<String> {
    super::connect_proto::parse_connect_end_error(payload)
}
