use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::Deserialize;
use serde_json::{json, Value};
use sinew_browser::{gif_to_base64, BrowserSession, BrowserSessions, RecordingHandle};
use sinew_core::ToolDescriptor;
use tokio::sync::Mutex;

use crate::tool_names;
use crate::tool_run::{ToolRunImage, ToolRunResult};

const EVAL_OUTPUT_LIMIT: usize = 16 * 1024;
const MAX_CONSOLE_DISPLAY: usize = 200;
const MAX_NETWORK_DISPLAY: usize = 100;
const DOM_CHAR_LIMIT: usize = 32 * 1024;

pub struct BrowserTools {
    sessions: BrowserSessions,
    workspace_id: String,
    recording: Arc<Mutex<Option<RecordingHandle>>>,
}

impl BrowserTools {
    pub fn new(workspace_id: String, sessions: BrowserSessions) -> Self {
        Self {
            sessions,
            workspace_id,
            recording: Arc::new(Mutex::new(None)),
        }
    }

    pub fn all_descriptors(&self) -> Vec<ToolDescriptor> {
        vec![
            descriptor(tool_names::BROWSER_OPEN, "Open a URL in the browser. Launches a new browser session if none is active. Required before any other browser tool.", json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "URL to open (e.g. http://localhost:5173)" },
                    "wait_for": { "type": "string", "enum": ["load", "networkidle", "domcontentloaded"], "description": "When to consider navigation complete. Default: load" },
                    "headless": { "type": "boolean", "description": "Run headless (no visible window). Default: false" }
                },
                "required": ["url"],
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_SCREENSHOT, "Capture a PNG screenshot of the current browser page.", json!({
                "type": "object",
                "properties": {
                    "full_page": { "type": "boolean", "description": "Capture full scrollable page. Default: false" }
                },
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_DOM, "Get a compact accessibility-tree representation of the page. More token-efficient than a screenshot for understanding page structure.", json!({
                "type": "object",
                "properties": {
                    "selector": { "type": "string", "description": "CSS selector to scope the tree to a subtree. Default: entire body" }
                },
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_CLICK, "Click an element on the page.", json!({
                "type": "object",
                "properties": {
                    "selector": { "type": "string", "description": "CSS selector of the element to click" },
                    "button": { "type": "string", "enum": ["left", "right", "middle"], "description": "Mouse button. Default: left" }
                },
                "required": ["selector"],
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_TYPE, "Type text into an input element.", json!({
                "type": "object",
                "properties": {
                    "selector": { "type": "string", "description": "CSS selector of the input element" },
                    "text": { "type": "string", "description": "Text to type" },
                    "clear": { "type": "boolean", "description": "Clear existing value before typing. Default: false" }
                },
                "required": ["selector", "text"],
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_EVAL, "Execute JavaScript in the page context and return the result.", json!({
                "type": "object",
                "properties": {
                    "js": { "type": "string", "description": "JavaScript expression to evaluate. Must be an expression, not statements." }
                },
                "required": ["js"],
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_CONSOLE, "Read console logs captured since the session started.", json!({
                "type": "object",
                "properties": {
                    "clear": { "type": "boolean", "description": "Clear the log buffer after reading. Default: false" }
                },
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_NETWORK, "Read network requests captured since the session started.", json!({
                "type": "object",
                "properties": {
                    "clear": { "type": "boolean", "description": "Clear the request buffer after reading. Default: false" },
                    "filter": { "type": "string", "description": "Filter to URLs containing this string" }
                },
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_WAIT, "Wait for an element to appear or disappear.", json!({
                "type": "object",
                "properties": {
                    "selector": { "type": "string", "description": "CSS selector to wait for" },
                    "state": { "type": "string", "enum": ["visible", "hidden", "attached"], "description": "State to wait for. Default: visible" },
                    "timeout_ms": { "type": "number", "description": "Maximum wait time in ms. Default: 5000" }
                },
                "required": ["selector"],
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_SCROLL, "Scroll the page or scroll an element into view.", json!({
                "type": "object",
                "properties": {
                    "selector": { "type": "string", "description": "CSS selector to scroll into view. Takes priority over x/y." },
                    "x": { "type": "number", "description": "Horizontal scroll position in pixels" },
                    "y": { "type": "number", "description": "Vertical scroll position in pixels" }
                },
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_SELECT, "Select an option in a <select> element.", json!({
                "type": "object",
                "properties": {
                    "selector": { "type": "string", "description": "CSS selector of the <select> element" },
                    "value": { "type": "string", "description": "Value attribute of the <option> to select" }
                },
                "required": ["selector", "value"],
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_HOVER, "Hover the mouse over an element (triggers hover styles and tooltips).", json!({
                "type": "object",
                "properties": {
                    "selector": { "type": "string", "description": "CSS selector of the element to hover" }
                },
                "required": ["selector"],
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_CLOSE, "Close the browser session and free resources.", json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_RECORD_START, "Start recording the browser session as a GIF. Stop with browser_record_stop.", json!({
                "type": "object",
                "properties": {
                    "fps": { "type": "number", "description": "Frames per second (1–10). Default: 4" },
                    "max_duration_s": { "type": "number", "description": "Maximum recording duration in seconds (1–120). Default: 30" }
                },
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_RECORD_STOP, "Stop the active recording and return the result. Format 'mp4' requires ffmpeg installed, falls back to GIF otherwise.", json!({
                "type": "object",
                "properties": {
                    "format": { "type": "string", "enum": ["gif", "mp4"], "description": "Output format. mp4 requires ffmpeg." }
                },
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_RESIZE, "Resize the browser viewport.", json!({
                "type": "object",
                "properties": {
                    "width": { "type": "number", "description": "Viewport width in pixels" },
                    "height": { "type": "number", "description": "Viewport height in pixels" }
                },
                "required": ["width", "height"],
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_BACK, "Navigate back in browser history.", json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_FORWARD, "Navigate forward in browser history.", json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_FIND, "Find elements on the page by visible text, aria-label, placeholder, or title — without needing a CSS selector. Returns matching CSS selectors you can pass to browser_click.", json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "Text to search for (case-insensitive, partial match)" },
                    "role": { "type": "string", "description": "Optional element role/tag filter (e.g. 'button', 'input', 'a')" }
                },
                "required": ["text"],
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_PDF, "Export the current page as a PDF. Returns base64-encoded PDF data.", json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_UPLOAD, "Upload a local file to a file input element on the page.", json!({
                "type": "object",
                "properties": {
                    "selector": { "type": "string", "description": "CSS selector of the input[type=file] element" },
                    "path": { "type": "string", "description": "Absolute path to the local file to upload" }
                },
                "required": ["selector", "path"],
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_COOKIES, "Get, set, or delete cookies for the current page.", json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["get", "set", "delete"], "description": "Operation to perform" },
                    "name": { "type": "string", "description": "Cookie name (required for set/delete)" },
                    "value": { "type": "string", "description": "Cookie value (required for set)" },
                    "domain": { "type": "string", "description": "Cookie domain (required for set, optional for delete)" },
                    "path": { "type": "string", "description": "Cookie path (optional, default /)" },
                    "urls": { "type": "array", "items": { "type": "string" }, "description": "Filter URLs for get action" }
                },
                "required": ["action"],
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_KEYS, "Send special keyboard keys to the page (Enter, Tab, Escape, arrow keys, Ctrl+A, etc.).", json!({
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "Key to press: enter, tab, escape, backspace, delete, arrowup, arrowdown, arrowleft, arrowright, home, end, pageup, pagedown, space, ctrl+a, ctrl+c, ctrl+v" }
                },
                "required": ["key"],
                "additionalProperties": false
            })),
            descriptor(tool_names::BROWSER_IFRAME, "Interact with elements inside an iframe — evaluate JS or click elements that are isolated in an iframe context.", json!({
                "type": "object",
                "properties": {
                    "iframe_selector": { "type": "string", "description": "CSS selector of the iframe element" },
                    "action": { "type": "string", "enum": ["eval", "click"], "description": "Action to perform inside the iframe" },
                    "js": { "type": "string", "description": "JS expression to evaluate inside the iframe (for action=eval)" },
                    "selector": { "type": "string", "description": "CSS selector of the element inside the iframe (for action=click)" }
                },
                "required": ["iframe_selector", "action"],
                "additionalProperties": false
            })),
        ]
    }

    pub async fn run(&self, name: &str, input: Value) -> ToolRunResult {
        match name {
            n if n == tool_names::BROWSER_OPEN => self.run_open(input).await,
            n if n == tool_names::BROWSER_SCREENSHOT => self.run_screenshot(input).await,
            n if n == tool_names::BROWSER_DOM => self.run_dom(input).await,
            n if n == tool_names::BROWSER_CLICK => self.run_click(input).await,
            n if n == tool_names::BROWSER_TYPE => self.run_type(input).await,
            n if n == tool_names::BROWSER_EVAL => self.run_eval(input).await,
            n if n == tool_names::BROWSER_CONSOLE => self.run_console(input).await,
            n if n == tool_names::BROWSER_NETWORK => self.run_network(input).await,
            n if n == tool_names::BROWSER_WAIT => self.run_wait(input).await,
            n if n == tool_names::BROWSER_SCROLL => self.run_scroll(input).await,
            n if n == tool_names::BROWSER_SELECT => self.run_select(input).await,
            n if n == tool_names::BROWSER_HOVER => self.run_hover(input).await,
            n if n == tool_names::BROWSER_CLOSE => self.run_close().await,
            n if n == tool_names::BROWSER_RECORD_START => self.run_record_start(input).await,
            n if n == tool_names::BROWSER_RECORD_STOP => self.run_record_stop(input).await,
            n if n == tool_names::BROWSER_RESIZE => self.run_resize(input).await,
            n if n == tool_names::BROWSER_BACK => self.run_back().await,
            n if n == tool_names::BROWSER_FORWARD => self.run_forward().await,
            n if n == tool_names::BROWSER_FIND => self.run_find(input).await,
            n if n == tool_names::BROWSER_PDF => self.run_pdf().await,
            n if n == tool_names::BROWSER_UPLOAD => self.run_upload(input).await,
            n if n == tool_names::BROWSER_COOKIES => self.run_cookies(input).await,
            n if n == tool_names::BROWSER_KEYS => self.run_keys(input).await,
            n if n == tool_names::BROWSER_IFRAME => self.run_iframe(input).await,
            _ => ToolRunResult::err(format!("unknown browser tool: {name}"), Vec::new()),
        }
    }

    async fn run_open(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize)]
        struct Input {
            url: String,
            #[serde(default = "default_wait_for")]
            wait_for: String,
            #[serde(default)]
            headless: bool,
        }
        fn default_wait_for() -> String { "load".into() }

        let parsed: Input = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => return ToolRunResult::err(e.to_string(), Vec::new()),
        };

        let mut sessions = self.sessions.0.lock().await;
        if !sessions.contains_key(&self.workspace_id) {
            match BrowserSession::launch(parsed.headless).await {
                Ok(session) => { sessions.insert(self.workspace_id.clone(), session); }
                Err(e) => return ToolRunResult::err(e.to_string(), Vec::new()),
            }
        }
        let session = sessions.get_mut(&self.workspace_id).unwrap();
        match session.navigate(&parsed.url, &parsed.wait_for).await {
            Ok(title) => ToolRunResult::ok(
                format!("Opened: {}\nTitle: {}", parsed.url, title),
                Vec::new(),
            ),
            Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
        }
    }

    async fn run_screenshot(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize, Default)]
        struct Input {
            #[serde(default)]
            full_page: bool,
        }
        let parsed: Input = serde_json::from_value(input).unwrap_or_default();
        let mut sessions = self.sessions.0.lock().await;
        let session = match sessions.get_mut(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        match session.screenshot(parsed.full_page).await {
            Ok(bytes) => {
                let (w, h) = png_dimensions(&bytes);
                let dim_str = if w > 0 {
                    format!("{}×{}", w, h)
                } else {
                    (if parsed.full_page { "full page" } else { "viewport" }).to_string()
                };

                let save_path = save_screenshot(&self.workspace_id, &bytes);

                let image = ToolRunImage {
                    media_type: "image/png".into(),
                    data: BASE64.encode(&bytes),
                    path: save_path.clone(),
                };
                let saved_note = match &save_path {
                    Some(p) => format!(" — saved to {}", p),
                    None => String::new(),
                };
                ToolRunResult::ok_with_images(
                    format!("Screenshot captured ({} px, {} bytes){}", dim_str, bytes.len(), saved_note),
                    vec![image],
                    Vec::new(),
                )
            }
            Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
        }
    }

    async fn run_dom(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize, Default)]
        struct Input {
            selector: Option<String>,
        }
        let parsed: Input = serde_json::from_value(input).unwrap_or_default();
        let mut sessions = self.sessions.0.lock().await;
        let session = match sessions.get_mut(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        match session.dom(parsed.selector.as_deref()).await {
            Ok(tree) => {
                let truncated = sinew_browser::recording::gif_to_base64; // use dom module
                let _ = truncated; // suppress unused warning
                let output = if tree.len() > DOM_CHAR_LIMIT {
                    format!("{}\n... (truncated)", &tree[..DOM_CHAR_LIMIT])
                } else {
                    tree
                };
                ToolRunResult::ok(output, Vec::new())
            }
            Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
        }
    }

    async fn run_click(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize)]
        struct Input {
            selector: String,
            #[serde(default = "default_button")]
            button: String,
        }
        fn default_button() -> String { "left".into() }
        let parsed: Input = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => return ToolRunResult::err(e.to_string(), Vec::new()),
        };
        let mut sessions = self.sessions.0.lock().await;
        let session = match sessions.get_mut(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        match session.click(&parsed.selector, &parsed.button).await {
            Ok(()) => ToolRunResult::ok(format!("Clicked {}", parsed.selector), Vec::new()),
            Err(e) => {
                // Auto-screenshot so the agent can see what's actually on the page.
                let screenshot = session.screenshot(false).await.ok();
                let images = match screenshot {
                    Some(bytes) => vec![ToolRunImage {
                        media_type: "image/png".into(),
                        data: BASE64.encode(&bytes),
                        path: None,
                    }],
                    None => Vec::new(),
                };
                ToolRunResult::err_with_images(e.to_string(), images, vec![])
            }
        }
    }

    async fn run_type(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize)]
        struct Input {
            selector: String,
            text: String,
            #[serde(default)]
            clear: bool,
        }
        let parsed: Input = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => return ToolRunResult::err(e.to_string(), Vec::new()),
        };
        let char_count = parsed.text.chars().count();
        let mut sessions = self.sessions.0.lock().await;
        let session = match sessions.get_mut(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        match session.type_text(&parsed.selector, &parsed.text, parsed.clear).await {
            Ok(()) => ToolRunResult::ok(
                format!("Typed {} characters into {}", char_count, parsed.selector),
                Vec::new(),
            ),
            Err(e) => {
                let screenshot = session.screenshot(false).await.ok();
                let images = match screenshot {
                    Some(bytes) => vec![ToolRunImage {
                        media_type: "image/png".into(),
                        data: BASE64.encode(&bytes),
                        path: None,
                    }],
                    None => Vec::new(),
                };
                ToolRunResult::err_with_images(e.to_string(), images, vec![])
            }
        }
    }

    async fn run_eval(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize)]
        struct Input { js: String }
        let parsed: Input = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => return ToolRunResult::err(e.to_string(), Vec::new()),
        };
        let mut sessions = self.sessions.0.lock().await;
        let session = match sessions.get_mut(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        // page.evaluate() sends JS to the Chrome process via CDP — not an Rust eval
        match session.eval(&parsed.js).await {
            Ok(value) => {
                let text = serde_json::to_string_pretty(&value).unwrap_or_default();
                let truncated = if text.len() > EVAL_OUTPUT_LIMIT {
                    format!("{}\n... (truncated)", &text[..EVAL_OUTPUT_LIMIT])
                } else {
                    text
                };
                ToolRunResult::ok(truncated, Vec::new())
            }
            Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
        }
    }

    async fn run_console(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize, Default)]
        struct Input { #[serde(default)] clear: bool }
        let parsed: Input = serde_json::from_value(input).unwrap_or_default();
        let sessions = self.sessions.0.lock().await;
        let session = match sessions.get(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        match session.console(parsed.clear).await {
            Ok(entries) => {
                let displayed = entries.iter().take(MAX_CONSOLE_DISPLAY);
                let text = displayed
                    .map(|e| format!("[{}] {}", e.level.to_uppercase(), e.text))
                    .collect::<Vec<_>>()
                    .join("\n");
                let header = format!("{} console entries\n", entries.len());
                ToolRunResult::ok(format!("{header}{text}"), Vec::new())
            }
            Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
        }
    }

    async fn run_network(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize, Default)]
        struct Input {
            #[serde(default)] clear: bool,
            filter: Option<String>,
        }
        let parsed: Input = serde_json::from_value(input).unwrap_or_default();
        let sessions = self.sessions.0.lock().await;
        let session = match sessions.get(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        match session.network(parsed.clear, parsed.filter.as_deref()).await {
            Ok(entries) => {
                let displayed = entries.iter().take(MAX_NETWORK_DISPLAY);
                let text = displayed
                    .map(|e| {
                        let status = e.status.map(|s| s.to_string()).unwrap_or_else(|| "pending".into());
                        let duration = e.duration_ms.map(|d| format!("{}ms", d)).unwrap_or_default();
                        format!("{} {} {} {}", e.method, e.url, status, duration)
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let header = format!("{} network requests\n", entries.len());
                ToolRunResult::ok(format!("{header}{text}"), Vec::new())
            }
            Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
        }
    }

    async fn run_wait(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize)]
        struct Input {
            selector: String,
            #[serde(default = "default_state")]
            state: String,
            #[serde(default = "default_timeout")]
            timeout_ms: u64,
        }
        fn default_state() -> String { "visible".into() }
        fn default_timeout() -> u64 { 5000 }
        let parsed: Input = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => return ToolRunResult::err(e.to_string(), Vec::new()),
        };
        let mut sessions = self.sessions.0.lock().await;
        let session = match sessions.get_mut(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        match session.wait_for_with_hint(&parsed.selector, &parsed.state, parsed.timeout_ms).await {
            Ok(msg) if msg.starts_with("Timeout") => ToolRunResult::err(msg, Vec::new()),
            Ok(msg) => ToolRunResult::ok(msg, Vec::new()),
            Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
        }
    }

    async fn run_scroll(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize, Default)]
        struct Input {
            selector: Option<String>,
            #[serde(default)] x: f64,
            #[serde(default)] y: f64,
        }
        let parsed: Input = serde_json::from_value(input).unwrap_or_default();
        let mut sessions = self.sessions.0.lock().await;
        let session = match sessions.get_mut(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        match session.scroll(parsed.selector.as_deref(), parsed.x, parsed.y).await {
            Ok(()) => ToolRunResult::ok("Scrolled", Vec::new()),
            Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
        }
    }

    async fn run_select(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize)]
        struct Input { selector: String, value: String }
        let parsed: Input = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => return ToolRunResult::err(e.to_string(), Vec::new()),
        };
        let mut sessions = self.sessions.0.lock().await;
        let session = match sessions.get_mut(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        match session.select(&parsed.selector, &parsed.value).await {
            Ok(()) => ToolRunResult::ok(
                format!("Selected '{}' in {}", parsed.value, parsed.selector),
                Vec::new(),
            ),
            Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
        }
    }

    async fn run_hover(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize)]
        struct Input { selector: String }
        let parsed: Input = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => return ToolRunResult::err(e.to_string(), Vec::new()),
        };
        let mut sessions = self.sessions.0.lock().await;
        let session = match sessions.get_mut(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        match session.hover(&parsed.selector).await {
            Ok(()) => ToolRunResult::ok(format!("Hovering over {}", parsed.selector), Vec::new()),
            Err(e) => {
                let screenshot = session.screenshot(false).await.ok();
                let images = match screenshot {
                    Some(bytes) => vec![ToolRunImage {
                        media_type: "image/png".into(),
                        data: BASE64.encode(&bytes),
                        path: None,
                    }],
                    None => Vec::new(),
                };
                ToolRunResult::err_with_images(e.to_string(), images, vec![])
            }
        }
    }

    async fn run_close(&self) -> ToolRunResult {
        let mut recording = self.recording.lock().await;
        *recording = None;
        let mut sessions = self.sessions.0.lock().await;
        sessions.remove(&self.workspace_id);
        ToolRunResult::ok("Browser session closed", Vec::new())
    }

    async fn run_record_start(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize, Default)]
        struct Input {
            #[serde(default = "default_fps")] fps: u8,
            #[serde(default = "default_max_duration")] max_duration_s: u32,
        }
        fn default_fps() -> u8 { 4 }
        fn default_max_duration() -> u32 { 30 }
        let parsed: Input = serde_json::from_value(input).unwrap_or_default();
        let fps = parsed.fps.clamp(1, 10);
        let max_duration_s = parsed.max_duration_s.clamp(1, 120);

        let sessions = self.sessions.0.lock().await;
        let session = match sessions.get(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        let page = session.page.clone();
        drop(sessions);

        let mut recording = self.recording.lock().await;
        *recording = Some(RecordingHandle::start(page, fps, max_duration_s));
        ToolRunResult::ok(
            format!("Recording started at {fps}fps (max {max_duration_s}s)"),
            Vec::new(),
        )
    }

    async fn run_record_stop(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize, Default)]
        struct Input {
            #[serde(default = "default_format")]
            format: String,
        }
        fn default_format() -> String { "gif".into() }
        let parsed: Input = serde_json::from_value(input).unwrap_or_default();
        let format = parsed.format.to_lowercase();
        let format = if format == "mp4" { "mp4" } else { "gif" };

        let handle = {
            let mut recording = self.recording.lock().await;
            recording.take()
        };
        let Some(handle) = handle else {
            return ToolRunResult::err("No active recording", Vec::new());
        };
        match handle.stop(format).await {
            Ok((bytes, media_type, frame_count, duration_ms)) if frame_count > 0 => {
                let data = gif_to_base64(&bytes);
                let image = ToolRunImage {
                    media_type: media_type.into(),
                    data,
                    path: None,
                };
                ToolRunResult::ok_with_images(
                    format!("{frame_count} frames · {duration_ms}ms · {} KB · {media_type}", bytes.len() / 1024),
                    vec![image],
                    Vec::new(),
                )
            }
            Ok(_) => ToolRunResult::err("Recording stopped but no frames were captured", Vec::new()),
            Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
        }
    }

    async fn run_resize(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize)]
        struct Input { width: u32, height: u32 }
        let parsed: Input = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => return ToolRunResult::err(e.to_string(), Vec::new()),
        };
        let sessions = self.sessions.0.lock().await;
        let session = match sessions.get(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        match session.resize(parsed.width, parsed.height).await {
            Ok(()) => ToolRunResult::ok(
                format!("Viewport resized to {}×{}", parsed.width, parsed.height),
                Vec::new(),
            ),
            Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
        }
    }

    async fn run_back(&self) -> ToolRunResult {
        let mut sessions = self.sessions.0.lock().await;
        let session = match sessions.get_mut(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        match session.back().await {
            Ok(url) => ToolRunResult::ok(format!("Navigated back → {url}"), Vec::new()),
            Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
        }
    }

    async fn run_forward(&self) -> ToolRunResult {
        let mut sessions = self.sessions.0.lock().await;
        let session = match sessions.get_mut(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        match session.forward().await {
            Ok(url) => ToolRunResult::ok(format!("Navigated forward → {url}"), Vec::new()),
            Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
        }
    }

    async fn run_find(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize)]
        struct Input {
            text: String,
            role: Option<String>,
        }
        let parsed: Input = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => return ToolRunResult::err(e.to_string(), Vec::new()),
        };
        let mut sessions = self.sessions.0.lock().await;
        let session = match sessions.get_mut(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        match session.find_by_text(&parsed.text, parsed.role.as_deref()).await {
            Ok(selectors) if selectors.is_empty() => ToolRunResult::ok(
                format!("No elements found matching '{}'", parsed.text),
                Vec::new(),
            ),
            Ok(selectors) => ToolRunResult::ok(
                format!(
                    "Found {} element(s) matching '{}':\n{}",
                    selectors.len(),
                    parsed.text,
                    selectors.join("\n")
                ),
                Vec::new(),
            ),
            Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
        }
    }

    async fn run_pdf(&self) -> ToolRunResult {
        let mut sessions = self.sessions.0.lock().await;
        let session = match sessions.get_mut(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        match session.pdf().await {
            Ok(bytes) => {
                let path = save_pdf(&self.workspace_id, &bytes);
                match path {
                    Some(p) => ToolRunResult::ok(
                        format!("PDF saved ({} bytes) → {}", bytes.len(), p),
                        Vec::new(),
                    ),
                    None => ToolRunResult::err("Failed to save PDF to disk", Vec::new()),
                }
            }
            Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
        }
    }

    async fn run_upload(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize)]
        struct Input {
            selector: String,
            path: String,
        }
        let parsed: Input = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => return ToolRunResult::err(e.to_string(), Vec::new()),
        };
        let mut sessions = self.sessions.0.lock().await;
        let session = match sessions.get_mut(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        match session.upload_file(&parsed.selector, &parsed.path).await {
            Ok(()) => ToolRunResult::ok(
                format!("File '{}' uploaded to '{}'", parsed.path, parsed.selector),
                Vec::new(),
            ),
            Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
        }
    }

    async fn run_cookies(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize)]
        struct Input {
            action: String,
            name: Option<String>,
            value: Option<String>,
            domain: Option<String>,
            path: Option<String>,
            urls: Option<Vec<String>>,
        }
        let parsed: Input = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => return ToolRunResult::err(e.to_string(), Vec::new()),
        };
        let mut sessions = self.sessions.0.lock().await;
        let session = match sessions.get_mut(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        match parsed.action.as_str() {
            "get" => match session.get_cookies(parsed.urls).await {
                Ok(cookies) => ToolRunResult::ok(
                    format!("{} cookie(s):\n{}", cookies.len(), serde_json::to_string_pretty(&cookies).unwrap_or_default()),
                    Vec::new(),
                ),
                Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
            },
            "set" => {
                let name = match &parsed.name { Some(n) => n.as_str(), None => return ToolRunResult::err("name required for set", Vec::new()) };
                let value = match &parsed.value { Some(v) => v.as_str(), None => return ToolRunResult::err("value required for set", Vec::new()) };
                let domain = match &parsed.domain { Some(d) => d.as_str(), None => return ToolRunResult::err("domain required for set", Vec::new()) };
                match session.set_cookie(name, value, domain, parsed.path.as_deref()).await {
                    Ok(()) => ToolRunResult::ok(format!("Cookie '{name}' set on {domain}"), Vec::new()),
                    Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
                }
            }
            "delete" => {
                let name = match &parsed.name { Some(n) => n.as_str(), None => return ToolRunResult::err("name required for delete", Vec::new()) };
                match session.delete_cookies(name, parsed.domain.as_deref()).await {
                    Ok(()) => ToolRunResult::ok(format!("Cookie '{name}' deleted"), Vec::new()),
                    Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
                }
            }
            other => ToolRunResult::err(format!("unknown action: {other}"), Vec::new()),
        }
    }

    async fn run_keys(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize)]
        struct Input { key: String }
        let parsed: Input = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => return ToolRunResult::err(e.to_string(), Vec::new()),
        };
        let mut sessions = self.sessions.0.lock().await;
        let session = match sessions.get_mut(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        match session.send_keys(&parsed.key).await {
            Ok(()) => ToolRunResult::ok(format!("Key '{}' sent", parsed.key), Vec::new()),
            Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
        }
    }

    async fn run_iframe(&self, input: Value) -> ToolRunResult {
        #[derive(Deserialize)]
        struct Input {
            iframe_selector: String,
            action: String,
            js: Option<String>,
            selector: Option<String>,
        }
        let parsed: Input = match serde_json::from_value(input) {
            Ok(v) => v,
            Err(e) => return ToolRunResult::err(e.to_string(), Vec::new()),
        };
        let mut sessions = self.sessions.0.lock().await;
        let session = match sessions.get_mut(&self.workspace_id) {
            Some(s) => s,
            None => return no_session_error(),
        };
        match parsed.action.as_str() {
            "eval" => {
                let js = match &parsed.js { Some(j) => j.as_str(), None => return ToolRunResult::err("js required for eval", Vec::new()) };
                match session.eval_in_iframe(&parsed.iframe_selector, js).await {
                    Ok(v) => ToolRunResult::ok(v.to_string(), Vec::new()),
                    Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
                }
            }
            "click" => {
                let sel = match &parsed.selector { Some(s) => s.as_str(), None => return ToolRunResult::err("selector required for click", Vec::new()) };
                match session.click_in_iframe(&parsed.iframe_selector, sel).await {
                    Ok(()) => ToolRunResult::ok(format!("Clicked '{}' inside iframe '{}'", sel, parsed.iframe_selector), Vec::new()),
                    Err(e) => ToolRunResult::err(e.to_string(), Vec::new()),
                }
            }
            other => ToolRunResult::err(format!("unknown action: {other}"), Vec::new()),
        }
    }
}

fn png_dimensions(bytes: &[u8]) -> (u32, u32) {
    if bytes.len() < 24 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" {
        return (0, 0);
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().unwrap_or([0; 4]));
    let h = u32::from_be_bytes(bytes[20..24].try_into().unwrap_or([0; 4]));
    (w, h)
}

fn save_pdf(workspace_id: &str, bytes: &[u8]) -> Option<String> {
    let dir = std::path::Path::new(workspace_id).join(".sinew").join("screens");
    std::fs::create_dir_all(&dir).ok()?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();
    let path = dir.join(format!("{ts}.pdf"));
    std::fs::write(&path, bytes).ok()?;
    path.to_str().map(|s| s.trim_start_matches(r"\\?\").to_string())
}

fn save_screenshot(workspace_id: &str, bytes: &[u8]) -> Option<String> {
    let dir = std::path::Path::new(workspace_id).join(".sinew").join("screens");
    std::fs::create_dir_all(&dir).ok()?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();
    let path = dir.join(format!("{ts}.png"));
    std::fs::write(&path, bytes).ok()?;
    path.to_str().map(|s| s.trim_start_matches(r"\\?\").to_string())
}

fn descriptor(name: &str, description: &str, input_schema: Value) -> ToolDescriptor {
    ToolDescriptor {
        name: name.into(),
        description: description.into(),
        input_schema,
    }
}

fn no_session_error() -> ToolRunResult {
    ToolRunResult::err(
        "No active browser session. Call browser_open first.",
        Vec::new(),
    )
}
