use crate::*;

#[tauri::command]
pub(super) async fn estimate_context(
    state: State<'_, DesktopState>,
    input: ContextEstimateInput,
) -> std::result::Result<ContextEstimateOutput, String> {
    let requested_mode = input.mode.map(AgentMode::from).unwrap_or_default();
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let workspace_id = workspace_root.display().to_string();
    let effective_system_prompt =
        system_prompt_for_workspace(&workspace_root, &state.system_prompt)
            .map_err(error_to_string)?;

    let mut conversation = state
        .store
        .load_conversation(&workspace_id, &input.conversation_id)
        .map_err(error_to_string)?
        .ok_or_else(|| "conversation not found".to_string())?;

    if let Some(index) = input.rewrite_from_history_index {
        if index >= conversation.history.len() {
            return Err("rewrite index out of bounds".into());
        }
        let message = &conversation.history[index];
        if !is_rewritable_user_message(message) {
            return Err("rewrite index must point to a rewritable user message".into());
        }
        conversation.history.truncate(index);
        conversation.todo_list = todo_list_from_history(&conversation.history);
        conversation.plan_workflow = PlanWorkflowState::Idle;
    }

    let mode = plan_estimate_mode(&conversation.plan_workflow, requested_mode);
    let mode_model_settings = conversation.mode_model_settings.clone();
    let selected_model = model_with_optional_selection(
        mode_model_settings.get(mode),
        input.model,
        input.thinking,
        input.use_1m_context,
    );
    conversation.mode_model_settings = mode_model_settings;
    conversation.model = selected_model;
    let provider = provider_from_registry(&state, &conversation.model.provider)?;

    let draft = input.text.trim();
    let has_pending_user_input = !draft.is_empty() || !input.attachments.is_empty();
    let cache_stable_message_count = conversation.history.len();
    if has_pending_user_input {
        conversation.history.push(build_user_message(
            draft,
            &input.attachments,
            &workspace_root,
            None,
            MessageVisibilityInput::Normal,
        ));
    }

    let tool_settings = state.store.load_tool_settings().map_err(error_to_string)?;
    let skill_settings = state.store.load_skill_settings().map_err(error_to_string)?;
    let mut tools = tool_descriptors_for_workspace(&workspace_root, mode, &skill_settings);
    let mcp_settings = state.store.load_mcp_settings().map_err(error_to_string)?;
    let mcp = McpToolRegistry::new(mcp_settings.clone());
    let mcp_tools = mcp.refresh_catalog(&conversation.history).await;
    let mcp_tool_names = tool_name_set(&mcp_tools);
    tools.extend(mcp_tools);
    let sub_agent_settings = state
        .store
        .load_sub_agent_settings()
        .map_err(error_to_string)?;
    let sub_agent_tools = SubAgentTool::new(
        workspace_root.clone(),
        effective_system_prompt.clone(),
        provider_registry_snapshot(&state)?,
        sub_agent_settings,
        mcp_settings,
        tool_settings.clone(),
        skill_settings,
        DatabaseTool::new(state.store.clone()),
        state.max_tool_rounds,
        None,
        TurnCancel::empty(),
    )
    .descriptors();
    let team_tools = TeamTool::descriptors_static();
    let database_tools = DatabaseTool::descriptors_static();
    let mut sub_agent_tool_names = tool_name_set(&sub_agent_tools);
    sub_agent_tool_names.extend(tool_name_set(&team_tools));
    tools.extend(sub_agent_tools);
    tools.extend(team_tools);
    tools.extend(database_tools);
    let tools = tool_settings.apply_to_descriptors(tools);
    let system = system_prompt_with_todo(&effective_system_prompt, &conversation.todo_list);
    let system_prompt =
        system_prompt_for_mode_with_plan_prompt(&system, mode, tool_settings.plan_mode_prompt());
    let workspace_rules_weight =
        workspace_rules_weight(&workspace_root).map_err(error_to_string)?;
    let breakdown_weights = context_breakdown_weights(
        &system_prompt,
        workspace_rules_weight,
        &conversation.history,
        &tools,
        &mcp_tool_names,
        &sub_agent_tool_names,
    );
    estimate_model_context(
        provider,
        conversation.model.clone(),
        conversation.history.clone(),
        system_prompt,
        tools,
        Some(conversation.id.clone()),
        cache_stable_message_count,
        breakdown_weights,
        !has_pending_user_input,
    )
    .await
}

#[tauri::command]
pub(super) async fn estimate_sub_agent_context(
    state: State<'_, DesktopState>,
    input: SubAgentContextEstimateInput,
) -> std::result::Result<ContextEstimateOutput, String> {
    let mode = input.mode.map(AgentMode::from).unwrap_or_default();
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let effective_system_prompt =
        system_prompt_for_workspace(&workspace_root, &state.system_prompt)
            .map_err(error_to_string)?;
    let settings = state
        .store
        .load_sub_agent_settings()
        .map_err(error_to_string)?
        .normalized();
    let configured_agent = settings
        .agents
        .iter()
        .find(|agent| agent.id == input.agent_id);
    let team_agent = configured_agent
        .is_none()
        .then(|| team_agent_estimate_identity(&input.agent_id, input.agent_name.as_deref()))
        .flatten();
    if configured_agent.is_none() && team_agent.is_none() {
        return Err("sub-agent not found".to_string());
    }
    let provider = provider_from_registry(&state, &input.model.provider)?;

    let tool_settings = state.store.load_tool_settings().map_err(error_to_string)?;
    let skill_settings = state.store.load_skill_settings().map_err(error_to_string)?;
    let mut tools = tool_descriptors_for_workspace(&workspace_root, mode, &skill_settings);
    let mcp_settings = state.store.load_mcp_settings().map_err(error_to_string)?;
    let mcp = McpToolRegistry::new(mcp_settings);
    let mcp_tools = mcp.refresh_catalog(&input.history).await;
    let mcp_tool_names = tool_name_set(&mcp_tools);
    tools.extend(mcp_tools);
    if team_agent.is_some() {
        tools.retain(|tool| tool.name != "ToDoList" && tool.name != "Question");
        tools.extend(TeamTool::agent_descriptors_static());
    }
    let tools = tool_settings.apply_to_descriptors(tools);
    let agent_system_prompt = if let Some(agent) = configured_agent {
        subagent_system_prompt(&effective_system_prompt, agent)
    } else if let Some((team_name, agent_name)) = team_agent.as_ref() {
        team_agent_system_prompt_for_estimate(
            &effective_system_prompt,
            team_name,
            &input.agent_id,
            agent_name,
            &input.model,
        )
    } else {
        return Err("sub-agent not found".to_string());
    };
    let system = system_prompt_with_todo(&agent_system_prompt, &TodoListState::default());
    let system_prompt =
        system_prompt_for_mode_with_plan_prompt(&system, mode, tool_settings.plan_mode_prompt());
    let workspace_rules_weight =
        workspace_rules_weight(&workspace_root).map_err(error_to_string)?;
    let breakdown_weights = context_breakdown_weights(
        &system_prompt,
        workspace_rules_weight,
        &input.history,
        &tools,
        &mcp_tool_names,
        &HashSet::new(),
    );
    estimate_model_context(
        provider,
        input.model.clone(),
        input.history.clone(),
        system_prompt,
        tools,
        Some(format!(
            "subagent:{}:{}",
            workspace_root.display(),
            input.agent_id
        )),
        input.history.len(),
        breakdown_weights,
        true,
    )
    .await
}

pub(super) fn team_agent_estimate_identity(
    agent_id: &str,
    agent_name: Option<&str>,
) -> Option<(String, String)> {
    let (raw_agent, raw_team) = agent_id.rsplit_once('@')?;
    let team_name = raw_team.trim();
    if team_name.is_empty() {
        return None;
    }
    let name = agent_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let fallback = raw_agent.trim();
            (!fallback.is_empty()).then_some(fallback)
        })?;
    Some((team_name.to_string(), name.to_string()))
}

pub(super) fn team_agent_system_prompt_for_estimate(
    base: &str,
    team_name: &str,
    agent_id: &str,
    agent_name: &str,
    model: &ModelRef,
) -> String {
    let config_agent = SubAgentConfig {
        id: agent_id.to_string(),
        name: agent_name.to_string(),
        description: String::new(),
        prompt: String::new(),
        model: model.clone(),
        enabled: true,
    };
    let base = subagent_system_prompt(base, &config_agent);
    format!(
        "{base}\n\n<agent_team_profile team=\"{}\" name=\"{}\">\nYou are part of an autonomous agent team.\nYour work is coordinated through the task system and teammate messaging, use SendMessage tool to talk with your team.\nIf your owned work is blocked by incomplete tasks, end your turn and sleep; you will be woken automatically when your owned tasks unlock or when a teammate sends you a direct message.\n</agent_team_profile>",
        html_escape(team_name),
        html_escape(agent_name)
    )
}

pub(super) async fn estimate_model_context(
    provider: Arc<dyn Provider>,
    model: ModelRef,
    history: Vec<ChatMessage>,
    system_prompt: String,
    tools: Vec<ToolDescriptor>,
    cache_key: Option<String>,
    cache_stable_message_count: usize,
    breakdown_weights: Vec<ContextBreakdownWeight>,
    prefer_latest_stream_usage: bool,
) -> std::result::Result<ContextEstimateOutput, String> {
    let caps = provider
        .capabilities(&model)
        .ok_or_else(|| format!("model `{}` is not supported", model.name))?;
    let latest_stream_usage = prefer_latest_stream_usage
        .then(|| latest_stream_context_usage(&history, &model.provider, &model.name))
        .flatten();
    let (usage, exact, error) = match latest_stream_usage {
        Some(usage) => (usage, true, None),
        None => {
            let mut request = ProviderRequest::new(model.clone(), history)
                .with_system(system_prompt)
                .with_tools(tools)
                .with_cache_stable_message_count(cache_stable_message_count);
            if let Some(cache_key) = cache_key {
                request = request.with_cache_key(cache_key);
            }

            match provider.estimate_tokens(request).await {
                Ok(estimate) => (
                    ContextTokenUsage::from_input_tokens(estimate.input_tokens),
                    estimate.exact,
                    None,
                ),
                Err(err) => {
                    let local_estimate = local_context_token_estimate(&breakdown_weights);
                    (
                        ContextTokenUsage::from_input_tokens(local_estimate),
                        false,
                        Some(err.to_string()),
                    )
                }
            }
        }
    };
    let used_tokens = usage.total();

    Ok(ContextEstimateOutput {
        used_tokens,
        context_window: caps.context_window,
        preferred_window: caps.preferred_window,
        max_output_tokens: caps.max_output_tokens,
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        reasoning_tokens: usage.reasoning_tokens,
        cache_read_tokens: usage.cache_read_tokens,
        cache_creation_tokens: usage.cache_creation_tokens,
        exact,
        error,
        breakdown: context_usage_breakdown(usage)
            .unwrap_or_else(|| scale_context_breakdown(used_tokens, breakdown_weights)),
    })
}

impl ContextTokenUsage {
    fn from_input_tokens(input_tokens: u32) -> Self {
        Self {
            input_tokens,
            total_tokens: input_tokens,
            ..Self::default()
        }
    }

    fn from_stream_usage(usage: &Value) -> Self {
        let input_tokens = usage_u32(usage, "input_tokens").unwrap_or(0);
        let output_tokens = usage_u32(usage, "output_tokens").unwrap_or(0);
        let reasoning_tokens = usage_u32(usage, "reasoning_tokens").unwrap_or(0);
        let cache_read_tokens = usage_u32(usage, "cache_read_tokens").unwrap_or(0);
        let cache_creation_tokens = usage_u32(usage, "cache_creation_tokens").unwrap_or(0);
        let total_tokens = usage_u32(usage, "total_tokens").unwrap_or(0);
        Self {
            input_tokens,
            output_tokens,
            reasoning_tokens,
            cache_read_tokens,
            cache_creation_tokens,
            total_tokens,
        }
    }

    fn total(self) -> u32 {
        if self.total_tokens > 0 {
            self.total_tokens
        } else {
            self.input_tokens
                .saturating_add(self.output_tokens)
                .saturating_add(self.reasoning_tokens)
                .saturating_add(self.cache_read_tokens)
                .saturating_add(self.cache_creation_tokens)
        }
    }
}

pub(super) fn latest_stream_context_usage(
    history: &[ChatMessage],
    provider: &str,
    model: &str,
) -> Option<ContextTokenUsage> {
    for message in history.iter().rev() {
        if !matches!(message.role, Role::Assistant) {
            continue;
        }

        for part in message.parts.iter().rev() {
            if let Some(usage) = token_usage_meta(part) {
                if usage.get("source").and_then(Value::as_str) != Some("stream") {
                    continue;
                }
                if usage.get("provider").and_then(Value::as_str) != Some(provider) {
                    continue;
                }
                if usage.get("model").and_then(Value::as_str) != Some(model) {
                    continue;
                }
                let usage = ContextTokenUsage::from_stream_usage(usage);
                if usage.total() > 0 {
                    return Some(usage);
                }
            }
        }
    }

    None
}

pub(super) fn context_usage_breakdown(
    usage: ContextTokenUsage,
) -> Option<Vec<ContextBreakdownItem>> {
    let mut items = Vec::new();
    push_context_usage_breakdown(&mut items, "input", "Input", usage.input_tokens);
    push_context_usage_breakdown(&mut items, "output", "Output", usage.output_tokens);
    push_context_usage_breakdown(&mut items, "reasoning", "Reasoning", usage.reasoning_tokens);
    push_context_usage_breakdown(&mut items, "cache", "Cache read", usage.cache_read_tokens);
    push_context_usage_breakdown(
        &mut items,
        "cache_write",
        "Cache write",
        usage.cache_creation_tokens,
    );
    (!items.is_empty()).then_some(items)
}

pub(super) fn push_context_usage_breakdown(
    items: &mut Vec<ContextBreakdownItem>,
    key: &'static str,
    label: &'static str,
    tokens: u32,
) {
    if tokens > 0 {
        items.push(ContextBreakdownItem {
            key: key.to_string(),
            label: label.to_string(),
            tokens,
        });
    }
}

pub(super) fn token_usage_meta(part: &Part) -> Option<&Value> {
    part_meta(part)?.get("token_usage")
}

pub(super) fn usage_u32(usage: &Value, key: &str) -> Option<u32> {
    usage
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

pub(super) fn part_meta(part: &Part) -> Option<&Value> {
    match part {
        Part::Text { meta, .. }
        | Part::Image { meta, .. }
        | Part::Thinking { meta, .. }
        | Part::ToolCall { meta, .. }
        | Part::ToolResult { meta, .. } => meta.as_ref(),
    }
}

pub(super) fn tool_name_set(tools: &[ToolDescriptor]) -> HashSet<String> {
    tools.iter().map(|tool| tool.name.clone()).collect()
}

pub(super) fn workspace_rules_weight(workspace_root: &Path) -> Result<u64> {
    let mut weight = 0;

    if let Some(instructions) =
        read_workspace_prompt_file(workspace_root, WORKSPACE_INSTRUCTIONS_FILE)?
    {
        weight += context_text_weight(&format!(
            "# Workspace instructions\n\nThe following instructions come from the current workspace and should be treated as the project source of truth.\n\n{instructions}"
        ));
    }

    if let Some(design) = read_workspace_prompt_file(workspace_root, WORKSPACE_DESIGN_FILE)? {
        weight += context_text_weight(&format!(
            "# Workspace design context\n\nThe following design guidance comes from the current workspace and should guide product, UX, visual, and frontend decisions.\n\n{design}"
        ));
    }

    Ok(weight)
}

pub(super) fn context_breakdown_weights(
    system_prompt: &str,
    workspace_rules_weight: u64,
    history: &[ChatMessage],
    tools: &[ToolDescriptor],
    mcp_tool_names: &HashSet<String>,
    sub_agent_tool_names: &HashSet<String>,
) -> Vec<ContextBreakdownWeight> {
    let mut weights = Vec::new();
    let system_weight = context_text_weight(system_prompt).saturating_sub(workspace_rules_weight);

    push_context_weight(&mut weights, "system", "System prompt", system_weight);
    push_context_weight(&mut weights, "rules", "Rules", workspace_rules_weight);

    let mut base_tools_weight = 0;
    let mut skills_weight = 0;
    let mut mcp_weight = 0;
    let mut sub_agents_weight = 0;

    for tool in tools {
        let weight = tool_descriptor_weight(tool);
        if mcp_tool_names.contains(&tool.name) {
            mcp_weight += weight;
        } else if sub_agent_tool_names.contains(&tool.name) {
            sub_agents_weight += weight;
        } else if tool.name == "skill" {
            skills_weight += weight;
        } else {
            base_tools_weight += weight;
        }
    }

    push_context_weight(&mut weights, "tools", "Tools", base_tools_weight);
    push_context_weight(&mut weights, "skills", "Skills", skills_weight);
    push_context_weight(&mut weights, "mcp", "MCP", mcp_weight);
    push_context_weight(&mut weights, "subagents", "Subagents", sub_agents_weight);
    push_context_weight(
        &mut weights,
        "conversation",
        "Conversation",
        history_weight(history),
    );

    weights
}

pub(super) fn push_context_weight(
    weights: &mut Vec<ContextBreakdownWeight>,
    key: &'static str,
    label: &'static str,
    weight: u64,
) {
    if weight > 0 {
        weights.push(ContextBreakdownWeight { key, label, weight });
    }
}

pub(super) fn tool_descriptor_weight(tool: &ToolDescriptor) -> u64 {
    let schema = serde_json::to_string(&tool.input_schema).unwrap_or_default();
    96 + context_text_weight(&tool.name)
        + context_text_weight(&tool.description)
        + context_text_weight(&schema)
}

pub(super) fn history_weight(history: &[ChatMessage]) -> u64 {
    history.iter().map(message_weight).sum()
}

pub(super) fn message_weight(message: &ChatMessage) -> u64 {
    48 + message.parts.iter().map(part_weight).sum::<u64>()
}

pub(super) fn part_weight(part: &Part) -> u64 {
    match part {
        Part::Text { text, .. } | Part::Thinking { text, .. } => 24 + context_text_weight(text),
        Part::Image {
            media_type, data, ..
        } => 96 + context_text_weight(media_type) + image_weight(data),
        Part::ToolCall {
            id, name, input, ..
        } => {
            let input = serde_json::to_string(input).unwrap_or_default();
            80 + context_text_weight(id) + context_text_weight(name) + context_text_weight(&input)
        }
        Part::ToolResult {
            tool_call_id,
            content,
            images,
            ..
        } => {
            80 + context_text_weight(tool_call_id)
                + context_text_weight(content)
                + images
                    .iter()
                    .map(|image| {
                        80 + context_text_weight(&image.media_type)
                            + image
                                .path
                                .as_deref()
                                .map(context_text_weight)
                                .unwrap_or_default()
                            + image_weight(&image.data)
                    })
                    .sum::<u64>()
        }
    }
}

pub(super) fn context_text_weight(value: &str) -> u64 {
    value.chars().count() as u64
}

pub(super) fn image_weight(data: &str) -> u64 {
    1_200 + ((data.len() as u64) / 2_048).min(3_200)
}

pub(super) fn local_context_token_estimate(weights: &[ContextBreakdownWeight]) -> u32 {
    let total_weight = weights
        .iter()
        .fold(0_u64, |sum, item| sum.saturating_add(item.weight));
    let estimate = total_weight.saturating_add(3) / 4;
    estimate.max(1).min(u32::MAX as u64) as u32
}

pub(super) fn scale_context_breakdown(
    used_tokens: u32,
    weights: Vec<ContextBreakdownWeight>,
) -> Vec<ContextBreakdownItem> {
    let weights: Vec<_> = weights.into_iter().filter(|item| item.weight > 0).collect();
    if used_tokens == 0 || weights.is_empty() {
        return Vec::new();
    }

    let total_weight: u64 = weights.iter().map(|item| item.weight).sum();
    if total_weight == 0 {
        return Vec::new();
    }

    let mut scaled = weights
        .into_iter()
        .map(|item| {
            let raw = (item.weight as f64 / total_weight as f64) * used_tokens as f64;
            let tokens = raw.floor() as u32;
            let remainder = raw - tokens as f64;
            (item, tokens, remainder)
        })
        .collect::<Vec<_>>();

    let assigned = scaled
        .iter()
        .fold(0_u32, |sum, (_, tokens, _)| sum.saturating_add(*tokens));
    let mut remaining = used_tokens.saturating_sub(assigned);
    let mut order = (0..scaled.len()).collect::<Vec<_>>();
    order.sort_by(|a, b| {
        scaled[*b]
            .2
            .partial_cmp(&scaled[*a].2)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for index in order {
        if remaining == 0 {
            break;
        }
        scaled[index].1 = scaled[index].1.saturating_add(1);
        remaining -= 1;
    }

    scaled
        .into_iter()
        .filter(|(_, tokens, _)| *tokens > 0)
        .map(|(item, tokens, _)| ContextBreakdownItem {
            key: item.key.to_string(),
            label: item.label.to_string(),
            tokens,
        })
        .collect()
}
