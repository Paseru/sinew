use crate::*;

#[tauri::command]
pub(super) async fn list_conversations(
    state: State<'_, DesktopState>,
    input: WorkspaceInput,
) -> std::result::Result<Vec<ConversationSummary>, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let project_id = workspace::get_or_create_project_id(&workspace_root)?;
    let absolute_path = workspace_root.display().to_string();
    let lowercase_path = absolute_path.to_lowercase();
    let _ = state.store.migrate_conversations(&absolute_path, &project_id);
    let _ = state.store.migrate_conversations(&lowercase_path, &project_id);
    let git_remote_url = git::get_git_remote_url(&workspace_root);
    state
        .store
        .list_conversations(&project_id, git_remote_url.as_deref())
        .map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn create_conversation(
    state: State<'_, DesktopState>,
    input: WorkspaceInput,
) -> std::result::Result<WorkspaceBootstrap, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let project_id = workspace::get_or_create_project_id(&workspace_root)?;
    let git_remote_url = git::get_git_remote_url(&workspace_root);
    state
        .store
        .create_conversation(
            &project_id,
            git_remote_url.as_deref(),
            &state.default_model,
            &state.system_prompt,
        )
        .map_err(error_to_string)?;

    std::thread::spawn(|| {
        crate::backup_onedrive_db_on_exit();
    });

    state
        .store
        .bootstrap_workspace(&workspace_root, &project_id, git_remote_url.as_deref(), &state.default_model, &state.system_prompt)
        .map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn load_conversation(
    state: State<'_, DesktopState>,
    input: ConversationInput,
) -> std::result::Result<SavedConversation, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let project_id = workspace::get_or_create_project_id(&workspace_root)?;
    state
        .store
        .load_conversation(
            &project_id,
            &input.conversation_id,
        )
        .map_err(error_to_string)?
        .map(|mut conv| {
            conv.workspace_id = input.workspace_path.clone();
            conv
        })
        .ok_or_else(|| "conversation not found".to_string())
}

#[tauri::command]
pub(super) async fn rename_conversation(
    state: State<'_, DesktopState>,
    input: RenameConversationInput,
) -> std::result::Result<Vec<ConversationSummary>, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let title = input.title.trim();
    if title.is_empty() {
        return Err("title cannot be empty".into());
    }
    let project_id = workspace::get_or_create_project_id(&workspace_root)?;
    state
        .store
        .rename_conversation(&project_id, &input.conversation_id, title)
        .map_err(error_to_string)?;

    std::thread::spawn(|| {
        crate::backup_onedrive_db_on_exit();
    });

    let git_remote_url = git::get_git_remote_url(&workspace_root);
    state
        .store
        .list_conversations(&project_id, git_remote_url.as_deref())
        .map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn delete_conversation(
    state: State<'_, DesktopState>,
    input: ConversationInput,
) -> std::result::Result<WorkspaceBootstrap, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let project_id = workspace::get_or_create_project_id(&workspace_root)?;
    {
        let active_turns = state.active_turns.lock().await;
        if active_turns.contains_key(&input.conversation_id) {
            return Err("a turn is already running for this conversation".into());
        }
    }
    state
        .store
        .delete_conversation(&project_id, &input.conversation_id)
        .map_err(error_to_string)?;

    std::thread::spawn(|| {
        crate::backup_onedrive_db_on_exit();
    });

    let git_remote_url = git::get_git_remote_url(&workspace_root);
    state
        .store
        .bootstrap_workspace(&workspace_root, &project_id, git_remote_url.as_deref(), &state.default_model, &state.system_prompt)
        .map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn set_conversation_mode(
    state: State<'_, DesktopState>,
    input: ConversationModeInput,
) -> std::result::Result<SavedConversation, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let project_id = workspace::get_or_create_project_id(&workspace_root)?;
    {
        let active_turns = state.active_turns.lock().await;
        if active_turns.contains_key(&input.conversation_id) {
            return Err("a turn is already running for this conversation".into());
        }
    }

    let mut conversation = state
        .store
        .load_conversation(&project_id, &input.conversation_id)
        .map_err(error_to_string)?
        .ok_or_else(|| "conversation not found".to_string())?;

    let mode = AgentMode::from(input.mode);
    let current_plan_workflow = std::mem::take(&mut conversation.plan_workflow);
    conversation.plan_workflow = match mode {
        AgentMode::Act => PlanWorkflowState::Idle,
        AgentMode::Plan => match current_plan_workflow {
            PlanWorkflowState::Idle => PlanWorkflowState::PlanningQuestions,
            current => current,
        },
        AgentMode::Goal => PlanWorkflowState::Idle,
    };
    conversation.goal_workflow = match mode {
        AgentMode::Goal => resume_goal_workflow(std::mem::take(&mut conversation.goal_workflow)),
        AgentMode::Act | AgentMode::Plan => {
            pause_goal_workflow(std::mem::take(&mut conversation.goal_workflow))
        }
    };
    conversation.model = conversation.mode_model_settings.get(mode).clone();

    state
        .store
        .save_conversation(&conversation)
        .map_err(error_to_string)?;
    conversation.workspace_id = input.workspace_path.clone(); // Keep absolute path for frontend
    Ok(conversation)
}

#[tauri::command]
pub(super) async fn set_conversation_model_preference(
    state: State<'_, DesktopState>,
    input: ConversationModelPreferenceInput,
) -> std::result::Result<ModeModelSettings, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let project_id = workspace::get_or_create_project_id(&workspace_root)?;
    let conversation_id = input.conversation_id;
    let mode = AgentMode::from(input.mode);

    {
        let active_turns = state.active_turns.lock().await;
        if active_turns.contains_key(&conversation_id) {
            return Err("a turn is already running for this conversation".into());
        }
    }

    let mut conversation = state
        .store
        .load_conversation(&project_id, &conversation_id)
        .map_err(error_to_string)?
        .ok_or_else(|| "conversation not found".to_string())?;
    let selected = model_with_optional_selection(
        conversation.mode_model_settings.get(mode),
        input.model,
        input.thinking,
    );
    let provider = provider_from_registry(&state, &selected.provider)?;
    provider
        .capabilities(&selected)
        .ok_or_else(|| format!("model `{}` is not supported", selected.name))?;

    conversation.mode_model_settings.set(mode, selected.clone());
    if conversation_active_mode(&conversation) == mode {
        conversation.model = selected.clone();
    }

    let mut default_settings = state
        .store
        .load_mode_model_settings(&state.default_model)
        .map_err(error_to_string)?;
    default_settings.set(mode, selected);

    state
        .store
        .save_conversation_and_mode_model_settings(&conversation, &default_settings)
        .map_err(error_to_string)?;
    Ok(conversation.mode_model_settings)
}

#[tauri::command]
pub(super) async fn list_mcp_settings(
    state: State<'_, DesktopState>,
) -> std::result::Result<McpSettings, String> {
    state.store.load_mcp_settings().map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn save_mcp_settings(
    state: State<'_, DesktopState>,
    input: SaveMcpSettingsInput,
) -> std::result::Result<McpSettings, String> {
    state
        .store
        .save_mcp_settings(&input.settings)
        .map_err(error_to_string)?;
    Ok(input.settings)
}

#[tauri::command]
pub(super) async fn list_tool_settings(
    state: State<'_, DesktopState>,
    input: WorkspaceInput,
) -> std::result::Result<ToolSettingsView, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let settings = state.store.load_tool_settings().map_err(error_to_string)?;
    Ok(tool_settings_view(
        &settings,
        &configurable_tool_catalog(&workspace_root),
    ))
}

#[tauri::command]
pub(super) async fn save_tool_settings(
    state: State<'_, DesktopState>,
    input: SaveToolSettingsInput,
) -> std::result::Result<ToolSettingsView, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let catalog = configurable_tool_catalog(&workspace_root);
    let saved = state
        .store
        .save_tool_settings_for_catalog(&input.settings, &catalog)
        .map_err(error_to_string)?;
    Ok(tool_settings_view(&saved, &catalog))
}

#[tauri::command]
pub(super) async fn list_sub_agent_settings(
    state: State<'_, DesktopState>,
) -> std::result::Result<SubAgentSettings, String> {
    state
        .store
        .load_sub_agent_settings()
        .map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn save_sub_agent_settings(
    state: State<'_, DesktopState>,
    input: SaveSubAgentSettingsInput,
) -> std::result::Result<SubAgentSettings, String> {
    for agent in input.settings.agents.iter().filter(|agent| agent.enabled) {
        let provider = provider_from_registry(&state, &agent.model.provider)?;
        provider
            .capabilities(&agent.model)
            .ok_or_else(|| format!("model `{}` is not supported", agent.model.name))?;
    }
    state
        .store
        .save_sub_agent_settings(&input.settings)
        .map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn probe_mcp_tools(
    state: State<'_, DesktopState>,
) -> std::result::Result<Vec<sinew_app::McpServerProbe>, String> {
    let settings = state.store.load_mcp_settings().map_err(error_to_string)?;
    Ok(probe_mcp_servers(&settings).await)
}

#[tauri::command]
pub(super) async fn list_installed_skills_command(
    state: State<'_, DesktopState>,
    input: WorkspaceInput,
) -> std::result::Result<Vec<InstalledSkill>, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let settings = state.store.load_skill_settings().map_err(error_to_string)?;
    Ok(list_installed_skills(workspace_root, &settings))
}

#[tauri::command]
pub(super) async fn save_skill_settings(
    state: State<'_, DesktopState>,
    input: SaveSkillSettingsInput,
) -> std::result::Result<Vec<InstalledSkill>, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let saved = state
        .store
        .save_skill_settings(&input.settings)
        .map_err(error_to_string)?;
    Ok(list_installed_skills(workspace_root, &saved))
}

#[tauri::command]
pub(super) async fn register_chrome_bridge(
    app_handle: tauri::AppHandle,
    workspace_path: String,
) -> std::result::Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        use std::path::PathBuf;
        use tauri::Manager;

        // Try resource path first
        let mut ps_script = None;
        if let Ok(resource_dir) = app_handle.path().resource_dir() {
            let path = resource_dir.join("sinew-chrome-bridge").join("register.ps1");
            if path.exists() {
                ps_script = Some(path);
            }
        }

        // Fallback to workspace path
        if ps_script.is_none() {
            let workspace_root = PathBuf::from(&workspace_path);
            let path = workspace_root.join("sinew-chrome-bridge").join("register.ps1");
            if path.exists() {
                ps_script = Some(path);
            }
        }

        let ps_script = match ps_script {
            Some(path) => path,
            None => return Err("Le script d'enregistrement register.ps1 est introuvable. Veuillez vous assurer que le dossier sinew-chrome-bridge est présent dans vos ressources ou votre espace de travail.".to_string()),
        };

        use std::os::windows::process::CommandExt;
        let mut cmd = Command::new("powershell");
        cmd.creation_flags(0x08000000);
        let output = cmd
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-File")
            .arg(ps_script)
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                if out.status.success() {
                    Ok(stdout)
                } else {
                    Err(format!("Erreur lors de l'exécution du script :\n{}\n{}", stdout, stderr))
                }
            }
            Err(err) => Err(format!("Impossible de lancer PowerShell: {}", err)),
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("L'enregistrement du pont Chrome n'est supporté que sur Windows.".to_string())
    }
}

#[tauri::command]
pub(super) async fn list_other_workspaces_conversations(
    state: State<'_, DesktopState>,
    input: WorkspaceInput,
) -> std::result::Result<Vec<sinew_app::OtherWorkspaceSummary>, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    state
        .store
        .list_other_workspaces(&workspace_root.display().to_string())
        .map_err(error_to_string)
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MigrateConversationsInput {
    pub(super) src_workspace_path: String,
    pub(super) dest_workspace_path: String,
}

#[tauri::command]
pub(super) async fn migrate_conversations_to_current(
    state: State<'_, DesktopState>,
    input: MigrateConversationsInput,
) -> std::result::Result<(), String> {
    let src_root =
        normalize_workspace_root(&input.src_workspace_path).map_err(error_to_string)?;
    let dest_root =
        normalize_workspace_root(&input.dest_workspace_path).map_err(error_to_string)?;
    state
        .store
        .migrate_conversations(
            &src_root.display().to_string(),
            &dest_root.display().to_string(),
        )
        .map_err(error_to_string)?;
        
    std::thread::spawn(|| {
        crate::backup_onedrive_db_on_exit();
    });
    
    Ok(())
}


