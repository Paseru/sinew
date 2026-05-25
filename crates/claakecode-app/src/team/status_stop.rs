use super::*;

impl TeamTool {
    pub(super) async fn run_status(&self, input: Value) -> ToolRunResult {
        if self.current_agent.is_some() {
            return ToolRunResult::err(
                "TeamStatus is only available to the main agent.",
                Vec::new(),
            );
        }
        let input = normalize_optional_object_input(input);
        let parsed: TeamNameInput = match serde_json::from_value(input) {
            Ok(value) => value,
            Err(err) => {
                return ToolRunResult::err(format!("invalid TeamStatus input: {err}"), Vec::new())
            }
        };
        let team_name = match self.resolve_team_name(parsed.team_name.as_deref()).await {
            Ok(value) => value,
            Err(_) => return ToolRunResult::ok("no active Agent Swarm", Vec::new()),
        };
        let runtime = self.runtime.read().await;
        let Some(session) = runtime
            .scopes
            .get(&self.scope_id)
            .and_then(|scope| scope.teams.get(&team_name))
        else {
            return ToolRunResult::ok(format!("no Agent Swarm named `{team_name}`"), Vec::new());
        };
        let snapshot = TeamSnapshot::from_session(session);
        ToolRunResult::ok_with_meta(
            render_team_snapshot(&snapshot),
            Vec::new(),
            json!({ "team": snapshot }),
        )
    }

    pub(super) async fn run_stop(
        &self,
        input: Value,
        parent_event_tx: mpsc::UnboundedSender<AgentEvent>,
    ) -> ToolRunResult {
        if self.current_agent.is_some() {
            return ToolRunResult::err("TeamStop is only available to the main agent.", Vec::new());
        }
        let input = normalize_optional_object_input(input);
        let parsed: TeamStopInput = match serde_json::from_value(input) {
            Ok(value) => value,
            Err(err) => {
                return ToolRunResult::err(format!("invalid TeamStop input: {err}"), Vec::new())
            }
        };
        let team_name = match self.resolve_team_name(parsed.team_name.as_deref()).await {
            Ok(value) => value,
            Err(_) => return ToolRunResult::ok("no active Agent Swarm to stop", Vec::new()),
        };
        let mut runtime = self.runtime.write().await;
        let Some(session) = runtime
            .scopes
            .get_mut(&self.scope_id)
            .and_then(|scope| scope.teams.get_mut(&team_name))
        else {
            return ToolRunResult::ok(
                format!("no Agent Swarm named `{team_name}` to stop"),
                Vec::new(),
            );
        };
        let agent = parsed
            .agent
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(agent_name) = agent {
            let agent_key_value = agent_key(agent_name);
            let Some(agent) = session.agents.get_mut(&agent_key_value) else {
                return ToolRunResult::err(
                    format!("teammate `{agent_name}` not found"),
                    Vec::new(),
                );
            };
            agent.status = TeamAgentStatus::Stopped;
            agent.updated_at_ms = now_ms();
            let stopped_agent_name = agent.name.clone();
            let now = agent.updated_at_ms;
            let mut reset_count = 0usize;
            for task in &mut session.tasks {
                if task.owner.as_deref().map(agent_key).as_deref() != Some(agent_key_value.as_str())
                {
                    continue;
                }
                if matches!(
                    task.status,
                    TeamTaskStatus::InProgress | TeamTaskStatus::Blocked
                ) {
                    task.status = TeamTaskStatus::Pending;
                    task.owner = None;
                    task.completed_at_ms = None;
                    task.updated_at_ms = now;
                    reset_count += 1;
                }
            }
            session.updated_at_ms = now;
            drop(runtime);
            self.notify_all_team_agents(&team_name).await;
            if let Ok(messages) = self
                .queue_team_message(
                    &team_name,
                    "system",
                    "*",
                    &format!(
                        "@{} left the team. Their open task(s) are back in pending.",
                        stopped_agent_name
                    ),
                )
                .await
            {
                self.emit_peer_messages(&team_name, &messages, parent_event_tx)
                    .await;
            }
            let mut result = ToolRunResult::ok(
                format!(
                    "stopped teammate: {} ({} open task(s) reset to pending)",
                    stopped_agent_name, reset_count
                ),
                Vec::new(),
            );
            self.attach_team_snapshot_meta(&team_name, &mut result)
                .await;
            return result;
        }

        for agent in session.agents.values_mut() {
            agent.status = TeamAgentStatus::Stopped;
            agent.updated_at_ms = now_ms();
        }
        session.updated_at_ms = now_ms();
        let stopped_count = session.agents.len();
        let agent_names = session
            .agents
            .values()
            .map(|agent| agent.name.clone())
            .collect::<Vec<_>>();
        let snapshot = TeamSnapshot::from_session(session);
        let notifiers = agent_names
            .iter()
            .filter_map(|agent_name| {
                runtime
                    .agent_notifiers
                    .get(&agent_notify_key(&self.scope_id, &team_name, agent_name))
                    .cloned()
            })
            .collect::<Vec<_>>();
        let cancels = runtime
            .scopes
            .get_mut(&self.scope_id)
            .map(|scope| {
                if scope.active_team.as_deref() == Some(team_name.as_str()) {
                    scope.active_team = None;
                }
                scope.teams.remove(&team_name);
                scope.team_cancels.remove(&team_name).unwrap_or_default()
            })
            .unwrap_or_default();
        let notify_prefix = team_notify_key_prefix(&self.scope_id, &team_name);
        runtime
            .agent_notifiers
            .retain(|key, _| !key.starts_with(&notify_prefix));
        drop(runtime);
        for notifier in notifiers {
            wake_notifier(&notifier);
        }
        for cancel in cancels {
            cancel.cancel_all();
        }
        ToolRunResult::ok_with_meta(
            format!("stopped Agent Swarm ({} teammate(s))", stopped_count),
            Vec::new(),
            json!({ "team": snapshot }),
        )
    }

    pub(super) async fn resolve_team_name(
        &self,
        explicit: Option<&str>,
    ) -> std::result::Result<String, String> {
        if let Some(value) = explicit.map(str::trim).filter(|value| !value.is_empty()) {
            return Ok(value.to_string());
        }
        if let Some(identity) = &self.current_agent {
            return Ok(identity.team_name.clone());
        }
        let runtime = self.runtime.read().await;
        let Some(scope) = runtime.scopes.get(&self.scope_id) else {
            return Err("no active team found; start one with TeamRun first".to_string());
        };
        scope
            .active_team
            .clone()
            .or_else(|| {
                if scope.teams.len() == 1 {
                    scope.teams.keys().next().cloned()
                } else {
                    None
                }
            })
            .ok_or_else(|| "team_name is required when no active team exists".to_string())
    }
}
