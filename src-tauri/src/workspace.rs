use crate::*;

#[tauri::command]
pub(super) async fn open_workspace(
    state: State<'_, DesktopState>,
    window: tauri::WebviewWindow,
    input: WorkspaceInput,
) -> std::result::Result<WorkspaceBootstrap, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;

    // If Multi-PC sync is enabled, save and push the previous workspace first before switching
    if crate::is_sync_enabled() {
        if let Some(prev_path) = crate::load_last_workspace_path() {
            if prev_path != input.workspace_path {
                crate::run_git_auto_commit_and_push(&prev_path);
            }
        }
    }

    // Save last workspace path for Multi-PC sync
    crate::save_last_workspace_path(&input.workspace_path);

    // If Multi-PC sync is enabled, auto pull the opened workspace
    if crate::is_sync_enabled() {
        crate::run_git_auto_pull(&input.workspace_path);
    }

    let project_id = get_or_create_project_id(&workspace_root)?;
    let git_remote_url = crate::git::get_git_remote_url(&workspace_root);
    let mut bootstrap = state
        .store
        .bootstrap_workspace(&workspace_root, &project_id, git_remote_url.as_deref(), &state.default_model, &state.system_prompt)
        .map_err(error_to_string)?;
    let workspace_id = workspace_root.display().to_string();
    let active_conversation_id = state.active_turn_details.lock().ok().and_then(|active| {
        active
            .values()
            .filter(|record| record.workspace_id == workspace_id)
            .max_by_key(|record| record.started_at_ms)
            .map(|record| record.conversation_id.clone())
    });
    if let Some(conversation_id) = active_conversation_id {
        if let Some(mut active_conversation) = state
            .store
            .load_conversation(&project_id, &conversation_id)
            .map_err(error_to_string)?
        {
            active_conversation.workspace_id = workspace_id;
            bootstrap.active_conversation = active_conversation;
        }
    }
    apply_window_title(&window, &bootstrap.workspace.name);
    sinew_index::warm_workspace_index(&workspace_root);
    Ok(bootstrap)
}

#[tauri::command]
pub(super) async fn open_new_window(app: AppHandle) -> std::result::Result<(), String> {
    create_new_window(&app).map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn get_or_create_sandbox_workspace() -> std::result::Result<String, String> {
    let home =
        crate::platform::home_dir().ok_or_else(|| "Could not find home directory".to_string())?;
    let sandbox_path = home.join(".sinew-sandbox");
    if !sandbox_path.exists() {
        let _ = std::fs::create_dir_all(&sandbox_path);
        let readme_content = "# Sans dossier\n\nBienvenue dans votre espace temporaire ! Vous pouvez ici utiliser l'agent pour poser des questions générales, écrire du code, et tester des commandes sans polluer vos projets.\n";
        let _ = std::fs::write(sandbox_path.join("README.md"), readme_content);
    }
    Ok(sandbox_path.display().to_string())
}

#[tauri::command]
pub(super) async fn reset_window_title(
    window: tauri::WebviewWindow,
) -> std::result::Result<(), String> {
    apply_window_title(&window, "");
    Ok(())
}

#[tauri::command]
pub(super) async fn watch_workspace_command(
    app: AppHandle,
    state: State<'_, DesktopState>,
    input: WorkspaceInput,
) -> std::result::Result<(), String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let workspace_id = workspace_root.display().to_string();
    let mut watchers = state.file_watchers.lock().await;
    if watchers.contains_key(&workspace_id) {
        return Ok(());
    }

    let watcher_root = workspace_root.clone();
    let app_for_watcher = app.clone();
    let workspace_id_for_watcher = workspace_id.clone();
    let mut watcher =
        notify::recommended_watcher(move |event: notify::Result<notify::Event>| match event {
            Ok(event) => {
                if !is_workspace_file_event(&event.kind) {
                    return;
                }
                if event.paths.is_empty() {
                    let _ = app_for_watcher.emit(
                        FILE_CHANGE_EVENT_NAME,
                        WorkspaceFileChangeEvent {
                            workspace_path: workspace_id_for_watcher.clone(),
                            relative_path: String::new(),
                        },
                    );
                    return;
                }
                for path in event.paths {
                    if should_ignore_workspace_event_path(&watcher_root, &path) {
                        continue;
                    }
                    if let Some(relative_path) = event_relative_path(&watcher_root, &path) {
                        let _ = app_for_watcher.emit(
                            FILE_CHANGE_EVENT_NAME,
                            WorkspaceFileChangeEvent {
                                workspace_path: workspace_id_for_watcher.clone(),
                                relative_path,
                            },
                        );
                    }
                }
            }
            Err(err) => tracing::warn!(%err, "workspace watcher error"),
        })
        .map_err(error_to_string)?;
    watcher
        .watch(&workspace_root, RecursiveMode::Recursive)
        .map_err(error_to_string)?;
    sinew_index::warm_workspace_index(&workspace_root);
    watchers.insert(workspace_id, watcher);
    Ok(())
}

#[tauri::command]
pub(super) async fn unwatch_workspace_command(
    state: State<'_, DesktopState>,
    input: WorkspaceInput,
) -> std::result::Result<bool, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let workspace_id = workspace_root.display().to_string();
    Ok(state
        .file_watchers
        .lock()
        .await
        .remove(&workspace_id)
        .is_some())
}

#[tauri::command]
pub(super) async fn codebase_index_stats_command(
    input: WorkspaceInput,
) -> std::result::Result<sinew_app::CodebaseIndexStatus, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    Ok(codebase_index_status(&workspace_root))
}

#[tauri::command]
pub(super) async fn list_workspace_entries_command(
    input: WorkspaceEntriesInput,
) -> std::result::Result<Vec<sinew_app::WorkspaceEntry>, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    list_workspace_entries(&workspace_root, input.relative_path.as_deref()).map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn list_workspace_files_command(
    input: WorkspaceInput,
) -> std::result::Result<Vec<sinew_app::WorkspaceEntry>, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    list_workspace_files(&workspace_root).map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn search_workspace_files_command(
    input: WorkspaceSearchInput,
) -> std::result::Result<WorkspaceSearchResult, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    search_workspace_files(&workspace_root, &input.query).map_err(error_to_string)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ImportPathsInput {
    pub(super) workspace_path: String,
    pub(super) target_relative_path: Option<String>,
    pub(super) sources: Vec<String>,
}

#[tauri::command]
pub(super) async fn import_workspace_paths_command(
    app: AppHandle,
    input: ImportPathsInput,
) -> std::result::Result<Vec<ImportedEntry>, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let imported = import_workspace_paths(
        &workspace_root,
        input.target_relative_path.as_deref(),
        &input.sources,
    )
    .map_err(error_to_string)?;
    for entry in &imported {
        emit_workspace_file_change(&app, &workspace_root, &entry.relative_path);
    }
    Ok(imported)
}

#[tauri::command]
pub(super) async fn read_workspace_file_command(
    input: WorkspaceFileInput,
) -> std::result::Result<sinew_app::FileDocument, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    read_workspace_file(&workspace_root, &input.relative_path).map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn write_workspace_file_command(
    app: AppHandle,
    input: WriteWorkspaceFileInput,
) -> std::result::Result<sinew_app::FileDocument, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let doc = write_workspace_file(&workspace_root, &input.relative_path, &input.content)
        .map_err(error_to_string)?;
    emit_workspace_file_change(&app, &workspace_root, &doc.relative_path);
    Ok(doc)
}

#[tauri::command]
pub(super) async fn create_workspace_file_command(
    app: AppHandle,
    input: CreateWorkspaceEntryInput,
) -> std::result::Result<sinew_app::WorkspaceEntry, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let entry = create_workspace_file(
        &workspace_root,
        input.target_relative_path.as_deref(),
        &input.name,
    )
    .map_err(error_to_string)?;
    emit_workspace_file_change(&app, &workspace_root, &entry.relative_path);
    Ok(entry)
}

#[tauri::command]
pub(super) async fn create_workspace_directory_command(
    app: AppHandle,
    input: CreateWorkspaceEntryInput,
) -> std::result::Result<sinew_app::WorkspaceEntry, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let entry = create_workspace_directory(
        &workspace_root,
        input.target_relative_path.as_deref(),
        &input.name,
    )
    .map_err(error_to_string)?;
    emit_workspace_file_change(&app, &workspace_root, &entry.relative_path);
    Ok(entry)
}

#[tauri::command]
pub(super) async fn save_clipboard_image_attachment_command(
    input: ClipboardImageInput,
) -> std::result::Result<ClipboardImageAttachment, String> {
    normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let (_, extension) = clipboard_image_type(&input.media_type, input.name.as_deref())
        .ok_or_else(|| "unsupported pasted image type".to_string())?;
    let raw_data = input
        .data
        .split_once(',')
        .map(|(_, data)| data)
        .unwrap_or(input.data.as_str())
        .trim();
    let bytes = BASE64_STANDARD.decode(raw_data).map_err(error_to_string)?;
    if bytes.is_empty() {
        return Err("pasted image is empty".into());
    }
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err("pasted image is too large".into());
    }

    let display_name = clipboard_image_display_name(input.name.as_deref(), extension);
    let stem = Path::new(&display_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("pasted-image");
    let safe_stem = safe_temp_file_stem(stem);
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let file_name = format!("{safe_stem}-{}-{now_ms}.{extension}", std::process::id());
    let dir = std::env::temp_dir().join("sinew-clipboard-attachments");
    fs::create_dir_all(&dir).map_err(error_to_string)?;
    let path = dir.join(file_name);
    fs::write(&path, bytes).map_err(error_to_string)?;

    Ok(ClipboardImageAttachment {
        path: path.display().to_string(),
        name: display_name,
    })
}

#[tauri::command]
pub(super) async fn rename_workspace_entry_command(
    app: AppHandle,
    input: RenameWorkspaceEntryInput,
) -> std::result::Result<sinew_app::WorkspaceEntry, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let entry = rename_workspace_entry(&workspace_root, &input.relative_path, &input.new_name)
        .map_err(error_to_string)?;
    emit_workspace_file_change(&app, &workspace_root, &input.relative_path);
    emit_workspace_file_change(&app, &workspace_root, &entry.relative_path);
    Ok(entry)
}

#[tauri::command]
pub(super) async fn delete_workspace_entry_command(
    app: AppHandle,
    input: WorkspaceFileInput,
) -> std::result::Result<(), String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    delete_workspace_entry(&workspace_root, &input.relative_path).map_err(error_to_string)?;
    emit_workspace_file_change(&app, &workspace_root, &input.relative_path);
    Ok(())
}

#[tauri::command]
pub(super) async fn trash_workspace_entry_command(
    app: AppHandle,
    input: WorkspaceFileInput,
) -> std::result::Result<WorkspaceDeletedEntry, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let deleted =
        trash_workspace_entry(&workspace_root, &input.relative_path).map_err(error_to_string)?;
    emit_workspace_file_change(&app, &workspace_root, &deleted.relative_path);
    Ok(deleted)
}

#[tauri::command]
pub(super) async fn restore_workspace_deleted_entries_command(
    app: AppHandle,
    input: RestoreWorkspaceDeletedEntriesInput,
) -> std::result::Result<Vec<sinew_app::WorkspaceEntry>, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let entries = restore_workspace_deleted_entries(&workspace_root, &input.entries)
        .map_err(error_to_string)?;
    for entry in &entries {
        emit_workspace_file_change(&app, &workspace_root, &entry.relative_path);
    }
    Ok(entries)
}

#[tauri::command]
pub(super) async fn reveal_workspace_entry_command(
    input: WorkspaceFileInput,
) -> std::result::Result<(), String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let path = sinew_app::workspace::resolve_workspace_path(&workspace_root, &input.relative_path)
        .map_err(error_to_string)?;
    reveal_path(&path).map_err(error_to_string)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AbsolutePathInput {
    pub(super) path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SkillPathInput {
    pub(super) workspace_path: String,
    pub(super) path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UpdateSkillContentInput {
    pub(super) workspace_path: String,
    pub(super) path: String,
    pub(super) content: String,
}

#[tauri::command]
pub(super) async fn reveal_absolute_path_command(
    input: AbsolutePathInput,
) -> std::result::Result<(), String> {
    let path = std::path::PathBuf::from(&input.path);
    reveal_path(&path).map_err(error_to_string)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ResolveTerminalPathInput {
    pub(super) workspace_path: String,
    pub(super) raw_path: String,
}

#[tauri::command]
pub(super) async fn resolve_terminal_path_command(
    input: ResolveTerminalPathInput,
) -> std::result::Result<TerminalPathResolution, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    resolve_terminal_path(&workspace_root, &input.raw_path).map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn read_external_file_command(
    app: tauri::AppHandle,
    input: AbsolutePathInput,
) -> std::result::Result<sinew_app::FileDocument, String> {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

    let path = std::path::PathBuf::from(&input.path);

    // Ask user for permission to read external files outside the workspace
    let confirmed = app.dialog()
        .message(format!(
            "Sinew souhaite lire un fichier externe situé en dehors de votre espace de travail.\n\nFichier : {}\n\nAutoriser la lecture de ce fichier ?",
            path.display()
        ))
        .title("Autorisation de lecture de fichier externe")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::YesNo)
        .blocking_show();

    if !confirmed {
        return Err("Lecture du fichier externe refusée par l'utilisateur".to_string());
    }

    read_external_file(&path).map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn delete_skill_command(
    app: AppHandle,
    input: SkillPathInput,
) -> std::result::Result<(), String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let skill_md = PathBuf::from(&input.path);
    let folder = delete_installed_skill(&workspace_root, &skill_md).map_err(error_to_string)?;
    if let Ok(relative) = folder.strip_prefix(&workspace_root) {
        let relative_path = relative.to_string_lossy().to_string();
        emit_workspace_file_change(&app, &workspace_root, &relative_path);
    }
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CreateSkillOutput {
    pub(super) name: String,
    pub(super) skills: Vec<InstalledSkill>,
}

#[tauri::command]
pub(super) async fn create_skill_command(
    state: State<'_, DesktopState>,
    input: WorkspaceInput,
) -> std::result::Result<CreateSkillOutput, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let (name, _path) = create_installed_skill().map_err(error_to_string)?;
    let settings = state.store.load_skill_settings().map_err(error_to_string)?;
    let skills = list_installed_skills(workspace_root, &settings);
    Ok(CreateSkillOutput { name, skills })
}

#[tauri::command]
pub(super) async fn update_skill_content_command(
    app: AppHandle,
    state: State<'_, DesktopState>,
    input: UpdateSkillContentInput,
) -> std::result::Result<CreateSkillOutput, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let skill_md = PathBuf::from(&input.path);
    let written = write_installed_skill(&workspace_root, &skill_md, &input.content)
        .map_err(error_to_string)?;
    if let Ok(relative) = written.strip_prefix(&workspace_root) {
        let relative_path = relative.to_string_lossy().to_string();
        emit_workspace_file_change(&app, &workspace_root, &relative_path);
    }
    let settings = state.store.load_skill_settings().map_err(error_to_string)?;
    let skills = list_installed_skills(&workspace_root, &settings);
    let name = skills
        .iter()
        .find(|skill| skill.absolute_path == written.display().to_string())
        .map(|skill| skill.name.clone())
        .unwrap_or_default();
    Ok(CreateSkillOutput { name, skills })
}

#[tauri::command]
pub(super) async fn open_external_url_command(
    input: OpenExternalUrlInput,
) -> std::result::Result<(), String> {
    open_external_url(&input.url).map_err(error_to_string)
}

#[tauri::command]
pub(super) async fn open_path_with_default_app_command(
    app: tauri::AppHandle,
    input: AbsolutePathInput,
) -> std::result::Result<(), String> {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

    let path = std::path::PathBuf::from(&input.path);

    // Ask user for permission to launch/open an external path with the default application
    let confirmed = app.dialog()
        .message(format!(
            "Sinew souhaite ouvrir le fichier ou dossier suivant avec l'application par défaut de votre système :\n\nChemin : {}\n\nAutoriser l'ouverture ?",
            path.display()
        ))
        .title("Autorisation d'ouverture système")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::YesNo)
        .blocking_show();

    if !confirmed {
        return Err("Ouverture système refusée par l'utilisateur".to_string());
    }

    open_with_default_app(&path).map_err(error_to_string)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CopyFileToPathInput {
    pub(super) source_path: String,
    pub(super) destination_path: String,
}

#[tauri::command]
pub(super) async fn copy_file_to_path_command(
    input: CopyFileToPathInput,
) -> std::result::Result<(), String> {
    let source = std::path::PathBuf::from(&input.source_path);
    let destination = std::path::PathBuf::from(&input.destination_path);
    if !source.exists() {
        return Err("source file does not exist".to_string());
    }
    std::fs::copy(&source, &destination)
        .map(|_| ())
        .map_err(|err| format!("unable to copy file: {err}"))
}

#[tauri::command]
pub(super) async fn copy_workspace_entries_command(
    app: AppHandle,
    input: CopyWorkspaceEntriesInput,
) -> std::result::Result<Vec<sinew_app::WorkspaceEntry>, String> {
    let workspace_root =
        normalize_workspace_root(&input.workspace_path).map_err(error_to_string)?;
    let operation = if input.cut {
        WorkspaceCopyOperation::Move
    } else {
        WorkspaceCopyOperation::Copy
    };
    let entries = copy_workspace_entries(
        &workspace_root,
        input.target_relative_path.as_deref(),
        &input.sources,
        operation,
    )
    .map_err(error_to_string)?;
    for source in &input.sources {
        emit_workspace_file_change(&app, &workspace_root, source);
    }
    for entry in &entries {
        emit_workspace_file_change(&app, &workspace_root, &entry.relative_path);
    }
    Ok(entries)
}

#[tauri::command]
pub(super) async fn read_clipboard_file_paths_command() -> std::result::Result<Vec<String>, String>
{
    tauri::async_runtime::spawn_blocking(read_clipboard_file_paths)
        .await
        .map_err(error_to_string)?
        .map_err(error_to_string)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PushEditorDiagnosticsInput {
    pub diagnostics: Vec<EditorDiagnostic>,
}

#[tauri::command]
pub(super) async fn push_editor_diagnostics(
    state: State<'_, DesktopState>,
    input: PushEditorDiagnosticsInput,
) -> std::result::Result<(), String> {
    let mut store = state
        .editor_diagnostics
        .write()
        .map_err(|err| err.to_string())?;
    store.replace(input.diagnostics);
    Ok(())
}

#[tauri::command]
pub async fn set_semantic_embeddings_enabled(enabled: bool) -> std::result::Result<(), String> {
    if enabled {
        std::env::set_var("SINEW_INDEX_EMBEDDINGS", "1");
    } else {
        std::env::remove_var("SINEW_INDEX_EMBEDDINGS");
    }
    Ok(())
}

pub(crate) fn get_or_create_project_id(workspace_root: &std::path::Path) -> std::result::Result<String, String> {
    let sinew_dir = workspace_root.join(".sinew");
    let id_file = sinew_dir.join("project_id.txt");

    if id_file.exists() {
        if let Ok(id) = std::fs::read_to_string(&id_file) {
            let trimmed = id.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }
    }

    // Generate a new UUID
    let new_id = uuid::Uuid::new_v4().to_string();
    let _ = std::fs::create_dir_all(&sinew_dir);
    if let Err(e) = std::fs::write(&id_file, &new_id) {
        return Err(format!("Impossible d'écrire l'identifiant du projet : {}", e));
    }
    Ok(new_id)
}

pub(crate) fn resolve_project_id_str(workspace_path_str: &str) -> String {
    let path = std::path::Path::new(workspace_path_str);
    get_or_create_project_id(path).unwrap_or_else(|_| workspace_path_str.to_string())
}
