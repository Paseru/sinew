use std::sync::Arc;

use futures::{stream, SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::Value;
use sinew_core::{AppError, ProviderStream, Result};
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot, Mutex},
};
use tokio_tungstenite::{
    connect_async_tls_with_config,
    tungstenite::{
        client::IntoClientRequest,
        http::{HeaderValue, StatusCode},
        protocol::WebSocketConfig,
        Error as WsError, Message,
    },
    MaybeTlsStream, WebSocketStream,
};
use url::Url;

use crate::{
    auth::BearerToken,
    client::{OpenAiConfig, USER_AGENT},
    responses_stream::{event_provider_stream, is_terminal_response_event, STREAM_IDLE_TIMEOUT},
};

const RESPONSE_STREAM_CHANNEL_CAPACITY: usize = 1600;
const RESPONSES_WEBSOCKET_BETA: &str = "responses_websockets=2026-02-06";
const WEBSOCKET_CONNECTION_LIMIT_REACHED_CODE: &str = "websocket_connection_limit_reached";
const WEBSOCKET_CONNECTION_LIMIT_REACHED_MESSAGE: &str =
    "Responses websocket connection limit reached. Create a new websocket connection to continue.";

pub(crate) type WebsocketErrorCallback = Arc<dyn Fn() + Send + Sync + 'static>;

struct WsStream {
    tx_command: mpsc::Sender<WsCommand>,
    rx_message: mpsc::UnboundedReceiver<Result<Message, WsError>>,
    pump_task: tokio::task::JoinHandle<()>,
}

enum WsCommand {
    Send {
        message: Message,
        tx_result: oneshot::Sender<Result<(), WsError>>,
    },
}

impl WsStream {
    fn new(inner: WebSocketStream<MaybeTlsStream<TcpStream>>) -> Self {
        let (tx_command, mut rx_command) = mpsc::channel::<WsCommand>(32);
        let (tx_message, rx_message) = mpsc::unbounded_channel::<Result<Message, WsError>>();

        let pump_task = tokio::spawn(async move {
            let mut inner = inner;
            loop {
                tokio::select! {
                    command = rx_command.recv() => {
                        let Some(command) = command else {
                            break;
                        };
                        match command {
                            WsCommand::Send { message, tx_result } => {
                                let result = inner.send(message).await;
                                let should_break = result.is_err();
                                let _ = tx_result.send(result);
                                if should_break {
                                    break;
                                }
                            }
                        }
                    }
                    message = inner.next() => {
                        let Some(message) = message else {
                            break;
                        };
                        match message {
                            Ok(Message::Ping(payload)) => {
                                if let Err(err) = inner.send(Message::Pong(payload)).await {
                                    let _ = tx_message.send(Err(err));
                                    break;
                                }
                            }
                            Ok(Message::Pong(_)) => {}
                            Ok(message @ (Message::Text(_)
                                | Message::Binary(_)
                                | Message::Close(_)
                                | Message::Frame(_))) => {
                                let is_close = matches!(message, Message::Close(_));
                                if tx_message.send(Ok(message)).is_err() {
                                    break;
                                }
                                if is_close {
                                    break;
                                }
                            }
                            Err(err) => {
                                let _ = tx_message.send(Err(err));
                                break;
                            }
                        }
                    }
                }
            }
        });

        Self {
            tx_command,
            rx_message,
            pump_task,
        }
    }

    async fn request(
        &self,
        make_command: impl FnOnce(oneshot::Sender<Result<(), WsError>>) -> WsCommand,
    ) -> Result<(), WsError> {
        let (tx_result, rx_result) = oneshot::channel();
        if self.tx_command.send(make_command(tx_result)).await.is_err() {
            return Err(WsError::ConnectionClosed);
        }
        rx_result.await.unwrap_or(Err(WsError::ConnectionClosed))
    }

    async fn send(&self, message: Message) -> Result<(), WsError> {
        self.request(|tx_result| WsCommand::Send { message, tx_result })
            .await
    }

    async fn next(&mut self) -> Option<Result<Message, WsError>> {
        self.rx_message.recv().await
    }
}

impl Drop for WsStream {
    fn drop(&mut self) {
        self.pump_task.abort();
    }
}

#[derive(Clone)]
pub(crate) struct ResponsesWebsocketConnection {
    stream: Arc<Mutex<Option<WsStream>>>,
}

impl ResponsesWebsocketConnection {
    async fn connect(config: &OpenAiConfig, bearer: &BearerToken) -> Result<Self> {
        let ws_url = responses_websocket_url(if bearer.is_oauth {
            &config.codex_base_url
        } else {
            &config.api_base_url
        })?;
        let mut request = ws_url.as_str().into_client_request().map_err(|err| {
            AppError::Stream(format!("failed to build OpenAI websocket request: {err}"))
        })?;
        let headers = request.headers_mut();
        headers.insert(
            "authorization",
            header_value(&format!("Bearer {}", bearer.token), "authorization")?,
        );
        headers.insert("user-agent", HeaderValue::from_static(USER_AGENT));
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        headers.insert(
            "openai-beta",
            HeaderValue::from_static(RESPONSES_WEBSOCKET_BETA),
        );
        if bearer.is_oauth {
            if let Some(account_id) = bearer.account_id.as_deref() {
                headers.insert(
                    "chatgpt-account-id",
                    header_value(account_id, "chatgpt-account-id")?,
                );
            }
        }

        tracing::debug!(url = %ws_url, "connecting OpenAI responses websocket");
        let (websocket, response) =
            connect_async_tls_with_config(request, Some(WebSocketConfig::default()), false, None)
                .await
                .map_err(|err| map_ws_connect_error(err, &ws_url))?;
        tracing::debug!(status = %response.status(), "OpenAI responses websocket connected");

        Ok(Self {
            stream: Arc::new(Mutex::new(Some(WsStream::new(websocket)))),
        })
    }

    fn stream_request(
        &self,
        request_body: Value,
        default_model: String,
        on_stream_error: Option<WebsocketErrorCallback>,
    ) -> ProviderStream {
        let (tx_event, rx_event) = mpsc::channel::<Result<Value>>(RESPONSE_STREAM_CHANNEL_CAPACITY);
        let stream = Arc::clone(&self.stream);
        tokio::spawn(async move {
            let result =
                run_websocket_response_stream(stream, tx_event.clone(), request_body).await;
            if let Err(err) = result {
                if let Some(on_stream_error) = on_stream_error.as_ref() {
                    on_stream_error();
                }
                let _ = tx_event.send(Err(err)).await;
            }
        });
        let events = stream::unfold(rx_event, |mut rx_event| async move {
            rx_event.recv().await.map(|event| (event, rx_event))
        });
        event_provider_stream(events, default_model, "websocket")
    }
}

pub(crate) async fn stream_websocket_request(
    config: &OpenAiConfig,
    bearer: &BearerToken,
    request_body: Value,
    default_model: String,
    on_stream_error: Option<WebsocketErrorCallback>,
) -> Result<ProviderStream> {
    let connection = ResponsesWebsocketConnection::connect(config, bearer).await?;
    Ok(connection.stream_request(request_body, default_model, on_stream_error))
}

pub(crate) fn responses_websocket_url(base_url: &str) -> Result<Url> {
    let mut url = Url::parse(base_url)
        .map_err(|err| AppError::InvalidRequest(format!("invalid OpenAI base URL: {err}")))?;
    match url.scheme() {
        "http" => {
            let _ = url.set_scheme("ws");
        }
        "https" => {
            let _ = url.set_scheme("wss");
        }
        "ws" | "wss" => {}
        scheme => {
            return Err(AppError::InvalidRequest(format!(
                "unsupported OpenAI websocket URL scheme: {scheme}"
            )));
        }
    }

    let path = url.path().trim_end_matches('/').to_string();
    if path.ends_with("/responses") {
        url.set_path(&path);
    } else {
        url.set_path(&format!("{path}/responses"));
    }
    Ok(url)
}

async fn run_websocket_response_stream(
    stream: Arc<Mutex<Option<WsStream>>>,
    tx_event: mpsc::Sender<Result<Value>>,
    request_body: Value,
) -> Result<()> {
    let mut envelope = request_body;
    let object = envelope.as_object_mut().ok_or_else(|| {
        AppError::Decode("OpenAI responses request did not serialize to an object".into())
    })?;
    object.insert(
        "type".to_string(),
        Value::String("response.create".to_string()),
    );
    let payload = serde_json::to_string(&envelope).map_err(|err| {
        AppError::Decode(format!("failed to encode OpenAI websocket request: {err}"))
    })?;

    let mut guard = stream.lock().await;
    let result = match guard.as_mut() {
        Some(ws_stream) => run_websocket_response_stream_locked(ws_stream, tx_event, payload).await,
        None => Err(AppError::RetryableStream {
            message: "OpenAI websocket connection is closed".to_string(),
            delay_ms: None,
        }),
    };
    if result.is_err() {
        *guard = None;
    }
    result
}

async fn run_websocket_response_stream_locked(
    ws_stream: &mut WsStream,
    tx_event: mpsc::Sender<Result<Value>>,
    payload: String,
) -> Result<()> {
    tokio::time::timeout(
        STREAM_IDLE_TIMEOUT,
        ws_stream.send(Message::Text(payload.into())),
    )
    .await
    .map_err(|_| AppError::RetryableStream {
        message: "idle timeout sending OpenAI websocket request".to_string(),
        delay_ms: None,
    })?
    .map_err(|err| AppError::RetryableStream {
        message: format!("failed to send OpenAI websocket request: {err}"),
        delay_ms: None,
    })?;

    loop {
        let message = tokio::time::timeout(STREAM_IDLE_TIMEOUT, ws_stream.next())
            .await
            .map_err(|_| AppError::RetryableStream {
                message: "idle timeout waiting for OpenAI websocket".to_string(),
                delay_ms: None,
            })?;
        let message = match message {
            Some(Ok(message)) => message,
            Some(Err(err)) => {
                return Err(AppError::RetryableStream {
                    message: format!("failed to read OpenAI websocket event: {err}"),
                    delay_ms: None,
                });
            }
            None => {
                return Err(AppError::RetryableStream {
                    message: "OpenAI websocket closed before response.completed".to_string(),
                    delay_ms: None,
                });
            }
        };

        match message {
            Message::Text(text) => {
                let event = parse_text_event(text.as_ref())?;
                let terminal = is_terminal_response_event(&event);
                if tx_event.send(Ok(event)).await.is_err() {
                    return Err(AppError::Stream(
                        "OpenAI websocket event consumer dropped".to_string(),
                    ));
                }
                if terminal {
                    return Ok(());
                }
            }
            Message::Binary(_) => {
                return Err(AppError::RetryableStream {
                    message: "unexpected binary OpenAI websocket event".to_string(),
                    delay_ms: None,
                });
            }
            Message::Close(frame) => {
                return Err(AppError::RetryableStream {
                    message: format!(
                        "OpenAI websocket closed before response.completed: code={:?} reason={:?}",
                        frame.as_ref().map(|frame| frame.code),
                        frame.as_ref().map(|frame| frame.reason.to_string())
                    ),
                    delay_ms: None,
                });
            }
            Message::Frame(_) | Message::Ping(_) | Message::Pong(_) => {}
        }
    }
}

fn parse_text_event(text: &str) -> Result<Value> {
    tracing::trace!(target: "sinew_openai::websocket::wire", "OpenAI websocket event: {text}");
    if let Some(wrapped_error) = parse_wrapped_websocket_error_event(text) {
        if let Some(err) = map_wrapped_websocket_error_event(wrapped_error, text) {
            return Err(err);
        }
    }
    serde_json::from_str::<Value>(text)
        .map_err(|err| AppError::Decode(format!("bad OpenAI websocket event: {err}")))
}

fn retryable_ws_error(message: String) -> AppError {
    AppError::RetryableStream {
        message,
        delay_ms: None,
    }
}

fn header_value(value: &str, name: &str) -> Result<HeaderValue> {
    HeaderValue::from_str(value)
        .map_err(|err| AppError::InvalidRequest(format!("invalid {name} header: {err}")))
}

fn map_ws_connect_error(err: WsError, url: &Url) -> AppError {
    match err {
        WsError::Http(response) => map_http_status(
            response.status(),
            response
                .body()
                .as_ref()
                .and_then(|bytes| String::from_utf8(bytes.clone()).ok())
                .unwrap_or_else(|| format!("websocket upgrade failed for {url}")),
        ),
        WsError::ConnectionClosed | WsError::AlreadyClosed => {
            retryable_ws_error("OpenAI websocket closed during connect".into())
        }
        WsError::Io(err) => AppError::Network(format!("OpenAI websocket network error: {err}")),
        other => retryable_ws_error(format!("failed to connect OpenAI websocket: {other}")),
    }
}

#[derive(Debug, Deserialize)]
struct WrappedWebsocketErrorEvent {
    #[serde(rename = "type")]
    kind: String,
    #[serde(alias = "status_code")]
    status: Option<u16>,
    #[serde(default)]
    error: Option<WrappedWebsocketError>,
}

#[derive(Debug, Deserialize)]
struct WrappedWebsocketError {
    code: Option<String>,
    message: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
}

fn parse_wrapped_websocket_error_event(payload: &str) -> Option<WrappedWebsocketErrorEvent> {
    let event: WrappedWebsocketErrorEvent = serde_json::from_str(payload).ok()?;
    (event.kind == "error").then_some(event)
}

fn map_wrapped_websocket_error_event(
    event: WrappedWebsocketErrorEvent,
    original_payload: &str,
) -> Option<AppError> {
    if let Some(error) = event.error.as_ref() {
        if error.code.as_deref() == Some(WEBSOCKET_CONNECTION_LIMIT_REACHED_CODE) {
            return Some(retryable_ws_error(error.message.clone().unwrap_or_else(
                || WEBSOCKET_CONNECTION_LIMIT_REACHED_MESSAGE.to_string(),
            )));
        }
    }

    let status = StatusCode::from_u16(event.status?).ok()?;
    if status.is_success() {
        return None;
    }
    let message = event
        .error
        .and_then(|error| {
            let code = error.code.unwrap_or_default();
            let kind = error.kind.unwrap_or_default();
            let message = error.message.unwrap_or_default();
            let mut parts = Vec::new();
            if !kind.trim().is_empty() {
                parts.push(kind);
            }
            if !code.trim().is_empty() {
                parts.push(format!("({code})"));
            }
            if !message.trim().is_empty() {
                parts.push(message);
            }
            (!parts.is_empty()).then(|| parts.join(" "))
        })
        .unwrap_or_else(|| original_payload.to_string());
    Some(map_http_status(status, message))
}

fn map_http_status(status: StatusCode, message: String) -> AppError {
    if status == StatusCode::UNAUTHORIZED {
        AppError::Auth(message)
    } else if status == StatusCode::TOO_MANY_REQUESTS {
        AppError::RateLimit(message)
    } else if is_transient_http_status(status) {
        AppError::RetryableStream {
            message: format!("HTTP {status}: {message}"),
            delay_ms: None,
        }
    } else if status.is_client_error() {
        let lower = message.to_ascii_lowercase();
        if lower.contains("context") || lower.contains("too long") {
            AppError::ContextLength(message)
        } else {
            AppError::InvalidRequest(message)
        }
    } else {
        AppError::Provider(format!("HTTP {status}: {message}"))
    }
}

fn is_transient_http_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT
            | StatusCode::CONFLICT
            | StatusCode::TOO_EARLY
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    ) || status.as_u16() == 426
        || status.is_server_error()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_url_appends_responses_and_switches_scheme() {
        let url = responses_websocket_url("https://api.openai.com/v1").unwrap();
        assert_eq!(url.as_str(), "wss://api.openai.com/v1/responses");
    }

    #[test]
    fn websocket_url_keeps_existing_responses_path() {
        let url = responses_websocket_url("http://localhost:8080/v1/responses").unwrap();
        assert_eq!(url.as_str(), "ws://localhost:8080/v1/responses");
    }

    #[test]
    fn connection_limit_error_is_retryable() {
        let payload = serde_json::json!({
            "type": "error",
            "status": 429,
            "error": {
                "code": WEBSOCKET_CONNECTION_LIMIT_REACHED_CODE,
                "message": "limit"
            }
        })
        .to_string();
        let event = parse_wrapped_websocket_error_event(&payload).unwrap();
        assert!(matches!(
            map_wrapped_websocket_error_event(event, &payload),
            Some(AppError::RetryableStream { .. })
        ));
    }
}
