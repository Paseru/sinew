use eventsource_stream::Eventsource;
use futures::stream::{self, Stream};
use futures::{StreamExt, TryStreamExt};
use serde_json::Value;
use sinew_core::{AppError, ProviderStream, StreamEvent};

use crate::stream::EventParser;

pub(crate) const STREAM_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

pub(crate) fn event_provider_stream<S, E>(
    events: S,
    default_model: String,
    source_name: &'static str,
) -> ProviderStream
where
    S: Stream<Item = std::result::Result<Value, E>> + Send + 'static,
    E: Into<AppError> + Send + 'static,
{
    let source = Box::pin(events);
    let parser = EventParser::new(default_model);

    stream::unfold(
        (source, parser, Vec::<StreamEvent>::new(), false, false),
        move |(mut source, mut parser, mut pending, mut completed, mut saw_any_event)| async move {
            loop {
                if let Some(next) = pending.pop() {
                    return Some((
                        Ok(next),
                        (source, parser, pending, completed, saw_any_event),
                    ));
                }
                if completed {
                    return None;
                }

                match tokio::time::timeout(STREAM_IDLE_TIMEOUT, source.next()).await {
                    Ok(Some(Ok(event))) => {
                        saw_any_event = true;
                        let terminal = is_terminal_response_event(&event);
                        match parser.push(event) {
                            Ok(mut produced) => {
                                if terminal {
                                    completed = true;
                                }
                                produced.reverse();
                                pending.extend(produced);
                            }
                            Err(err) => {
                                return Some((
                                    Err(err),
                                    (source, parser, pending, true, saw_any_event),
                                ));
                            }
                        }
                    }
                    Ok(Some(Err(err))) => {
                        return Some((
                            Err(err.into()),
                            (source, parser, pending, true, saw_any_event),
                        ));
                    }
                    Ok(None) => {
                        if !saw_any_event {
                            return Some((
                                Err(AppError::RetryableStream {
                                    message: format!(
                                        "openai {source_name} closed before any event; the server likely dropped the connection"
                                    ),
                                    delay_ms: None,
                                }),
                                (source, parser, pending, true, saw_any_event),
                            ));
                        }
                        return Some((
                            Err(AppError::RetryableStream {
                                message: format!(
                                    "openai {source_name} stream closed before response.completed"
                                ),
                                delay_ms: None,
                            }),
                            (source, parser, pending, true, saw_any_event),
                        ));
                    }
                    Err(_) => {
                        return Some((
                            Err(AppError::RetryableStream {
                                message: format!("idle timeout waiting for OpenAI {source_name}"),
                                delay_ms: None,
                            }),
                            (source, parser, pending, true, saw_any_event),
                        ));
                    }
                }
            }
        },
    )
    .boxed()
}

pub(crate) fn sse_event_stream<S, E>(
    body: S,
) -> impl Stream<Item = Result<Value, AppError>> + Send + 'static
where
    S: Stream<Item = std::result::Result<bytes::Bytes, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    body.eventsource()
        .map_err(|err| AppError::RetryableStream {
            message: format!("openai SSE error: {err}"),
            delay_ms: None,
        })
        .try_filter_map(|event| async move {
            let data = event.data.trim();
            if data == "[DONE]" {
                return Ok(None);
            }
            serde_json::from_str::<Value>(&event.data)
                .map(Some)
                .map_err(|err| AppError::Decode(format!("bad openai SSE event: {err}")))
        })
}

pub(crate) fn is_terminal_response_event(event: &Value) -> bool {
    matches!(
        event.get("type").and_then(Value::as_str),
        Some("response.completed" | "response.incomplete")
    )
}
