use std::collections::BTreeSet;

use serde_json::json;

use claakecode_core::{ChatMessage, Part, Role};

use super::{
    clean_context::{run_clean_context, tool_result_cleaned, CLEAN_CONTEXT_RESULT_PLACEHOLDER},
    history::{
        history_with_current_tool_result_ids, normalize_tool_call_inputs,
        strip_all_visible_tool_result_ids, tool_result_content_with_id, tool_result_exposes_id,
    },
    turn::retain_cancelled_visible_parts,
};

#[test]
fn cancelled_visible_parts_keep_partial_text_only() {
    let mut message = ChatMessage {
        role: Role::Assistant,
        parts: vec![
            Part::Text {
                text: "partial answer".to_string(),
                meta: None,
            },
            Part::Thinking {
                text: "partial thought".to_string(),
                meta: None,
            },
            Part::Text {
                text: String::new(),
                meta: None,
            },
            Part::ToolCall {
                id: "call-1".to_string(),
                name: "read".to_string(),
                input: json!({ "path": "Cargo.toml" }),
                meta: None,
            },
        ],
    };

    retain_cancelled_visible_parts(&mut message);

    assert_eq!(message.parts.len(), 2);
    assert!(matches!(&message.parts[0], Part::Text { text, .. } if text == "partial answer"));
    assert!(matches!(&message.parts[1], Part::Thinking { text, .. } if text == "partial thought"));
}

#[test]
fn clean_context_replaces_matching_tool_results() {
    let mut history = vec![ChatMessage {
        role: Role::User,
        parts: vec![
            Part::ToolResult {
                tool_call_id: "call-1".to_string(),
                content: "noisy grep output".to_string(),
                images: Vec::new(),
                is_error: false,
                meta: None,
            },
            Part::ToolResult {
                tool_call_id: "call-2".to_string(),
                content: "useful read output".to_string(),
                images: Vec::new(),
                is_error: false,
                meta: None,
            },
        ],
    }];

    let result = run_clean_context(
        &mut history,
        json!({ "tool_call_ids": ["call-1", "missing"] }),
        &BTreeSet::from(["call-1".to_string()]),
    );

    assert!(!result.is_error);
    assert!(result.content.contains("cleaned: 1"));
    let Part::ToolResult { content, meta, .. } = &history[0].parts[0] else {
        panic!("expected tool result");
    };
    assert_eq!(content, CLEAN_CONTEXT_RESULT_PLACEHOLDER);
    assert!(tool_result_cleaned(meta));

    let Part::ToolResult { content, .. } = &history[0].parts[1] else {
        panic!("expected tool result");
    };
    assert_eq!(content, "useful read output");
}

#[test]
fn clean_context_ignores_ids_outside_current_turn() {
    let mut history = vec![ChatMessage {
        role: Role::User,
        parts: vec![Part::ToolResult {
            tool_call_id: "old-call".to_string(),
            content: "old useful output".to_string(),
            images: Vec::new(),
            is_error: false,
            meta: None,
        }],
    }];

    let result = run_clean_context(
        &mut history,
        json!({ "tool_call_ids": ["old-call"] }),
        &BTreeSet::new(),
    );

    assert!(!result.is_error);
    assert!(result.content.contains("cleaned: 0"));
    assert!(result.content.contains("requested: 1"));
    let Part::ToolResult { content, meta, .. } = &history[0].parts[0] else {
        panic!("expected tool result");
    };
    assert_eq!(content, "old useful output");
    assert!(!tool_result_cleaned(meta));
}

#[test]
fn tool_result_content_exposes_tool_call_id() {
    assert_eq!(
        tool_result_content_with_id("call-1", "hello"),
        "tool_call_id: call-1\nhello"
    );
}

#[test]
fn request_history_exposes_only_current_turn_tool_result_ids() {
    let history = vec![ChatMessage {
        role: Role::User,
        parts: vec![
            Part::ToolResult {
                tool_call_id: "call-1".to_string(),
                content: "current result".to_string(),
                images: Vec::new(),
                is_error: false,
                meta: None,
            },
            Part::ToolResult {
                tool_call_id: "call-2".to_string(),
                content: "old result".to_string(),
                images: Vec::new(),
                is_error: false,
                meta: None,
            },
        ],
    }];
    let ids = BTreeSet::from(["call-1".to_string()]);

    let request_history = history_with_current_tool_result_ids(&history, &ids);
    let Part::ToolResult {
        content: current_content,
        ..
    } = &request_history[0].parts[0]
    else {
        panic!("expected tool result");
    };
    let Part::ToolResult {
        content: old_content,
        ..
    } = &request_history[0].parts[1]
    else {
        panic!("expected tool result");
    };

    assert!(tool_result_exposes_id(current_content));
    assert!(!tool_result_exposes_id(old_content));
    let Part::ToolResult { content, .. } = &history[0].parts[0] else {
        panic!("expected tool result");
    };
    assert!(!tool_result_exposes_id(content));
}

#[test]
fn legacy_visible_tool_result_ids_are_stripped_from_history() {
    let mut history = vec![ChatMessage {
        role: Role::User,
        parts: vec![Part::ToolResult {
            tool_call_id: "call-1".to_string(),
            content: "tool_call_id: call-1\nhello".to_string(),
            images: Vec::new(),
            is_error: false,
            meta: None,
        }],
    }];

    strip_all_visible_tool_result_ids(&mut history);

    let Part::ToolResult { content, .. } = &history[0].parts[0] else {
        panic!("expected tool result");
    };
    assert_eq!(content, "hello");
}

#[test]
fn tool_call_inputs_are_normalized_for_provider_replay() {
    let mut history = vec![ChatMessage {
        role: Role::Assistant,
        parts: vec![
            Part::ToolCall {
                id: "call-empty".to_string(),
                name: "TeamStop".to_string(),
                input: json!(""),
                meta: None,
            },
            Part::ToolCall {
                id: "call-json".to_string(),
                name: "TeamStop".to_string(),
                input: json!("{\"agent\":\"ui\"}"),
                meta: None,
            },
            Part::ToolCall {
                id: "call-string".to_string(),
                name: "bash".to_string(),
                input: json!("ls"),
                meta: None,
            },
        ],
    }];

    normalize_tool_call_inputs(&mut history);

    let Part::ToolCall {
        input: empty_input, ..
    } = &history[0].parts[0]
    else {
        panic!("expected tool call");
    };
    let Part::ToolCall {
        input: json_input, ..
    } = &history[0].parts[1]
    else {
        panic!("expected tool call");
    };
    let Part::ToolCall {
        input: string_input,
        ..
    } = &history[0].parts[2]
    else {
        panic!("expected tool call");
    };

    assert_eq!(empty_input, &json!({}));
    assert_eq!(json_input, &json!({ "agent": "ui" }));
    assert_eq!(string_input, &json!({ "value": "ls" }));
}
