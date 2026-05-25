use crate::*;

pub(super) fn model_with_optional_selection(
    current: &ModelRef,
    model: Option<ModelInput>,
    thinking: Option<ThinkingLevelInput>,
    use_1m_context: Option<bool>,
) -> ModelRef {
    let mut selected = match model {
        Some(model) => {
            let mut m = ModelRef::new(model.provider, model.name);
            m.use_1m_context = model.use_1m_context;
            m
        }
        None => current.clone(),
    };
    if let Some(thinking) = thinking {
        selected.effort = Some(thinking.into_effort());
    }
    if let Some(flag) = use_1m_context {
        selected.use_1m_context = Some(flag);
    }
    selected
}

pub(super) fn provider_registry_snapshot(
    state: &DesktopState,
) -> std::result::Result<HashMap<String, Arc<dyn Provider>>, String> {
    state
        .providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())
        .map(|providers| providers.clone())
}

pub(super) fn provider_from_registry(
    state: &DesktopState,
    provider_id: &str,
) -> std::result::Result<Arc<dyn Provider>, String> {
    state
        .providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?
        .get(provider_id)
        .cloned()
        .ok_or_else(|| format!("provider `{provider_id}` is not configured or missing credentials"))
}

#[tauri::command]
pub(super) fn list_configured_model_providers(
    state: State<'_, DesktopState>,
) -> std::result::Result<Vec<String>, String> {
    let mut providers = state
        .providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    providers.sort();
    Ok(providers)
}

pub(super) fn install_openai_provider(
    providers: &Arc<StdMutex<HashMap<String, Arc<dyn Provider>>>>,
) -> std::result::Result<(), String> {
    let provider = OpenAiProvider::from_default_sources().map_err(error_to_string)?;
    providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?
        .insert("openai".into(), Arc::new(provider) as Arc<dyn Provider>);
    Ok(())
}

pub(super) fn install_anthropic_provider(
    providers: &Arc<StdMutex<HashMap<String, Arc<dyn Provider>>>>,
) -> std::result::Result<(), String> {
    let provider = AnthropicProvider::from_default_sources().map_err(error_to_string)?;
    providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?
        .insert("anthropic".into(), Arc::new(provider) as Arc<dyn Provider>);
    Ok(())
}

pub(super) fn install_google_provider(
    providers: &Arc<StdMutex<HashMap<String, Arc<dyn Provider>>>>,
) -> std::result::Result<(), String> {
    let provider = GoogleProvider::from_default_sources().map_err(error_to_string)?;
    providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?
        .insert("google".into(), Arc::new(provider) as Arc<dyn Provider>);
    Ok(())
}

pub(super) fn install_kimi_provider(
    providers: &Arc<StdMutex<HashMap<String, Arc<dyn Provider>>>>,
) -> std::result::Result<(), String> {
    let provider = KimiProvider::from_default_sources().map_err(error_to_string)?;
    providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?
        .insert("kimi".into(), Arc::new(provider) as Arc<dyn Provider>);
    Ok(())
}

pub(super) fn install_openrouter_provider(
    providers: &Arc<StdMutex<HashMap<String, Arc<dyn Provider>>>>,
    models: &[OpenRouterModelRecord],
) -> std::result::Result<(), String> {
    let provider = OpenRouterProvider::from_default_sources(openrouter_capabilities(models))
        .map_err(error_to_string)?;
    providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?
        .insert(
            OPENROUTER_PROVIDER_ID.into(),
            Arc::new(provider) as Arc<dyn Provider>,
        );
    Ok(())
}

pub(super) fn openrouter_capabilities(models: &[OpenRouterModelRecord]) -> Vec<ModelCapabilities> {
    models
        .iter()
        .map(|model| {
            claakecode_openrouter::capabilities_from_parts(
                &model.id,
                model.context_window,
                model.max_output_tokens,
                model.supports_images,
                model.supports_thinking,
                model.supports_tools,
            )
        })
        .collect()
}

pub(super) fn default_openrouter_model_ref(model: &OpenRouterModelRecord) -> ModelRef {
    let mut model_ref = ModelRef::new(OPENROUTER_PROVIDER_ID, model.id.clone());
    model_ref.effort = Some(if model.supports_thinking {
        Effort::Medium
    } else {
        Effort::None
    });
    model_ref
}

pub(super) fn remove_openai_provider(
    providers: &Arc<StdMutex<HashMap<String, Arc<dyn Provider>>>>,
) -> std::result::Result<(), String> {
    providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?
        .remove("openai");
    Ok(())
}

pub(super) fn remove_anthropic_provider(
    providers: &Arc<StdMutex<HashMap<String, Arc<dyn Provider>>>>,
) -> std::result::Result<(), String> {
    providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?
        .remove("anthropic");
    Ok(())
}

pub(super) fn remove_google_provider(
    providers: &Arc<StdMutex<HashMap<String, Arc<dyn Provider>>>>,
) -> std::result::Result<(), String> {
    providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?
        .remove("google");
    Ok(())
}

pub(super) fn remove_kimi_provider(
    providers: &Arc<StdMutex<HashMap<String, Arc<dyn Provider>>>>,
) -> std::result::Result<(), String> {
    providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?
        .remove("kimi");
    Ok(())
}

pub(super) fn remove_openrouter_provider(
    providers: &Arc<StdMutex<HashMap<String, Arc<dyn Provider>>>>,
) -> std::result::Result<(), String> {
    providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?
        .remove(OPENROUTER_PROVIDER_ID);
    Ok(())
}

pub(super) fn openai_provider_status_from_auth(
    auth: OpenAiAuthStatus,
    connection_state: &str,
    login_id: Option<String>,
    error: Option<String>,
) -> OpenAiProviderStatus {
    OpenAiProviderStatus {
        connected: auth.connected,
        connection_state: connection_state.to_string(),
        email: auth.email,
        account_id: auth.account_id,
        plan_type: auth.plan_type,
        expires_at_ms: auth.expires_at_ms,
        last_refresh_ms: auth.last_refresh_ms,
        login_id,
        error,
    }
}

pub(super) fn anthropic_provider_status_from_auth(
    auth: AnthropicAuthStatus,
    connection_state: &str,
    login_id: Option<String>,
    error: Option<String>,
) -> AnthropicProviderStatus {
    AnthropicProviderStatus {
        connected: auth.connected,
        connection_state: connection_state.to_string(),
        expires_at_ms: auth.expires_at_ms,
        last_refresh_ms: auth.last_refresh_ms,
        login_id,
        error,
    }
}

pub(super) fn google_provider_status_from_auth(
    auth: GoogleAuthStatus,
    connection_state: &str,
    login_id: Option<String>,
    error: Option<String>,
) -> GoogleProviderStatus {
    GoogleProviderStatus {
        connected: auth.connected,
        connection_state: connection_state.to_string(),
        email: auth.email,
        project_id: auth.project_id,
        user_tier: auth.user_tier,
        expires_at_ms: auth.expires_at_ms,
        last_refresh_ms: auth.last_refresh_ms,
        login_id,
        error,
    }
}

pub(super) fn kimi_provider_status_from_auth(
    auth: KimiAuthStatus,
    connection_state: &str,
    login_id: Option<String>,
    error: Option<String>,
) -> KimiProviderStatus {
    KimiProviderStatus {
        connected: auth.connected,
        connection_state: connection_state.to_string(),
        expires_at_ms: auth.expires_at_ms,
        last_refresh_ms: auth.last_refresh_ms,
        login_id,
        error,
    }
}

pub(super) fn openrouter_provider_status_from_auth(
    auth: OpenRouterAuthStatus,
    connection_state: &str,
    model_count: usize,
    error: Option<String>,
) -> OpenRouterProviderStatus {
    OpenRouterProviderStatus {
        connected: auth.connected && connection_state == "connected",
        connection_state: connection_state.to_string(),
        key_preview: auth.key_preview,
        last_validated_ms: auth.last_validated_ms,
        model_count,
        error,
    }
}

pub(super) async fn bind_openai_oauth_listener() -> Result<tokio::net::TcpListener> {
    const DEFAULT_PORT: u16 = 1455;
    const FALLBACK_PORT: u16 = 1457;

    match tokio::net::TcpListener::bind(("127.0.0.1", DEFAULT_PORT)).await {
        Ok(listener) => Ok(listener),
        Err(default_err) => {
            tokio::net::TcpListener::bind(("127.0.0.1", FALLBACK_PORT))
                .await
                .with_context(|| {
                    format!(
                        "unable to bind OAuth callback ports {DEFAULT_PORT} or {FALLBACK_PORT}: {default_err}"
                    )
                })
        }
    }
}

pub(super) async fn run_openai_oauth_server(
    listener: tokio::net::TcpListener,
    redirect_uri: String,
    expected_state: String,
    pkce: PkceCodes,
    cancel: Arc<Notify>,
) -> Result<()> {
    let http = reqwest::Client::builder()
        .user_agent("ClaakeCode/0.1")
        .build()
        .context("unable to build OAuth client")?;

    loop {
        tokio::select! {
            _ = cancel.notified() => {
                anyhow::bail!("Login canceled");
            }
            accepted = listener.accept() => {
                let (mut stream, _) = accepted.context("OAuth callback accept failed")?;
                if let Some(result) = handle_openai_oauth_request(
                    &http,
                    &mut stream,
                    &redirect_uri,
                    &expected_state,
                    &pkce,
                ).await? {
                    return result;
                }
            }
        }
    }
}

pub(super) async fn handle_openai_oauth_request(
    http: &reqwest::Client,
    stream: &mut tokio::net::TcpStream,
    redirect_uri: &str,
    expected_state: &str,
    pkce: &PkceCodes,
) -> Result<Option<Result<()>>> {
    let mut buffer = [0u8; 8192];
    let read = stream
        .read(&mut buffer)
        .await
        .context("OAuth callback read failed")?;
    if read == 0 {
        return Ok(None);
    }

    let request = String::from_utf8_lossy(&buffer[..read]);
    let Some(first_line) = request.lines().next() else {
        write_http_response(stream, 400, "Bad Request", "Bad Request").await?;
        return Ok(None);
    };
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();
    if method != "GET" {
        write_http_response(stream, 405, "Method Not Allowed", "Method Not Allowed").await?;
        return Ok(None);
    }

    let parsed = parse_local_oauth_url(target)?;
    match parsed.path() {
        "/auth/callback" => {
            let params = parsed
                .query_pairs()
                .into_owned()
                .collect::<HashMap<String, String>>();
            if params.get("state").map(String::as_str) != Some(expected_state) {
                write_html_response(stream, 400, openai_login_error_html("State mismatch")).await?;
                return Ok(Some(Err(anyhow::anyhow!("State mismatch"))));
            }
            if let Some(error) = params.get("error") {
                let message = params
                    .get("error_description")
                    .filter(|value| !value.trim().is_empty())
                    .cloned()
                    .unwrap_or_else(|| error.clone());
                write_html_response(stream, 400, openai_login_error_html(&message)).await?;
                return Ok(Some(Err(anyhow::anyhow!(message))));
            }
            let Some(code) = params.get("code").filter(|value| !value.is_empty()) else {
                write_html_response(
                    stream,
                    400,
                    openai_login_error_html("Missing authorization code"),
                )
                .await?;
                return Ok(Some(Err(anyhow::anyhow!("Missing authorization code"))));
            };

            match exchange_oauth_code(http, code, redirect_uri, pkce).await {
                Ok(_) => {
                    write_html_response(stream, 200, openai_login_success_html()).await?;
                    Ok(Some(Ok(())))
                }
                Err(err) => {
                    let message = err.to_string();
                    write_html_response(stream, 500, openai_login_error_html(&message)).await?;
                    Ok(Some(Err(anyhow::anyhow!(message))))
                }
            }
        }
        "/cancel" => {
            write_http_response(stream, 200, "OK", "Login canceled").await?;
            Ok(Some(Err(anyhow::anyhow!("Login canceled"))))
        }
        _ => {
            write_http_response(stream, 404, "Not Found", "Not Found").await?;
            Ok(None)
        }
    }
}

pub(super) async fn bind_anthropic_oauth_listener() -> Result<tokio::net::TcpListener> {
    const CALLBACK_PORT: u16 = 53692;
    tokio::net::TcpListener::bind(("127.0.0.1", CALLBACK_PORT))
        .await
        .context("unable to bind Anthropic OAuth callback port 53692")
}

pub(super) async fn run_anthropic_oauth_server(
    listener: tokio::net::TcpListener,
    redirect_uri: String,
    expected_state: String,
    pkce: AnthropicPkceCodes,
    cancel: Arc<Notify>,
) -> Result<()> {
    let http = reqwest::Client::builder()
        .user_agent("ClaakeCode/0.1")
        .build()
        .context("unable to build OAuth client")?;

    loop {
        tokio::select! {
            _ = cancel.notified() => {
                anyhow::bail!("Login canceled");
            }
            accepted = listener.accept() => {
                let (mut stream, _) = accepted.context("OAuth callback accept failed")?;
                if let Some(result) = handle_anthropic_oauth_request(
                    &http,
                    &mut stream,
                    &redirect_uri,
                    &expected_state,
                    &pkce,
                ).await? {
                    return result;
                }
            }
        }
    }
}

pub(super) async fn handle_anthropic_oauth_request(
    http: &reqwest::Client,
    stream: &mut tokio::net::TcpStream,
    redirect_uri: &str,
    expected_state: &str,
    pkce: &AnthropicPkceCodes,
) -> Result<Option<Result<()>>> {
    let mut buffer = [0u8; 8192];
    let read = stream
        .read(&mut buffer)
        .await
        .context("OAuth callback read failed")?;
    if read == 0 {
        return Ok(None);
    }

    let request = String::from_utf8_lossy(&buffer[..read]);
    let Some(first_line) = request.lines().next() else {
        write_http_response(stream, 400, "Bad Request", "Bad Request").await?;
        return Ok(None);
    };
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();
    if method != "GET" {
        write_http_response(stream, 405, "Method Not Allowed", "Method Not Allowed").await?;
        return Ok(None);
    }

    let parsed = parse_local_oauth_url(target)?;
    match parsed.path() {
        "/callback" => {
            let params = parsed
                .query_pairs()
                .into_owned()
                .collect::<HashMap<String, String>>();
            if let Some(error) = params.get("error") {
                let message = params
                    .get("error_description")
                    .filter(|value| !value.trim().is_empty())
                    .cloned()
                    .unwrap_or_else(|| error.clone());
                write_html_response(stream, 400, openai_login_error_html(&message)).await?;
                return Ok(Some(Err(anyhow::anyhow!(message))));
            }
            if params.get("state").map(String::as_str) != Some(expected_state) {
                write_html_response(stream, 400, openai_login_error_html("State mismatch")).await?;
                return Ok(Some(Err(anyhow::anyhow!("State mismatch"))));
            }
            let Some(code) = params.get("code").filter(|value| !value.is_empty()) else {
                write_html_response(
                    stream,
                    400,
                    openai_login_error_html("Missing authorization code"),
                )
                .await?;
                return Ok(Some(Err(anyhow::anyhow!("Missing authorization code"))));
            };

            match exchange_anthropic_oauth_code(http, code, expected_state, redirect_uri, pkce)
                .await
            {
                Ok(_) => {
                    write_html_response(stream, 200, anthropic_login_success_html()).await?;
                    Ok(Some(Ok(())))
                }
                Err(err) => {
                    let message = err.to_string();
                    write_html_response(stream, 500, openai_login_error_html(&message)).await?;
                    Ok(Some(Err(anyhow::anyhow!(message))))
                }
            }
        }
        "/cancel" => {
            write_http_response(stream, 200, "OK", "Login canceled").await?;
            Ok(Some(Err(anyhow::anyhow!("Login canceled"))))
        }
        _ => {
            write_http_response(stream, 404, "Not Found", "Not Found").await?;
            Ok(None)
        }
    }
}

pub(super) async fn bind_google_oauth_listener() -> Result<tokio::net::TcpListener> {
    // Antigravity expects the redirect URI http://localhost:51121/oauth-callback,
    // so we must bind that exact port (not a random one).
    tokio::net::TcpListener::bind(("127.0.0.1", 51121))
        .await
        .context("unable to bind Google OAuth callback port")
}

pub(super) async fn run_google_oauth_server(
    listener: tokio::net::TcpListener,
    redirect_uri: String,
    expected_state: String,
    pkce: GooglePkceCodes,
    cancel: Arc<Notify>,
) -> Result<()> {
    let http = reqwest::Client::builder()
        .user_agent("ClaakeCode/0.1")
        .build()
        .context("unable to build OAuth client")?;

    loop {
        tokio::select! {
            _ = cancel.notified() => {
                anyhow::bail!("Login canceled");
            }
            accepted = listener.accept() => {
                let (mut stream, _) = accepted.context("OAuth callback accept failed")?;
                if let Some(result) = handle_google_oauth_request(
                    &http,
                    &mut stream,
                    &redirect_uri,
                    &expected_state,
                    &pkce,
                ).await? {
                    return result;
                }
            }
        }
    }
}

pub(super) async fn handle_google_oauth_request(
    http: &reqwest::Client,
    stream: &mut tokio::net::TcpStream,
    redirect_uri: &str,
    expected_state: &str,
    pkce: &GooglePkceCodes,
) -> Result<Option<Result<()>>> {
    let mut buffer = [0u8; 8192];
    let read = stream
        .read(&mut buffer)
        .await
        .context("OAuth callback read failed")?;
    if read == 0 {
        return Ok(None);
    }

    let request = String::from_utf8_lossy(&buffer[..read]);
    let Some(first_line) = request.lines().next() else {
        write_http_response(stream, 400, "Bad Request", "Bad Request").await?;
        return Ok(None);
    };
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();
    if method != "GET" {
        write_http_response(stream, 405, "Method Not Allowed", "Method Not Allowed").await?;
        return Ok(None);
    }

    let parsed = parse_local_oauth_url(target)?;
    match parsed.path() {
        "/oauth-callback" => {
            let params = parsed
                .query_pairs()
                .into_owned()
                .collect::<HashMap<String, String>>();
            if let Some(error) = params.get("error") {
                let message = params
                    .get("error_description")
                    .filter(|value| !value.trim().is_empty())
                    .cloned()
                    .unwrap_or_else(|| error.clone());
                write_html_response(stream, 400, openai_login_error_html(&message)).await?;
                return Ok(Some(Err(anyhow::anyhow!(message))));
            }
            if params.get("state").map(String::as_str) != Some(expected_state) {
                write_html_response(stream, 400, openai_login_error_html("State mismatch")).await?;
                return Ok(Some(Err(anyhow::anyhow!("State mismatch"))));
            }
            let Some(code) = params.get("code").filter(|value| !value.is_empty()) else {
                write_html_response(
                    stream,
                    400,
                    openai_login_error_html("Missing authorization code"),
                )
                .await?;
                return Ok(Some(Err(anyhow::anyhow!("Missing authorization code"))));
            };

            match exchange_google_oauth_code(http, code, redirect_uri, pkce).await {
                Ok(_) => {
                    write_html_response(stream, 200, google_login_success_html()).await?;
                    Ok(Some(Ok(())))
                }
                Err(err) => {
                    let message = err.to_string();
                    write_html_response(stream, 500, openai_login_error_html(&message)).await?;
                    Ok(Some(Err(anyhow::anyhow!(message))))
                }
            }
        }
        "/cancel" => {
            write_http_response(stream, 200, "OK", "Login canceled").await?;
            Ok(Some(Err(anyhow::anyhow!("Login canceled"))))
        }
        _ => {
            write_http_response(stream, 404, "Not Found", "Not Found").await?;
            Ok(None)
        }
    }
}

pub(super) fn parse_local_oauth_url(target: &str) -> Result<url::Url> {
    if target.starts_with('/') {
        url::Url::parse(&format!("http://localhost{target}")).context("invalid OAuth callback URL")
    } else {
        url::Url::parse(target).context("invalid OAuth callback URL")
    }
}

pub(super) async fn write_http_response(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    reason: &str,
    body: &str,
) -> Result<()> {
    write_response(stream, status, reason, "text/plain; charset=utf-8", body).await
}

pub(super) async fn write_html_response(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    body: String,
) -> Result<()> {
    let reason = if status < 400 { "OK" } else { "Error" };
    write_response(stream, status, reason, "text/html; charset=utf-8", &body).await
}

pub(super) async fn write_response(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &str,
) -> Result<()> {
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .await
        .context("OAuth callback write failed")
}

pub(super) fn openai_login_success_html() -> String {
    r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <title>Claake Code connected</title>
    <style>
      body{margin:0;min-height:100vh;display:grid;place-items:center;background:#0a0b0d;color:#f4f4f5;font:15px -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
      main{max-width:420px;padding:32px;text-align:center}
      h1{font-size:22px;margin:0 0 10px}
      p{margin:0;color:#a1a1aa;line-height:1.5}
    </style>
  </head>
  <body><main><h1>OpenAI is connected</h1><p>You can close this tab and return to Claake Code.</p></main></body>
</html>"#
        .to_string()
}

pub(super) fn anthropic_login_success_html() -> String {
    r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <title>Claake Code connected</title>
    <style>
      body{margin:0;min-height:100vh;display:grid;place-items:center;background:#0a0b0d;color:#f4f4f5;font:15px -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
      main{max-width:420px;padding:32px;text-align:center}
      h1{font-size:22px;margin:0 0 10px}
      p{margin:0;color:#a1a1aa;line-height:1.5}
    </style>
  </head>
  <body><main><h1>Anthropic is connected</h1><p>You can close this tab and return to Claake Code.</p></main></body>
</html>"#
        .to_string()
}

pub(super) fn google_login_success_html() -> String {
    r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <title>Claake Code connected</title>
    <style>
      body{margin:0;min-height:100vh;display:grid;place-items:center;background:#0a0b0d;color:#f4f4f5;font:15px -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
      main{max-width:420px;padding:32px;text-align:center}
      h1{font-size:22px;margin:0 0 10px}
      p{margin:0;color:#a1a1aa;line-height:1.5}
    </style>
  </head>
  <body><main><h1>Google is connected</h1><p>You can close this tab and return to Claake Code.</p></main></body>
</html>"#
        .to_string()
}

pub(super) fn openai_login_error_html(message: &str) -> String {
    let escaped = html_escape(message);
    format!(
        r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <title>Claake Code connection failed</title>
    <style>
      body{{margin:0;min-height:100vh;display:grid;place-items:center;background:#0a0b0d;color:#f4f4f5;font:15px -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}}
      main{{max-width:480px;padding:32px;text-align:center}}
      h1{{font-size:22px;margin:0 0 10px}}
      p{{margin:0;color:#a1a1aa;line-height:1.5;overflow-wrap:anywhere}}
    </style>
  </head>
  <body><main><h1>Connection failed</h1><p>{escaped}</p></main></body>
</html>"#
    )
}

pub(super) fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[tauri::command]
pub(super) async fn get_openai_provider_status(
    state: State<'_, DesktopState>,
) -> std::result::Result<OpenAiProviderStatus, String> {
    let mut active_login = state.openai_login.lock().await;
    let attempt = active_login.clone();
    if let Some(attempt) = attempt {
        let outcome = attempt
            .outcome
            .lock()
            .map_err(|_| "login state is unavailable".to_string())?
            .clone();

        if let Some(outcome) = outcome {
            *active_login = None;
            let auth = load_default_auth_status().map_err(error_to_string)?;
            if outcome.success {
                return Ok(openai_provider_status_from_auth(
                    auth,
                    "connected",
                    None,
                    None,
                ));
            }
            return Ok(openai_provider_status_from_auth(
                auth,
                "error",
                None,
                outcome.error,
            ));
        }

        let auth = load_default_auth_status().map_err(error_to_string)?;
        return Ok(openai_provider_status_from_auth(
            auth,
            "connecting",
            Some(attempt.id),
            None,
        ));
    }

    let auth = load_default_auth_status().map_err(error_to_string)?;
    let state = if auth.connected {
        "connected"
    } else {
        "disconnected"
    };
    Ok(openai_provider_status_from_auth(auth, state, None, None))
}

#[tauri::command]
pub(super) async fn start_openai_oauth_login(
    state: State<'_, DesktopState>,
) -> std::result::Result<StartOpenAiLoginOutput, String> {
    if let Some(existing) = state.openai_login.lock().await.take() {
        existing.cancel.notify_one();
    }

    let listener = bind_openai_oauth_listener()
        .await
        .map_err(error_to_string)?;
    let port = listener.local_addr().map_err(error_to_string)?.port();
    let redirect_uri = format!("http://localhost:{port}/auth/callback");
    let pkce = generate_pkce();
    let oauth_state = generate_state();
    let auth_url = oauth_authorize_url(&redirect_uri, &pkce, &oauth_state);
    let login_id = generate_state();
    let cancel = Arc::new(Notify::new());
    let outcome = Arc::new(StdMutex::new(None));

    {
        let mut active_login = state.openai_login.lock().await;
        *active_login = Some(OpenAiLoginAttempt {
            id: login_id.clone(),
            cancel: cancel.clone(),
            outcome: outcome.clone(),
        });
    }

    let providers = state.providers.clone();
    tauri::async_runtime::spawn(async move {
        let result =
            run_openai_oauth_server(listener, redirect_uri, oauth_state, pkce, cancel).await;
        let login_outcome = match result {
            Ok(()) => match install_openai_provider(&providers) {
                Ok(()) => OpenAiLoginOutcome {
                    success: true,
                    error: None,
                },
                Err(err) => OpenAiLoginOutcome {
                    success: false,
                    error: Some(err),
                },
            },
            Err(err) => OpenAiLoginOutcome {
                success: false,
                error: Some(err.to_string()),
            },
        };
        if let Ok(mut slot) = outcome.lock() {
            *slot = Some(login_outcome);
        }
    });

    Ok(StartOpenAiLoginOutput { login_id, auth_url })
}

#[tauri::command]
pub(super) async fn cancel_openai_oauth_login(
    state: State<'_, DesktopState>,
) -> std::result::Result<OpenAiProviderStatus, String> {
    if let Some(attempt) = state.openai_login.lock().await.take() {
        attempt.cancel.notify_one();
    }
    let auth = load_default_auth_status().map_err(error_to_string)?;
    let state = if auth.connected {
        "connected"
    } else {
        "disconnected"
    };
    Ok(openai_provider_status_from_auth(auth, state, None, None))
}

#[tauri::command]
pub(super) async fn disconnect_openai_provider(
    state: State<'_, DesktopState>,
) -> std::result::Result<OpenAiProviderStatus, String> {
    if let Some(attempt) = state.openai_login.lock().await.take() {
        attempt.cancel.notify_one();
    }
    delete_default_auth().map_err(error_to_string)?;
    remove_openai_provider(&state.providers)?;
    let mut tool_settings = state.store.load_tool_settings().map_err(error_to_string)?;
    if tool_settings.openai_image_use_subscription {
        tool_settings.openai_image_use_subscription = false;
        state
            .store
            .save_tool_settings(&tool_settings)
            .map_err(error_to_string)?;
    }
    Ok(openai_provider_status_from_auth(
        OpenAiAuthStatus::disconnected(),
        "disconnected",
        None,
        None,
    ))
}

#[tauri::command]
pub(super) async fn get_anthropic_provider_status(
    state: State<'_, DesktopState>,
) -> std::result::Result<AnthropicProviderStatus, String> {
    let mut active_login = state.anthropic_login.lock().await;
    let attempt = active_login.clone();
    if let Some(attempt) = attempt {
        let outcome = attempt
            .outcome
            .lock()
            .map_err(|_| "login state is unavailable".to_string())?
            .clone();

        if let Some(outcome) = outcome {
            *active_login = None;
            let auth = load_default_anthropic_auth_status().map_err(error_to_string)?;
            if outcome.success {
                return Ok(anthropic_provider_status_from_auth(
                    auth,
                    "connected",
                    None,
                    None,
                ));
            }
            return Ok(anthropic_provider_status_from_auth(
                auth,
                "error",
                None,
                outcome.error,
            ));
        }

        let auth = load_default_anthropic_auth_status().map_err(error_to_string)?;
        return Ok(anthropic_provider_status_from_auth(
            auth,
            "connecting",
            Some(attempt.id),
            None,
        ));
    }

    let auth = load_default_anthropic_auth_status().map_err(error_to_string)?;
    let state = if auth.connected {
        "connected"
    } else {
        "disconnected"
    };
    Ok(anthropic_provider_status_from_auth(auth, state, None, None))
}

#[tauri::command]
pub(super) async fn start_anthropic_oauth_login(
    state: State<'_, DesktopState>,
) -> std::result::Result<StartAnthropicLoginOutput, String> {
    if let Some(existing) = state.anthropic_login.lock().await.take() {
        existing.cancel.notify_one();
    }

    let listener = bind_anthropic_oauth_listener()
        .await
        .map_err(error_to_string)?;
    let port = listener.local_addr().map_err(error_to_string)?.port();
    let redirect_uri = format!("http://localhost:{port}/callback");
    let pkce = generate_anthropic_pkce();
    let oauth_state = pkce.code_verifier.clone();
    let auth_url = anthropic_oauth_authorize_url(&redirect_uri, &pkce, &oauth_state);
    let login_id = generate_anthropic_state();
    let cancel = Arc::new(Notify::new());
    let outcome = Arc::new(StdMutex::new(None));

    {
        let mut active_login = state.anthropic_login.lock().await;
        *active_login = Some(AnthropicLoginAttempt {
            id: login_id.clone(),
            cancel: cancel.clone(),
            outcome: outcome.clone(),
        });
    }

    let providers = state.providers.clone();
    tauri::async_runtime::spawn(async move {
        let result =
            run_anthropic_oauth_server(listener, redirect_uri, oauth_state, pkce, cancel).await;
        let login_outcome = match result {
            Ok(()) => match install_anthropic_provider(&providers) {
                Ok(()) => AnthropicLoginOutcome {
                    success: true,
                    error: None,
                },
                Err(err) => AnthropicLoginOutcome {
                    success: false,
                    error: Some(err),
                },
            },
            Err(err) => AnthropicLoginOutcome {
                success: false,
                error: Some(err.to_string()),
            },
        };
        if let Ok(mut slot) = outcome.lock() {
            *slot = Some(login_outcome);
        }
    });

    Ok(StartAnthropicLoginOutput { login_id, auth_url })
}

#[tauri::command]
pub(super) async fn cancel_anthropic_oauth_login(
    state: State<'_, DesktopState>,
) -> std::result::Result<AnthropicProviderStatus, String> {
    if let Some(attempt) = state.anthropic_login.lock().await.take() {
        attempt.cancel.notify_one();
    }
    let auth = load_default_anthropic_auth_status().map_err(error_to_string)?;
    let state = if auth.connected {
        "connected"
    } else {
        "disconnected"
    };
    Ok(anthropic_provider_status_from_auth(auth, state, None, None))
}

#[tauri::command]
pub(super) async fn disconnect_anthropic_provider(
    state: State<'_, DesktopState>,
) -> std::result::Result<AnthropicProviderStatus, String> {
    if let Some(attempt) = state.anthropic_login.lock().await.take() {
        attempt.cancel.notify_one();
    }
    delete_default_anthropic_auth().map_err(error_to_string)?;
    remove_anthropic_provider(&state.providers)?;
    Ok(anthropic_provider_status_from_auth(
        AnthropicAuthStatus::disconnected(),
        "disconnected",
        None,
        None,
    ))
}

#[tauri::command]
pub(super) async fn get_google_provider_status(
    state: State<'_, DesktopState>,
) -> std::result::Result<GoogleProviderStatus, String> {
    let mut active_login = state.google_login.lock().await;
    let attempt = active_login.clone();
    if let Some(attempt) = attempt {
        let outcome = attempt
            .outcome
            .lock()
            .map_err(|_| "login state is unavailable".to_string())?
            .clone();

        if let Some(outcome) = outcome {
            *active_login = None;
            let auth = load_default_google_auth_status().map_err(error_to_string)?;
            if outcome.success {
                return Ok(google_provider_status_from_auth(
                    auth,
                    "connected",
                    None,
                    None,
                ));
            }
            return Ok(google_provider_status_from_auth(
                auth,
                "error",
                None,
                outcome.error,
            ));
        }

        let auth = load_default_google_auth_status().map_err(error_to_string)?;
        return Ok(google_provider_status_from_auth(
            auth,
            "connecting",
            Some(attempt.id),
            None,
        ));
    }

    let auth = load_default_google_auth_status().map_err(error_to_string)?;
    let state = if auth.connected {
        "connected"
    } else {
        "disconnected"
    };
    Ok(google_provider_status_from_auth(auth, state, None, None))
}

#[tauri::command]
pub(super) async fn start_google_oauth_login(
    state: State<'_, DesktopState>,
) -> std::result::Result<StartGoogleLoginOutput, String> {
    if let Some(existing) = state.google_login.lock().await.take() {
        existing.cancel.notify_one();
    }

    let listener = bind_google_oauth_listener()
        .await
        .map_err(error_to_string)?;
    let port = listener.local_addr().map_err(error_to_string)?.port();
    // Antigravity OAuth client whitelists this exact redirect URI.
    let _ = port;
    let redirect_uri = "http://localhost:51121/oauth-callback".to_string();
    let pkce = generate_google_pkce();
    let oauth_state = generate_google_state();
    let auth_url = google_oauth_authorize_url(&redirect_uri, &pkce, &oauth_state);
    let login_id = generate_google_state();
    let cancel = Arc::new(Notify::new());
    let outcome = Arc::new(StdMutex::new(None));

    {
        let mut active_login = state.google_login.lock().await;
        *active_login = Some(GoogleLoginAttempt {
            id: login_id.clone(),
            cancel: cancel.clone(),
            outcome: outcome.clone(),
        });
    }

    let providers = state.providers.clone();
    tauri::async_runtime::spawn(async move {
        let result =
            run_google_oauth_server(listener, redirect_uri, oauth_state, pkce, cancel).await;
        let login_outcome = match result {
            Ok(()) => match install_google_provider(&providers) {
                Ok(()) => GoogleLoginOutcome {
                    success: true,
                    error: None,
                },
                Err(err) => GoogleLoginOutcome {
                    success: false,
                    error: Some(err),
                },
            },
            Err(err) => GoogleLoginOutcome {
                success: false,
                error: Some(err.to_string()),
            },
        };
        if let Ok(mut slot) = outcome.lock() {
            *slot = Some(login_outcome);
        }
    });

    Ok(StartGoogleLoginOutput { login_id, auth_url })
}

#[tauri::command]
pub(super) async fn cancel_google_oauth_login(
    state: State<'_, DesktopState>,
) -> std::result::Result<GoogleProviderStatus, String> {
    if let Some(attempt) = state.google_login.lock().await.take() {
        attempt.cancel.notify_one();
    }
    let auth = load_default_google_auth_status().map_err(error_to_string)?;
    let state = if auth.connected {
        "connected"
    } else {
        "disconnected"
    };
    Ok(google_provider_status_from_auth(auth, state, None, None))
}

#[tauri::command]
pub(super) async fn disconnect_google_provider(
    state: State<'_, DesktopState>,
) -> std::result::Result<GoogleProviderStatus, String> {
    if let Some(attempt) = state.google_login.lock().await.take() {
        attempt.cancel.notify_one();
    }
    delete_default_google_auth().map_err(error_to_string)?;
    remove_google_provider(&state.providers)?;
    Ok(google_provider_status_from_auth(
        GoogleAuthStatus::disconnected(),
        "disconnected",
        None,
        None,
    ))
}

#[tauri::command]
pub(super) async fn get_kimi_provider_status(
    state: State<'_, DesktopState>,
) -> std::result::Result<KimiProviderStatus, String> {
    let mut active_login = state.kimi_login.lock().await;
    let attempt = active_login.clone();
    if let Some(attempt) = attempt {
        let outcome = attempt
            .outcome
            .lock()
            .map_err(|_| "login state is unavailable".to_string())?
            .clone();

        if let Some(outcome) = outcome {
            *active_login = None;
            let auth = load_default_kimi_auth_status().map_err(error_to_string)?;
            if outcome.success {
                return Ok(kimi_provider_status_from_auth(
                    auth,
                    "connected",
                    None,
                    None,
                ));
            }
            return Ok(kimi_provider_status_from_auth(
                auth,
                "error",
                None,
                outcome.error,
            ));
        }

        let auth = load_default_kimi_auth_status().map_err(error_to_string)?;
        return Ok(kimi_provider_status_from_auth(
            auth,
            "connecting",
            Some(attempt.id),
            None,
        ));
    }

    let auth = load_default_kimi_auth_status().map_err(error_to_string)?;
    let state = if auth.connected {
        "connected"
    } else {
        "disconnected"
    };
    Ok(kimi_provider_status_from_auth(auth, state, None, None))
}

#[tauri::command]
pub(super) async fn start_kimi_oauth_login(
    state: State<'_, DesktopState>,
) -> std::result::Result<StartKimiLoginOutput, String> {
    if let Some(existing) = state.kimi_login.lock().await.take() {
        existing.cancel.notify_one();
    }

    let http = reqwest::Client::builder()
        .user_agent("ClaakeCode/0.1")
        .build()
        .map_err(error_to_string)?;
    let auth = request_kimi_device_authorization(&http)
        .await
        .map_err(error_to_string)?;
    let login_id = generate_kimi_state();
    let auth_url = auth.verification_uri_complete.clone();
    let user_code = auth.user_code.clone();
    let cancel = Arc::new(Notify::new());
    let outcome = Arc::new(StdMutex::new(None));

    {
        let mut active_login = state.kimi_login.lock().await;
        *active_login = Some(KimiLoginAttempt {
            id: login_id.clone(),
            cancel: cancel.clone(),
            outcome: outcome.clone(),
        });
    }

    let providers = state.providers.clone();
    tauri::async_runtime::spawn(async move {
        let result = run_kimi_device_login(http, auth, cancel).await;
        let login_outcome = match result {
            Ok(()) => match install_kimi_provider(&providers) {
                Ok(()) => KimiLoginOutcome {
                    success: true,
                    error: None,
                },
                Err(err) => KimiLoginOutcome {
                    success: false,
                    error: Some(err),
                },
            },
            Err(err) => KimiLoginOutcome {
                success: false,
                error: Some(err.to_string()),
            },
        };
        if let Ok(mut slot) = outcome.lock() {
            *slot = Some(login_outcome);
        }
    });

    Ok(StartKimiLoginOutput {
        login_id,
        auth_url,
        user_code,
    })
}

pub(super) async fn run_kimi_device_login(
    http: reqwest::Client,
    auth: KimiDeviceAuthorization,
    cancel: Arc<Notify>,
) -> Result<()> {
    tokio::select! {
        _ = cancel.notified() => {
            anyhow::bail!("Login canceled");
        }
        result = wait_for_kimi_device_token(&http, &auth) => {
            result.map(|_| ()).map_err(|err| anyhow::anyhow!(err.to_string()))
        }
    }
}

#[tauri::command]
pub(super) async fn cancel_kimi_oauth_login(
    state: State<'_, DesktopState>,
) -> std::result::Result<KimiProviderStatus, String> {
    if let Some(attempt) = state.kimi_login.lock().await.take() {
        attempt.cancel.notify_one();
    }
    let auth = load_default_kimi_auth_status().map_err(error_to_string)?;
    let state = if auth.connected {
        "connected"
    } else {
        "disconnected"
    };
    Ok(kimi_provider_status_from_auth(auth, state, None, None))
}

#[tauri::command]
pub(super) async fn disconnect_kimi_provider(
    state: State<'_, DesktopState>,
) -> std::result::Result<KimiProviderStatus, String> {
    if let Some(attempt) = state.kimi_login.lock().await.take() {
        attempt.cancel.notify_one();
    }
    delete_default_kimi_auth().map_err(error_to_string)?;
    remove_kimi_provider(&state.providers)?;
    Ok(kimi_provider_status_from_auth(
        KimiAuthStatus::disconnected(),
        "disconnected",
        None,
        None,
    ))
}

#[tauri::command]
pub(super) async fn get_openrouter_provider_status(
    state: State<'_, DesktopState>,
) -> std::result::Result<OpenRouterProviderStatus, String> {
    let model_count = state
        .store
        .load_openrouter_models()
        .map_err(error_to_string)?
        .len();
    let auth = load_default_openrouter_auth_status().map_err(error_to_string)?;
    let Some(api_key) = load_default_openrouter_api_key().map_err(error_to_string)? else {
        remove_openrouter_provider(&state.providers)?;
        return Ok(openrouter_provider_status_from_auth(
            auth,
            "disconnected",
            model_count,
            None,
        ));
    };

    match validate_openrouter_api_key_remote(&api_key).await {
        Ok(()) => {
            let auth = touch_default_openrouter_auth_validation().map_err(error_to_string)?;
            let models = state
                .store
                .load_openrouter_models()
                .map_err(error_to_string)?;
            install_openrouter_provider(&state.providers, &models)?;
            Ok(openrouter_provider_status_from_auth(
                auth,
                "connected",
                models.len(),
                None,
            ))
        }
        Err(err) => {
            remove_openrouter_provider(&state.providers)?;
            Ok(openrouter_provider_status_from_auth(
                auth,
                "error",
                model_count,
                Some(err.to_string()),
            ))
        }
    }
}

#[tauri::command]
pub(super) async fn validate_openrouter_api_key(
    state: State<'_, DesktopState>,
    input: ValidateOpenRouterApiKeyInput,
) -> std::result::Result<OpenRouterProviderStatus, String> {
    let api_key = input.api_key.trim().to_string();
    if api_key.is_empty() {
        return Ok(openrouter_provider_status_from_auth(
            OpenRouterAuthStatus::disconnected(),
            "disconnected",
            state
                .store
                .load_openrouter_models()
                .map_err(error_to_string)?
                .len(),
            None,
        ));
    }

    validate_openrouter_api_key_remote(&api_key)
        .await
        .map_err(error_to_string)?;
    let auth = save_default_openrouter_api_key(&api_key).map_err(error_to_string)?;
    let models = state
        .store
        .load_openrouter_models()
        .map_err(error_to_string)?;
    install_openrouter_provider(&state.providers, &models)?;
    Ok(openrouter_provider_status_from_auth(
        auth,
        "connected",
        models.len(),
        None,
    ))
}

#[tauri::command]
pub(super) async fn disconnect_openrouter_provider(
    state: State<'_, DesktopState>,
) -> std::result::Result<OpenRouterProviderStatus, String> {
    cancel_active_turns_for_provider(&state, OPENROUTER_PROVIDER_ID).await;
    delete_default_openrouter_auth().map_err(error_to_string)?;
    remove_openrouter_provider(&state.providers)?;
    let model_count = state
        .store
        .load_openrouter_models()
        .map_err(error_to_string)?
        .len();
    Ok(openrouter_provider_status_from_auth(
        OpenRouterAuthStatus::disconnected(),
        "disconnected",
        model_count,
        None,
    ))
}

#[tauri::command]
pub(super) fn list_openrouter_models(
    state: State<'_, DesktopState>,
) -> std::result::Result<Vec<OpenRouterModelRecord>, String> {
    state
        .store
        .load_openrouter_models()
        .map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn search_openrouter_models(
    state: State<'_, DesktopState>,
    input: SearchOpenRouterModelsInput,
) -> std::result::Result<Vec<OpenRouterCatalogModel>, String> {
    let query = input.query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let api_key = load_default_openrouter_api_key()
        .map_err(error_to_string)?
        .ok_or_else(|| "OpenRouter is not connected".to_string())?;
    let catalog = match fetch_openrouter_model_catalog(&api_key).await {
        Ok(catalog) => catalog,
        Err(err) => {
            if matches!(err, claakecode_core::AppError::Auth(_)) {
                remove_openrouter_provider(&state.providers)?;
            }
            return Err(error_to_string(err));
        }
    };
    let mut matches = catalog
        .into_iter()
        .filter(|model| {
            model.name.to_ascii_lowercase().contains(&query)
                || model.id.to_ascii_lowercase().contains(&query)
        })
        .collect::<Vec<_>>();
    matches.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    matches.truncate(20);
    Ok(matches)
}

#[tauri::command]
pub(super) fn add_openrouter_model(
    state: State<'_, DesktopState>,
    input: AddOpenRouterModelInput,
) -> std::result::Result<Vec<OpenRouterModelRecord>, String> {
    let model = OpenRouterModelRecord {
        id: input.model.id,
        name: input.model.name,
        context_window: input.model.context_window,
        max_output_tokens: input.model.max_output_tokens,
        supports_images: input.model.supports_images,
        supports_thinking: input.model.supports_thinking,
        supports_tools: input.model.supports_tools,
        added_at_ms: now_ms(),
    };
    let models = state
        .store
        .add_openrouter_model(model)
        .map_err(error_to_string)?;
    refresh_openrouter_provider_if_present(&state, &models)?;
    Ok(models)
}

#[tauri::command]
pub(super) fn remove_openrouter_model(
    state: State<'_, DesktopState>,
    input: RemoveOpenRouterModelInput,
) -> std::result::Result<Vec<OpenRouterModelRecord>, String> {
    let models = state
        .store
        .remove_openrouter_model(&input.id)
        .map_err(error_to_string)?;
    refresh_openrouter_provider_if_present(&state, &models)?;
    Ok(models)
}

pub(super) fn refresh_openrouter_provider_if_present(
    state: &DesktopState,
    models: &[OpenRouterModelRecord],
) -> std::result::Result<(), String> {
    let present = state
        .providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?
        .contains_key(OPENROUTER_PROVIDER_ID);
    if present {
        install_openrouter_provider(&state.providers, models)?;
    }
    Ok(())
}

pub(super) async fn cancel_active_turns_for_provider(state: &DesktopState, provider_id: &str) {
    let active = state
        .active_turns
        .lock()
        .await
        .iter()
        .map(|(conversation_id, cancel)| (conversation_id.clone(), cancel.clone()))
        .collect::<Vec<_>>();
    for (conversation_id, cancel) in active {
        match state.store.load_conversation_model_by_id(&conversation_id) {
            Ok(Some(model)) if model.provider == provider_id => {
                cancel.cancel_all();
            }
            Ok(_) => {}
            Err(err) => {
                tracing::warn!(conversation_id, error = %err, "unable to inspect active turn model before provider disconnect");
            }
        }
    }
}
