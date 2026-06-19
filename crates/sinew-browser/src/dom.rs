// DOM formatting utilities — unused directly (logic embedded in session.rs via JS injection)

pub fn truncate_dom(text: String, char_limit: usize) -> String {
    if text.len() <= char_limit {
        return text;
    }
    let truncated = &text[..char_limit];
    let last_newline = truncated.rfind('\n').unwrap_or(char_limit);
    format!("{}\n... (truncated)", &text[..last_newline])
}
