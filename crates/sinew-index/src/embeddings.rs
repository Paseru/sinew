use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

static EMBEDDER: OnceLock<Mutex<Option<TextEmbedding>>> = OnceLock::new();

pub fn is_available() -> bool {
    if std::env::var_os("SINEW_INDEX_EMBEDDINGS").is_none() {
        return false;
    }
    embedder().is_ok()
}

pub fn embed_query(text: &str) -> Result<Vec<f32>> {
    let model = embedder()?;
    let mut guard = model
        .lock()
        .map_err(|_| anyhow::anyhow!("embedding model lock poisoned"))?;
    let embedder = guard.as_mut().context("embedding model unavailable")?;
    let prefixed = format!("query: {}", text.trim());
    let vectors = embedder
        .embed(vec![prefixed], None)
        .context("unable to embed query")?;
    vectors
        .into_iter()
        .next()
        .context("embedding model returned no vector")
}

pub fn embed_passages(texts: &[String]) -> Result<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let model = embedder()?;
    let mut guard = model
        .lock()
        .map_err(|_| anyhow::anyhow!("embedding model lock poisoned"))?;
    let embedder = guard.as_mut().context("embedding model unavailable")?;
    let prefixed = texts
        .iter()
        .map(|text| format!("passage: {}", text.trim()))
        .collect::<Vec<_>>();
    embedder
        .embed(prefixed, None)
        .context("unable to embed passages")
}

pub fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    if left.len() != right.len() || left.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut left_norm = 0.0f32;
    let mut right_norm = 0.0f32;
    for (a, b) in left.iter().zip(right.iter()) {
        dot += a * b;
        left_norm += a * a;
        right_norm += b * b;
    }
    if left_norm <= f32::EPSILON || right_norm <= f32::EPSILON {
        return 0.0;
    }
    dot / (left_norm.sqrt() * right_norm.sqrt())
}

pub fn vector_to_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

pub fn bytes_to_vector(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn embedder() -> Result<&'static Mutex<Option<TextEmbedding>>> {
    let slot = EMBEDDER.get_or_init(|| {
        let mut options = InitOptions::new(EmbeddingModel::BGESmallENV15);
        if let Some(proj_dirs) = directories::ProjectDirs::from("dev", "hyrak", "sinew") {
            let cache_dir = proj_dirs.cache_dir().join("fastembed_cache");
            let _ = std::fs::create_dir_all(&cache_dir);
            options = options.with_cache_dir(cache_dir);
        }
        let model = TextEmbedding::try_new(options).ok();
        Mutex::new(model)
    });
    if slot
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|_| ()))
        .is_none()
    {
        anyhow::bail!("local embedding model is unavailable");
    }
    Ok(slot)
}
