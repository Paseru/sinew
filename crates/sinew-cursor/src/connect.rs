use bytes::Bytes;
use sinew_core::{AppError, Result};

pub fn frame_connect_json(payload: &[u8], flags: u8) -> Vec<u8> {
    let mut frame = Vec::with_capacity(5 + payload.len());
    frame.push(flags);
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}

pub fn decode_connect_frames(buffer: &mut Vec<u8>) -> Result<Vec<Bytes>> {
    let mut frames = Vec::new();
    loop {
        if buffer.len() < 5 {
            break;
        }
        let flags = buffer[0];
        let length = u32::from_be_bytes([buffer[1], buffer[2], buffer[3], buffer[4]]) as usize;
        if buffer.len() < 5 + length {
            break;
        }
        let payload = buffer[5..5 + length].to_vec();
        buffer.drain(..5 + length);
        if flags & 0x02 != 0 && payload.is_empty() {
            continue;
        }
        frames.push(Bytes::from(payload));
    }
    Ok(frames)
}

pub fn parse_json_text(payload: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(payload).ok()?;
    if let Some(text) = value.get("text").and_then(|item| item.as_str()) {
        return Some(text.to_string());
    }
    if let Some(error) = value.get("error") {
        let message = error
            .get("message")
            .and_then(|item| item.as_str())
            .unwrap_or("Cursor composer error");
        return Some(format!("[cursor-error] {message}"));
    }
    None
}

pub fn connect_error(payload: &[u8]) -> Option<AppError> {
    let value: serde_json::Value = serde_json::from_slice(payload).ok()?;
    let error = value.get("error")?;
    let message = error
        .get("message")
        .and_then(|item| item.as_str())
        .unwrap_or("Cursor connect error");
    Some(AppError::Network(message.to_string()))
}
