use thiserror::Error;

pub type Result<T, E = AppError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("auth error: {0}")]
    Auth(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("rate limited: {0}")]
    RateLimit(String),
    #[error("context window exceeded: {0}")]
    ContextLength(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("stream error: {0}")]
    Stream(String),
    #[error("retryable stream error: {message}")]
    RetryableStream {
        message: String,
        delay_ms: Option<u64>,
    },
    #[error("unsupported feature: {0}")]
    Unsupported(String),
    #[error("provider error: {0}")]
    Provider(String),
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::Decode(value.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Network(value.to_string())
    }
}
