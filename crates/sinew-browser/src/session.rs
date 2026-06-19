use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;
use chromiumoxide::cdp::browser_protocol::network::{
    EventRequestWillBeSent, EventResponseReceived, GetCookiesParams, DeleteCookiesParams,
    SetCookieParams, CookieSameSite,
};
use chromiumoxide::cdp::browser_protocol::fetch::{
    EnableParams, EventRequestPaused, FulfillRequestParams, ContinueRequestParams,
    HeaderEntry, RequestPattern, RequestStage,
};
use chromiumoxide::cdp::js_protocol::runtime::EventConsoleApiCalled;
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::Page;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

const CLICK_RETRY_ATTEMPTS: usize = 3;
const CLICK_RETRY_DELAY_MS: u64 = 300;
const CLICK_FIND_TIMEOUT_MS: u64 = 2000;
const SELECTOR_HINT_TIMEOUT_MS: u64 = 3000;
const WAIT_POLL_INTERVAL_MS: u64 = 100;
const MAX_CONSOLE_ENTRIES: usize = 500;
const MAX_NETWORK_ENTRIES: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleEntry {
    pub level: String,
    pub text: String,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEntry {
    pub method: String,
    pub url: String,
    pub status: Option<u16>,
    pub started_ms: u64,
    pub duration_ms: Option<u64>,
}

pub struct BrowserSession {
    _browser: Browser,
    pub page: Page,
    pub console_logs: Arc<Mutex<Vec<ConsoleEntry>>>,
    pub network_requests: Arc<Mutex<Vec<NetworkEntry>>>,
    pub current_url: String,
}

impl BrowserSession {
    pub async fn launch(headless: bool) -> Result<Self> {
        let executable = find_browser_executable()
            .context("No Chrome or Edge found. Install Google Chrome or Microsoft Edge.")?;

        let user_data_dir = std::env::temp_dir().join("sinew-browser-profile");

        let mut builder = BrowserConfig::builder()
            .chrome_executable(executable)
            .user_data_dir(&user_data_dir)
            .arg("--no-sandbox")
            .arg("--disable-blink-features=AutomationControlled")
            .arg("--disable-infobars")
            .arg("--disable-features=AutomationControlled")
            .arg("--window-size=1920,1080")
            .arg("--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36");

        if headless {
            builder = builder.arg("--headless=new");
        }

        let config = builder.build().map_err(|e| anyhow::anyhow!("{e}"))?;
        let (browser, mut handler) = Browser::launch(config).await?;

        tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if let Err(err) = event {
                    // Deserialization failures (unrecognized CDP events from newer browsers)
                    // are non-fatal — the WS connection is still alive. Only None closes the loop.
                    tracing::debug!("browser handler: {err}");
                }
            }
        });

        let page = browser.new_page("about:blank").await?;

        // Inject stealth overrides on every new document to hide CDP automation fingerprint.
        page.execute(
            chromiumoxide::cdp::browser_protocol::page::AddScriptToEvaluateOnNewDocumentParams::builder()
                .source(
                    "Object.defineProperty(navigator,'webdriver',{get:()=>undefined});\
                     delete navigator.__proto__.webdriver;",
                )
                .build()
                .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .await
        .ok();

        let console_logs: Arc<Mutex<Vec<ConsoleEntry>>> = Arc::new(Mutex::new(Vec::new()));
        let network_requests: Arc<Mutex<Vec<NetworkEntry>>> = Arc::new(Mutex::new(Vec::new()));

        subscribe_console(&page, Arc::clone(&console_logs)).await;
        subscribe_network(&page, Arc::clone(&network_requests)).await;

        Ok(Self {
            _browser: browser,
            page,
            console_logs,
            network_requests,
            current_url: String::new(),
        })
    }

    pub async fn navigate(&mut self, url: &str, wait_for: &str) -> Result<String> {
        self.page.goto(url).await?;
        if wait_for == "networkidle" {
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
        self.current_url = url.to_string();
        let title = self
            .page
            .get_title()
            .await
            .unwrap_or_default()
            .unwrap_or_default();
        Ok(title)
    }

    pub async fn screenshot(&self, full_page: bool) -> Result<Vec<u8>> {
        let params = ScreenshotParams::builder()
            .format(chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat::Png)
            .full_page(full_page)
            .build();
        let bytes = self.page.screenshot(params).await?;
        Ok(bytes)
    }

    pub async fn dom(&self, selector: Option<&str>) -> Result<String> {
        // page.evaluate sends JS to Chrome via CDP — not Rust eval
        let js = build_dom_js(selector);
        let result = self
            .page
            .evaluate(js)
            .await?
            .into_value::<String>()
            .unwrap_or_default();
        let header = format!("Page: {}\n", self.current_url);
        Ok(format!("{header}{result}"))
    }

    pub async fn click(&self, selector: &str, _button: &str) -> Result<()> {
        for attempt in 0..CLICK_RETRY_ATTEMPTS {
            let find = tokio::time::timeout(
                Duration::from_millis(CLICK_FIND_TIMEOUT_MS),
                self.page.find_element(selector),
            )
            .await;
            match find {
                Ok(Ok(el)) => {
                    el.click().await?;
                    return Ok(());
                }
                Ok(Err(_)) | Err(_) => {
                    if attempt + 1 == CLICK_RETRY_ATTEMPTS {
                        let hint = tokio::time::timeout(
                            Duration::from_millis(SELECTOR_HINT_TIMEOUT_MS),
                            self.selector_hint(selector),
                        )
                        .await
                        .unwrap_or_else(|_| "hint timed out".into());
                        return Err(anyhow::anyhow!(
                            "Element not found: {selector}\n\n{hint}"
                        ));
                    }
                    tokio::time::sleep(Duration::from_millis(CLICK_RETRY_DELAY_MS)).await;
                }

            }
        }
        Ok(())
    }

    pub async fn type_text(&self, selector: &str, text: &str, clear: bool) -> Result<()> {
        let el = self.page.find_element(selector).await.map_err(|_| {
            // Will be replaced by hint in an async context — use sync fallback here
            anyhow::anyhow!("Element not found: {selector}")
        })?;
        el.click().await?;
        if clear {
            // Select all and delete via JS — page.evaluate sends to Chrome via CDP
            self.page
                .evaluate(
                    "document.activeElement.select && document.activeElement.select(); \
                     document.execCommand && document.execCommand('selectAll'); \
                     document.execCommand && document.execCommand('delete')",
                )
                .await
                .ok();
        }
        // CDP keyboard dispatch with human-like inter-key delays.
        for (i, ch) in text.chars().enumerate() {
            let char_str = ch.to_string();
            self.page
                .execute(
                    chromiumoxide::cdp::browser_protocol::input::DispatchKeyEventParams::builder()
                        .r#type(
                            chromiumoxide::cdp::browser_protocol::input::DispatchKeyEventType::Char,
                        )
                        .text(char_str)
                        .build()
                        .map_err(|e| anyhow::anyhow!("{e}"))?,
                )
                .await?;
            let delay = 35u64 + ((i as u64 * 13 + ch as u64) % 55u64);
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
        Ok(())
    }

    pub async fn eval(&self, js: &str) -> Result<serde_json::Value> {
        // page.evaluate sends JS to the Chrome process via CDP — not a Rust eval
        let result = self.page.evaluate(js).await?;
        let value = result
            .into_value::<serde_json::Value>()
            .unwrap_or(serde_json::Value::Null);
        Ok(value)
    }

    pub async fn console(&self, clear: bool) -> Result<Vec<ConsoleEntry>> {
        let mut logs = self.console_logs.lock().await;
        let entries = logs.clone();
        if clear {
            logs.clear();
        }
        Ok(entries)
    }

    pub async fn network(&self, clear: bool, filter: Option<&str>) -> Result<Vec<NetworkEntry>> {
        let mut requests = self.network_requests.lock().await;
        let entries: Vec<NetworkEntry> = if let Some(pattern) = filter {
            requests.iter().filter(|r| r.url.contains(pattern)).cloned().collect()
        } else {
            requests.clone()
        };
        if clear {
            requests.clear();
        }
        Ok(entries)
    }

    pub async fn wait_for(&self, selector: &str, state: &str, timeout_ms: u64) -> Result<bool> {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            if Instant::now() >= deadline {
                return Ok(false);
            }
            // Evaluate sends JS to Chrome via CDP
            let found = match state {
                "hidden" => self
                    .page
                    .evaluate(format!(
                        "!document.querySelector({:?}) || \
                         getComputedStyle(document.querySelector({:?})).display === 'none'",
                        selector, selector
                    ))
                    .await
                    .ok()
                    .and_then(|r| r.into_value::<bool>().ok())
                    .unwrap_or(false),
                _ => self
                    .page
                    .evaluate(format!("!!document.querySelector({:?})", selector))
                    .await
                    .ok()
                    .and_then(|r| r.into_value::<bool>().ok())
                    .unwrap_or(false),
            };
            if found {
                return Ok(true);
            }
            tokio::time::sleep(Duration::from_millis(WAIT_POLL_INTERVAL_MS)).await;
        }
    }

    pub async fn wait_for_with_hint(&self, selector: &str, state: &str, timeout_ms: u64) -> Result<String> {
        let found = self.wait_for(selector, state, timeout_ms).await?;
        if found {
            Ok(format!("Element '{selector}' is {state}"))
        } else {
            let hint = self.selector_hint(selector).await;
            Ok(format!("Timeout waiting for '{selector}' (state={state})\n\n{hint}"))
        }
    }

    pub async fn scroll(&self, selector: Option<&str>, x: f64, y: f64) -> Result<()> {
        if let Some(sel) = selector {
            self.page
                .evaluate(format!(
                    "document.querySelector({:?})?.scrollIntoView({{behavior:'smooth',block:'center'}})",
                    sel
                ))
                .await?;
        } else {
            self.page
                .evaluate(format!(
                    "window.scrollTo({{left:{x},top:{y},behavior:'smooth'}})"
                ))
                .await?;
        }
        Ok(())
    }

    pub async fn select(&self, selector: &str, value: &str) -> Result<()> {
        self.page
            .evaluate(format!(
                "(function(){{var el=document.querySelector({:?});\
                 if(!el)throw new Error('not found');\
                 el.value={:?};\
                 el.dispatchEvent(new Event('change',{{bubbles:true}}));}})()",
                selector, value
            ))
            .await?;
        Ok(())
    }

    pub async fn hover(&self, selector: &str) -> Result<()> {
        let el = self.page.find_element(selector).await.map_err(|_| {
            anyhow::anyhow!("Element not found: {selector}")
        })?;
        el.hover().await?;
        Ok(())
    }

    pub async fn selector_hint(&self, failed_selector: &str) -> String {
        let js = format!(
            r#"(function(){{
  var sel = {sel:?};
  var out = [];
  var url = location.href;
  var title = document.title;
  out.push('URL: ' + url);
  out.push('Title: ' + title);

  // 1. Try the exact selector and nearby variants
  var directMatch = null;
  try {{ directMatch = document.querySelector(sel); }} catch(e) {{}}
  if (directMatch) {{
    out.push('NOTE: selector EXISTS but element may be hidden, off-screen, or inside iframe');
    var rect = directMatch.getBoundingClientRect();
    out.push('  bounds: ' + Math.round(rect.top) + ',' + Math.round(rect.left) + ' ' + Math.round(rect.width) + 'x' + Math.round(rect.height) + ' visible=' + (rect.width > 0 && rect.height > 0));
  }}

  // 2. All interactive elements with rich selectors
  var interactive = Array.from(document.querySelectorAll(
    'button,a[href],input,select,textarea,[role="button"],[role="link"],[role="tab"],[role="menuitem"],[tabindex],[onclick],[data-testid]'
  )).filter(function(el) {{
    var r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0;
  }});
  out.push('Visible interactive elements (' + interactive.length + '):');
  var shown = 0;
  for (var i = 0; i < interactive.length && shown < 15; i++) {{
    var el = interactive[i];
    var tag = el.tagName.toLowerCase();
    // Build multiple selector forms
    var byId = el.id ? '#' + el.id : null;
    var byTestId = el.getAttribute('data-testid') ? '[data-testid="' + el.getAttribute('data-testid') + '"]' : null;
    var cls = typeof el.className === 'string' && el.className.trim()
      ? '.' + el.className.trim().split(/\s+/).slice(0,3).join('.') : '';
    var role = el.getAttribute('role') ? '[role="' + el.getAttribute('role') + '"]' : '';
    var text = (el.getAttribute('aria-label') || el.getAttribute('placeholder') ||
                el.getAttribute('title') || el.innerText || el.value || '').trim().replace(/\s+/g,' ').slice(0,60);
    var type_ = el.type ? '[type=' + el.type + ']' : '';
    // Parent context (1 level)
    var parent = el.parentElement;
    var parentCtx = parent ? parent.tagName.toLowerCase() + (parent.id ? '#'+parent.id : '') : '';
    var selectors = [];
    if (byId) selectors.push(tag + byId);
    if (byTestId) selectors.push(byTestId);
    if (cls) selectors.push(tag + cls);
    selectors.push(tag + role + type_);
    var line = '  [' + shown + '] ' + selectors.join(' | ');
    if (text) line += ' — "' + text + '"';
    if (parentCtx) line += ' (in ' + parentCtx + ')';
    out.push(line);
    shown++;
  }}
  if (interactive.length > 15) out.push('  ... +' + (interactive.length - 15) + ' more');

  // 3. Loose partial match on the failed selector
  try {{
    var parts = sel.match(/[#\.\[]?[a-zA-Z0-9_-]+/g) || [];
    var candidates = [];
    parts.forEach(function(p) {{
      if (p.length < 3) return;
      var found = Array.from(document.querySelectorAll('[class*="' + p.replace(/[#.\[]/g,'') + '"],[id*="' + p.replace(/[#.\[]/g,'') + '"],[data-testid*="' + p.replace(/[#.\[]/g,'') + '"]')).slice(0,3);
      found.forEach(function(el) {{
        var s = el.tagName.toLowerCase();
        if (el.id) s += '#' + el.id;
        else if (el.getAttribute('data-testid')) s += '[data-testid="'+el.getAttribute('data-testid')+'"]';
        else if (el.className) s += '.' + String(el.className).trim().split(/\s+/)[0];
        candidates.push(s);
      }});
    }});
    if (candidates.length) out.push('Partial matches for "' + sel + '": ' + [...new Set(candidates)].slice(0,5).join(', '));
  }} catch(e) {{}}

  return out.join('\n');
}})()"#,
            sel = failed_selector
        );
        self.page
            .evaluate(js)
            .await
            .ok()
            .and_then(|r| r.into_value::<String>().ok())
            .unwrap_or_else(|| "Could not retrieve page elements.".into())
    }

    pub async fn find_by_text(&self, query: &str, role: Option<&str>) -> Result<Vec<String>> {
        let role_str = role.unwrap_or("");
        let js = format!(
            r#"(function(){{
  var q = {q:?}.toLowerCase();
  var r = {r:?}.toLowerCase();
  var results = [];
  var all = Array.from(document.querySelectorAll('*'));
  for (var i = 0; i < all.length && results.length < 5; i++) {{
    var el = all[i];
    if (el.children.length > 5) continue;
    var txt = (el.getAttribute('aria-label') || el.getAttribute('placeholder') ||
               el.getAttribute('title') || el.innerText || el.value || '').trim().toLowerCase();
    if (!txt.includes(q)) continue;
    if (r) {{
      var elRole = (el.getAttribute('role') || el.tagName).toLowerCase();
      if (!elRole.includes(r)) continue;
    }}
    var sel = el.tagName.toLowerCase();
    if (el.id) sel += '#' + el.id;
    else if (typeof el.className === 'string' && el.className.trim())
      sel += '.' + el.className.trim().split(/\s+/).slice(0,2).join('.');
    results.push(sel);
  }}
  return results;
}})()"#,
            q = query,
            r = role_str
        );
        let result = self.page.evaluate(js).await?;
        Ok(result.into_value::<Vec<String>>().unwrap_or_default())
    }

    pub async fn pdf(&self) -> Result<Vec<u8>> {
        use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
        let result = self
            .page
            .execute(
                chromiumoxide::cdp::browser_protocol::page::PrintToPdfParams::default(),
            )
            .await?;
        let b64: String = serde_json::from_value(
            serde_json::to_value(result.result.data).context("serialize PDF Binary")?,
        )
        .context("deserialize PDF base64 string")?;
        BASE64.decode(&b64).context("decode PDF base64")
    }

    pub async fn upload_file(&self, selector: &str, path: &str) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::dom::{
            GetDocumentParams, QuerySelectorParams, SetFileInputFilesParams,
        };
        let doc = self.page.execute(GetDocumentParams::default()).await?;
        let node_id = doc.result.root.node_id;
        let qs = self
            .page
            .execute(
                QuerySelectorParams::builder()
                    .node_id(node_id)
                    .selector(selector)
                    .build()
                    .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .await?;
        self.page
            .execute(
                SetFileInputFilesParams::builder()
                    .files(vec![path.to_string()])
                    .node_id(qs.result.node_id)
                    .build()
                    .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .await?;
        Ok(())
    }

    pub async fn resize(&self, width: u32, height: u32) -> Result<()> {
        let params = SetDeviceMetricsOverrideParams::builder()
            .width(width as i64)
            .height(height as i64)
            .device_scale_factor(1.0)
            .mobile(false)
            .build()
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        self.page.execute(params).await?;
        Ok(())
    }

    pub async fn back(&mut self) -> Result<String> {
        self.page.evaluate("history.back()").await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        let url = self
            .page
            .evaluate("location.href")
            .await?
            .into_value::<String>()
            .unwrap_or_default();
        self.current_url = url.clone();
        Ok(url)
    }

    pub async fn forward(&mut self) -> Result<String> {
        self.page.evaluate("history.forward()").await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        let url = self
            .page
            .evaluate("location.href")
            .await?
            .into_value::<String>()
            .unwrap_or_default();
        self.current_url = url.clone();
        Ok(url)
    }

    pub async fn get_cookies(&self, urls: Option<Vec<String>>) -> Result<Vec<serde_json::Value>> {
        let mut params = GetCookiesParams::default();
        if let Some(u) = urls {
            params.urls = Some(u);
        }
        let result = self.page.execute(params).await?;
        let cookies: Vec<serde_json::Value> = result
            .result
            .cookies
            .iter()
            .map(|c| serde_json::json!({
                "name": c.name,
                "value": c.value,
                "domain": c.domain,
                "path": c.path,
                "secure": c.secure,
                "http_only": c.http_only,
            }))
            .collect();
        Ok(cookies)
    }

    pub async fn set_cookie(&self, name: &str, value: &str, domain: &str, path: Option<&str>) -> Result<()> {
        let mut params = SetCookieParams::builder()
            .name(name)
            .value(value)
            .domain(domain)
            .build()
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        params.path = path.map(|p| p.to_string());
        self.page.execute(params).await?;
        Ok(())
    }

    pub async fn delete_cookies(&self, name: &str, domain: Option<&str>) -> Result<()> {
        let mut params = DeleteCookiesParams::builder()
            .name(name)
            .build()
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        params.domain = domain.map(|d| d.to_string());
        self.page.execute(params).await?;
        Ok(())
    }

    pub async fn send_keys(&self, key: &str) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::input::{
            DispatchKeyEventParams, DispatchKeyEventType,
        };
        // Map friendly key names to CDP key identifiers.
        let key_lower = key.to_lowercase();
        let (key_id, code, text) = match key_lower.as_str() {
            "enter" | "return" => ("Enter", "Enter", "\r"),
            "tab" => ("Tab", "Tab", "\t"),
            "escape" | "esc" => ("Escape", "Escape", ""),
            "backspace" => ("Backspace", "Backspace", ""),
            "delete" | "del" => ("Delete", "Delete", ""),
            "arrowup" | "up" => ("ArrowUp", "ArrowUp", ""),
            "arrowdown" | "down" => ("ArrowDown", "ArrowDown", ""),
            "arrowleft" | "left" => ("ArrowLeft", "ArrowLeft", ""),
            "arrowright" | "right" => ("ArrowRight", "ArrowRight", ""),
            "home" => ("Home", "Home", ""),
            "end" => ("End", "End", ""),
            "pageup" => ("PageUp", "PageUp", ""),
            "pagedown" => ("PageDown", "PageDown", ""),
            "space" => ("Space", "Space", " "),
            "ctrl+a" => {
                self.page.execute(
                    DispatchKeyEventParams::builder()
                        .r#type(DispatchKeyEventType::KeyDown)
                        .key("a".to_string())
                        .code("KeyA".to_string())
                        .modifiers(2i64)
                        .build().map_err(|e| anyhow::anyhow!("{e}"))?,
                ).await?;
                self.page.execute(
                    DispatchKeyEventParams::builder()
                        .r#type(DispatchKeyEventType::KeyUp)
                        .key("a".to_string())
                        .code("KeyA".to_string())
                        .modifiers(2i64)
                        .build().map_err(|e| anyhow::anyhow!("{e}"))?,
                ).await?;
                return Ok(());
            }
            "ctrl+c" => {
                self.page.evaluate("document.execCommand('copy')").await.ok();
                return Ok(());
            }
            "ctrl+v" => {
                self.page.evaluate("document.execCommand('paste')").await.ok();
                return Ok(());
            }
            other => (other, other, ""),
        };
        self.page.execute(
            DispatchKeyEventParams::builder()
                .r#type(DispatchKeyEventType::KeyDown)
                .key(key_id.to_string())
                .code(code.to_string())
                .build().map_err(|e| anyhow::anyhow!("{e}"))?,
        ).await?;
        if !text.is_empty() {
            self.page.execute(
                DispatchKeyEventParams::builder()
                    .r#type(DispatchKeyEventType::Char)
                    .key(key_id.to_string())
                    .text(text.to_string())
                    .build().map_err(|e| anyhow::anyhow!("{e}"))?,
            ).await?;
        }
        self.page.execute(
            DispatchKeyEventParams::builder()
                .r#type(DispatchKeyEventType::KeyUp)
                .key(key_id.to_string())
                .code(code.to_string())
                .build().map_err(|e| anyhow::anyhow!("{e}"))?,
        ).await?;
        Ok(())
    }

    pub async fn eval_in_iframe(&self, selector: &str, js: &str) -> Result<serde_json::Value> {
        let wrapped = format!(
            r#"(function() {{
  var frame = document.querySelector({sel:?});
  if (!frame) return {{error: 'iframe not found: ' + {sel:?}}};
  try {{
    var doc = frame.contentDocument || frame.contentWindow.document;
    var fn = new frame.contentWindow.Function({js:?});
    return fn();
  }} catch(e) {{
    return {{error: e.toString()}};
  }}
}})()"#,
            sel = selector,
            js = js,
        );
        let result = self.page.evaluate(wrapped).await?;
        Ok(result.into_value::<serde_json::Value>().unwrap_or(serde_json::Value::Null))
    }

    pub async fn click_in_iframe(&self, iframe_selector: &str, element_selector: &str) -> Result<()> {
        let js = format!(
            r#"(function() {{
  var frame = document.querySelector({iframe:?});
  if (!frame) return 'iframe not found';
  var doc = frame.contentDocument || frame.contentWindow.document;
  var el = doc.querySelector({el:?});
  if (!el) return 'element not found in iframe: ' + {el:?};
  el.click();
  return 'ok';
}})()"#,
            iframe = iframe_selector,
            el = element_selector,
        );
        let result = self.page.evaluate(js).await?
            .into_value::<String>()
            .unwrap_or_default();
        if result != "ok" {
            bail!("{result}");
        }
        Ok(())
    }
}

pub struct BrowserSessions(pub Arc<Mutex<HashMap<String, BrowserSession>>>);

impl BrowserSessions {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(HashMap::new())))
    }
}

impl Default for BrowserSessions {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for BrowserSessions {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

async fn subscribe_console(page: &Page, logs: Arc<Mutex<Vec<ConsoleEntry>>>) {
    if let Ok(mut events) = page.event_listener::<EventConsoleApiCalled>().await {
        tokio::spawn(async move {
            while let Some(event) = events.next().await {
                let mut guard = logs.lock().await;
                if guard.len() >= MAX_CONSOLE_ENTRIES {
                    guard.remove(0);
                }
                let text = event
                    .args
                    .iter()
                    .map(|a| {
                        a.value
                            .as_ref()
                            .and_then(|v| {
                                if v.is_string() {
                                    v.as_str().map(|s| s.to_string())
                                } else {
                                    Some(v.to_string())
                                }
                            })
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                guard.push(ConsoleEntry {
                    level: format!("{:?}", event.r#type),
                    text,
                    timestamp_ms: (*event.timestamp.inner() * 1000.0) as u64,
                });
            }
        });
    }
}

async fn subscribe_network(page: &Page, requests: Arc<Mutex<Vec<NetworkEntry>>>) {
    let requests_sent = Arc::clone(&requests);
    let requests_recv = Arc::clone(&requests);

    if let Ok(mut events) = page.event_listener::<EventRequestWillBeSent>().await {
        tokio::spawn(async move {
            while let Some(event) = events.next().await {
                let mut guard = requests_sent.lock().await;
                if guard.len() >= MAX_NETWORK_ENTRIES {
                    guard.remove(0);
                }
                guard.push(NetworkEntry {
                    method: event.request.method.clone(),
                    url: event.request.url.clone(),
                    status: None,
                    started_ms: (*event.timestamp.inner() * 1000.0) as u64,
                    duration_ms: None,
                });
            }
        });
    }

    if let Ok(mut events) = page.event_listener::<EventResponseReceived>().await {
        tokio::spawn(async move {
            while let Some(event) = events.next().await {
                let mut guard = requests_recv.lock().await;
                if let Some(entry) = guard.iter_mut().rev().find(|r| r.url == event.response.url) {
                    entry.status = Some(event.response.status as u16);
                    let now = (*event.timestamp.inner() * 1000.0) as u64;
                    entry.duration_ms = now.checked_sub(entry.started_ms);
                }
            }
        });
    }
}

fn find_browser_executable() -> Option<std::path::PathBuf> {
    let candidates: &[&str] = &[
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        "google-chrome",
        "chromium",
        "chromium-browser",
        "microsoft-edge",
    ];
    for candidate in candidates {
        let path = std::path::Path::new(candidate);
        if path.exists() {
            return Some(path.to_path_buf());
        }
        if !path.is_absolute() {
            if let Ok(found) = which_like(candidate) {
                return Some(found);
            }
        }
    }
    None
}

fn which_like(name: &str) -> Result<std::path::PathBuf> {
    let cmd = if cfg!(windows) { "where" } else { "which" };
    let output = std::process::Command::new(cmd).arg(name).output()?;
    if output.status.success() {
        let path = std::str::from_utf8(&output.stdout)?
            .trim()
            .lines()
            .next()
            .unwrap_or("")
            .trim();
        if !path.is_empty() {
            return Ok(std::path::PathBuf::from(path));
        }
    }
    bail!("not found")
}

fn build_dom_js(selector: Option<&str>) -> String {
    let root = match selector {
        Some(s) => format!("document.querySelector({:?}) || document.body", s),
        None => "document.body".to_string(),
    };
    // JS executed in Chrome via CDP — inspects page DOM structure
    format!(
        r#"(function(){{
  var count=[0],LIMIT=400;
  var INTERACTIVE={{'a':1,'button':1,'input':1,'select':1,'textarea':1,'details':1,'summary':1}};
  var LANDMARK={{'main':1,'nav':1,'header':1,'footer':1,'aside':1,'section':1,'form':1,'article':1}};
  var HEADING=/^h[1-6]$/;
  function walk(el,depth){{
    if(!el||depth>10||count[0]>=LIMIT)return'';
    var tag=(el.tagName||'').toLowerCase();
    var role=el.getAttribute&&el.getAttribute('role')||tag;
    var name=(el.getAttribute&&(el.getAttribute('aria-label')||el.getAttribute('placeholder')||el.getAttribute('alt')))||'';
    if(!name&&INTERACTIVE[tag])name=(el.innerText||el.value||'').trim().substring(0,60);
    if(!name&&HEADING.test(tag))name=(el.innerText||'').trim().substring(0,80);
    var skip=!INTERACTIVE[tag]&&!LANDMARK[tag]&&!HEADING.test(tag)&&!name;
    var children=Array.from(el.children).map(function(c){{return walk(c,depth+1);}}).join('');
    if(skip)return children;
    count[0]++;
    var indent='  '.repeat(depth);
    var extra='';
    if(tag==='input')extra=' [type='+(el.type||'text')+(el.checked?',checked':'')+']';
    if(el.getAttribute&&el.getAttribute('aria-disabled')==='true')extra+=' [disabled]';
    return indent+role+':"'+name.replace(/\n/g,' ').replace(/"/g,"'")+'"'+extra+'\n'+children;
  }}
  var root={root};
  var result=walk(root,0);
  if(count[0]>=LIMIT)result+='...(truncated at '+LIMIT+' nodes)\n';
  return result;
}})()"#,
        root = root
    )
}
