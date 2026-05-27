use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::params;

use crate::store::IndexStore;

#[derive(Debug, Clone)]
pub struct CodebaseHit {
    pub path: String,
    pub start_line: i64,
    pub end_line: i64,
    pub snippet: String,
    pub score: f64,
}

pub fn search_workspace(
    workspace_root: &Path,
    query: &str,
    path_prefix: Option<&str>,
    limit: usize,
) -> Result<Vec<CodebaseHit>> {
    let store = IndexStore::open(workspace_root)?;
    let fts_query = build_fts_query(query);
    if fts_query.is_empty() {
        return Ok(Vec::new());
    }
    let conn = rusqlite::Connection::open(store.db_path())?;
    let sql = if let Some(prefix) = path_prefix.filter(|value| !value.trim().is_empty()) {
        search_sql_with_prefix(prefix)
    } else {
        search_sql()
    };
    let mut stmt = conn.prepare(&sql)?;
    let limit = limit.max(1).min(50) as i64;
    let rows = if let Some(prefix) = path_prefix.filter(|value| !value.trim().is_empty()) {
        stmt.query_map(params![fts_query, format!("{prefix}%"), limit], map_hit)?
    } else {
        stmt.query_map(params![fts_query, limit], map_hit)?
    };
    rows.collect::<Result<Vec<_>, _>>()
        .context("unable to search codebase index")
}

fn search_sql() -> String {
    "
    SELECT c.path, c.start_line, c.end_line, snippet(chunks_fts, 1, '[[', ']]', '…', 10) AS snippet,
           bm25(chunks_fts) AS score
    FROM chunks_fts
    JOIN chunks c ON c.id = chunks_fts.rowid
    WHERE chunks_fts MATCH ?1
    ORDER BY score
    LIMIT ?2
    "
    .to_string()
}

fn search_sql_with_prefix(prefix: &str) -> String {
    let _ = prefix;
    "
    SELECT c.path, c.start_line, c.end_line, snippet(chunks_fts, 1, '[[', ']]', '…', 10) AS snippet,
           bm25(chunks_fts) AS score
    FROM chunks_fts
    JOIN chunks c ON c.id = chunks_fts.rowid
    WHERE chunks_fts MATCH ?1 AND c.path LIKE ?2
    ORDER BY score
    LIMIT ?3
    "
    .to_string()
}

fn map_hit(row: &rusqlite::Row<'_>) -> rusqlite::Result<CodebaseHit> {
    Ok(CodebaseHit {
        path: row.get(0)?,
        start_line: row.get(1)?,
        end_line: row.get(2)?,
        snippet: row.get(3)?,
        score: row.get(4)?,
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
