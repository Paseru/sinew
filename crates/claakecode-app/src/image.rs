use std::{
    env, fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use reqwest::{
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    StatusCode,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use claakecode_core::ToolDescriptor;
use claakecode_openai::{Credential, MODEL_ID as OPENAI_RESPONSES_IMAGE_MODEL};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::store::ImageProvider;
use crate::tool_run::{FileChange, FileChangeKind, ToolRunImage, ToolRunResult};

const OPENAI_IMAGES_URL: &str = "https://api.openai.com/v1/images/generations";
const OPENAI_CODEX_RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
const GPT_IMAGE_MODEL: &str = "gpt-image-2";
const NANO_BANANA_MODEL: &str = "gemini-3.1-flash-image-preview";
const NANO_BANANA_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.1-flash-image-preview:generateContent";
const OPENAI_SUBSCRIPTION_IMAGE_INSTRUCTIONS: &str = "You are Sinew, a concise coding assistant. When the user asks for an image, immediately call the image_generation tool with their prompt. Do not reply with text.";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);
const USER_AGENT: &str = "ClaakeCode/0.1";

#[derive(Debug, Clone)]
pub struct CreateImageTool {
    http: reqwest::Client,
    workspace_root: PathBuf,
    image_provider: ImageProvider,
    openai_image_use_subscription: bool,
    openai_api_key: Option<String>,
    nano_banana_api_key: Option<String>,
    write_lock: Option<Arc<Semaphore>>,
}

impl CreateImageTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self::with_api_key(workspace_root, None)
    }

    pub fn with_api_key(workspace_root: impl Into<PathBuf>, api_key: Option<String>) -> Self {
        Self::with_settings(
            workspace_root,
            ImageProvider::GptImage2,
            false,
            api_key,
            None,
        )
    }

    pub fn with_settings(
        workspace_root: impl Into<PathBuf>,
        image_provider: ImageProvider,
        openai_image_use_subscription: bool,
        openai_api_key: Option<String>,
        nano_banana_api_key: Option<String>,
    ) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .user_agent(USER_AGENT)
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            workspace_root: workspace_root.into(),
            image_provider,
            openai_image_use_subscription,
            openai_api_key: normalize_configured_key(openai_api_key),
            nano_banana_api_key: normalize_configured_key(nano_banana_api_key),
            write_lock: None,
        }
    }

    pub fn with_workspace_write_lock(mut self, write_lock: Arc<Semaphore>) -> Self {
        self.write_lock = Some(write_lock);
        self
    }

    pub fn descriptor(&self) -> ToolDescriptor {
        match self.image_provider {
            ImageProvider::GptImage2 => ToolDescriptor {
                name: "CreateImage".into(),
                description: "Use this when the user asks to generate or create a new image. Returns the generated image visually.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "Detailed description of the image to create. Keep user-visible text exact when text appears inside the image.",
                            "maxLength": 32000
                        },
                        "size": {
                            "type": "string",
                            "enum": [
                                "1024x1024",
                                "1024x1536",
                                "1536x1024",
                                "3840x2160"
                            ],
                            "description": "Output size and format. Choose one based on what you want."
                        },
                        "n": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 10,
                            "description": "Number of images to generate. Defaults to 1."
                        },
                        "output_format": {
                            "type": "string",
                            "enum": ["png", "jpeg", "webp"],
                            "description": "Returned image format. Defaults to png."
                        },
                        "background": {
                            "type": "string",
                            "enum": ["auto", "opaque"],
                            "description": "Background style. gpt-image-2 does not support transparent backgrounds."
                        },
                        "moderation": {
                            "type": "string",
                            "enum": ["auto", "low"],
                            "description": "Content filtering strictness. Defaults to low."
                        }
                    },
                    "required": ["prompt"],
                    "additionalProperties": false
                }),
            },
            ImageProvider::NanoBanana2 => ToolDescriptor {
                name: "CreateImage".into(),
                description: "Create images with Nano Banana 2. Use this when the user asks to generate or create a new image. Returns the generated image visually.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "Detailed description of the image to create. Keep user-visible text exact when text appears inside the image.",
                            "maxLength": 32000
                        },
                        "aspect_ratio": {
                            "type": "string",
                            "enum": [
                                "1:1",
                                "1:4",
                                "1:8",
                                "2:3",
                                "3:2",
                                "3:4",
                                "4:1",
                                "4:3",
                                "4:5",
                                "5:4",
                                "8:1",
                                "9:16",
                                "16:9",
                                "21:9"
                            ],
                            "description": "Image aspect ratio. Defaults to 1:1."
                        },
                        "image_size": {
                            "type": "string",
                            "enum": ["512", "1K", "2K", "4K"],
                            "description": "Output image size. Defaults to 1K."
                        }
                    },
                    "required": ["prompt"],
                    "additionalProperties": false
                }),
            },
        }
    }

    pub async fn run(&self, input: Value) -> ToolRunResult {
        match self.create(input).await {
            Ok(output) => output,
            Err(err) => ToolRunResult::err(err.to_string(), Vec::new()),
        }
    }

    async fn create(&self, input: Value) -> Result<ToolRunResult> {
        match self.image_provider {
            ImageProvider::GptImage2 => self.create_gpt_image(input).await,
            ImageProvider::NanoBanana2 => self.create_nano_banana(input).await,
        }
    }

    async fn create_gpt_image(&self, input: Value) -> Result<ToolRunResult> {
        let parsed: GptImageInput = serde_json::from_value(input)
            .map_err(|err| anyhow::anyhow!("invalid CreateImage input: {err}"))?;
        let prompt = parsed.prompt.trim();
        if prompt.is_empty() {
            bail!("prompt is required");
        }

        let api_key = load_openai_api_key(self.openai_api_key.as_deref())?;
        let size = normalize_size(parsed.size.as_deref())?;
        let quality = "high";
        let n = parsed.n.unwrap_or(1);
        if !(1..=4).contains(&n) {
            bail!("n must be between 1 and 4");
        }
        let output_format = normalize_output_format(parsed.output_format.as_deref())?;
        let background = normalize_background(parsed.background.as_deref())?;
        let moderation = normalize_moderation(parsed.moderation.as_deref())?;

        if self.openai_image_use_subscription {
            return self
                .create_gpt_image_with_subscription(
                    prompt,
                    size,
                    quality,
                    n,
                    output_format,
                    background,
                    moderation,
                )
                .await;
        }

        let mut body = Map::new();
        body.insert("model".into(), json!(GPT_IMAGE_MODEL));
        body.insert("prompt".into(), json!(prompt));
        body.insert("size".into(), json!(size));
        body.insert("quality".into(), json!(quality));
        body.insert("n".into(), json!(n));
        body.insert("output_format".into(), json!(output_format));
        if let Some(background) = background {
            body.insert("background".into(), json!(background));
        }
        if let Some(moderation) = moderation {
            body.insert("moderation".into(), json!(moderation));
        }
        let response = self
            .http
            .post(OPENAI_IMAGES_URL)
            .header(AUTHORIZATION, format!("Bearer {api_key}"))
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json")
            .json(&Value::Object(body))
            .send()
            .await
            .context("OpenAI image request failed")?;

        let status = response.status();
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let text = response
            .text()
            .await
            .context("unable to read OpenAI image response")?;
        if !status.is_success() {
            bail!(
                "{}",
                format_openai_error(status, request_id.as_deref(), &text)
            );
        }

        let payload: ImageGenerationResponse =
            serde_json::from_str(&text).context("invalid OpenAI image response")?;
        let mut file_changes = Vec::new();
        let _write_permit = self.acquire_write_permit().await?;
        let images = payload
            .data
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let data = item.b64_json.as_deref().ok_or_else(|| {
                    anyhow::anyhow!("image {} did not include base64 data", idx + 1)
                })?;
                let bytes = decode_image(data, idx + 1)?;
                let relative_path = self.save_image(&bytes, extension_for(output_format), idx)?;
                file_changes.push(FileChange {
                    relative_path: relative_path.clone(),
                    kind: FileChangeKind::Added,
                    summary: format!("Added generated image ({} bytes)", bytes.len()),
                    binary: true,
                    added_lines: 0,
                    removed_lines: 0,
                    truncated: false,
                    lines: Vec::new(),
                });
                Ok(ToolRunImage {
                    media_type: media_type_for(output_format).to_string(),
                    data: String::new(),
                    path: Some(
                        self.workspace_root
                            .join(&relative_path)
                            .display()
                            .to_string(),
                    ),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if images.is_empty() {
            bail!("OpenAI returned no images");
        }

        let mut output = format!(
            "model: {GPT_IMAGE_MODEL}\nimages: {}\nsize: {size}\nquality: {quality}\nformat: {output_format}",
            images.len()
        );
        if let Some(request_id) = request_id {
            output.push_str(&format!("\nrequest_id: {request_id}"));
        }
        output.push_str("\nsaved:");
        for image in &images {
            if let Some(path) = &image.path {
                if let Ok(relative) = PathBuf::from(path).strip_prefix(&self.workspace_root) {
                    output.push_str(&format!("\n- {}", relative.display()));
                }
            }
        }
        let revised_prompts = payload
            .data
            .iter()
            .filter_map(|item| item.revised_prompt.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        if !revised_prompts.is_empty() {
            output.push_str("\n\nrevised_prompt:\n");
            output.push_str(&revised_prompts.join("\n\n---\n\n"));
        }
        output.push_str("\n\n[Image attached visually.]");

        Ok(ToolRunResult::ok_with_images(output, images, file_changes))
    }

    async fn create_gpt_image_with_subscription(
        &self,
        prompt: &str,
        size: &str,
        quality: &str,
        n: u8,
        output_format: &str,
        background: Option<&str>,
        moderation: Option<&str>,
    ) -> Result<ToolRunResult> {
        let credential = Credential::load_default()?.ok_or_else(|| {
            anyhow::anyhow!(
                "OpenAI subscription image generation requires OpenAI to be connected in Settings > Providers."
            )
        })?;
        let bearer = credential.bearer(&self.http).await?;
        if !bearer.is_oauth {
            bail!(
                "OpenAI subscription image generation requires OpenAI OAuth. Connect OpenAI in Settings > Providers."
            );
        }

        let mut generated = Vec::new();
        for _ in 0..n {
            let mut images = self
                .request_subscription_image(
                    &bearer.token,
                    bearer.account_id.as_deref(),
                    prompt,
                    size,
                    quality,
                    output_format,
                    background,
                    moderation,
                )
                .await?;
            generated.append(&mut images);
            if generated.len() >= n as usize {
                break;
            }
        }
        generated.truncate(n as usize);

        let mut file_changes = Vec::new();
        let _write_permit = self.acquire_write_permit().await?;
        let images = generated
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let bytes = decode_image(&item.b64_json, idx + 1)?;
                let relative_path = self.save_image(&bytes, extension_for(output_format), idx)?;
                file_changes.push(FileChange {
                    relative_path: relative_path.clone(),
                    kind: FileChangeKind::Added,
                    summary: format!("Added generated image ({} bytes)", bytes.len()),
                    binary: true,
                    added_lines: 0,
                    removed_lines: 0,
                    truncated: false,
                    lines: Vec::new(),
                });
                Ok(ToolRunImage {
                    media_type: media_type_for(output_format).to_string(),
                    data: String::new(),
                    path: Some(
                        self.workspace_root
                            .join(&relative_path)
                            .display()
                            .to_string(),
                    ),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if images.is_empty() {
            bail!("OpenAI subscription returned no images");
        }

        let mut output = format!(
            "model: {GPT_IMAGE_MODEL}\nsource: OpenAI subscription\nimages: {}\nsize: {size}\nquality: {quality}\nformat: {output_format}",
            images.len()
        );
        let request_ids = generated
            .iter()
            .filter_map(|item| item.request_id.as_deref())
            .collect::<Vec<_>>();
        if !request_ids.is_empty() {
            output.push_str("\nrequest_id:");
            for request_id in request_ids {
                output.push_str(&format!("\n- {request_id}"));
            }
        }
        output.push_str("\nsaved:");
        for image in &images {
            if let Some(path) = &image.path {
                if let Ok(relative) = PathBuf::from(path).strip_prefix(&self.workspace_root) {
                    output.push_str(&format!("\n- {}", relative.display()));
                }
            }
        }
        let revised_prompts = generated
            .iter()
            .filter_map(|item| item.revised_prompt.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        if !revised_prompts.is_empty() {
            output.push_str("\n\nrevised_prompt:\n");
            output.push_str(&revised_prompts.join("\n\n---\n\n"));
        }
        output.push_str("\n\n[Image attached visually.]");

        Ok(ToolRunResult::ok_with_images(output, images, file_changes))
    }

    async fn request_subscription_image(
        &self,
        access_token: &str,
        account_id: Option<&str>,
        prompt: &str,
        size: &str,
        quality: &str,
        output_format: &str,
        background: Option<&str>,
        moderation: Option<&str>,
    ) -> Result<Vec<SubscriptionImageItem>> {
        let mut image_tool = Map::new();
        image_tool.insert("type".into(), json!("image_generation"));
        image_tool.insert("action".into(), json!("generate"));
        image_tool.insert("output_format".into(), json!(output_format));
        image_tool.insert("quality".into(), json!(quality));
        if size != "auto" {
            image_tool.insert("size".into(), json!(size));
        }
        if let Some(background) = background {
            image_tool.insert("background".into(), json!(background));
        }
        if let Some(moderation) = moderation {
            image_tool.insert("moderation".into(), json!(moderation));
        }

        let body = json!({
            "model": OPENAI_RESPONSES_IMAGE_MODEL,
            "instructions": OPENAI_SUBSCRIPTION_IMAGE_INSTRUCTIONS,
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{ "type": "input_text", "text": prompt }],
            }],
            "tools": [Value::Object(image_tool)],
            "tool_choice": "auto",
            "stream": true,
            "store": false,
        });

        let mut request = self
            .http
            .post(OPENAI_CODEX_RESPONSES_URL)
            .header(AUTHORIZATION, format!("Bearer {access_token}"))
            .header("openai-beta", "responses=experimental")
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "text/event-stream");
        if let Some(account_id) = account_id {
            request = request.header("chatgpt-account-id", account_id);
        }

        let response = request
            .json(&body)
            .send()
            .await
            .context("OpenAI subscription image request failed")?;

        let status = response.status();
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        if !status.is_success() {
            let text = response
                .text()
                .await
                .context("unable to read OpenAI subscription image response")?;
            bail!(
                "{}",
                format_openai_error(status, request_id.as_deref(), &text)
            );
        }

        let calls = collect_subscription_image_calls(response).await?;
        subscription_images_from_calls(calls, request_id)
    }

    async fn create_nano_banana(&self, input: Value) -> Result<ToolRunResult> {
        let parsed: NanoBananaInput = serde_json::from_value(input)
            .map_err(|err| anyhow::anyhow!("invalid CreateImage input: {err}"))?;
        let prompt = parsed.prompt.trim();
        if prompt.is_empty() {
            bail!("prompt is required");
        }

        let api_key = load_nano_banana_api_key(self.nano_banana_api_key.as_deref())?;
        let aspect_ratio = normalize_aspect_ratio(parsed.aspect_ratio.as_deref())?;
        let image_size = normalize_image_size(parsed.image_size.as_deref())?;

        let body = json!({
            "contents": [{
                "parts": [{ "text": prompt }]
            }],
            "generationConfig": {
                "responseModalities": ["IMAGE"],
                "imageConfig": {
                    "aspectRatio": aspect_ratio,
                    "imageSize": image_size
                },
                "thinkingConfig": {
                    "thinkingLevel": "High",
                    "includeThoughts": false
                }
            }
        });
        let response = self
            .http
            .post(NANO_BANANA_URL)
            .header("x-goog-api-key", api_key)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json")
            .json(&body)
            .send()
            .await
            .context("Nano Banana 2 image request failed")?;

        let status = response.status();
        let request_id = response
            .headers()
            .get("x-goog-request-id")
            .or_else(|| response.headers().get("x-request-id"))
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let text = response
            .text()
            .await
            .context("unable to read Nano Banana 2 image response")?;
        if !status.is_success() {
            bail!(
                "{}",
                format_gemini_error(status, request_id.as_deref(), &text)
            );
        }

        let payload: GeminiGenerateResponse =
            serde_json::from_str(&text).context("invalid Nano Banana 2 image response")?;
        let parts = payload
            .candidates
            .iter()
            .filter_map(|candidate| candidate.content.as_ref())
            .flat_map(|content| content.parts.iter())
            .filter(|part| !part.thought)
            .collect::<Vec<_>>();
        let notes = parts
            .iter()
            .filter_map(|part| part.text.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        let image_parts = parts
            .iter()
            .filter_map(|part| part.inline_data.as_ref())
            .collect::<Vec<_>>();

        let mut file_changes = Vec::new();
        let _write_permit = self.acquire_write_permit().await?;
        let images = image_parts
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let bytes = decode_image(&item.data, idx + 1)?;
                let media_type = normalize_media_type(&item.mime_type);
                let relative_path =
                    self.save_image(&bytes, extension_for_media_type(media_type), idx)?;
                file_changes.push(FileChange {
                    relative_path: relative_path.clone(),
                    kind: FileChangeKind::Added,
                    summary: format!("Added generated image ({} bytes)", bytes.len()),
                    binary: true,
                    added_lines: 0,
                    removed_lines: 0,
                    truncated: false,
                    lines: Vec::new(),
                });
                Ok(ToolRunImage {
                    media_type: media_type.to_string(),
                    data: String::new(),
                    path: Some(
                        self.workspace_root
                            .join(&relative_path)
                            .display()
                            .to_string(),
                    ),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if images.is_empty() {
            bail!("Nano Banana 2 returned no images");
        }

        let mut output = format!(
            "model: {NANO_BANANA_MODEL}\nimages: {}\naspect_ratio: {aspect_ratio}\nimage_size: {image_size}\nthinking_level: high",
            images.len()
        );
        if let Some(request_id) = request_id {
            output.push_str(&format!("\nrequest_id: {request_id}"));
        }
        output.push_str("\nsaved:");
        for image in &images {
            if let Some(path) = &image.path {
                if let Ok(relative) = PathBuf::from(path).strip_prefix(&self.workspace_root) {
                    output.push_str(&format!("\n- {}", relative.display()));
                }
            }
        }
        if !notes.is_empty() {
            output.push_str("\n\nnotes:\n");
            output.push_str(&notes.join("\n\n---\n\n"));
        }
        output.push_str("\n\n[Image attached visually.]");

        Ok(ToolRunResult::ok_with_images(output, images, file_changes))
    }

    fn save_image(&self, bytes: &[u8], extension: &str, idx: usize) -> Result<String> {
        let dir = self.workspace_root.join(".claakecode/images");
        fs::create_dir_all(&dir).context("unable to create .claakecode/images")?;
        let name = format!("{}-{}.{}", now_ms(), idx + 1, extension);
        let relative_path = format!(".claakecode/images/{name}");
        fs::write(self.workspace_root.join(&relative_path), bytes)
            .context("unable to save generated image")?;
        Ok(relative_path)
    }

    async fn acquire_write_permit(&self) -> Result<Option<OwnedSemaphorePermit>> {
        let Some(write_lock) = &self.write_lock else {
            return Ok(None);
        };
        write_lock
            .clone()
            .acquire_owned()
            .await
            .map(Some)
            .map_err(|_| anyhow::anyhow!("workspace write lock is closed"))
    }
}

#[derive(Debug, Deserialize)]
struct GptImageInput {
    prompt: String,
    #[serde(default)]
    size: Option<String>,
    #[serde(default)]
    n: Option<u8>,
    #[serde(default)]
    output_format: Option<String>,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    moderation: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NanoBananaInput {
    prompt: String,
    #[serde(default)]
    aspect_ratio: Option<String>,
    #[serde(default)]
    image_size: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ImageGenerationResponse {
    data: Vec<ImageGenerationItem>,
}

#[derive(Debug, Deserialize)]
struct ImageGenerationItem {
    #[serde(default)]
    b64_json: Option<String>,
    #[serde(default)]
    revised_prompt: Option<String>,
}

struct SubscriptionImageItem {
    b64_json: String,
    revised_prompt: Option<String>,
    request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiGenerateResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    #[serde(default)]
    content: Option<GeminiContent>,
}

#[derive(Debug, Deserialize)]
struct GeminiContent {
    #[serde(default)]
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Deserialize)]
struct GeminiPart {
    #[serde(default)]
    text: Option<String>,
    #[serde(default, rename = "inlineData", alias = "inline_data")]
    inline_data: Option<GeminiInlineData>,
    #[serde(default)]
    thought: bool,
}

#[derive(Debug, Deserialize)]
struct GeminiInlineData {
    #[serde(rename = "mimeType", alias = "mime_type")]
    mime_type: String,
    data: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiErrorEnvelope {
    error: OpenAiError,
}

#[derive(Debug, Deserialize)]
struct OpenAiError {
    message: String,
    #[serde(rename = "type")]
    kind: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiErrorEnvelope {
    error: GeminiError,
}

#[derive(Debug, Deserialize)]
struct GeminiError {
    message: String,
    status: Option<String>,
    code: Option<u16>,
}

fn normalize_configured_key(key: Option<String>) -> Option<String> {
    key.map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

fn load_openai_api_key(configured: Option<&str>) -> Result<String> {
    configured
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_string)
        .or_else(|| {
            env::var("OPENAI_API_KEY")
                .ok()
                .map(|key| key.trim().to_string())
        })
        .filter(|key| !key.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "OpenAI API key is missing. Add it in Settings > Tools before using CreateImage."
            )
        })
}

fn load_nano_banana_api_key(configured: Option<&str>) -> Result<String> {
    configured
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_string)
        .or_else(|| {
            env::var("GEMINI_API_KEY")
                .ok()
                .map(|key| key.trim().to_string())
        })
        .or_else(|| {
            env::var("GOOGLE_API_KEY")
                .ok()
                .map(|key| key.trim().to_string())
        })
        .filter(|key| !key.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Nano Banana 2 API key is missing. Add it in Settings > Tools before using CreateImage."
            )
        })
}

fn normalize_size(raw: Option<&str>) -> Result<&'static str> {
    match raw.unwrap_or("auto").trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok("auto"),
        "1024x1024" | "square" => Ok("1024x1024"),
        "1024x1536" | "portrait" => Ok("1024x1536"),
        "1536x1024" | "landscape" => Ok("1536x1024"),
        "2560x1440" | "2k" => Ok("2560x1440"),
        "3840x2160" | "4k" => Ok("3840x2160"),
        other => bail!(
            "unsupported size `{other}`; use auto, 1024x1024, 1024x1536, 1536x1024, 2560x1440, or 3840x2160"
        ),
    }
}

fn normalize_output_format(raw: Option<&str>) -> Result<&'static str> {
    match raw.unwrap_or("png").trim().to_ascii_lowercase().as_str() {
        "" | "png" => Ok("png"),
        "jpg" | "jpeg" => Ok("jpeg"),
        "webp" => Ok("webp"),
        other => bail!("unsupported output_format `{other}`; use png, jpeg, or webp"),
    }
}

fn normalize_background(raw: Option<&str>) -> Result<Option<&'static str>> {
    match raw.unwrap_or("auto").trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(None),
        "opaque" => Ok(Some("opaque")),
        "transparent" => {
            bail!("gpt-image-2 does not support transparent backgrounds; use opaque or auto")
        }
        other => bail!("unsupported background `{other}`; use auto or opaque"),
    }
}

fn normalize_moderation(raw: Option<&str>) -> Result<Option<&'static str>> {
    match raw.unwrap_or("low").trim().to_ascii_lowercase().as_str() {
        "" | "low" => Ok(Some("low")),
        "auto" => Ok(None),
        other => bail!("unsupported moderation `{other}`; use auto or low"),
    }
}

fn normalize_aspect_ratio(raw: Option<&str>) -> Result<&'static str> {
    match raw.unwrap_or("1:1").trim() {
        "" | "1:1" => Ok("1:1"),
        "1:4" => Ok("1:4"),
        "1:8" => Ok("1:8"),
        "2:3" => Ok("2:3"),
        "3:2" => Ok("3:2"),
        "3:4" => Ok("3:4"),
        "4:1" => Ok("4:1"),
        "4:3" => Ok("4:3"),
        "4:5" => Ok("4:5"),
        "5:4" => Ok("5:4"),
        "8:1" => Ok("8:1"),
        "9:16" => Ok("9:16"),
        "16:9" => Ok("16:9"),
        "21:9" => Ok("21:9"),
        other => bail!(
            "unsupported aspect_ratio `{other}`; use 1:1, 1:4, 1:8, 2:3, 3:2, 3:4, 4:1, 4:3, 4:5, 5:4, 8:1, 9:16, 16:9, or 21:9"
        ),
    }
}

fn normalize_image_size(raw: Option<&str>) -> Result<&'static str> {
    match raw.unwrap_or("1K").trim() {
        "" | "1K" => Ok("1K"),
        "512" => Ok("512"),
        "2K" => Ok("2K"),
        "4K" => Ok("4K"),
        other => bail!("unsupported image_size `{other}`; use 512, 1K, 2K, or 4K"),
    }
}

fn media_type_for(output_format: &str) -> &'static str {
    match output_format {
        "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    }
}

fn normalize_media_type(raw: &str) -> &'static str {
    match raw.trim().to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => "image/jpeg",
        "image/webp" => "image/webp",
        _ => "image/png",
    }
}

fn decode_image(data: &str, idx: usize) -> Result<Vec<u8>> {
    BASE64_STANDARD
        .decode(data)
        .with_context(|| format!("image {idx} returned invalid base64"))
}

fn subscription_images_from_calls(
    calls: Vec<Value>,
    request_id: Option<String>,
) -> Result<Vec<SubscriptionImageItem>> {
    let raw_calls = calls.clone();
    let images = calls
        .into_iter()
        .filter_map(|item| {
            let b64_json = item
                .get("result")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?
                .to_string();
            let revised_prompt = item
                .get("revised_prompt")
                .and_then(Value::as_str)
                .map(str::to_string);
            Some(SubscriptionImageItem {
                b64_json,
                revised_prompt,
                request_id: request_id.clone(),
            })
        })
        .collect::<Vec<_>>();
    if images.is_empty() {
        if raw_calls.is_empty() {
            bail!(
                "OpenAI subscription returned no image_generation_call (the backend may not support the image_generation tool for this account/model)"
            );
        }
        let summary = raw_calls
            .iter()
            .map(|item| {
                let status = item
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("<missing>");
                let result_kind = match item.get("result") {
                    None => "no-result",
                    Some(Value::String(_)) => "result:string",
                    Some(Value::Null) => "result:null",
                    Some(Value::Object(_)) => "result:object",
                    Some(Value::Array(_)) => "result:array",
                    Some(_) => "result:other",
                };
                format!("status={status},{result_kind}")
            })
            .collect::<Vec<_>>()
            .join("; ");
        bail!(
            "OpenAI subscription returned {} image_generation_call(s) but no usable result ({summary})",
            raw_calls.len(),
        );
    }
    Ok(images)
}

async fn collect_subscription_image_calls(response: reqwest::Response) -> Result<Vec<Value>> {
    let mut stream = response.bytes_stream().eventsource();
    let mut calls = Vec::new();
    let mut last_error: Option<String> = None;
    let mut completed = false;
    while let Some(event) = stream.next().await {
        let event =
            event.map_err(|err| anyhow::anyhow!("OpenAI subscription image SSE error: {err}"))?;
        let data = event.data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let value: Value = match serde_json::from_str(data) {
            Ok(value) => value,
            Err(_) => continue,
        };
        match value.get("type").and_then(Value::as_str) {
            Some("response.output_item.done") => {
                if let Some(item) = value.get("item") {
                    if item.get("type").and_then(Value::as_str) == Some("image_generation_call") {
                        calls.push(item.clone());
                    }
                }
            }
            Some("response.completed") => {
                if calls.is_empty() {
                    if let Some(output) = value
                        .get("response")
                        .and_then(|response| response.get("output"))
                        .and_then(Value::as_array)
                    {
                        for item in output {
                            if item.get("type").and_then(Value::as_str)
                                == Some("image_generation_call")
                            {
                                calls.push(item.clone());
                            }
                        }
                    }
                }
                completed = true;
                break;
            }
            Some("response.failed" | "response.incomplete") => {
                last_error = value
                    .get("response")
                    .and_then(|response| response.get("error"))
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                break;
            }
            _ => {}
        }
    }
    if !completed && last_error.is_none() && calls.is_empty() {
        bail!("OpenAI subscription image stream ended before completion");
    }
    if let Some(message) = last_error {
        bail!(message);
    }
    Ok(calls)
}

fn extension_for(output_format: &str) -> &'static str {
    match output_format {
        "jpeg" => "jpg",
        "webp" => "webp",
        _ => "png",
    }
}

fn extension_for_media_type(media_type: &str) -> &'static str {
    match media_type {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        _ => "png",
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn format_openai_error(status: StatusCode, request_id: Option<&str>, body: &str) -> String {
    let mut message = serde_json::from_str::<OpenAiErrorEnvelope>(body)
        .map(|payload| match payload.error.kind {
            Some(kind) => format!("{kind}: {}", payload.error.message),
            None => payload.error.message,
        })
        .unwrap_or_else(|_| body.trim().to_string());
    if message.is_empty() {
        message = status.to_string();
    }
    if let Some(request_id) = request_id {
        format!("OpenAI image request failed ({status}, request {request_id}): {message}")
    } else {
        format!("OpenAI image request failed ({status}): {message}")
    }
}

fn format_gemini_error(status: StatusCode, request_id: Option<&str>, body: &str) -> String {
    let mut message = serde_json::from_str::<GeminiErrorEnvelope>(body)
        .map(|payload| match (payload.error.status, payload.error.code) {
            (Some(kind), Some(code)) => format!("{kind} ({code}): {}", payload.error.message),
            (Some(kind), None) => format!("{kind}: {}", payload.error.message),
            (None, Some(code)) => format!("{code}: {}", payload.error.message),
            (None, None) => payload.error.message,
        })
        .unwrap_or_else(|_| body.trim().to_string());
    if message.is_empty() {
        message = status.to_string();
    }
    if let Some(request_id) = request_id {
        format!("Nano Banana 2 image request failed ({status}, request {request_id}): {message}")
    } else {
        format!("Nano Banana 2 image request failed ({status}): {message}")
    }
}
