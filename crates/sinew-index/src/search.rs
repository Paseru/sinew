use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{embeddings, store::IndexStore};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodebaseHit {
    pub path: String,
    pub start_line: i64,
    pub end_line: i64,
    pub snippet: String,
    pub score: f64,
}

struct RawHit {
    path: String,
    start_line: i64,
    end_line: i64,
    snippet: String,
    bm25: f64,
    embedding: Option<Vec<f32>>,
}

pub fn search_workspace(
    workspace_root: &Path,
    query: &str,
    path_prefix: Option<&str>,
    limit: usize,
) -> Result<Vec<CodebaseHit>> {
    let start = Instant::now();
    let store = IndexStore::open(workspace_root)?;
    let fts_query = build_fts_query(query);
    if fts_query.is_empty() {
        return Ok(Vec::new());
    }
    let limit = limit.clamp(1, 50);
    let candidate_limit = (limit * 4).max(8) as i64;
    let mut hits = search_fts_candidates(&store, &fts_query, path_prefix, candidate_limit)?;
    if hits.is_empty() {
        return Ok(Vec::new());
    }
    if embeddings::is_available() {
        if let Ok(query_embedding) = query_embedding(&store, query) {
            hits = rerank_with_embeddings(hits, &query_embedding);
        }
    }
    hits.truncate(limit);
    let hit_count = hits.len();
    let search_ms = start.elapsed().as_millis();
    tracing::debug!(query, hit_count, search_ms, "workspace search completed");
    Ok(hits
        .into_iter()
        .map(|hit| CodebaseHit {
            path: hit.path,
            start_line: hit.start_line,
            end_line: hit.end_line,
            snippet: hit.snippet,
            score: hit.bm25,
        })
        .collect())
}

fn search_fts_candidates(
    store: &IndexStore,
    fts_query: &str,
    path_prefix: Option<&str>,
    limit: i64,
) -> Result<Vec<RawHit>> {
    let conn = store.connection()?;
    let sql = if path_prefix
        .filter(|value| !value.trim().is_empty())
        .is_some()
    {
        search_sql_with_prefix()
    } else {
        search_sql()
    };
    let mut stmt = conn.prepare(&sql)?;
    let rows = if let Some(prefix) = path_prefix.filter(|value| !value.trim().is_empty()) {
        stmt.query_map(params![fts_query, format!("{prefix}%"), limit], map_raw_hit)?
    } else {
        stmt.query_map(params![fts_query, limit], map_raw_hit)?
    };
    rows.collect::<Result<Vec<_>, _>>()
        .context("unable to search codebase index")
}

fn query_embedding(store: &IndexStore, query: &str) -> Result<Vec<f32>> {
    let hash = query_hash(query);
    if let Some(cached) = store.load_query_embedding(&hash)? {
        return Ok(cached);
    }
    let embedding = embeddings::embed_query(query)?;
    store.save_query_embedding(&hash, &embedding)?;
    Ok(embedding)
}

fn rerank_with_embeddings(mut hits: Vec<RawHit>, query_embedding: &[f32]) -> Vec<RawHit> {
    for hit in &mut hits {
        let fts_score = bm25_to_score(hit.bm25);
        let semantic = hit
            .embedding
            .as_deref()
            .map(|embedding| embeddings::cosine_similarity(query_embedding, embedding) as f64)
            .unwrap_or(0.0);
        hit.bm25 = 0.35 * fts_score + 0.65 * semantic;
    }
    hits.sort_by(|left, right| {
        right
            .bm25
            .partial_cmp(&left.bm25)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits
}

fn bm25_to_score(value: f64) -> f64 {
    1.0 / (1.0 + value.abs())
}

fn query_hash(query: &str) -> String {
    let digest = Sha256::digest(query.trim().to_ascii_lowercase().as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn search_sql() -> String {
    "
    SELECT c.path, c.start_line, c.end_line, snippet(chunks_fts, 1, '[[', ']]', '…', 10) AS snippet,
           bm25(chunks_fts) AS score, c.embedding
    FROM chunks_fts
    JOIN chunks c ON c.id = chunks_fts.rowid
    WHERE chunks_fts MATCH ?1
    ORDER BY score
    LIMIT ?2
    "
    .to_string()
}

fn search_sql_with_prefix() -> String {
    "
    SELECT c.path, c.start_line, c.end_line, snippet(chunks_fts, 1, '[[', ']]', '…', 10) AS snippet,
           bm25(chunks_fts) AS score, c.embedding
    FROM chunks_fts
    JOIN chunks c ON c.id = chunks_fts.rowid
    WHERE chunks_fts MATCH ?1 AND c.path LIKE ?2
    ORDER BY score
    LIMIT ?3
    "
    .to_string()
}

fn map_raw_hit(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawHit> {
    let embedding = row
        .get::<_, Option<Vec<u8>>>(5)?
        .map(|bytes| embeddings::bytes_to_vector(&bytes));
    Ok(RawHit {
        path: row.get(0)?,
        start_line: row.get(1)?,
        end_line: row.get(2)?,
        snippet: row.get(3)?,
        bm25: row.get(4)?,
        embedding,
    })
}

fn build_fts_query(query: &str) -> String {
    query
        .split_whitespace()
        .filter(|term| !term.is_empty())
        .map(|term| {
            let cleaned: String = term
                .chars()
                .filter(|ch| ch.is_alphanumeric() || *ch == '_' || *ch == '-')
                .collect();
            if cleaned.is_empty() {
                String::new()
            } else {
                format!("{}*", cleaned.replace('"', ""))
            }
        })
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>()
        .join(" OR ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use crate::indexer::ensure_workspace_index;

    #[test]
    fn indexes_and_searches_workspace() {
        let dir = std::env::temp_dir().join(format!("sinew-index-test-{}", uuid_simple()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("alpha.rs"), "pub fn authenticate_user() {}\n").unwrap();
        ensure_workspace_index(&dir).unwrap();
        let hits = search_workspace(&dir, "authenticate", None, 5).unwrap();
        assert!(!hits.is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    fn uuid_simple() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }
}
