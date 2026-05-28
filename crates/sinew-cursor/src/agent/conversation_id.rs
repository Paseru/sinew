use sha2::{Digest, Sha256};

/// Stable Cursor `conversationId` from Sinew chat cache key (survives restarts).
pub fn stable_agent_conversation_id(cache_key: Option<&str>) -> String {
    let seed = cache_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("sinew-default-conversation");
    deterministic_uuid(&format!("cursor-conv-id:{seed}"))
}

fn deterministic_uuid(seed: &str) -> String {
    let digest = Sha256::digest(seed.as_bytes());
    let bytes: [u8; 16] = digest[..16].try_into().expect("16 bytes");
    let b = bytes;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-4{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3],
        b[4], b[5],
        b[6], b[7],
        b[8], b[9],
        b[10], b[11], b[12], b[13], b[14], b[15],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_for_same_key() {
        let a = stable_agent_conversation_id(Some("chat-42"));
        let b = stable_agent_conversation_id(Some("chat-42"));
        assert_eq!(a, b);
        assert_ne!(a, stable_agent_conversation_id(Some("chat-43")));
    }
}
