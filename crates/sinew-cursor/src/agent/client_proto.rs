//! Encode outbound `AgentClientMessage` frames.

use bytes::Bytes;
use prost::Message as _;
use prost_reflect::{DynamicMessage, Value};

use sinew_core::Result;

use super::connect_proto::frame_connect_proto;
use super::proto_dynamic::{message_desc, setf};

pub fn frame_client_message(msg: &DynamicMessage) -> Vec<u8> {
    frame_connect_proto(&msg.encode_to_vec())
}

pub fn encode_client_heartbeat() -> Result<Vec<u8>> {
    let client_desc = message_desc("agent.v1.AgentClientMessage")?;
    let hb_desc = message_desc("agent.v1.ClientHeartbeat")?;
    let mut client = DynamicMessage::new(client_desc);
    let heartbeat = DynamicMessage::new(hb_desc);
    setf(&mut client, "client_heartbeat", Value::Message(heartbeat))?;
    Ok(frame_client_message(&client))
}

pub fn encode_exec_client_message(
    exec_id: &str,
    id: u32,
    result_field: &str,
    result: DynamicMessage,
) -> Result<Vec<u8>> {
    let exec_desc = message_desc("agent.v1.ExecClientMessage")?;
    let mut exec = DynamicMessage::new(exec_desc);
    setf(&mut exec, "exec_id", Value::String(exec_id.to_string()))?;
    setf(&mut exec, "id", Value::U32(id))?;
    setf(&mut exec, result_field, Value::Message(result))?;

    let client_desc = message_desc("agent.v1.AgentClientMessage")?;
    let mut client = DynamicMessage::new(client_desc);
    setf(
        &mut client,
        "exec_client_message",
        Value::Message(exec),
    )?;
    Ok(frame_client_message(&client))
}

pub fn encode_kv_get_blob_result(id: u32, blob_data: Option<Vec<u8>>) -> Result<Vec<u8>> {
    let get_desc = message_desc("agent.v1.GetBlobResult")?;
    let mut get_result = DynamicMessage::new(get_desc);
    if let Some(data) = blob_data {
        setf(
            &mut get_result,
            "blob_data",
            Value::Bytes(Bytes::from(data)),
        )?;
    }

    let kv_desc = message_desc("agent.v1.KvClientMessage")?;
    let mut kv = DynamicMessage::new(kv_desc);
    setf(&mut kv, "id", Value::U32(id))?;
    setf(&mut kv, "get_blob_result", Value::Message(get_result))?;

    let client_desc = message_desc("agent.v1.AgentClientMessage")?;
    let mut client = DynamicMessage::new(client_desc);
    setf(&mut client, "kv_client_message", Value::Message(kv))?;
    Ok(frame_client_message(&client))
}

pub fn encode_kv_set_blob_result(id: u32) -> Result<Vec<u8>> {
    let set_desc = message_desc("agent.v1.SetBlobResult")?;
    let set_result = DynamicMessage::new(set_desc);

    let kv_desc = message_desc("agent.v1.KvClientMessage")?;
    let mut kv = DynamicMessage::new(kv_desc);
    setf(&mut kv, "id", Value::U32(id))?;
    setf(&mut kv, "set_blob_result", Value::Message(set_result))?;

    let client_desc = message_desc("agent.v1.AgentClientMessage")?;
    let mut client = DynamicMessage::new(client_desc);
    setf(&mut client, "kv_client_message", Value::Message(kv))?;
    Ok(frame_client_message(&client))
}
