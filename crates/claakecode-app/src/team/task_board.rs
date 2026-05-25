use super::*;

impl TeamTool {
    pub(super) async fn run_task_create(
        &self,
        input: Value,
        _mode: AgentMode,
        _parent_event_tx: mpsc::UnboundedSender<AgentEvent>,
    ) -> ToolRunResult {
        if self.current_agent.is_none() {
            return ToolRunResult::err(
                "TaskCreate is only available to team teammates. Use TeamRun to start the team.",
                Vec::new(),
            );
        }
        if input.get("blocker").is_some() {
            return ToolRunResult::err("blocker was removed; use blockedBy instead", Vec::new());
        }
        let parsed: TaskCreateInput = match serde_json::from_value(input) {
            Ok(value) => value,
            Err(err) => {
                return ToolRunResult::err(format!("invalid TaskCreate input: {err}"), Vec::new())
            }
        };
        let subject = parsed.subject.trim();
        if subject.is_empty() {
            return ToolRunResult::err("subject is required", Vec::new());
        }
        let description = parsed
            .description
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let owner = normalized_owner(parsed.owner.as_deref());
        let blocked_by = match normalize_task_ids(merge_task_id_inputs(
            parsed.blocked_by,
            parsed.blocked_by_snake,
        )) {
            Ok(value) => value,
            Err(err) => return ToolRunResult::err(err, Vec::new()),
        };
        let team_name = match self.resolve_team_name(parsed.team_name.as_deref()).await {
            Ok(value) => value,
            Err(err) => return ToolRunResult::err(err, Vec::new()),
        };
        let actor = self.current_actor_name(&team_name);
        let task = {
            let mut runtime = self.runtime.write().await;
            let Some(scope) = runtime.scopes.get_mut(&self.scope_id) else {
                return ToolRunResult::err(
                    "no active team found; start one with TeamRun first",
                    Vec::new(),
                );
            };
            scope.active_team = Some(team_name.clone());
            let Some(session) = scope.teams.get_mut(&team_name) else {
                return ToolRunResult::err(format!("team `{team_name}` not found"), Vec::new());
            };
            if let Err(err) = validate_task_dependencies(session, None, &blocked_by) {
                return ToolRunResult::err(err, Vec::new());
            }
            let now = now_ms();
            let status = if blocked_by.is_empty() {
                TeamTaskStatus::Pending
            } else {
                TeamTaskStatus::Blocked
            };
            let task = TeamTask {
                id: session.next_task_id,
                subject: subject.to_string(),
                description,
                status,
                owner,
                blocked_by,
                created_by: actor,
                created_at_ms: now,
                updated_at_ms: now,
                completed_at_ms: None,
            };
            session.next_task_id += 1;
            session.updated_at_ms = now;
            session.tasks.push(task.clone());
            let done_ids = completed_task_ids(session);
            refresh_unblocked_tasks(session, &done_ids);
            if task.status == TeamTaskStatus::Pending
                && queue_task_wake_for_ready_task(session, task.id, &done_ids, now)
            {
                session.updated_at_ms = now;
            }
            task
        };

        self.notify_all_team_agents(&team_name).await;

        ToolRunResult::ok(
            format!("Task #{} created successfully: {}", task.id, task.subject),
            Vec::new(),
        )
    }

    pub(super) async fn run_task_list(
        &self,
        input: Value,
        mode: AgentMode,
        parent_event_tx: mpsc::UnboundedSender<AgentEvent>,
    ) -> ToolRunResult {
        if input.get("blocker").is_some() {
            return ToolRunResult::err("blocker was removed; use blockedBy instead", Vec::new());
        }
        let parsed: TaskListInput = match serde_json::from_value(input) {
            Ok(value) => value,
            Err(err) => {
                return ToolRunResult::err(format!("invalid TaskList input: {err}"), Vec::new())
            }
        };
        let Some(action) = parsed.action else {
            return ToolRunResult::err(
                "TaskList action is required: create, update, delete, or claim",
                Vec::new(),
            );
        };

        if self.current_agent.is_none() {
            return ToolRunResult::err(
                "TaskList mutations are only available to team teammates. Use TeamRun to start the team.",
                Vec::new(),
            );
        }

        match action {
            TaskListAction::List => self.run_task_list_snapshot(parsed).await,
            TaskListAction::Create => {
                let team_name = match self.resolve_team_name(parsed.team_name.as_deref()).await {
                    Ok(value) => value,
                    Err(err) => return ToolRunResult::err(err, Vec::new()),
                };
                let mut payload = serde_json::Map::new();
                payload.insert("team_name".into(), json!(team_name));
                if let Some(subject) = parsed.subject {
                    payload.insert("subject".into(), json!(subject));
                }
                if let Some(description) = parsed.description {
                    payload.insert("description".into(), json!(description));
                }
                if let Some(owner) = parsed.owner {
                    payload.insert("owner".into(), json!(owner));
                }
                if let Some(blocked_by) = parsed.blocked_by {
                    payload.insert(
                        "blockedBy".into(),
                        serde_json::to_value(blocked_by).unwrap_or(Value::Null),
                    );
                }
                if let Some(blocked_by) = parsed.blocked_by_snake {
                    payload.insert(
                        "blocked_by".into(),
                        serde_json::to_value(blocked_by).unwrap_or(Value::Null),
                    );
                }
                let mut result = self
                    .run_task_create(Value::Object(payload), mode, parent_event_tx)
                    .await;
                self.attach_team_snapshot_meta(&team_name, &mut result)
                    .await;
                result
            }
            TaskListAction::Update => {
                let mut payload = serde_json::Map::new();
                if let Some(team_name) = parsed.team_name {
                    payload.insert("team_name".into(), json!(team_name));
                }
                if let Some(task_id) = parsed.task_id {
                    payload.insert(
                        "taskId".into(),
                        serde_json::to_value(task_id).unwrap_or(Value::Null),
                    );
                }
                if let Some(status) = parsed.status {
                    payload.insert("status".into(), json!(status));
                }
                if let Some(owner) = parsed.owner {
                    payload.insert("owner".into(), json!(owner));
                }
                if let Some(clear_owner) = parsed.clear_owner {
                    payload.insert("clear_owner".into(), json!(clear_owner));
                }
                if let Some(subject) = parsed.subject {
                    payload.insert("subject".into(), json!(subject));
                }
                if let Some(description) = parsed.description {
                    payload.insert("description".into(), json!(description));
                }
                if let Some(blocked_by) = parsed.blocked_by {
                    payload.insert(
                        "blockedBy".into(),
                        serde_json::to_value(blocked_by).unwrap_or(Value::Null),
                    );
                }
                if let Some(blocked_by) = parsed.blocked_by_snake {
                    payload.insert(
                        "blocked_by".into(),
                        serde_json::to_value(blocked_by).unwrap_or(Value::Null),
                    );
                }
                if let Some(blocked_by) = parsed.add_blocked_by {
                    payload.insert(
                        "addBlockedBy".into(),
                        serde_json::to_value(blocked_by).unwrap_or(Value::Null),
                    );
                }
                if let Some(blocked_by) = parsed.add_blocked_by_snake {
                    payload.insert(
                        "add_blocked_by".into(),
                        serde_json::to_value(blocked_by).unwrap_or(Value::Null),
                    );
                }
                self.run_task_update(Value::Object(payload), mode, parent_event_tx)
                    .await
            }
            TaskListAction::Delete => self.run_task_delete(parsed).await,
            TaskListAction::Claim => self.run_task_claim(parsed).await,
        }
    }

    pub(super) async fn run_task_list_snapshot(&self, parsed: TaskListInput) -> ToolRunResult {
        let team_name = match self.resolve_team_name(parsed.team_name.as_deref()).await {
            Ok(value) => value,
            Err(err) => return ToolRunResult::err(err, Vec::new()),
        };
        let (content, snapshot) = {
            let mut runtime = self.runtime.write().await;
            let Some(scope) = runtime.scopes.get_mut(&self.scope_id) else {
                return ToolRunResult::err(
                    "no active team found; start one with TeamRun first",
                    Vec::new(),
                );
            };
            scope.active_team = Some(team_name.clone());
            let Some(session) = scope.teams.get_mut(&team_name) else {
                return ToolRunResult::err(format!("team `{team_name}` not found"), Vec::new());
            };
            let done_ids = completed_task_ids(session);
            refresh_unblocked_tasks(session, &done_ids);
            prune_stale_task_wakes(session, &done_ids);
            let snapshot = TeamSnapshot::from_session(session);
            (
                format!("Task board:\n{}", render_team_snapshot(&snapshot)),
                snapshot,
            )
        };
        ToolRunResult::ok_with_meta(content, Vec::new(), json!({ "team": snapshot }))
    }

    pub(super) async fn run_task_update(
        &self,
        input: Value,
        _mode: AgentMode,
        _parent_event_tx: mpsc::UnboundedSender<AgentEvent>,
    ) -> ToolRunResult {
        if self.current_agent.is_none() {
            return ToolRunResult::err(
                "TaskUpdate is only available to team teammates. Use TeamRun to start the team.",
                Vec::new(),
            );
        }
        if input.get("blocker").is_some() {
            return ToolRunResult::err("blocker was removed; use blockedBy instead", Vec::new());
        }
        let parsed: TaskUpdateInput = match serde_json::from_value(input) {
            Ok(value) => value,
            Err(err) => {
                return ToolRunResult::err(format!("invalid TaskUpdate input: {err}"), Vec::new())
            }
        };
        let task_id = match parsed.task_id.to_u64() {
            Ok(value) => value,
            Err(err) => return ToolRunResult::err(err, Vec::new()),
        };
        let owner = normalized_owner(parsed.owner.as_deref());
        let subject = match parsed.subject.as_deref().map(str::trim) {
            Some("") => return ToolRunResult::err("subject cannot be empty", Vec::new()),
            Some(value) => Some(value.to_string()),
            None => None,
        };
        let description = parsed.description.as_ref().map(|value| {
            let value = value.trim();
            if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        });
        let replace_blocked_by = match normalize_optional_task_ids(merge_task_id_inputs(
            parsed.blocked_by,
            parsed.blocked_by_snake,
        )) {
            Ok(value) => value,
            Err(err) => return ToolRunResult::err(err, Vec::new()),
        };
        let add_blocked_by = match normalize_optional_task_ids(merge_task_id_inputs(
            parsed.add_blocked_by,
            parsed.add_blocked_by_snake,
        )) {
            Ok(value) => value,
            Err(err) => return ToolRunResult::err(err, Vec::new()),
        };
        let blocked_by_was_requested = replace_blocked_by.is_some() || add_blocked_by.is_some();
        let team_name = match self.resolve_team_name(parsed.team_name.as_deref()).await {
            Ok(value) => value,
            Err(err) => return ToolRunResult::err(err, Vec::new()),
        };
        let actor = self.current_actor_name(&team_name);
        let (content, snapshot) = {
            let mut runtime = self.runtime.write().await;
            let Some(scope) = runtime.scopes.get_mut(&self.scope_id) else {
                return ToolRunResult::err(
                    "no active team found; start one with TeamRun first",
                    Vec::new(),
                );
            };
            scope.active_team = Some(team_name.clone());
            let Some(session) = scope.teams.get_mut(&team_name) else {
                return ToolRunResult::err(format!("team `{team_name}` not found"), Vec::new());
            };
            let done_ids = completed_task_ids(session);
            refresh_unblocked_tasks(session, &done_ids);
            let Some(task_index) = session.tasks.iter().position(|task| task.id == task_id) else {
                return ToolRunResult::err(format!("task #{task_id} not found"), Vec::new());
            };

            let mut next_blocked_by = replace_blocked_by
                .clone()
                .unwrap_or_else(|| session.tasks[task_index].blocked_by.clone());
            if let Some(additional) = add_blocked_by {
                next_blocked_by.extend(additional);
                next_blocked_by = normalize_task_id_values(next_blocked_by);
            }
            if let Err(err) = validate_task_dependencies(session, Some(task_id), &next_blocked_by) {
                return ToolRunResult::err(err, Vec::new());
            }
            let done_ids = completed_task_ids(session);
            if let Err(err) = validate_task_dependency_lock(
                task_id,
                &session.tasks[task_index].blocked_by,
                &next_blocked_by,
                parsed.status,
                &done_ids,
            ) {
                return ToolRunResult::err(err, Vec::new());
            }
            if parsed.status == Some(TeamTaskStatus::Blocked) && next_blocked_by.is_empty() {
                return ToolRunResult::err(
                    "blocked tasks require blockedBy task IDs".to_string(),
                    Vec::new(),
                );
            }

            let now = now_ms();
            let task = &mut session.tasks[task_index];
            let mut updated_fields = Vec::new();

            if let Some(subject) = subject {
                if task.subject != subject {
                    task.subject = subject;
                    updated_fields.push("subject");
                }
            }
            if let Some(description) = description {
                if task.description != description {
                    task.description = description;
                    updated_fields.push("description");
                }
            }
            if parsed.clear_owner.unwrap_or(false) && task.owner.is_some() {
                task.owner = None;
                updated_fields.push("owner");
            }
            if let Some(owner) = owner {
                if task.owner.as_deref() != Some(owner.as_str()) {
                    task.owner = Some(owner);
                    updated_fields.push("owner");
                }
            }
            if task.blocked_by != next_blocked_by {
                task.blocked_by = next_blocked_by;
                updated_fields.push("blockedBy");
            }
            if parsed.status.is_none() && blocked_by_was_requested {
                if !task.blocked_by.is_empty() && task.status != TeamTaskStatus::Completed {
                    if task.status != TeamTaskStatus::Blocked {
                        task.status = TeamTaskStatus::Blocked;
                        task.completed_at_ms = None;
                        updated_fields.push("status");
                    }
                } else if task.blocked_by.is_empty() && task.status == TeamTaskStatus::Blocked {
                    task.status = TeamTaskStatus::Pending;
                    task.completed_at_ms = None;
                    updated_fields.push("status");
                }
            }
            if let Some(status) = parsed.status {
                if task.status != status {
                    task.status = status;
                    updated_fields.push("status");
                }
                match status {
                    TeamTaskStatus::Completed => {
                        task.completed_at_ms = Some(now);
                    }
                    TeamTaskStatus::InProgress => {
                        task.completed_at_ms = None;
                        if task.owner.is_none() && self.current_agent.is_some() {
                            task.owner = Some(actor.clone());
                            updated_fields.push("owner");
                        }
                    }
                    TeamTaskStatus::Blocked => {
                        task.completed_at_ms = None;
                    }
                    TeamTaskStatus::Pending => {
                        task.completed_at_ms = None;
                    }
                }
            }

            task.updated_at_ms = now;
            let updated_task_id = task.id;
            session.updated_at_ms = now;
            refresh_unblocked_tasks(session, &done_ids);
            let task_snapshot = session
                .tasks
                .iter()
                .find(|task| task.id == updated_task_id)
                .cloned()
                .expect("updated task should still exist");
            if task_snapshot.status == TeamTaskStatus::Pending
                && queue_task_wake_for_ready_task(session, task_snapshot.id, &done_ids, now)
            {
                session.updated_at_ms = now;
            }
            let snapshot = TeamSnapshot::from_session(session);
            let updated = if updated_fields.is_empty() {
                "no fields changed".to_string()
            } else {
                format!("updated {}", updated_fields.join(", "))
            };
            (
                format!(
                    "Task #{} {}:\n{}",
                    task_snapshot.id,
                    updated,
                    render_task_line(&task_snapshot)
                ),
                snapshot,
            )
        };

        self.notify_all_team_agents(&team_name).await;

        ToolRunResult::ok_with_meta(content, Vec::new(), json!({ "team": snapshot }))
    }

    pub(super) async fn run_task_delete(&self, parsed: TaskListInput) -> ToolRunResult {
        let Some(task_id) = parsed.task_id else {
            return ToolRunResult::err("taskId is required for TaskList action=delete", Vec::new());
        };
        let task_id = match task_id.to_u64() {
            Ok(value) => value,
            Err(err) => return ToolRunResult::err(err, Vec::new()),
        };
        let team_name = match self.resolve_team_name(parsed.team_name.as_deref()).await {
            Ok(value) => value,
            Err(err) => return ToolRunResult::err(err, Vec::new()),
        };
        let (subject, snapshot) = {
            let mut runtime = self.runtime.write().await;
            let Some(scope) = runtime.scopes.get_mut(&self.scope_id) else {
                return ToolRunResult::err(
                    "no active team found; start one with TeamRun first",
                    Vec::new(),
                );
            };
            scope.active_team = Some(team_name.clone());
            let Some(session) = scope.teams.get_mut(&team_name) else {
                return ToolRunResult::err(format!("team `{team_name}` not found"), Vec::new());
            };
            let done_ids = completed_task_ids(session);
            refresh_unblocked_tasks(session, &done_ids);
            let Some(task_index) = session.tasks.iter().position(|task| task.id == task_id) else {
                return ToolRunResult::err(format!("task #{task_id} not found"), Vec::new());
            };
            let task = session.tasks.remove(task_index);
            for other in &mut session.tasks {
                other.blocked_by.retain(|id| *id != task_id);
            }
            session.updated_at_ms = now_ms();
            let done_ids = completed_task_ids(session);
            refresh_unblocked_tasks(session, &done_ids);
            (task.subject, TeamSnapshot::from_session(session))
        };
        self.notify_all_team_agents(&team_name).await;
        ToolRunResult::ok_with_meta(
            format!("Task #{task_id} deleted: {subject}"),
            Vec::new(),
            json!({ "team": snapshot }),
        )
    }

    pub(super) async fn run_task_claim(&self, parsed: TaskListInput) -> ToolRunResult {
        let team_name = match self.resolve_team_name(parsed.team_name.as_deref()).await {
            Ok(value) => value,
            Err(err) => return ToolRunResult::err(err, Vec::new()),
        };
        let owner = normalized_owner(parsed.owner.as_deref())
            .unwrap_or_else(|| self.current_actor_name(&team_name));
        let requested_id = match parsed.task_id.as_ref() {
            Some(task_id) => match task_id.to_u64() {
                Ok(value) => Some(value),
                Err(err) => return ToolRunResult::err(err, Vec::new()),
            },
            None => None,
        };
        let (line, snapshot) = {
            let mut runtime = self.runtime.write().await;
            let Some(scope) = runtime.scopes.get_mut(&self.scope_id) else {
                return ToolRunResult::err(
                    "no active team found; start one with TeamRun first",
                    Vec::new(),
                );
            };
            scope.active_team = Some(team_name.clone());
            let Some(session) = scope.teams.get_mut(&team_name) else {
                return ToolRunResult::err(format!("team `{team_name}` not found"), Vec::new());
            };
            let done_ids = completed_task_ids(session);
            refresh_unblocked_tasks(session, &done_ids);
            prune_stale_task_wakes(session, &done_ids);
            let owner_key = agent_key(&owner);
            let task_index = if let Some(task_id) = requested_id {
                let Some(task_index) = session.tasks.iter().position(|task| task.id == task_id)
                else {
                    return ToolRunResult::err(format!("task #{task_id} not found"), Vec::new());
                };
                task_index
            } else {
                let Some(task_index) = session.tasks.iter().position(|task| {
                    task.status == TeamTaskStatus::Pending
                        && task_dependencies_satisfied(task, &done_ids)
                        && task
                            .owner
                            .as_deref()
                            .map(agent_key)
                            .map(|current_owner| current_owner == owner_key)
                            .unwrap_or(true)
                }) else {
                    return ToolRunResult::ok_with_meta(
                        "No unblocked pending task available to claim",
                        Vec::new(),
                        json!({ "team": TeamSnapshot::from_session(session) }),
                    );
                };
                task_index
            };
            let task = &mut session.tasks[task_index];
            if task.status == TeamTaskStatus::Completed {
                return ToolRunResult::err(
                    format!("task #{} is already completed", task.id),
                    Vec::new(),
                );
            }
            let now = now_ms();
            task.owner = Some(owner);
            task.updated_at_ms = now;
            session.updated_at_ms = now;
            let claimed_task_id = task.id;
            let line = render_task_line(task);
            remove_task_wakes_for_task(session, claimed_task_id);
            let snapshot = TeamSnapshot::from_session(session);
            (line, snapshot)
        };
        self.notify_all_team_agents(&team_name).await;
        ToolRunResult::ok_with_meta(
            format!("Task claimed:\n{line}"),
            Vec::new(),
            json!({ "team": snapshot }),
        )
    }
}

pub(super) fn queued_message_wakes_agent(message: &TeamQueuedMessage) -> bool {
    message
        .target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .map(|target| target != "*")
        .unwrap_or(true)
}

pub(super) fn merge_task_id_inputs(
    first: Option<Vec<TaskIdInput>>,
    second: Option<Vec<TaskIdInput>>,
) -> Option<Vec<TaskIdInput>> {
    match (first, second) {
        (Some(mut first), Some(second)) => {
            first.extend(second);
            Some(first)
        }
        (Some(first), None) => Some(first),
        (None, Some(second)) => Some(second),
        (None, None) => None,
    }
}

pub(super) fn normalize_task_ids(
    ids: Option<Vec<TaskIdInput>>,
) -> std::result::Result<Vec<u64>, String> {
    normalize_optional_task_ids(ids).map(Option::unwrap_or_default)
}

pub(super) fn normalize_optional_task_ids(
    ids: Option<Vec<TaskIdInput>>,
) -> std::result::Result<Option<Vec<u64>>, String> {
    let Some(ids) = ids else {
        return Ok(None);
    };
    let mut values = Vec::new();
    for id in ids {
        values.push(id.to_u64()?);
    }
    Ok(Some(normalize_task_id_values(values)))
}

pub(super) fn normalize_task_id_values(ids: Vec<u64>) -> Vec<u64> {
    ids.into_iter()
        .filter(|id| *id > 0)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn validate_task_dependencies(
    session: &TeamSession,
    task_id: Option<u64>,
    blocked_by: &[u64],
) -> std::result::Result<(), String> {
    if let Some(task_id) = task_id {
        if blocked_by.contains(&task_id) {
            return Err(format!("task #{task_id} cannot block itself"));
        }
    }
    let unknown = blocked_by
        .iter()
        .filter(|id| !session.tasks.iter().any(|task| task.id == **id))
        .copied()
        .collect::<Vec<_>>();
    if unknown.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "unknown blocking task(s): {}",
            render_task_ids(&unknown)
        ))
    }
}

pub(super) fn validate_task_dependency_lock(
    task_id: u64,
    current_blocked_by: &[u64],
    next_blocked_by: &[u64],
    requested_status: Option<TeamTaskStatus>,
    done_ids: &BTreeSet<u64>,
) -> std::result::Result<(), String> {
    let removed_unresolved = current_blocked_by
        .iter()
        .filter(|id| !done_ids.contains(id) && !next_blocked_by.contains(id))
        .copied()
        .collect::<Vec<_>>();
    if !removed_unresolved.is_empty() {
        return Err(format!(
            "task #{task_id} is still blocked by {}; complete blocking tasks before clearing blockedBy",
            render_task_ids(&removed_unresolved)
        ));
    }

    let unresolved = next_blocked_by
        .iter()
        .filter(|id| !done_ids.contains(id))
        .copied()
        .collect::<Vec<_>>();
    if unresolved.is_empty() {
        return Ok(());
    }
    match requested_status {
        Some(status @ TeamTaskStatus::Pending)
        | Some(status @ TeamTaskStatus::InProgress)
        | Some(status @ TeamTaskStatus::Completed) => Err(format!(
            "task #{task_id} is blocked by {}; complete blocking tasks before setting status={}",
            render_task_ids(&unresolved),
            task_status_label(status)
        )),
        Some(TeamTaskStatus::Blocked) | None => Ok(()),
    }
}

pub(super) fn completed_task_ids(session: &TeamSession) -> BTreeSet<u64> {
    session
        .tasks
        .iter()
        .filter(|task| task.status == TeamTaskStatus::Completed)
        .map(|task| task.id)
        .collect()
}

pub(super) fn refresh_unblocked_tasks(
    session: &mut TeamSession,
    done_ids: &BTreeSet<u64>,
) -> Vec<u64> {
    let now = now_ms();
    let mut changed = false;
    let mut unblocked_task_ids = Vec::new();
    for task in &mut session.tasks {
        let dependencies_are_ready =
            !task.blocked_by.is_empty() && task_dependencies_satisfied(task, done_ids);
        let dependencies_are_unresolved = !task.blocked_by.is_empty() && !dependencies_are_ready;
        let invalid_empty_dependency = task.blocked_by.is_empty();
        if dependencies_are_ready {
            let was_blocked = task.status == TeamTaskStatus::Blocked;
            task.blocked_by.clear();
            if was_blocked {
                task.status = TeamTaskStatus::Pending;
                task.completed_at_ms = None;
                unblocked_task_ids.push(task.id);
            }
            task.updated_at_ms = now;
            changed = true;
        } else if task.status == TeamTaskStatus::Blocked && invalid_empty_dependency {
            task.status = TeamTaskStatus::Pending;
            task.completed_at_ms = None;
            task.updated_at_ms = now;
            changed = true;
            unblocked_task_ids.push(task.id);
        } else if dependencies_are_unresolved
            && matches!(
                task.status,
                TeamTaskStatus::Pending | TeamTaskStatus::InProgress
            )
        {
            task.status = TeamTaskStatus::Blocked;
            task.completed_at_ms = None;
            task.updated_at_ms = now;
            changed = true;
        }
    }
    if changed {
        session.updated_at_ms = now;
    }
    for task_id in &unblocked_task_ids {
        queue_task_wake_for_ready_task(session, *task_id, done_ids, now);
    }
    unblocked_task_ids
}

pub(super) fn queue_task_wake_for_ready_task(
    session: &mut TeamSession,
    task_id: u64,
    done_ids: &BTreeSet<u64>,
    now: u64,
) -> bool {
    let Some(task) = session.tasks.iter().find(|task| task.id == task_id) else {
        return false;
    };
    if task.status != TeamTaskStatus::Pending || !task_dependencies_satisfied(task, done_ids) {
        return false;
    }
    let Some(owner) = task
        .owner
        .as_deref()
        .map(str::trim)
        .filter(|owner| !owner.is_empty())
    else {
        return false;
    };
    let owner_key = agent_key(owner);
    let Some(agent) = session.agents.get(&owner_key) else {
        return false;
    };
    if agent.status != TeamAgentStatus::Idle {
        return false;
    }
    if session
        .pending_task_wakes
        .iter()
        .any(|wake| wake.task_id == task_id && agent_key(&wake.owner) == owner_key)
    {
        return false;
    }
    session.pending_task_wakes.push(TeamTaskWake {
        task_id,
        owner: agent.name.clone(),
        created_at_ms: now,
    });
    true
}

pub(super) fn prune_stale_task_wakes(session: &mut TeamSession, done_ids: &BTreeSet<u64>) {
    let wakes = std::mem::take(&mut session.pending_task_wakes);
    session.pending_task_wakes = wakes
        .into_iter()
        .filter(|wake| task_wake_is_still_valid(session, wake, done_ids))
        .collect();
}

pub(super) fn task_wake_is_still_valid(
    session: &TeamSession,
    wake: &TeamTaskWake,
    done_ids: &BTreeSet<u64>,
) -> bool {
    let owner_key = agent_key(&wake.owner);
    let Some(agent) = session.agents.get(&owner_key) else {
        return false;
    };
    if agent.status != TeamAgentStatus::Idle {
        return false;
    }
    let Some(task) = session.tasks.iter().find(|task| task.id == wake.task_id) else {
        return false;
    };
    task.status == TeamTaskStatus::Pending
        && task_dependencies_satisfied(task, done_ids)
        && task.owner.as_deref().map(agent_key).as_deref() == Some(owner_key.as_str())
}

pub(super) fn task_wake_ids_for_agent(
    session: &TeamSession,
    agent_key_value: &str,
) -> BTreeSet<u64> {
    session
        .pending_task_wakes
        .iter()
        .filter(|wake| agent_key(&wake.owner) == agent_key_value)
        .map(|wake| wake.task_id)
        .collect()
}

pub(super) fn remove_task_wakes_for_task(session: &mut TeamSession, task_id: u64) {
    session
        .pending_task_wakes
        .retain(|wake| wake.task_id != task_id);
}

pub(super) fn in_progress_task_id_for_agent(
    session: &TeamSession,
    agent_key_value: &str,
) -> Option<u64> {
    session
        .tasks
        .iter()
        .find(|task| {
            task.status == TeamTaskStatus::InProgress
                && task.owner.as_deref().map(agent_key).as_deref() == Some(agent_key_value)
        })
        .map(|task| task.id)
}

pub(super) fn ready_pending_task_id_for_agent(
    session: &TeamSession,
    agent_key_value: &str,
    done_ids: &BTreeSet<u64>,
) -> Option<u64> {
    session
        .tasks
        .iter()
        .find(|task| {
            task.status == TeamTaskStatus::Pending
                && task_dependencies_satisfied(task, done_ids)
                && task.owner.as_deref().map(agent_key).as_deref() == Some(agent_key_value)
        })
        .map(|task| task.id)
}

pub(super) fn agent_has_runnable_task(
    session: &TeamSession,
    agent_key_value: &str,
    done_ids: &BTreeSet<u64>,
) -> bool {
    in_progress_task_id_for_agent(session, agent_key_value).is_some()
        || ready_pending_task_id_for_agent(session, agent_key_value, done_ids).is_some()
}

pub(super) fn agent_has_blocked_task(session: &TeamSession, agent_key_value: &str) -> bool {
    session.tasks.iter().any(|task| {
        task.status == TeamTaskStatus::Blocked
            && task.owner.as_deref().map(agent_key).as_deref() == Some(agent_key_value)
    })
}

pub(super) fn task_dependencies_satisfied(task: &TeamTask, done_ids: &BTreeSet<u64>) -> bool {
    task.blocked_by.iter().all(|id| done_ids.contains(id))
}

pub(super) fn file_change_line_counts(change: &FileChange) -> (usize, usize) {
    change
        .lines
        .iter()
        .fold((0usize, 0usize), |(added, removed), line| match line.kind {
            DiffLineKind::Added => (added + 1, removed),
            DiffLineKind::Removed => (added, removed + 1),
            DiffLineKind::Context => (added, removed),
        })
}

pub(super) fn team_ready_task_message(task: &TeamTask) -> String {
    format!(
        "<task_ready>\n{}\n</task_ready>\n\nThis task is ready and assigned to you. The current board is already in your system context; do not call TaskList action=list just to inspect it. Start this task with TaskList action=update taskId={} status=in_progress, do the work, then mark it completed when finished. Only sleep if the task is actually status=blocked with real blockedBy task IDs.",
        render_task_line(task),
        task.id
    )
}

pub(super) fn team_continue_task_message(task: &TeamTask) -> String {
    format!(
        "<task_continue>\n{}\n</task_continue>\n\nThis task is still in progress and assigned to you. Continue working now. If it is genuinely blocked, use TaskList action=update taskId={} status=blocked blockedBy=[...] with real blocking task IDs; otherwise keep working and mark it completed when finished. Do not sleep while this task is not truly blocked.",
        render_task_line(task),
        task.id
    )
}

#[cfg(test)]
pub(super) fn team_unlocked_task_message(task: &TeamTask) -> String {
    team_ready_task_message(task)
}
