use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ignore::WalkBuilder;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

use crate::{
    chunker::chunk_file,
    embedder::{embed, from_bytes, to_bytes},
};

const INDEXABLE_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "mjs", "py", "go",
    "md", "toml", "json", "css", "scss", "html",
];
const BATCH_SIZE: usize = 64;

pub struct SearchIndex {
    db_path: PathBuf,
}

impl SearchIndex {
    pub fn new(workspace_root: &Path) -> Result<Self> {
        let dir = workspace_root.join(".sinew").join("search");
        std::fs::create_dir_all(&dir).context("create .sinew/search")?;
        let db_path = dir.join("index.db");
        let idx = Self { db_path };
        idx.init_schema()?;
        Ok(idx)
    }

    fn open(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        Ok(conn)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.open()?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS file_hashes (
                file_path TEXT PRIMARY KEY,
                hash TEXT NOT NULL,
                indexed_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY,
                file_path TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                language TEXT NOT NULL,
                content TEXT NOT NULL,
                embedding BLOB NOT NULL
            );
            CREATE INDEX IF NOT EXISTS chunks_file ON chunks(file_path);
            CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
                content,
                file_path UNINDEXED,
                start_line UNINDEXED,
                chunk_id UNINDEXED,
                tokenize='porter ascii'
            );
            ",
        )?;
        Ok(())
    }

    pub fn index_workspace(&self, workspace_root: &Path, force: bool) -> Result<IndexStats> {
        let mut stats = IndexStats::default();
        let conn = self.open()?;

        let files: Vec<PathBuf> = WalkBuilder::new(workspace_root)
            .hidden(false)
            .ignore(true)
            .git_ignore(true)
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().map(|t| t.is_file()).unwrap_or(false)
                    && e.path()
                        .extension()
                        .and_then(|x| x.to_str())
                        .map(|ext| INDEXABLE_EXTENSIONS.contains(&ext))
                        .unwrap_or(false)
                    && !e.path().starts_with(workspace_root.join(".sinew"))
                    && !e.path().starts_with(workspace_root.join("target"))
                    && !e.path().starts_with(workspace_root.join("node_modules"))
            })
            .map(|e| e.into_path())
            .collect();

        info!("indexing {} files", files.len());

        for file in &files {
            let path_str = file.to_string_lossy().to_string();
            let source = match std::fs::read_to_string(file) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let hash = {
                let mut h = Sha256::new();
                h.update(source.as_bytes());
                format!("{:x}", h.finalize())
            };

            if !force {
                let cached: Option<String> = conn
                    .query_row(
                        "SELECT hash FROM file_hashes WHERE file_path = ?1",
                        params![path_str],
                        |r| r.get(0),
                    )
                    .ok();
                if cached.as_deref() == Some(&hash) {
                    stats.skipped += 1;
                    continue;
                }
            }

            let chunks = chunk_file(file, &source);
            if chunks.is_empty() {
                continue;
            }

            // Delete old data for this file
            conn.execute("DELETE FROM chunks WHERE file_path = ?1", params![path_str])?;
            conn.execute("DELETE FROM chunks_fts WHERE file_path = ?1", params![path_str])?;

            // Embed in batches
            let texts: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();
            for batch_start in (0..texts.len()).step_by(BATCH_SIZE) {
                let batch_end = (batch_start + BATCH_SIZE).min(texts.len());
                let batch_texts = texts[batch_start..batch_end].to_vec();
                let embeddings = match embed(&batch_texts) {
                    Ok(e) => e,
                    Err(e) => {
                        warn!("embed error for {}: {}", path_str, e);
                        continue;
                    }
                };

                for (i, embedding) in embeddings.iter().enumerate() {
                    let chunk = &chunks[batch_start + i];
                    let embedding_blob = to_bytes(embedding);
                    conn.execute(
                        "INSERT INTO chunks (file_path, start_line, end_line, language, content, embedding)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            path_str,
                            chunk.start_line,
                            chunk.end_line,
                            chunk.language,
                            chunk.content,
                            embedding_blob,
                        ],
                    )?;
                    let id: i64 = conn.last_insert_rowid();
                    conn.execute(
                        "INSERT INTO chunks_fts (content, file_path, start_line, chunk_id)
                         VALUES (?1, ?2, ?3, ?4)",
                        params![chunk.content, path_str, chunk.start_line, id],
                    )?;
                }
            }

            conn.execute(
                "INSERT OR REPLACE INTO file_hashes (file_path, hash, indexed_at)
                 VALUES (?1, ?2, strftime('%s','now'))",
                params![path_str, hash],
            )?;
            stats.indexed += 1;
            debug!("indexed {}", path_str);
        }

        stats.total_chunks = conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get::<_, i64>(0))
            .unwrap_or(0) as usize;

        info!("index complete: {} indexed, {} skipped, {} total chunks", stats.indexed, stats.skipped, stats.total_chunks);
        Ok(stats)
    }

    pub fn load_all_embeddings(&self) -> Result<Vec<(i64, String, u32, u32, String, Vec<f32>)>> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT id, file_path, start_line, end_line, content, embedding FROM chunks",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, u32>(2)?,
                r.get::<_, u32>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Vec<u8>>(5)?,
            ))
        })?;

        let mut result = Vec::new();
        for row in rows {
            let (id, fp, sl, el, content, blob) = row?;
            result.push((id, fp, sl, el, content, from_bytes(&blob)));
        }
        Ok(result)
    }

    pub fn fts_search(&self, query: &str, limit: usize) -> Result<Vec<(i64, f64)>> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT chunk_id, bm25(chunks_fts) FROM chunks_fts
             WHERE chunks_fts MATCH ?1
             ORDER BY bm25(chunks_fts)
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![query, limit as i64], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?))
        })?;
        let mut result = Vec::new();
        for row in rows.flatten() {
            result.push(row);
        }
        Ok(result)
    }

    pub fn total_chunks(&self) -> usize {
        self.open()
            .ok()
            .and_then(|c| c.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get::<_, i64>(0)).ok())
            .unwrap_or(0) as usize
    }
}

#[derive(Debug, Default)]
pub struct IndexStats {
    pub indexed: usize,
    pub skipped: usize,
    pub total_chunks: usize,
}
