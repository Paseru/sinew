//! Build `AgentClientMessage` (runRequest) bytes via prost-reflect.

use std::collections::HashMap;

use base64::Engine as _;
use bytes::Bytes;
use prost::Message as _;
use prost_reflect::{DynamicMessage, Value};
use sha2::{Digest, Sha256};
use sinew_core::Result;
use uuid::Uuid;

use super::proto_dynamic::{message_desc, setf};
use super::proto_pool::agent_pool;
use super::state::PersistedAgentConversation;
use super::transcript::TranscriptTurn;

pub struct RunRequestInput<'a> {
    pub model_id: &'a str,
    pub system_prompt: &'a str,
    pub user_text: &'a str,
    pub conversation_id: &'a str,
    pub history_turns: &'a [TranscriptTurn],
    pub persisted: Option<PersistedAgentConversation>,
}

pub struct RunRequestOutput {
    pub request_bytes: Vec<u8>,
    pub blob_store: HashMap<String, Vec<u8>>,
}

pub fn build_run_request(input: &RunRequestInput<'_>) -> Result<RunRequestOutput> {
    let _pool = agent_pool()?;
    let mut blob_store = restore_blobs(&input.persisted);
    let conversation_state = load_or_build_state(input, &mut blob_store)?;
    let action = build_user_message_action(input.user_text)?;
    let model_details = build_model_details(input.model_id)?;
    let run_request = build_agent_run_request(
        conversation_state,
        action,
        model_details,
        input.conversation_id,
    )?;
    let client_msg = wrap_run_request(run_request)?;
    let request_bytes = client_msg.encode_to_vec();
    Ok(RunRequestOutput {
        request_bytes,
        blob_store,
    })
}

fn restore_blobs(persisted: &Option<PersistedAgentConversation>) -> HashMap<String, Vec<u8>> {
    let mut blob_store = HashMap::new();
    if let Some(state) = persisted {
        for (hex, b64) in &state.blobs {
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
                blob_store.insert(hex.clone(), bytes);
            }
        }
    }
    blob_store
}

fn load_or_build_state(
    input: &RunRequestInput<'_>,
    blob_store: &mut HashMap<String, Vec<u8>>,
) -> Result<DynamicMessage> {
    if let Some(state) = &input.persisted {
        if let Some(checkpoint) = &state.checkpoint_b64 {
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(checkpoint) {
                let desc = message_desc("agent.v1.ConversationStateStructure")?;
                if let Ok(msg) = DynamicMessage::decode(desc, bytes.as_slice()) {
                    return Ok(msg);
                }
            }
        }
    }
    build_conversation_state(input, blob_store)
}

fn build_conversation_state(
    input: &RunRequestInput<'_>,
    blob_store: &mut HashMap<String, Vec<u8>>,
) -> Result<DynamicMessage> {
    let desc = message_desc("agent.v1.ConversationStateStructure")?;
    let mut msg = DynamicMessage::new(desc);
    let root_ids = build_root_prompt_blob_ids(input.system_prompt, input.history_turns, blob_store)?;
    setf(
        &mut msg,
        "root_prompt_messages_json",
        Value::List(bytes_list(root_ids)),
    )?;
    let turn_ids = build_history_turn_blob_ids(input.history_turns, blob_store)?;
    setf(&mut msg, "turns", Value::List(bytes_list(turn_ids)))?;
    Ok(msg)
}

fn bytes_list(ids: Vec<Vec<u8>>) -> Vec<Value> {
    ids.into_iter()
        .map(|b| Value::Bytes(Bytes::from(b)))
        .collect()
}

fn build_root_prompt_blob_ids(
    system_prompt: &str,
    turns: &[TranscriptTurn],
    blob_store: &mut HashMap<String, Vec<u8>>,
) -> Result<Vec<Vec<u8>>> {
    let mut ids = Vec::new();
    let system_json = serde_json::json!({
        "role": "system",
        "content": system_prompt,
    });
    ids.push(store_blob(blob_store, system_json.to_string().as_bytes()));
    for turn in turns {
        if !turn.user_text.trim().is_empty() {
            let user_json = serde_json::json!({
                "role": "user",
                "content": [{ "type": "text", "text": turn.user_text }],
            });
            ids.push(store_blob(blob_store, user_json.to_string().as_bytes()));
        }
        if !turn.assistant_text.trim().is_empty() {
            let assistant_json = serde_json::json!({
                "role": "assistant",
                "content": [{ "type": "text", "text": turn.assistant_text }],
            });
            ids.push(store_blob(
                blob_store,
                assistant_json.to_string().as_bytes(),
            ));
        }
    }
    Ok(ids)
}

fn build_history_turn_blob_ids(
    turns: &[TranscriptTurn],
    blob_store: &mut HashMap<String, Vec<u8>>,
) -> Result<Vec<Vec<u8>>> {
    let mut turn_ids = Vec::new();
    for turn in turns {
        if turn.user_text.trim().is_empty() {
            continue;
        }
        let user_msg = build_user_message(&turn.user_text)?;
        let user_blob = store_blob(blob_store, &user_msg.encode_to_vec());
        let mut step_ids = Vec::new();
        if !turn.assistant_text.trim().is_empty() {
            let step = build_conversation_step_assistant(&turn.assistant_text)?;
            step_ids.push(store_blob(blob_store, &step.encode_to_vec()));
        }
        let agent_turn = build_agent_conversation_turn(user_blob, step_ids)?;
        let turn_structure = build_conversation_turn_structure(agent_turn)?;
        turn_ids.push(store_blob(
            blob_store,
            &turn_structure.encode_to_vec(),
        ));
    }
    Ok(turn_ids)
}

fn build_user_message_action(user_text: &str) -> Result<DynamicMessage> {
    let desc = message_desc("agent.v1.ConversationAction")?;
    let mut action = DynamicMessage::new(desc);
    let user_msg = build_user_message(user_text)?;
    let user_action_desc = message_desc("agent.v1.UserMessageAction")?;
    let mut user_action = DynamicMessage::new(user_action_desc);
    setf(&mut user_action, "user_message", Value::Message(user_msg))?;
    setf(
        &mut action,
        "user_message_action",
        Value::Message(user_action),
    )?;
    Ok(action)
}

fn build_user_message(text: &str) -> Result<DynamicMessage> {
    let desc = message_desc("agent.v1.UserMessage")?;
    let mut msg = DynamicMessage::new(desc);
    setf(&mut msg, "text", Value::String(text.to_string()))?;
    setf(
        &mut msg,
        "message_id",
        Value::String(Uuid::new_v4().to_string()),
    )?;
    Ok(msg)
}

fn build_conversation_step_assistant(text: &str) -> Result<DynamicMessage> {
    let desc = message_desc("agent.v1.ConversationStep")?;
    let mut step = DynamicMessage::new(desc);
    let assistant_desc = message_desc("agent.v1.AssistantMessage")?;
    let mut assistant = DynamicMessage::new(assistant_desc);
    setf(&mut assistant, "text", Value::String(text.to_string()))?;
    setf(&mut step, "assistant_message", Value::Message(assistant))?;
    Ok(step)
}

fn build_agent_conversation_turn(
    user_blob: Vec<u8>,
    step_blobs: Vec<Vec<u8>>,
) -> Result<DynamicMessage> {
    let desc = message_desc("agent.v1.AgentConversationTurnStructure")?;
    let mut msg = DynamicMessage::new(desc);
    setf(
        &mut msg,
        "user_message",
        Value::Bytes(Bytes::from(user_blob)),
    )?;
    setf(&mut msg, "steps", Value::List(bytes_list(step_blobs)))?;
    Ok(msg)
}

fn build_conversation_turn_structure(agent_turn: DynamicMessage) -> Result<DynamicMessage> {
    let desc = message_desc("agent.v1.ConversationTurnStructure")?;
    let mut msg = DynamicMessage::new(desc);
    setf(
        &mut msg,
        "agent_conversation_turn",
        Value::Message(agent_turn),
    )?;
    Ok(msg)
}

fn build_model_details(model_id: &str) -> Result<DynamicMessage> {
    let desc = message_desc("agent.v1.ModelDetails")?;
    let mut msg = DynamicMessage::new(desc);
    setf(&mut msg, "model_id", Value::String(model_id.to_string()))?;
    setf(
        &mut msg,
        "display_model_id",
        Value::String(model_id.to_string()),
    )?;
    setf(&mut msg, "display_name", Value::String(model_id.to_string()))?;
    Ok(msg)
}

fn build_agent_run_request(
    conversation_state: DynamicMessage,
    action: DynamicMessage,
    model_details: DynamicMessage,
    conversation_id: &str,
) -> Result<DynamicMessage> {
    let desc = message_desc("agent.v1.AgentRunRequest")?;
    let mut msg = DynamicMessage::new(desc);
    setf(
        &mut msg,
        "conversation_state",
        Value::Message(conversation_state),
    )?;
    setf(&mut msg, "action", Value::Message(action))?;
    setf(&mut msg, "model_details", Value::Message(model_details))?;
    setf(
        &mut msg,
        "conversation_id",
        Value::String(conversation_id.to_string()),
    )?;
    Ok(msg)
}

fn wrap_run_request(run_request: DynamicMessage) -> Result<DynamicMessage> {
    let desc = message_desc("agent.v1.AgentClientMessage")?;
    let mut msg = DynamicMessage::new(desc);
    setf(&mut msg, "run_request", Value::Message(run_request))?;
    Ok(msg)
}

fn store_blob(blob_store: &mut HashMap<String, Vec<u8>>, data: &[u8]) -> Vec<u8> {
    let id = Sha256::digest(data).to_vec();
    blob_store.insert(hex::encode(&id), data.to_vec());
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_minimal_run_request() {
        let input = RunRequestInput {
            model_id: "composer-2",
            system_prompt: "You are Composer.",
            user_text: "Hello",
            conversation_id: "test-conv-id",
            history_turns: &[],
            persisted: None,
        };
        let out = build_run_request(&input).expect("encode");
        assert!(!out.request_bytes.is_empty());
    }
}
