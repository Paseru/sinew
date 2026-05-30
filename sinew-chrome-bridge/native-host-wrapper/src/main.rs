#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use serde_json::json;
use futures_util::StreamExt;
use futures_util::sink::SinkExt;

// ---------------------------------------------------------------------------
// Structs & State definitions
// ---------------------------------------------------------------------------

struct WsClient {
    id: u64,
    tx: mpsc::UnboundedSender<String>,
}

enum PendingRequest {
    Http(oneshot::Sender<serde_json::Value>),
    Cdp {
        client_tx: mpsc::UnboundedSender<String>,
        original_id: u64,
        session_id: Option<String>,
    },
}

enum ExtensionConnection {
    Stdio,
    WebSocket(mpsc::UnboundedSender<String>),
}

struct ProxyState {
    extension_conn: RwLock<Option<ExtensionConnection>>,
    native_stdout_tx: mpsc::UnboundedSender<String>,
    pending_requests: Mutex<HashMap<u64, PendingRequest>>,
    next_request_id: Mutex<u64>,
    bridge_secret: String,
    browser_sockets: Mutex<Vec<WsClient>>,
    page_sockets: Mutex<HashMap<String, Vec<WsClient>>>,
    port: u16,
}

struct HttpRequest {
    method: String,
    path: String,
    query: HashMap<String, String>,
    headers: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Main Entrypoint
// ---------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = env::args().collect();
    let is_mcp = args.iter().any(|arg| arg == "--mcp");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    if is_mcp {
        rt.block_on(async {
            let secret = load_or_create_secret();
            let port = get_bridge_port();
            run_mcp_server(secret, port).await;
        });
    } else {
        rt.block_on(async {
            let secret = load_or_create_secret();
            let port = get_bridge_port();
            run_bridge_proxy(secret, port).await;
        });
    }
}

// ---------------------------------------------------------------------------
// Configuration & Helpers
// ---------------------------------------------------------------------------

fn get_bridge_port() -> u16 {
    env::var("SINEW_CHROME_BRIDGE_PORT")
        .ok()
        .and_then(|val| val.parse::<u16>().ok())
        .unwrap_or(29002)
}

fn load_or_create_secret() -> String {
    let home = directories::UserDirs::new()
        .map(|dirs| dirs.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let local_app_data = env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join("AppData").join("Local"));
    let state_dir = local_app_data.join("Sinew").join("ChromeBridge");
    let secret_path = state_dir.join("bridge-secret.txt");

    if let Ok(contents) = std::fs::read_to_string(&secret_path) {
        let trimmed = contents.trim().to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }

    let secret = uuid::Uuid::new_v4().to_string();
    let _ = std::fs::create_dir_all(&state_dir);
    if std::fs::write(&secret_path, secret.as_bytes()).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&secret_path, std::fs::Permissions::from_mode(0o600));
        }
    }
    secret
}

fn url_decode(s: &str) -> String {
    let mut res = String::new();
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            let mut hex = String::new();
            if let Some(c1) = chars.next() { hex.push(c1); }
            if let Some(c2) = chars.next() { hex.push(c2); }
            if let Ok(val) = u8::from_str_radix(&hex, 16) {
                res.push(val as char);
            }
        } else if ch == '+' {
            res.push(' ');
        } else {
            res.push(ch);
        }
    }
    res
}

fn parse_http_request(raw: &str) -> Option<HttpRequest> {
    let mut lines = raw.lines();
    let first_line = lines.next()?;
    let mut parts = first_line.split_whitespace();
    let method = parts.next()?.to_string();
    let full_path = parts.next()?.to_string();

    let (path, query) = if let Some(idx) = full_path.find('?') {
        let (p, q) = full_path.split_at(idx);
        let q = &q[1..];
        let mut query_map = HashMap::new();
        for pair in q.split('&') {
            let mut parts = pair.splitn(2, '=');
            if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                query_map.insert(url_decode(k), url_decode(v));
            }
        }
        (p.to_string(), query_map)
    } else {
        (full_path, HashMap::new())
    };

    let mut headers = HashMap::new();
    for line in lines {
        if line.trim().is_empty() {
            break;
        }
        if let Some(idx) = line.find(':') {
            let k = line[..idx].trim().to_lowercase();
            let v = line[idx + 1..].trim().to_string();
            headers.insert(k, v);
        }
    }

    Some(HttpRequest {
        method,
        path,
        query,
        headers,
    })
}

// ---------------------------------------------------------------------------
// Native Host / Bridge Proxy logic
// ---------------------------------------------------------------------------

async fn send_to_extension(state: &ProxyState, msg: &str) -> bool {
    let conn = state.extension_conn.read().await;
    match &*conn {
        Some(ExtensionConnection::Stdio) => {
            state.native_stdout_tx.send(msg.to_string()).is_ok()
        }
        Some(ExtensionConnection::WebSocket(tx)) => {
            tx.send(msg.to_string()).is_ok()
        }
        None => false,
    }
}

async fn run_bridge_proxy(secret: String, port: u16) {
    let (stdout_tx, mut stdout_rx) = mpsc::unbounded_channel::<String>();

    let state = Arc::new(ProxyState {
        extension_conn: RwLock::new(Some(ExtensionConnection::Stdio)),
        native_stdout_tx: stdout_tx,
        pending_requests: Mutex::new(HashMap::new()),
        next_request_id: Mutex::new(1),
        bridge_secret: secret,
        browser_sockets: Mutex::new(Vec::new()),
        page_sockets: Mutex::new(HashMap::new()),
        port,
    });

    // Stdin / Stdout handling tasks
    let state_stdin = state.clone();
    tokio::spawn(async move {
        run_native_host(state_stdin).await;
    });

    tokio::spawn(async move {
        while let Some(msg) = stdout_rx.recv().await {
            let _ = write_native_msg_stdout(&msg).await;
        }
    });

    // Start TCP Listener / HTTP & WebSocket proxy server
    let listener = match TcpListener::bind(format!("127.0.0.1:{port}")).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind to port {port}: {e}");
            return;
        }
    };

    loop {
        if let Ok((stream, _)) = listener.accept().await {
            let state_conn = state.clone();
            tokio::spawn(async move {
                handle_connection(stream, state_conn).await;
            });
        }
    }
}

async fn read_native_msg_stdin(reader: &mut tokio::io::Stdin) -> std::io::Result<Option<String>> {
    let mut len_buf = [0u8; 4];
    if let Err(e) = reader.read_exact(&mut len_buf).await {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            return Ok(None);
        }
        return Err(e);
    }
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut msg_buf = vec![0u8; len];
    reader.read_exact(&mut msg_buf).await?;
    let msg = String::from_utf8(msg_buf)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(Some(msg))
}

async fn write_native_msg_stdout(msg: &str) -> std::io::Result<()> {
    let mut stdout = tokio::io::stdout();
    let len = msg.len() as u32;
    stdout.write_all(&len.to_le_bytes()).await?;
    stdout.write_all(msg.as_bytes()).await?;
    stdout.flush().await?;
    Ok(())
}

async fn run_native_host(state: Arc<ProxyState>) {
    let mut stdin = tokio::io::stdin();

    let init_msg = json!({
        "type": "init_secret",
        "token": state.bridge_secret
    });
    let _ = write_native_msg_stdout(&init_msg.to_string()).await;

    loop {
        match read_native_msg_stdin(&mut stdin).await {
            Ok(Some(msg_str)) => {
                if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&msg_str) {
                    if let Some(msg_type) = msg.get("type").and_then(|t| t.as_str()) {
                        match msg_type {
                            "ping" => {
                                let pong = json!({ "type": "pong" });
                                let _ = write_native_msg_stdout(&pong.to_string()).await;
                            }
                            "response" => {
                                if let Some(id) = msg.get("id").and_then(|i| i.as_u64()) {
                                    let mut pending = state.pending_requests.lock().await;
                                    if let Some(req) = pending.remove(&id) {
                                        let data = msg.get("data").cloned().unwrap_or(json!({}));
                                        match req {
                                            PendingRequest::Http(tx) => {
                                                let _ = tx.send(data);
                                            }
                                            PendingRequest::Cdp {
                                                client_tx,
                                                original_id,
                                                session_id,
                                            } => {
                                                let mut response = json!({
                                                    "id": original_id,
                                                    "result": data.get("result").cloned().unwrap_or(json!({})),
                                                });
                                                if let Some(err) = data.get("error") {
                                                    response["error"] = json!({ "message": err });
                                                }
                                                if let Some(sess) = session_id {
                                                    response["sessionId"] = json!(sess);
                                                }
                                                let _ = client_tx.send(response.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                            "event" => {
                                let tab_id = msg.get("tabId").and_then(|t| t.as_u64()).map(|t| t.to_string())
                                    .or_else(|| msg.get("tabId").and_then(|t| t.as_str()).map(|s| s.to_string()))
                                    .unwrap_or_default();
                                let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
                                let params = msg.get("params").cloned().unwrap_or(json!({}));

                                {
                                    let browser = state.browser_sockets.lock().await;
                                    let payload = json!({
                                        "method": method,
                                        "params": params,
                                        "sessionId": format!("session-{tab_id}")
                                    }).to_string();
                                    for client in browser.iter() {
                                        let _ = client.tx.send(payload.clone());
                                    }
                                }
                                {
                                    let page = state.page_sockets.lock().await;
                                    if let Some(clients) = page.get(&tab_id) {
                                        let payload = json!({
                                            "method": method,
                                            "params": params
                                        }).to_string();
                                        for client in clients.iter() {
                                            let _ = client.tx.send(payload.clone());
                                        }
                                    }
                                }
                            }
                            "target_event" => {
                                let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
                                let tab = msg.get("tab").cloned().unwrap_or(json!({}));
                                let tab_id = tab.get("id").and_then(|t| t.as_u64()).map(|t| t.to_string())
                                    .or_else(|| tab.get("id").and_then(|t| t.as_str()).map(|s| s.to_string()))
                                    .unwrap_or_default();
                                let title = tab.get("title").and_then(|t| t.as_str()).unwrap_or("Chrome Tab");
                                let url = tab.get("url").and_then(|t| t.as_str()).unwrap_or("about:blank");

                                let browser = state.browser_sockets.lock().await;
                                let payload = json!({
                                    "method": method,
                                    "params": {
                                        "targetInfo": {
                                            "targetId": tab_id,
                                            "type": "page",
                                            "title": title,
                                            "url": url,
                                            "attached": false,
                                            "canAccessOpener": false,
                                            "browserContextId": "default"
                                        }
                                    }
                                }).to_string();
                                for client in browser.iter() {
                                    let _ = client.tx.send(payload.clone());
                                }
                            }
                            "target_destroyed" => {
                                let tab_id = msg.get("tabId").and_then(|t| t.as_u64()).map(|t| t.to_string())
                                    .or_else(|| msg.get("tabId").and_then(|t| t.as_str()).map(|s| s.to_string()))
                                    .unwrap_or_default();

                                let browser = state.browser_sockets.lock().await;
                                let payload = json!({
                                    "method": "Target.targetDestroyed",
                                    "params": {
                                        "targetId": tab_id
                                    }
                                }).to_string();
                                for client in browser.iter() {
                                    let _ = client.tx.send(payload.clone());
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(None) => {
                break;
            }
            Err(_) => {
                break;
            }
        }
    }
}

async fn handle_connection(mut stream: TcpStream, state: Arc<ProxyState>) {
    let mut buf = [0u8; 1536];
    let n = match stream.read(&mut buf).await {
        Ok(n) if n > 0 => n,
        _ => return,
    };
    let raw_req = String::from_utf8_lossy(&buf[..n]);
    let req = match parse_http_request(&raw_req) {
        Some(r) => r,
        None => return,
    };

    let is_protected = req.path.starts_with("/api/") || req.path.starts_with("/json") || req.path.starts_with("/devtools/");
    let client_token = req.query.get("token").cloned()
        .or_else(|| req.headers.get("x-sinew-token").cloned());

    if is_protected && client_token.as_deref() != Some(&state.bridge_secret) {
        let response = "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{\"error\":\"Unauthorized: Invalid or missing token\"}";
        let _ = stream.write_all(response.as_bytes()).await;
        return;
    }

    let is_ws = req.headers.get("upgrade").map(|v| v.to_lowercase() == "websocket").unwrap_or(false);
    if is_ws {
        handle_websocket_upgrade(stream, req, state).await;
        return;
    }

    handle_http_request(stream, req, state).await;
}

async fn handle_http_request(mut stream: TcpStream, req: HttpRequest, state: Arc<ProxyState>) {
    let mut headers = String::new();
    headers.push_str("HTTP/1.1 200 OK\r\n");
    headers.push_str("Content-Type: application/json\r\n");
    headers.push_str("Access-Control-Allow-Origin: *\r\n");
    headers.push_str("Access-Control-Allow-Methods: GET, OPTIONS\r\n\r\n");

    if req.method == "OPTIONS" {
        let _ = stream.write_all(headers.as_bytes()).await;
        return;
    }

    if req.path == "/json/version" {
        let version_info = json!({
            "Browser": "Chrome/120.0.0.0",
            "Protocol-Version": "1.3",
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            "V8-Version": "12.0.267",
            "WebKit-Version": "537.36 (@a06414a2754673bc28ea7c71d60dd4d9c7af4718)",
            "webSocketDebuggerUrl": format!("ws://localhost:{}/devtools/browser?token={}", state.port, state.bridge_secret)
        });
        let body = version_info.to_string();
        let _ = stream.write_all(format!("{headers}{body}").as_bytes()).await;
    }
    else if req.path == "/json" || req.path == "/json/list" {
        let id = {
            let mut id_lock = state.next_request_id.lock().await;
            *id_lock += 1;
            *id_lock
        };
        let (tx, rx) = oneshot::channel::<serde_json::Value>();
        {
            let mut pending = state.pending_requests.lock().await;
            pending.insert(id, PendingRequest::Http(tx));
        }

        let cmd = json!({ "id": id, "command": "list_tabs" });
        if send_to_extension(&state, &cmd.to_string()).await {
            match tokio::time::timeout(Duration::from_secs(3), rx).await {
                Ok(Ok(data)) => {
                    let tabs = data.get("tabs").and_then(|t| t.as_array());
                    let mut debug_tabs = Vec::new();
                    if let Some(tabs) = tabs {
                        for t in tabs {
                            let url = t.get("url").and_then(|u| u.as_str()).unwrap_or("");
                            if !url.starts_with("chrome://") && !url.starts_with("chrome-extension://") {
                                let tab_id = t.get("id").and_then(|id| id.as_u64()).map(|id| id.to_string())
                                    .or_else(|| t.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
                                    .unwrap_or_default();
                                debug_tabs.push(json!({
                                    "description": "",
                                    "devtoolsFrontendUrl": format!("devtools://devtools/bundled/js_app.html?experiments=true&v8only=true&ws=localhost:{}/devtools/page/{}&token={}", state.port, tab_id, state.bridge_secret),
                                    "id": tab_id,
                                    "title": t.get("title").and_then(|title| title.as_str()).unwrap_or("Chrome Tab"),
                                    "type": "page",
                                    "url": url,
                                    "active": t.get("active").and_then(|a| a.as_bool()).unwrap_or(false),
                                    "webSocketDebuggerUrl": format!("ws://localhost:{}/devtools/page/{}?token={}", state.port, tab_id, state.bridge_secret)
                                }));
                            }
                        }
                    }
                    let body = json!(debug_tabs).to_string();
                    let _ = stream.write_all(format!("{headers}{body}").as_bytes()).await;
                }
                _ => {
                    let _ = stream.write_all(format!("{headers}[]").as_bytes()).await;
                }
            }
        } else {
            let _ = stream.write_all(format!("{headers}[]").as_bytes()).await;
        }
    }
    else if req.path == "/api/status" {
        let body = json!({
            "isNativeMode": true,
            "extensionConnected": true,
            "hasExtensionSocket": true,
            "chromeExecutable": "chrome",
            "sessions": []
        }).to_string();
        let _ = stream.write_all(format!("{headers}{body}").as_bytes()).await;
    }
    else if req.path.starts_with("/api/") {
        let command = req.path.trim_start_matches("/api/").to_string();

        let id = {
            let mut id_lock = state.next_request_id.lock().await;
            *id_lock += 1;
            *id_lock
        };
        let (tx, rx) = oneshot::channel::<serde_json::Value>();
        {
            let mut pending = state.pending_requests.lock().await;
            pending.insert(id, PendingRequest::Http(tx));
        }

        let mut params = json!({});
        for (k, v) in &req.query {
            if k == "tabId" || k == "timeoutMs" || k == "limit" || k == "index" {
                if let Ok(n) = v.parse::<i64>() {
                    params[k] = json!(n);
                    continue;
                }
            }
            if k == "ctrlKey" || k == "shiftKey" || k == "altKey" || k == "metaKey" || k == "submit" || k == "visible" || k == "scroll" {
                if v == "true" || v == "1" {
                    params[k] = json!(true);
                    continue;
                } else if v == "false" || v == "0" {
                    params[k] = json!(false);
                    continue;
                }
            }
            params[k] = json!(v);
        }

        let cmd = json!({
            "id": id,
            "command": command,
            "params": params
        });

        if send_to_extension(&state, &cmd.to_string()).await {
            let timeout_val = req.query.get("timeoutMs").and_then(|t| t.parse::<u64>().ok()).unwrap_or(20000);
            match tokio::time::timeout(Duration::from_millis(timeout_val + 2000), rx).await {
                Ok(Ok(data)) => {
                    let body = data.to_string();
                    let _ = stream.write_all(format!("{headers}{body}").as_bytes()).await;
                }
                _ => {
                    let err_body = json!({ "success": false, "error": "Timeout waiting for extension response" }).to_string();
                    let _ = stream.write_all(format!("{headers}{err_body}").as_bytes()).await;
                }
            }
        } else {
            let err_body = json!({ "success": false, "error": "Extension not connected" }).to_string();
            let _ = stream.write_all(format!("{headers}{err_body}").as_bytes()).await;
        }
    }
    else {
        let not_found_headers = "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{\"error\":\"Not Found\"}";
        let _ = stream.write_all(not_found_headers.as_bytes()).await;
    }
}

async fn handle_websocket_upgrade(stream: TcpStream, req: HttpRequest, state: Arc<ProxyState>) {
    let ws_stream = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(_) => return,
    };

    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_write.send(tokio_tungstenite::tungstenite::Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    let client_id = CLIENT_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let client = WsClient { id: client_id, tx: tx.clone() };

    if req.path == "/extension" {
        *state.extension_conn.write().await = Some(ExtensionConnection::WebSocket(tx.clone()));
        
        while let Some(Ok(msg)) = ws_read.next().await {
            if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(msg_type) = val.get("type").and_then(|t| t.as_str()) {
                        if msg_type == "ping" {
                            let _ = tx.send(json!({ "type": "pong" }).to_string());
                        } else if msg_type == "response" {
                            if let Some(id) = val.get("id").and_then(|i| i.as_u64()) {
                                let mut pending = state.pending_requests.lock().await;
                                if let Some(req) = pending.remove(&id) {
                                    let data = val.get("data").cloned().unwrap_or(json!({}));
                                    match req {
                                        PendingRequest::Http(otx) => {
                                            let _ = otx.send(data);
                                        }
                                        PendingRequest::Cdp {
                                            client_tx,
                                            original_id,
                                            session_id,
                                        } => {
                                            let mut response = json!({
                                                "id": original_id,
                                                "result": data.get("result").cloned().unwrap_or(json!({})),
                                            });
                                            if let Some(err) = data.get("error") {
                                                response["error"] = json!({ "message": err });
                                            }
                                            if let Some(sess) = session_id {
                                                response["sessionId"] = json!(sess);
                                            }
                                            let _ = client_tx.send(response.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        *state.extension_conn.write().await = None;
    }
    else if req.path == "/devtools/browser" {
        {
            let mut browser = state.browser_sockets.lock().await;
            browser.push(client);
        }

        while let Some(Ok(msg)) = ws_read.next().await {
            if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                handle_browser_cdp_message(text.to_string(), tx.clone(), state.clone()).await;
            }
        }

        {
            let mut browser = state.browser_sockets.lock().await;
            browser.retain(|c| c.id != client_id);
        }
    }
    else if req.path.starts_with("/devtools/page/") {
        let tab_id = req.path.trim_start_matches("/devtools/page/").to_string();
        {
            let mut page = state.page_sockets.lock().await;
            page.entry(tab_id.clone()).or_insert_with(Vec::new).push(client);
        }

        while let Some(Ok(msg)) = ws_read.next().await {
            if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                handle_page_cdp_message(tab_id.clone(), text.to_string(), tx.clone(), state.clone()).await;
            }
        }

        {
            let mut page = state.page_sockets.lock().await;
            if let Some(clients) = page.get_mut(&tab_id) {
                clients.retain(|c| c.id != client_id);
            }
        }
    }
}

static CLIENT_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

async fn handle_browser_cdp_message(msg_str: String, client_tx: mpsc::UnboundedSender<String>, state: Arc<ProxyState>) {
    if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&msg_str) {
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let msg_id = msg.get("id").and_then(|i| i.as_u64()).unwrap_or(0);

        if method == "Browser.getVersion" {
            let res = json!({
                "id": msg_id,
                "result": {
                    "protocolVersion": "1.3",
                    "product": "Chrome/120.0.0.0",
                    "revision": "@a06414a2754673bc28ea7c71d60dd4d9c7af4718",
                    "userAgent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                    "jsVersion": "12.0.267"
                }
            });
            let _ = client_tx.send(res.to_string());
        }
        else if method == "Target.getTargets" {
            let id = {
                let mut id_lock = state.next_request_id.lock().await;
                *id_lock += 1;
                *id_lock
            };
            let (tx, rx) = oneshot::channel::<serde_json::Value>();
            {
                let mut pending = state.pending_requests.lock().await;
                pending.insert(id, PendingRequest::Http(tx));
            }
            let cmd = json!({ "id": id, "command": "list_tabs" });
            if send_to_extension(&state, &cmd.to_string()).await {
                if let Ok(Ok(data)) = tokio::time::timeout(Duration::from_secs(2), rx).await {
                    let tabs = data.get("tabs").and_then(|t| t.as_array());
                    let mut target_infos = Vec::new();
                    if let Some(tabs) = tabs {
                        for t in tabs {
                            let tab_id = t.get("id").and_then(|id| id.as_u64()).map(|id| id.to_string())
                                .or_else(|| t.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
                                .unwrap_or_default();
                            target_infos.push(json!({
                                "targetId": tab_id,
                                "type": "page",
                                "title": t.get("title").and_then(|title| title.as_str()).unwrap_or("Chrome Tab"),
                                "url": t.get("url").and_then(|url| url.as_str()).unwrap_or("about:blank"),
                                "attached": false,
                                "canAccessOpener": false,
                                "browserContextId": "default"
                            }));
                        }
                    }
                    let res = json!({
                        "id": msg_id,
                        "result": { "targetInfos": target_infos }
                    });
                    let _ = client_tx.send(res.to_string());
                }
            }
        }
        else if method == "Target.attachToTarget" {
            let tab_id = msg.get("params").and_then(|p| p.get("targetId")).and_then(|t| t.as_str()).unwrap_or("");
            let session_id = format!("session-{tab_id}");

            let id = {
                let mut id_lock = state.next_request_id.lock().await;
                *id_lock += 1;
                *id_lock
            };
            let cmd = json!({
                "id": id,
                "command": "attach",
                "params": { "tabId": tab_id }
            });
            let _ = send_to_extension(&state, &cmd.to_string()).await;

            let res = json!({
                "id": msg_id,
                "result": { "sessionId": session_id }
            });
            let _ = client_tx.send(res.to_string());

            let event = json!({
                "method": "Target.attachedToTarget",
                "params": {
                    "sessionId": session_id,
                    "targetInfo": {
                        "targetId": tab_id,
                        "type": "page",
                        "title": "Chrome Tab",
                        "url": "about:blank",
                        "attached": true,
                        "canAccessOpener": false,
                        "browserContextId": "default"
                    },
                    "waitingForDebugger": false
                }
            });
            let _ = client_tx.send(event.to_string());
        }
    }
}

async fn handle_page_cdp_message(tab_id: String, msg_str: String, client_tx: mpsc::UnboundedSender<String>, state: Arc<ProxyState>) {
    if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&msg_str) {
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let msg_id = msg.get("id").and_then(|i| i.as_u64()).unwrap_or(0);
        let params = msg.get("params").cloned().unwrap_or(json!({}));

        if method.starts_with("Browser.") {
            if method == "Browser.getVersion" {
                let res = json!({
                    "id": msg_id,
                    "result": {
                        "protocolVersion": "1.3",
                        "product": "Chrome/120.0.0.0",
                        "revision": "@a06414a2754673bc28ea7c71d60dd4d9c7af4718",
                        "userAgent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                        "jsVersion": "12.0.267"
                    }
                });
                let _ = client_tx.send(res.to_string());
            } else {
                let res = json!({ "id": msg_id, "result": {} });
                let _ = client_tx.send(res.to_string());
            }
            return;
        }
        if method == "Target.setAutoAttach" {
            let res = json!({ "id": msg_id, "result": {} });
            let _ = client_tx.send(res.to_string());
            return;
        }
        if method == "Target.getTargetInfo" {
            let res = json!({
                "id": msg_id,
                "result": {
                    "targetInfo": {
                        "targetId": tab_id,
                        "type": "page",
                        "title": "Chrome Tab",
                        "url": "about:blank",
                        "attached": true,
                        "canAccessOpener": false,
                        "browserContextId": "default"
                    }
                }
            });
            let _ = client_tx.send(res.to_string());
            return;
        }

        let id = {
            let mut id_lock = state.next_request_id.lock().await;
            *id_lock += 1;
            *id_lock
        };
        {
            let mut pending = state.pending_requests.lock().await;
            pending.insert(id, PendingRequest::Cdp {
                client_tx: client_tx.clone(),
                original_id: msg_id,
                session_id: msg.get("sessionId").and_then(|s| s.as_str()).map(|s| s.to_string()),
            });
        }

        let cmd = json!({
            "id": id,
            "command": "cdp_command",
            "params": {
                "tabId": tab_id,
                "method": method,
                "cdpParams": params
            }
        });

        if !send_to_extension(&state, &cmd.to_string()).await {
            let mut pending = state.pending_requests.lock().await;
            pending.remove(&id);
            let res = json!({
                "id": msg_id,
                "error": { "message": "Extension not connected" }
            });
            let _ = client_tx.send(res.to_string());
        }
    }
}

// ---------------------------------------------------------------------------
// MCP Server Logic
// ---------------------------------------------------------------------------

async fn run_mcp_server(secret: String, port: u16) {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();

    while let Ok(Some(line)) = reader.next_line().await {
        if let Ok(req) = serde_json::from_str::<serde_json::Value>(&line) {
            let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
            let req_id = req.get("id").cloned();

            match method {
                "initialize" => {
                    let res = json!({
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "result": {
                            "protocolVersion": "2025-06-18",
                            "capabilities": {},
                            "serverInfo": {
                                "name": "sinew-chrome-rust",
                                "version": "1.0.0"
                            }
                        }
                    });
                    println!("{}", res.to_string());
                }
                "tools/list" => {
                    let tools = get_mcp_tools_list();
                    let res = json!({
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "result": { "tools": tools }
                    });
                    println!("{}", res.to_string());
                }
                "tools/call" => {
                    let params = req.get("params");
                    let name = params.and_then(|p| p.get("name")).and_then(|n| n.as_str()).unwrap_or("");
                    let arguments = params.and_then(|p| p.get("arguments")).cloned().unwrap_or(json!({}));

                    let call_res = handle_mcp_tool_call(name, arguments, &secret, port).await;
                    let res = json!({
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "result": call_res
                    });
                    println!("{}", res.to_string());
                }
                _ => {
                    if let Some(id) = req_id {
                        let res = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": { "code": -32601, "message": "Method not found" }
                        });
                        println!("{}", res.to_string());
                    }
                }
            }
        }
    }
}

fn get_mcp_tools_list() -> serde_json::Value {
    json!([
        {
            "name": "open_browser",
            "description": "Ouvre Google Chrome localement vers une URL et prépare un onglet contrôlable. Pour les requêtes de navigation pure, utilisez ceci et arrêtez-vous ; ne cliquez pas après.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "URL optionnelle à ouvrir" }
                }
            }
        },
        {
            "name": "navigate",
            "description": "Navigue l’onglet Chrome contrôlé vers une URL. Pour les requêtes de navigation pure, utilisez ceci et arrêtez-vous.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "URL ou domaine à ouvrir" }
                },
                "required": ["url"]
            }
        },
        {
            "name": "click_selector",
            "description": "TURBO: clique directement sur un sélecteur CSS visible, sans délai de curseur humain. Préféré quand le sélecteur est connu.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "selector": { "type": "string" },
                    "timeoutMs": { "type": "number" },
                    "scroll": { "type": "boolean" }
                },
                "required": ["selector"]
            }
        },
        {
            "name": "type_selector",
            "description": "TURBO: tape du texte directement dans un champ de saisie sélectionné par son sélecteur CSS. Préféré pour saisir du texte.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "selector": { "type": "string" },
                    "text": { "type": "string" },
                    "submit": { "type": "boolean" },
                    "timeoutMs": { "type": "number" }
                },
                "required": ["selector", "text"]
            }
        },
        {
            "name": "page_snapshot",
            "description": "Retourne une capture structurée du DOM des éléments interactifs visibles. À utiliser avant click_selector/type_selector quand le sélecteur est inconnu.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": { "type": "number", "description": "Nombre maximal d'éléments" }
                }
            }
        },
        {
            "name": "click",
            "description": "Clic heuristique par texte visible, label aria, id, classe ou description. Préférer click_selector si le sélecteur est connu.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "Cible à cliquer" },
                    "timeoutMs": { "type": "number" },
                    "cursor": {
                        "type": "object",
                        "properties": {
                            "mode": { "type": "string", "enum": ["visible", "hidden"] },
                            "speed": { "type": "string", "enum": ["slow", "normal", "fast"] }
                        }
                    }
                },
                "required": ["target"]
            }
        },
        {
            "name": "evaluate",
            "description": "Évalue une petite expression JavaScript sur la page active et retourne la valeur sérialisable.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "expression": { "type": "string" },
                    "timeoutMs": { "type": "number" }
                },
                "required": ["expression"]
            }
        },
        {
            "name": "screenshot",
            "description": "Capture une image de l’onglet Chrome actif via CDP local.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "format": { "type": "string", "enum": ["jpeg", "png"] },
                    "quality": { "type": "number" }
                }
            }
        },
        {
            "name": "wait_for_selector",
            "description": "Attend qu'un sélecteur CSS existe/soit visible sur la page.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "selector": { "type": "string" },
                    "visible": { "type": "boolean" },
                    "timeoutMs": { "type": "number" }
                },
                "required": ["selector"]
            }
        },
        {
            "name": "query_selector",
            "description": "Inspecte un sélecteur CSS et retourne ses textes, attributs, visibilité et coordonnées.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "selector": { "type": "string" },
                    "timeoutMs": { "type": "number" },
                    "scroll": { "type": "boolean" }
                },
                "required": ["selector"]
            }
        },
        {
            "name": "wait_for_text",
            "description": "Attend qu'un texte apparaisse sur la page active.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string" },
                    "timeoutMs": { "type": "number" }
                },
                "required": ["text"]
            }
        },
        {
            "name": "get_page_state",
            "description": "Retourne l'état local de la page active.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "select_option",
            "description": "Sélectionne une option dans un élément HTML select par sa valeur, son texte ou son index.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "selector": { "type": "string" },
                    "value": { "type": "string" },
                    "label": { "type": "string" },
                    "index": { "type": "number" },
                    "timeoutMs": { "type": "number" }
                },
                "required": ["selector"]
            }
        },
        {
            "name": "press_key",
            "description": "Simule l'appui d'une touche clavier sur l'élément actif ou le sélecteur spécifié.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "key": { "type": "string" },
                    "selector": { "type": "string" },
                    "code": { "type": "string" },
                    "ctrlKey": { "type": "boolean" },
                    "shiftKey": { "type": "boolean" },
                    "altKey": { "type": "boolean" },
                    "metaKey": { "type": "boolean" },
                    "submit": { "type": "boolean" },
                    "timeoutMs": { "type": "number" }
                },
                "required": ["key"]
            }
        },
        {
            "name": "run_browser_agent",
            "description": "Agent de navigation en langage naturel de secours pour tâches complexes ou ambiguës.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task": { "type": "string", "description": "Description de la tâche à accomplir" },
                    "cursor": {
                        "type": "object",
                        "properties": {
                            "mode": { "type": "string", "enum": ["visible", "hidden"] },
                            "speed": { "type": "string", "enum": ["slow", "normal", "fast"] }
                        }
                    }
                },
                "required": ["task"]
            }
        }
    ])
}

async fn make_api_call(
    client: &reqwest::Client,
    base_url: &str,
    secret: &str,
    tab_id: &str,
    endpoint: &str,
    query: Vec<(&str, String)>,
) -> serde_json::Value {
    let mut request = client.get(format!("{base_url}/api/{endpoint}"))
        .query(&[("token", secret), ("tabId", tab_id)]);
    if !query.is_empty() {
        request = request.query(&query);
    }
    match request.send().await {
        Ok(res) => match res.json::<serde_json::Value>().await {
            Ok(val) => {
                let is_error = val.get("success").and_then(|s| s.as_bool()).map(|s| !s)
                    .or_else(|| val.get("ok").and_then(|o| o.as_bool()).map(|o| !o))
                    .unwrap_or(false);
                json!({
                    "content": [{ "type": "text", "text": val.to_string() }],
                    "isError": is_error
                })
            }
            Err(e) => json!({ "content": [{ "type": "text", "text": format!("Failed to parse response: {e}") }], "isError": true }),
        },
        Err(e) => json!({ "content": [{ "type": "text", "text": format!("API call failed: {e}") }], "isError": true }),
    }
}

async fn handle_mcp_tool_call(name: &str, arguments: serde_json::Value, secret: &str, port: u16) -> serde_json::Value {
    let client = reqwest::Client::new();
    let base_url = format!("http://127.0.0.1:{port}");

    let tabs_url = format!("{}/json?token={}", base_url, secret);
    let tabs_res = match client.get(&tabs_url).send().await {
        Ok(res) => res.json::<serde_json::Value>().await.unwrap_or(json!([])),
        Err(_) => json!([]),
    };

    let active_tab_id = tabs_res.as_array()
        .and_then(|arr| arr.iter().find(|t| t.get("active").and_then(|a| a.as_bool()).unwrap_or(false))
            .or_else(|| arr.first())
        )
        .and_then(|t| t.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()));

    let tab_id = active_tab_id.unwrap_or_else(|| "1".to_string());

    match name {
        "open_browser" => {
            let url = arguments.get("url").and_then(|u| u.as_str()).unwrap_or("https://www.google.com");
            make_api_call(&client, &base_url, secret, &tab_id, "launch_chrome", vec![("url", url.to_string())]).await
        }
        "navigate" => {
            let url = arguments.get("url").and_then(|u| u.as_str()).unwrap_or("");
            make_api_call(&client, &base_url, secret, &tab_id, "navigate_tab", vec![("url", url.to_string())]).await
        }
        "click_selector" => {
            let selector = arguments.get("selector").and_then(|s| s.as_str()).unwrap_or("");
            let scroll = arguments.get("scroll").and_then(|s| s.as_bool()).map(|s| s.to_string()).unwrap_or("true".to_string());
            let timeout = arguments.get("timeoutMs").and_then(|t| t.as_f64()).map(|t| (t as u64).to_string()).unwrap_or("15000".to_string());
            make_api_call(&client, &base_url, secret, &tab_id, "click_selector", vec![
                ("selector", selector.to_string()),
                ("scroll", scroll),
                ("timeoutMs", timeout),
            ]).await
        }
        "type_selector" => {
            let selector = arguments.get("selector").and_then(|s| s.as_str()).unwrap_or("");
            let text = arguments.get("text").and_then(|t| t.as_str()).unwrap_or("");
            let submit = arguments.get("submit").and_then(|s| s.as_bool()).map(|s| s.to_string()).unwrap_or("false".to_string());
            let timeout = arguments.get("timeoutMs").and_then(|t| t.as_f64()).map(|t| (t as u64).to_string()).unwrap_or("15000".to_string());
            make_api_call(&client, &base_url, secret, &tab_id, "type_selector", vec![
                ("selector", selector.to_string()),
                ("text", text.to_string()),
                ("submit", submit),
                ("timeoutMs", timeout),
            ]).await
        }
        "page_snapshot" => {
            let limit = arguments.get("limit").and_then(|l| l.as_f64()).map(|l| (l as u64).to_string()).unwrap_or("80".to_string());
            make_api_call(&client, &base_url, secret, &tab_id, "page_snapshot", vec![("limit", limit)]).await
        }
        "click" => {
            let target = arguments.get("target").and_then(|t| t.as_str()).unwrap_or("");
            let cursor = arguments.get("cursor").cloned().unwrap_or(json!({}));
            make_api_call(&client, &base_url, secret, &tab_id, "human_click", vec![
                ("target", target.to_string()),
                ("cursor", cursor.to_string()),
            ]).await
        }
        "evaluate" => {
            let expression = arguments.get("expression").and_then(|e| e.as_str()).unwrap_or("");
            make_api_call(&client, &base_url, secret, &tab_id, "evaluate", vec![("expression", expression.to_string())]).await
        }
        "screenshot" => {
            let format = arguments.get("format").and_then(|f| f.as_str()).unwrap_or("jpeg");
            let quality = arguments.get("quality").and_then(|q| q.as_f64()).unwrap_or(70.0) as i64;

            let cdp_params = json!({
                "format": format,
                "quality": quality,
                "fromSurface": true
            });

            let request = client.get(format!("{base_url}/api/cdp_command"))
                .query(&[
                    ("token", secret),
                    ("tabId", &tab_id),
                    ("method", "Page.captureScreenshot"),
                    ("cdpParams", &cdp_params.to_string()),
                ]);

            match request.send().await {
                Ok(res) => match res.json::<serde_json::Value>().await {
                    Ok(val) => {
                        let data = val.get("result").and_then(|r| r.get("data")).and_then(|d| d.as_str()).unwrap_or("");
                        if !data.is_empty() {
                            json!({
                                "content": [
                                    { "type": "text", "text": "[image/jpeg]" },
                                    { "type": "image", "mimeType": format!("image/{format}"), "data": data }
                                ],
                                "isError": false
                            })
                        } else {
                            json!({ "content": [{ "type": "text", "text": format!("Screenshot empty: {val}") }], "isError": true })
                        }
                    }
                    Err(e) => json!({ "content": [{ "type": "text", "text": format!("Failed to parse response: {e}") }], "isError": true }),
                },
                Err(e) => json!({ "content": [{ "type": "text", "text": format!("API call failed: {e}") }], "isError": true }),
            }
        }
        "wait_for_selector" => {
            let selector = arguments.get("selector").and_then(|s| s.as_str()).unwrap_or("");
            let visible = arguments.get("visible").and_then(|v| v.as_bool()).map(|v| v.to_string()).unwrap_or("true".to_string());
            let timeout = arguments.get("timeoutMs").and_then(|t| t.as_f64()).map(|t| (t as u64).to_string()).unwrap_or("15000".to_string());
            make_api_call(&client, &base_url, secret, &tab_id, "wait_selector", vec![
                ("selector", selector.to_string()),
                ("visible", visible),
                ("timeoutMs", timeout),
            ]).await
        }
        "query_selector" => {
            let selector = arguments.get("selector").and_then(|s| s.as_str()).unwrap_or("");
            make_api_call(&client, &base_url, secret, &tab_id, "query_selector", vec![("selector", selector.to_string())]).await
        }
        "wait_for_text" => {
            let text = arguments.get("text").and_then(|t| t.as_str()).unwrap_or("");
            let expression = format!(
                "(() => (document.body?.innerText || document.documentElement?.innerText || '').toLowerCase().includes({}))()",
                json!(text.to_lowercase())
            );
            make_api_call(&client, &base_url, secret, &tab_id, "evaluate", vec![("expression", expression)]).await
        }
        "get_page_state" => {
            let expression = r#"(() => ({
                href: location.href,
                title: document.title,
                readyState: document.readyState,
                visibleTextLength: (document.body?.innerText || '').length,
                interactiveCount: document.querySelectorAll('button, a, input, select, textarea, [role="button"], [onclick], article, section').length,
                viewport: { width: window.innerWidth, height: window.innerHeight }
            }))()"#;
            make_api_call(&client, &base_url, secret, &tab_id, "evaluate", vec![("expression", expression.to_string())]).await
        }
        "select_option" => {
            let selector = arguments.get("selector").and_then(|s| s.as_str()).unwrap_or("");
            let value = arguments.get("value").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_default();
            let label = arguments.get("label").and_then(|l| l.as_str()).map(|s| s.to_string()).unwrap_or_default();
            let index = arguments.get("index").and_then(|i| i.as_i64()).map(|i| i.to_string()).unwrap_or_default();
            make_api_call(&client, &base_url, secret, &tab_id, "select_option", vec![
                ("selector", selector.to_string()),
                ("value", value),
                ("label", label),
                ("index", index),
            ]).await
        }
        "press_key" => {
            let key = arguments.get("key").and_then(|k| k.as_str()).unwrap_or("");
            let selector = arguments.get("selector").and_then(|s| s.as_str()).unwrap_or_default();
            let code = arguments.get("code").and_then(|c| c.as_str()).unwrap_or_default();
            let ctrl = arguments.get("ctrlKey").and_then(|b| b.as_bool()).map(|b| b.to_string()).unwrap_or_default();
            let shift = arguments.get("shiftKey").and_then(|b| b.as_bool()).map(|b| b.to_string()).unwrap_or_default();
            let alt = arguments.get("altKey").and_then(|b| b.as_bool()).map(|b| b.to_string()).unwrap_or_default();
            let meta = arguments.get("metaKey").and_then(|b| b.as_bool()).map(|b| b.to_string()).unwrap_or_default();
            let submit = arguments.get("submit").and_then(|b| b.as_bool()).map(|b| b.to_string()).unwrap_or_default();
            make_api_call(&client, &base_url, secret, &tab_id, "press_key", vec![
                ("key", key.to_string()),
                ("selector", selector.to_string()),
                ("code", code.to_string()),
                ("ctrlKey", ctrl),
                ("shiftKey", shift),
                ("altKey", alt),
                ("metaKey", meta),
                ("submit", submit),
            ]).await
        }
        "run_browser_agent" => {
            let task = arguments.get("task").and_then(|t| t.as_str()).unwrap_or("");
            make_api_call(&client, &base_url, secret, &tab_id, "execute_silent_task", vec![("task", task.to_string())]).await
        }
        _ => json!({ "content": [{ "type": "text", "text": format!("Tool {name} not implemented") }], "isError": true }),
    }
}
