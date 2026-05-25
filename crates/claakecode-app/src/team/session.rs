use super::*;

impl TeamTool {
    pub(super) async fn create_or_reset_team(&self, team_name: &str, description: Option<String>) {
        let now = now_ms();
        let mut runtime = self.runtime.write().await;
        let notify_prefix = team_notify_key_prefix(&self.scope_id, team_name);
        runtime
            .agent_notifiers
            .retain(|key, _| !key.starts_with(&notify_prefix));
        let scope = runtime.scopes.entry(self.scope_id.clone()).or_default();
        scope.team_cancels.remove(team_name);
        scope.teams.insert(
            team_name.to_string(),
            TeamSession {
                name: team_name.to_string(),
                description,
                created_at_ms: now,
                updated_at_ms: now,
                agents: HashMap::new(),
                tasks: Vec::new(),
                next_task_id: 1,
                queued_messages: Vec::new(),
                next_message_id: 1,
                pending_task_wakes: Vec::new(),
                recent_file_changes: Vec::new(),
            },
        );
        scope.active_team = Some(team_name.to_string());
    }

    pub(super) async fn seed_team_run_tasks(
        &self,
        team_name: &str,
        tasks: Vec<PreparedTeamRunTask>,
    ) -> std::result::Result<(), String> {
        if tasks.is_empty() {
            return Ok(());
        }
        let mut runtime = self.runtime.write().await;
        let scope = runtime
            .scopes
            .get_mut(&self.scope_id)
            .ok_or_else(|| "no active team found; start one with TeamRun first".to_string())?;
        let session = scope
            .teams
            .get_mut(team_name)
            .ok_or_else(|| format!("team `{team_name}` not found"))?;
        let now = now_ms();
        for task in tasks {
            let id = session.next_task_id;
            session.next_task_id += 1;
            let status = if task.blocked_by.is_empty() {
                TeamTaskStatus::Pending
            } else {
                TeamTaskStatus::Blocked
            };
            session.tasks.push(TeamTask {
                id,
                subject: task.subject,
                description: task.description,
                status,
                owner: task.owner,
                blocked_by: task.blocked_by,
                created_by: "main-agent".to_string(),
                created_at_ms: now,
                updated_at_ms: now,
                completed_at_ms: None,
            });
        }
        session.updated_at_ms = now;
        Ok(())
    }

    pub(super) async fn team_snapshot(&self, team_name: &str) -> Option<TeamSnapshot> {
        let runtime = self.runtime.read().await;
        runtime
            .scopes
            .get(&self.scope_id)
            .and_then(|scope| scope.teams.get(team_name))
            .map(TeamSnapshot::from_session)
    }

    pub(super) async fn attach_team_snapshot_meta(
        &self,
        team_name: &str,
        result: &mut ToolRunResult,
    ) {
        if result.is_error {
            return;
        }
        let Some(snapshot) = self.team_snapshot(team_name).await else {
            return;
        };
        let mut meta = match result.meta.take() {
            Some(Value::Object(map)) => map,
            Some(value) => {
                let mut map = serde_json::Map::new();
                map.insert("previousMeta".into(), value);
                map
            }
            None => serde_json::Map::new(),
        };
        meta.insert("team".into(), json!(snapshot));
        result.meta = Some(Value::Object(meta));
    }

    pub(super) async fn attach_team_run_status_meta(
        &self,
        team_name: &str,
        result: &mut ToolRunResult,
        fallback_status: &str,
    ) {
        let snapshot = self.team_snapshot(team_name).await;
        let subagents = self.team_subagents_meta(team_name).await;
        let status = team_run_status_label(snapshot.as_ref(), result.is_error, fallback_status);
        let mut meta = match result.meta.take() {
            Some(Value::Object(map)) => map,
            Some(value) => {
                let mut map = serde_json::Map::new();
                map.insert("previousMeta".into(), value);
                map
            }
            None => serde_json::Map::new(),
        };
        if let Some(snapshot) = snapshot {
            meta.insert("team".into(), json!(snapshot));
        }
        meta.insert("subagents".into(), json!(subagents));
        meta.insert("teamRunStatus".into(), json!(status));
        result.meta = Some(Value::Object(meta));
    }

    pub(super) async fn team_subagents_meta(&self, team_name: &str) -> Vec<Value> {
        let runtime = self.runtime.read().await;
        let mut agents = runtime
            .scopes
            .get(&self.scope_id)
            .and_then(|scope| scope.teams.get(team_name))
            .map(|session| {
                session
                    .agents
                    .values()
                    .map(|agent| {
                        let queued_messages = session
                            .queued_messages
                            .iter()
                            .filter(|message| agent_key(&message.to) == agent_key(&agent.name))
                            .map(|message| {
                                json!({
                                    "id": message.id.to_string(),
                                    "from": message.from.clone(),
                                    "to": message.target.clone().unwrap_or_else(|| message.to.clone()),
                                    "message": message.message.clone(),
                                })
                            })
                            .collect::<Vec<_>>();
                        json!({
                            "id": agent.id.clone(),
                            "name": agent.name.clone(),
                            "model": agent.model.clone(),
                            "history": agent.history.clone(),
                            "status": agent.status,
                            "queuedMessages": queued_messages,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        agents.sort_by(|left, right| {
            left.get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .cmp(
                    right
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                )
        });
        agents
    }

    pub(super) async fn team_agent_final_responses(
        &self,
        team_name: &str,
    ) -> Vec<TeamAgentFinalResponse> {
        let runtime = self.runtime.read().await;
        runtime
            .scopes
            .get(&self.scope_id)
            .and_then(|scope| scope.teams.get(team_name))
            .map(team_agent_final_responses_from_session)
            .unwrap_or_default()
    }

    pub(super) async fn block_task_after_agent_error(
        &self,
        team_name: &str,
        task_id: u64,
        _agent_name: &str,
        _error: &str,
    ) {
        let changed = {
            let mut runtime = self.runtime.write().await;
            let Some(session) = runtime
                .scopes
                .get_mut(&self.scope_id)
                .and_then(|scope| scope.teams.get_mut(team_name))
            else {
                return;
            };
            let Some(task) = session.tasks.iter_mut().find(|task| task.id == task_id) else {
                return;
            };
            let now = now_ms();
            task.status = TeamTaskStatus::Pending;
            task.owner = None;
            task.completed_at_ms = None;
            task.updated_at_ms = now;
            session.updated_at_ms = now;
            true
        };
        if changed {
            self.notify_all_team_agents(team_name).await;
        }
    }

    pub(super) async fn ensure_agent(
        &self,
        team_name: &str,
        agent_name: &str,
        description: String,
        prompt: String,
        model: ModelRef,
    ) {
        let now = now_ms();
        let mut runtime = self.runtime.write().await;
        let scope = runtime.scopes.entry(self.scope_id.clone()).or_default();
        let session = scope
            .teams
            .entry(team_name.to_string())
            .or_insert_with(|| TeamSession {
                name: team_name.to_string(),
                description: None,
                created_at_ms: now,
                updated_at_ms: now,
                agents: HashMap::new(),
                tasks: Vec::new(),
                next_task_id: 1,
                queued_messages: Vec::new(),
                next_message_id: 1,
                pending_task_wakes: Vec::new(),
                recent_file_changes: Vec::new(),
            });
        let key = agent_key(agent_name);
        let agent = session.agents.entry(key).or_insert_with(|| TeamAgent {
            id: format!("{}@{}", agent_key(agent_name), team_name),
            name: agent_name.trim().to_string(),
            description: description.clone(),
            prompt: prompt.clone(),
            model: model.clone(),
            status: TeamAgentStatus::Idle,
            history: Vec::new(),
            created_at_ms: now,
            updated_at_ms: now,
            last_summary: None,
            last_error: None,
        });
        agent.description = description;
        agent.prompt = prompt;
        agent.model = model;
        if agent.status == TeamAgentStatus::Stopped {
            agent.status = TeamAgentStatus::Idle;
        }
        agent.updated_at_ms = now;
        session.updated_at_ms = now;
        scope.active_team = Some(team_name.to_string());
    }

    pub(super) async fn prepare_agent_restart(
        &self,
        team_name: &str,
        agent_name: &str,
    ) -> std::result::Result<String, String> {
        let mut runtime = self.runtime.write().await;
        let scope = runtime
            .scopes
            .get_mut(&self.scope_id)
            .ok_or_else(|| "no active team found; start one with TeamRun first".to_string())?;
        scope.active_team = Some(team_name.to_string());
        let session = scope
            .teams
            .get_mut(team_name)
            .ok_or_else(|| format!("team `{team_name}` not found"))?;
        let key = agent_key(agent_name);
        let agent_name = {
            let agent = session
                .agents
                .get_mut(&key)
                .ok_or_else(|| format!("teammate `{agent_name}` not found"))?;
            if agent.status == TeamAgentStatus::Running {
                return Err(format!("teammate `{}` is already running", agent.name));
            }
            agent.status = TeamAgentStatus::Idle;
            agent.updated_at_ms = now_ms();
            agent.name.clone()
        };
        let done_ids = completed_task_ids(session);
        refresh_unblocked_tasks(session, &done_ids);
        session.updated_at_ms = now_ms();
        drop(runtime);
        self.notify_all_team_agents(team_name).await;
        Ok(agent_name)
    }
}
