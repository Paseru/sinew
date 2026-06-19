use anyhow::Result;
use std::path::Path;
use tree_sitter::{Language, Parser, Query, QueryCursor};

pub struct Chunk {
    pub content: String,
    pub start_line: u32,
    pub end_line: u32,
    pub language: String,
}

const MAX_CHUNK_LINES: usize = 100;
const FALLBACK_CHUNK_LINES: usize = 50;
const FALLBACK_OVERLAP_LINES: usize = 5;

pub fn chunk_file(path: &Path, source: &str) -> Vec<Chunk> {
    let lang = detect_language(path);
    if let Some((ts_lang, lang_name, query_src)) = ts_config(&lang) {
        if let Ok(chunks) = ts_chunks(source, ts_lang, lang_name, query_src) {
            if !chunks.is_empty() {
                return chunks;
            }
        }
    }
    line_chunks(source, lang.as_deref().unwrap_or("text"))
}

fn detect_language(path: &Path) -> Option<String> {
    match path.extension()?.to_str()? {
        "rs" => Some("rust".into()),
        "ts" => Some("typescript".into()),
        "tsx" => Some("tsx".into()),
        "js" | "mjs" | "cjs" => Some("javascript".into()),
        "jsx" => Some("jsx".into()),
        "py" => Some("python".into()),
        "go" => Some("go".into()),
        "md" => Some("markdown".into()),
        "toml" => Some("toml".into()),
        "json" => Some("json".into()),
        "css" | "scss" => Some("css".into()),
        "html" => Some("html".into()),
        _ => None,
    }
}

fn ts_config(lang: &Option<String>) -> Option<(Language, &'static str, &'static str)> {
    match lang.as_deref()? {
        "rust" => Some((
            tree_sitter_rust::language(),
            "rust",
            "[(function_item)(impl_item)(struct_item)(enum_item)(trait_item)(type_item)(mod_item)] @chunk",
        )),
        "typescript" | "tsx" => Some((
            tree_sitter_typescript::language_typescript(),
            "typescript",
            "[(function_declaration)(class_declaration)(interface_declaration)(type_alias_declaration)(export_statement)] @chunk",
        )),
        "javascript" | "jsx" => Some((
            tree_sitter_javascript::language(),
            "javascript",
            "[(function_declaration)(class_declaration)(export_statement)(lexical_declaration)] @chunk",
        )),
        "python" => Some((
            tree_sitter_python::language(),
            "python",
            "[(function_definition)(class_definition)] @chunk",
        )),
        "go" => Some((
            tree_sitter_go::language(),
            "go",
            "[(function_declaration)(method_declaration)(type_declaration)] @chunk",
        )),
        _ => None,
    }
}

fn ts_chunks(source: &str, ts_lang: Language, lang_name: &str, query_src: &str) -> Result<Vec<Chunk>> {
    let mut parser = Parser::new();
    parser.set_language(&ts_lang)?;
    let tree = parser.parse(source, None).ok_or_else(|| anyhow::anyhow!("parse failed"))?;
    let root = tree.root_node();

    let query = Query::new(&ts_lang, query_src)?;
    let mut cursor = QueryCursor::new();
    let source_bytes = source.as_bytes();

    let mut chunks = Vec::new();
    for m in cursor.matches(&query, root, source_bytes) {
        for cap in m.captures {
            let node = cap.node;
            let start_line = node.start_position().row as u32;
            let end_line = node.end_position().row as u32;
            let line_count = (end_line - start_line + 1) as usize;

            let text = &source[node.byte_range()];
            if text.trim().is_empty() {
                continue;
            }

            if line_count <= MAX_CHUNK_LINES {
                chunks.push(Chunk {
                    content: text.to_string(),
                    start_line,
                    end_line,
                    language: lang_name.to_string(),
                });
            } else {
                // Split oversized nodes by lines
                let lines: Vec<&str> = text.lines().collect();
                let mut offset = 0usize;
                while offset < lines.len() {
                    let end = (offset + MAX_CHUNK_LINES).min(lines.len());
                    chunks.push(Chunk {
                        content: lines[offset..end].join("\n"),
                        start_line: start_line + offset as u32,
                        end_line: start_line + end as u32 - 1,
                        language: lang_name.to_string(),
                    });
                    offset += MAX_CHUNK_LINES - 5; // 5-line overlap
                }
            }
        }
    }
    Ok(chunks)
}

fn line_chunks(source: &str, lang_name: &str) -> Vec<Chunk> {
    let lines: Vec<&str> = source.lines().collect();
    if lines.is_empty() {
        return vec![];
    }
    let mut chunks = Vec::new();
    let mut offset = 0usize;
    while offset < lines.len() {
        let end = (offset + FALLBACK_CHUNK_LINES).min(lines.len());
        let content = lines[offset..end].join("\n");
        if !content.trim().is_empty() {
            chunks.push(Chunk {
                content,
                start_line: offset as u32,
                end_line: end as u32 - 1,
                language: lang_name.to_string(),
            });
        }
        if end == lines.len() {
            break;
        }
        offset += FALLBACK_CHUNK_LINES - FALLBACK_OVERLAP_LINES;
    }
    chunks
}
