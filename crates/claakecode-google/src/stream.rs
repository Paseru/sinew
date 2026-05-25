use eventsource_stream::Eventsource;
use futures::{stream::Stream, StreamExt};
use serde_json::{json, Value};

use claakecode_core::{
    AppError, PartKind, ProviderStream, StopReason, StreamEvent, ToolCallIntro, Usage,
};

use crate::wire::{self, CodeAssistResponse};

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
                        if event.data.trim() == "[DONE]" {
                            let mut produced = parser.finish();
                            produced.reverse();
                            pending = produced;
                            continue;
                        }

                        if let Ok(value) = serde_json::from_str::<Value>(&event.data) {
                            if let Some(error) = value.get("error") {
                                let message = error
                                    .get("message")
                                    .and_then(Value::as_str)
                                    .unwrap_or("google stream error");
                                return Some((
                                    Err(AppError::Provider(message.to_string())),
                                    (source, parser, pending, true, saw_any_event),
                                ));
                            }
                        }

                        let parsed: std::result::Result<CodeAssistResponse, _> =
                            serde_json::from_str(&event.data);
                        match parsed {
                            Ok(parsed) => {
                                let mut produced = parser.push(parsed);
                                produced.reverse();
                                pending = produced;
                            }
                            Err(err) => {
                                return Some((
                                    Err(AppError::Decode(format!("bad google event: {err}"))),
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
                                    "google SSE closed before any event; \
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

struct EventParser {
    model: String,
    started: bool,
    next_index: usize,
    open_part: Option<(usize, PartKind)>,
    stop_reason: Option<StopReason>,
    usage: Usage,
    saw_tool_call: bool,
    call_counter: usize,
    done: bool,
}

impl EventParser {
    fn new(model: String) -> Self {
        Self {
            model,
            started: false,
            next_index: 0,
            open_part: None,
            stop_reason: None,
            usage: Usage::default(),
            saw_tool_call: false,
            call_counter: 0,
            done: false,
        }
    }

    fn push(&mut self, event: CodeAssistResponse) -> Vec<StreamEvent> {
        if self.done {
            return Vec::new();
        }

        let mut out = Vec::new();
        self.ensure_started(&mut out);

        if let Some(response) = event.response {
            if let Some(usage) = response.usage_metadata {
                self.usage = usage_from_metadata(usage);
            }
            if let Some(model_version) = response.model_version {
                if !model_version.trim().is_empty() {
                    self.model = model_version;
                }
            }

            for candidate in response.candidates {
                if let Some(content) = candidate.content {
                    for part in content.parts {
                        self.push_part(part, &mut out);
                    }
                }
                if let Some(reason) = candidate.finish_reason {
                    self.stop_reason = Some(map_stop_reason(&reason));
                }
            }
        }

        if let Some(trace_id) = event.trace_id {
            if let Some((index, _)) = self.open_part {
                out.push(StreamEvent::PartMeta {
                    index,
                    meta: json!({ "provider": "google", "trace_id": trace_id }),
                });
            }
        }

        if self.stop_reason.is_some() {
            out.extend(self.finish());
        }

        out
    }

    fn push_part(&mut self, part: wire::ResponsePart, out: &mut Vec<StreamEvent>) {
        if let Some(call) = part.function_call {
            self.close_open(out);
            let name = call
                .name
                .map(|name| name.trim().to_string())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "generic_tool".into());
            let raw_id = call.id.unwrap_or_else(|| {
                self.call_counter += 1;
                format!("call_google_{}", self.call_counter)
            });
            let id = prefixed_tool_id(&name, &raw_id);
            let index = self.next_index();
            self.saw_tool_call = true;
            out.push(StreamEvent::PartStart {
                index,
                kind: PartKind::ToolCall,
                tool: Some(ToolCallIntro {
                    id: id.clone(),
                    name: name.clone(),
                }),
            });
            out.push(StreamEvent::ToolJsonDelta {
                index,
                chunk: call.args.unwrap_or_else(|| json!({})).to_string(),
            });
            // Gemini 3 models attach a `thoughtSignature` to functionCall parts.
            // Propagate it so it can be replayed on follow-up turns; the server
            // rejects requests that drop it ("Function call is missing a
            // thought_signature in functionCall parts").
            let mut meta = json!({ "provider": "google", "raw_id": raw_id });
            if let Some(signature) = part.thought_signature.as_ref() {
                if !signature.trim().is_empty() {
                    meta["signature"] = json!(signature);
                }
            }
            out.push(StreamEvent::PartMeta { index, meta });
            out.push(StreamEvent::PartStop { index });
            return;
        }

        let thought = part.thought;
        let text = part.text.unwrap_or_else(|| match &thought {
            Some(Value::String(value)) => value.clone(),
            _ => String::new(),
        });
        if text.is_empty() {
            return;
        }

        let is_thought = matches!(thought, Some(Value::Bool(true) | Value::String(_)));
        if is_thought {
            let index = self.ensure_open(PartKind::Thinking, out);
            out.push(StreamEvent::ThinkingDelta { index, delta: text });
            if let Some(signature) = part.thought_signature {
                out.push(StreamEvent::PartMeta {
                    index,
                    meta: json!({ "provider": "google", "signature": signature }),
                });
            }
        } else {
            let index = self.ensure_open(PartKind::Text, out);
            out.push(StreamEvent::TextDelta { index, delta: text });
        }
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
        let mut out = Vec::new();
        self.ensure_started(&mut out);
        self.close_open(&mut out);
        let mut stop_reason = self.stop_reason.unwrap_or(StopReason::EndTurn);
        if self.saw_tool_call {
            stop_reason = StopReason::ToolUse;
        }
        out.push(StreamEvent::MessageStop {
            stop_reason,
            usage: self.usage,
        });
        self.done = true;
        out
    }
}

fn prefixed_tool_id(name: &str, raw_id: &str) -> String {
    let prefix = format!("{name}__");
    if raw_id.starts_with(&prefix) {
        raw_id.to_string()
    } else {
        format!("{prefix}{raw_id}")
    }
}

fn usage_from_metadata(usage: wire::UsageMetadata) -> Usage {
    let total = if usage.total_token_count > 0 {
        usage.total_token_count
    } else {
        usage
            .prompt_token_count
            .saturating_add(usage.candidates_token_count)
            .saturating_add(usage.thoughts_token_count)
    };
    Usage {
        input_tokens: usage.prompt_token_count,
        output_tokens: usage.candidates_token_count,
        total_tokens: total,
        reasoning_tokens: usage.thoughts_token_count,
        cache_read_tokens: 0,
        cache_creation_tokens: 0,
    }
}

fn map_stop_reason(raw: &str) -> StopReason {
    match raw {
        "STOP" => StopReason::EndTurn,
        "MAX_TOKENS" => StopReason::MaxTokens,
        "MALFORMED_FUNCTION_CALL" | "UNEXPECTED_TOOL_CALL" => StopReason::ToolUse,
        "RECITATION" | "SAFETY" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "SPII" => StopReason::Other,
        _ => StopReason::Other,
    }
}
