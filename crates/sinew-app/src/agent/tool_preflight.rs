use sinew_core::Part;

use crate::{tool_names, ReadFingerprint, ToolRunResult, WriteFileTool};

use std::collections::HashMap;

#[derive(Debug, Default)]
pub(super) struct WriteFileStreamPreflight {
    scanner: TopLevelJsonKeyScanner,
    path_checked: bool,
    failed: Option<ToolRunResult>,
}

impl WriteFileStreamPreflight {
    pub(super) fn push_chunk(
        &mut self,
        chunk: &str,
        write_file: &WriteFileTool,
        read_fingerprints: &HashMap<String, ReadFingerprint>,
    ) -> Option<ToolRunResult> {
        if let Some(result) = &self.failed {
            return Some(result.clone());
        }

        for event in self.scanner.push(chunk) {
            let TopLevelJsonEvent::Path { value } = event;
            if !self.path_checked {
                self.path_checked = true;
                if let Err(err) = write_file.preflight_path(&value, read_fingerprints) {
                    let result = ToolRunResult::err(err.to_string(), Vec::new());
                    self.failed = Some(result.clone());
                    return Some(result);
                }
            }
        }

        None
    }

    pub(super) fn failed(&self) -> Option<ToolRunResult> {
        self.failed.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TopLevelJsonEvent {
    Path { value: String },
}

#[derive(Debug, Default)]
struct TopLevelJsonKeyScanner {
    buffer: String,
    processed_key_count: usize,
}

impl TopLevelJsonKeyScanner {
    fn push(&mut self, chunk: &str) -> Vec<TopLevelJsonEvent> {
        self.buffer.push_str(chunk);
        let entries = scan_top_level_entries(&self.buffer);
        let mut events = Vec::new();

        for entry in entries.into_iter().skip(self.processed_key_count) {
            if entry.key == "path" {
                let Some(value) = entry.value else {
                    break;
                };
                self.processed_key_count += 1;
                events.push(TopLevelJsonEvent::Path { value });
                continue;
            }

            self.processed_key_count += 1;
        }

        events
    }
}

#[derive(Debug)]
struct TopLevelJsonEntry {
    key: String,
    value: Option<String>,
}

fn scan_top_level_entries(input: &str) -> Vec<TopLevelJsonEntry> {
    let bytes = input.as_bytes();
    let mut entries = Vec::new();
    let mut index = 0usize;
    let mut depth = 0usize;

    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                if depth == 1 {
                    let Some((key, end_quote)) = parse_json_string_at(input, index) else {
                        break;
                    };
                    let colon = skip_ws(bytes, end_quote + 1);
                    if colon < bytes.len() && bytes[colon] == b':' {
                        let value = if key == "path" {
                            parse_string_value_after_key(input, colon + 1).map(|(value, _)| value)
                        } else {
                            None
                        };
                        entries.push(TopLevelJsonEntry { key, value });
                    }
                    index = end_quote + 1;
                    continue;
                }

                let Some((_, end_quote)) = parse_json_string_at(input, index) else {
                    break;
                };
                index = end_quote + 1;
                continue;
            }
            b'{' | b'[' => depth = depth.saturating_add(1),
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
        index += 1;
    }

    entries
}

fn parse_string_value_after_key(input: &str, key_end: usize) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    let value_start = skip_ws(bytes, key_end);
    if value_start >= bytes.len() || bytes[value_start] != b'"' {
        return None;
    }
    let (value, end_quote) = parse_json_string_at(input, value_start)?;
    Some((value, end_quote + 1))
}

fn parse_json_string_at(input: &str, quote_index: usize) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    if quote_index >= bytes.len() || bytes[quote_index] != b'"' {
        return None;
    }

    let mut index = quote_index + 1;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            let raw = &input[quote_index..=index];
            let value = serde_json::from_str::<String>(raw).ok()?;
            return Some((value, index));
        }
        index += 1;
    }

    None
}

fn skip_ws(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

#[derive(Debug, Default)]
pub(super) struct ToolPreflightRegistry {
    write_file: HashMap<usize, WriteFileStreamPreflight>,
}

impl ToolPreflightRegistry {
    pub(super) fn start_tool(&mut self, index: usize, name: &str) {
        if tool_names::is_tool_name(name, tool_names::WRITE_FILE) {
            self.write_file
                .insert(index, WriteFileStreamPreflight::default());
        }
    }

    pub(super) fn push_tool_json(
        &mut self,
        index: usize,
        chunk: &str,
        write_file: &WriteFileTool,
        read_fingerprints: &HashMap<String, ReadFingerprint>,
    ) -> Option<ToolRunResult> {
        self.write_file
            .get_mut(&index)
            .and_then(|preflight| preflight.push_chunk(chunk, write_file, read_fingerprints))
    }

    pub(super) fn failed(&self, index: usize) -> Option<ToolRunResult> {
        self.write_file
            .get(&index)
            .and_then(WriteFileStreamPreflight::failed)
    }
}

pub(super) fn preflight_error_result(part: &Part) -> Option<ToolRunResult> {
    let Part::ToolCall { name, meta, .. } = part else {
        return None;
    };
    if !tool_names::is_tool_name(name, tool_names::WRITE_FILE) {
        return None;
    }
    let content = meta
        .as_ref()
        .and_then(|value| value.get("preflight_error"))
        .and_then(serde_json::Value::as_str)?;
    Some(ToolRunResult::err(content.to_string(), Vec::new()))
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, fs};

    use super::*;
    use crate::WriteFileTool;
    use uuid::Uuid;

    #[test]
    fn allows_content_before_path_across_chunks() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let tool = WriteFileTool::new(&root);
        let mut preflight = WriteFileStreamPreflight::default();

        assert!(preflight
            .push_chunk("{\"con", &tool, &HashMap::new())
            .is_none());
        let result = preflight.push_chunk(
            "tent\":\"hello\",\"path\":\"new.txt\"}",
            &tool,
            &HashMap::new(),
        );

        assert!(result.is_none());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn ignores_content_key_inside_path_string() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let tool = WriteFileTool::new(&root);
        let mut preflight = WriteFileStreamPreflight::default();

        let result = preflight.push_chunk(
            "{\"path\":\"content-folder/file.txt\",\"content\":\"hello\"}",
            &tool,
            &HashMap::new(),
        );

        assert!(result.is_none());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn ignores_content_key_inside_nested_object_before_path() {
        let mut scanner = TopLevelJsonKeyScanner::default();

        let events = scanner
            .push("{\"meta\":{\"content\":true},\"path\":\"new.txt\",\"content\":\"hello\"}");

        assert_eq!(
            events,
            vec![TopLevelJsonEvent::Path {
                value: "new.txt".to_string()
            }]
        );
    }

    #[test]
    fn waits_for_complete_path_value_across_chunks() {
        let mut scanner = TopLevelJsonKeyScanner::default();

        assert!(scanner.push("{\"path\":\"new").is_empty());
        assert_eq!(
            scanner.push(".txt\",\"content\":\"hello\"}"),
            vec![TopLevelJsonEvent::Path {
                value: "new.txt".to_string()
            }]
        );
    }

    #[test]
    fn preflights_existing_file_without_read_as_soon_as_path_arrives() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        fs::write(root.join("notes.txt"), "old\n").expect("write file");
        let tool = WriteFileTool::new(&root);
        let mut preflight = WriteFileStreamPreflight::default();

        let result = preflight
            .push_chunk(
                "{\"path\":\"notes.txt\",\"content\":\"",
                &tool,
                &HashMap::new(),
            )
            .expect("existing file without read should fail");

        assert!(result.is_error);
        assert!(result.content.contains("requires a successful read"));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn preflight_error_result_reads_tool_call_meta() {
        let part = Part::ToolCall {
            id: "call-1".to_string(),
            name: tool_names::WRITE_FILE.to_string(),
            input: serde_json::json!({ "value": "{\"path\":" }),
            meta: Some(serde_json::json!({ "preflight_error": "preflight failed" })),
        };

        let result = preflight_error_result(&part).expect("preflight error result");

        assert!(result.is_error);
        assert_eq!(result.content, "preflight failed");
    }

    fn unique_temp_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("sinew-tool-preflight-test-{}", Uuid::new_v4()))
    }
}
