use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use crate::chunk::FileChunk;

pub struct IndexStore {
    path: PathBuf,
}

impl IndexStore {
    pub fn open(workspace_root: &Path) -> Result<Self> {
        let path = index_db_path(workspace_root)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("unable to create index dir {}", parent.display()))?;
        }
        let store = Self { path };
        store.init_schema()?;
        Ok(store)
    }

    pub fn db_path(&self) -> &Path {
        &self.path
    }

    fn connection(&self) -> Result<Connection> {
        Connection::open(&self.path).context("unable to open codebase index database")
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.connection()?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS files (
                path TEXT PRIMARY KEY,
                content_hash TEXT NOT NULL,
                mtime_ms INTEGER NOT NULL,
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
            ",
        )?;
        Ok(())
    }

    pub fn file_hash(&self, path: &str) -> Result<Option<String>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare("SELECT content_hash FROM files WHERE path = ?1")?;
        let mut rows = stmt.query(params![path])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row.get(0)?));
        }
        Ok(None)
    }

    pub fn list_files(&self) -> Result<Vec<String>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare("SELECT path FROM files")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect::<Result<Vec<String>, _>>()
            .context("unable to list indexed files")
    }

    pub fn replace_file(
        &self,
        path: &str,
        content_hash: &str,
        mtime_ms: i64,
        chunks: &[FileChunk],
    ) -> Result<()> {
        let conn = self.connection()?;
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM chunks WHERE path = ?1", params![path])?;
        tx.execute("DELETE FROM files WHERE path = ?1", params![path])?;
        let now = now_ms();
        tx.execute(
            "INSERT INTO files (path, content_hash, mtime_ms, indexed_at_ms) VALUES (?1, ?2, ?3, ?4)",
            params![path, content_hash, mtime_ms, now],
        )?;
        for chunk in chunks {
            tx.execute(
                "INSERT INTO chunks (path, start_line, end_line, content) VALUES (?1, ?2, ?3, ?4)",
                params![path, chunk.start_line, chunk.end_line, chunk.content],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn remove_file(&self, path: &str) -> Result<()> {
        let conn = self.connection()?;
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM chunks WHERE path = ?1", params![path])?;
        tx.execute("DELETE FROM files WHERE path = ?1", params![path])?;
        tx.commit()?;
        Ok(())
    }

    pub fn stats(&self) -> Result<(usize, usize)> {
        let conn = self.connection()?;
        let files: i64 = conn.query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?;
        let chunks: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;
        Ok((files as usize, chunks as usize))
    }
}

pub fn index_db_path(workspace_root: &Path) -> Result<PathBuf> {
    let workspace_id = sha256_hex(workspace_root.display().to_string().as_bytes());
    let dirs = directories::ProjectDirs::from("dev", "hyrak", "sinew")
        .context("unable to resolve Sinew data directory")?;
    Ok(dirs
        .data_dir()
        .join("codebase-index")
        .join(workspace_id)
        .join("index.db"))
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
