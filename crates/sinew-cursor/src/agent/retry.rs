//! Retry/backoff for transient agent.v1 HTTP errors (rate limits, gateway).

use std::time::Duration;

use hyper::StatusCode;
use sinew_core::AppError;

pub const MAX_RUN_ATTEMPTS: u32 = 4;

pub fn is_retryable_status(status: StatusCode) -> bool {
    matches!(
        status.as_u16(),
        408 | 429 | 500 | 502 | 503 | 504
    )
}

pub fn backoff_before_retry(attempt: u32) -> Duration {
    let exp = attempt.min(6);
    let base_ms = 800u64.saturating_mul(1u64 << exp);
    let jitter = rand::random::<u64>() % 500;
    Duration::from_millis(base_ms.saturating_add(jitter))
}

pub fn is_retryable_network_err(err: &AppError) -> bool {
    let AppError::Network(msg) = err else {
        return false;
    };
    let lower = msg.to_ascii_lowercase();
    [
        "timeout",
        "timed out",
        "connection reset",
        "connection refused",
        "broken pipe",
        "temporarily unavailable",
        "dns",
        "handshake",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retryable_statuses() {
        assert!(is_retryable_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(is_retryable_status(StatusCode::SERVICE_UNAVAILABLE));
        assert!(!is_retryable_status(StatusCode::UNAUTHORIZED));
        assert!(!is_retryable_status(StatusCode::OK));
    }

    #[test]
    fn backoff_grows() {
        assert!(backoff_before_retry(0) >= Duration::from_millis(800));
        assert!(backoff_before_retry(2) > backoff_before_retry(0));
    }
}
