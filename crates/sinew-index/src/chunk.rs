use std::path::Path;

use regex::Regex;

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
const MIN_SYMBOL_LINES: usize = 4;
const MAX_SYMBOL_LINES: usize = 120;

pub fn chunk_file_content(content: &str, relative_path: &str) -> Vec<FileChunk> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }
    if let Some(chunks) = chunk_by_symbols(&lines, relative_path) {
        if !chunks.is_empty() {
            return chunks;
        }
    }
    chunk_by_lines(&lines)
}

fn chunk_by_symbols(lines: &[&str], relative_path: &str) -> Option<Vec<FileChunk>> {
    let ext = Path::new(relative_path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let pattern = symbol_pattern(&ext)?;
    let regex = Regex::new(pattern).ok()?;
    let mut starts = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if regex.is_match(line) {
            starts.push(index);
        }
    }
    if starts.is_empty() {
        return None;
    }
    let mut chunks = Vec::new();
    for (index, start) in starts.iter().enumerate() {
        let next = starts.get(index + 1).copied().unwrap_or(lines.len());
        let mut end = next;
        if end.saturating_sub(*start) > MAX_SYMBOL_LINES {
            end = (*start + MAX_SYMBOL_LINES).min(lines.len());
        }
        if end.saturating_sub(*start) < MIN_SYMBOL_LINES && index + 1 < starts.len() {
            continue;
        }
        push_chunk(&mut chunks, lines, *start, end);
    }
    if chunks.is_empty() { None } else { Some(chunks) }
}

fn symbol_pattern(ext: &str) -> Option<&'static str> {
    match ext {
        "rs" => Some(
            r"^\s*(pub(\([^)]*\))?\s+)?(async\s+)?(fn|struct|enum|trait|impl|mod|type|const|static|macro_rules!)\b",
        ),
        "ts" | "tsx" | "js" | "jsx" => Some(
            r"^\s*(export\s+)?(async\s+)?(function|class|interface|type|enum|const)\b",
        ),
        "py" => Some(r"^\s*(async\s+)?(def|class)\b"),
        "go" => Some(r"^\s*func(\s|\()"),
        "java" | "kt" => Some(r"^\s*(public|private|protected|internal|data|sealed|open|abstract|class|interface|enum|fun)\b"),
        "cs" => Some(r"^\s*(public|private|protected|internal|static|class|interface|struct|enum|record)\b"),
        _ => None,
    }
}

fn chunk_by_lines(lines: &[&str]) -> Vec<FileChunk> {
    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < lines.len() {
        let end = (start + CHUNK_LINES).min(lines.len());
        push_chunk(&mut chunks, lines, start, end);
        if end >= lines.len() {
            break;
        }
        start = end.saturating_sub(CHUNK_OVERLAP);
    }
    chunks
}

fn push_chunk(chunks: &mut Vec<FileChunk>, lines: &[&str], start: usize, end: usize) {
    let body = lines[start..end].join("\n");
    if body.trim().is_empty() {
        return;
    }
    let content = if body.len() > MAX_CHUNK_CHARS {
        body.chars().take(MAX_CHUNK_CHARS).collect::<String>()
    } else {
        body
    };
    chunks.push(FileChunk {
        start_line: (start + 1) as i64,
        end_line: end as i64,
        content,
        embedding: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_rust_symbols_when_present() {
        let content = "fn alpha() {}\n\npub fn beta() {\n  let x = 1;\n}\n\nstruct Gamma;\n";
        let chunks = chunk_file_content(content, "src/lib.rs");
        assert!(chunks.len() >= 2);
        assert!(chunks.iter().any(|chunk| chunk.content.contains("beta")));
    }

    #[test]
    fn chunks_overlap_for_long_files_without_symbols() {
        let content = (1..=200)
            .map(|line| format!("// line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = chunk_file_content(&content, "notes.txt");
        assert!(chunks.len() > 1);
    }
}
