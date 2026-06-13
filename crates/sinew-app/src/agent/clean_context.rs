use std::collections::BTreeSet;

use serde_json::{json, Map, Value};

use sinew_core::{ChatMessage, Part, ToolDescriptor};

use crate::ToolRunResult;

pub(super) const CLEAN_CONTEXT_RESULT_PLACEHOLDER: &str =
    "[Tool result cleaned by you: irrelevant to future context.]";

pub fn clean_context_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        name: "clean_context".into(),
        description: "Prune useless or voluminous tool results from your context to keep it clean and fast. Highly recommended (often mandatory) before finishing your turn if you ran tool calls that produced significant noise (e.g. large irrelevant Glob/Grep outputs, reading a wrong/unrelated file, or failed attempts). Keep anything you quoted, referenced, edited, or based decisions on. If unsure, keep it. You can ONLY clean results from your current turn (which start with a visible 'tool_call_id' line); you cannot clean them retroactively in future turns.".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "tool_call_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Exact tool_call_id values."
                }
            },
            "required": ["tool_call_ids"],
            "additionalProperties": false
        }),
    }
}

pub(super) fn run_clean_context(
    history: &mut [ChatMessage],
    input: Value,
    current_turn_tool_result_ids: &BTreeSet<String>,
) -> ToolRunResult {
    let Some(values) = input
        .get("tool_call_ids")
        .or_else(|| input.get("ids"))
        .and_then(Value::as_array)
    else {
        return ToolRunResult::err("tool_call_ids must be an array", Vec::new());
    };
    let requested_ids = values
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let ids = requested_ids
        .intersection(current_turn_tool_result_ids)
        .cloned()
        .collect::<BTreeSet<_>>();
    let cleaned = clean_tool_results_by_ids(history, &ids);
    ToolRunResult::ok(
        format!(
            "cleaned: {}\nrequested: {}",
            cleaned.len(),
            requested_ids.len()
        ),
        Vec::new(),
    )
}

fn clean_tool_results_by_ids(history: &mut [ChatMessage], ids: &BTreeSet<String>) -> Vec<String> {
    let mut cleaned = Vec::new();
    if ids.is_empty() {
        return cleaned;
    }

    for message in history {
        for part in &mut message.parts {
            let Part::ToolResult {
                tool_call_id,
                content,
                images,
                meta,
                ..
            } = part
            else {
                continue;
            };
            if !ids.contains(tool_call_id) {
                continue;
            }
            *content = CLEAN_CONTEXT_RESULT_PLACEHOLDER.to_string();
            images.clear();
            mark_tool_result_cleaned(meta);
            cleaned.push(tool_call_id.clone());
        }
    }
    cleaned
}

fn mark_tool_result_cleaned(meta: &mut Option<Value>) {
    let mut map = match meta.take() {
        Some(Value::Object(map)) => map,
        Some(value) => {
            let mut map = Map::new();
            map.insert("previous_meta".into(), value);
            map
        }
        None => Map::new(),
    };
    map.insert("tool_result_cleaned".into(), json!(true));
    *meta = Some(Value::Object(map));
}

pub(super) fn tool_result_cleaned(meta: &Option<Value>) -> bool {
    meta.as_ref()
        .and_then(|meta| meta.get("tool_result_cleaned"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}
