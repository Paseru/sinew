#[derive(Debug, Clone)]
pub(crate) struct DecodedText {
    pub content: String,
    pub encoding: TextEncoding,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum TextEncoding {
    Utf8,
    Utf8Bom,
    Utf16Le { bom: bool },
    Utf16Be { bom: bool },
}

pub(crate) fn decode_text(bytes: &[u8]) -> Option<DecodedText> {
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        let content = std::str::from_utf8(&bytes[3..]).ok()?.to_string();
        return text_if_readable(content, TextEncoding::Utf8Bom);
    }

    if bytes.starts_with(&[0xff, 0xfe]) {
        return decode_utf16(&bytes[2..], true, TextEncoding::Utf16Le { bom: true });
    }

    if bytes.starts_with(&[0xfe, 0xff]) {
        return decode_utf16(&bytes[2..], false, TextEncoding::Utf16Be { bom: true });
    }

    if let Ok(content) = std::str::from_utf8(bytes) {
        return text_if_readable(content.to_string(), TextEncoding::Utf8);
    }

    if looks_like_utf16_le(bytes) {
        return decode_utf16(bytes, true, TextEncoding::Utf16Le { bom: false });
    }

    if looks_like_utf16_be(bytes) {
        return decode_utf16(bytes, false, TextEncoding::Utf16Be { bom: false });
    }

    None
}

pub(crate) fn encode_text(content: &str, encoding: TextEncoding) -> Vec<u8> {
    match encoding {
        TextEncoding::Utf8 => content.as_bytes().to_vec(),
        TextEncoding::Utf8Bom => {
            let mut bytes = vec![0xef, 0xbb, 0xbf];
            bytes.extend_from_slice(content.as_bytes());
            bytes
        }
        TextEncoding::Utf16Le { bom } => encode_utf16(content, true, bom),
        TextEncoding::Utf16Be { bom } => encode_utf16(content, false, bom),
    }
}

fn decode_utf16(bytes: &[u8], little_endian: bool, encoding: TextEncoding) -> Option<DecodedText> {
    let chunks = bytes.chunks_exact(2);
    if !chunks.remainder().is_empty() {
        return None;
    }

    let units = chunks
        .map(|chunk| {
            if little_endian {
                u16::from_le_bytes([chunk[0], chunk[1]])
            } else {
                u16::from_be_bytes([chunk[0], chunk[1]])
            }
        })
        .collect::<Vec<_>>();
    let content = String::from_utf16(&units).ok()?;
    text_if_readable(content, encoding)
}

fn encode_utf16(content: &str, little_endian: bool, bom: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    if bom {
        if little_endian {
            bytes.extend_from_slice(&[0xff, 0xfe]);
        } else {
            bytes.extend_from_slice(&[0xfe, 0xff]);
        }
    }

    for unit in content.encode_utf16() {
        let pair = if little_endian {
            unit.to_le_bytes()
        } else {
            unit.to_be_bytes()
        };
        bytes.extend_from_slice(&pair);
    }

    bytes
}

fn text_if_readable(content: String, encoding: TextEncoding) -> Option<DecodedText> {
    if has_too_many_control_chars(&content) {
        return None;
    }

    Some(DecodedText { content, encoding })
}

fn has_too_many_control_chars(content: &str) -> bool {
    let mut checked = 0usize;
    let mut control = 0usize;

    for ch in content.chars().take(4096) {
        checked += 1;
        if ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t' {
            control += 1;
        }
    }

    checked > 0 && control * 100 > checked * 2
}

fn looks_like_utf16_le(bytes: &[u8]) -> bool {
    looks_like_utf16(bytes, true)
}

fn looks_like_utf16_be(bytes: &[u8]) -> bool {
    looks_like_utf16(bytes, false)
}

fn looks_like_utf16(bytes: &[u8], little_endian: bool) -> bool {
    let mut total = 0usize;
    let mut nul_in_expected_slot = 0usize;
    let mut nul_in_other_slot = 0usize;

    for chunk in bytes.chunks_exact(2).take(512) {
        total += 1;
        let expected = if little_endian { chunk[1] } else { chunk[0] };
        let other = if little_endian { chunk[0] } else { chunk[1] };
        if expected == 0 {
            nul_in_expected_slot += 1;
        }
        if other == 0 {
            nul_in_other_slot += 1;
        }
    }

    total >= 2 && nul_in_expected_slot * 2 >= total && nul_in_other_slot * 4 <= total
}

#[cfg(test)]
mod tests {
    use super::{decode_text, encode_text, TextEncoding};

    #[test]
    fn decodes_utf8_text() {
        let decoded = decode_text(b"import SwiftUI\n").expect("utf8 text should decode");
        assert_eq!(decoded.content, "import SwiftUI\n");
    }

    #[test]
    fn decodes_utf16_le_with_bom() {
        let bytes = encode_text("import SwiftUI\n", TextEncoding::Utf16Le { bom: true });
        let decoded = decode_text(&bytes).expect("utf16 text should decode");
        assert_eq!(decoded.content, "import SwiftUI\n");
    }

    #[test]
    fn rejects_binary_bytes() {
        let bytes = [0, 159, 146, 150, 0, 1, 2, 3];
        assert!(decode_text(&bytes).is_none());
    }
}
