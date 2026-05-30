use crate::*;

pub(super) fn model_with_optional_selection(
    current: &ModelRef,
    model: Option<ModelInput>,
    thinking: Option<ThinkingLevelInput>,
) -> ModelRef {
    let mut selected = match model {
        Some(model) => ModelRef::new(model.provider, model.name),
        None => current.clone(),
    };
    if let Some(thinking) = thinking {
        selected.effort = Some(thinking.into_effort());
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
    if let Ok(archived) = state.store.list_archived_providers() {
        providers.retain(|p| !archived.contains(p));
    }
    providers.sort_by(|a, b| compare_provider_keys(a, b));
    Ok(providers)
}

#[tauri::command]
pub(super) fn archive_provider(
    state: State<'_, DesktopState>,
    provider_id: String,
) -> std::result::Result<(), String> {
    state.store.set_provider_status(&provider_id, "archived").map_err(|e| e.to_string())
}

#[tauri::command]
pub(super) fn restore_provider(
    state: State<'_, DesktopState>,
    provider_id: String,
) -> std::result::Result<(), String> {
    state.store.set_provider_status(&provider_id, "active").map_err(|e| e.to_string())
}

#[tauri::command]
pub(super) fn list_archived_providers(
    state: State<'_, DesktopState>,
) -> std::result::Result<Vec<String>, String> {
    state.store.list_archived_providers().map_err(|e| e.to_string())
}

fn compare_provider_keys(a: &str, b: &str) -> std::cmp::Ordering {
    let split_a = a.split_once(':');
    let split_b = b.split_once(':');
    match (split_a, split_b) {
        (Some((p_a, s_a)), Some((p_b, s_b))) if p_a == p_b => {
            if let (Ok(num_a), Ok(num_b)) = (s_a.parse::<u32>(), s_b.parse::<u32>()) {
                num_a.cmp(&num_b)
            } else {
                s_a.cmp(s_b)
            }
        }
        _ => a.cmp(b),
    }
}

pub(super) fn install_openai_provider(
    providers: &Arc<StdMutex<HashMap<String, Arc<dyn Provider>>>>,
) -> std::result::Result<(), String> {
    if let Ok(default_path) = default_auth_path() {
        let dir = default_path.parent().unwrap();
        let old_first_path = dir.join("openai-auth-1.json");
        if old_first_path.exists() && !default_path.exists() {
            if let Err(err) = std::fs::rename(&old_first_path, &default_path) {
                tracing::warn!(
                    "failed to auto-rename openai-auth-1.json back to openai-auth.json: {:?}",
                    err
                );
            } else {
                tracing::info!(
                    "successfully restored openai-auth-1.json as openai-auth.json (principal)"
                );
            }
        }
    }

    if let Ok(files) = all_auth_files() {
        let mut lock = providers
            .lock()
            .map_err(|_| "provider registry is unavailable".to_string())?;
        for (key, path) in files {
            if let Ok(provider) = OpenAiProvider::from_file(&path) {
                lock.insert(key, Arc::new(provider) as Arc<dyn Provider>);
            }
        }
    } else {
        let provider = OpenAiProvider::from_default_sources().map_err(error_to_string)?;
        providers
            .lock()
            .map_err(|_| "provider registry is unavailable".to_string())?
            .insert("openai".into(), Arc::new(provider) as Arc<dyn Provider>);
    }
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
    if let Ok(default_path) = sinew_google::auth::default_auth_path() {
        let dir = default_path.parent().unwrap();
        let old_first_path = dir.join("google-auth-1.json");
        if old_first_path.exists() && !default_path.exists() {
            if let Err(err) = std::fs::rename(&old_first_path, &default_path) {
                tracing::warn!(
                    "failed to auto-rename google-auth-1.json back to google-auth.json: {:?}",
                    err
                );
            } else {
                tracing::info!(
                    "successfully restored google-auth-1.json as google-auth.json (principal)"
                );
            }
        }
    }

    if let Ok(files) = sinew_google::auth::all_auth_files() {
        let mut lock = providers
            .lock()
            .map_err(|_| "provider registry is unavailable".to_string())?;
        for (key, path) in files {
            if let Ok(provider) = GoogleProvider::from_file(&path) {
                lock.insert(key, Arc::new(provider) as Arc<dyn Provider>);
            }
        }
    } else {
        let provider = GoogleProvider::from_default_sources().map_err(error_to_string)?;
        providers
            .lock()
            .map_err(|_| "provider registry is unavailable".to_string())?
            .insert("google".into(), Arc::new(provider) as Arc<dyn Provider>);
    }
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
            sinew_openrouter::capabilities_from_parts(
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

pub(super) fn install_ollama_provider(
    providers: &Arc<StdMutex<HashMap<String, Arc<dyn Provider>>>>,
    models: &[OpenRouterModelRecord],
) -> std::result::Result<(), String> {
    let provider = OllamaProvider::from_default_sources(ollama_capabilities(models))
        .map_err(error_to_string)?;
    providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?
        .insert(
            OLLAMA_PROVIDER_ID.into(),
            Arc::new(provider) as Arc<dyn Provider>,
        );
    Ok(())
}

pub(super) fn ollama_capabilities(models: &[OpenRouterModelRecord]) -> Vec<ModelCapabilities> {
    models
        .iter()
        .map(|model| {
            sinew_ollama::capabilities_from_parts(
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

pub(super) fn default_ollama_model_ref(model: &OpenRouterModelRecord) -> ModelRef {
    let mut model_ref = ModelRef::new(OLLAMA_PROVIDER_ID, model.id.clone());
    model_ref.effort = Some(if model.supports_thinking {
        Effort::Medium
    } else {
        Effort::None
    });
    model_ref
}

pub(super) fn remove_ollama_provider(
    providers: &Arc<StdMutex<HashMap<String, Arc<dyn Provider>>>>,
) -> std::result::Result<(), String> {
    providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?
        .remove(OLLAMA_PROVIDER_ID);
    Ok(())
}

pub(super) fn ollama_provider_status_from_auth(
    auth: OllamaAuthStatus,
    connection_state: &str,
    model_count: usize,
    error: Option<String>,
) -> OllamaProviderStatus {
    OllamaProviderStatus {
        connected: auth.connected && connection_state == "connected",
        connection_state: connection_state.to_string(),
        base_url: auth.base_url,
        last_validated_ms: auth.last_validated_ms,
        model_count,
        error,
    }
}

pub(super) fn install_deepseek_provider(
    providers: &Arc<StdMutex<HashMap<String, Arc<dyn Provider>>>>,
) -> std::result::Result<(), String> {
    let provider = DeepSeekProvider::from_default_sources().map_err(error_to_string)?;
    providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?
        .insert(
            DEEPSEEK_PROVIDER_ID.into(),
            Arc::new(provider) as Arc<dyn Provider>,
        );
    Ok(())
}

pub(super) fn remove_deepseek_provider(
    providers: &Arc<StdMutex<HashMap<String, Arc<dyn Provider>>>>,
) -> std::result::Result<(), String> {
    providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?
        .remove(DEEPSEEK_PROVIDER_ID);
    Ok(())
}

pub(super) fn deepseek_provider_status_from_auth(
    auth: DeepSeekAuthStatus,
    connection_state: &str,
    error: Option<String>,
) -> DeepSeekProviderStatus {
    DeepSeekProviderStatus {
        connected: auth.connected && connection_state == "connected",
        connection_state: connection_state.to_string(),
        key_preview: auth.key_preview,
        last_validated_ms: auth.last_validated_ms,
        error,
    }
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
    target_key: Option<String>,
) -> Result<()> {
    let http = reqwest::Client::builder()
        .user_agent("sinew/0.1")
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
                    target_key.clone(),
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
    target_key: Option<String>,
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

            match exchange_oauth_code(http, code, redirect_uri, pkce, target_key).await {
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
    match tokio::net::TcpListener::bind(("127.0.0.1", CALLBACK_PORT)).await {
        Ok(listener) => Ok(listener),
        Err(err) => {
            let mut message = format!("unable to bind Anthropic OAuth callback port {CALLBACK_PORT}");
            #[cfg(target_os = "windows")]
            if err.raw_os_error() == Some(10013) {
                message.push_str(
                    "; Windows may have reserved this port. Check excluded TCP port ranges or restart WinNAT/HNS before trying again",
                );
            }
            Err(err).with_context(|| message)
        }
    }
}

pub(super) async fn run_anthropic_oauth_server(
    listener: tokio::net::TcpListener,
    redirect_uri: String,
    expected_state: String,
    pkce: AnthropicPkceCodes,
    cancel: Arc<Notify>,
) -> Result<()> {
    let http = reqwest::Client::builder()
        .user_agent("sinew/0.1")
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
    const CALLBACK_PORT: u16 = 51121;
    match tokio::net::TcpListener::bind(("127.0.0.1", CALLBACK_PORT)).await {
        Ok(listener) => Ok(listener),
        Err(err) => {
            let mut message = format!("unable to bind Google OAuth callback port {CALLBACK_PORT}");
            #[cfg(target_os = "windows")]
            if err.raw_os_error() == Some(10013) {
                message.push_str(
                    "; Windows may have reserved this port. Check excluded TCP port ranges or restart WinNAT/HNS before trying again",
                );
            }
            Err(err).with_context(|| message)
        }
    }
}

pub(super) async fn run_google_oauth_server(
    listener: tokio::net::TcpListener,
    redirect_uri: String,
    expected_state: String,
    pkce: GooglePkceCodes,
    cancel: Arc<Notify>,
    target_key: Option<String>,
) -> Result<()> {
    let http = reqwest::Client::builder()
        .user_agent("sinew/0.1")
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
                    target_key.as_deref(),
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
    target_key: Option<&str>,
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

            match exchange_google_oauth_code(http, code, redirect_uri, pkce, target_key.map(|s| s.to_string())).await {
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
    <title>Sinew connected</title>
    <style>
      body{margin:0;min-height:100vh;display:grid;place-items:center;background:#0a0b0d;color:#f4f4f5;font:15px -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
      main{max-width:420px;padding:32px;text-align:center}
      h1{font-size:22px;margin:0 0 10px}
      p{margin:0;color:#a1a1aa;line-height:1.5}
    </style>
  </head>
  <body><main><h1>OpenAI is connected</h1><p>You can close this tab and return to Sinew.</p></main></body>
</html>"#
        .to_string()
}

pub(super) fn anthropic_login_success_html() -> String {
    r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <title>Sinew connected</title>
    <style>
      body{margin:0;min-height:100vh;display:grid;place-items:center;background:#0a0b0d;color:#f4f4f5;font:15px -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
      main{max-width:420px;padding:32px;text-align:center}
      h1{font-size:22px;margin:0 0 10px}
      p{margin:0;color:#a1a1aa;line-height:1.5}
    </style>
  </head>
  <body><main><h1>Anthropic is connected</h1><p>You can close this tab and return to Sinew.</p></main></body>
</html>"#
        .to_string()
}

pub(super) fn google_login_success_html() -> String {
    r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <title>Sinew connected</title>
    <style>
      body{margin:0;min-height:100vh;display:grid;place-items:center;background:#0a0b0d;color:#f4f4f5;font:15px -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
      main{max-width:420px;padding:32px;text-align:center}
      h1{font-size:22px;margin:0 0 10px}
      p{margin:0;color:#a1a1aa;line-height:1.5}
    </style>
  </head>
  <body><main><h1>Google is connected</h1><p>You can close this tab and return to Sinew.</p></main></body>
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
    <title>Sinew connection failed</title>
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
    key: Option<String>,
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
            target_key: key.clone(),
        });
    }

    let providers = state.providers.clone();
    let target_key = key.clone();
    tauri::async_runtime::spawn(async move {
        let result = run_openai_oauth_server(
            listener,
            redirect_uri,
            oauth_state,
            pkce,
            cancel,
            target_key,
        )
        .await;
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OpenAiAccountInfo {
    pub(super) key: String,
    pub(super) email: Option<String>,
    pub(super) plan_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RateLimitWindowInfo {
    pub(super) used_percent: f64,
    pub(super) remaining_percent: f64,
    pub(super) window_minutes: Option<i64>,
    pub(super) reset_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OpenAiCodexRateLimitInfo {
    pub(super) key: String,
    pub(super) email: Option<String>,
    pub(super) plan_type: Option<String>,
    pub(super) workspace_name: Option<String>,
    pub(super) limit_id: Option<String>,
    pub(super) primary: Option<RateLimitWindowInfo>,
    pub(super) secondary: Option<RateLimitWindowInfo>,
    pub(super) raw: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AntigravityQuotaGroupInfo {
    pub(super) group: String,
    pub(super) label: String,
    pub(super) remaining_percent: Option<f64>,
    pub(super) reset_time: Option<String>,
    pub(super) count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AntigravityQuotaInfo {
    pub(super) project_id: Option<String>,
    pub(super) groups: Vec<AntigravityQuotaGroupInfo>,
    pub(super) raw: serde_json::Value,
}

static ANTIGRAVITY_QUOTA_CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, (AntigravityQuotaInfo, std::time::Instant)>>> = std::sync::OnceLock::new();
static CURSOR_USAGE_CACHE: std::sync::OnceLock<std::sync::Mutex<Option<(CursorUsageQuotaInfo, std::time::Instant)>>> = std::sync::OnceLock::new();
static ANTHROPIC_USAGE_CACHE: std::sync::OnceLock<std::sync::Mutex<Option<(serde_json::Value, std::time::Instant)>>> = std::sync::OnceLock::new();
static DEEPSEEK_BALANCE_CACHE: std::sync::OnceLock<std::sync::Mutex<Option<(serde_json::Value, std::time::Instant)>>> = std::sync::OnceLock::new();
static OPENROUTER_KEY_CACHE: std::sync::OnceLock<std::sync::Mutex<Option<(serde_json::Value, std::time::Instant)>>> = std::sync::OnceLock::new();
static OPENAI_CODEX_CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, (OpenAiCodexRateLimitInfo, std::time::Instant)>>> = std::sync::OnceLock::new();

#[tauri::command]
pub(super) async fn get_openai_codex_rate_limits(
    key: Option<String>,
) -> std::result::Result<OpenAiCodexRateLimitInfo, String> {
    let target_key = key.clone().unwrap_or_else(|| "openai".to_string());

    // Check cache
    let cache = OPENAI_CODEX_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some((cached, fetched_at)) = guard.get(&target_key) {
            if fetched_at.elapsed() < std::time::Duration::from_secs(30) {
                return Ok(cached.clone());
            }
        }
    }

    let path = if target_key == "openai" {
        default_auth_path().map_err(error_to_string)?
    } else {
        let suffix = target_key.strip_prefix("openai:").unwrap_or(&target_key);
        default_auth_path()
            .map_err(error_to_string)?
            .parent()
            .ok_or_else(|| "OpenAI auth directory is unavailable".to_string())?
            .join(format!("openai-auth-{suffix}.json"))
    };

    let status = load_auth_status(&path).map_err(error_to_string)?;
    let credential = OpenAiCredential::from_sinew_auth_file(&path)
        .map_err(error_to_string)?
        .ok_or_else(|| "OpenAI OAuth credential not found".to_string())?;
    let http = reqwest::Client::new();
    let bearer = credential.bearer(&http).await.map_err(error_to_string)?;
    if !bearer.is_oauth {
        return Err("Codex quotas require OpenAI OAuth".to_string());
    }

    let mut request = http
        .get("https://chatgpt.com/backend-api/codex/wham/usage")
        .header("authorization", format!("Bearer {}", bearer.token))
        .header("user-agent", "codex-cli")
        .header("accept", "application/json");
    if let Some(account_id) = bearer.account_id.as_deref() {
        request = request.header("ChatGPT-Account-Id", account_id);
    }

    let mut response = request
        .send()
        .await
        .map_err(|err| format!("Failed to fetch Codex quotas: {err}"))?;

    // Fallback suggéré par la communauté si /codex/wham/usage renvoie 403 (Business/Workspace)
    if response.status() == reqwest::StatusCode::FORBIDDEN
        || response.status() == reqwest::StatusCode::NOT_FOUND
    {
        let mut fallback_req = http
            .get("https://chatgpt.com/backend-api/wham/usage")
            .header("authorization", format!("Bearer {}", bearer.token))
            .header("user-agent", "codex-cli")
            .header("accept", "application/json");
        if let Some(account_id) = bearer.account_id.as_deref() {
            fallback_req = fallback_req.header("ChatGPT-Account-Id", account_id);
        }
        if let Ok(fb_resp) = fallback_req.send().await {
            if fb_resp.status().is_success() {
                response = fb_resp;
            }
        }
    }

    if !response.status().is_success() {
        return Err(format!(
            "Codex quota endpoint returned status {}",
            response.status()
        ));
    }
    let raw: serde_json::Value = response
        .json()
        .await
        .map_err(|err| format!("Failed to parse Codex quota response: {err}"))?;

    let rate_limit =
        raw.get("rate_limit")
            .and_then(|value| if value.is_null() { None } else { Some(value) });
    let primary = rate_limit
        .and_then(|value| value.get("primary_window"))
        .and_then(parse_rate_limit_window);
    let secondary = rate_limit
        .and_then(|value| value.get("secondary_window"))
        .and_then(parse_rate_limit_window);

    // Nom du workspace
    let mut workspace_name = None;
    let mut accounts_req = http
        .get("https://chatgpt.com/backend-api/wham/accounts/check")
        .header("authorization", format!("Bearer {}", bearer.token))
        .header("user-agent", "codex-cli")
        .header("accept", "application/json");
    if let Some(account_id) = bearer.account_id.as_deref() {
        accounts_req = accounts_req.header("ChatGPT-Account-Id", account_id);
    }
    if let Ok(accounts_resp) = accounts_req.send().await {
        if accounts_resp.status().is_success() {
            if let Ok(accounts_raw) = accounts_resp.json::<serde_json::Value>().await {
                if let Some(accounts_list) = accounts_raw.get("accounts").and_then(|v| v.as_array())
                {
                    let target_id = bearer.account_id.as_deref().or_else(|| {
                        accounts_raw
                            .get("default_account_id")
                            .and_then(|v| v.as_str())
                    });
                    if let Some(tid) = target_id {
                        if let Some(matched) = accounts_list
                            .iter()
                            .find(|acc| acc.get("id").and_then(|id| id.as_str()) == Some(tid))
                        {
                            if let Some(name) = matched.get("name").and_then(|n| n.as_str()) {
                                workspace_name = Some(name.to_string());
                            }
                        }
                    }
                    if workspace_name.is_none() {
                        for acc in accounts_list {
                            if let Some(name) = acc.get("name").and_then(|n| n.as_str()) {
                                workspace_name = Some(name.to_string());
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    let result = OpenAiCodexRateLimitInfo {
        key: target_key.clone(),
        email: status.email,
        plan_type: raw
            .get("plan_type")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string())
            .or(status.plan_type),
        workspace_name,
        limit_id: Some("codex".to_string()),
        primary,
        secondary,
        raw,
    };

    if let Ok(mut guard) = cache.lock() {
        guard.insert(target_key, (result.clone(), std::time::Instant::now()));
    }

    Ok(result)
}

fn parse_rate_limit_window(value: &serde_json::Value) -> Option<RateLimitWindowInfo> {
    if value.is_null() {
        return None;
    }
    let used_percent = value.get("used_percent")?.as_f64()?;
    let remaining_percent = (100.0 - used_percent).clamp(0.0, 100.0);
    let window_minutes = value
        .get("limit_window_seconds")
        .and_then(|value| value.as_i64())
        .filter(|seconds| *seconds > 0)
        .map(|seconds| (seconds + 59) / 60);
    let reset_at = value.get("reset_at").and_then(|value| value.as_i64());
    Some(RateLimitWindowInfo {
        used_percent,
        remaining_percent,
        window_minutes,
        reset_at,
    })
}

#[tauri::command]
pub(super) async fn get_antigravity_quota(
    key: Option<String>,
) -> std::result::Result<AntigravityQuotaInfo, String> {
    let cache_key = key.clone().unwrap_or_else(|| "default".to_string());
    
    // Check cache
    let cache = ANTIGRAVITY_QUOTA_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some((cached, fetched_at)) = guard.get(&cache_key) {
            if fetched_at.elapsed() < std::time::Duration::from_secs(30) {
                return Ok(cached.clone());
            }
        }
    }

    let path = if let Some(k) = key.as_deref() {
        sinew_google::auth::path_for_auth_key(k).map_err(error_to_string)?
    } else {
        sinew_google::auth::default_auth_path().map_err(error_to_string)?
    };

    let credential = GoogleCredential::from_sinew_auth_file(&path)
        .map_err(error_to_string)?
        .ok_or_else(|| "Google OAuth credential not found".to_string())?;
    let http = reqwest::Client::new();
    let token = credential.bearer(&http).await.map_err(error_to_string)?;
    let project = sinew_google::load_user_data(&path)
        .map_err(error_to_string)?
        .map(|user| user.project_id);
    let body = if let Some(project_id) = project.as_deref() {
        serde_json::json!({ "project": project_id })
    } else {
        serde_json::json!({})
    };
    let response = http
        .post("https://cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels")
        .bearer_auth(token)
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .header(
            "user-agent",
            sinew_google::antigravity_load_code_assist_user_agent(),
        )
        .header("x-goog-api-client", "gl-node/22.21.1")
        .json(&body)
        .send()
        .await
        .map_err(|err| format!("Failed to fetch Antigravity quotas: {err}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Antigravity quota endpoint returned status {}",
            response.status()
        ));
    }
    let raw: serde_json::Value = response
        .json()
        .await
        .map_err(|err| format!("Failed to parse Antigravity quota response: {err}"))?;

    let mut groups: HashMap<String, AntigravityQuotaGroupInfo> = HashMap::new();
    if let Some(models) = raw.get("models").and_then(|value| value.as_object()) {
        for (model_name, info) in models {
            // On ignore les modèles internes qui n'ont pas de vrai "displayName" public
            let Some(label) = info.get("displayName").and_then(|v| v.as_str()) else {
                continue;
            };

            // On ignore aussi s'il s'agit explicitement de endpoints internes (ex: tab_, chat_)
            if label.starts_with("tab_")
                || label.starts_with("chat_")
                || model_name.starts_with("tab_")
                || model_name.starts_with("chat_")
            {
                continue;
            }

            // On ne garde que les 8 modèles officiels de l'abonnement Antigravity
            let is_official = matches!(
                label,
                "Claude Opus 4.6 (Thinking)"
                    | "Claude Sonnet 4.6 (Thinking)"
                    | "GPT-OSS 120B (Medium)"
                    | "Gemini 3.5 Flash (Low)"
                    | "Gemini 3.5 Flash (Medium)"
                    | "Gemini 3.5 Flash (High)"
                    | "Gemini 3.1 Pro (Low)"
                    | "Gemini 3.1 Pro (High)"
            );
            if !is_official {
                continue;
            }

            let quota = info.get("quotaInfo");
            let remaining = quota
                .and_then(|value| value.get("remainingFraction"))
                .and_then(|value| value.as_f64())
                .map(|value| (value * 100.0).clamp(0.0, 100.0));
            let reset_time = quota
                .and_then(|value| value.get("resetTime"))
                .and_then(|value| value.as_str())
                .map(|value| value.to_string());

            // Si pas de quota, on ignore
            if remaining.is_none() && reset_time.is_none() {
                continue;
            }

            let group_label = match label {
                "Claude Opus 4.6 (Thinking)"
                | "Claude Sonnet 4.6 (Thinking)"
                | "GPT-OSS 120B (Medium)" => "Claude & GPT-OSS",
                "Gemini 3.5 Flash (Low)"
                | "Gemini 3.5 Flash (Medium)"
                | "Gemini 3.5 Flash (High)"
                | "Gemini 3.1 Pro (Low)"
                | "Gemini 3.1 Pro (High)" => "Gemini",
                _ => label,
            };

            let entry =
                groups
                    .entry(group_label.to_string())
                    .or_insert_with(|| AntigravityQuotaGroupInfo {
                        group: group_label.to_string(),
                        label: group_label.to_string(),
                        remaining_percent: remaining,
                        reset_time: reset_time.clone(),
                        count: 0,
                    });
            entry.count += 1;
            if entry.reset_time.is_none() && reset_time.is_some() {
                entry.reset_time = reset_time;
            }
            if let Some(rem) = remaining {
                entry.remaining_percent = Some(
                    entry
                        .remaining_percent
                        .map_or(rem, |current| current.min(rem)),
                );
            }
        }
    }
    let mut groups = groups.into_values().collect::<Vec<_>>();
    groups.sort_by(|a, b| a.label.cmp(&b.label));

    let result = AntigravityQuotaInfo {
        project_id: project,
        groups,
        raw,
    };

    if let Ok(mut guard) = cache.lock() {
        guard.insert(cache_key, (result.clone(), std::time::Instant::now()));
    }

    Ok(result)
}

#[tauri::command]
pub(super) async fn get_all_openai_accounts() -> std::result::Result<Vec<OpenAiAccountInfo>, String>
{
    let mut accounts = Vec::new();
    if let Ok(files) = all_auth_files() {
        for (key, path) in files {
            if let Ok(status) = load_auth_status(&path) {
                if status.connected {
                    accounts.push(OpenAiAccountInfo {
                        key,
                        email: status.email,
                        plan_type: status.plan_type,
                    });
                }
            }
        }
    }
    accounts.sort_by(|a, b| compare_provider_keys(&a.key, &b.key));
    Ok(accounts)
}

#[tauri::command]
pub(super) async fn disconnect_openai_account(
    state: State<'_, DesktopState>,
    key: String,
) -> std::result::Result<(), String> {
    let mut lock = state
        .providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?;
    lock.remove(&key);

    if let Ok(files) = all_auth_files() {
        for (fkey, path) in files {
            if fkey == key {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GoogleAccountInfo {
    pub(super) key: String,
    pub(super) email: Option<String>,
    pub(super) project_id: Option<String>,
    pub(super) user_tier: Option<String>,
}

#[tauri::command]
pub(super) async fn get_all_google_accounts() -> std::result::Result<Vec<GoogleAccountInfo>, String> {
    let mut accounts = Vec::new();
    if let Ok(files) = sinew_google::auth::all_auth_files() {
        for (key, path) in files {
            if let Ok(status) = sinew_google::auth::load_auth_status(&path) {
                if status.connected {
                    accounts.push(GoogleAccountInfo {
                        key,
                        email: status.email,
                        project_id: status.project_id,
                        user_tier: status.user_tier,
                    });
                }
            }
        }
    }
    accounts.sort_by(|a, b| compare_provider_keys(&a.key, &b.key));
    Ok(accounts)
}

#[tauri::command]
pub(super) async fn disconnect_google_account(
    state: State<'_, DesktopState>,
    key: String,
) -> std::result::Result<(), String> {
    let mut lock = state
        .providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?;
    lock.remove(&key);

    if let Ok(files) = sinew_google::auth::all_auth_files() {
        for (fkey, path) in files {
            if fkey == key {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub(super) async fn save_openai_access_token(
    state: State<'_, DesktopState>,
    token: String,
    key: Option<String>,
) -> std::result::Result<(), String> {
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err("access token is empty".to_string());
    }

    let default_path = default_auth_path().map_err(error_to_string)?;
    let target_path = if let Some(ref k) = key {
        if k == "openai" {
            default_path
        } else {
            let suffix = k.strip_prefix("openai:").unwrap_or(k);
            default_path
                .parent()
                .unwrap()
                .join(format!("openai-auth-{}.json", suffix))
        }
    } else {
        default_path
    };

    sinew_openai::save_raw_access_token(&target_path, &token).map_err(error_to_string)?;

    install_openai_provider(&state.providers)?;
    Ok(())
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
    key: Option<String>,
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
            target_key: key.clone(),
        });
    }

    let providers = state.providers.clone();
    let target_key = key.clone();
    tauri::async_runtime::spawn(async move {
        let result =
            run_google_oauth_server(listener, redirect_uri, oauth_state, pkce, cancel, target_key).await;
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
    let mut tool_settings = state.store.load_tool_settings().map_err(error_to_string)?;
    if tool_settings.gemini_image_use_subscription {
        tool_settings.gemini_image_use_subscription = false;
        state
            .store
            .save_tool_settings(&tool_settings)
            .map_err(error_to_string)?;
    }
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
        .user_agent("sinew/0.1")
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
    let state_str = if auth.connected {
        let models = state
            .store
            .load_openrouter_models()
            .map_err(error_to_string)?;
        install_openrouter_provider(&state.providers, &models)?;
        "connected"
    } else {
        remove_openrouter_provider(&state.providers)?;
        "disconnected"
    };
    Ok(openrouter_provider_status_from_auth(
        auth,
        state_str,
        model_count,
        None,
    ))
}

#[tauri::command]
pub(super) async fn get_openrouter_key_details() -> std::result::Result<serde_json::Value, String> {
    let cache = OPENROUTER_KEY_CACHE.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some((ref cached, fetched_at)) = *guard {
            if fetched_at.elapsed() < std::time::Duration::from_secs(30) {
                return Ok(cached.clone());
            }
        }
    }

    let api_key = load_default_openrouter_api_key().map_err(error_to_string)?;
    let Some(api_key) = api_key else {
        return Err("No OpenRouter API key found".to_string());
    };
    let client = reqwest::Client::new();
    let response = client
        .get("https://openrouter.ai/api/v1/auth/key")
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|err| format!("Failed to fetch OpenRouter key details: {err}"))?;
    if !response.status().is_success() {
        return Err(format!("OpenRouter returned status {}", response.status()));
    }
    let data: serde_json::Value = response
        .json()
        .await
        .map_err(|err| format!("Failed to parse response: {err}"))?;

    if let Ok(mut guard) = cache.lock() {
        *guard = Some((data.clone(), std::time::Instant::now()));
    }

    Ok(data)
}

#[tauri::command]
pub(super) async fn get_deepseek_balance() -> std::result::Result<serde_json::Value, String> {
    let cache = DEEPSEEK_BALANCE_CACHE.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some((ref cached, fetched_at)) = *guard {
            if fetched_at.elapsed() < std::time::Duration::from_secs(30) {
                return Ok(cached.clone());
            }
        }
    }

    let api_key = load_default_deepseek_api_key().map_err(error_to_string)?;
    let Some(api_key) = api_key else {
        return Err("No DeepSeek API key found".to_string());
    };
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.deepseek.com/user/balance")
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|err| format!("Failed to fetch DeepSeek balance: {err}"))?;
    if !response.status().is_success() {
        return Err(format!("DeepSeek returned status {}", response.status()));
    }
    let data: serde_json::Value = response
        .json()
        .await
        .map_err(|err| format!("Failed to parse response: {err}"))?;

    if let Ok(mut guard) = cache.lock() {
        *guard = Some((data.clone(), std::time::Instant::now()));
    }

    Ok(data)
}

#[tauri::command]
pub(super) async fn list_deepseek_models_remote() -> std::result::Result<serde_json::Value, String> {
    let api_key = load_default_deepseek_api_key().map_err(error_to_string)?;
    let Some(api_key) = api_key else {
        return Err("No DeepSeek API key found".to_string());
    };
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.deepseek.com/models")
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|err| format!("Failed to fetch DeepSeek models: {err}"))?;
    if !response.status().is_success() {
        return Err(format!("DeepSeek returned status {}", response.status()));
    }
    let data: serde_json::Value = response
        .json()
        .await
        .map_err(|err| format!("Failed to parse response: {err}"))?;
    Ok(data)
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
pub(super) async fn get_ollama_provider_status(
    state: State<'_, DesktopState>,
) -> std::result::Result<OllamaProviderStatus, String> {
    let model_count = state.store.load_ollama_models().map_err(error_to_string)?.len();
    let auth = load_default_ollama_auth_status().map_err(error_to_string)?;
    let state_str = if auth.connected {
        let models = state.store.load_ollama_models().map_err(error_to_string)?;
        install_ollama_provider(&state.providers, &models)?;
        "connected"
    } else {
        remove_ollama_provider(&state.providers)?;
        "disconnected"
    };
    Ok(ollama_provider_status_from_auth(
        auth, state_str, model_count, None,
    ))
}

#[tauri::command]
pub(super) async fn connect_ollama_provider(
    state: State<'_, DesktopState>,
    input: ConnectOllamaInput,
) -> std::result::Result<OllamaProviderStatus, String> {
    let base_url = input
        .base_url
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(default_ollama_base_url);

    validate_ollama_endpoint(&base_url)
        .await
        .map_err(error_to_string)?;
    let auth = save_default_ollama_base_url(&base_url).map_err(error_to_string)?;

    let catalog = fetch_ollama_model_catalog(&base_url)
        .await
        .map_err(error_to_string)?;
    let records = catalog.into_iter().map(ollama_record_from_catalog).collect::<Vec<_>>();
    let models = state
        .store
        .save_ollama_models(&records)
        .map_err(error_to_string)?;
    install_ollama_provider(&state.providers, &models)?;
    Ok(ollama_provider_status_from_auth(
        auth,
        "connected",
        models.len(),
        None,
    ))
}

#[tauri::command]
pub(super) async fn refresh_ollama_models(
    state: State<'_, DesktopState>,
) -> std::result::Result<Vec<OpenRouterModelRecord>, String> {
    let base_url = load_default_ollama_base_url()
        .map_err(error_to_string)?
        .ok_or_else(|| "Ollama is not connected".to_string())?;
    let catalog = fetch_ollama_model_catalog(&base_url)
        .await
        .map_err(error_to_string)?;
    let records = catalog.into_iter().map(ollama_record_from_catalog).collect::<Vec<_>>();
    let models = state
        .store
        .save_ollama_models(&records)
        .map_err(error_to_string)?;
    refresh_ollama_provider_if_present(&state, &models)?;
    Ok(models)
}

#[tauri::command]
pub(super) fn list_ollama_models(
    state: State<'_, DesktopState>,
) -> std::result::Result<Vec<OpenRouterModelRecord>, String> {
    state.store.load_ollama_models().map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn disconnect_ollama_provider(
    state: State<'_, DesktopState>,
) -> std::result::Result<OllamaProviderStatus, String> {
    cancel_active_turns_for_provider(&state, OLLAMA_PROVIDER_ID).await;
    delete_default_ollama_auth().map_err(error_to_string)?;
    remove_ollama_provider(&state.providers)?;
    let model_count = state.store.load_ollama_models().map_err(error_to_string)?.len();
    Ok(ollama_provider_status_from_auth(
        OllamaAuthStatus::disconnected(),
        "disconnected",
        model_count,
        None,
    ))
}

pub(super) fn ollama_record_from_catalog(
    model: sinew_ollama::OllamaCatalogModel,
) -> OpenRouterModelRecord {
    OpenRouterModelRecord {
        id: model.id,
        name: model.name,
        context_window: model.context_window,
        max_output_tokens: model.max_output_tokens,
        supports_images: model.supports_images,
        supports_thinking: model.supports_thinking,
        supports_tools: model.supports_tools,
        added_at_ms: now_ms(),
    }
}

pub(super) fn refresh_ollama_provider_if_present(
    state: &DesktopState,
    models: &[OpenRouterModelRecord],
) -> std::result::Result<(), String> {
    let present = state
        .providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?
        .contains_key(OLLAMA_PROVIDER_ID);
    if present {
        install_ollama_provider(&state.providers, models)?;
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ValidateDeepSeekApiKeyInput {
    pub(super) api_key: String,
}

#[tauri::command]
pub(super) async fn get_deepseek_provider_status(
    state: State<'_, DesktopState>,
) -> std::result::Result<DeepSeekProviderStatus, String> {
    let auth = load_default_deepseek_auth_status().map_err(error_to_string)?;
    let api_key = load_default_deepseek_api_key().map_err(error_to_string)?;
    let state_str = if api_key.is_some() {
        install_deepseek_provider(&state.providers)?;
        "connected"
    } else {
        remove_deepseek_provider(&state.providers)?;
        "disconnected"
    };
    Ok(deepseek_provider_status_from_auth(
        auth,
        state_str,
        None,
    ))
}

#[tauri::command]
pub(super) async fn validate_deepseek_api_key(
    state: State<'_, DesktopState>,
    input: ValidateDeepSeekApiKeyInput,
) -> std::result::Result<DeepSeekProviderStatus, String> {
    let api_key = input.api_key.trim().to_string();
    if api_key.is_empty() {
        return Ok(deepseek_provider_status_from_auth(
            DeepSeekAuthStatus::disconnected(),
            "disconnected",
            None,
        ));
    }

    validate_deepseek_api_key_remote(&api_key)
        .await
        .map_err(error_to_string)?;
    let auth = save_default_deepseek_api_key(&api_key).map_err(error_to_string)?;
    install_deepseek_provider(&state.providers)?;
    Ok(deepseek_provider_status_from_auth(
        auth,
        "connected",
        None,
    ))
}

#[tauri::command]
pub(super) async fn disconnect_deepseek_provider(
    state: State<'_, DesktopState>,
) -> std::result::Result<DeepSeekProviderStatus, String> {
    cancel_active_turns_for_provider(&state, DEEPSEEK_PROVIDER_ID).await;
    delete_default_deepseek_auth().map_err(error_to_string)?;
    remove_deepseek_provider(&state.providers)?;
    Ok(deepseek_provider_status_from_auth(
        DeepSeekAuthStatus::disconnected(),
        "disconnected",
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
            if matches!(err, sinew_core::AppError::Auth(_)) {
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

pub(super) fn install_cursor_provider(
    providers: &Arc<StdMutex<HashMap<String, Arc<dyn Provider>>>>,
) -> std::result::Result<(), String> {
    let provider = CursorProvider::from_default_sources().map_err(error_to_string)?;
    providers
        .lock()
        .map_err(|_| "provider registry is unavailable".to_string())?
        .insert(CURSOR_PROVIDER_ID.into(), Arc::new(provider) as Arc<dyn Provider>);
    Ok(())
}

pub(super) fn cursor_composer_status_from_auth(
    auth: CursorComposerAuthStatus,
    connection_state: &str,
    login_id: Option<String>,
    error: Option<String>,
) -> CursorComposerAuthStatus {
    auth.with_connection_state(connection_state, login_id, error)
}

#[tauri::command]
pub(super) async fn get_cursor_composer_status(
    state: State<'_, DesktopState>,
) -> std::result::Result<CursorComposerAuthStatus, String> {
    let mut active_login = state.cursor_login.lock().await;
    let attempt = active_login.clone();
    if let Some(attempt) = attempt {
        let outcome = attempt
            .outcome
            .lock()
            .map_err(|_| "login state is unavailable".to_string())?
            .clone();

        if let Some(outcome) = outcome {
            *active_login = None;
            let auth = load_composer_auth_status().map_err(error_to_string)?;
            if outcome.success {
                return Ok(cursor_composer_status_from_auth(
                    auth,
                    "connected",
                    None,
                    None,
                ));
            }
            return Ok(cursor_composer_status_from_auth(
                auth,
                "error",
                None,
                outcome.error,
            ));
        }

        let auth = load_composer_auth_status().map_err(error_to_string)?;
        return Ok(cursor_composer_status_from_auth(
            auth,
            "connecting",
            Some(attempt.id),
            None,
        ));
    }

    let auth = load_composer_auth_status().map_err(error_to_string)?;
    let connection_state = if auth.connected {
        "connected"
    } else {
        "disconnected"
    };
    Ok(cursor_composer_status_from_auth(auth, connection_state, None, None))
}

#[tauri::command]
pub(super) async fn start_cursor_oauth_login(
    state: State<'_, DesktopState>,
) -> std::result::Result<StartCursorLoginOutput, String> {
    if let Some(existing) = state.cursor_login.lock().await.take() {
        existing.cancel.notify_one();
    }

    let challenge = create_login_challenge();
    let login_id = generate_kimi_state();
    let cancel = Arc::new(Notify::new());
    let outcome = Arc::new(StdMutex::new(None));

    {
        let mut active_login = state.cursor_login.lock().await;
        *active_login = Some(CursorLoginAttempt {
            id: login_id.clone(),
            cancel: cancel.clone(),
            outcome: outcome.clone(),
        });
    }

    let providers = state.providers.clone();
    let auth_url = challenge.auth_url.clone();
    tauri::async_runtime::spawn(async move {
        let result = run_cursor_oauth_login(challenge, cancel).await;
        let login_outcome = match result {
            Ok(()) => match install_cursor_provider(&providers) {
                Ok(()) => CursorLoginOutcome {
                    success: true,
                    error: None,
                },
                Err(err) => CursorLoginOutcome {
                    success: false,
                    error: Some(err),
                },
            },
            Err(err) => CursorLoginOutcome {
                success: false,
                error: Some(err.to_string()),
            },
        };
        if let Ok(mut slot) = outcome.lock() {
            *slot = Some(login_outcome);
        }
    });

    Ok(StartCursorLoginOutput { login_id, auth_url })
}

pub(super) async fn run_cursor_oauth_login(
    challenge: CursorLoginChallenge,
    cancel: Arc<Notify>,
) -> Result<()> {
    let http = reqwest::Client::builder()
        .user_agent(CursorIdeIdentity::load().user_agent())
        .build()
        .context("unable to build Cursor OAuth client")?;

    tokio::select! {
        _ = cancel.notified() => {
            anyhow::bail!("Login canceled");
        }
        result = wait_for_oauth_login(&http, &challenge, &cancel) => {
            result.map(|_| ()).map_err(|err| anyhow::anyhow!(err.to_string()))
        }
    }
}

#[tauri::command]
pub(super) async fn cancel_cursor_oauth_login(
    state: State<'_, DesktopState>,
) -> std::result::Result<CursorComposerAuthStatus, String> {
    if let Some(attempt) = state.cursor_login.lock().await.take() {
        attempt.cancel.notify_one();
    }
    let auth = load_composer_auth_status().map_err(error_to_string)?;
    let connection_state = if auth.connected {
        "connected"
    } else {
        "disconnected"
    };
    Ok(cursor_composer_status_from_auth(
        auth,
        connection_state,
        None,
        None,
    ))
}

#[tauri::command]
pub(super) async fn sync_cursor_composer_auth(
    state: State<'_, DesktopState>,
) -> std::result::Result<CursorComposerAuthStatus, String> {
    if let Ok(Some(session)) = load_composer_session() {
        let http = reqwest::Client::builder()
            .user_agent(CursorIdeIdentity::load().user_agent())
            .build()
            .map_err(error_to_string)?;
        if let Err(err) = ensure_fresh_composer_token(&http, &session).await {
            tracing::warn!("cursor composer token refresh during sync failed: {err}");
        }
    }

    let auth = load_composer_auth_status().map_err(error_to_string)?;
    install_cursor_provider(&state.providers)?;
    let connection_state = if auth.connected {
        "connected"
    } else {
        "disconnected"
    };
    Ok(cursor_composer_status_from_auth(
        auth,
        connection_state,
        None,
        None,
    ))
}

#[tauri::command]
pub(super) async fn disconnect_cursor_composer(
    state: State<'_, DesktopState>,
) -> std::result::Result<(), String> {
    if let Some(attempt) = state.cursor_login.lock().await.take() {
        attempt.cancel.notify_one();
    }
    delete_composer_auth().map_err(error_to_string)?;
    install_cursor_provider(&state.providers).ok();
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CursorUsageQuotaInfo {
    pub auto_percent_used: f64,
    pub api_percent_used: f64,
    pub total_percent_used: f64,
}

#[tauri::command]
pub(super) async fn get_cursor_usage() -> std::result::Result<CursorUsageQuotaInfo, String> {
    let cache = CURSOR_USAGE_CACHE.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some((ref cached, fetched_at)) = *guard {
            if fetched_at.elapsed() < std::time::Duration::from_secs(30) {
                return Ok(cached.clone());
            }
        }
    }

    let provider = CursorProvider::from_default_sources().map_err(error_to_string)?;
    let usage = provider
        .usage_snapshot()
        .await
        .map_err(error_to_string)?
        .ok_or_else(|| "Cursor composer session is not connected".to_string())?;
    let result = CursorUsageQuotaInfo {
        auto_percent_used: usage.auto_percent_used,
        api_percent_used: usage.api_percent_used,
        total_percent_used: usage.total_percent_used,
    };

    if let Ok(mut guard) = cache.lock() {
        *guard = Some((result.clone(), std::time::Instant::now()));
    }

    Ok(result)
}

#[tauri::command]
pub(super) async fn get_anthropic_usage() -> std::result::Result<serde_json::Value, String> {
    let cache = ANTHROPIC_USAGE_CACHE.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some((ref cached, fetched_at)) = *guard {
            if fetched_at.elapsed() < std::time::Duration::from_secs(30) {
                return Ok(cached.clone());
            }
        }
    }

    let provider = AnthropicProvider::from_default_sources().map_err(error_to_string)?;
    let usage = provider
        .get_usage()
        .await
        .map_err(error_to_string)?;

    if let Ok(mut guard) = cache.lock() {
        *guard = Some((usage.clone(), std::time::Instant::now()));
    }

    Ok(usage)
}
