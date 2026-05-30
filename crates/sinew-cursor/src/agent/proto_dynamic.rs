//! Shared prost-reflect helpers for agent.v1 messages.

use prost_reflect::{DynamicMessage, ReflectMessage, SetFieldError, Value};
use sinew_core::{AppError, Result};

use super::proto_pool::agent_pool;

pub fn message_desc(name: &str) -> Result<prost_reflect::MessageDescriptor> {
    agent_pool()?
        .get_message_by_name(name)
        .ok_or_else(|| AppError::Provider(format!("proto message not found: {name}")))
}

pub fn setf(msg: &mut DynamicMessage, name: &str, value: Value) -> Result<()> {
    msg.try_set_field_by_name(name, value).map_err(field_err)
}

pub fn field_err(err: SetFieldError) -> AppError {
    AppError::Provider(format!("proto field: {err}"))
}

pub fn get_message_field(msg: &DynamicMessage, name: &str) -> Option<DynamicMessage> {
    let field = msg.descriptor().get_field_by_name(name)?;
    if !msg.has_field(&field) {
        return None;
    }
    match msg.get_field(&field).as_ref() {
        Value::Message(m) => Some(m.clone()),
        _ => None,
    }
}

pub fn get_string_field(msg: &DynamicMessage, name: &str) -> Option<String> {
    match msg.get_field_by_name(name)?.as_ref() {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

pub fn get_i32_field(msg: &DynamicMessage, name: &str) -> Option<i32> {
    match msg.get_field_by_name(name)?.as_ref() {
        Value::I32(n) => Some(*n),
        _ => None,
    }
}

pub fn get_u32_field(msg: &DynamicMessage, name: &str) -> Option<u32> {
    match msg.get_field_by_name(name)?.as_ref() {
        Value::U32(n) => Some(*n),
        _ => None,
    }
}

pub fn get_bytes_field(msg: &DynamicMessage, name: &str) -> Option<Vec<u8>> {
    match msg.get_field_by_name(name)?.as_ref() {
        Value::Bytes(b) => Some(b.to_vec()),
        _ => None,
    }
}

/// First set field in a protobuf oneof (exec/kv-style messages).
pub fn oneof_case(msg: &DynamicMessage) -> Option<String> {
    for oneof in msg.descriptor().oneofs() {
        for field in oneof.fields() {
            if msg.has_field(&field) {
                return Some(field.name().to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_message_field_ignores_unset_message_defaults() {
        let asm_desc = message_desc("agent.v1.AgentServerMessage").expect("AgentServerMessage");
        let kv_desc = message_desc("agent.v1.KvServerMessage").expect("KvServerMessage");
        let mut asm = DynamicMessage::new(asm_desc);

        assert!(get_message_field(&asm, "exec_server_message").is_none());
        assert!(get_message_field(&asm, "kv_server_message").is_none());

        setf(
            &mut asm,
            "kv_server_message",
            Value::Message(DynamicMessage::new(kv_desc)),
        )
        .expect("kv_server_message");

        assert_eq!(oneof_case(&asm).as_deref(), Some("kv_server_message"));
        assert!(get_message_field(&asm, "exec_server_message").is_none());
        assert!(get_message_field(&asm, "kv_server_message").is_some());
    }
}
