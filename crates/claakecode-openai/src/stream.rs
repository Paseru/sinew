use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};

use claakecode_core::{AppError, PartKind, StopReason, StreamEvent, ToolCallIntro, Usage};

pub(crate) struct EventParser {
    default_model: String,
    message_started: bool,
    stopped: bool,
    saw_tool_call: bool,
    open_parts: HashSet<usize>,
    thinking_text: HashMap<usize, String>,
    tool_args: HashMap<usize, String>,
    usage: Usage,
}

impl EventParser {
    pub(crate) fn new(default_model: String) -> Self {
        Self {
            default_model,
            message_started: false,
            stopped: false,
            saw_tool_call: false,
            open_parts: HashSet::new(),
            thinking_text: HashMap::new(),
            tool_args: HashMap::new(),
            usage: Usage::default(),
        }
    }

    pub(crate) fn push(&mut self, event: Value) -> Result<Vec<StreamEvent>, AppError> {
        let kind = event.get("type").and_then(Value::as_str).unwrap_or("");
        let mut out = Vec::new();

        match kind {
            "response.created" | "response.in_progress" => {
                self.ensure_message_start(&event, &mut out);
            }
            "response.output_item.added" => {
                self.ensure_message_start(&event, &mut out);
                let index = output_index(&event);
                if let Some(item) = event.get("item") {
                    self.start_output_item(index, item, &mut out);
                }
            }
            "response.content_part.added" => {
                self.ensure_message_start(&event, &mut out);
                let index = output_index(&event);
                let part_type = event
                    .get("part")
                    .and_then(|part| part.get("type"))
                    .and_then(Value::as_str);
                if matches!(part_type, Some("output_text" | "text")) {
                    self.start_text(index, &mut out);
                }
            }
            "response.output_text.delta" => {
                self.ensure_message_start(&event, &mut out);
                let index = output_index(&event);
                self.start_text(index, &mut out);
                if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                    out.push(StreamEvent::TextDelta {
                        index,
                        delta: delta.to_string(),
                    });
                }
            }
            "response.reasoning_summary_part.added" => {
                self.ensure_message_start(&event, &mut out);
                let index = output_index(&event);
                self.start_thinking(index, &mut out);
                if let Some(text) = event
                    .get("part")
                    .and_then(|part| part.get("text"))
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                {
                    self.thinking_text.entry(index).or_default().push_str(text);
                    out.push(StreamEvent::ThinkingDelta {
                        index,
                        delta: text.to_string(),
                    });
                }
            }
            "response.reasoning_summary_text.delta"
            | "response.reasoning_summary.delta"
            | "response.reasoning_text.delta" => {
                self.ensure_message_start(&event, &mut out);
                let index = output_index(&event);
                self.start_thinking(index, &mut out);
                if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                    self.thinking_text.entry(index).or_default().push_str(delta);
                    out.push(StreamEvent::ThinkingDelta {
                        index,
                        delta: delta.to_string(),
                    });
                }
            }
            "response.reasoning_summary_text.done"
            | "response.reasoning_summary.done"
            | "response.reasoning_text.done" => {
                self.ensure_message_start(&event, &mut out);
                let index = output_index(&event);
                self.start_thinking(index, &mut out);
                let text = event
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let buffer = self.thinking_text.entry(index).or_default();
                if buffer.is_empty() && !text.is_empty() {
                    buffer.push_str(text);
                    out.push(StreamEvent::ThinkingDelta {
                        index,
                        delta: text.to_string(),
                    });
                }
            }
            "response.reasoning_summary_part.done" => {
                self.ensure_message_start(&event, &mut out);
                let index = output_index(&event);
                self.start_thinking(index, &mut out);
                let text = event
                    .get("part")
                    .and_then(|part| part.get("text"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let buffer = self.thinking_text.entry(index).or_default();
                if buffer.is_empty() && !text.is_empty() {
                    buffer.push_str(text);
                    out.push(StreamEvent::ThinkingDelta {
                        index,
                        delta: text.to_string(),
                    });
                }
            }
            "response.function_call_arguments.delta" => {
                self.ensure_message_start(&event, &mut out);
                let index = output_index(&event);
                let delta = event
                    .get("delta")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                self.tool_args.entry(index).or_default().push_str(&delta);
                out.push(StreamEvent::ToolJsonDelta {
                    index,
                    chunk: delta,
                });
            }
            "response.function_call_arguments.done" => {
                self.ensure_message_start(&event, &mut out);
                let index = output_index(&event);
                let arguments = event
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if self
                    .tool_args
                    .get(&index)
                    .map(|value| value.is_empty())
                    .unwrap_or(true)
                    && !arguments.is_empty()
                {
                    self.tool_args.insert(index, arguments.to_string());
                    out.push(StreamEvent::ToolJsonDelta {
                        index,
                        chunk: arguments.to_string(),
                    });
                }
            }
            "response.output_item.done" => {
                self.ensure_message_start(&event, &mut out);
                let index = output_index(&event);
                if let Some(item) = event.get("item") {
                    self.finish_output_item(index, item, &mut out);
                }
            }
            "response.completed" => {
                self.ensure_message_start(&event, &mut out);
                self.capture_usage(&event);
                out.push(self.message_stop(StopReason::EndTurn));
            }
            "response.incomplete" => {
                self.ensure_message_start(&event, &mut out);
                self.capture_usage(&event);
                let reason = event
                    .get("response")
                    .and_then(|response| response.get("incomplete_details"))
                    .and_then(|details| details.get("reason"))
                    .and_then(Value::as_str)
                    .map(map_incomplete_reason)
                    .unwrap_or(StopReason::Other);
                out.push(self.message_stop(reason));
            }
            "response.failed" => {
                return Err(event_error(&event));
            }
            "error" | "response.error" => {
                return Err(event_error(&event));
            }
            _ => {}
        }

        Ok(out)
    }

    fn ensure_message_start(&mut self, event: &Value, out: &mut Vec<StreamEvent>) {
        if self.message_started {
            return;
        }
        self.message_started = true;
        let model = event
            .get("response")
            .and_then(|response| response.get("model"))
            .and_then(Value::as_str)
            .unwrap_or(&self.default_model)
            .to_string();
        out.push(StreamEvent::MessageStart { model });
    }

    fn start_output_item(&mut self, index: usize, item: &Value, out: &mut Vec<StreamEvent>) {
        match item.get("type").and_then(Value::as_str) {
            Some("function_call") => {
                self.saw_tool_call = true;
                let id = item
                    .get("call_id")
                    .or_else(|| item.get("id"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let name = item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                self.open_parts.insert(index);
                out.push(StreamEvent::PartStart {
                    index,
                    kind: PartKind::ToolCall,
                    tool: Some(ToolCallIntro { id, name }),
                });
            }
            Some("reasoning") => {
                let summary = reasoning_item_text(item);
                self.start_thinking(index, out);
                if !summary.is_empty() {
                    self.thinking_text
                        .entry(index)
                        .or_default()
                        .push_str(&summary);
                    out.push(StreamEvent::ThinkingDelta {
                        index,
                        delta: summary,
                    });
                }
            }
            Some("message") => {
                if message_text(item).is_some() {
                    self.start_text(index, out);
                }
            }
            _ => {}
        }
    }

    fn finish_output_item(&mut self, index: usize, item: &Value, out: &mut Vec<StreamEvent>) {
        match item.get("type").and_then(Value::as_str) {
            Some("function_call") => {
                self.saw_tool_call = true;
                if !self.open_parts.contains(&index) {
                    self.start_output_item(index, item, out);
                }
                let arguments = item
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if self
                    .tool_args
                    .get(&index)
                    .map(|value| value.is_empty())
                    .unwrap_or(true)
                    && !arguments.is_empty()
                {
                    out.push(StreamEvent::ToolJsonDelta {
                        index,
                        chunk: arguments.to_string(),
                    });
                }
                out.push(StreamEvent::PartMeta {
                    index,
                    meta: json!({ "provider": "openai", "item": item }),
                });
                out.push(StreamEvent::PartStop { index });
                self.open_parts.remove(&index);
            }
            Some("message") => {
                if !self.open_parts.contains(&index) {
                    if let Some(text) = message_text(item) {
                        self.start_text(index, out);
                        if !text.is_empty() {
                            out.push(StreamEvent::TextDelta { index, delta: text });
                        }
                    }
                }
                if self.open_parts.remove(&index) {
                    out.push(StreamEvent::PartStop { index });
                }
            }
            Some("reasoning") => {
                self.start_thinking(index, out);
                let final_text = reasoning_item_text(item);
                let already = self.thinking_text.get(&index).cloned().unwrap_or_default();
                let missing = if final_text.starts_with(&already) {
                    final_text[already.len()..].to_string()
                } else if already.is_empty() {
                    final_text
                } else {
                    String::new()
                };
                if !missing.is_empty() {
                    self.thinking_text
                        .entry(index)
                        .or_default()
                        .push_str(&missing);
                    out.push(StreamEvent::ThinkingDelta {
                        index,
                        delta: missing,
                    });
                }
                out.push(StreamEvent::PartMeta {
                    index,
                    meta: json!({ "provider": "openai", "item": item }),
                });
                out.push(StreamEvent::PartStop { index });
                self.open_parts.remove(&index);
            }
            _ => {}
        }
    }

    fn start_text(&mut self, index: usize, out: &mut Vec<StreamEvent>) {
        if self.open_parts.insert(index) {
            out.push(StreamEvent::PartStart {
                index,
                kind: PartKind::Text,
                tool: None,
            });
        }
    }

    fn start_thinking(&mut self, index: usize, out: &mut Vec<StreamEvent>) {
        if self.open_parts.insert(index) {
            out.push(StreamEvent::PartStart {
                index,
                kind: PartKind::Thinking,
                tool: None,
            });
        }
    }

    fn capture_usage(&mut self, event: &Value) {
        let Some(usage) = event
            .get("response")
            .and_then(|response| response.get("usage"))
        else {
            return;
        };
        self.usage.input_tokens = usage_u32(usage, "input_tokens");
        self.usage.output_tokens = usage_u32(usage, "output_tokens");
        self.usage.total_tokens = usage_u32(usage, "total_tokens");
        self.usage.reasoning_tokens = usage
            .get("output_tokens_details")
            .and_then(|details| details.get("reasoning_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32;
        self.usage.cache_read_tokens = usage
            .get("input_tokens_details")
            .and_then(|details| details.get("cached_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32;
    }

    fn message_stop(&mut self, reason: StopReason) -> StreamEvent {
        self.stopped = true;
        StreamEvent::MessageStop {
            stop_reason: if self.saw_tool_call {
                StopReason::ToolUse
            } else {
                reason
            },
            usage: self.usage,
        }
    }
}

fn output_index(event: &Value) -> usize {
    event
        .get("output_index")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize
}

fn usage_u32(usage: &Value, key: &str) -> u32 {
    usage.get(key).and_then(Value::as_u64).unwrap_or(0) as u32
}

fn map_incomplete_reason(raw: &str) -> StopReason {
    match raw {
        "max_output_tokens" => StopReason::MaxTokens,
        "content_filter" => StopReason::Other,
        _ => StopReason::Other,
    }
}

fn event_error_message(event: &Value) -> String {
    let message = event
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .or_else(|| event.get("message").and_then(Value::as_str));
    let code = event
        .get("error")
        .and_then(|error| error.get("code"))
        .and_then(Value::as_str);

    match (code, message) {
        (Some(code), Some(message)) if !code.is_empty() && !message.is_empty() => {
            format!("{code}: {message}")
        }
        (_, Some(message)) if !message.is_empty() => message.to_string(),
        (Some(code), _) if !code.is_empty() => code.to_string(),
        _ => "openai stream error".to_string(),
    }
}

fn event_error(event: &Value) -> AppError {
    let message = event_error_message(event);
    if is_context_length_message(&message) {
        AppError::ContextLength(message)
    } else {
        AppError::Provider(message)
    }
}

fn is_context_length_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("context window")
        || lower.contains("context length")
        || lower.contains("context_length")
        || lower.contains("maximum context")
        || lower.contains("input exceeds")
        || lower.contains("input is too long")
        || lower.contains("too many tokens")
}

fn message_text(item: &Value) -> Option<String> {
    let content = item.get("content")?.as_array()?;
    let text = content
        .iter()
        .filter_map(|part| {
            let kind = part.get("type").and_then(Value::as_str)?;
            if matches!(kind, "output_text" | "text") {
                part.get("text").and_then(Value::as_str)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("");
    Some(text)
}

fn reasoning_summary_text(item: &Value) -> String {
    item.get("summary")
        .and_then(Value::as_array)
        .map(|summary| {
            summary
                .iter()
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n\n")
        })
        .unwrap_or_default()
}

fn reasoning_content_text(item: &Value) -> String {
    item.get("content")
        .and_then(Value::as_array)
        .map(|content| {
            content
                .iter()
                .filter(|part| {
                    part.get("type")
                        .and_then(Value::as_str)
                        .map(|kind| kind == "reasoning_text")
                        .unwrap_or(true)
                })
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n\n")
        })
        .unwrap_or_default()
}

fn reasoning_item_text(item: &Value) -> String {
    let summary = reasoning_summary_text(item);
    if !summary.is_empty() {
        return summary;
    }
    reasoning_content_text(item)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_failed_context_error_maps_to_context_length() {
        let mut parser = EventParser::new("gpt-5.5".to_string());
        let err = parser
            .push(json!({
                "type": "response.failed",
                "error": {
                    "code": "context_length_exceeded",
                    "message": "Your input exceeds the context window of this model. Please adjust your input and try again."
                }
            }))
            .expect_err("context failure should be an error");

        assert!(matches!(err, AppError::ContextLength(_)));
    }
}
