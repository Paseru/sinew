#[cfg(test)]
mod tests {
    use sinew_core::{ChatMessage, ModelRef, Part, ProviderRequest, Role};

    use crate::{
        conversation::build_stream_request,
        identity::CursorIdeIdentity,
        sanitize::sanitize_outbound_text,
        tools::{build_client_tool_result, parse_tool_call, SUPPORTED_TOOLS},
    };

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
            build_stream_request(&request, "conv", "idem", 0, &identity, "key");
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
        let (payload, _) = build_stream_request(&request, "conv", "idem", 1, &identity, "key");
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
        let (payload, _) = build_stream_request(&request, "conv", "idem", 1, &identity, "key");
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
        let (payload, _) =
            build_stream_request(&request, "sinew-conv-123", "idem", 0, &identity, "key");
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
        let (payload, _) = build_stream_request(&request, "conv", "idem", 0, &identity, "key");
        let body = String::from_utf8_lossy(&payload);
        assert!(body.contains("clientSideToolV2Calls"));
        assert!(body.contains("CLIENT_SIDE_TOOL_V2_READ_FILE_V2"));
    }

    #[tokio::test]
    async fn test_live_composer_request() {
        use sinew_core::Provider;
        use futures::StreamExt;
        
        let provider = match crate::client::CursorProvider::from_default_sources() {
            Ok(provider) => provider,
            Err(e) => {
                println!("Skipping live test: unable to load provider: {e:?}");
                return;
            }
        };
        let request = ProviderRequest::new(
            ModelRef::new("cursor", "composer-2.5"),
            vec![ChatMessage::user_text("Say OK")],
        );
        println!("Sending live Composer request...");
        match provider.stream(request).await {
            Ok(mut stream) => {
                println!("Stream established. Reading events:");
                while let Some(event) = stream.next().await {
                    println!("EVENT: {:?}", event);
                }
            }
            Err(err) => {
                println!("STREAM ERROR: {:?}", err);
            }
        }
    }
}
