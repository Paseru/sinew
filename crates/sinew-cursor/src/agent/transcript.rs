use sinew_core::{ChatMessage, Role};

#[derive(Debug, Clone, serde::Serialize)]
pub struct TranscriptTurn {
    pub user_text: String,
    pub assistant_text: String,
}

/// Split transcript into prior turns and the latest user message (Composer action).
pub fn split_transcript(transcript: &[ChatMessage]) -> (Vec<TranscriptTurn>, String) {
    let mut turns = Vec::new();
    let mut pending_user: Option<String> = None;

    for message in transcript {
        let text = message.text();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        match message.role {
            Role::User => {
                if let Some(user) = pending_user.take() {
                    turns.push(TranscriptTurn {
                        user_text: user,
                        assistant_text: String::new(),
                    });
                }
                pending_user = Some(trimmed.to_string());
            }
            Role::Assistant => {
                if let Some(user) = pending_user.take() {
                    turns.push(TranscriptTurn {
                        user_text: user,
                        assistant_text: trimmed.to_string(),
                    });
                }
            }
        }
    }

    let current_user = pending_user.unwrap_or_default();
    (turns, current_user)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sinew_core::ChatMessage;

    #[test]
    fn splits_history_and_current_user() {
        let transcript = vec![
            ChatMessage::user_text("hello"),
            ChatMessage::assistant_text("hi there"),
            ChatMessage::user_text("follow up"),
        ];
        let (turns, current) = split_transcript(&transcript);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].user_text, "hello");
        assert_eq!(turns[0].assistant_text, "hi there");
        assert_eq!(current, "follow up");
    }
}
