#[cfg(test)]
mod tests {
    use sinew_core::{ChatMessage, ModelRef, Part, ProviderRequest, Role};

    use crate::{
        conversation::build_stream_request,
        encryption::BlobEncryptionKey,
        identity::CursorIdeIdentity,
        sanitize::sanitize_outbound_text,
        tools::{build_client_tool_result, parse_tool_call, SUPPORTED_TOOLS},
    };

    fn test_blob_key() -> BlobEncryptionKey {
        BlobEncryptionKey::from_raw([0xAB; 32])
    }

    #[test]
    fn sanitizes_sinew_branding() {
        let text = sanitize_outbound_text("You are Sinew from Hyrak");
        assert!(!text.contains("Sinew"));
        assert!(text.contains("Cursor"));
    }

    #[test]
    fn oauth_only_identity_is_ready() {
        let identity = CursorIdeIdentity {
            machine_id: uuid::Uuid::new_v4().to_string(),
            mac_machine_id: None,
            client_version: crate::identity::CURSOR_CLIENT_VERSION.into(),
            timezone: "UTC".into(),
            platform: "windows".into(),
            arch: "x64".into(),
            shell: "powershell".into(),
        };
        identity
            .ensure_ready()
            .expect("oauth-only identity should be ready");
    }

    #[test]
    fn loaded_identity_is_ready_without_cursor_ide() {
        let identity = CursorIdeIdentity::load();
        identity
            .ensure_ready()
            .expect("loaded identity should always have a machine id");
        assert!(!identity.machine_id.is_empty());
    }

    #[test]
    fn composer_supports_images() {
        let caps = crate::model_info::capabilities(&ModelRef::new("cursor", "composer-2.5-fast"));
        assert!(caps.supports_images);
    }

    #[test]
    fn supported_tools_include_generate_image() {
        assert!(SUPPORTED_TOOLS
            .iter()
            .any(|tool| *tool == "CLIENT_SIDE_TOOL_V2_GENERATE_IMAGE"));
    }

    #[test]
    fn builds_idempotent_request_with_workspace() {
        let request = ProviderRequest::new(
            ModelRef::new("cursor", "composer-2.5-fast"),
            vec![ChatMessage::user_text("hello")],
        )
        .with_system("You are Sinew")
        .with_workspace_root(std::env::current_dir().unwrap().display().to_string());
        let identity = CursorIdeIdentity::load();
        let (payload, next_seqno) =
            build_stream_request(&request, "conv", "idem", 0, &identity, &test_blob_key());
        assert!(next_seqno >= 1);
        assert!(!payload.is_empty());
        let body = String::from_utf8_lossy(&payload);
        assert!(body.contains("streamUnifiedChatRequest") || body.contains("clientChunk"));
    }

    #[test]
    fn parses_read_file_tool_call() {
        let value = serde_json::json!({
            "tool": "CLIENT_SIDE_TOOL_V2_READ_FILE_V2",
            "toolCallId": "call_1",
            "readFileV2Params": {
                "targetFile": "src/main.rs",
                "limit": 100
            }
        });
        let parsed = parse_tool_call(&value).expect("parsed");
        assert_eq!(parsed.cursor_tool, "CLIENT_SIDE_TOOL_V2_READ_FILE_V2");
        assert_eq!(parsed.sinew_name, "read");
        assert_eq!(parsed.input["path"], "src/main.rs");
    }

    #[test]
    fn builds_tool_result_payload() {
        let result = build_client_tool_result(
            "call_1",
            "read",
            "CLIENT_SIDE_TOOL_V2_READ_FILE_V2",
            "file contents",
            false,
            None,
        );
        assert_eq!(result["toolCallId"], "call_1");
        assert_eq!(result["readFileV2Result"]["contents"], "file contents");
    }

    #[test]
    fn assistant_tool_call_history_roundtrip_fields() {
        let request = ProviderRequest::new(
            ModelRef::new("cursor", "composer-2.5-fast"),
            vec![
                ChatMessage {
                    role: Role::Assistant,
                    parts: vec![Part::ToolCall {
                        id: "call_1".into(),
                        name: "read".into(),
                        input: serde_json::json!({ "path": "a.rs", "limit": 10 }),
                        meta: None,
                    }],
                },
                ChatMessage {
                    role: Role::User,
                    parts: vec![Part::ToolResult {
                        tool_call_id: "call_1".into(),
                        content: "ok".into(),
                        images: Vec::new(),
                        is_error: false,
                        meta: None,
                    }],
                },
            ],
        );
        let identity = CursorIdeIdentity::load();
        let (payload, _) =
            build_stream_request(&request, "conv", "idem", 1, &identity, &test_blob_key());
        let body = String::from_utf8_lossy(&payload);
        assert!(body.contains("toolResults") || body.contains("clientSideToolV2Result"));
    }

    #[test]
    fn assistant_tool_call_history_uses_cursor_tool_meta() {
        let request = ProviderRequest::new(
            ModelRef::new("cursor", "composer-2.5-fast"),
            vec![
                ChatMessage {
                    role: Role::Assistant,
                    parts: vec![Part::ToolCall {
                        id: "call_1".into(),
                        name: "todo_list".into(),
                        input: serde_json::json!({}),
                        meta: Some(serde_json::json!({
                            "cursor_tool": "CLIENT_SIDE_TOOL_V2_TODO_READ"
                        })),
                    }],
                },
                ChatMessage {
                    role: Role::User,
                    parts: vec![Part::ToolResult {
                        tool_call_id: "call_1".into(),
                        content: "ok".into(),
                        images: Vec::new(),
                        is_error: false,
                        meta: None,
                    }],
                },
            ],
        );
        let identity = CursorIdeIdentity::load();
        let (payload, _) =
            build_stream_request(&request, "conv", "idem", 1, &identity, &test_blob_key());
        let body = String::from_utf8_lossy(&payload);
        assert!(body.contains("CLIENT_SIDE_TOOL_V2_TODO_READ"));
    }

    #[test]
    fn request_uses_conversation_cache_key_as_conversation_id() {
        let request = ProviderRequest::new(
            ModelRef::new("cursor", "composer-2.5-fast"),
            vec![ChatMessage::user_text("hello")],
        )
        .with_cache_key("sinew-conv-123");
        let identity = CursorIdeIdentity::load();
        let (payload, _) = build_stream_request(
            &request,
            "sinew-conv-123",
            "idem",
            0,
            &identity,
            &test_blob_key(),
        );
        let body = String::from_utf8_lossy(&payload);
        assert!(body.contains("sinew-conv-123"));
    }

    #[test]
    fn assistant_history_includes_tool_calls_in_ai_bubble() {
        let request = ProviderRequest::new(
            ModelRef::new("cursor", "composer-2.5-fast"),
            vec![ChatMessage {
                role: Role::Assistant,
                parts: vec![Part::ToolCall {
                    id: "call_1".into(),
                    name: "read".into(),
                    input: serde_json::json!({ "path": "a.rs" }),
                    meta: Some(serde_json::json!({
                        "cursor_tool": "CLIENT_SIDE_TOOL_V2_READ_FILE_V2"
                    })),
                }],
            }],
        );
        let identity = CursorIdeIdentity::load();
        let (payload, _) =
            build_stream_request(&request, "conv", "idem", 0, &identity, &test_blob_key());
        let body = String::from_utf8_lossy(&payload);
        assert!(body.contains("clientSideToolV2Calls"));
        assert!(body.contains("CLIENT_SIDE_TOOL_V2_READ_FILE_V2"));
    }

    // Tests live : ils dépendent d'un compte et du réseau, donc ils restent hors contrôle courant.
    #[tokio::test]
    #[ignore]
    async fn test_live_cursor_usage() {
        let provider = match crate::client::CursorProvider::from_default_sources() {
            Ok(provider) => provider,
            Err(err) => {
                println!("Skipping usage test: {err:?}");
                return;
            }
        };
        match provider.usage_snapshot().await {
            Ok(Some(usage)) => println!("USAGE OK: {usage:?}"),
            Ok(None) => println!("USAGE: not connected"),
            Err(err) => println!("USAGE ERROR: {err:?}"),
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_live_agent_usable_models() {
        let session = match crate::auth::composer::load_composer_session() {
            Ok(Some(session)) => session,
            _ => {
                println!("Skipping agent models test: not connected");
                return;
            }
        };
        let http = reqwest::Client::builder()
            .user_agent(crate::identity::CursorIdeIdentity::load().user_agent())
            .build()
            .expect("http client");
        let token = match crate::auth::composer::ensure_fresh_composer_token(&http, &session).await
        {
            Ok(token) => token,
            Err(err) => {
                println!("Skipping agent models test: {err:?}");
                return;
            }
        };
        let identity = crate::identity::CursorIdeIdentity::load();
        match crate::agent::fetch_usable_models(&http, &identity, &token).await {
            Ok(bytes) => {
                let models = crate::agent::scan_model_ids(&bytes);
                println!(
                    "GetUsableModels OK: {} bytes, models={models:?}",
                    bytes.len()
                );
                assert!(!bytes.is_empty());
            }
            Err(err) => println!("GetUsableModels ERROR: {err:?}"),
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_live_rust_agent_bridge() {
        use futures::StreamExt;
        use sinew_core::{ChatMessage, ModelRef, ProviderRequest, StreamEvent};

        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("sinew_cursor=debug")),
            )
            .with_test_writer()
            .try_init();

        let session = match crate::auth::composer::load_composer_session() {
            Ok(Some(session)) => session,
            _ => {
                println!("Skipping rust bridge live test: not connected");
                return;
            }
        };
        let http = reqwest::Client::builder()
            .user_agent(crate::identity::CursorIdeIdentity::load().user_agent())
            .build()
            .expect("http client");
        let token = match crate::auth::composer::ensure_fresh_composer_token(&http, &session).await
        {
            Ok(token) => token,
            Err(err) => {
                println!("Skipping rust bridge live test: {err:?}");
                return;
            }
        };
        let identity = crate::identity::CursorIdeIdentity::load();
        let test_model =
            std::env::var("SINEW_TEST_MODEL").unwrap_or_else(|_| "composer-2.5".to_string());
        let request = ProviderRequest::new(
            ModelRef::new("cursor", &test_model),
            vec![ChatMessage::user_text("Dis OK en une phrase.")],
        )
        .with_workspace_root(r"C:\Dev\Sinew")
        .with_cache_key(format!("rust-live-{}", uuid::Uuid::new_v4()));
        let assert_live = std::env::var("SINEW_CURSOR_LIVE_ASSERT")
            .map(|v| v.trim() == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        match crate::agent::stream_via_rust_bridge(&identity, token, request).await {
            Ok(mut stream) => {
                let mut saw_text = false;
                while let Some(event) = stream.next().await {
                    match event {
                        Ok(StreamEvent::TextDelta { .. })
                        | Ok(StreamEvent::ThinkingDelta { .. }) => {
                            saw_text = true;
                        }
                        Err(err) => {
                            println!("RUST BRIDGE STREAM ERROR: {err:?}");
                            if assert_live {
                                panic!("rust agent bridge stream error: {err:?}");
                            }
                            return;
                        }
                        Ok(_) => {}
                    }
                }
                println!("Rust agent bridge live: saw_text={saw_text}");
                if assert_live && !saw_text {
                    panic!("rust agent bridge returned no text");
                }
            }
            Err(err) => {
                println!("RUST BRIDGE ERROR: {err:?}");
                if assert_live {
                    panic!("rust agent bridge failed: {err:?}");
                }
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_live_composer_request() {
        use futures::StreamExt;
        use sinew_core::Provider;

        let provider = match crate::client::CursorProvider::from_default_sources() {
            Ok(provider) => provider,
            Err(e) => {
                println!("Skipping live test: unable to load provider: {e:?}");
                return;
            }
        };
        let identity = crate::identity::CursorIdeIdentity::load();
        println!("Using machine_id={}", identity.machine_id);
        println!("mac_machine_id={:?}", identity.mac_machine_id);
        let request = ProviderRequest::new(
            ModelRef::new("cursor", "composer-2.5"),
            vec![ChatMessage::user_text("Say OK")],
        )
        .with_workspace_root(r"C:\Dev\sinew")
        .with_cache_key(format!("live-test-{}", uuid::Uuid::new_v4()));
        println!("Sending live Composer request...");
        let assert_live = std::env::var("SINEW_CURSOR_LIVE_ASSERT")
            .map(|value| {
                let trimmed = value.trim();
                trimmed == "1" || trimmed.eq_ignore_ascii_case("true")
            })
            .unwrap_or(false);
        match provider.stream(request).await {
            Ok(mut stream) => {
                println!("Stream established. Reading events:");
                let mut saw_message = false;
                while let Some(event) = stream.next().await {
                    println!("EVENT: {:?}", event);
                    match event {
                        Ok(sinew_core::StreamEvent::MessageStart { .. })
                        | Ok(sinew_core::StreamEvent::TextDelta { .. })
                        | Ok(sinew_core::StreamEvent::ThinkingDelta { .. }) => {
                            saw_message = true;
                        }
                        Ok(_) => {}
                        Err(err) => {
                            println!("STREAM EVENT ERROR: {:?}", err);
                            if assert_live {
                                panic!("live Composer stream error: {err:?}");
                            }
                        }
                    }
                }
                if assert_live && !saw_message {
                    panic!(
                        "live Composer stream returned no text/thinking events (idempotent key still blocked?)"
                    );
                }
            }
            Err(err) => {
                println!("STREAM ERROR: {:?}", err);
                if assert_live {
                    panic!("live Composer stream failed: {err:?}");
                }
            }
        }
    }

    #[test]
    fn sinew_ui_fast_default_maps_to_composer_25_fast() {
        use sinew_core::ServiceTier;

        let ui_model = ModelRef::new("cursor", "composer-2.5");
        let effective =
            crate::model_info::resolve_agent_model_id(&ui_model, Some(ServiceTier::Fast));
        assert_eq!(effective, crate::model_info::MODEL_COMPOSER_25_FAST);
    }

    async fn live_sinew_stream(
        label: &str,
        request: ProviderRequest,
        max_wait: std::time::Duration,
    ) -> Result<(), String> {
        use futures::StreamExt;
        use sinew_core::{Provider, StreamEvent};

        let provider = crate::client::CursorProvider::from_default_sources()
            .map_err(|e| format!("provider: {e}"))?;
        let setup_started = std::time::Instant::now();
        let mut stream = tokio::time::timeout(max_wait, provider.stream(request))
            .await
            .map_err(|_| format!("stream setup timed out after {:?}", max_wait))?
            .map_err(|e| format!("stream setup: {e}"))?;
        println!(
            "{label}: stream setup OK in {}ms",
            setup_started.elapsed().as_millis()
        );
        let read_started = std::time::Instant::now();
        let result = tokio::time::timeout(max_wait, async {
            let mut saw_lifecycle = false;
            while let Some(event) = stream.next().await {
                match event {
                    Ok(StreamEvent::MessageStart { model }) => {
                        println!("{label}: MessageStart model={model}");
                        // Same criterion as sinew_app::agent::turn "first token received".
                        saw_lifecycle = true;
                        break;
                    }
                    Ok(StreamEvent::TextDelta { .. }) | Ok(StreamEvent::ThinkingDelta { .. }) => {
                        saw_lifecycle = true;
                        break;
                    }
                    Ok(_) => {}
                    Err(err) => return Err(format!("stream event: {err}")),
                }
            }
            if saw_lifecycle {
                Ok(())
            } else {
                Err("stream ended before MessageStart or text/thinking".into())
            }
        })
        .await;
        match result {
            Ok(Ok(())) => {
                println!(
                    "{label}: OK (first event in {}ms)",
                    read_started.elapsed().as_millis()
                );
                Ok(())
            }
            Ok(Err(err)) => Err(err),
            Err(_) => Err(format!("read timed out after {:?}", max_wait)),
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_live_sinew_composer_25_fast_tier() {
        use sinew_core::{ChatMessage, ServiceTier};

        let request = ProviderRequest::new(
            ModelRef::new("cursor", "composer-2.5"),
            vec![ChatMessage::user_text("Réponds uniquement: OK")],
        )
        .with_service_tier(ServiceTier::Fast)
        .with_workspace_root(r"C:\Dev\sinew")
        .with_cache_key(format!("sinew-fast-{}", uuid::Uuid::new_v4()));

        let effective =
            crate::model_info::resolve_agent_model_id(&request.model, request.service_tier);
        println!("Sinew UI model=composer-2.5 + fast tier → agent model_id={effective}");

        match live_sinew_stream(
            "composer-2.5 + ServiceTier::Fast (éclair Sinew)",
            request,
            std::time::Duration::from_secs(90),
        )
        .await
        {
            Ok(()) => {}
            Err(err) => {
                println!("FAIL: {err}");
                if std::env::var("SINEW_CURSOR_LIVE_ASSERT")
                    .map(|v| v.trim() == "1" || v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false)
                {
                    panic!("{err}");
                }
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_live_sinew_composer_25_no_fast_tier() {
        let request = ProviderRequest::new(
            ModelRef::new("cursor", "composer-2.5"),
            vec![ChatMessage::user_text("Réponds uniquement: OK")],
        )
        .with_workspace_root(r"C:\Dev\sinew")
        .with_cache_key(format!("sinew-slow-{}", uuid::Uuid::new_v4()));

        let effective =
            crate::model_info::resolve_agent_model_id(&request.model, request.service_tier);
        println!("Sinew UI model=composer-2.5 sans fast → agent model_id={effective}");

        match live_sinew_stream(
            "composer-2.5 sans fast",
            request,
            std::time::Duration::from_secs(90),
        )
        .await
        {
            Ok(()) => {}
            Err(err) => {
                println!("FAIL: {err}");
                if std::env::var("SINEW_CURSOR_LIVE_ASSERT")
                    .map(|v| v.trim() == "1" || v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false)
                {
                    panic!("{err}");
                }
            }
        }
    }
}
