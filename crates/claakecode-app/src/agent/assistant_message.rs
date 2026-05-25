use std::time::Instant;

use serde_json::{json, Map, Value};

use claakecode_core::{ChatMessage, Part, PartKind, Role};

use super::history::normalize_tool_call_input;

#[derive(Default)]
pub(super) struct AssistantMessageBuilder {
    order: Vec<(usize, PartKind)>,
    text_parts: std::collections::HashMap<usize, String>,
    tool_json_parts: std::collections::HashMap<usize, String>,
    tool_heads: std::collections::HashMap<usize, (String, String)>,
    meta: std::collections::HashMap<usize, Value>,
    thinking_started: std::collections::HashMap<usize, Instant>,
}

impl AssistantMessageBuilder {
    pub(super) fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub(super) fn open(&mut self, index: usize, kind: PartKind) {
        self.order.push((index, kind));
        if matches!(kind, PartKind::Thinking) {
            self.thinking_started.insert(index, Instant::now());
        }
    }

    pub(super) fn thinking_duration_ms(&self, index: usize) -> Option<u64> {
        self.thinking_started
            .get(&index)
            .map(|start| start.elapsed().as_millis() as u64)
    }

    pub(super) fn kind(&self, index: usize) -> Option<PartKind> {
        self.order
            .iter()
            .find(|(candidate, _)| *candidate == index)
            .map(|(_, kind)| *kind)
    }

    pub(super) fn register_tool(&mut self, index: usize, id: String, name: String) {
        self.tool_heads.insert(index, (id, name));
    }

    pub(super) fn tool_head(&self, index: usize) -> Option<(String, String)> {
        self.tool_heads.get(&index).cloned()
    }

    pub(super) fn push_text(&mut self, index: usize, chunk: &str) {
        self.text_parts.entry(index).or_default().push_str(chunk);
    }

    pub(super) fn push_tool_json(&mut self, index: usize, chunk: &str) {
        self.tool_json_parts
            .entry(index)
            .or_default()
            .push_str(chunk);
    }

    pub(super) fn push_meta(&mut self, index: usize, meta: Value) {
        self.meta.insert(index, meta);
    }

    pub(super) fn insert_meta_field(&mut self, index: usize, key: &str, value: Value) {
        let current = self.meta.remove(&index);
        let mut meta = match current {
            Some(Value::Object(map)) => map,
            Some(value) => {
                let mut map = Map::new();
                map.insert("previous_meta".into(), value);
                map
            }
            None => Map::new(),
        };
        meta.insert(key.to_string(), value);
        self.meta.insert(index, Value::Object(meta));
    }

    pub(super) fn finalize_tool(&self, index: usize) -> (String, String, Value) {
        let (id, name) = self.tool_heads.get(&index).cloned().unwrap_or_default();
        let raw = self
            .tool_json_parts
            .get(&index)
            .cloned()
            .unwrap_or_default();
        let value =
            normalize_tool_call_input(serde_json::from_str(&raw).unwrap_or(Value::String(raw)));
        (id, name, value)
    }

    pub(super) fn finish(mut self) -> ChatMessage {
        let pending_thinking: Vec<(usize, u64)> = self
            .order
            .iter()
            .filter_map(|(index, kind)| {
                if !matches!(kind, PartKind::Thinking) {
                    return None;
                }
                let has_duration = matches!(
                    self.meta.get(index),
                    Some(Value::Object(map)) if map.contains_key("duration_ms")
                );
                if has_duration {
                    return None;
                }
                self.thinking_duration_ms(*index).map(|ms| (*index, ms))
            })
            .collect();
        for (index, ms) in pending_thinking {
            self.insert_meta_field(index, "duration_ms", json!(ms));
        }

        let mut parts = Vec::with_capacity(self.order.len());
        let order = self.order.clone();
        for (index, kind) in order {
            let meta = self.meta.get(&index).cloned();
            match kind {
                PartKind::Text => parts.push(Part::Text {
                    text: self.text_parts.get(&index).cloned().unwrap_or_default(),
                    meta,
                }),
                PartKind::Thinking => parts.push(Part::Thinking {
                    text: self.text_parts.get(&index).cloned().unwrap_or_default(),
                    meta,
                }),
                PartKind::ToolCall => {
                    let (id, name, input) = self.finalize_tool(index);
                    parts.push(Part::ToolCall {
                        id,
                        name,
                        input,
                        meta,
                    });
                }
            }
        }

        ChatMessage {
            role: Role::Assistant,
            parts,
        }
    }
}
