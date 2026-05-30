use std::ptr::null_mut;
use serde_json::{json, Value};
use sinew_core::ToolDescriptor;
use crate::{tool_names, tool_run::{ToolRunImage, ToolRunResult}};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};

#[derive(Debug, Clone, Default)]
pub struct ComputerUseTool;

#[repr(C)]
#[derive(Clone, Copy)]
struct BITMAPINFOHEADER {
    bi_size: u32,
    bi_width: i32,
    bi_height: i32,
    bi_planes: u16,
    bi_bit_count: u16,
    bi_compression: u32,
    bi_size_image: u32,
    bi_x_pels_per_meter: i32,
    bi_y_pels_per_meter: i32,
    bi_clr_used: u32,
    bi_clr_important: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RGBQUAD {
    rgb_blue: u8,
    rgb_green: u8,
    rgb_red: u8,
    rgb_reserved: u8,
}

#[repr(C)]
struct BITMAPINFO {
    bmi_header: BITMAPINFOHEADER,
    bmi_colors: [RGBQUAD; 1],
}

#[repr(C)]
struct POINT {
    x: i32,
    y: i32,
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn GetDC(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn ReleaseDC(hwnd: *mut std::ffi::c_void, hdc: *mut std::ffi::c_void) -> i32;
    fn GetSystemMetrics(n_index: i32) -> i32;
    fn SetCursorPos(x: i32, y: i32) -> i32;
    fn mouse_event(dw_flags: u32, dx: i32, dy: i32, dw_data: u32, dw_extra_info: usize);
    fn keybd_event(b_vk: u8, b_scan: u8, dw_flags: u32, dw_extra_info: usize);
    fn GetCursorPos(lp_point: *mut POINT) -> i32;
    fn VkKeyScanW(ch: u16) -> i16;
}

#[cfg(target_os = "windows")]
#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(hdc: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn CreateCompatibleBitmap(hdc: *mut std::ffi::c_void, cx: i32, cy: i32) -> *mut std::ffi::c_void;
    fn SelectObject(hdc: *mut std::ffi::c_void, hgdiobj: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn BitBlt(hdc_dest: *mut std::ffi::c_void, x_dest: i32, y_dest: i32, width: i32, height: i32, hdc_src: *mut std::ffi::c_void, x_src: i32, y_src: i32, dw_rop: u32) -> i32;
    fn DeleteDC(hdc: *mut std::ffi::c_void) -> i32;
    fn DeleteObject(ho: *mut std::ffi::c_void) -> i32;
    fn GetDIBits(hdc: *mut std::ffi::c_void, hbm: *mut std::ffi::c_void, start: u32, lines: u32, lpv_bits: *mut u8, lpbmi: *mut BITMAPINFO, usage: u32) -> i32;
}

#[cfg(target_os = "windows")]
const SRCCOPY: u32 = 0x00CC0020;
#[cfg(target_os = "windows")]
const DIB_RGB_COLORS: u32 = 0;
#[cfg(target_os = "windows")]
const BI_RGB: u32 = 0;

impl ComputerUseTool {
    pub fn new() -> Self {
        Self
    }

    pub fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: tool_names::COMPUTER_USE.into(),
            description: "Control the Windows desktop. Take screenshots, move mouse, click, type text, or press keyboard shortcuts.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": [
                            "screenshot",
                            "mouse_move",
                            "left_click",
                            "right_click",
                            "middle_click",
                            "double_click",
                            "left_click_drag",
                            "type",
                            "key",
                            "cursor_position"
                        ],
                        "description": "The computer use action to perform."
                    },
                    "coordinate": {
                        "type": "array",
                        "items": { "type": "integer" },
                        "minItems": 2,
                        "maxItems": 2,
                        "description": "Optional [x, y] screen coordinates for mouse action."
                    },
                    "text": {
                        "type": "string",
                        "description": "Text to type or key name to press."
                    }
                },
                "required": ["action"],
                "additionalProperties": false
            }),
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub async fn run(&self, _input: Value) -> ToolRunResult {
        ToolRunResult::err("Computer Use is only supported on Windows.", Vec::new())
    }

    #[cfg(target_os = "windows")]
    pub async fn run(&self, input: Value) -> ToolRunResult {
        let action = input.get("action").and_then(Value::as_str).unwrap_or("");
        let coordinate = input.get("coordinate").and_then(Value::as_array);
        let text = input.get("text").and_then(Value::as_str).unwrap_or("");

        // If coordinates are provided for a mouse action, move cursor first
        if let Some(coords) = coordinate {
            if coords.len() >= 2 {
                let x = coords[0].as_i64().unwrap_or(0) as i32;
                let y = coords[1].as_i64().unwrap_or(0) as i32;
                unsafe {
                    SetCursorPos(x, y);
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }

        match action {
            "screenshot" => {
                match self.capture_screen_jpeg() {
                    Ok((width, height, jpeg_bytes)) => {
                        let base64_data = BASE64_STANDARD.encode(&jpeg_bytes);
                        let img = ToolRunImage {
                            media_type: "image/jpeg".to_string(),
                            data: base64_data,
                            path: None,
                        };
                        ToolRunResult::ok_with_images(
                            format!("Screenshot captured. Screen resolution: {}x{}", width, height),
                            vec![img],
                            Vec::new()
                        )
                    }
                    Err(err) => ToolRunResult::err(format!("Failed to capture screenshot: {}", err), Vec::new())
                }
            }
            "mouse_move" => {
                if let Some(coords) = coordinate {
                    let x = coords[0].as_i64().unwrap_or(0) as i32;
                    let y = coords[1].as_i64().unwrap_or(0) as i32;
                    ToolRunResult::ok(format!("Moved mouse to ({}, {})", x, y), Vec::new())
                } else {
                    ToolRunResult::err("action mouse_move requires coordinate argument", Vec::new())
                }
            }
            "left_click" => {
                unsafe {
                    mouse_event(0x0002, 0, 0, 0, 0); // DOWN
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    mouse_event(0x0004, 0, 0, 0, 0); // UP
                }
                ToolRunResult::ok("Left click performed", Vec::new())
            }
            "right_click" => {
                unsafe {
                    mouse_event(0x0008, 0, 0, 0, 0); // DOWN
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    mouse_event(0x0010, 0, 0, 0, 0); // UP
                }
                ToolRunResult::ok("Right click performed", Vec::new())
            }
            "middle_click" => {
                unsafe {
                    mouse_event(0x0020, 0, 0, 0, 0); // DOWN
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    mouse_event(0x0040, 0, 0, 0, 0); // UP
                }
                ToolRunResult::ok("Middle click performed", Vec::new())
            }
            "double_click" => {
                unsafe {
                    mouse_event(0x0002, 0, 0, 0, 0); // DOWN
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    mouse_event(0x0004, 0, 0, 0, 0); // UP
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    mouse_event(0x0002, 0, 0, 0, 0); // DOWN
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    mouse_event(0x0004, 0, 0, 0, 0); // UP
                }
                ToolRunResult::ok("Double click performed", Vec::new())
            }
            "left_click_drag" => {
                if let Some(coords) = coordinate {
                    let x = coords[0].as_i64().unwrap_or(0) as i32;
                    let y = coords[1].as_i64().unwrap_or(0) as i32;
                    unsafe {
                        mouse_event(0x0002, 0, 0, 0, 0); // DOWN
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        SetCursorPos(x, y);
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        mouse_event(0x0004, 0, 0, 0, 0); // UP
                    }
                    ToolRunResult::ok(format!("Left click drag performed to ({}, {})", x, y), Vec::new())
                } else {
                    ToolRunResult::err("action left_click_drag requires coordinate argument", Vec::new())
                }
            }
            "type" => {
                if text.is_empty() {
                    return ToolRunResult::err("action type requires text argument", Vec::new());
                }
                match self.type_text(text) {
                    Ok(_) => ToolRunResult::ok(format!("Typed text: \"{}\"", text), Vec::new()),
                    Err(err) => ToolRunResult::err(format!("Failed to type text: {}", err), Vec::new())
                }
            }
            "key" => {
                if text.is_empty() {
                    return ToolRunResult::err("action key requires key name in text argument", Vec::new());
                }
                match self.press_key(text) {
                    Ok(_) => ToolRunResult::ok(format!("Pressed key: {}", text), Vec::new()),
                    Err(err) => ToolRunResult::err(format!("Failed to press key: {}", err), Vec::new())
                }
            }
            "cursor_position" => {
                let mut pt = POINT { x: 0, y: 0 };
                let success = unsafe { GetCursorPos(&mut pt) };
                if success != 0 {
                    ToolRunResult::ok(format!("Cursor position: [{}, {}]", pt.x, pt.y), Vec::new())
                } else {
                    ToolRunResult::err("Failed to get cursor position", Vec::new())
                }
            }
            _ => ToolRunResult::err(format!("Unknown computer use action: {}", action), Vec::new())
        }
    }

    #[cfg(target_os = "windows")]
    fn capture_screen_jpeg(&self) -> anyhow::Result<(i32, i32, Vec<u8>)> {
        unsafe {
            let width = GetSystemMetrics(0);
            let height = GetSystemMetrics(1);
            if width <= 0 || height <= 0 {
                anyhow::bail!("invalid screen dimensions: {}x{}", width, height);
            }

            let hdc_screen = GetDC(null_mut());
            if hdc_screen.is_null() {
                anyhow::bail!("failed to get screen DC");
            }

            let hdc_mem = CreateCompatibleDC(hdc_screen);
            if hdc_mem.is_null() {
                ReleaseDC(null_mut(), hdc_screen);
                anyhow::bail!("failed to create compatible DC");
            }

            let h_bitmap = CreateCompatibleBitmap(hdc_screen, width, height);
            if h_bitmap.is_null() {
                DeleteDC(hdc_mem);
                ReleaseDC(null_mut(), hdc_screen);
                anyhow::bail!("failed to create compatible bitmap");
            }

            let h_old = SelectObject(hdc_mem, h_bitmap);
            let success = BitBlt(hdc_mem, 0, 0, width, height, hdc_screen, 0, 0, SRCCOPY);
            if success == 0 {
                SelectObject(hdc_mem, h_old);
                DeleteObject(h_bitmap);
                DeleteDC(hdc_mem);
                ReleaseDC(null_mut(), hdc_screen);
                anyhow::bail!("failed BitBlt");
            }

            let mut bmi = BITMAPINFO {
                bmi_header: BITMAPINFOHEADER {
                    bi_size: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    bi_width: width,
                    bi_height: -height,
                    bi_planes: 1,
                    bi_bit_count: 32,
                    bi_compression: BI_RGB,
                    bi_size_image: 0,
                    bi_x_pels_per_meter: 0,
                    bi_y_pels_per_meter: 0,
                    bi_clr_used: 0,
                    bi_clr_important: 0,
                },
                bmi_colors: [RGBQUAD { rgb_blue: 0, rgb_green: 0, rgb_red: 0, rgb_reserved: 0 }],
            };

            let mut buffer = vec![0u8; (width * height * 4) as usize];
            let lines_copied = GetDIBits(
                hdc_mem,
                h_bitmap,
                0,
                height as u32,
                buffer.as_mut_ptr(),
                &mut bmi,
                DIB_RGB_COLORS,
            );

            SelectObject(hdc_mem, h_old);
            DeleteObject(h_bitmap);
            DeleteDC(hdc_mem);
            ReleaseDC(null_mut(), hdc_screen);

            if lines_copied == 0 {
                anyhow::bail!("failed GetDIBits");
            }

            // Convert BGRA to RGBA
            for chunk in buffer.chunks_exact_mut(4) {
                let b = chunk[0];
                let r = chunk[2];
                chunk[0] = r;
                chunk[2] = b;
            }

            // Compress to JPEG using image crate
            use image::ImageEncoder;
            let mut jpeg_bytes = Vec::new();
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_bytes, 85);
            encoder.write_image(
                &buffer,
                width as u32,
                height as u32,
                image::ExtendedColorType::Rgba8,
            )?;

            Ok((width, height, jpeg_bytes))
        }
    }

    #[cfg(target_os = "windows")]
    fn type_text(&self, text: &str) -> anyhow::Result<()> {
        for c in text.encode_utf16() {
            unsafe {
                let res = VkKeyScanW(c);
                if res == -1 {
                    continue;
                }
                let vk = (res & 0xFF) as u8;
                let shift = (res >> 8) & 1 != 0;
                let ctrl = (res >> 8) & 2 != 0;
                let alt = (res >> 8) & 4 != 0;

                if shift { keybd_event(0x10, 0, 0, 0); }
                if ctrl { keybd_event(0x11, 0, 0, 0); }
                if alt { keybd_event(0x12, 0, 0, 0); }

                keybd_event(vk, 0, 0, 0);
                keybd_event(vk, 0, 0x0002, 0);

                if alt { keybd_event(0x12, 0, 0x0002, 0); }
                if ctrl { keybd_event(0x11, 0, 0x0002, 0); }
                if shift { keybd_event(0x10, 0, 0x0002, 0); }

                std::thread::sleep(std::time::Duration::from_millis(15));
            }
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn press_key(&self, key: &str) -> anyhow::Result<()> {
        let parts: Vec<&str> = key.split('+').collect();
        let mut vks = Vec::new();
        for part in parts {
            if let Some(vk) = self.map_key_to_vk(part) {
                vks.push(vk);
            } else {
                anyhow::bail!("unknown key name: {}", part);
            }
        }

        unsafe {
            for &vk in &vks {
                keybd_event(vk, 0, 0, 0);
            }
            for &vk in vks.iter().rev() {
                keybd_event(vk, 0, 0x0002, 0);
            }
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn map_key_to_vk(&self, key: &str) -> Option<u8> {
        match key.to_lowercase().as_str() {
            "enter" | "return" => Some(0x0D),
            "esc" | "escape" => Some(0x1B),
            "tab" => Some(0x09),
            "backspace" => Some(0x08),
            "space" => Some(0x20),
            "up" => Some(0x26),
            "down" => Some(0x28),
            "left" => Some(0x25),
            "right" => Some(0x27),
            "pgup" | "pageup" => Some(0x21),
            "pgdn" | "pagedown" => Some(0x22),
            "home" => Some(0x24),
            "end" => Some(0x23),
            "insert" => Some(0x2D),
            "delete" => Some(0x2E),
            "ctrl" | "control" => Some(0x11),
            "alt" => Some(0x12),
            "shift" => Some(0x10),
            "win" | "super" | "command" => Some(0x5B),
            "f1" => Some(0x70),
            "f2" => Some(0x71),
            "f3" => Some(0x72),
            "f4" => Some(0x73),
            "f5" => Some(0x74),
            "f6" => Some(0x75),
            "f7" => Some(0x76),
            "f8" => Some(0x77),
            "f9" => Some(0x78),
            "f10" => Some(0x79),
            "f11" => Some(0x7A),
            "f12" => Some(0x7B),
            _ => {
                if key.len() == 1 {
                    let c = key.chars().next().unwrap();
                    if c.is_ascii_alphabetic() {
                        return Some(c.to_ascii_uppercase() as u8);
                    }
                    if c.is_ascii_digit() {
                        return Some(c as u8);
                    }
                }
                None
            }
        }
    }
}
