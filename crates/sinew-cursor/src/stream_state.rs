use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::encryption::BlobEncryptionKey;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PersistedStateFile {
    conversations: HashMap<String, PersistedConversationState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedConversationState {
    idempotency_key: String,
    #[serde(default)]
    encryption_key: Option<String>,
    seqno: u32,
}

#[derive(Debug, Clone)]
pub struct ConversationStreamState {
    pub idempotency_key: String,
    pub encryption_key: String,
    pub seqno: u32,
}

pub struct StreamStateStore {
    path: PathBuf,
    conversations: HashMap<String, ConversationStreamState>,
}

impl StreamStateStore {
    pub fn load() -> Self {
        let path = stream_state_path();
        let conversations = fs::read_to_string(&path)
            .ok()
            .and_then(|json| serde_json::from_str::<PersistedStateFile>(&json).ok())
            .map(|file| {
                file.conversations
                    .into_iter()
                    .map(|(key, value)| {
                        let encryption_key = value
                            .encryption_key
                            .and_then(|stored| BlobEncryptionKey::from_stored(&stored).ok())
                            .unwrap_or_else(BlobEncryptionKey::random)
                            .persist_standard_b64();
                        (
                            key,
                            ConversationStreamState {
                                idempotency_key: value.idempotency_key,
                                encryption_key,
                                seqno: value.seqno,
                            },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self { path, conversations }
    }

    pub fn conversation_state(&mut self, cache_key: &str) -> ConversationStreamState {
        self.conversations
            .entry(cache_key.to_string())
            .or_insert_with(|| {
                let encryption_key = BlobEncryptionKey::random().persist_standard_b64();
                ConversationStreamState {
                    idempotency_key: uuid::Uuid::new_v4().to_string(),
                    encryption_key,
                    seqno: 0,
                }
            })
            .clone()
    }

    pub fn update_seqno(&mut self, cache_key: &str, seqno: u32) {
        if let Some(state) = self.conversations.get_mut(cache_key) {
            state.seqno = seqno;
            let _ = self.save();
        }
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("unable to create {}", parent.display()))?;
        }
        let payload = PersistedStateFile {
            conversations: self
                .conversations
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        PersistedConversationState {
                            idempotency_key: value.idempotency_key.clone(),
                            encryption_key: Some(value.encryption_key.clone()),
                            seqno: value.seqno,
                        },
                    )
                })
                .collect(),
        };
        let json = serde_json::to_string_pretty(&payload)?;
        fs::write(&self.path, json)
            .with_context(|| format!("unable to write {}", self.path.display()))?;
        Ok(())
    }
}

fn stream_state_path() -> PathBuf {
    directories::ProjectDirs::from("dev", "hyrak", "sinew")
        .map(|dirs| dirs.data_dir().join("cursor-composer-stream-state.json"))
        .unwrap_or_else(|| PathBuf::from("cursor-composer-stream-state.json"))
}
