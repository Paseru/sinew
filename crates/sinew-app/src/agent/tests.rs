use std::collections::BTreeSet;

use serde_json::json;

use sinew_core::{ChatMessage, Part, Role, StopReason};

use crate::{TodoListState, TodoStatus, TodoTask};

use super::{
    clean_context::{run_clean_context, tool_result_cleaned, CLEAN_CONTEXT_RESULT_PLACEHOLDER},
    history::{
        history_with_current_tool_result_ids, normalize_tool_call_inputs,
        strip_all_visible_tool_result_ids, tool_result_content_with_id, tool_result_exposes_id,
    },
    turn::{
        retain_cancelled_visible_parts, should_trigger_todo_final_answer_guard,
        todo_final_answer_guard_message,
    },
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
fn todo_final_answer_guard_triggers_for_active_list_and_final_text() {
    let state = active_todo_state();
    let assistant = ChatMessage::assistant_text("C'est termine ✅");

    assert!(should_trigger_todo_final_answer_guard(
        &state,
        true,
        StopReason::EndTurn,
        &assistant,
        false,
        0,
        8,
    ));
}

#[test]
fn todo_final_answer_guard_does_not_trigger_when_unavailable_or_already_attempted() {
    let state = active_todo_state();
    let assistant = ChatMessage::assistant_text("C'est termine ✅");

    assert!(!should_trigger_todo_final_answer_guard(
        &state,
        false,
        StopReason::EndTurn,
        &assistant,
        false,
        0,
        8,
    ));
    assert!(!should_trigger_todo_final_answer_guard(
        &state,
        true,
        StopReason::EndTurn,
        &assistant,
        true,
        0,
        8,
    ));
}

#[test]
fn todo_final_answer_guard_does_not_trigger_for_tool_use_or_closed_list() {
    let assistant = ChatMessage::assistant_text("Je vais mettre a jour la liste.");

    assert!(!should_trigger_todo_final_answer_guard(
        &active_todo_state(),
        true,
        StopReason::ToolUse,
        &assistant,
        false,
        0,
        8,
    ));
    assert!(!should_trigger_todo_final_answer_guard(
        &TodoListState::default(),
        true,
        StopReason::EndTurn,
        &assistant,
        false,
        0,
        8,
    ));
}

#[test]
fn todo_final_answer_guard_message_is_hidden_system_reminder() {
    let message = todo_final_answer_guard_message(&active_todo_state());
    assert_eq!(message.role, Role::User);
    let [Part::Text { text, meta }] = message.parts.as_slice() else {
        panic!("expected a single text part");
    };

    assert!(text.contains("<todo_final_answer_guard>"));
    assert!(text.contains("call todo_list now"));
    assert!(text.contains("1. [in_progress] Build + deploy"));
    assert_eq!(
        meta.as_ref()
            .and_then(|value| value.get("system_reminder"))
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert_eq!(
        meta.as_ref()
            .and_then(|value| value.get("todo_final_answer_guard"))
            .and_then(|value| value.as_bool()),
        Some(true)
    );
}

fn active_todo_state() -> TodoListState {
    TodoListState {
        active: true,
        tasks: vec![TodoTask {
            id: "1".to_string(),
            text: "Build + deploy".to_string(),
            status: TodoStatus::InProgress,
        }],
        next_id: 2,
    }
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
