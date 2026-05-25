use eventsource_stream::Eventsource;
use futures::{stream::Stream, StreamExt};
use serde_json::json;

use claakecode_core::{
    AppError, PartKind, ProviderStream, StopReason, StreamEvent, ToolCallIntro, Usage,
};

use crate::wire::{self, SseEvent};

const SSE_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

pub fn map_stream<S, E>(body: S) -> ProviderStream
where
    S: Stream<Item = std::result::Result<bytes::Bytes, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    let source = Box::pin(body.eventsource());
    let parser = EventParser::default();

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

                match tokio::time::timeout(SSE_IDLE_TIMEOUT, source.next()).await {
                    Ok(Some(Ok(event))) => {
                        saw_any_event = true;
                        let parsed: std::result::Result<SseEvent, _> =
                            serde_json::from_str(&event.data);
                        match parsed {
                            Ok(SseEvent::Error { error }) => {
                                return Some((
                                    Err(event_error(error)),
                                    (source, parser, pending, true, saw_any_event),
                                ));
                            }
                            Ok(parsed) => {
                                let mut produced = parser.push(parsed);
                                produced.reverse();
                                pending = produced;
                            }
                            Err(err) => {
                                return Some((
                                    Err(AppError::Decode(format!("bad anthropic event: {err}"))),
                                    (source, parser, pending, true, saw_any_event),
                                ));
                            }
                        }
                    }
                    Ok(Some(Err(err))) => {
                        return Some((
                            Err(AppError::Stream(err.to_string())),
                            (source, parser, pending, true, saw_any_event),
                        ));
                    }
                    Ok(None) => {
                        if !saw_any_event {
                            return Some((
                                Err(AppError::Stream(
                                    "anthropic SSE closed before any event; \
                                     the server likely dropped the connection"
                                        .into(),
                                )),
                                (source, parser, pending, true, saw_any_event),
                            ));
                        }
                        return None;
                    }
                    Err(_) => {
                        return Some((
                            Err(AppError::Stream(
                                "idle timeout waiting for Anthropic SSE".into(),
                            )),
                            (source, parser, pending, true, saw_any_event),
                        ));
                    }
                }
            }
        },
    )
    .boxed()
}

#[derive(Default)]
struct EventParser {
    usage: wire::UsageAccumulator,
    stop_reason: Option<StopReason>,
}

impl EventParser {
    fn push(&mut self, event: SseEvent) -> Vec<StreamEvent> {
        match event {
            SseEvent::MessageStart { message } => {
                if let Some(usage) = message.usage {
                    usage.merge_into(&mut self.usage);
                }
                vec![StreamEvent::MessageStart {
                    model: message.model,
                }]
            }
            SseEvent::ContentBlockStart {
                index,
                content_block,
            } => match content_block {
                wire::ContentBlockStart::Text { .. } => vec![StreamEvent::PartStart {
                    index,
                    kind: PartKind::Text,
                    tool: None,
                }],
                wire::ContentBlockStart::Thinking { thinking } => {
                    let mut out = vec![StreamEvent::PartStart {
                        index,
                        kind: PartKind::Thinking,
                        tool: None,
                    }];
                    if !thinking.is_empty() {
                        out.push(StreamEvent::ThinkingDelta {
                            index,
                            delta: thinking,
                        });
                    }
                    out
                }
                wire::ContentBlockStart::ToolUse { id, name, .. } => vec![StreamEvent::PartStart {
                    index,
                    kind: PartKind::ToolCall,
                    tool: Some(ToolCallIntro { id, name }),
                }],
                wire::ContentBlockStart::Unknown => Vec::new(),
            },
            SseEvent::ContentBlockDelta { index, delta } => match delta {
                wire::ContentDelta::TextDelta { text } => {
                    vec![StreamEvent::TextDelta { index, delta: text }]
                }
                wire::ContentDelta::ThinkingDelta { thinking } => {
                    vec![StreamEvent::ThinkingDelta {
                        index,
                        delta: thinking,
                    }]
                }
                wire::ContentDelta::SignatureDelta { signature } => vec![StreamEvent::PartMeta {
                    index,
                    meta: json!({ "provider": "anthropic", "signature": signature }),
                }],
                wire::ContentDelta::InputJsonDelta { partial_json } => {
                    vec![StreamEvent::ToolJsonDelta {
                        index,
                        chunk: partial_json,
                    }]
                }
                wire::ContentDelta::Unknown => Vec::new(),
            },
            SseEvent::ContentBlockStop { index } => vec![StreamEvent::PartStop { index }],
            SseEvent::MessageDelta { delta, usage } => {
                let mut out = Vec::new();
                if let Some(usage) = usage {
                    usage.merge_into(&mut self.usage);
                    out.push(StreamEvent::Usage {
                        usage: Usage {
                            input_tokens: self.usage.input_tokens,
                            output_tokens: self.usage.output_tokens,
                            total_tokens: self
                                .usage
                                .input_tokens
                                .saturating_add(self.usage.cache_read_tokens)
                                .saturating_add(self.usage.cache_creation_tokens)
                                .saturating_add(self.usage.output_tokens),
                            reasoning_tokens: 0,
                            cache_read_tokens: self.usage.cache_read_tokens,
                            cache_creation_tokens: self.usage.cache_creation_tokens,
                        },
                    });
                }
                if let Some(stop_reason) = delta.stop_reason.as_deref() {
                    self.stop_reason = Some(map_stop_reason(stop_reason));
                }
                out
            }
            SseEvent::MessageStop => vec![StreamEvent::MessageStop {
                stop_reason: self.stop_reason.unwrap_or(StopReason::EndTurn),
                usage: Usage {
                    input_tokens: self.usage.input_tokens,
                    output_tokens: self.usage.output_tokens,
                    total_tokens: self
                        .usage
                        .input_tokens
                        .saturating_add(self.usage.cache_read_tokens)
                        .saturating_add(self.usage.cache_creation_tokens)
                        .saturating_add(self.usage.output_tokens),
                    reasoning_tokens: 0,
                    cache_read_tokens: self.usage.cache_read_tokens,
                    cache_creation_tokens: self.usage.cache_creation_tokens,
                },
            }],
            SseEvent::Ping | SseEvent::Unknown => Vec::new(),
            SseEvent::Error { .. } => Vec::new(),
        }
    }
}

fn map_stop_reason(raw: &str) -> StopReason {
    match raw {
        "end_turn" => StopReason::EndTurn,
        "tool_use" => StopReason::ToolUse,
        "max_tokens" => StopReason::MaxTokens,
        "stop_sequence" => StopReason::StopSequence,
        _ => StopReason::Other,
    }
}

fn event_error(error: wire::ApiErrorBody) -> AppError {
    let message = format_error_message(&error);
    if is_context_length_message(&message) {
        return AppError::ContextLength(message);
    }
    if is_retryable_error(&error.kind, &message) {
        let delay_ms = try_parse_retry_after_ms(&message);
        return AppError::RetryableStream { message, delay_ms };
    }
    AppError::Provider(message)
}

fn format_error_message(error: &wire::ApiErrorBody) -> String {
    match (error.kind.trim(), error.message.trim()) {
        (kind, message) if !kind.is_empty() && !message.is_empty() => {
            format!("{kind}: {message}")
        }
        (_, message) if !message.is_empty() => message.to_string(),
        (kind, _) if !kind.is_empty() => kind.to_string(),
        _ => "anthropic stream error".to_string(),
    }
}

fn is_retryable_error(kind: &str, message: &str) -> bool {
    let kind = kind.to_ascii_lowercase();
    let message = message.to_ascii_lowercase();
    matches!(
        kind.as_str(),
        "api_error"
            | "overloaded_error"
            | "rate_limit_error"
            | "timeout_error"
            | "service_unavailable"
            | "internal_server_error"
            | "server_error"
    ) || message.contains("overloaded")
        || message.contains("try again")
        || message.contains("temporarily")
        || message.contains("timeout")
        || message.contains("connection reset")
}

fn is_context_length_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    if lower.contains("beta") || lower.contains("not yet available") {
        return false;
    }
    lower.contains("prompt is too long")
        || lower.contains("input is too long")
        || lower.contains("too many tokens")
        || lower.contains("context window")
        || lower.contains("context length")
        || lower.contains("maximum context")
        || lower.contains("exceed") && lower.contains("context")
}

fn try_parse_retry_after_ms(message: &str) -> Option<u64> {
    let bytes = message.as_bytes();
    let lower = message.to_ascii_lowercase();
    let idx = lower.find("try again in")?;
    let mut cursor = idx + "try again in".len();
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    let number_start = cursor;
    while cursor < bytes.len() && (bytes[cursor].is_ascii_digit() || bytes[cursor] == b'.') {
        cursor += 1;
    }
    if cursor == number_start {
        return None;
    }
    let number: f64 = message.get(number_start..cursor)?.parse().ok()?;
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    let unit_start = cursor;
    while cursor < bytes.len() && bytes[cursor].is_ascii_alphabetic() {
        cursor += 1;
    }
    let unit = message.get(unit_start..cursor)?.to_ascii_lowercase();
    let ms = if unit == "ms" {
        number as u64
    } else if unit == "s" || unit.starts_with("second") {
        (number * 1_000.0) as u64
    } else {
        return None;
    };
    Some(ms.min(60_000))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overloaded_sse_error_is_retryable() {
        let err = event_error(wire::ApiErrorBody {
            kind: "overloaded_error".into(),
            message: "Overloaded. Please try again in 1.5s".into(),
        });

        assert!(matches!(
            err,
            AppError::RetryableStream {
                delay_ms: Some(1500),
                ..
            }
        ));
    }

    #[test]
    fn context_sse_error_is_context_length() {
        let err = event_error(wire::ApiErrorBody {
            kind: "invalid_request_error".into(),
            message: "prompt is too long: 250000 tokens > 200000 maximum".into(),
        });

        assert!(matches!(err, AppError::ContextLength(_)));
    }

    #[test]
    fn beta_unavailable_sse_error_is_not_context_length() {
        let err = event_error(wire::ApiErrorBody {
            kind: "invalid_request_error".into(),
            message: "The long context beta is not yet available for this subscription.".into(),
        });

        assert!(matches!(err, AppError::Provider(_)));
    }
}
