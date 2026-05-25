use super::*;

impl TeamTool {
    pub(super) async fn run_agent_turn(
        &self,
        tool_call_id: &str,
        team_name: &str,
        agent_name: &str,
        message: String,
        mode: AgentMode,
        parent_event_tx: mpsc::UnboundedSender<AgentEvent>,
    ) -> ToolRunResult {
        let agent = match self.take_agent_for_run(team_name, agent_name).await {
            Ok(value) => value,
            Err(err) => return ToolRunResult::err(err, Vec::new()),
        };

        let Some(provider) = self.providers.get(&agent.model.provider).cloned() else {
            let error = format!(
                "provider `{}` is not configured or missing credentials",
                agent.model.provider
            );
            self.finish_agent_error(team_name, &agent.name, &error)
                .await;
            return ToolRunResult::err(error, Vec::new());
        };
        if provider.capabilities(&agent.model).is_none() {
            let error = format!("model `{}` is not supported", agent.model.name);
            self.finish_agent_error(team_name, &agent.name, &error)
                .await;
            return ToolRunResult::err(error, Vec::new());
        }

        let mut history = agent.history.clone();
        history.push(ChatMessage {
            role: Role::User,
            parts: vec![Part::Text {
                text: message.clone(),
                meta: None,
            }],
        });

        let (child_cmd_tx, child_cmd_rx) = mpsc::unbounded_channel();
        let (child_event_tx, mut child_event_rx) = mpsc::unbounded_channel();
        self.cancel.register(child_cmd_tx);
        let team_tool = Arc::new(self.for_agent(team_name.to_string(), agent.name.clone()));
        let workspace_write_lock = self.workspace_write_lock().await;
        let child_mode = if mode == AgentMode::Goal {
            AgentMode::Act
        } else {
            mode
        };
        let child_context = TurnContext {
            provider,
            model: agent.model.clone(),
            cache_key: Some(format!(
                "team:{}:{}:{}",
                self.scope_id,
                team_name,
                agent_key(&agent.name)
            )),
            cache_stable_message_count: agent.history.len(),
            service_tier: self.service_tier,
            auto_compact: true,
            mode: child_mode,
            stop_questions: false,
            system_prompt: team_agent_system_prompt(&self.system_prompt, team_name, &agent),
            history,
            todo_list: TodoListState::default(),
            goal_workflow: GoalWorkflowState::Idle,
            bash: Arc::new(BashTool::new(self.workspace_root.clone())),
            glob: Arc::new(GlobTool::new(self.workspace_root.clone())),
            grep: Arc::new(GrepTool::new(self.workspace_root.clone())),
            read: Arc::new(ReadTool::new(self.workspace_root.clone())),
            edit_file: Arc::new(
                EditFileTool::new(self.workspace_root.clone())
                    .with_workspace_write_lock(workspace_write_lock.clone()),
            ),
            write_file: Arc::new(
                WriteFileTool::new(self.workspace_root.clone())
                    .with_workspace_write_lock(workspace_write_lock.clone()),
            ),
            create_image: Arc::new(
                CreateImageTool::with_settings(
                    self.workspace_root.clone(),
                    self.tool_settings.image_provider,
                    self.tool_settings.openai_image_use_subscription,
                    self.tool_settings.openai_image_api_key(),
                    self.tool_settings.nano_banana_api_key(),
                )
                .with_workspace_write_lock(workspace_write_lock),
            ),
            todo_list_tool: None,
            question: None,
            web_search: Arc::new(WebSearchTool::with_settings(
                self.tool_settings.web_search_provider,
                self.tool_settings.linkup_api_key(),
            )),
            web_fetch: Arc::new(WebFetchTool::new()),
            skill: Arc::new(SkillTool::with_settings(
                self.workspace_root.clone(),
                self.skill_settings.clone(),
            )),
            database: Arc::new(self.database.clone()),
            mcp: Arc::new(McpToolRegistry::new(self.mcp_settings.clone())),
            subagents: None,
            teams: Some(team_tool),
            tool_settings: self.tool_settings.clone(),
            event_scope: Some(AgentEventScope {
                id: tool_call_id.to_string(),
                agent_id: agent.id.clone(),
                agent_name: agent.name.clone(),
                team_name: Some(team_name.to_string()),
                model: agent.model.clone(),
                initial_message: message,
            }),
            max_tool_rounds: self.max_tool_rounds,
            event_tx: child_event_tx,
            cancel: self.cancel.clone(),
            cmd_rx: child_cmd_rx,
        };

        let engine = tokio::spawn(async move { run_turn(child_context).await });
        let mut child_error: Option<String> = None;
        while let Some(event) = child_event_rx.recv().await {
            if let AgentEvent::SubAgentEvent { event: inner, .. } = &event {
                if let AgentEvent::Error { message } = inner.as_ref() {
                    child_error.get_or_insert_with(|| message.clone());
                }
            }
            let _ = parent_event_tx.send(event);
        }
        let output = match engine.await {
            Ok(output) => output,
            Err(err) => {
                let error = format!("teammate task failed: {err}");
                self.finish_agent_error(team_name, &agent.name, &error)
                    .await;
                return ToolRunResult::err(error, Vec::new());
            }
        };
        let file_changes = file_changes_from_history(&output.history);
        if let Some(error) = child_error {
            let updated_agent = self
                .finish_agent_failure(team_name, &agent.name, output.history, error.clone())
                .await;
            let mut result = ToolRunResult::err(
                render_agent_result(team_name, &updated_agent, &error),
                file_changes,
            );
            result.meta = Some(json!({
                "subagent": {
                    "id": updated_agent.id,
                    "name": updated_agent.name,
                    "model": updated_agent.model,
                    "history": updated_agent.history,
                },
                "team": {
                    "name": team_name,
                    "agent": updated_agent,
                }
            }));
            return result;
        }
        let final_answer = final_assistant_text(&output.history)
            .unwrap_or_else(|| "Teammate finished without a final answer.".to_string());
        let updated_agent = self
            .finish_agent_success(team_name, &agent.name, output.history, final_answer.clone())
            .await;
        if self
            .agent_sleep_allowed(team_name, &updated_agent.name)
            .await
        {
            emit_agent_slept_event(tool_call_id, team_name, &updated_agent, &parent_event_tx);
        }

        ToolRunResult::ok_with_meta(
            render_agent_result(team_name, &updated_agent, &final_answer),
            file_changes,
            json!({
                "subagent": {
                    "id": updated_agent.id,
                    "name": updated_agent.name,
                    "model": updated_agent.model,
                    "history": updated_agent.history,
                },
                "team": {
                    "name": team_name,
                    "agent": updated_agent,
                }
            }),
        )
    }

    pub(super) async fn take_agent_for_run(
        &self,
        team_name: &str,
        agent_name: &str,
    ) -> std::result::Result<TeamAgent, String> {
        let mut runtime = self.runtime.write().await;
        let scope = runtime
            .scopes
            .get_mut(&self.scope_id)
            .ok_or_else(|| "no active team found; start one with TeamRun first".to_string())?;
        let session = scope
            .teams
            .get_mut(team_name)
            .ok_or_else(|| format!("team `{team_name}` not found"))?;
        let key = agent_key(agent_name);
        let agent = session
            .agents
            .get_mut(&key)
            .ok_or_else(|| format!("teammate `{agent_name}` not found"))?;
        if agent.status == TeamAgentStatus::Running {
            return Ok(agent.clone());
        }
        if agent.status == TeamAgentStatus::Stopped {
            agent.status = TeamAgentStatus::Idle;
        }
        agent.status = TeamAgentStatus::Running;
        agent.updated_at_ms = now_ms();
        session.updated_at_ms = agent.updated_at_ms;
        Ok(agent.clone())
    }

    pub(super) async fn finish_agent_success(
        &self,
        team_name: &str,
        agent_name: &str,
        history: Vec<ChatMessage>,
        summary: String,
    ) -> TeamAgent {
        let updated_agent = {
            let mut runtime = self.runtime.write().await;
            let session = runtime
                .scopes
                .entry(self.scope_id.clone())
                .or_default()
                .teams
                .entry(team_name.to_string())
                .or_insert_with(|| TeamSession {
                    name: team_name.to_string(),
                    description: None,
                    created_at_ms: now_ms(),
                    updated_at_ms: now_ms(),
                    agents: HashMap::new(),
                    tasks: Vec::new(),
                    next_task_id: 1,
                    queued_messages: Vec::new(),
                    next_message_id: 1,
                    pending_task_wakes: Vec::new(),
                    recent_file_changes: Vec::new(),
                });
            let key = agent_key(agent_name);
            let agent = session
                .agents
                .get_mut(&key)
                .expect("agent exists after run");
            if agent.status != TeamAgentStatus::Stopped {
                agent.status = TeamAgentStatus::Idle;
            }
            agent.history = history;
            agent.last_summary = Some(summary);
            agent.last_error = None;
            agent.updated_at_ms = now_ms();
            session.updated_at_ms = agent.updated_at_ms;
            agent.clone()
        };
        self.notify_all_team_agents(team_name).await;
        updated_agent
    }

    pub(super) async fn finish_agent_failure(
        &self,
        team_name: &str,
        agent_name: &str,
        history: Vec<ChatMessage>,
        error: String,
    ) -> TeamAgent {
        let updated_agent = {
            let mut runtime = self.runtime.write().await;
            let session = runtime
                .scopes
                .entry(self.scope_id.clone())
                .or_default()
                .teams
                .entry(team_name.to_string())
                .or_insert_with(|| TeamSession {
                    name: team_name.to_string(),
                    description: None,
                    created_at_ms: now_ms(),
                    updated_at_ms: now_ms(),
                    agents: HashMap::new(),
                    tasks: Vec::new(),
                    next_task_id: 1,
                    queued_messages: Vec::new(),
                    next_message_id: 1,
                    pending_task_wakes: Vec::new(),
                    recent_file_changes: Vec::new(),
                });
            let key = agent_key(agent_name);
            let agent = session
                .agents
                .get_mut(&key)
                .expect("agent exists after run");
            let now = now_ms();
            if agent.status != TeamAgentStatus::Stopped {
                agent.status = TeamAgentStatus::Error;
            }
            agent.history = history;
            agent.last_error = Some(truncate_line(&error, 300));
            agent.last_summary = Some(format!("error: {}", truncate_line(&error, 180)));
            agent.updated_at_ms = now;
            session.updated_at_ms = now;
            agent.clone()
        };
        self.notify_all_team_agents(team_name).await;
        updated_agent
    }

    pub(super) async fn finish_agent_error(&self, team_name: &str, agent_name: &str, error: &str) {
        let changed = {
            let mut runtime = self.runtime.write().await;
            if let Some(team) = runtime
                .scopes
                .get_mut(&self.scope_id)
                .and_then(|scope| scope.teams.get_mut(team_name))
            {
                if let Some(agent) = team.agents.get_mut(&agent_key(agent_name)) {
                    let now = now_ms();
                    agent.status = TeamAgentStatus::Error;
                    agent.last_error = Some(truncate_line(error, 300));
                    agent.last_summary = Some(format!("error: {}", truncate_line(error, 180)));
                    agent.updated_at_ms = now;
                    team.updated_at_ms = now;
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };
        if changed {
            self.notify_all_team_agents(team_name).await;
        }
    }

    pub(super) async fn agent_sleep_allowed(&self, team_name: &str, agent_name: &str) -> bool {
        let mut runtime = self.runtime.write().await;
        let Some(session) = runtime
            .scopes
            .get_mut(&self.scope_id)
            .and_then(|scope| scope.teams.get_mut(team_name))
        else {
            return false;
        };
        let done_ids = completed_task_ids(session);
        refresh_unblocked_tasks(session, &done_ids);
        prune_stale_task_wakes(session, &done_ids);
        let agent_key_value = agent_key(agent_name);
        agent_has_blocked_task(session, &agent_key_value)
            && !agent_has_runnable_task(session, &agent_key_value, &done_ids)
            && !session.queued_messages.iter().any(|message| {
                agent_key(&message.to) == agent_key_value && queued_message_wakes_agent(message)
            })
    }

    pub(super) fn current_actor_name(&self, team_name: &str) -> String {
        self.current_agent
            .as_ref()
            .filter(|identity| identity.team_name == team_name)
            .map(|identity| identity.agent_name.clone())
            .unwrap_or_else(|| "user".to_string())
    }

    pub(super) fn select_profile(&self, value: Option<&str>) -> Option<SubAgentConfig> {
        let needle = value?.trim();
        if needle.is_empty() {
            return None;
        }
        let wanted = agent_key(needle);
        self.sub_agent_settings
            .agents
            .iter()
            .find(|agent| {
                agent.enabled
                    && (agent_key(&agent.id) == wanted
                        || agent_key(&agent.name) == wanted
                        || agent_key(&format!("subagent_{}", agent.id)) == wanted)
            })
            .cloned()
    }

    pub(super) fn prepare_team_agent_configs(
        &self,
        agent_names: &[String],
        agent_profiles: Option<&HashMap<String, String>>,
    ) -> std::result::Result<Vec<PreparedTeamAgentConfig>, String> {
        let mut profile_by_agent = HashMap::<String, SubAgentConfig>::new();
        if let Some(agent_profiles) = agent_profiles {
            for (agent_name, profile_name) in agent_profiles {
                let agent_name = agent_name.trim();
                let profile_name = profile_name.trim();
                if agent_name.is_empty() || profile_name.is_empty() {
                    return Err("agent_profiles keys and values cannot be empty".to_string());
                }
                let agent_key_value = agent_key(agent_name);
                let Some(canonical_name) = agent_names
                    .iter()
                    .find(|name| agent_key(name) == agent_key_value)
                else {
                    return Err(format!(
                        "agent_profiles references unknown teammate `{agent_name}`"
                    ));
                };
                let Some(profile) = self.select_profile(Some(profile_name)) else {
                    return Err(format!("sub-agent profile `{profile_name}` not found"));
                };
                profile_by_agent.insert(agent_key(canonical_name), profile);
            }
        }

        let mut configs = Vec::with_capacity(agent_names.len());
        for name in agent_names {
            let profile = profile_by_agent.get(&agent_key(name));
            let description = profile
                .map(|agent| agent.description.clone())
                .unwrap_or_else(|| "Team collaborator".to_string());
            let model = profile
                .map(|agent| agent.model.clone())
                .unwrap_or_else(|| self.default_model.clone());
            self.validate_model(&model)?;
            let prompt = profile
                .map(|agent| agent.prompt.clone())
                .unwrap_or_default();
            configs.push(PreparedTeamAgentConfig {
                name: name.clone(),
                description,
                prompt,
                model,
            });
        }
        Ok(configs)
    }

    pub(super) fn validate_model(&self, model: &ModelRef) -> std::result::Result<(), String> {
        let provider = self.providers.get(&model.provider).ok_or_else(|| {
            format!(
                "provider `{}` is not configured or missing credentials",
                model.provider
            )
        })?;
        provider
            .capabilities(model)
            .map(|_| ())
            .ok_or_else(|| format!("model `{}` is not supported", model.name))
    }
}

pub(super) fn emit_agent_slept_event(
    tool_call_id: &str,
    team_name: &str,
    agent: &TeamAgent,
    event_tx: &mpsc::UnboundedSender<AgentEvent>,
) {
    let _ = event_tx.send(AgentEvent::SubAgentEvent {
        id: tool_call_id.to_string(),
        agent_id: agent.id.clone(),
        agent_name: agent.name.clone(),
        team_name: Some(team_name.to_string()),
        model: agent.model.clone(),
        initial_message: None,
        event: Box::new(AgentEvent::AgentSlept),
    });
}
