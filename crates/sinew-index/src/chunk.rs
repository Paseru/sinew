#[derive(Debug, Clone)]
pub struct FileChunk {
    pub start_line: i64,
    pub end_line: i64,
    pub content: String,
    pub embedding: Option<Vec<f32>>,
}

const CHUNK_LINES: usize = 80;
const CHUNK_OVERLAP: usize = 12;
const MAX_CHUNK_CHARS: usize = 12_000;

pub fn chunk_file_content(content: &str) -> Vec<FileChunk> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }
    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < lines.len() {
        let end = (start + CHUNK_LINES).min(lines.len());
        let body = lines[start..end].join("\n");
        if body.len() > MAX_CHUNK_CHARS {
            let truncated = body.chars().take(MAX_CHUNK_CHARS).collect::<String>();
            chunks.push(FileChunk {
                start_line: (start + 1) as i64,
                end_line: end as i64,
                content: truncated,
                embedding: None,
            });
        } else if !body.trim().is_empty() {
            chunks.push(FileChunk {
                start_line: (start + 1) as i64,
                end_line: end as i64,
                content: body,
                embedding: None,
            });
        }
        if end >= lines.len() {
            break;
        }
        start = end.saturating_sub(CHUNK_OVERLAP);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_overlap_for_long_files() {
        let content = (1..=200)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = chunk_file_content(&content);
        assert!(chunks.len() > 1);
        assert!(chunks[0].start_line == 1);
    }
}
