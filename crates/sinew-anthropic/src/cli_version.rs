//! Tracks the Claude Code CLI version advertised in our `user-agent` header.
//!
//! Anthropic gates newer models behind a minimum Claude Code version, so a
//! hardcoded constant goes stale every time a model ships (e.g. Opus 5.5
//! requires >= 2.1.280 while Fable 5.1 required >= 2.1.251). Three layers keep
//! us current without a manual bump per release:
//!
//!   1. [`FALLBACK_VERSION`] — compiled into the binary, used before the first
//!      successful refresh and whenever the network is unavailable.
//!   2. a background refresh of the latest published `@anthropic-ai/claude-code`
//!      version from the npm registry, cached on disk with a TTL.
//!   3. [`adopt_required_version`] — if the API answers
//!      "version X or newer is required", we adopt X immediately and the caller
//!      retries the request once.

use std::cmp::Ordering;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

/// Claude Code version compiled into this build.
///
/// Only a floor: the runtime refresh below normally beats it, and the
/// server-driven fallback in [`adopt_required_version`] covers the case where
/// both the cache and the registry are unavailable.
pub const FALLBACK_VERSION: &str = "2.1.280";

/// `latest` dist-tag of the Claude Code npm package.
const REGISTRY_URL: &str = "https://registry.npmjs.org/@anthropic-ai%2Fclaude-code/latest";
/// How long a successful lookup is trusted before we ask the registry again.
const REFRESH_TTL_MS: i64 = 6 * 60 * 60 * 1_000;
/// Backoff after a failed lookup, so a flaky network does not retry on every request.
const FAILURE_BACKOFF_MS: i64 = 30 * 60 * 1_000;
const FETCH_TIMEOUT: Duration = Duration::from_secs(5);

struct State {
    version: String,
    checked_at_ms: i64,
    in_flight: bool,
    cache_loaded: bool,
}

#[derive(Serialize, Deserialize)]
struct CachedVersion {
    version: String,
    checked_at_ms: i64,
}

#[derive(Deserialize)]
struct RegistryLatest {
    #[serde(default)]
    version: String,
}

/// Current `claude-cli` version we advertise. Loads the on-disk cache on first
/// use, so a fresh process starts from the newest version we already know.
pub fn version() -> String {
    let mut state = lock();
    load_cache(&mut state);
    state.version.clone()
}

/// Full `user-agent` value, e.g. `claude-cli/2.1.280`.
pub fn user_agent() -> String {
    format!("claude-cli/{}", version())
}

/// Kick off a background refresh when the cached version is older than the TTL.
///
/// Never blocks the caller and never fails: any lookup problem leaves the
/// current version in place.
pub fn ensure_fresh(http: &reqwest::Client) {
    let mut state = lock();
    load_cache(&mut state);
    let now = now_ms();
    if state.in_flight || now.saturating_sub(state.checked_at_ms) < REFRESH_TTL_MS {
        return;
    }
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        // No async runtime (sync CLI/tests): keep the cached or compiled version.
        return;
    };
    state.in_flight = true;
    drop(state);

    let http = http.clone();
    handle.spawn(async move {
        refresh(&http).await;
        lock().in_flight = false;
    });
}

/// Adopt the version demanded by an API error message, if there is one.
///
/// Returns the adopted version so the caller can log it and retry once.
pub fn adopt_required_version(message: &str) -> Option<String> {
    let required = parse_required_version(message)?;
    let mut state = lock();
    load_cache(&mut state);
    if is_newer(&required, &state.version) {
        state.version = required.clone();
        persist(&state.version);
    }
    // We just learned the truth from the server; do not refetch immediately.
    state.checked_at_ms = now_ms();
    Some(required)
}

/// Extract `X.Y.Z` from messages like
/// `version 2.1.280 or newer is required` / `2.1.280 or later`.
pub fn parse_required_version(message: &str) -> Option<String> {
    let index = message
        .find("or newer")
        .or_else(|| message.find("or later"))
        .or_else(|| message.find("or higher"))?;
    let candidate = message[..index]
        .split_whitespace()
        .next_back()?
        .trim_matches(|ch: char| !(ch.is_ascii_digit() || ch == '.'));
    is_version_like(candidate).then(|| candidate.to_string())
}

async fn refresh(http: &reqwest::Client) {
    let fetched = fetch_latest(http).await;
    let mut state = lock();
    state.checked_at_ms = now_ms();
    match fetched {
        Some(version) => {
            if is_newer(&version, &state.version) {
                tracing::debug!(%version, "claude-cli version refreshed from npm");
                state.version = version;
            }
            persist(&state.version);
        }
        None => {
            state.checked_at_ms = now_ms() - REFRESH_TTL_MS + FAILURE_BACKOFF_MS;
            tracing::debug!(
                version = %state.version,
                "claude-cli version lookup failed; keeping current version"
            );
        }
    }
}

async fn fetch_latest(http: &reqwest::Client) -> Option<String> {
    let response = http
        .get(REGISTRY_URL)
        .timeout(FETCH_TIMEOUT)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let payload: RegistryLatest = response.json().await.ok()?;
    let version = payload.version.trim().to_string();
    is_version_like(&version).then_some(version)
}

fn lock() -> MutexGuard<'static, State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE
        .get_or_init(|| {
            Mutex::new(State {
                version: FALLBACK_VERSION.to_string(),
                checked_at_ms: 0,
                in_flight: false,
                cache_loaded: false,
            })
        })
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

fn load_cache(state: &mut State) {
    if state.cache_loaded {
        return;
    }
    state.cache_loaded = true;
    let Some(path) = cache_path() else {
        return;
    };
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let Ok(cached) = serde_json::from_slice::<CachedVersion>(&bytes) else {
        return;
    };
    if is_version_like(&cached.version) && is_newer(&cached.version, &state.version) {
        state.version = cached.version;
    }
    state.checked_at_ms = cached.checked_at_ms;
}

fn persist(version: &str) {
    if cfg!(test) {
        return;
    }
    let Some(path) = cache_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let payload = CachedVersion {
        version: version.to_string(),
        checked_at_ms: now_ms(),
    };
    if let Ok(bytes) = serde_json::to_vec(&payload) {
        let _ = std::fs::write(path, bytes);
    }
}

fn cache_path() -> Option<PathBuf> {
    let dirs = ProjectDirs::from("dev", "hyrak", "sinew")?;
    Some(dirs.data_local_dir().join("claude-code-version.json"))
}

fn is_version_like(value: &str) -> bool {
    !value.is_empty()
        && value.contains('.')
        && value.ends_with(|ch: char| ch.is_ascii_digit())
        && value.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
}

fn is_newer(candidate: &str, current: &str) -> bool {
    compare(candidate, current) == Ordering::Greater
}

fn compare(left: &str, right: &str) -> Ordering {
    let mut left = left.split('.');
    let mut right = right.split('.');
    loop {
        match (left.next(), right.next()) {
            (None, None) => return Ordering::Equal,
            (lhs, rhs) => {
                let lhs = lhs.and_then(|part| part.parse::<u64>().ok()).unwrap_or(0);
                let rhs = rhs.and_then(|part| part.parse::<u64>().ok()).unwrap_or(0);
                match lhs.cmp(&rhs) {
                    Ordering::Equal => continue,
                    other => return other,
                }
            }
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_required_version_from_api_error() {
        let message = "invalid_request_error: Claude Code 2.1.251 does not support this model; \
                       version 2.1.280 or newer is required. Run 'claude update', or update the \
                       Claude desktop app, then try again.";
        assert_eq!(parse_required_version(message).as_deref(), Some("2.1.280"));
    }

    #[test]
    fn parses_alternate_wording() {
        assert_eq!(
            parse_required_version("version 3.0.1 or later is required").as_deref(),
            Some("3.0.1")
        );
    }

    #[test]
    fn ignores_unrelated_errors() {
        assert_eq!(
            parse_required_version("prompt is too long: 250000 tokens"),
            None
        );
        assert_eq!(parse_required_version("version or newer is required"), None);
    }

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("2.1.280", "2.1.251"));
        assert!(is_newer("2.2.0", "2.1.999"));
        assert!(is_newer("3.0", "2.99.99"));
        assert!(!is_newer("2.1.251", "2.1.280"));
        assert!(!is_newer("2.1.280", "2.1.280"));
        assert!(!is_newer("garbage", "2.1.280"));
    }

    #[test]
    fn adopts_a_newer_version_immediately() {
        let adopted = adopt_required_version("version 9.9.9 or newer is required");
        assert_eq!(adopted.as_deref(), Some("9.9.9"));
        assert_eq!(version(), "9.9.9");
    }

    #[test]
    fn user_agent_is_prefixed() {
        assert!(user_agent().starts_with("claude-cli/"));
    }
}
