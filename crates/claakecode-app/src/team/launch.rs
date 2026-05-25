use super::*;

impl TeamTool {
    pub async fn run(
        &self,
        tool_call_id: &str,
        name: &str,
        input: Value,
        mode: AgentMode,
        parent_event_tx: mpsc::UnboundedSender<AgentEvent>,
    ) -> Option<ToolRunResult> {
        let result = match name {
            TEAM_RUN_TOOL => {
                self.run_team_run(tool_call_id, input, mode, parent_event_tx)
                    .await
            }
            TEAM_CREATE_TOOL => ToolRunResult::err(
                "TeamCreate is disabled. Use TeamRun to start an agent team.",
                Vec::new(),
            ),
            AGENT_TOOL => ToolRunResult::err(
                "Agent is disabled for teams. Use TeamRun to start an agent team.",
                Vec::new(),
            ),
            SEND_MESSAGE_TOOL => {
                self.run_send_message(tool_call_id, input, mode, parent_event_tx)
                    .await
            }
            TASK_CREATE_TOOL => self.run_task_create(input, mode, parent_event_tx).await,
            TASK_LIST_TOOL => self.run_task_list(input, mode, parent_event_tx).await,
            TASK_UPDATE_TOOL => self.run_task_update(input, mode, parent_event_tx).await,
            TEAM_STATUS_TOOL => self.run_status(input).await,
            TEAM_STOP_TOOL => self.run_stop(input, parent_event_tx).await,
            _ => return None,
        };
        Some(result)
    }

    pub(super) async fn run_team_run(
        &self,
        tool_call_id: &str,
        input: Value,
        mode: AgentMode,
        parent_event_tx: mpsc::UnboundedSender<AgentEvent>,
    ) -> ToolRunResult {
        if self.current_agent.is_some() {
            return ToolRunResult::err(
                "TeamRun can only be started by the user-facing agent",
                Vec::new(),
            );
        }
        let parsed: TeamRunInput = match serde_json::from_value(input) {
            Ok(value) => value,
            Err(err) => {
                return ToolRunResult::err(format!("invalid TeamRun input: {err}"), Vec::new())
            }
        };
        if let Some(key) = parsed.extra.keys().next() {
            return ToolRunResult::err(format!("unknown TeamRun field `{key}`"), Vec::new());
        }
        let agent_profiles = match parsed.agent_profiles.as_ref() {
            Some(value) => match value.to_profile_map() {
                Ok(value) => Some(value),
                Err(err) => return ToolRunResult::err(err, Vec::new()),
            },
            None => None,
        };
        let agent_prompt_inputs = match parsed.agent_prompts.as_ref() {
            Some(value) => match value.to_prompt_map() {
                Ok(value) => Some(value),
                Err(err) => return ToolRunResult::err(err, Vec::new()),
            },
            None => None,
        };
        let has_start_only_fields = parsed.objective.is_some()
            || parsed.agent_names.is_some()
            || agent_profiles.is_some()
            || agent_prompt_inputs.is_some()
            || parsed.tasks.is_some();
        if let Some(agent_name) = parsed
            .agent
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if has_start_only_fields {
                return ToolRunResult::err("TeamRun restart accepts only agent", Vec::new());
            }
            return self
                .run_team_agent_restart(tool_call_id, None, agent_name, mode, parent_event_tx)
                .await;
        }

        let objective = parsed.objective.as_deref().map(str::trim).unwrap_or("");
        if objective.is_empty() {
            return ToolRunResult::err(
                "objective is required when starting a new team",
                Vec::new(),
            );
        }
        let team_name = format!(
            "team-{}",
            agent_key(objective).chars().take(32).collect::<String>()
        );
        let agent_names = match prepare_team_agent_names(parsed.agent_names) {
            Ok(value) => value,
            Err(err) => return ToolRunResult::err(err, Vec::new()),
        };
        let initial_tasks = match prepare_team_run_tasks(parsed.tasks.as_deref(), &agent_names) {
            Ok(value) => value,
            Err(err) => return ToolRunResult::err(err, Vec::new()),
        };
        let agent_prompts =
            match prepare_team_agent_prompts(&agent_names, agent_prompt_inputs.as_ref()) {
                Ok(value) => value,
                Err(err) => return ToolRunResult::err(err, Vec::new()),
            };
        let agent_configs =
            match self.prepare_team_agent_configs(&agent_names, agent_profiles.as_ref()) {
                Ok(value) => value,
                Err(err) => return ToolRunResult::err(err, Vec::new()),
            };
        self.create_or_reset_team(&team_name, Some(objective.to_string()))
            .await;
        for config in &agent_configs {
            self.ensure_agent(
                &team_name,
                &config.name,
                config.description.clone(),
                config.prompt.clone(),
                config.model.clone(),
            )
            .await;
        }
        if let Err(err) = self.seed_team_run_tasks(&team_name, initial_tasks).await {
            return ToolRunResult::err(err, Vec::new());
        }

        let initial_turns = agent_names
            .iter()
            .map(|agent_name| {
                let agent_prompt = agent_prompts
                    .get(&agent_key(agent_name))
                    .map(String::as_str);
                TeamTurn {
                    agent_name: agent_name.clone(),
                    message: team_kickoff_message(objective, agent_name, agent_prompt),
                    task_id: None,
                    label: "initial team kickoff".to_string(),
                }
            })
            .collect::<Vec<_>>();
        let team_cancel = TurnCancel::empty();
        self.register_team_cancel(&team_name, team_cancel.clone())
            .await;
        self.with_cancel(team_cancel).spawn_agent_team_live(
            tool_call_id.to_string(),
            team_name.clone(),
            initial_turns,
            mode,
            parent_event_tx,
        );

        self.team_run_started_result(&team_name, "Agent Swarm started in background")
            .await
    }

    pub(super) async fn run_team_agent_restart(
        &self,
        tool_call_id: &str,
        team_name: Option<&str>,
        agent_name: &str,
        mode: AgentMode,
        parent_event_tx: mpsc::UnboundedSender<AgentEvent>,
    ) -> ToolRunResult {
        let team_name = match self.resolve_team_name(team_name).await {
            Ok(value) => value,
            Err(err) => return ToolRunResult::err(err, Vec::new()),
        };
        let agent_name = match self.prepare_agent_restart(&team_name, agent_name).await {
            Ok(value) => value,
            Err(err) => return ToolRunResult::err(err, Vec::new()),
        };
        let message = team_restart_message(&team_name, &agent_name);
        let team_cancel = TurnCancel::empty();
        self.register_team_cancel(&team_name, team_cancel.clone())
            .await;
        let restart_turn = TeamTurn {
            agent_name: agent_name.clone(),
            message,
            task_id: None,
            label: "restart".to_string(),
        };
        self.with_cancel(team_cancel).spawn_agent_team_live(
            tool_call_id.to_string(),
            team_name.clone(),
            vec![restart_turn],
            mode,
            parent_event_tx,
        );
        self.team_run_started_result(
            &team_name,
            &format!("restarted teammate @{agent_name} in background"),
        )
        .await
    }

    pub(super) fn spawn_agent_team_live(
        &self,
        tool_call_id: String,
        team_name: String,
        initial_turns: Vec<TeamTurn>,
        mode: AgentMode,
        parent_event_tx: mpsc::UnboundedSender<AgentEvent>,
    ) {
        let tool = self.clone();
        tokio::spawn(async move {
            let mut result = tool
                .run_agent_team_live(
                    &tool_call_id,
                    &team_name,
                    initial_turns,
                    mode,
                    parent_event_tx.clone(),
                )
                .await;
            tool.attach_team_run_status_meta(&team_name, &mut result, "completed")
                .await;
            let _ = parent_event_tx.send(AgentEvent::ToolFinished {
                id: tool_call_id,
                output: result.content,
                is_error: result.is_error,
                file_changes: result.file_changes,
                images: result.images,
                meta: result.meta,
            });
        });
    }

    pub(super) async fn register_team_cancel(&self, team_name: &str, cancel: TurnCancel) {
        let mut runtime = self.runtime.write().await;
        runtime
            .scopes
            .entry(self.scope_id.clone())
            .or_default()
            .team_cancels
            .entry(team_name.to_string())
            .or_default()
            .push(cancel);
    }

    pub(super) async fn workspace_write_lock(&self) -> Arc<Semaphore> {
        let key = workspace_write_lock_key(&self.workspace_root);
        let mut runtime = self.runtime.write().await;
        runtime
            .workspace_write_locks
            .entry(key)
            .or_insert_with(|| Arc::new(Semaphore::new(1)))
            .clone()
    }

    pub(super) async fn agent_notify(&self, team_name: &str, agent_name: &str) -> Arc<Notify> {
        let key = agent_notify_key(&self.scope_id, team_name, agent_name);
        let mut runtime = self.runtime.write().await;
        runtime
            .agent_notifiers
            .entry(key)
            .or_insert_with(|| Arc::new(Notify::new()))
            .clone()
    }

    pub(super) async fn notify_team_agents(&self, team_name: &str, agent_names: &[String]) {
        if agent_names.is_empty() {
            return;
        }
        let keys = agent_names
            .iter()
            .map(|agent_name| agent_notify_key(&self.scope_id, team_name, agent_name))
            .collect::<Vec<_>>();
        let notifiers = {
            let runtime = self.runtime.read().await;
            keys.iter()
                .filter_map(|key| runtime.agent_notifiers.get(key).cloned())
                .collect::<Vec<_>>()
        };
        for notifier in notifiers {
            wake_notifier(&notifier);
        }
    }

    pub(super) async fn notify_all_team_agents(&self, team_name: &str) {
        let notifiers = {
            let runtime = self.runtime.read().await;
            let Some(session) = runtime
                .scopes
                .get(&self.scope_id)
                .and_then(|scope| scope.teams.get(team_name))
            else {
                return;
            };
            let mut notifiers = Vec::new();
            for agent in session.agents.values() {
                let key = agent_notify_key(&self.scope_id, team_name, &agent.name);
                if let Some(notifier) = runtime.agent_notifiers.get(&key) {
                    notifiers.push(notifier.clone());
                }
            }
            notifiers
        };
        for notifier in notifiers {
            wake_notifier(&notifier);
        }
    }

    pub(super) async fn team_run_started_result(
        &self,
        team_name: &str,
        label: &str,
    ) -> ToolRunResult {
        let snapshot = self.team_snapshot(team_name).await;
        let subagents = self.team_subagents_meta(team_name).await;
        let content = match &snapshot {
            Some(snapshot) => format!(
                "{label}\n\n{}\n\nAgent Swarm is running asynchronously. Do not poll with shell commands or TeamStatus to check progress; end this turn after acknowledging launch and wait for a user/system wake.",
                render_team_snapshot(snapshot)
            ),
            None => format!(
                "{label}\n\nAgent Swarm is running asynchronously. Do not poll with shell commands or TeamStatus to check progress; end this turn after acknowledging launch and wait for a user/system wake."
            ),
        };
        let mut result = match snapshot {
            Some(snapshot) => ToolRunResult::ok_with_meta(
                content,
                Vec::new(),
                json!({ "team": snapshot, "subagents": subagents }),
            ),
            None => ToolRunResult::ok(content, Vec::new()),
        };
        self.attach_team_run_status_meta(team_name, &mut result, "running")
            .await;
        result
    }
}
