use std::{
    fs,
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use claakecode_core::ToolDescriptor;
use sha2::{Digest, Sha256};

use crate::{
    text::decode_text,
    tool_run::{ToolRunImage, ToolRunResult},
    workspace::{normalize_workspace_relative_path, resolve_workspace_path},
};

const MAX_LIMIT: usize = 500;
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
            description: "Read text files or attach supported image files visually.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to read. Relative paths are resolved from the workspace root; absolute paths are allowed." },
                    "offset": { "type": "integer", "minimum": 0, "description": "Optional zero-based line offset. Defaults to 0." },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_LIMIT,
                        "description": "Required number of lines to read. Hard-capped at 500."
                    }
                },
                "required": ["path", "limit"],
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

        let offset = parsed.offset.unwrap_or_default();
        let requested_limit = parsed
            .limit
            .ok_or_else(|| anyhow::anyhow!("limit is required"))?;
        if requested_limit == 0 {
            bail!("limit must be greater than 0");
        }
        let limit = requested_limit.min(MAX_LIMIT);

        let path = resolve_read_path(&self.workspace_root, &parsed.path)?;
        let metadata = fs::metadata(&path)
            .with_context(|| format!("unable to read file metadata {}", path.display()))?;
        if !metadata.is_file() {
            bail!("path is not a file");
        }
        let image_media_type = supported_image_media_type(&path);
        let max_bytes = if image_media_type.is_some() {
            MAX_IMAGE_BYTES
        } else {
            MAX_TEXT_READ_BYTES
        };
        if metadata.len() > max_bytes {
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

        let lines = split_lines(&content);
        let total_lines = lines.len();
        let selected_lines = lines
            .iter()
            .skip(offset)
            .take(limit)
            .copied()
            .collect::<Vec<_>>();
        let numbered = number_lines(&selected_lines, offset, total_lines);

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
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
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
            .read(json!({ "path": "pixel.gif", "limit": 1 }))
            .expect("image should read");

        assert!(!result.is_error);
        assert!(result.content.contains("[Image attached visually.]"));
        assert_eq!(result.images.len(), 1);
        assert_eq!(result.images[0].media_type, "image/gif");
        assert_eq!(result.images[0].data, BASE64_STANDARD.encode(image_bytes));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn read_requires_limit() {
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
            .read(json!({ "path": "many.txt", "limit": 9999 }))
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
            .read(json!({ "path": external_file.display().to_string(), "limit": 10 }))
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
        std::env::temp_dir().join(format!("claakecode-read-test-{}-{nanos}", std::process::id()))
    }
}
