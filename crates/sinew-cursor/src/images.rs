use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde_json::{json, Value};
use sinew_core::{ChatMessage, Part};

/// Build Cursor `ConversationMessage.images` payloads (`ImageProto` in JSON).
pub fn message_images(message: &ChatMessage) -> Vec<Value> {
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            Part::Image { media_type, data, .. } => wire_image(media_type, data),
            _ => None,
        })
        .collect()
}

fn wire_image(media_type: &str, data: &str) -> Option<Value> {
    let trimmed = data.trim();
    if trimmed.is_empty() {
        return None;
    }
    let bytes = BASE64_STANDARD.decode(trimmed).ok()?;
    if bytes.is_empty() {
        return None;
    }
    let (width, height) = image_dimensions(&bytes, media_type);
    Some(json!({
        "data": trimmed,
        "dimension": {
            "width": width.max(1),
            "height": height.max(1),
        }
    }))
}

fn image_dimensions(bytes: &[u8], media_type: &str) -> (i32, i32) {
    match media_type {
        "image/png" => png_dimensions(bytes),
        "image/jpeg" | "image/jpg" => jpeg_dimensions(bytes),
        "image/gif" => gif_dimensions(bytes),
        "image/webp" => webp_dimensions(bytes),
        _ => (0, 0),
    }
}

fn png_dimensions(bytes: &[u8]) -> (i32, i32) {
    if bytes.len() < 24 || &bytes[0..8] != [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        return (0, 0);
    }
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as i32;
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]) as i32;
    (width, height)
}

fn gif_dimensions(bytes: &[u8]) -> (i32, i32) {
    if bytes.len() < 10 || &bytes[0..3] != b"GIF" {
        return (0, 0);
    }
    let width = u16::from_le_bytes([bytes[6], bytes[7]]) as i32;
    let height = u16::from_le_bytes([bytes[8], bytes[9]]) as i32;
    (width, height)
}

fn jpeg_dimensions(bytes: &[u8]) -> (i32, i32) {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return (0, 0);
    }
    let mut index = 2usize;
    while index + 4 < bytes.len() {
        if bytes[index] != 0xFF {
            index += 1;
            continue;
        }
        while index < bytes.len() && bytes[index] == 0xFF {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        let marker = bytes[index];
        index += 1;
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        if index + 1 >= bytes.len() {
            break;
        }
        let segment_len = u16::from_be_bytes([bytes[index], bytes[index + 1]]) as usize;
        if segment_len < 2 || index + segment_len > bytes.len() {
            break;
        }
        if matches!(
            marker,
            0xC0 | 0xC1 | 0xC2 | 0xC3 | 0xC5 | 0xC6 | 0xC7 | 0xC9 | 0xCA | 0xCB | 0xCD | 0xCE | 0xCF
        ) && index + 7 <= bytes.len()
        {
            let height = u16::from_be_bytes([bytes[index + 3], bytes[index + 4]]) as i32;
            let width = u16::from_be_bytes([bytes[index + 5], bytes[index + 6]]) as i32;
            return (width, height);
        }
        index += segment_len;
    }
    (0, 0)
}

fn webp_dimensions(bytes: &[u8]) -> (i32, i32) {
    if bytes.len() < 30 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return (0, 0);
    }
    let mut offset = 12usize;
    while offset + 8 <= bytes.len() {
        let tag = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        offset += 8;
        if tag == b"VP8 " && offset + 10 <= bytes.len() {
            let width = (u16::from_le_bytes([bytes[offset + 6], bytes[offset + 7]]) & 0x3FFF) as i32;
            let height = (u16::from_le_bytes([bytes[offset + 8], bytes[offset + 9]]) & 0x3FFF) as i32;
            return (width, height);
        }
        if tag == b"VP8L" && offset + 5 <= bytes.len() {
            let bits = u32::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
            let width = ((bits & 0x3FFF) + 1) as i32;
            let height = (((bits >> 14) & 0x3FFF) + 1) as i32;
            return (width, height);
        }
        if tag == b"VP8X" && offset + 10 <= bytes.len() {
            let width = 1
                + (bytes[offset + 4] as i32
                    | ((bytes[offset + 5] as i32) << 8)
                    | ((bytes[offset + 6] as i32) << 16));
            let height = 1
                + (bytes[offset + 7] as i32
                    | ((bytes[offset + 8] as i32) << 8)
                    | ((bytes[offset + 9] as i32) << 16));
            return (width, height);
        }
        offset = offset.saturating_add(size + (size & 1));
    }
    (0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sinew_core::{ChatMessage, Role};

    const PNG_1X1: &str =
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

    #[test]
    fn builds_wire_image_with_dimensions() {
        let images = message_images(&ChatMessage {
            role: Role::User,
            parts: vec![
                Part::Image {
                    media_type: "image/png".into(),
                    data: PNG_1X1.into(),
                    meta: None,
                },
                Part::Text {
                    text: "What is this?".into(),
                    meta: None,
                },
            ],
        });
        assert_eq!(images.len(), 1);
        assert_eq!(images[0]["dimension"]["width"], 1);
        assert_eq!(images[0]["dimension"]["height"], 1);
        assert_eq!(images[0]["data"].as_str(), Some(PNG_1X1));
    }
}
