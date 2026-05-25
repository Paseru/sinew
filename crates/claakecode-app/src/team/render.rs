use super::*;

pub(super) fn team_agent_system_prompt(base: &str, team_name: &str, agent: &TeamAgent) -> String {
    let config_agent = SubAgentConfig {
        id: agent.id.clone(),
        name: agent.name.clone(),
        description: agent.description.clone(),
        prompt: agent.prompt.clone(),
        model: agent.model.clone(),
        enabled: true,
    };
    let base = subagent_system_prompt(base, &config_agent);
    format!(
        "{base}\n\n<agent_team_profile team=\"{}\" name=\"{}\">\nYou are part of an autonomous agent team.\nYour work is coordinated through the task system and teammate messaging, use SendMessage tool to talk with your team.\nYou may sleep only when your owned task is actually status=blocked in the task board with real blockedBy task IDs. If a task is pending or in_progress, keep working; if it is genuinely blocked, update the task to status=blocked with blockedBy before ending your turn. You will be woken automatically when your owned tasks unlock or when a teammate sends you a direct message.\n</agent_team_profile>",
        escape_attr(team_name),
        escape_attr(&agent.name)
    )
}

pub(super) fn prepare_team_agent_names(
    names: Option<Vec<String>>,
) -> std::result::Result<Vec<String>, String> {
    let Some(names) = names else {
        return Err("agent_names is required when starting a new team".to_string());
    };
    let mut out: Vec<String> = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, name) in names.into_iter().enumerate() {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(format!("agent_names[{index}] cannot be empty"));
        }
        let key = agent_key(&name);
        if !seen.insert(key) {
            return Err(format!("duplicate teammate name `{name}`"));
        }
        out.push(name);
    }
    if out.len() < 2 {
        return Err("agent_names must include at least 2 teammates".to_string());
    }
    if out.len() > 8 {
        return Err("agent_names can include at most 8 teammates".to_string());
    }
    Ok(out)
}

pub(super) fn team_kickoff_message(
    objective: &str,
    agent_name: &str,
    agent_prompt: Option<&str>,
) -> String {
    let mut sections = vec![format!("Objective:\n{}", objective.trim())];
    if let Some(prompt) = agent_prompt
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        sections.push(format!(
            "Message from the main agent for @{}:\n{}",
            agent_name.trim(),
            prompt
        ));
    }
    sections.join("\n\n")
}

pub(super) fn team_restart_message(team_name: &str, agent_name: &str) -> String {
    format!(
        "You are being relaunched in Agent Swarm `{}` as @{}.\n\nReview the current team state in your system context, continue your owned work if it is still relevant, and coordinate with peers through TaskList and SendMessage.",
        team_name.trim(),
        agent_name.trim()
    )
}

pub(super) fn queued_messages_prompt(messages: &[TeamQueuedMessage]) -> String {
    let mut lines = vec!["<queued_peer_messages>".to_string()];
    for message in messages {
        let to_attr = message
            .target
            .as_deref()
            .map(|target| format!(" to=\"{}\"", escape_attr(target)))
            .unwrap_or_default();
        lines.push(format!(
            "<teammate-message teammate_id=\"{}\"{}>\n{}\n</teammate-message>",
            escape_attr(&message.from),
            to_attr,
            escape_text(message.message.trim())
        ));
    }
    lines.push("</queued_peer_messages>".to_string());
    lines.join("\n")
}

pub(super) fn render_agent_team_system_reminder(session: &TeamSession, agent_name: &str) -> String {
    let mut agents = session.agents.values().collect::<Vec<_>>();
    agents.sort_by(|left, right| left.name.cmp(&right.name));
    let mut tasks = session.tasks.iter().collect::<Vec<_>>();
    tasks.sort_by_key(|task| task.id);
    let mut lines = vec![
        "<agent_team_state>".to_string(),
        format!("team: {} | you: @{}", session.name, agent_name),
    ];
    if agents.is_empty() {
        lines.push("teammates: none".to_string());
    } else {
        lines.push("teammates:".to_string());
        for agent in agents {
            let you = if agent_key(&agent.name) == agent_key(agent_name) {
                " you"
            } else {
                ""
            };
            lines.push(format!(
                "- @{} [{}]{}",
                agent.name,
                status_label(agent.status),
                you
            ));
        }
    }
    if tasks.is_empty() {
        lines.push("tasks: none".to_string());
    } else {
        lines.push("tasks:".to_string());
        for task in tasks {
            lines.push(render_task_line(task));
        }
    }
    if !session.recent_file_changes.is_empty() {
        lines.push("recent file changes (newest -> oldest):".to_string());
        let total = session.recent_file_changes.len();
        for (index, change) in session.recent_file_changes.iter().rev().enumerate() {
            let marker = if index == 0 {
                "newest -> "
            } else if index + 1 == total {
                "oldest -> "
            } else {
                "          "
            };
            lines.push(format!("{marker}{}", render_recent_file_change(change)));
        }
    }
    lines.push("</agent_team_state>".to_string());
    lines.join("\n")
}

pub(super) fn render_main_agent_team_system_reminder(session: &TeamSession) -> String {
    let mut agents = session.agents.values().collect::<Vec<_>>();
    agents.sort_by(|left, right| left.name.cmp(&right.name));
    let any_running = agents
        .iter()
        .any(|agent| agent.status == TeamAgentStatus::Running);
    let mut lines = vec![
        "<agent_swarm_state>".to_string(),
        format!("team: {}", session.name),
    ];
    if agents.is_empty() {
        lines.push("teammates: none".to_string());
    } else {
        lines.push("teammates:".to_string());
        for agent in &agents {
            let error = agent
                .last_error
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!(" error: {}", truncate_line(value, 180)))
                .unwrap_or_default();
            lines.push(format!(
                "- @{} [{}]{}",
                agent.name,
                status_label(agent.status),
                error
            ));
        }
    }
    let errors = agents
        .into_iter()
        .filter(|agent| agent.status == TeamAgentStatus::Error)
        .filter_map(|agent| {
            agent
                .last_error
                .as_deref()
                .map(|error| format!("- @{}: {}", agent.name, truncate_line(error, 220)))
        })
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        lines.push("errors:".to_string());
        lines.extend(errors);
        lines.push("main-agent guidance: handle only these failures. Relaunch the failed teammate with TeamRun agent=... when useful, or stop that teammate if it is looping. Do not take over normal team work.".to_string());
    } else if any_running {
        lines.push("main-agent guidance: the Agent Swarm runs asynchronously in the background. Do not poll with shell commands, file checks, or TeamStatus just to see whether it is done. End your turn after acknowledging launch/status and wait for a user or system wake.".to_string());
    } else {
        lines.push("main-agent guidance: the Agent Swarm has no running teammates right now. If the current turn was triggered by an agent_swarm_finished system reminder, tell the user the Agent Swarm finished and summarize the final teammate responses. Do not poll with shell commands, file checks, or TeamStatus just to check completion.".to_string());
    }
    lines.push("</agent_swarm_state>".to_string());
    lines.join("\n")
}

pub(super) fn render_team_snapshot(snapshot: &TeamSnapshot) -> String {
    let mut lines = vec![format!("team: {}", snapshot.name)];
    if let Some(description) = snapshot.description.as_deref() {
        lines.push(format!("description: {description}"));
    }
    if snapshot.agents.is_empty() {
        lines.push("teammates: none".to_string());
    } else {
        lines.push(format!("teammates: {}", snapshot.agents.len()));
        for agent in &snapshot.agents {
            let summary = agent
                .last_summary
                .as_deref()
                .and_then(first_line)
                .unwrap_or("no report yet");
            lines.push(format!(
                "- @{} [{}] {} — {}",
                agent.name,
                status_label(agent.status),
                agent.description,
                summary
            ));
        }
    }
    if snapshot.tasks.is_empty() {
        lines.push("tasks: none".to_string());
    } else {
        lines.push(format!("tasks: {}", snapshot.tasks.len()));
        for task in &snapshot.tasks {
            lines.push(render_task_snapshot_line(task));
        }
    }
    if snapshot.queued_messages > 0 {
        lines.push(format!("queued messages: {}", snapshot.queued_messages));
    }
    lines.join("\n")
}

pub(super) fn team_agent_final_responses_from_session(
    session: &TeamSession,
) -> Vec<TeamAgentFinalResponse> {
    let mut agents = session.agents.values().collect::<Vec<_>>();
    agents.sort_by(|left, right| left.name.cmp(&right.name));
    agents
        .into_iter()
        .map(|agent| TeamAgentFinalResponse {
            agent: agent.name.clone(),
            status: final_response_status_label(agent.status).to_string(),
            last_response: final_response_for_agent(agent),
            last_error: agent.last_error.clone(),
        })
        .collect()
}

pub(super) fn final_response_for_agent(agent: &TeamAgent) -> String {
    final_assistant_text(&agent.history)
        .or_else(|| agent.last_summary.clone())
        .unwrap_or_else(|| "No final response recorded.".to_string())
}

pub(super) fn render_team_agent_final_responses(responses: &[TeamAgentFinalResponse]) -> String {
    responses
        .iter()
        .map(|response| {
            let mut lines = vec![format!("- @{} [{}]", response.agent, response.status)];
            if let Some(error) = response
                .last_error
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                lines.push(format!("  error: {}", truncate_text(error, 500)));
            }
            lines.push(format!(
                "  lastResponse: {}",
                indent_multiline(&truncate_text(&response.last_response, 1200), "  ").trim_start()
            ));
            lines.join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn final_response_status_label(status: TeamAgentStatus) -> &'static str {
    match status {
        TeamAgentStatus::Idle => "finished",
        TeamAgentStatus::Running => "running",
        TeamAgentStatus::Stopped => "stopped",
        TeamAgentStatus::Error => "error",
    }
}

pub(super) fn render_task_snapshot_line(task: &TeamTaskSnapshot) -> String {
    let owner = task
        .owner
        .as_deref()
        .map(|owner| format!(" @{}", owner))
        .unwrap_or_default();
    let mut detail = Vec::new();
    if !task.blocked_by.is_empty() {
        detail.push(format!("blocked by {}", render_task_ids(&task.blocked_by)));
    }
    let detail = if detail.is_empty() {
        String::new()
    } else {
        format!(" ({})", detail.join("; "))
    };
    format!(
        "- #{} [{}]{} {}{}",
        task.id,
        task_status_label(task.status),
        owner,
        task.subject,
        detail
    )
}

pub(super) fn render_task_line(task: &TeamTask) -> String {
    render_task_snapshot_line(&TeamTaskSnapshot::from_task(task))
}

pub(super) fn render_recent_file_change(change: &TeamRecentFileChange) -> String {
    format!(
        "@{} {} {} {} (+{} -{})",
        change.agent,
        change.tool,
        file_change_kind_label(change.kind),
        change.relative_path,
        change.added,
        change.removed
    )
}

pub(super) fn render_agent_result(team_name: &str, agent: &TeamAgent, answer: &str) -> String {
    format!(
        "team: {team_name}\nagent: @{}\nstatus: {}\n\n{}",
        agent.name,
        status_label(agent.status),
        answer.trim()
    )
}

pub(super) fn final_assistant_text(history: &[ChatMessage]) -> Option<String> {
    history.iter().rev().find_map(|message| {
        if !matches!(message.role, Role::Assistant) {
            return None;
        }
        let text = message
            .parts
            .iter()
            .filter_map(|part| match part {
                Part::Text { text, .. } if !text.trim().is_empty() => Some(text.trim()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        (!text.trim().is_empty()).then_some(text)
    })
}

pub(super) fn file_changes_from_history(history: &[ChatMessage]) -> Vec<FileChange> {
    history
        .iter()
        .flat_map(|message| message.parts.iter())
        .filter_map(|part| match part {
            Part::ToolResult { meta, .. } => meta
                .as_ref()
                .and_then(|meta| meta.get("file_changes"))
                .and_then(|value| serde_json::from_value::<Vec<FileChange>>(value.clone()).ok()),
            _ => None,
        })
        .flatten()
        .collect()
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or_default()
}

pub(super) fn agent_notify_key(scope_id: &str, team_name: &str, agent_name: &str) -> String {
    format!(
        "{}\0{}\0{}",
        scope_id,
        agent_key(team_name),
        agent_key(agent_name)
    )
}

pub(super) fn team_notify_key_prefix(scope_id: &str, team_name: &str) -> String {
    format!("{}\0{}\0", scope_id, agent_key(team_name))
}

pub(super) fn workspace_write_lock_key(workspace_root: &Path) -> String {
    workspace_root.display().to_string()
}

pub(super) fn wake_notifier(notifier: &Notify) {
    notifier.notify_waiters();
    notifier.notify_one();
}

pub(super) fn agent_key(value: &str) -> String {
    let key = value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if key.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        key
    }
}

pub(super) fn status_label(status: TeamAgentStatus) -> &'static str {
    match status {
        TeamAgentStatus::Idle => "idle",
        TeamAgentStatus::Running => "running",
        TeamAgentStatus::Stopped => "stopped",
        TeamAgentStatus::Error => "error",
    }
}

pub(super) fn team_run_status_label(
    snapshot: Option<&TeamSnapshot>,
    is_error: bool,
    fallback: &str,
) -> String {
    if is_error {
        return "error".to_string();
    }
    let Some(snapshot) = snapshot else {
        return fallback.to_string();
    };
    if snapshot
        .agents
        .iter()
        .any(|agent| agent.status == TeamAgentStatus::Running)
    {
        return "running".to_string();
    }
    if snapshot
        .agents
        .iter()
        .any(|agent| agent.status == TeamAgentStatus::Error)
    {
        return "error".to_string();
    }
    if !snapshot.agents.is_empty()
        && snapshot
            .agents
            .iter()
            .all(|agent| agent.status == TeamAgentStatus::Stopped)
    {
        return "stopped".to_string();
    }
    fallback.to_string()
}

pub(super) fn task_status_label(status: TeamTaskStatus) -> &'static str {
    match status {
        TeamTaskStatus::Pending => "pending",
        TeamTaskStatus::InProgress => "in_progress",
        TeamTaskStatus::Blocked => "blocked",
        TeamTaskStatus::Completed => "completed",
    }
}

pub(super) fn file_change_kind_label(kind: FileChangeKind) -> &'static str {
    match kind {
        FileChangeKind::Added => "added",
        FileChangeKind::Modified => "modified",
        FileChangeKind::Deleted => "deleted",
    }
}

pub(super) fn normalized_owner(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn prepare_team_agent_prompts(
    agent_names: &[String],
    agent_prompts: Option<&HashMap<String, String>>,
) -> std::result::Result<HashMap<String, String>, String> {
    let mut prompts = HashMap::new();
    let Some(agent_prompts) = agent_prompts else {
        return Ok(prompts);
    };
    for (agent_name, prompt) in agent_prompts {
        let agent_name = agent_name.trim();
        let prompt = prompt.trim();
        if agent_name.is_empty() || prompt.is_empty() {
            return Err("agent_prompts keys and values cannot be empty".to_string());
        }
        let agent_key_value = agent_key(agent_name);
        let Some(canonical_name) = agent_names
            .iter()
            .find(|name| agent_key(name) == agent_key_value)
        else {
            return Err(format!(
                "agent_prompts references unknown teammate `{agent_name}`"
            ));
        };
        let canonical_key = agent_key(canonical_name);
        if prompts.insert(canonical_key, prompt.to_string()).is_some() {
            return Err(format!(
                "agent_prompts contains duplicate teammate `{agent_name}`"
            ));
        }
    }
    Ok(prompts)
}

pub(super) fn prepare_team_run_tasks(
    tasks: Option<&[TeamRunTaskInput]>,
    agent_names: &[String],
) -> std::result::Result<Vec<PreparedTeamRunTask>, String> {
    let Some(tasks) = tasks else {
        return Ok(Vec::new());
    };
    let task_count = tasks.len() as u64;
    let mut prepared = Vec::with_capacity(tasks.len());
    for (index, task) in tasks.iter().enumerate() {
        let task_id = index as u64 + 1;
        let subject = task.subject.trim();
        if subject.is_empty() {
            return Err(format!("tasks[{index}].subject is required"));
        }
        let description = task
            .description
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let owner = match normalized_owner(task.owner.as_deref()) {
            Some(owner) => {
                let owner_key = agent_key(&owner);
                let Some(canonical) = agent_names
                    .iter()
                    .find(|agent_name| agent_key(agent_name) == owner_key)
                else {
                    return Err(format!(
                        "tasks[{index}].owner `{owner}` does not match a teammate"
                    ));
                };
                Some(canonical.clone())
            }
            None => None,
        };
        let blocked_by = normalize_task_ids(merge_task_id_inputs(
            task.blocked_by.clone(),
            task.blocked_by_snake.clone(),
        ))?;
        if blocked_by.contains(&task_id) {
            return Err(format!("task #{task_id} cannot block itself"));
        }
        let unknown = blocked_by
            .iter()
            .filter(|id| **id > task_count)
            .copied()
            .collect::<Vec<_>>();
        if !unknown.is_empty() {
            return Err(format!(
                "unknown initial blocking task(s): {}",
                render_task_ids(&unknown)
            ));
        }
        prepared.push(PreparedTeamRunTask {
            subject: subject.to_string(),
            description,
            owner,
            blocked_by,
        });
    }
    Ok(prepared)
}

pub(super) fn normalize_optional_object_input(input: Value) -> Value {
    match input {
        Value::Null => json!({}),
        Value::String(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                json!({})
            } else {
                match serde_json::from_str::<Value>(trimmed) {
                    Ok(Value::Object(map)) => Value::Object(map),
                    _ => Value::String(raw),
                }
            }
        }
        other => other,
    }
}

pub(super) fn render_task_ids(ids: &[u64]) -> String {
    ids.iter()
        .map(|id| format!("#{id}"))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn truncate_line(value: &str, limit: usize) -> String {
    let line = first_line(value).unwrap_or(value).trim();
    if line.chars().count() > limit {
        let keep = limit.saturating_sub(3);
        let mut truncated = line.chars().take(keep).collect::<String>();
        truncated.push_str("...");
        truncated
    } else {
        line.to_string()
    }
}

pub(super) fn truncate_text(value: &str, limit: usize) -> String {
    let value = value.trim();
    if value.chars().count() > limit {
        let keep = limit.saturating_sub(3);
        let mut truncated = value.chars().take(keep).collect::<String>();
        truncated.push_str("...");
        truncated
    } else {
        value.to_string()
    }
}

pub(super) fn indent_multiline(value: &str, indent: &str) -> String {
    value
        .lines()
        .map(|line| format!("{indent}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn first_line(value: &str) -> Option<&str> {
    value.lines().map(str::trim).find(|line| !line.is_empty())
}

pub(super) fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(super) fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
