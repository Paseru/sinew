use super::*;

impl TeamTool {
    pub(super) async fn run_send_message(
        &self,
        _tool_call_id: &str,
        input: Value,
        _mode: AgentMode,
        _parent_event_tx: mpsc::UnboundedSender<AgentEvent>,
    ) -> ToolRunResult {
        let parsed: SendMessageInput = match serde_json::from_value(input) {
            Ok(value) => value,
            Err(err) => {
                return ToolRunResult::err(format!("invalid SendMessage input: {err}"), Vec::new())
            }
        };
        let to = parsed.to.trim();
        let message = parsed.message.trim();
        if to.is_empty() {
            return ToolRunResult::err("to is required", Vec::new());
        }
        if message.is_empty() {
            return ToolRunResult::err("message is required", Vec::new());
        }
        let team_name = match self.resolve_team_name(parsed.team_name.as_deref()).await {
            Ok(value) => value,
            Err(err) => return ToolRunResult::err(err, Vec::new()),
        };
        let from = self
            .current_agent
            .as_ref()
            .filter(|identity| identity.team_name == team_name)
            .map(|identity| identity.agent_name.as_str())
            .unwrap_or("user");
        if self.current_agent.is_none() {
            return ToolRunResult::err(
                "SendMessage is peer-only. Start a team with TeamRun.",
                Vec::new(),
            );
        }
        if agent_key(to) == "team-lead" {
            return ToolRunResult::ok(
                "message not delivered: an Agent Swarm has no lead; use the shared task board or message teammates directly.",
                Vec::new(),
            );
        }

        let queued = self.queue_team_message(&team_name, from, to, message).await;
        match queued {
            Ok(messages) => {
                self.emit_peer_messages(&team_name, &messages, _parent_event_tx)
                    .await;
                let count = messages.len();
                ToolRunResult::ok(format!("queued {count} peer message(s)"), Vec::new())
            }
            Err(err) => ToolRunResult::err(err, Vec::new()),
        }
    }

    pub(super) async fn queue_team_message(
        &self,
        team_name: &str,
        from: &str,
        to: &str,
        message: &str,
    ) -> std::result::Result<Vec<TeamQueuedMessage>, String> {
        let (queued, recipients) = {
            let mut runtime = self.runtime.write().await;
            let session = runtime
                .scopes
                .get_mut(&self.scope_id)
                .and_then(|scope| scope.teams.get_mut(team_name))
                .ok_or_else(|| format!("team `{team_name}` not found"))?;
            let recipients = if to == "*" {
                session
                    .agents
                    .values()
                    .filter(|agent| {
                        agent.status != TeamAgentStatus::Stopped
                            && agent_key(&agent.name) != agent_key(from)
                    })
                    .map(|agent| agent.name.clone())
                    .collect::<Vec<_>>()
            } else {
                let key = agent_key(to);
                let Some(agent) = session
                    .agents
                    .values()
                    .find(|agent| agent_key(&agent.name) == key)
                else {
                    return Err(format!("teammate `{to}` not found"));
                };
                if agent.status == TeamAgentStatus::Stopped {
                    return Err(format!("teammate `{}` is stopped", agent.name));
                }
                vec![agent.name.clone()]
            };
            if recipients.is_empty() {
                return Err("no teammates to message".to_string());
            }
            let now = now_ms();
            let mut queued = Vec::new();
            for recipient in &recipients {
                let id = session.next_message_id;
                session.next_message_id += 1;
                let message = TeamQueuedMessage {
                    id,
                    from: from.to_string(),
                    to: recipient.clone(),
                    target: Some(to.to_string()),
                    message: message.to_string(),
                    created_at_ms: now,
                };
                session.queued_messages.push(message.clone());
                queued.push(message);
            }
            session.updated_at_ms = now;
            (queued, recipients)
        };
        self.notify_team_agents(team_name, &recipients).await;
        Ok(queued)
    }

    pub(super) async fn emit_peer_messages(
        &self,
        team_name: &str,
        messages: &[TeamQueuedMessage],
        event_tx: mpsc::UnboundedSender<AgentEvent>,
    ) {
        if messages.is_empty() {
            return;
        }
        let runtime = self.runtime.read().await;
        let Some(session) = runtime
            .scopes
            .get(&self.scope_id)
            .and_then(|scope| scope.teams.get(team_name))
        else {
            return;
        };
        for message in messages {
            let Some(agent) = session
                .agents
                .values()
                .find(|agent| agent_key(&agent.name) == agent_key(&message.to))
            else {
                continue;
            };
            let _ = event_tx.send(AgentEvent::SubAgentEvent {
                id: format!("agent:{}", agent.id),
                agent_id: agent.id.clone(),
                agent_name: agent.name.clone(),
                team_name: Some(team_name.to_string()),
                model: agent.model.clone(),
                initial_message: None,
                event: Box::new(AgentEvent::PeerMessageReceived {
                    id: message.id.to_string(),
                    from: message.from.clone(),
                    to: message.target.clone().unwrap_or_else(|| message.to.clone()),
                    message: message.message.clone(),
                }),
            });
        }
    }
}
