/// Force the Node `agent-bridge` subprocess (`SINEW_CURSOR_BRIDGE=node`).
pub fn prefer_node_bridge() -> bool {
    match std::env::var("SINEW_CURSOR_BRIDGE") {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "node" | "0" | "false"
        ),
        Err(_) => false,
    }
}

/// Use the native Rust HTTP/2 bridge (default).
pub fn use_rust_agent_bridge() -> bool {
    !prefer_node_bridge()
}

/// Force-disable automatic Node fallback (`SINEW_CURSOR_BRIDGE_FALLBACK=0`).
pub fn forbid_node_fallback() -> bool {
    match std::env::var("SINEW_CURSOR_BRIDGE_FALLBACK") {
        Ok(value) => matches!(value.trim().to_ascii_lowercase().as_str(), "0" | "false" | "no"),
        Err(_) => false,
    }
}

/// Retry via Node when Rust fails if the bundled/dev bridge is ready (no env var for users).
pub fn should_auto_fallback_to_node() -> bool {
    if prefer_node_bridge() || forbid_node_fallback() {
        return false;
    }
    super::setup::node_bridge_available()
}

/// Background `npm ci` only when a bridge dir exists but deps are missing (packaged app repair).
pub fn should_prepare_node_bridge_at_startup() -> bool {
    if prefer_node_bridge() {
        return true;
    }
    if forbid_node_fallback() {
        return false;
    }
    super::setup::bridge_directory().is_some_and(|dir| !super::setup::bridge_ready(&dir))
}

/// Transport selection for Cursor Composer streaming.
///
/// Defaults to `agent.v1` (works with Sinew OAuth). Set `SINEW_CURSOR_TRANSPORT=idempotent`
/// only to force the legacy IdempotentSSE path (currently broken server-side).
pub fn use_agent_transport() -> bool {
    match std::env::var("SINEW_CURSOR_TRANSPORT") {
        Ok(value) => {
            let trimmed = value.trim().to_ascii_lowercase();
            !matches!(trimmed.as_str(), "idempotent" | "sse" | "idempotent_sse")
        }
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::use_agent_transport;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn defaults_to_agent_without_env() {
        let _guard = env_lock();
        std::env::remove_var("SINEW_CURSOR_TRANSPORT");
        assert!(use_agent_transport());
    }

    #[test]
    fn rust_bridge_by_default() {
        let _guard = env_lock();
        std::env::remove_var("SINEW_CURSOR_BRIDGE");
        assert!(super::use_rust_agent_bridge());
        assert!(!super::prefer_node_bridge());
    }

    #[test]
    fn node_bridge_when_forced() {
        let _guard = env_lock();
        std::env::set_var("SINEW_CURSOR_BRIDGE", "node");
        assert!(!super::use_rust_agent_bridge());
        assert!(super::prefer_node_bridge());
        std::env::remove_var("SINEW_CURSOR_BRIDGE");
    }

    #[test]
    fn auto_fallback_uses_bundled_bridge_when_ready() {
        let _guard = env_lock();
        std::env::remove_var("SINEW_CURSOR_BRIDGE");
        std::env::remove_var("SINEW_CURSOR_BRIDGE_FALLBACK");
        let _ = super::should_auto_fallback_to_node();
    }

    #[test]
    fn forbid_fallback_when_disabled() {
        let _guard = env_lock();
        std::env::set_var("SINEW_CURSOR_BRIDGE_FALLBACK", "0");
        assert!(super::forbid_node_fallback());
        std::env::remove_var("SINEW_CURSOR_BRIDGE_FALLBACK");
    }

    #[test]
    fn idempotent_only_when_forced() {
        let _guard = env_lock();
        std::env::set_var("SINEW_CURSOR_TRANSPORT", "idempotent");
        assert!(!use_agent_transport());
        std::env::remove_var("SINEW_CURSOR_TRANSPORT");
    }
}
