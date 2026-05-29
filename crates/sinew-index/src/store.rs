use std::{
    collections::HashMap,
    ffi::OsString,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use crate::chunk::FileChunk;

#[derive(Debug, Clone)]
pub struct FileSignature {
    pub content_hash: String,
    pub mtime_ms: i64,
    pub size_bytes: i64,
}

#[derive(Debug, Clone)]
pub struct IndexFileMetadata {
    pub path: String,
    pub content_hash: String,
    pub mtime_ms: i64,
    pub size_bytes: i64,
}

#[derive(Debug, Clone)]
pub struct IndexFileData {
    pub path: String,
    pub content_hash: String,
    pub mtime_ms: i64,
    pub size_bytes: i64,
    pub chunks: Vec<FileChunk>,
}

pub struct IndexStore {
    path: PathBuf,
}

impl IndexStore {
    pub fn open(workspace_root: &Path) -> Result<Self> {
        let path = index_db_path(workspace_root)?;
        migrate_legacy_index_db(workspace_root, &path)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("unable to create index dir {}", parent.display()))?;
        }
        let store = Self { path };
        store.init_schema()?;
        Ok(store)
    }

    pub(crate) fn connection(&self) -> Result<Connection> {
        let conn =
            Connection::open(&self.path).context("unable to open codebase index database")?;
        let _ = conn.busy_timeout(Duration::from_secs(10));
        let tuning = sqlite_tuning();
        let pragmas = format!(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -{};
             PRAGMA mmap_size = {};
             PRAGMA temp_store = MEMORY;
             PRAGMA cache_spill = FALSE;
             PRAGMA busy_timeout = 10000;",
            tuning.cache_kib, tuning.mmap_bytes
        );
        let _ = conn.execute_batch(&pragmas);
        Ok(conn)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.connection()?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS files (
                path TEXT PRIMARY KEY,
                content_hash TEXT NOT NULL,
                mtime_ms INTEGER NOT NULL,
                size_bytes INTEGER NOT NULL DEFAULT 0,
                indexed_at_ms INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                content TEXT NOT NULL
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
                path,
                content,
                content='chunks',
                content_rowid='id',
                tokenize='unicode61'
            );
            CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
                INSERT INTO chunks_fts(rowid, path, content) VALUES (new.id, new.path, new.content);
            END;
            CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
                INSERT INTO chunks_fts(chunks_fts, rowid, path, content) VALUES('delete', old.id, old.path, old.content);
            END;
            CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE ON chunks BEGIN
                INSERT INTO chunks_fts(chunks_fts, rowid, path, content) VALUES('delete', old.id, old.path, old.content);
                INSERT INTO chunks_fts(rowid, path, content) VALUES (new.id, new.path, new.content);
            END;
            CREATE TABLE IF NOT EXISTS query_cache (
                query_hash TEXT PRIMARY KEY,
                embedding BLOB NOT NULL,
                cached_at_ms INTEGER NOT NULL
            );
            ",
        )?;
        let _ = conn.execute(
            "ALTER TABLE files ADD COLUMN size_bytes INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = conn.execute("ALTER TABLE chunks ADD COLUMN embedding BLOB", []);
        Ok(())
    }

    pub fn file_signatures(&self) -> Result<HashMap<String, FileSignature>> {
        let conn = self.connection()?;
        let mut stmt =
            conn.prepare("SELECT path, content_hash, mtime_ms, size_bytes FROM files")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                FileSignature {
                    content_hash: row.get(1)?,
                    mtime_ms: row.get(2)?,
                    size_bytes: row.get(3)?,
                },
            ))
        })?;

        let mut signatures = HashMap::new();
        for row in rows {
            let (path, signature) = row?;
            signatures.insert(path, signature);
        }
        Ok(signatures)
    }

    pub fn list_files(&self) -> Result<Vec<String>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare("SELECT path FROM files")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect::<Result<Vec<String>, _>>()
            .context("unable to list indexed files")
    }

    pub fn replace_files(&self, files: &[IndexFileData]) -> Result<()> {
        if files.is_empty() {
            return Ok(());
        }

        let conn = self.connection()?;
        let tx = conn.unchecked_transaction()?;
        let now = now_ms();

        {
            let mut delete_chunks = tx.prepare("DELETE FROM chunks WHERE path = ?1")?;
            let mut delete_file = tx.prepare("DELETE FROM files WHERE path = ?1")?;
            let mut insert_file = tx.prepare(
                "INSERT INTO files (path, content_hash, mtime_ms, size_bytes, indexed_at_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            let mut insert_chunk = tx.prepare(
                "INSERT INTO chunks (path, start_line, end_line, content, embedding) VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;

            for file in files {
                delete_chunks.execute(params![file.path.as_str()])?;
                delete_file.execute(params![file.path.as_str()])?;
                insert_file.execute(params![
                    file.path.as_str(),
                    file.content_hash.as_str(),
                    file.mtime_ms,
                    file.size_bytes,
                    now
                ])?;
                for chunk in &file.chunks {
                    let embedding = chunk
                        .embedding
                        .as_ref()
                        .map(|values| crate::embeddings::vector_to_bytes(values));
                    insert_chunk.execute(params![
                        file.path.as_str(),
                        chunk.start_line,
                        chunk.end_line,
                        chunk.content.as_str(),
                        embedding
                    ])?;
                }
            }
        }

        tx.commit()?;
        Ok(())
    }

    pub fn touch_file_metadata_batch(&self, files: &[IndexFileMetadata]) -> Result<()> {
        if files.is_empty() {
            return Ok(());
        }

        let conn = self.connection()?;
        let tx = conn.unchecked_transaction()?;
        let now = now_ms();
        {
            let mut update = tx.prepare(
                "UPDATE files SET content_hash = ?2, mtime_ms = ?3, size_bytes = ?4, indexed_at_ms = ?5 WHERE path = ?1",
            )?;
            for file in files {
                update.execute(params![
                    file.path.as_str(),
                    file.content_hash.as_str(),
                    file.mtime_ms,
                    file.size_bytes,
                    now
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn remove_files(&self, paths: &[String]) -> Result<usize> {
        if paths.is_empty() {
            return Ok(0);
        }

        let conn = self.connection()?;
        let tx = conn.unchecked_transaction()?;
        let mut removed = 0usize;
        {
            let mut delete_chunks = tx.prepare("DELETE FROM chunks WHERE path = ?1")?;
            let mut delete_file = tx.prepare("DELETE FROM files WHERE path = ?1")?;
            for path in paths {
                delete_chunks.execute(params![path.as_str()])?;
                removed += delete_file.execute(params![path.as_str()])?;
            }
        }
        tx.commit()?;
        Ok(removed)
    }

    pub fn update_chunk_embedding(&self, chunk_id: i64, embedding: &[f32]) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE chunks SET embedding = ?1 WHERE id = ?2",
            params![crate::embeddings::vector_to_bytes(embedding), chunk_id],
        )?;
        Ok(())
    }

    pub fn list_chunks_without_embedding(&self, limit: usize) -> Result<Vec<(i64, String)>> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, content FROM chunks WHERE embedding IS NULL OR length(embedding) = 0 ORDER BY id LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>()
            .context("unable to list chunks missing embeddings")
    }

    pub fn stats(&self) -> Result<(usize, usize)> {
        let conn = self.connection()?;
        let files: i64 = conn.query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?;
        let chunks: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;
        Ok((files as usize, chunks as usize))
    }

    pub fn load_query_embedding(&self, query_hash: &str) -> Result<Option<Vec<f32>>> {
        let conn = self.connection()?;
        let mut stmt =
            conn.prepare("SELECT embedding FROM query_cache WHERE query_hash = ?1 LIMIT 1")?;
        let mut rows = stmt.query(params![query_hash])?;
        if let Some(row) = rows.next()? {
            let bytes: Vec<u8> = row.get(0)?;
            return Ok(Some(crate::embeddings::bytes_to_vector(&bytes)));
        }
        Ok(None)
    }

    pub fn save_query_embedding(&self, query_hash: &str, embedding: &[f32]) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO query_cache (query_hash, embedding, cached_at_ms) VALUES (?1, ?2, ?3)
             ON CONFLICT(query_hash) DO UPDATE SET embedding = excluded.embedding, cached_at_ms = excluded.cached_at_ms",
            params![
                query_hash,
                crate::embeddings::vector_to_bytes(embedding),
                now_ms()
            ],
        )?;
        Ok(())
    }
}

pub fn index_db_path(workspace_root: &Path) -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("dev", "hyrak", "sinew")
        .context("unable to resolve Sinew data directory")?;
    Ok(index_db_path_under(dirs.data_local_dir(), workspace_root))
}

fn legacy_index_db_path(workspace_root: &Path) -> Result<Option<PathBuf>> {
    let dirs = directories::ProjectDirs::from("dev", "hyrak", "sinew")
        .context("unable to resolve Sinew data directory")?;
    if dirs.data_dir() == dirs.data_local_dir() {
        return Ok(None);
    }
    Ok(Some(index_db_path_under(dirs.data_dir(), workspace_root)))
}

fn index_db_path_under(base: &Path, workspace_root: &Path) -> PathBuf {
    let workspace_id = sha256_hex(workspace_root.display().to_string().as_bytes());
    base.join("codebase-index")
        .join(workspace_id)
        .join("index.db")
}

fn migrate_legacy_index_db(workspace_root: &Path, local_path: &Path) -> Result<()> {
    let Some(legacy_path) = legacy_index_db_path(workspace_root)? else {
        return Ok(());
    };
    if !legacy_path.exists() || local_path.exists() {
        return Ok(());
    }
    if let Some(parent) = local_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("unable to create local index dir {}", parent.display()))?;
    }

    for suffix in ["", "-wal", "-shm"] {
        let source = sqlite_sibling_path(&legacy_path, suffix);
        if !source.exists() {
            continue;
        }
        let target = sqlite_sibling_path(local_path, suffix);
        if !target.exists() {
            let _ = std::fs::copy(source, target);
        }
    }
    Ok(())
}

fn sqlite_sibling_path(path: &Path, suffix: &str) -> PathBuf {
    if suffix.is_empty() {
        return path.to_path_buf();
    }
    let mut value = OsString::from(path.as_os_str());
    value.push(suffix);
    PathBuf::from(value)
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

#[derive(Debug, Clone, Copy)]
struct SqliteTuning {
    cache_kib: i64,
    mmap_bytes: i64,
}

fn sqlite_tuning() -> SqliteTuning {
    let profile = MachinePowerProfile::current();
    let cache_kib = (profile.parallelism * 8 * 1024).clamp(32 * 1024, 256 * 1024);
    let mmap_bytes = (cache_kib * 1024 * profile.storage_multiplier())
        .clamp(128 * 1024 * 1024, 1024 * 1024 * 1024);
    SqliteTuning {
        cache_kib,
        mmap_bytes,
    }
}

#[derive(Debug, Clone, Copy)]
struct MachinePowerProfile {
    parallelism: i64,
    high_throughput_storage: bool,
}

impl MachinePowerProfile {
    fn current() -> Self {
        Self {
            parallelism: thread::available_parallelism()
                .map(|value| value.get() as i64)
                .unwrap_or(4),
            high_throughput_storage: high_throughput_storage_available(),
        }
    }

    fn storage_multiplier(self) -> i64 {
        if self.high_throughput_storage {
            4
        } else {
            2
        }
    }
}

#[cfg(target_os = "windows")]
fn high_throughput_storage_available() -> bool {
    use std::os::windows::process::CommandExt;

    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_DiskDrive | Select-Object -ExpandProperty Model",
        ])
        .creation_flags(0x08000000)
        .output();
    let Ok(output) = output else {
        return true;
    };
    let text = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
    text.contains("nvme") || text.contains("ssd")
}

#[cfg(not(target_os = "windows"))]
fn high_throughput_storage_available() -> bool {
    true
}

