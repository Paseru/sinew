use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::Page;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

const MAX_FRAMES: usize = 12;
const MAX_GIF_WIDTH: u32 = 480;
const ENCODE_TIMEOUT_SECS: u64 = 60;

pub struct RecordingHandle {
    task: JoinHandle<()>,
    frames: Arc<Mutex<Vec<Vec<u8>>>>,
    fps: u8,
    started_at: Instant,
}

impl RecordingHandle {
    pub fn start(page: Page, fps: u8, max_duration_s: u32) -> Self {
        let fps = fps.clamp(1, 10);
        let frames: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
        let frames_ref = Arc::clone(&frames);
        let interval = Duration::from_millis(1000 / fps as u64);
        let deadline = Duration::from_secs(max_duration_s as u64);
        let started = Instant::now();

        let task = tokio::spawn(async move {
            loop {
                if started.elapsed() >= deadline {
                    break;
                }
                let guard = frames_ref.lock().await;
                if guard.len() >= MAX_FRAMES {
                    break;
                }
                drop(guard);

                let params = ScreenshotParams::builder()
                    .format(chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat::Png)
                    .build();
                if let Ok(bytes) = page.screenshot(params).await {
                    let mut guard = frames_ref.lock().await;
                    guard.push(bytes);
                }
                tokio::time::sleep(interval).await;
            }
        });

        Self { task, frames, fps, started_at: Instant::now() }
    }

    pub async fn stop(self, format: &str) -> Result<(Vec<u8>, &'static str, usize, u64)> {
        self.task.abort();

        let frames_guard = self.frames.lock().await;
        let frame_count = frames_guard.len();
        let duration_ms = self.started_at.elapsed().as_millis() as u64;

        if frame_count == 0 {
            return Ok((Vec::new(), "image/gif", 0, duration_ms));
        }

        let frames: Vec<Vec<u8>> = frames_guard.clone();
        drop(frames_guard);
        let fps = self.fps;

        let timeout = Duration::from_secs(ENCODE_TIMEOUT_SECS);

        if format == "mp4" {
            let mp4_handle = tokio::task::spawn_blocking({
                let frames = frames.clone();
                move || encode_mp4_ffmpeg(&frames, fps)
            });
            match tokio::time::timeout(timeout, mp4_handle).await {
                Ok(Ok(Ok(bytes))) => return Ok((bytes, "video/mp4", frame_count, duration_ms)),
                Ok(Ok(Err(err))) => tracing::warn!("MP4 failed ({err}), falling back to GIF"),
                Ok(Err(_)) => tracing::warn!("MP4 task panicked, falling back to GIF"),
                Err(_) => tracing::warn!("MP4 encoding timed out, falling back to GIF"),
            }
        }

        let gif_handle = tokio::task::spawn_blocking(move || encode_gif(&frames, fps));
        let gif_bytes = tokio::time::timeout(timeout, gif_handle)
            .await
            .context("GIF encoding timed out after 30s")?
            .context("GIF task panicked")?
            .context("GIF encoding failed")?;

        Ok((gif_bytes, "image/gif", frame_count, duration_ms))
    }
}

fn encode_mp4_ffmpeg(png_frames: &[Vec<u8>], fps: u8) -> Result<Vec<u8>> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let temp_dir = std::env::temp_dir().join(format!("sinew-rec-{ts}"));
    std::fs::create_dir_all(&temp_dir).context("create temp dir")?;

    for (i, frame) in png_frames.iter().enumerate() {
        std::fs::write(temp_dir.join(format!("f{i:05}.png")), frame)
            .with_context(|| format!("write frame {i}"))?;
    }

    let output_path = temp_dir.join("out.mp4");

    let result = Command::new("ffmpeg")
        .args([
            "-y",
            "-framerate", &fps.to_string(),
            "-i", temp_dir.join("f%05d.png").to_str().unwrap_or(""),
            "-c:v", "libx264",
            "-pix_fmt", "yuv420p",
            "-crf", "23",
            "-movflags", "+faststart",
            output_path.to_str().unwrap_or("out.mp4"),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let status = match result {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let _ = std::fs::remove_dir_all(&temp_dir);
            bail!("ffmpeg not found — install ffmpeg to enable MP4 recording");
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err(e).context("ffmpeg failed to start");
        }
        Ok(s) => s,
    };

    if !status.success() {
        let _ = std::fs::remove_dir_all(&temp_dir);
        bail!("ffmpeg exited with non-zero status");
    }

    let bytes = std::fs::read(&output_path).context("read MP4 output")?;
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(bytes)
}

fn encode_gif(png_frames: &[Vec<u8>], fps: u8) -> Result<Vec<u8>> {
    let delay_cs = (100u16).saturating_div(fps as u16).max(2);

    let first = image::load_from_memory(png_frames[0].as_slice())
        .context("decode first frame")?
        .into_rgba8();
    let (orig_w, orig_h) = (first.width(), first.height());
    let (gif_w, gif_h) = if orig_w > MAX_GIF_WIDTH {
        let ratio = MAX_GIF_WIDTH as f32 / orig_w as f32;
        (MAX_GIF_WIDTH, (orig_h as f32 * ratio) as u32)
    } else {
        (orig_w, orig_h)
    };

    // Decode and resize all frames — use Nearest for speed (Triangle is 3× slower).
    let decoded: Vec<Vec<u8>> = png_frames
        .iter()
        .map(|png| {
            let img = image::load_from_memory(png)
                .unwrap_or_else(|_| image::DynamicImage::new_rgba8(gif_w, gif_h));
            let rgba = if img.width() != gif_w || img.height() != gif_h {
                img.resize_exact(gif_w, gif_h, image::imageops::FilterType::Nearest)
                    .into_rgba8()
            } else {
                img.into_rgba8()
            };
            rgba.into_raw()
        })
        .collect();

    // Build global palette from first frame only — fast and sufficient for UI recordings.
    let sample = &decoded[0];
    // NeuQuant sample_faction=30: trains on every 30th pixel — very fast, acceptable quality.
    let nq = color_quant::NeuQuant::new(30, 256, sample);
    let global_palette = nq.color_map_rgb();

    let mut output = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut output, gif_w as u16, gif_h as u16, &global_palette)?;
        encoder.set_repeat(gif::Repeat::Infinite)?;

        for frame_rgba in &decoded {
            let indices: Vec<u8> = frame_rgba
                .chunks(4)
                .map(|px| nq.index_of(px) as u8)
                .collect();
            let frame = gif::Frame {
                delay: delay_cs,
                dispose: gif::DisposalMethod::Any,
                transparent: None,
                needs_user_input: false,
                top: 0,
                left: 0,
                width: gif_w as u16,
                height: gif_h as u16,
                interlaced: false,
                palette: None,
                buffer: std::borrow::Cow::Owned(indices),
            };
            encoder.write_frame(&frame)?;
        }
    }

    Ok(output)
}

pub fn gif_to_base64(bytes: &[u8]) -> String {
    BASE64.encode(bytes)
}
