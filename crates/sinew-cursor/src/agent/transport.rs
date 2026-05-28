/// Transport selection for Cursor Composer streaming.
pub fn use_agent_transport() -> bool {
    std::env::var("SINEW_CURSOR_TRANSPORT")
        .map(|value| {
            let trimmed = value.trim().to_ascii_lowercase();
            trimmed == "agent"
        })
        .unwrap_or(false)
}
