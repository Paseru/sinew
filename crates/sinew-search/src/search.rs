use std::collections::HashMap;

use anyhow::Result;

use crate::{
    embedder::{cosine, embed_one},
    index::SearchIndex,
};

pub struct SearchResult {
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub content: String,
    pub score: f32,
}

const RRF_K: usize = 60;

pub fn search(index: &SearchIndex, query: &str, max_results: usize) -> Result<Vec<SearchResult>> {
    let total = index.total_chunks();
    if total == 0 {
        return Ok(vec![]);
    }

    // 1. Embed query
    let query_vec = embed_one(query)?;

    // 2. Load all embeddings + vector ranking
    let all = index.load_all_embeddings()?;
    let mut vector_ranked: Vec<(i64, f32)> = all
        .iter()
        .map(|(id, _, _, _, _, emb)| (*id, cosine(&query_vec, emb)))
        .collect();
    vector_ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // 3. FTS BM25 ranking (sanitize query for FTS5)
    let fts_query = sanitize_fts_query(query);
    let fts_ranked = if !fts_query.is_empty() {
        index.fts_search(&fts_query, max_results * 4).unwrap_or_default()
    } else {
        vec![]
    };

    // 4. RRF fusion
    let mut rrf_scores: HashMap<i64, f32> = HashMap::new();
    for (rank, (id, _)) in vector_ranked.iter().enumerate() {
        *rrf_scores.entry(*id).or_default() += 1.0 / (RRF_K + rank) as f32;
    }
    for (rank, (id, _)) in fts_ranked.iter().enumerate() {
        *rrf_scores.entry(*id).or_default() += 1.0 / (RRF_K + rank) as f32;
    }

    // 5. Build result map from all chunks
    let chunk_map: HashMap<i64, _> = all
        .into_iter()
        .map(|(id, fp, sl, el, content, _)| (id, (fp, sl, el, content)))
        .collect();

    let mut ranked: Vec<(i64, f32)> = rrf_scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let results = ranked
        .into_iter()
        .take(max_results)
        .filter_map(|(id, score)| {
            let (fp, sl, el, content) = chunk_map.get(&id)?.clone();
            Some(SearchResult { file_path: fp, start_line: sl, end_line: el, content, score })
        })
        .collect();

    Ok(results)
}

fn sanitize_fts_query(q: &str) -> String {
    // FTS5 special chars: " ' * ( ) . - ^
    // Simple approach: extract words, join with AND
    let words: Vec<&str> = q
        .split_whitespace()
        .filter(|w| w.len() >= 2 && w.chars().all(|c| c.is_alphanumeric() || c == '_'))
        .collect();
    words.join(" AND ")
}
