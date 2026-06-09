use std::sync::Arc;

use anyhow::{anyhow, bail, Result};
use futures_util::StreamExt;
use serde_json::{json, Value};
use sinew_core::{
    AppError, ChatMessage, ModelRef, Part, Provider, ProviderRequest, Role, ServiceTier,
    StreamEvent,
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
const COMPACTION_INPUT_SAFETY_TOKENS: u32 = 1_024;

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

    let prompt = compaction_prompt(user_instruction.as_deref());
    let request_history = build_compaction_request_history_that_fits(
        provider.as_ref(),
        &model,
        &system_prompt,
        &history,
        &prompt,
        cache_key.as_deref(),
        cache_stable_message_count,
        service_tier,
    )
    .await;

    let request = build_compaction_provider_request(
        &model,
        &system_prompt,
        request_history,
        cache_key.as_deref(),
        cache_stable_message_count,
        service_tier,
    );

    let summary = match stream_compaction_summary(
        provider.as_ref(),
        request,
        cmd_rx,
        summary_delta_tx.as_ref(),
    )
    .await
    {
        Ok(summary) => summary,
        Err(err) if is_context_length_anyhow(&err) => {
            let retry_history = build_skipped_latest_compaction_history(&history, &prompt);
            let retry_request = build_compaction_provider_request(
                &model,
                &system_prompt,
                retry_history,
                cache_key.as_deref(),
                cache_stable_message_count,
                service_tier,
            );
            stream_compaction_summary(
                provider.as_ref(),
                retry_request,
                cmd_rx,
                summary_delta_tx.as_ref(),
            )
            .await?
        }
        Err(err) => return Err(err),
    };

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

fn build_compaction_provider_request(
    model: &ModelRef,
    system_prompt: &str,
    request_history: Vec<ChatMessage>,
    cache_key: Option<&str>,
    cache_stable_message_count: usize,
    service_tier: Option<ServiceTier>,
) -> ProviderRequest {
    let stable_message_count = cache_stable_message_count.min(request_history.len());
    let mut request = ProviderRequest::new(model.clone(), request_history)
        .with_system(system_prompt.to_string())
        .with_cache_stable_message_count(stable_message_count);
    if let Some(cache_key) = cache_key {
        request = request.with_cache_key(cache_key.to_string());
    }
    if let Some(service_tier) = service_tier {
        request = request.with_service_tier(service_tier);
    }
    request
}

async fn stream_compaction_summary(
    provider: &dyn Provider,
    request: ProviderRequest,
    cmd_rx: &mut mpsc::UnboundedReceiver<EngineCommand>,
    summary_delta_tx: Option<&mpsc::UnboundedSender<String>>,
) -> Result<String> {
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
                        if let Some(tx) = summary_delta_tx {
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

    Ok(summary)
}

async fn build_compaction_request_history_that_fits(
    provider: &dyn Provider,
    model: &ModelRef,
    system_prompt: &str,
    history: &[ChatMessage],
    prompt: &str,
    cache_key: Option<&str>,
    cache_stable_message_count: usize,
    service_tier: Option<ServiceTier>,
) -> Vec<ChatMessage> {
    let full_history = build_compaction_request_history(history, prompt);
    let Some(input_budget) = compaction_input_budget(provider, model) else {
        return full_history;
    };

    match compaction_history_fits(
        provider,
        model,
        system_prompt,
        &full_history,
        cache_key,
        cache_stable_message_count,
        service_tier,
        input_budget,
    )
    .await
    {
        Some(true) | None => return full_history,
        Some(false) => {}
    }

    let Some(latest_message) = history.last() else {
        return full_history;
    };
    let tail_chars = message_clampable_chars(latest_message);
    if tail_chars == 0 {
        return full_history;
    }

    let mut best = None;
    let mut low = 0usize;
    let mut high = tail_chars;
    while low <= high {
        let keep_chars = low + (high - low) / 2;
        let candidate = if keep_chars == 0 {
            build_skipped_latest_compaction_history(history, prompt)
        } else {
            build_tail_clamped_compaction_history(history, prompt, keep_chars)
        };
        match compaction_history_fits(
            provider,
            model,
            system_prompt,
            &candidate,
            cache_key,
            cache_stable_message_count,
            service_tier,
            input_budget,
        )
        .await
        {
            Some(true) => {
                best = Some(candidate);
                low = keep_chars.saturating_add(1);
            }
            Some(false) => {
                if keep_chars == 0 {
                    break;
                }
                high = keep_chars - 1;
            }
            None => return full_history,
        }
    }

    best.unwrap_or_else(|| build_skipped_latest_compaction_history(history, prompt))
}

async fn compaction_history_fits(
    provider: &dyn Provider,
    model: &ModelRef,
    system_prompt: &str,
    request_history: &[ChatMessage],
    cache_key: Option<&str>,
    cache_stable_message_count: usize,
    service_tier: Option<ServiceTier>,
    input_budget: u32,
) -> Option<bool> {
    let request = build_compaction_provider_request(
        model,
        system_prompt,
        request_history.to_vec(),
        cache_key,
        cache_stable_message_count,
        service_tier,
    );
    match provider.estimate_tokens(request).await {
        Ok(estimate) => Some(estimate.input_tokens <= input_budget),
        Err(err) if is_context_length_app_error(&err) => Some(false),
        Err(err) => {
            tracing::debug!(error = %err, "compaction token estimate failed; using unmodified history");
            None
        }
    }
}

fn compaction_input_budget(provider: &dyn Provider, model: &ModelRef) -> Option<u32> {
    let caps = provider.capabilities(model)?;
    if caps.context_window == 0 {
        return None;
    }
    let request = ProviderRequest::new(model.clone(), Vec::new());
    let output_budget = request.output_token_budget(&caps);
    let input_budget = caps.context_window.saturating_sub(output_budget);
    Some(if input_budget > COMPACTION_INPUT_SAFETY_TOKENS {
        input_budget - COMPACTION_INPUT_SAFETY_TOKENS
    } else {
        input_budget
    })
}

fn build_compaction_request_history(history: &[ChatMessage], prompt: &str) -> Vec<ChatMessage> {
    let mut request_history = history.to_vec();
    request_history.push(ChatMessage::user_text(prompt.to_string()));
    request_history
}

fn build_tail_clamped_compaction_history(
    history: &[ChatMessage],
    prompt: &str,
    keep_tail_chars: usize,
) -> Vec<ChatMessage> {
    let mut request_history = history.to_vec();
    if let Some(message) = request_history.last_mut() {
        clamp_message_to_prefix_chars(message, keep_tail_chars);
    }
    request_history.push(ChatMessage::user_text(prompt.to_string()));
    request_history
}

fn build_skipped_latest_compaction_history(
    history: &[ChatMessage],
    prompt: &str,
) -> Vec<ChatMessage> {
    let mut request_history = history.to_vec();
    skip_latest_compaction_block(&mut request_history);
    request_history.push(ChatMessage::user_text(prompt.to_string()));
    request_history
}

fn skip_latest_compaction_block(history: &mut Vec<ChatMessage>) {
    let Some(removed) = history.pop() else {
        return;
    };
    if message_has_tool_result(&removed) && history.last().is_some_and(message_has_tool_call) {
        history.pop();
    }
}

fn clamp_message_to_prefix_chars(message: &mut ChatMessage, max_chars: usize) {
    let mut remaining = max_chars;
    for part in &mut message.parts {
        clamp_part_to_prefix_chars(part, &mut remaining);
    }
}

fn clamp_part_to_prefix_chars(part: &mut Part, remaining: &mut usize) {
    match part {
        Part::Text { text, .. } | Part::Thinking { text, .. } => {
            clamp_string_to_prefix_chars(text, remaining);
        }
        Part::Image { data, .. } => {
            clamp_opaque_string_to_prefix_budget(data, remaining);
        }
        Part::ToolCall { .. } => {}
        Part::ToolResult {
            content, images, ..
        } => {
            clamp_string_to_prefix_chars(content, remaining);
            for image in images {
                clamp_opaque_string_to_prefix_budget(&mut image.data, remaining);
            }
        }
    }
}

fn clamp_string_to_prefix_chars(value: &mut String, remaining: &mut usize) {
    let char_count = value.chars().count();
    if char_count <= *remaining {
        *remaining -= char_count;
        return;
    }
    *value = value.chars().take(*remaining).collect();
    *remaining = 0;
}

fn clamp_opaque_string_to_prefix_budget(value: &mut String, remaining: &mut usize) {
    let char_count = value.chars().count();
    if char_count <= *remaining {
        *remaining -= char_count;
        return;
    }
    value.clear();
    *remaining = 0;
}

fn message_clampable_chars(message: &ChatMessage) -> usize {
    message.parts.iter().map(part_clampable_chars).sum()
}

fn part_clampable_chars(part: &Part) -> usize {
    match part {
        Part::Text { text, .. } | Part::Thinking { text, .. } => text.chars().count(),
        Part::Image { data, .. } => data.chars().count(),
        Part::ToolCall { .. } => 0,
        Part::ToolResult {
            content, images, ..
        } => {
            content.chars().count()
                + images
                    .iter()
                    .map(|image| image.data.chars().count())
                    .sum::<usize>()
        }
    }
}

fn message_has_tool_result(message: &ChatMessage) -> bool {
    message
        .parts
        .iter()
        .any(|part| matches!(part, Part::ToolResult { .. }))
}

fn message_has_tool_call(message: &ChatMessage) -> bool {
    message
        .parts
        .iter()
        .any(|part| matches!(part, Part::ToolCall { .. }))
}

fn is_context_length_anyhow(err: &anyhow::Error) -> bool {
    err.downcast_ref::<AppError>()
        .is_some_and(is_context_length_app_error)
}

fn is_context_length_app_error(err: &AppError) -> bool {
    matches!(err, AppError::ContextLength(_))
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
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    };

    use futures_util::stream;
    use sinew_core::{
        EffortMode, ModelCapabilities, ProviderStream, StopReason, TokenEstimate, Usage,
    };

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

    #[tokio::test]
    async fn compaction_slices_latest_message_for_request_only() {
        let provider = Arc::new(RecordingProvider::new(5_000, 100));
        let model = ModelRef::new("test", "model");
        let huge_output = "x".repeat(10_000);
        let history = vec![
            ChatMessage::user_text("continue the task"),
            assistant_tool_call("call-1"),
            tool_result_message("call-1", &huge_output),
        ];
        let original_history = history.clone();
        let (_cmd_tx, mut cmd_rx) = mpsc::unbounded_channel();

        let output = compact_conversation_history(
            provider.clone(),
            model,
            "system".to_string(),
            history,
            None,
            0,
            None,
            None,
            &mut cmd_rx,
            None,
        )
        .await
        .expect("compaction should succeed with a sliced request");

        assert_eq!(output.summary, "compact summary");
        let requests = provider.stream_requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        let sent_history = &requests[0].transcript;
        assert_eq!(sent_history.len(), original_history.len() + 1);
        let sent_tail = &sent_history[sent_history.len() - 2];
        let sent_content = tool_result_content(sent_tail);
        assert!(!sent_content.is_empty());
        assert!(sent_content.len() < huge_output.len());
        assert!(!sent_content.contains("truncated"));
        assert_eq!(tool_result_content(&original_history[2]), huge_output);
    }

    #[tokio::test]
    async fn compaction_retries_by_skipping_latest_block_on_stream_context_length() {
        let provider =
            Arc::new(RecordingProvider::new(50_000, 100).with_first_stream_context_error());
        let model = ModelRef::new("test", "model");
        let history = vec![
            ChatMessage::user_text("continue the task"),
            assistant_tool_call("call-1"),
            tool_result_message("call-1", &"x".repeat(1_000)),
        ];
        let (_cmd_tx, mut cmd_rx) = mpsc::unbounded_channel();

        let output = compact_conversation_history(
            provider.clone(),
            model,
            "system".to_string(),
            history,
            None,
            0,
            None,
            None,
            &mut cmd_rx,
            None,
        )
        .await
        .expect("compaction should retry with the latest block skipped");

        assert_eq!(output.summary, "compact summary");
        let requests = provider.stream_requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        let retry_history = &requests[1].transcript;
        assert_eq!(retry_history.len(), 2);
        assert!(matches!(retry_history[0].role, Role::User));
        assert!(!retry_history.iter().any(message_has_tool_call));
        assert!(!retry_history.iter().any(message_has_tool_result));
        assert!(retry_history[1].text().contains(COMPACTION_PROMPT));
    }

    struct RecordingProvider {
        caps: ModelCapabilities,
        stream_requests: Mutex<Vec<ProviderRequest>>,
        fail_first_stream_with_context_length: AtomicBool,
    }

    impl RecordingProvider {
        fn new(context_window: u32, max_output_tokens: u32) -> Self {
            let model = ModelRef::new("test", "model");
            Self {
                caps: ModelCapabilities {
                    model,
                    context_window,
                    preferred_window: context_window,
                    max_output_tokens,
                    supports_thinking: false,
                    visible_thinking: false,
                    supports_tools: true,
                    supports_images: true,
                    effort_mode: EffortMode::None,
                },
                stream_requests: Mutex::new(Vec::new()),
                fail_first_stream_with_context_length: AtomicBool::new(false),
            }
        }

        fn with_first_stream_context_error(self) -> Self {
            self.fail_first_stream_with_context_length
                .store(true, Ordering::SeqCst);
            self
        }
    }

    #[async_trait::async_trait]
    impl Provider for RecordingProvider {
        fn name(&self) -> &str {
            "recording"
        }

        fn capabilities(&self, _model: &ModelRef) -> Option<ModelCapabilities> {
            Some(self.caps.clone())
        }

        async fn estimate_tokens(
            &self,
            request: ProviderRequest,
        ) -> sinew_core::Result<TokenEstimate> {
            Ok(TokenEstimate {
                input_tokens: request_chars(&request),
                exact: true,
            })
        }

        async fn stream(&self, request: ProviderRequest) -> sinew_core::Result<ProviderStream> {
            self.stream_requests.lock().unwrap().push(request);
            if self
                .fail_first_stream_with_context_length
                .swap(false, Ordering::SeqCst)
            {
                return Err(AppError::ContextLength("too large".to_string()));
            }
            Ok(Box::pin(stream::iter(vec![
                Ok(StreamEvent::TextDelta {
                    index: 0,
                    delta: "compact summary".to_string(),
                }),
                Ok(StreamEvent::MessageStop {
                    stop_reason: StopReason::EndTurn,
                    usage: Usage::default(),
                }),
            ])))
        }
    }

    fn assistant_tool_call(id: &str) -> ChatMessage {
        ChatMessage {
            role: Role::Assistant,
            parts: vec![Part::ToolCall {
                id: id.to_string(),
                name: "grep".to_string(),
                input: json!({ "pattern": "x" }),
                meta: None,
            }],
        }
    }

    fn tool_result_message(id: &str, content: &str) -> ChatMessage {
        ChatMessage {
            role: Role::User,
            parts: vec![Part::ToolResult {
                tool_call_id: id.to_string(),
                content: content.to_string(),
                images: Vec::new(),
                is_error: false,
                meta: None,
            }],
        }
    }

    fn tool_result_content(message: &ChatMessage) -> String {
        message
            .parts
            .iter()
            .filter_map(|part| match part {
                Part::ToolResult { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    fn request_chars(request: &ProviderRequest) -> u32 {
        let mut chars = request
            .system_prompt
            .as_deref()
            .map(str::chars)
            .map(Iterator::count)
            .unwrap_or_default();
        for message in &request.transcript {
            for part in &message.parts {
                chars += match part {
                    Part::Text { text, .. } | Part::Thinking { text, .. } => text.chars().count(),
                    Part::Image { data, .. } => data.chars().count(),
                    Part::ToolCall { name, input, .. } => {
                        name.chars().count() + input.to_string().chars().count()
                    }
                    Part::ToolResult {
                        content, images, ..
                    } => {
                        content.chars().count()
                            + images
                                .iter()
                                .map(|image| image.data.chars().count())
                                .sum::<usize>()
                    }
                };
            }
        }
        chars.min(u32::MAX as usize) as u32
    }
}
