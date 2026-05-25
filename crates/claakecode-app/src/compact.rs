use std::sync::Arc;

use anyhow::{anyhow, bail, Result};
use futures_util::StreamExt;
use serde_json::{json, Value};
use claakecode_core::{
    ChatMessage, ModelRef, Part, Provider, ProviderRequest, Role, ServiceTier, StreamEvent,
};
use tokio::sync::mpsc;

use crate::agent::EngineCommand;

const COMPACTION_PROMPT: &str = r#"You are performing a CONTEXT CHECKPOINT COMPACTION. Create a handoff summary for another LLM that will resume the task.

Include:
- Current progress and key decisions made
- Important context, constraints, or user preferences
- What remains to be done (clear next steps)
- Any critical data, examples, or references needed to continue

Be concise, structured, and focused on helping the next LLM seamlessly continue the work."#;

const SUMMARY_PREFIX: &str = r#"Another language model started to solve this problem and produced a summary of its thinking process. You also have access to the state of the tools that were used by that language model. Use this to build on the work that has already been done and avoid duplicating work. Here is the summary produced by the other language model, use the information in this summary to assist with your own analysis:"#;

const MAX_RETAINED_USER_CHARS: usize = 80_000;

#[derive(Debug, Clone)]
pub struct CompactConversationOutput {
    pub history: Vec<ChatMessage>,
    pub retained_user_messages: usize,
    pub summary: String,
}

pub async fn compact_conversation_history(
    provider: Arc<dyn Provider>,
    model: ModelRef,
    system_prompt: String,
    history: Vec<ChatMessage>,
    cache_key: Option<String>,
    cache_stable_message_count: usize,
    service_tier: Option<ServiceTier>,
    user_instruction: Option<String>,
    cmd_rx: &mut mpsc::UnboundedReceiver<EngineCommand>,
    summary_delta_tx: Option<mpsc::UnboundedSender<String>>,
) -> Result<CompactConversationOutput> {
    if history.is_empty() {
        bail!("conversation has no history to compact");
    }

    let mut request_history = history.clone();
    request_history.push(ChatMessage::user_text(compaction_prompt(
        user_instruction.as_deref(),
    )));

    let mut request = ProviderRequest::new(model, request_history)
        .with_system(system_prompt)
        .with_cache_stable_message_count(cache_stable_message_count);
    if let Some(cache_key) = cache_key {
        request = request.with_cache_key(cache_key);
    }
    if let Some(service_tier) = service_tier {
        request = request.with_service_tier(service_tier);
    }

    let mut stream = provider.stream(request).await?;
    let mut summary = String::new();
    let mut completed = false;

    loop {
        tokio::select! {
            biased;

            command = cmd_rx.recv() => {
                if matches!(command, Some(EngineCommand::Cancel)) {
                    bail!("compaction cancelled");
                }
            }
            event = stream.next() => {
                let Some(event) = event else {
                    break;
                };
                match event? {
                    StreamEvent::TextDelta { delta, .. } => {
                        if let Some(tx) = &summary_delta_tx {
                            let _ = tx.send(delta.clone());
                        }
                        summary.push_str(&delta);
                    }
                    StreamEvent::MessageStop { .. } => {
                        completed = true;
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    if !completed {
        bail!("compaction stream closed before completion");
    }

    let summary = summary.trim();
    if summary.is_empty() {
        return Err(anyhow!("compaction produced an empty summary"));
    }

    let compacted = build_compacted_history(&history, summary);
    Ok(CompactConversationOutput {
        retained_user_messages: compacted.retained_user_messages,
        history: compacted.history,
        summary: summary.to_string(),
    })
}

fn compaction_prompt(user_instruction: Option<&str>) -> String {
    let Some(instruction) = user_instruction
        .map(str::trim)
        .filter(|instruction| !instruction.is_empty())
    else {
        return COMPACTION_PROMPT.to_string();
    };

    format!(
        r#"{COMPACTION_PROMPT}

Additional user instruction for this compaction:
{instruction}

Honor this instruction when deciding what to keep. If it asks to focus on a topic or subset, summarize only the relevant context and omit unrelated details unless they are necessary for continuity."#
    )
}

struct BuiltCompactedHistory {
    history: Vec<ChatMessage>,
    retained_user_messages: usize,
}

fn build_compacted_history(history: &[ChatMessage], summary: &str) -> BuiltCompactedHistory {
    let retained_user_messages = collect_recent_user_messages(history);
    let mut compacted = retained_user_messages
        .iter()
        .map(|message| ChatMessage {
            role: Role::User,
            parts: vec![Part::Text {
                text: message.clone(),
                meta: Some(json!({ "compaction_retained_user": true })),
            }],
        })
        .collect::<Vec<_>>();

    compacted.push(ChatMessage {
        role: Role::User,
        parts: vec![Part::Text {
            text: format!("{SUMMARY_PREFIX}\n\n{summary}"),
            meta: Some(json!({ "compaction_summary": true })),
        }],
    });

    BuiltCompactedHistory {
        retained_user_messages: retained_user_messages.len(),
        history: compacted,
    }
}

fn collect_recent_user_messages(history: &[ChatMessage]) -> Vec<String> {
    let user_messages = history
        .iter()
        .filter_map(visible_user_text)
        .filter(|message| !is_compaction_summary(message))
        .collect::<Vec<_>>();

    let mut selected = Vec::new();
    let mut remaining = MAX_RETAINED_USER_CHARS;
    for message in user_messages.iter().rev() {
        if remaining == 0 {
            break;
        }
        let char_count = message.chars().count();
        if char_count <= remaining {
            selected.push(message.clone());
            remaining = remaining.saturating_sub(char_count);
        } else {
            selected.push(truncate_chars(message, remaining));
            break;
        }
    }
    selected.reverse();
    selected
}

fn visible_user_text(message: &ChatMessage) -> Option<String> {
    if message.role != Role::User {
        return None;
    }
    let parts = message
        .parts
        .iter()
        .filter_map(|part| match part {
            Part::Text { text, meta } if !is_hidden_user_text(meta) => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let joined = parts.join("");
    let trimmed = joined.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn is_hidden_user_text(meta: &Option<Value>) -> bool {
    let Some(Value::Object(meta)) = meta else {
        return false;
    };
    meta.get("attachment_context").and_then(Value::as_bool) == Some(true)
        || meta.get("ui_only").and_then(Value::as_bool) == Some(true)
        || meta.get("system_reminder").and_then(Value::as_bool) == Some(true)
        || meta.get("compaction_summary").and_then(Value::as_bool) == Some(true)
        || meta.get("plan_control").and_then(Value::as_str).is_some()
}

fn is_compaction_summary(message: &str) -> bool {
    message.starts_with(SUMMARY_PREFIX)
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    const MARKER: &str = "\n\n[truncated during compaction]";
    if max_chars <= MARKER.chars().count() {
        return value.chars().take(max_chars).collect();
    }
    let keep = max_chars - MARKER.chars().count();
    let mut output = value.chars().take(keep).collect::<String>();
    output.push_str(MARKER);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compaction_prompt_without_instruction_is_default_prompt() {
        assert_eq!(compaction_prompt(None), COMPACTION_PROMPT);
        assert_eq!(compaction_prompt(Some("   \n  ")), COMPACTION_PROMPT);
    }

    #[test]
    fn compaction_prompt_includes_manual_instruction() {
        let prompt = compaction_prompt(Some("  Keep only topic X.  "));

        assert!(prompt.starts_with(COMPACTION_PROMPT));
        assert!(prompt.contains("Additional user instruction for this compaction:"));
        assert!(prompt.contains("Keep only topic X."));
        assert!(prompt.contains("Honor this instruction"));
    }
}
