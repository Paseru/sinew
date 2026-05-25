use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    error::AppError,
    message::{StopReason, Usage},
};

pub type ProviderStream = BoxStream<'static, Result<StreamEvent, AppError>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartKind {
    Text,
    Thinking,
    ToolCall,
}

#[derive(Debug, Clone)]
pub struct ToolCallIntro {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub enum StreamEvent {
    MessageStart {
        model: String,
    },
    PartStart {
        index: usize,
        kind: PartKind,
        tool: Option<ToolCallIntro>,
    },
    TextDelta {
        index: usize,
        delta: String,
    },
    ThinkingDelta {
        index: usize,
        delta: String,
    },
    ToolJsonDelta {
        index: usize,
        chunk: String,
    },
    PartMeta {
        index: usize,
        meta: Value,
    },
    PartStop {
        index: usize,
    },
    Usage {
        usage: Usage,
    },
    MessageStop {
        stop_reason: StopReason,
        usage: Usage,
    },
}
