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

/// Opt-in Node fallback when the Rust bridge fails (`SINEW_CURSOR_BRIDGE_FALLBACK=1`).
pub fn allow_node_fallback() -> bool {
    match std::env::var("SINEW_CURSOR_BRIDGE_FALLBACK") {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "node"
        ),
        Err(_) => false,
    }
}

/// Pre-install Node `agent-bridge` at startup only when Node may run.
pub fn should_prepare_node_bridge_at_startup() -> bool {
    prefer_node_bridge() || allow_node_fallback()
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
    fn node_fallback_off_by_default() {
        let _guard = env_lock();
        std::env::remove_var("SINEW_CURSOR_BRIDGE_FALLBACK");
        assert!(!super::allow_node_fallback());
    }

    #[test]
    fn startup_skips_node_without_fallback() {
        let _guard = env_lock();
        std::env::remove_var("SINEW_CURSOR_BRIDGE");
        std::env::remove_var("SINEW_CURSOR_BRIDGE_FALLBACK");
        assert!(!super::should_prepare_node_bridge_at_startup());
    }

    #[test]
    fn idempotent_only_when_forced() {
        let _guard = env_lock();
        std::env::set_var("SINEW_CURSOR_TRANSPORT", "idempotent");
        assert!(!use_agent_transport());
        std::env::remove_var("SINEW_CURSOR_TRANSPORT");
    }
}
