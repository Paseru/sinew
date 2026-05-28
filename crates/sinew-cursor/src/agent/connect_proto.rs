//! Connect-RPC framing for `application/connect+proto` (agent.v1).

pub use crate::connect::{append_end_stream_frame, decode_connect_frames, frame_connect_json};

/// Wrap raw protobuf payload in a Connect data frame (flags = 0).
pub fn frame_connect_proto(payload: &[u8]) -> Vec<u8> {
    frame_connect_json(payload, 0)
}

/// Parse Connect end-stream trailer JSON for an error message, if any.
pub fn parse_connect_end_error(payload: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(payload).ok()?;
    let error = value.get("error")?;
    let code = error.get("code").and_then(|v| v.as_str()).unwrap_or("?");
    let message = error
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    Some(format!("{code}: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_connect_proto_frame() {
        let payload = b"hello agent";
        let mut buffer = frame_connect_proto(payload);
        let frames = decode_connect_frames(&mut buffer).expect("decode");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].as_ref(), payload);
    }
}
