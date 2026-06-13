use std::{
    fs,
    io::Read as _,
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sinew_core::ToolDescriptor;

use crate::{
    text::decode_text,
    tool_run::{ToolRunImage, ToolRunResult},
    workspace::{normalize_workspace_relative_path, resolve_workspace_path},
};

const MAX_LIMIT: usize = 500;
const MAX_RANGES: usize = 20;
const MAX_TEXT_READ_BYTES: u64 = 4 * 1024 * 1024;
const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ReadTool {
    workspace_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadFingerprint {
    pub relative_path: String,
    pub size: u64,
    pub modified_ms: i64,
    pub sha256: String,
}

impl ReadTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    pub fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: "read".into(),
            description: "Read text files by line ranges or attach supported image files visually."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to read. Relative paths are resolved from the workspace root; absolute paths are allowed." },
                    "ranges": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": MAX_RANGES,
                        "description": "Line ranges to read from text files. Each range uses a zero-based optional offset and a required limit. Ignored for supported images.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "offset": { "type": "integer", "minimum": 0, "default": 0, "description": "Optional zero-based line offset. Defaults to 0." },
                                "limit": {
                                    "type": "integer",
                                    "minimum": 1,
                                    "maximum": MAX_LIMIT,
                                    "description": "Required number of lines for this range. Hard-capped at 500."
                                }
                            },
                            "required": ["limit"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["path", "ranges"],
                "additionalProperties": false
            }),
        }
    }

    pub async fn run(&self, input: Value) -> ToolRunResult {
        match self.read(input) {
            Ok(output) => output,
            Err(err) => ToolRunResult::err(err.to_string(), Vec::new()),
        }
    }

    pub fn normalize_path(&self, raw: &str) -> Result<String> {
        normalize_read_path(&self.workspace_root, raw)
    }

    fn read(&self, input: Value) -> Result<ToolRunResult> {
        let parsed: ReadInput = serde_json::from_value(input)
            .map_err(|err| anyhow::anyhow!("invalid read input: {err}"))?;

        if parsed.path.trim().is_empty() {
            bail!("path is required");
        }

        let path = resolve_read_path(&self.workspace_root, &parsed.path)?;
        let metadata = fs::metadata(&path)
            .with_context(|| format!("unable to read file metadata {}", path.display()))?;
        if !metadata.is_file() {
            bail!("path is not a file");
        }
        let image_media_type =
            detect_file_image_media_type(&path)?.or_else(|| supported_image_media_type(&path));
        if image_media_type.is_some() && metadata.len() > MAX_IMAGE_BYTES {
            bail!("file is too large to read safely");
        }
        if image_media_type.is_none() && metadata.len() > MAX_TEXT_READ_BYTES {
            bail!("file is too large to read safely");
        }

        let display_path = display_read_path(&self.workspace_root, &path);
        let bytes =
            fs::read(&path).with_context(|| format!("unable to read file {}", path.display()))?;
        if let Some(media_type) = image_media_type {
            return Ok(ToolRunResult::ok_with_images(
                format!(
                    "path: {display_path}\ntype: {media_type}\nsize: {} bytes\n\n[Image attached visually.]",
                    bytes.len()
                ),
                vec![ToolRunImage {
                    media_type: media_type.to_string(),
                    data: BASE64_STANDARD.encode(bytes),
                    path: None,
                }],
                Vec::new(),
            ));
        }

        let content = decode_text(&bytes)
            .map(|decoded| decoded.content)
            .context("file is binary")?;

        let ranges = parsed.text_ranges()?;
        let lines = split_lines(&content);
        let total_lines = lines.len();
        let numbered = render_ranges(&lines, &ranges, total_lines);

        Ok(ToolRunResult::ok_with_meta(
            format!("path: {display_path}\ntotal: {total_lines}\n\n{numbered}"),
            Vec::new(),
            json!({ "read_fingerprint": fingerprint_for_bytes(display_path.clone(), metadata.len(), &metadata, &bytes) }),
        ))
    }
}

#[derive(Debug, Deserialize)]
struct ReadInput {
    path: String,
    #[serde(default)]
    ranges: Option<Vec<ReadRangeInput>>,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ReadRangeInput {
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
struct ReadRange {
    offset: usize,
    limit: usize,
}

impl ReadInput {
    fn text_ranges(&self) -> Result<Vec<ReadRange>> {
        if let Some(ranges) = &self.ranges {
            if ranges.is_empty() {
                bail!("ranges must contain at least one range");
            }
            if ranges.len() > MAX_RANGES {
                bail!("ranges cannot contain more than {MAX_RANGES} ranges");
            }
            return ranges
                .iter()
                .enumerate()
                .map(|(index, range)| {
                    let requested_limit = range
                        .limit
                        .ok_or_else(|| anyhow::anyhow!("ranges[{index}].limit is required"))?;
                    validate_limit(requested_limit, &format!("ranges[{index}].limit")).map(
                        |limit| ReadRange {
                            offset: range.offset.unwrap_or_default(),
                            limit,
                        },
                    )
                })
                .collect();
        }

        let requested_limit = self
            .limit
            .ok_or_else(|| anyhow::anyhow!("limit is required"))?;
        validate_limit(requested_limit, "limit").map(|limit| {
            vec![ReadRange {
                offset: self.offset.unwrap_or_default(),
                limit,
            }]
        })
    }
}

fn validate_limit(requested_limit: usize, label: &str) -> Result<usize> {
    if requested_limit == 0 {
        bail!("{label} must be greater than 0");
    }
    Ok(requested_limit.min(MAX_LIMIT))
}

pub fn detect_image_media_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.len() >= 3 && bytes[0] == 0xff && bytes[1] == 0xd8 && bytes[2] == 0xff {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

fn split_lines(content: &str) -> Vec<&str> {
    if content.is_empty() {
        Vec::new()
    } else {
        content.split_inclusive('\n').collect()
    }
}

fn number_lines(lines: &[&str], offset: usize, total_lines: usize) -> String {
    let width = total_lines.to_string().len().max(1);
    let mut output = String::new();

    for (index, line) in lines.iter().enumerate() {
        let line_number = offset + index + 1;
        output.push_str(&format!("{line_number:>width$} | {line}"));
    }

    output
}

fn render_ranges(lines: &[&str], ranges: &[ReadRange], total_lines: usize) -> String {
    if ranges.len() == 1 {
        let range = ranges[0];
        let selected_lines = select_lines(lines, range);
        return number_lines(&selected_lines, range.offset, total_lines);
    }

    let mut output = String::new();
    for (index, range) in ranges.iter().copied().enumerate() {
        if !output.is_empty() {
            ensure_trailing_newline(&mut output);
            output.push('\n');
        }

        let selected_lines = select_lines(lines, range);
        output.push_str(&range_header(index, range, selected_lines.len()));
        output.push('\n');
        output.push_str(&number_lines(&selected_lines, range.offset, total_lines));
    }
    output
}

fn select_lines<'a>(lines: &[&'a str], range: ReadRange) -> Vec<&'a str> {
    lines
        .iter()
        .skip(range.offset)
        .take(range.limit)
        .copied()
        .collect()
}

fn range_header(index: usize, range: ReadRange, selected_count: usize) -> String {
    if selected_count == 0 {
        return format!("range {}: empty (offset {})", index + 1, range.offset);
    }

    let first = range.offset + 1;
    let last = range.offset + selected_count;
    format!("range {}: {first}-{last}", index + 1)
}

fn ensure_trailing_newline(output: &mut String) {
    if !output.ends_with('\n') {
        output.push('\n');
    }
}

fn supported_image_media_type(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

fn detect_file_image_media_type(path: &Path) -> Result<Option<&'static str>> {
    let mut file =
        fs::File::open(path).with_context(|| format!("unable to read file {}", path.display()))?;
    let mut header = [0_u8; 12];
    let len = file
        .read(&mut header)
        .with_context(|| format!("unable to read file {}", path.display()))?;
    Ok(detect_image_media_type(&header[..len]))
}

fn resolve_read_path(root: &Path, raw: &str) -> Result<PathBuf> {
    let candidate = Path::new(raw);
    if candidate.is_absolute() {
        return candidate
            .canonicalize()
            .with_context(|| format!("unable to resolve path {}", candidate.display()));
    }

    resolve_workspace_path(root, raw)
}

fn normalize_read_path(root: &Path, raw: &str) -> Result<String> {
    let candidate = Path::new(raw);
    if candidate.is_absolute() {
        let canonical = candidate
            .canonicalize()
            .with_context(|| format!("unable to resolve path {}", candidate.display()))?;
        return Ok(display_read_path(root, &canonical));
    }

    normalize_workspace_relative_path(raw)
}

fn display_read_path(root: &Path, path: &Path) -> String {
    relative_from_root(root, path).unwrap_or_else(|_| path.display().to_string())
}

pub fn fingerprint_path(root: &Path, path: &Path) -> Result<ReadFingerprint> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("unable to read file metadata {}", path.display()))?;
    if !metadata.is_file() {
        bail!("path is not a file");
    }
    let bytes =
        fs::read(path).with_context(|| format!("unable to read file {}", path.display()))?;
    Ok(fingerprint_for_bytes(
        display_read_path(root, path),
        metadata.len(),
        &metadata,
        &bytes,
    ))
}

fn fingerprint_for_bytes(
    relative_path: String,
    size: u64,
    metadata: &fs::Metadata,
    bytes: &[u8],
) -> ReadFingerprint {
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_millis() as i64)
        .unwrap_or_default();
    let sha256 = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    ReadFingerprint {
        relative_path,
        size,
        modified_ms,
        sha256,
    }
}

fn relative_from_root(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .with_context(|| format!("{} is outside the workspace", path.display()))?;
    Ok(relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/"))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;

    use super::*;

    #[test]
    fn read_supported_image_returns_visual_payload() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        let image_bytes = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff!\xf9\x04\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;";
        fs::write(root.join("pixel.gif"), image_bytes).expect("write image");

        let tool = ReadTool::new(&root);
        let result = tool
            .read(json!({ "path": "pixel.gif" }))
            .expect("image should read without limit");

        assert!(!result.is_error);
        assert!(result.content.contains("[Image attached visually.]"));
        assert_eq!(result.images.len(), 1);
        assert_eq!(result.images[0].media_type, "image/gif");
        assert_eq!(result.images[0].data, BASE64_STANDARD.encode(image_bytes));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn read_supported_image_ignores_ranges() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        let image_bytes = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff!\xf9\x04\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;";
        fs::write(root.join("pixel.gif"), image_bytes).expect("write image");

        let tool = ReadTool::new(&root);
        let result = tool
            .read(json!({ "path": "pixel.gif", "ranges": [{ "offset": 99, "limit": 1 }] }))
            .expect("image should ignore text-only ranges");

        assert!(!result.is_error);
        assert!(result.content.contains("[Image attached visually.]"));
        assert_eq!(result.images.len(), 1);
        assert_eq!(result.images[0].media_type, "image/gif");
        assert_eq!(result.images[0].data, BASE64_STANDARD.encode(image_bytes));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn read_uses_image_bytes_to_correct_mislabeled_extension() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        let image_bytes = b"\x89PNG\r\n\x1a\nrest";
        fs::write(root.join("pixel.webp"), image_bytes).expect("write mislabeled png");

        let tool = ReadTool::new(&root);
        let result = tool
            .read(json!({ "path": "pixel.webp", "ranges": [{ "limit": 1 }] }))
            .expect("image should read");

        assert!(!result.is_error);
        assert_eq!(result.images.len(), 1);
        assert_eq!(result.images[0].media_type, "image/png");
        assert_eq!(result.images[0].data, BASE64_STANDARD.encode(image_bytes));
        assert!(result.content.contains("type: image/png"));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn read_requires_limit_without_ranges() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(root.join("app.rs"), "fn main() {}\n").expect("write file");

        let tool = ReadTool::new(&root);
        let error = tool
            .read(json!({ "path": "app.rs" }))
            .expect_err("missing limit should fail");

        assert!(error.to_string().contains("limit is required"));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn read_requires_limit_in_each_range() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        fs::write(root.join("app.rs"), "fn main() {}\n").expect("write file");

        let tool = ReadTool::new(&root);
        let error = tool
            .read(json!({ "path": "app.rs", "ranges": [{ "offset": 0 }] }))
            .expect_err("missing range limit should fail");

        assert!(error.to_string().contains("ranges[0].limit is required"));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn read_multiple_ranges_from_one_file() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        let content = (1..=10)
            .map(|line| format!("line {line}\n"))
            .collect::<String>();
        fs::write(root.join("many.txt"), content).expect("write file");

        let tool = ReadTool::new(&root);
        let result = tool
            .read(json!({
                "path": "many.txt",
                "ranges": [
                    { "offset": 1, "limit": 2 },
                    { "offset": 7, "limit": 3 }
                ]
            }))
            .expect("read should support multiple ranges");

        assert!(result.content.contains("total: 10"));
        assert!(result.content.contains("range 1: 2-3"));
        assert!(result.content.contains(" 2 | line 2"));
        assert!(result.content.contains(" 3 | line 3"));
        assert!(!result.content.contains(" 4 | line 4"));
        assert!(result.content.contains("range 2: 8-10"));
        assert!(result.content.contains(" 8 | line 8"));
        assert!(result.content.contains("10 | line 10"));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn read_caps_limit_at_max() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        let content = (1..=600)
            .map(|line| format!("line {line}\n"))
            .collect::<String>();
        fs::write(root.join("many.txt"), content).expect("write file");

        let tool = ReadTool::new(&root);
        let result = tool
            .read(json!({ "path": "many.txt", "ranges": [{ "limit": 9999 }] }))
            .expect("read should cap requested limit");

        assert!(result.content.contains("total: 600"));
        assert!(result.content.contains("500 | line 500"));
        assert!(!result.content.contains("501 | line 501"));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn read_accepts_absolute_path_outside_workspace() {
        let base = unique_temp_dir();
        let workspace = base.join("workspace");
        let external = base.join("external");
        fs::create_dir_all(&workspace).expect("create temp workspace");
        fs::create_dir_all(&external).expect("create external directory");
        let workspace = workspace.canonicalize().expect("canonical temp workspace");
        let external_file = external.join("notes.txt");
        fs::write(&external_file, "outside\nworkspace\n").expect("write external file");
        let external_file = external_file
            .canonicalize()
            .expect("canonical external file");

        let tool = ReadTool::new(&workspace);
        let result = tool
            .read(
                json!({ "path": external_file.display().to_string(), "ranges": [{ "limit": 10 }] }),
            )
            .expect("read should accept absolute paths outside the workspace");

        assert!(!result.is_error);
        assert!(result
            .content
            .contains(&format!("path: {}", external_file.display())));
        assert!(result.content.contains("1 | outside"));

        fs::remove_dir_all(base).ok();
    }

    #[test]
    fn normalize_path_accepts_absolute_workspace_path() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("create temp workspace");
        let root = root.canonicalize().expect("canonical temp workspace");
        let file = root.join("app.rs");
        fs::write(&file, "fn main() {}\n").expect("write file");
        let file = file.canonicalize().expect("canonical file");

        let tool = ReadTool::new(&root);
        let normalized = tool
            .normalize_path(&file.display().to_string())
            .expect("absolute workspace path should normalize");

        assert_eq!(normalized, "app.rs");

        fs::remove_dir_all(root).ok();
    }

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("sinew-read-test-{}-{nanos}", std::process::id()))
    }
}
