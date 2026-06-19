use anyhow::Result;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use once_cell::sync::OnceCell;

static EMBEDDER: OnceCell<TextEmbedding> = OnceCell::new();

pub const DIMS: usize = 384;

fn get_embedder() -> Result<&'static TextEmbedding> {
    EMBEDDER.get_or_try_init(|| {
        TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2).with_show_download_progress(false),
        )
        .map_err(|e| anyhow::anyhow!("embedder init: {e}"))
    })
}

pub fn embed(texts: &[String]) -> Result<Vec<Vec<f32>>> {
    let model = get_embedder()?;
    let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
    model
        .embed(refs, None)
        .map_err(|e| anyhow::anyhow!("embed: {e}"))
}

pub fn embed_one(text: &str) -> Result<Vec<f32>> {
    let results = embed(&[text.to_string()])?;
    results.into_iter().next().ok_or_else(|| anyhow::anyhow!("embed returned empty"))
}

pub fn to_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub fn from_bytes(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}
