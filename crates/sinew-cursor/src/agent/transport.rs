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

/// Use the native Rust HTTP/2 bridge (default). Falls back to Node on error unless
/// `SINEW_CURSOR_BRIDGE=node` forces Node only.
pub fn use_rust_agent_bridge() -> bool {
    !prefer_node_bridge()
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
    fn idempotent_only_when_forced() {
        let _guard = env_lock();
        std::env::set_var("SINEW_CURSOR_TRANSPORT", "idempotent");
        assert!(!use_agent_transport());
        std::env::remove_var("SINEW_CURSOR_TRANSPORT");
    }
}
