use std::path::Path;

use sinew_core::{Part, ProviderRequest, Role};

use crate::sanitize::sanitize_outbound_text;

const MIN_QUERY_CHARS: usize = 12;
const MAX_INJECTED_CHARS: usize = 12_000;
const INJECT_CHUNK_LIMIT: usize = 8;

pub fn append_local_index_excerpts(request: &ProviderRequest, context: &mut String) {
    if is_tool_result_continuation(request) {
        return;
    }
    let Some(workspace_root) = request.workspace_root.as_deref() else {
        return;
    };
    let Some(query) = latest_user_search_query(request) else {
        return;
    };
    if query.chars().count() < MIN_QUERY_CHARS {
        return;
    }

    let root = Path::new(workspace_root);
    let Ok((_stats, hits)) =
        sinew_index::index_and_search_workspace_isolated(root, &query, None, INJECT_CHUNK_LIMIT)
    else {
        return;
    };
    if hits.is_empty() {
        return;
    }

    context.push_str("\n\n## Relevant workspace excerpts\n");
    let mut remaining = MAX_INJECTED_CHARS;
    for hit in hits {
        let block = format!(
            "\n### {}:{}-{}\n{}\n",
            hit.path,
            hit.start_line,
            hit.end_line,
            hit.snippet.replace("[[", "").replace("]]", "")
        );
        let block = sanitize_outbound_text(&block);
        if block.len() > remaining {
            break;
        }
        context.push_str(&block);
        remaining -= block.len();
    }
}

fn latest_user_search_query(request: &ProviderRequest) -> Option<String> {
    for message in request.transcript.iter().rev() {
        if message.role != Role::User {
            continue;
        }
        let text = user_visible_text(message);
        if text.chars().count() >= MIN_QUERY_CHARS {
            return Some(text);
        }
        let only_tool_results = message
            .parts
            .iter()
            .all(|part| matches!(part, Part::ToolResult { .. }));
        if only_tool_results {
            continue;
        }
    }
    None
}

fn user_visible_text(message: &sinew_core::ChatMessage) -> String {
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            Part::Text { text, .. } if !text.trim().is_empty() => {
                Some(sanitize_outbound_text(text))
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_tool_result_continuation(request: &ProviderRequest) -> bool {
    let Some(last) = request.transcript.last() else {
        return false;
    };
    last.role == Role::User
        && !last.parts.is_empty()
        && last
            .parts
            .iter()
            .all(|part| matches!(part, Part::ToolResult { .. }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sinew_core::{ChatMessage, ModelRef, ProviderRequest};

    #[test]
    fn skips_injection_on_tool_result_turns() {
        let mut context = String::from("base");
        let request = ProviderRequest::new(
            ModelRef::new("cursor", "composer-2.5-fast"),
            vec![ChatMessage {
                role: Role::User,
                parts: vec![Part::ToolResult {
                    tool_call_id: "call_1".into(),
                    content: "ok".into(),
                    images: Vec::new(),
                    is_error: false,
                    meta: None,
                }],
            }],
        )
        .with_workspace_root("/tmp/example");
        append_local_index_excerpts(&request, &mut context);
        assert_eq!(context, "base");
    }
}
