use std::sync::Arc;

use serde::Serialize;
use serde_json::{json, Map, Value};
use tokio::sync::mpsc;

use claakecode_core::{ChatMessage, ModelRef, Part, Provider, Usage};

use crate::tool_run::{FileChange, ToolRunImage};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    TurnStarted,
    TextStarted,
    TextChunk {
        delta: String,
    },
    TextFinished,
    ThinkingStarted,
    ThinkingChunk {
        delta: String,
    },
    ThinkingFinished,
    ToolStarted {
        id: String,
        name: String,
    },
    ToolArgsDelta {
        id: String,
        delta: String,
    },
    ToolOutputDelta {
        id: String,
        delta: String,
    },
    ToolReady {
        id: String,
        summary: String,
        args_pretty: String,
    },
    ToolFinished {
        id: String,
        output: String,
        is_error: bool,
        file_changes: Vec<FileChange>,
        images: Vec<ToolRunImage>,
        #[serde(skip_serializing_if = "Option::is_none")]
        meta: Option<Value>,
    },
    TokenUsage {
        provider: String,
        model: String,
        context_window: u32,
        preferred_window: u32,
        max_output_tokens: u32,
        usage: Usage,
    },
    Interrupted,
    Error {
        message: String,
    },
    PeerMessageReceived {
        id: String,
        from: String,
        to: String,
        message: String,
    },
    SubAgentEvent {
        id: String,
        agent_id: String,
        agent_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        team_name: Option<String>,
        model: ModelRef,
        #[serde(skip_serializing_if = "Option::is_none")]
        initial_message: Option<String>,
        event: Box<AgentEvent>,
    },
    AgentSlept,
    TurnFinished,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationEvent {
    pub workspace_id: String,
    pub conversation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u64>,
    pub event: AgentEvent,
}

#[derive(Debug, Clone)]
pub struct AgentEventScope {
    pub id: String,
    pub agent_id: String,
    pub agent_name: String,
    pub team_name: Option<String>,
    pub model: ModelRef,
    pub initial_message: String,
}

pub(super) fn send_event(
    event_tx: &mpsc::UnboundedSender<AgentEvent>,
    scope: Option<&AgentEventScope>,
    event: AgentEvent,
) {
    let event = match scope {
        Some(scope) => {
            let initial_message = if matches!(&event, AgentEvent::TurnStarted) {
                Some(scope.initial_message.clone())
            } else {
                None
            };
            AgentEvent::SubAgentEvent {
                id: scope.id.clone(),
                agent_id: scope.agent_id.clone(),
                agent_name: scope.agent_name.clone(),
                team_name: scope.team_name.clone(),
                model: scope.model.clone(),
                initial_message,
                event: Box::new(event),
            }
        }
        None => event,
    };
    let _ = event_tx.send(event);
}

pub(super) fn send_token_usage_event(
    event_tx: &mpsc::UnboundedSender<AgentEvent>,
    scope: Option<&AgentEventScope>,
    provider: &Arc<dyn Provider>,
    model: &ModelRef,
    usage: Usage,
) {
    if usage.total_tokens == 0 && usage.input_tokens == 0 && usage.output_tokens == 0 {
        return;
    }

    let Some(caps) = provider.capabilities(model) else {
        return;
    };

    send_event(
        event_tx,
        scope,
        AgentEvent::TokenUsage {
            provider: model.provider.clone(),
            model: model.name.clone(),
            context_window: caps.context_window,
            preferred_window: caps.preferred_window,
            max_output_tokens: caps.max_output_tokens,
            usage,
        },
    );
}

pub(super) fn attach_token_usage(
    message: &mut ChatMessage,
    provider: &str,
    model: &str,
    usage: Usage,
) {
    if usage.total_tokens == 0 && usage.input_tokens == 0 && usage.output_tokens == 0 {
        return;
    }

    let Some(first_part) = message.parts.first_mut() else {
        return;
    };

    let slot = part_meta_mut(first_part);
    let mut meta = match slot.take() {
        Some(Value::Object(map)) => map,
        Some(value) => {
            let mut map = Map::new();
            map.insert("previous_meta".into(), value);
            map
        }
        None => Map::new(),
    };

    meta.insert(
        "token_usage".into(),
        json!({
            "source": "stream",
            "provider": provider,
            "model": model,
            "input_tokens": usage.input_tokens,
            "output_tokens": usage.output_tokens,
            "total_tokens": usage.total_tokens,
            "reasoning_tokens": usage.reasoning_tokens,
            "cache_read_tokens": usage.cache_read_tokens,
            "cache_creation_tokens": usage.cache_creation_tokens,
        }),
    );
    *slot = Some(Value::Object(meta));
}

fn part_meta_mut(part: &mut Part) -> &mut Option<Value> {
    match part {
        Part::Text { meta, .. }
        | Part::Image { meta, .. }
        | Part::Thinking { meta, .. }
        | Part::ToolCall { meta, .. }
        | Part::ToolResult { meta, .. } => meta,
    }
}
