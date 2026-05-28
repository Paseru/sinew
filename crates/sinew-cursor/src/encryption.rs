use base64::Engine as _;
use sinew_core::{AppError, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobEncryptionKey([u8; 32]);

impl BlobEncryptionKey {
    pub fn from_raw(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn random() -> Self {
        let mut bytes = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rng(), &mut bytes);
        Self(bytes)
    }

    pub fn from_stored(value: &str) -> Result<Self> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(AppError::Provider("empty blob encryption key".into()));
        }
        if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(trimmed) {
            return Self::from_bytes(&decoded);
        }
        if let Ok(decoded) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(trimmed) {
            return Self::from_bytes(&decoded);
        }
        if trimmed.len() == 64 && trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
            let mut bytes = [0u8; 32];
            for (index, chunk) in bytes.iter_mut().enumerate() {
                let byte = u8::from_str_radix(&trimmed[index * 2..index * 2 + 2], 16)
                    .map_err(|err| AppError::Provider(format!("invalid encryption key hex: {err}")))?;
                *chunk = byte;
            }
            return Ok(Self(bytes));
        }
        Err(AppError::Provider(
            "unsupported blob encryption key encoding".into(),
        ))
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 32 {
            return Err(AppError::Provider(format!(
                "blob encryption key must be 32 bytes, got {}",
                bytes.len()
            )));
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(bytes);
        Ok(Self(out))
    }

    pub fn blob_header_hex(&self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    pub fn idempotent_header_b64(&self) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(self.0)
    }

    pub fn body_json_string(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(self.0)
    }

    pub fn persist_standard_b64(&self) -> String {
        self.body_json_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_standard_and_url_safe_storage() {
        let key = BlobEncryptionKey::random();
        let standard = key.persist_standard_b64();
        let restored = BlobEncryptionKey::from_stored(&standard).expect("standard b64");
        assert_eq!(restored, key);

        let url_safe = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key.0);
        let restored = BlobEncryptionKey::from_stored(&url_safe).expect("url-safe b64");
        assert_eq!(restored, key);
    }

    #[test]
    fn header_formats_are_distinct() {
        let key = BlobEncryptionKey([0xAB; 32]);
        assert_eq!(key.blob_header_hex(), "ab".repeat(32));
        assert_eq!(key.idempotent_header_b64(), base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0xAB; 32]));
    }
}
