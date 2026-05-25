use super::*;

impl TeamTool {
    pub(super) async fn run_agent_team_live(
        &self,
        tool_call_id: &str,
        team_name: &str,
        initial_turns: Vec<TeamTurn>,
        mode: AgentMode,
        parent_event_tx: mpsc::UnboundedSender<AgentEvent>,
    ) -> ToolRunResult {
        let futures = initial_turns.into_iter().map(|initial_turn| {
            let agent_name = initial_turn.agent_name.clone();
            let tool = self.clone();
            let team_name = team_name.to_string();
            let tool_call_id = tool_call_id.to_string();
            let parent_event_tx = parent_event_tx.clone();
            async move {
                let mut report = LiveAgentReport::default();
                let mut next_turn = Some(initial_turn);

                loop {
                    let turn = match next_turn.take() {
                        Some(turn) => turn,
                        None => match tool.wait_for_next_live_turn(&team_name, &agent_name).await {
                            Some(turn) => turn,
                            None => break,
                        },
                    };

                    let child_tool_call_id = format!(
                        "{tool_call_id}-live-{}-{}",
                        agent_key(&agent_name),
                        Uuid::new_v4()
                    );
                    let result = tool
                        .run_agent_turn(
                            &child_tool_call_id,
                            &team_name,
                            &turn.agent_name,
                            turn.message.clone(),
                            mode,
                            parent_event_tx.clone(),
                        )
                        .await;
                    let line = first_line(&result.content)
                        .unwrap_or(if result.is_error {
                            "agent turn failed"
                        } else {
                            "agent turn finished"
                        })
                        .to_string();
                    if result.is_error {
                        if let Some(task_id) = turn.task_id {
                            tool.block_task_after_agent_error(
                                &team_name,
                                task_id,
                                &turn.agent_name,
                                &line,
                            )
                            .await;
                        }
                    }
                    report.file_changes.extend(result.file_changes);
                    report.images.extend(result.images);
                    report.last_meta = result.meta;
                    report
                        .reports
                        .push(format!("@{}: {} ({})", turn.agent_name, line, turn.label));
                    next_turn = tool.next_live_turn_for_agent(&team_name, &agent_name).await;
                }

                report
            }
        });

        let outputs = join_all(futures).await;
        let mut reports = Vec::new();
        let mut file_changes = Vec::new();
        let mut images = Vec::new();
        let mut last_meta = None;

        for mut output in outputs {
            reports.append(&mut output.reports);
            file_changes.append(&mut output.file_changes);
            images.append(&mut output.images);
            if output.last_meta.is_some() {
                last_meta = output.last_meta;
            }
        }

        let final_responses = self.team_agent_final_responses(team_name).await;
        let final_responses_text = render_team_agent_final_responses(&final_responses);

        let content = if reports.is_empty() {
            if final_responses_text.is_empty() {
                "Agent Swarm had no runnable teammates".to_string()
            } else {
                format!(
                    "Agent Swarm finished.\n\nFinal teammate responses:\n{final_responses_text}"
                )
            }
        } else {
            let mut content = format!(
                "Agent Swarm finished after {} teammate turn(s):\n{}",
                reports.len(),
                reports.join("\n")
            );
            if !final_responses_text.is_empty() {
                content.push_str("\n\nFinal teammate responses:\n");
                content.push_str(&final_responses_text);
            }
            content
        };
        let mut result = ToolRunResult::ok(content, file_changes);
        result.images = images;
        let mut meta = serde_json::Map::new();
        if let Some(last_meta) = last_meta {
            meta.insert("lastAgentTurnMeta".into(), last_meta);
        }
        if !final_responses.is_empty() {
            meta.insert("agentFinalResponses".into(), json!(final_responses));
        }
        result.meta = (!meta.is_empty()).then_some(Value::Object(meta));
        result
    }

    pub(super) async fn wait_for_next_live_turn(
        &self,
        team_name: &str,
        agent_name: &str,
    ) -> Option<TeamTurn> {
        let notify = self.agent_notify(team_name, agent_name).await;
        loop {
            if let Some(turn) = self.next_live_turn_for_agent(team_name, agent_name).await {
                return Some(turn);
            }
            if self.team_is_settled_for_agent(team_name, agent_name).await {
                tokio::select! {
                    _ = notify.notified() => continue,
                    _ = tokio::time::sleep(Duration::from_millis(TEAM_SETTLE_GRACE_MS)) => {}
                }
                if let Some(turn) = self.next_live_turn_for_agent(team_name, agent_name).await {
                    return Some(turn);
                }
                if self.team_is_settled_for_agent(team_name, agent_name).await {
                    return None;
                }
                continue;
            }
            notify.notified().await;
        }
    }

    pub(super) async fn next_live_turn_for_agent(
        &self,
        team_name: &str,
        agent_name: &str,
    ) -> Option<TeamTurn> {
        let mut runtime = self.runtime.write().await;
        let session = runtime
            .scopes
            .get_mut(&self.scope_id)
            .and_then(|scope| scope.teams.get_mut(team_name))?;

        let agent_key_value = agent_key(agent_name);
        let agent = session.agents.get(&agent_key_value)?;
        if matches!(
            agent.status,
            TeamAgentStatus::Stopped | TeamAgentStatus::Error
        ) {
            return None;
        }

        let has_wake_message = session.queued_messages.iter().any(|message| {
            agent_key(&message.to) == agent_key_value && queued_message_wakes_agent(message)
        });
        if has_wake_message {
            let mut messages = Vec::new();
            let mut index = 0usize;
            while index < session.queued_messages.len() {
                if agent_key(&session.queued_messages[index].to) == agent_key_value {
                    messages.push(session.queued_messages.remove(index));
                } else {
                    index += 1;
                }
            }
            let now = now_ms();
            if let Some(agent) = session.agents.get_mut(&agent_key_value) {
                agent.status = TeamAgentStatus::Running;
                agent.updated_at_ms = now;
            }
            session.updated_at_ms = now;
            return Some(TeamTurn {
                agent_name: agent_name.to_string(),
                message: queued_messages_prompt(&messages),
                task_id: None,
                label: format!("{} queued message(s)", messages.len()),
            });
        }

        let done_ids = completed_task_ids(session);
        refresh_unblocked_tasks(session, &done_ids);
        prune_stale_task_wakes(session, &done_ids);

        if let Some(task_id) = in_progress_task_id_for_agent(session, &agent_key_value) {
            let task = session
                .tasks
                .iter()
                .find(|task| task.id == task_id)
                .cloned()?;
            let now = now_ms();
            if let Some(agent) = session.agents.get_mut(&agent_key_value) {
                agent.status = TeamAgentStatus::Running;
                agent.updated_at_ms = now;
            }
            session.updated_at_ms = now;
            return Some(TeamTurn {
                agent_name: agent_name.to_string(),
                message: team_continue_task_message(&task),
                task_id: Some(task_id),
                label: format!("continue task #{}", task_id),
            });
        }

        let wake_task_ids = task_wake_ids_for_agent(session, &agent_key_value);
        let task_id =
            wake_task_ids.iter().next().copied().or_else(|| {
                ready_pending_task_id_for_agent(session, &agent_key_value, &done_ids)
            })?;
        let task = session
            .tasks
            .iter()
            .find(|task| task.id == task_id)
            .cloned()?;
        remove_task_wakes_for_task(session, task_id);
        let now = now_ms();
        if let Some(agent) = session.agents.get_mut(&agent_key_value) {
            agent.status = TeamAgentStatus::Running;
            agent.updated_at_ms = now;
        }
        session.updated_at_ms = now;

        Some(TeamTurn {
            agent_name: agent_name.to_string(),
            message: team_ready_task_message(&task),
            task_id: Some(task_id),
            label: format!("ready task #{}", task_id),
        })
    }

    pub(super) async fn team_is_settled_for_agent(
        &self,
        team_name: &str,
        agent_name: &str,
    ) -> bool {
        let mut runtime = self.runtime.write().await;
        let Some(session) = runtime
            .scopes
            .get_mut(&self.scope_id)
            .and_then(|scope| scope.teams.get_mut(team_name))
        else {
            return true;
        };

        let done_ids = completed_task_ids(session);
        refresh_unblocked_tasks(session, &done_ids);
        prune_stale_task_wakes(session, &done_ids);

        let agent_key_value = agent_key(agent_name);
        let Some(agent) = session.agents.get(&agent_key_value) else {
            return true;
        };
        if matches!(
            agent.status,
            TeamAgentStatus::Stopped | TeamAgentStatus::Error
        ) {
            return true;
        }

        if session
            .agents
            .values()
            .any(|agent| agent.status == TeamAgentStatus::Running)
        {
            return false;
        }
        if session.queued_messages.iter().any(|message| {
            agent_key(&message.to) == agent_key_value && queued_message_wakes_agent(message)
        }) {
            return false;
        }
        if agent_has_runnable_task(session, &agent_key_value, &done_ids) {
            return false;
        }
        task_wake_ids_for_agent(session, &agent_key_value).is_empty()
    }
}
