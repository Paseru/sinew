use super::*;

impl TeamTool {
    pub fn new(
        scope_id: String,
        workspace_root: PathBuf,
        system_prompt: String,
        providers: HashMap<String, Arc<dyn Provider>>,
        sub_agent_settings: SubAgentSettings,
        mcp_settings: McpSettings,
        tool_settings: ToolSettings,
        skill_settings: SkillSettings,
        database: DatabaseTool,
        default_model: ModelRef,
        max_tool_rounds: usize,
        service_tier: Option<ServiceTier>,
        runtime: Arc<RwLock<TeamRuntime>>,
        cancel: TurnCancel,
    ) -> Self {
        Self {
            scope_id,
            workspace_root,
            system_prompt,
            providers,
            sub_agent_settings: sub_agent_settings.normalized(),
            mcp_settings,
            tool_settings,
            skill_settings,
            database,
            default_model,
            max_tool_rounds,
            service_tier,
            runtime,
            cancel,
            current_agent: None,
        }
    }

    pub(super) fn for_agent(&self, team_name: String, agent_name: String) -> Self {
        let mut next = self.clone();
        next.current_agent = Some(TeamIdentity {
            team_name,
            agent_name,
        });
        next
    }

    pub(super) fn with_cancel(&self, cancel: TurnCancel) -> Self {
        let mut next = self.clone();
        next.cancel = cancel;
        next
    }

    pub async fn current_agent_system_reminder(&self) -> Option<String> {
        let Some(identity) = self.current_agent.as_ref() else {
            let runtime = self.runtime.read().await;
            let scope = runtime.scopes.get(&self.scope_id)?;
            let team_name = scope.active_team.as_deref().or_else(|| {
                if scope.teams.len() == 1 {
                    scope.teams.keys().next().map(String::as_str)
                } else {
                    None
                }
            })?;
            let session = scope.teams.get(team_name)?;
            return Some(render_main_agent_team_system_reminder(session));
        };
        let mut runtime = self.runtime.write().await;
        let session = runtime
            .scopes
            .get_mut(&self.scope_id)
            .and_then(|scope| scope.teams.get_mut(&identity.team_name))?;
        let done_ids = completed_task_ids(session);
        refresh_unblocked_tasks(session, &done_ids);
        Some(render_agent_team_system_reminder(
            session,
            &identity.agent_name,
        ))
    }

    pub async fn drain_current_agent_messages_prompt(&self) -> Option<String> {
        let identity = self.current_agent.as_ref()?;
        let mut runtime = self.runtime.write().await;
        let session = runtime
            .scopes
            .get_mut(&self.scope_id)
            .and_then(|scope| scope.teams.get_mut(&identity.team_name))?;
        let key = agent_key(&identity.agent_name);
        let mut messages = Vec::new();
        let mut index = 0usize;
        while index < session.queued_messages.len() {
            if agent_key(&session.queued_messages[index].to) == key {
                messages.push(session.queued_messages.remove(index));
            } else {
                index += 1;
            }
        }
        if messages.is_empty() {
            return None;
        }
        session.updated_at_ms = now_ms();
        Some(queued_messages_prompt(&messages))
    }

    pub async fn record_current_agent_file_changes(
        &self,
        tool_name: &str,
        file_changes: &[FileChange],
    ) {
        if file_changes.is_empty() {
            return;
        }
        let Some(identity) = self.current_agent.as_ref() else {
            return;
        };
        let now = now_ms();
        let mut runtime = self.runtime.write().await;
        let Some(session) = runtime
            .scopes
            .get_mut(&self.scope_id)
            .and_then(|scope| scope.teams.get_mut(&identity.team_name))
        else {
            return;
        };
        for change in file_changes {
            let (added, removed) = file_change_line_counts(change);
            session.recent_file_changes.push(TeamRecentFileChange {
                agent: identity.agent_name.clone(),
                tool: tool_name.to_string(),
                relative_path: change.relative_path.clone(),
                kind: change.kind,
                added,
                removed,
                created_at_ms: now,
            });
        }
        if session.recent_file_changes.len() > TEAM_RECENT_FILE_CHANGE_LIMIT {
            let drain_count = session.recent_file_changes.len() - TEAM_RECENT_FILE_CHANGE_LIMIT;
            session.recent_file_changes.drain(0..drain_count);
        }
        session.updated_at_ms = now;
    }
}
