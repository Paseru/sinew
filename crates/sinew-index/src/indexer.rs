use std::{
    fs,
    io::Read,
    path::{Component, Path},
    time::SystemTime,
};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::{
    chunk::chunk_file_content,
    store::IndexStore,
    SKIP_DIRS, TEXT_EXTENSIONS,
};

#[derive(Debug, Clone, Default)]
pub struct IndexStats {
    pub files_indexed: usize,
    pub chunks_indexed: usize,
    pub files_updated: usize,
    pub embeddings_backfilled: usize,
}

pub fn ensure_workspace_index(workspace_root: &Path) -> Result<IndexStats> {
    let store = IndexStore::open(workspace_root)?;
    let mut stats = IndexStats::default();
    let mut seen = std::collections::HashSet::<String>::new();

    for entry in WalkDir::new(workspace_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !should_skip_entry(entry.path(), workspace_root))
    {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !is_text_candidate(path) {
            continue;
        }
        let relative = normalize_relative_path(workspace_root, path);
        seen.insert(relative.clone());

        let metadata = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
        let mtime_ms = metadata
            .modified()
            .ok()
            .and_then(system_time_to_ms)
            .unwrap_or(0);
        let content = match read_text_file_limited(path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let hash = sha256_hex(content.as_bytes());
        if store.file_hash(&relative)?.as_deref() == Some(hash.as_str()) {
            continue;
        }
        let mut chunks = chunk_file_content(&content, &relative);
        if crate::embeddings::is_available() {
            let texts = chunks
                .iter()
                .map(|chunk| chunk.content.clone())
                .collect::<Vec<_>>();
            if let Ok(vectors) = crate::embeddings::embed_passages(&texts) {
                for (chunk, vector) in chunks.iter_mut().zip(vectors) {
                    chunk.embedding = Some(vector);
                }
            }
        }
        store.replace_file(&relative, &hash, mtime_ms, &chunks)?;
        stats.files_updated += 1;
    }

    for path in store.list_files()? {
        if !seen.contains(&path) {
            store.remove_file(&path)?;
        }
    }

    let (files, chunks) = store.stats()?;
    stats.files_indexed = files;
    stats.chunks_indexed = chunks;
    stats.embeddings_backfilled = backfill_missing_embeddings(&store)?;
    Ok(stats)
}

pub fn index_stats(workspace_root: &Path) -> Result<IndexStats> {
    let store = IndexStore::open(workspace_root)?;
    let (files, chunks) = store.stats()?;
    Ok(IndexStats {
        files_indexed: files,
        chunks_indexed: chunks,
        files_updated: 0,
        embeddings_backfilled: 0,
    })
}

fn should_skip_entry(path: &Path, workspace_root: &Path) -> bool {
    if path == workspace_root {
        return false;
    }
    path.components().any(|component| {
        matches!(component, Component::Normal(name) if SKIP_DIRS.contains(&name.to_string_lossy().as_ref()))
    })
}

fn normalize_relative_path(workspace_root: &Path, path: &Path) -> String {
    path.strip_prefix(workspace_root)
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

fn is_text_candidate(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    TEXT_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
}

fn read_text_file_limited(path: &Path) -> Result<String> {
    const MAX_BYTES: u64 = 512 * 1024;
    let metadata = fs::metadata(path)?;
    if metadata.len() > MAX_BYTES {
        anyhow::bail!("file too large");
    }
    let mut file = fs::File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    if buffer.iter().take(8192).any(|byte| *byte == 0) {
        anyhow::bail!("binary file");
    }
    Ok(String::from_utf8_lossy(&buffer).into_owned())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn system_time_to_ms(value: SystemTime) -> Option<i64> {
    value
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
}

const EMBEDDING_BACKFILL_BATCH: usize = 32;

fn backfill_missing_embeddings(store: &IndexStore) -> Result<usize> {
    if !crate::embeddings::is_available() {
        return Ok(0);
    }
    let pending = store.list_chunks_without_embedding()?;
    if pending.is_empty() {
        return Ok(0);
    }
    let mut updated = 0usize;
    for batch in pending.chunks(EMBEDDING_BACKFILL_BATCH) {
        let texts = batch
            .iter()
            .map(|(_, content)| content.clone())
            .collect::<Vec<_>>();
        let Ok(vectors) = crate::embeddings::embed_passages(&texts) else {
            break;
        };
        for ((chunk_id, _), vector) in batch.iter().zip(vectors) {
            store.update_chunk_embedding(*chunk_id, &vector)?;
            updated += 1;
        }
    }
    Ok(updated)
}
