use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PersistedFile {
    conversations: HashMap<String, PersistedAgentConversation>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistedAgentConversation {
    #[serde(default)]
    pub checkpoint_b64: Option<String>,
    /// hex blob id -> base64 blob bytes
    #[serde(default)]
    pub blobs: HashMap<String, String>,
}

pub struct AgentConversationStore {
    path: PathBuf,
    conversations: HashMap<String, PersistedAgentConversation>,
}

impl AgentConversationStore {
    pub fn load() -> Self {
        let path = store_path();
        let conversations = fs::read_to_string(&path)
            .ok()
            .and_then(|json| serde_json::from_str::<PersistedFile>(&json).ok())
            .map(|file| file.conversations)
            .unwrap_or_default();
        Self {
            path,
            conversations,
        }
    }

    pub fn get(&self, cache_key: &str) -> Option<PersistedAgentConversation> {
        if cache_key.trim().is_empty() {
            return None;
        }
        self.conversations.get(cache_key).cloned()
    }

    pub fn save_checkpoint(
        &mut self,
        cache_key: &str,
        checkpoint_b64: String,
        blobs: HashMap<String, String>,
    ) -> Result<()> {
        if cache_key.trim().is_empty() {
            return Ok(());
        }
        let (checkpoint_b64, blobs) = trim_checkpoint_payload(checkpoint_b64, blobs);
        self.conversations.insert(
            cache_key.to_string(),
            PersistedAgentConversation {
                checkpoint_b64: Some(checkpoint_b64),
                blobs,
            },
        );
        self.flush()
    }

    fn flush(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("unable to create {}", parent.display()))?;
        }
        let payload = PersistedFile {
            conversations: self.conversations.clone(),
        };
        let json = serde_json::to_string_pretty(&payload)?;
        fs::write(&self.path, json)
            .with_context(|| format!("unable to write {}", self.path.display()))?;
        Ok(())
    }
}

fn store_path() -> PathBuf {
    directories::ProjectDirs::from("dev", "hyrak", "sinew")
        .map(|dirs| dirs.data_dir().join("cursor-agent-conversations.json"))
        .unwrap_or_else(|| PathBuf::from("cursor-agent-conversations.json"))
}

const MAX_CHECKPOINT_B64_CHARS: usize = 2 * 1024 * 1024;
const MAX_BLOB_B64_CHARS: usize = 256 * 1024;
const MAX_BLOBS_PER_CONVERSATION: usize = 24;

fn trim_checkpoint_payload(
    checkpoint_b64: String,
    blobs: HashMap<String, String>,
) -> (String, HashMap<String, String>) {
    let checkpoint = if checkpoint_b64.len() > MAX_CHECKPOINT_B64_CHARS {
        checkpoint_b64[..MAX_CHECKPOINT_B64_CHARS].to_string()
    } else {
        checkpoint_b64
    };
    let mut trimmed = HashMap::new();
    for (id, blob) in blobs.into_iter().take(MAX_BLOBS_PER_CONVERSATION) {
        if blob.len() > MAX_BLOB_B64_CHARS {
            trimmed.insert(id, blob[..MAX_BLOB_B64_CHARS].to_string());
        } else {
            trimmed.insert(id, blob);
        }
    }
    (checkpoint, trimmed)
}
