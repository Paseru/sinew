use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    time::SystemTime,
};

use anyhow::Result;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::{
    chunk::chunk_file_content,
    store::{FileSignature, IndexFileData, IndexFileMetadata, IndexStore},
    SKIP_DIRS, TEXT_EXTENSIONS,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexStats {
    pub files_indexed: usize,
    pub chunks_indexed: usize,
    pub files_updated: usize,
    pub embeddings_backfilled: usize,
}

#[derive(Debug, Clone)]
struct FileCandidate {
    path: PathBuf,
    relative: String,
    mtime_ms: i64,
    size_bytes: i64,
}

#[derive(Debug)]
enum PreparedIndexChange {
    Unchanged,
    Touch(IndexFileMetadata),
    Replace(IndexFileData),
    Remove(String),
}

pub fn ensure_workspace_index(workspace_root: &Path) -> Result<IndexStats> {
    let store = IndexStore::open(workspace_root)?;
    let mut stats = IndexStats::default();
    let signatures = store.file_signatures()?;
    let candidates = collect_workspace_candidates(workspace_root, workspace_root, None);
    let seen = candidates
        .iter()
        .map(|candidate| candidate.relative.clone())
        .collect::<HashSet<_>>();

    let changes = prepare_index_changes(&candidates, &signatures)?;
    apply_prepared_changes(&store, changes, &mut stats)?;

    let stale_paths = store
        .list_files()?
        .into_iter()
        .filter(|path| !seen.contains(path))
        .collect::<Vec<_>>();
    store.remove_files(&stale_paths)?;

    let (files, chunks) = store.stats()?;
    stats.files_indexed = files;
    stats.chunks_indexed = chunks;
    stats.embeddings_backfilled = backfill_missing_embeddings(&store)?;
    Ok(stats)
}

pub fn sync_changed_paths(
    workspace_root: &Path,
    paths: impl IntoIterator<Item = PathBuf>,
) -> Result<IndexStats> {
    const MAX_DIRECTORY_FILES_PER_EVENT: usize = 256;

    let store = IndexStore::open(workspace_root)?;
    let mut stats = IndexStats::default();
    let mut unique = HashSet::<String>::new();
    let mut candidates = Vec::<FileCandidate>::new();
    let mut removals = HashSet::<String>::new();

    for path in paths {
        let path = normalize_absolute_path(&path);
        if !is_under_workspace(workspace_root, &path) || should_skip_entry(&path, workspace_root) {
            continue;
        }

        if path.is_dir() {
            for candidate in collect_workspace_candidates(
                workspace_root,
                &path,
                Some(MAX_DIRECTORY_FILES_PER_EVENT),
            ) {
                if unique.insert(candidate.relative.clone()) {
                    candidates.push(candidate);
                }
            }
            continue;
        }

        let relative = normalize_relative_path(workspace_root, &path);
        if !unique.insert(relative.clone()) {
            continue;
        }

        if path.exists() && is_text_candidate(&path) {
            if let Some(candidate) = candidate_from_path(workspace_root, &path) {
                candidates.push(candidate);
            }
        } else {
            removals.insert(relative);
        }
    }

    let signatures = store.file_signatures()?;
    let changes = prepare_index_changes(&candidates, &signatures)?;
    apply_prepared_changes(&store, changes, &mut stats)?;

    let removals = removals.into_iter().collect::<Vec<_>>();
    stats.files_updated += store.remove_files(&removals)?;

    let (files, chunks) = store.stats()?;
    stats.files_indexed = files;
    stats.chunks_indexed = chunks;
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

fn collect_workspace_candidates(
    workspace_root: &Path,
    scan_root: &Path,
    max_files: Option<usize>,
) -> Vec<FileCandidate> {
    let mut candidates = Vec::new();

    for entry in WalkDir::new(scan_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !should_skip_entry(entry.path(), workspace_root))
    {
        if max_files.is_some_and(|max| candidates.len() >= max) {
            break;
        }

        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() || !is_text_candidate(entry.path()) {
            continue;
        }
        if let Some(candidate) = candidate_from_path(workspace_root, entry.path()) {
            candidates.push(candidate);
        }
    }

    candidates
}

fn candidate_from_path(workspace_root: &Path, path: &Path) -> Option<FileCandidate> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let mtime_ms = metadata
        .modified()
        .ok()
        .and_then(system_time_to_ms)
        .unwrap_or(0);
    Some(FileCandidate {
        path: path.to_path_buf(),
        relative: normalize_relative_path(workspace_root, path),
        mtime_ms,
        size_bytes: metadata.len().min(i64::MAX as u64) as i64,
    })
}

fn prepare_index_changes(
    candidates: &[FileCandidate],
    signatures: &HashMap<String, FileSignature>,
) -> Result<Vec<PreparedIndexChange>> {
    let prepared = candidates
        .par_iter()
        .map(|candidate| prepare_index_change(candidate, signatures.get(&candidate.relative)))
        .collect::<Vec<_>>();
    prepared.into_iter().collect()
}

fn prepare_index_change(
    candidate: &FileCandidate,
    existing: Option<&FileSignature>,
) -> Result<PreparedIndexChange> {
    if existing
        .map(|signature| {
            signature.mtime_ms == candidate.mtime_ms && signature.size_bytes == candidate.size_bytes
        })
        .unwrap_or(false)
    {
        return Ok(PreparedIndexChange::Unchanged);
    }

    if candidate.size_bytes > MAX_TEXT_FILE_BYTES as i64 {
        return Ok(PreparedIndexChange::Remove(candidate.relative.clone()));
    }

    let content = match read_text_file_limited(&candidate.path) {
        Ok(content) => content,
        Err(_) => return Ok(PreparedIndexChange::Remove(candidate.relative.clone())),
    };
    let hash = sha256_hex(content.as_bytes());

    if existing
        .map(|signature| signature.content_hash.as_str() == hash.as_str())
        .unwrap_or(false)
    {
        return Ok(PreparedIndexChange::Touch(IndexFileMetadata {
            path: candidate.relative.clone(),
            content_hash: hash,
            mtime_ms: candidate.mtime_ms,
            size_bytes: candidate.size_bytes,
        }));
    }

    Ok(PreparedIndexChange::Replace(IndexFileData {
        path: candidate.relative.clone(),
        content_hash: hash,
        mtime_ms: candidate.mtime_ms,
        size_bytes: candidate.size_bytes,
        chunks: chunk_file_content(&content, &candidate.relative),
    }))
}

fn apply_prepared_changes(
    store: &IndexStore,
    changes: Vec<PreparedIndexChange>,
    stats: &mut IndexStats,
) -> Result<()> {
    let mut replacements = Vec::new();
    let mut metadata_updates = Vec::new();
    let mut removals = Vec::new();

    for change in changes {
        match change {
            PreparedIndexChange::Unchanged => {}
            PreparedIndexChange::Touch(metadata) => metadata_updates.push(metadata),
            PreparedIndexChange::Replace(file) => replacements.push(file),
            PreparedIndexChange::Remove(path) => removals.push(path),
        }
    }

    store.replace_files(&replacements)?;
    stats.files_updated += replacements.len();
    store.touch_file_metadata_batch(&metadata_updates)?;
    stats.files_updated += store.remove_files(&removals)?;
    Ok(())
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

fn normalize_absolute_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

fn is_under_workspace(workspace_root: &Path, path: &Path) -> bool {
    let root = normalize_absolute_path(workspace_root);
    path.starts_with(&root) || path.starts_with(workspace_root)
}

fn is_text_candidate(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    TEXT_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
}

const MAX_TEXT_FILE_BYTES: u64 = 512 * 1024;

fn read_text_file_limited(path: &Path) -> Result<String> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > MAX_TEXT_FILE_BYTES {
        anyhow::bail!("file too large");
    }
    let mut file = fs::File::open(path)?;
    let mut buffer = Vec::with_capacity(metadata.len().min(MAX_TEXT_FILE_BYTES) as usize);
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

const EMBEDDING_BACKFILL_BATCH: usize = 64;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_changed_paths_updates_and_removes_one_file() {
        let dir = std::env::temp_dir().join(format!("sinew-index-sync-test-{}", unique_id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("alpha.rs");
        fs::write(&file, "pub fn first_name() {}\n").unwrap();

        let stats = sync_changed_paths(&dir, vec![file.clone()]).unwrap();
        assert_eq!(stats.files_indexed, 1);
        assert_eq!(stats.files_updated, 1);

        fs::write(&file, "pub fn second_name() {}\n").unwrap();
        let stats = sync_changed_paths(&dir, vec![file.clone()]).unwrap();
        assert_eq!(stats.files_indexed, 1);
        assert_eq!(stats.files_updated, 1);

        fs::remove_file(&file).unwrap();
        let stats = sync_changed_paths(&dir, vec![file]).unwrap();
        assert_eq!(stats.files_indexed, 0);
        assert_eq!(stats.files_updated, 1);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn unchanged_files_are_skipped_after_initial_index() {
        let dir = std::env::temp_dir().join(format!("sinew-index-skip-test-{}", unique_id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("alpha.rs"), "pub fn stable_name() {}\n").unwrap();

        let first = ensure_workspace_index(&dir).unwrap();
        assert_eq!(first.files_updated, 1);

        let second = ensure_workspace_index(&dir).unwrap();
        assert_eq!(second.files_indexed, 1);
        assert_eq!(second.files_updated, 0);

        let _ = fs::remove_dir_all(dir);
    }

    fn unique_id() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }
}
