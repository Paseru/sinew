use std::collections::HashMap;

use eventsource_stream::Eventsource;
use futures::{stream::Stream, StreamExt};
use serde_json::{json, Value};

use claakecode_core::{
    AppError, PartKind, ProviderStream, StopReason, StreamEvent, ToolCallIntro, Usage,
};

use crate::wire::{self, ChatChunk};

pub fn map_stream<S, E>(body: S, model: String) -> ProviderStream
where
    S: Stream<Item = std::result::Result<bytes::Bytes, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    let source = Box::pin(body.eventsource());
    let parser = EventParser::new(model);

    futures::stream::unfold(
        (source, parser, Vec::<StreamEvent>::new(), false, false),
        |(mut source, mut parser, mut pending, done, mut saw_any_event)| async move {
            loop {
                if let Some(next) = pending.pop() {
                    return Some((Ok(next), (source, parser, pending, done, saw_any_event)));
                }
                if done {
                    return None;
                }

                match source.next().await {
                    Some(Ok(event)) => {
                        saw_any_event = true;
                        let data = event.data.trim();
                        if data == "[DONE]" {
                            let mut produced = parser.finish();
                            produced.reverse();
                            pending = produced;
                            if pending.is_empty() {
                                return None;
                            }
                            continue;
                        }

                        if let Ok(value) = serde_json::from_str::<Value>(data) {
                            if let Some(error) = value.get("error") {
                                let message = error
                                    .get("message")
                                    .and_then(Value::as_str)
                                    .unwrap_or("kimi stream error");
                                return Some((
                                    Err(AppError::Provider(message.to_string())),
                                    (source, parser, pending, true, saw_any_event),
                                ));
                            }
                        }

                        let parsed: std::result::Result<ChatChunk, _> = serde_json::from_str(data);
                        match parsed {
                            Ok(parsed) => {
                                let mut produced = parser.push(parsed);
                                produced.reverse();
                                pending = produced;
                            }
                            Err(err) => {
                                return Some((
                                    Err(AppError::Decode(format!("bad kimi event: {err}"))),
                                    (source, parser, pending, true, saw_any_event),
                                ));
                            }
                        }
                    }
                    Some(Err(err)) => {
                        return Some((
                            Err(AppError::Stream(err.to_string())),
                            (source, parser, pending, true, saw_any_event),
                        ));
                    }
                    None => {
                        if !saw_any_event {
                            return Some((
                                Err(AppError::Stream(
                                    "kimi SSE closed before any event; \
                                     the server likely dropped the connection"
                                        .into(),
                                )),
                                (source, parser, pending, true, saw_any_event),
                            ));
                        }
                        let mut produced = parser.finish();
                        produced.reverse();
                        pending = produced;
                        if pending.is_empty() {
                            return None;
                        }
                    }
                }
            }
        },
    )
    .boxed()
}

#[derive(Debug, Default)]
struct ToolState {
    part_index: Option<usize>,
    id: String,
    name: String,
    pending_args: String,
}

struct EventParser {
    model: String,
    started: bool,
    next_index: usize,
    open_part: Option<(usize, PartKind)>,
    tool_states: HashMap<usize, ToolState>,
    saw_tool_call: bool,
    stop_reason: Option<StopReason>,
    usage: Usage,
    done: bool,
}

impl EventParser {
    fn new(model: String) -> Self {
        Self {
            model,
            started: false,
            next_index: 0,
            open_part: None,
            tool_states: HashMap::new(),
            saw_tool_call: false,
            stop_reason: None,
            usage: Usage::default(),
            done: false,
        }
    }

    fn push(&mut self, chunk: ChatChunk) -> Vec<StreamEvent> {
        if self.done {
            return Vec::new();
        }
        if let Some(model) = chunk.model.filter(|value| !value.trim().is_empty()) {
            self.model = model;
        }
        if let Some(usage) = chunk.usage {
            self.usage = usage_from_body(usage);
        }

        let mut out = Vec::new();
        for choice in chunk.choices {
            if let Some(usage) = choice.usage {
                self.usage = usage_from_body(usage);
            }
            if let Some(reasoning) = choice
                .delta
                .reasoning_content
                .filter(|value| !value.is_empty())
            {
                self.ensure_started(&mut out);
                let index = self.ensure_open(PartKind::Thinking, &mut out);
                out.push(StreamEvent::ThinkingDelta {
                    index,
                    delta: reasoning,
                });
            }
            if let Some(text) = choice.delta.content.filter(|value| !value.is_empty()) {
                self.ensure_started(&mut out);
                let index = self.ensure_open(PartKind::Text, &mut out);
                out.push(StreamEvent::TextDelta { index, delta: text });
            }
            for call in choice.delta.tool_calls {
                self.push_tool_delta(call, &mut out);
            }
            if let Some(reason) = choice.finish_reason {
                self.stop_reason = Some(map_stop_reason(&reason, self.saw_tool_call));
                out.extend(self.finish());
            }
        }

        out
    }

    fn push_tool_delta(&mut self, call: wire::ToolCallDelta, out: &mut Vec<StreamEvent>) {
        self.ensure_started(out);
        self.saw_tool_call = true;
        let key = call.index.unwrap_or(self.tool_states.len());
        let mut state = self.tool_states.remove(&key).unwrap_or_default();
        if let Some(id) = call.id.filter(|value| !value.trim().is_empty()) {
            state.id = id;
        }
        let mut new_args = String::new();
        if let Some(function) = call.function {
            if let Some(name) = function.name.filter(|value| !value.trim().is_empty()) {
                state.name = name;
            }
            if let Some(arguments) = function.arguments.filter(|value| !value.is_empty()) {
                new_args = arguments;
            }
        }

        if state.part_index.is_none() && !state.name.is_empty() {
            self.close_open(out);
            let part_index = self.next_index();
            let id = if state.id.is_empty() {
                format!("call_kimi_{key}")
            } else {
                state.id.clone()
            };
            state.id = id.clone();
            state.part_index = Some(part_index);
            out.push(StreamEvent::PartStart {
                index: part_index,
                kind: PartKind::ToolCall,
                tool: Some(ToolCallIntro {
                    id,
                    name: state.name.clone(),
                }),
            });
            if !state.pending_args.is_empty() {
                out.push(StreamEvent::ToolJsonDelta {
                    index: part_index,
                    chunk: std::mem::take(&mut state.pending_args),
                });
            }
        }

        if let Some(part_index) = state.part_index {
            if !new_args.is_empty() {
                out.push(StreamEvent::ToolJsonDelta {
                    index: part_index,
                    chunk: new_args,
                });
            }
        } else if !new_args.is_empty() {
            state.pending_args.push_str(&new_args);
        }

        self.tool_states.insert(key, state);
    }

    fn ensure_started(&mut self, out: &mut Vec<StreamEvent>) {
        if self.started {
            return;
        }
        self.started = true;
        out.push(StreamEvent::MessageStart {
            model: self.model.clone(),
        });
    }

    fn ensure_open(&mut self, kind: PartKind, out: &mut Vec<StreamEvent>) -> usize {
        if self.open_part.map(|(_, current)| current) == Some(kind) {
            return self.open_part.map(|(index, _)| index).unwrap_or(0);
        }
        self.close_open(out);
        let index = self.next_index();
        self.open_part = Some((index, kind));
        out.push(StreamEvent::PartStart {
            index,
            kind,
            tool: None,
        });
        index
    }

    fn close_open(&mut self, out: &mut Vec<StreamEvent>) {
        if let Some((index, _)) = self.open_part.take() {
            out.push(StreamEvent::PartStop { index });
        }
    }

    fn next_index(&mut self) -> usize {
        let index = self.next_index;
        self.next_index += 1;
        index
    }

    fn finish(&mut self) -> Vec<StreamEvent> {
        if self.done {
            return Vec::new();
        }
        self.done = true;
        let mut out = Vec::new();
        if !self.started {
            self.ensure_started(&mut out);
        }
        self.close_open(&mut out);
        let mut keys = self.tool_states.keys().copied().collect::<Vec<_>>();
        keys.sort_unstable();
        for key in keys {
            if let Some(state) = self.tool_states.remove(&key) {
                if let Some(index) = state.part_index {
                    out.push(StreamEvent::PartMeta {
                        index,
                        meta: json!({ "provider": "kimi", "id": state.id, "name": state.name }),
                    });
                    out.push(StreamEvent::PartStop { index });
                }
            }
        }
        out.push(StreamEvent::MessageStop {
            stop_reason: self.stop_reason.unwrap_or({
                if self.saw_tool_call {
                    StopReason::ToolUse
                } else {
                    StopReason::EndTurn
                }
            }),
            usage: self.usage,
        });
        out
    }
}

fn usage_from_body(body: wire::UsageBody) -> Usage {
    let cache_read_tokens = body
        .cached_tokens
        .or_else(|| {
            body.prompt_tokens_details
                .and_then(|details| details.cached_tokens)
        })
        .unwrap_or(0);
    let reasoning_tokens = body
        .completion_tokens_details
        .and_then(|details| details.reasoning_tokens)
        .unwrap_or(0);
    Usage {
        input_tokens: body.prompt_tokens,
        output_tokens: body.completion_tokens,
        total_tokens: if body.total_tokens > 0 {
            body.total_tokens
        } else {
            body.prompt_tokens.saturating_add(body.completion_tokens)
        },
        reasoning_tokens,
        cache_read_tokens,
        cache_creation_tokens: 0,
    }
}

fn map_stop_reason(raw: &str, saw_tool_call: bool) -> StopReason {
    match raw {
        "stop" => StopReason::EndTurn,
        "tool_calls" | "function_call" => StopReason::ToolUse,
        "length" => StopReason::MaxTokens,
        "content_filter" => StopReason::Other,
        _ if saw_tool_call => StopReason::ToolUse,
        _ => StopReason::Other,
    }
}
