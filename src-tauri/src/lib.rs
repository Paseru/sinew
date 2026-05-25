use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex as StdMutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
#[cfg(target_os = "macos")]
use objc2::{
    ffi::class_addMethod,
    rc::Retained,
    runtime::{AnyClass, AnyObject, Imp, Sel},
    MainThreadMarker,
};
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem};
#[cfg(target_os = "macos")]
use objc2_foundation::NSString;
use portable_pty::{native_pty_system, Child, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use claakecode_anthropic::{
    delete_default_auth as delete_default_anthropic_auth,
    exchange_oauth_code as exchange_anthropic_oauth_code, generate_pkce as generate_anthropic_pkce,
    generate_state as generate_anthropic_state,
    load_default_auth_status as load_default_anthropic_auth_status,
    oauth_authorize_url as anthropic_oauth_authorize_url, AnthropicAuthStatus, AnthropicProvider,
    PkceCodes as AnthropicPkceCodes, MODEL_ID as ANTHROPIC_MODEL_ID,
};
use claakecode_app::{
    checkpoint_from_snapshots, clean_context_descriptor, compact_conversation_history,
    copy_workspace_entries, create_workspace_directory,
    create_workspace_file, delete_workspace_entry, import_workspace_paths, list_installed_skills,
    list_workspace_entries, list_workspace_files, normalize_workspace_root, probe_mcp_servers,
    read_external_file, read_workspace_file, rename_workspace_entry, resolve_terminal_path,
    restore_turn_checkpoints, restore_workspace_deleted_entries, run_turn, search_workspace_files,
    shell_system_prompt, snapshot_workspace_for_checkpoint, subagent_system_prompt,
    system_prompt_for_mode_with_plan_prompt, system_prompt_with_todo, todo_list_from_history,
    tool_settings_view, trash_workspace_entry, write_workspace_file, AgentEvent, AgentMode,
    AppStore, BashTool, ConversationEvent, ConversationSummary, CreateImageTool,
    DatabaseActivityEntry, DatabaseConnectionStatus, DatabaseConnectionTestResult,
    DatabaseSettings, DatabaseSourceConfig, DatabaseTool, EditFileTool, GlobTool,
    GoalWorkflowState, GrepTool, ImportedEntry, InstalledSkill, McpSettings,
    McpToolRegistry, ModeModelSettings, OpenRouterModelRecord, PlanArtifactState,
    PlanWorkflowState, QuestionTool, ReadTool, SavedConversation, SkillSettings, SkillTool,
    SubAgentConfig, SubAgentSettings, SubAgentTool, TeamRuntime, TeamTool, TerminalPathResolution,
    ToDoListTool, TodoListState, ToolSettings, ToolSettingsView, TurnCancel, TurnContext,
    WebFetchTool, WebSearchTool, WorkspaceBootstrap, WorkspaceCopyOperation, WorkspaceDeletedEntry,
    WorkspaceFileChangeEvent, WorkspaceSearchResult, WriteFileTool,
    test_database_source_connection,
};
use claakecode_core::{
    ChatMessage, Effort, ModelCapabilities, ModelRef, Part, Provider, ProviderRequest, Role,
    ServiceTier, ToolDescriptor,
};
use claakecode_google::{
    delete_default_auth as delete_default_google_auth,
    exchange_oauth_code as exchange_google_oauth_code, generate_pkce as generate_google_pkce,
    generate_state as generate_google_state,
    load_default_auth_status as load_default_google_auth_status,
    oauth_authorize_url as google_oauth_authorize_url,
    purge_legacy_oauth_if_needed as purge_legacy_google_oauth, GoogleAuthStatus, GoogleProvider,
    PkceCodes as GooglePkceCodes, MODEL_ID as GOOGLE_MODEL_ID,
};
use claakecode_kimi::{
    delete_default_auth as delete_default_kimi_auth, generate_state as generate_kimi_state,
    load_default_auth_status as load_default_kimi_auth_status,
    request_device_authorization as request_kimi_device_authorization,
    wait_for_device_token as wait_for_kimi_device_token,
    DeviceAuthorization as KimiDeviceAuthorization, KimiAuthStatus, KimiProvider,
    MODEL_ID as KIMI_MODEL_ID,
};
use claakecode_openai::{
    delete_default_auth, exchange_oauth_code, generate_pkce, generate_state,
    load_default_auth_status, oauth_authorize_url, OpenAiAuthStatus, OpenAiProvider, PkceCodes,
    MODEL_ID as OPENAI_MODEL_ID,
};
use claakecode_openrouter::{
    delete_default_auth as delete_default_openrouter_auth,
    fetch_model_catalog as fetch_openrouter_model_catalog,
    load_default_api_key as load_default_openrouter_api_key,
    load_default_auth_status as load_default_openrouter_auth_status,
    save_default_api_key as save_default_openrouter_api_key,
    touch_default_auth_validation as touch_default_openrouter_auth_validation,
    validate_api_key as validate_openrouter_api_key_remote, OpenRouterAuthStatus,
    OpenRouterCatalogModel, OpenRouterProvider, PROVIDER_ID as OPENROUTER_PROVIDER_ID,
};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc, Mutex, Notify, RwLock},
};

mod context;
mod conversations;
mod git;
mod models;
mod platform;
mod providers;
mod state;
mod swarm;
mod terminal;
#[cfg(test)]
mod tests;
mod turns;
mod updater;
mod workflow;
mod workspace;

use context::*;
use models::*;
use platform::*;
use providers::*;
use state::*;
use swarm::*;
use turns::*;
use workflow::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .try_init();

    let store = AppStore::open_default().expect("unable to open app store");
    let openrouter_models = store.load_openrouter_models().unwrap_or_default();
    let mut providers: HashMap<String, Arc<dyn Provider>> = HashMap::new();
    if let Ok(provider) = AnthropicProvider::from_default_sources() {
        providers.insert("anthropic".into(), Arc::new(provider) as Arc<dyn Provider>);
    }
    if let Ok(provider) = OpenAiProvider::from_default_sources() {
        providers.insert("openai".into(), Arc::new(provider) as Arc<dyn Provider>);
    }
    if let Ok(provider) = GoogleProvider::from_default_sources() {
        providers.insert("google".into(), Arc::new(provider) as Arc<dyn Provider>);
    }
    if let Ok(provider) = KimiProvider::from_default_sources() {
        providers.insert("kimi".into(), Arc::new(provider) as Arc<dyn Provider>);
    }
    if let Ok(provider) =
        OpenRouterProvider::from_default_sources(openrouter_capabilities(&openrouter_models))
    {
        providers.insert(
            OPENROUTER_PROVIDER_ID.into(),
            Arc::new(provider) as Arc<dyn Provider>,
        );
    }

    let default_model = if providers.contains_key("anthropic") {
        ModelRef::new("anthropic", ANTHROPIC_MODEL_ID).with_effort(Effort::Max)
    } else if providers.contains_key("openai") {
        ModelRef::new("openai", OPENAI_MODEL_ID).with_effort(Effort::Medium)
    } else if providers.contains_key("kimi") {
        ModelRef::new("kimi", KIMI_MODEL_ID).with_effort(Effort::High)
    } else if providers.contains_key(OPENROUTER_PROVIDER_ID) {
        openrouter_models
            .first()
            .map(default_openrouter_model_ref)
            .unwrap_or_else(|| ModelRef::new("google", GOOGLE_MODEL_ID).with_effort(Effort::Medium))
    } else {
        ModelRef::new("google", GOOGLE_MODEL_ID).with_effort(Effort::Medium)
    };

    let state = DesktopState {
        providers: Arc::new(StdMutex::new(providers)),
        store,
        default_model,
        system_prompt: DEFAULT_SYSTEM_PROMPT.into(),
        max_tool_rounds: 200,
        active_turns: Arc::new(Mutex::new(HashMap::new())),
        active_turn_details: Arc::new(StdMutex::new(HashMap::new())),
        team_runtime: Arc::new(RwLock::new(TeamRuntime::default())),
        file_watchers: Arc::new(Mutex::new(HashMap::new())),
        terminal_sessions: Arc::new(Mutex::new(HashMap::new())),
        openai_login: Arc::new(Mutex::new(None)),
        anthropic_login: Arc::new(Mutex::new(None)),
        google_login: Arc::new(Mutex::new(None)),
        kimi_login: Arc::new(Mutex::new(None)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // Updater plugin is desktop-only (no iOS / Android support upstream).
        .plugin({
            #[cfg(desktop)]
            {
                tauri_plugin_updater::Builder::new().build()
            }
            #[cfg(not(desktop))]
            {
                // No-op plugin so the chain stays uniform on mobile builds.
                tauri::plugin::Builder::new("updater-stub").build()
            }
        })
        .setup(|app| {
            #[cfg(target_os = "windows")]
            let _ = app;

            // One-shot purge of legacy Google OAuth tokens so users coming from
            // pre-0.1.14 builds reconnect against the fixed Antigravity flow.
            match purge_legacy_google_oauth() {
                Ok(true) => {
                    tracing::info!("purged legacy Google OAuth state (forced re-login)");
                }
                Ok(false) => {}
                Err(err) => {
                    tracing::warn!(error = %err, "google auth migration check failed");
                }
            }

            #[cfg(target_os = "macos")]
            {
                install_macos_dock_menu(app.handle());
            }

            #[cfg(not(target_os = "windows"))]
            {
                let handle = app.handle();
                let menu = tauri::menu::Menu::default(handle)?;
                let new_window_item =
                    tauri::menu::MenuItemBuilder::with_id(NEW_WINDOW_MENU_ID, "New Window")
                        .accelerator("CmdOrCtrl+Shift+N")
                        .build(handle)?;
                let file_menu = tauri::menu::SubmenuBuilder::new(handle, "File")
                    .item(&new_window_item)
                    .build()?;
                let terminal_menu = tauri::menu::SubmenuBuilder::new(handle, "Terminal")
                    .text(TERMINAL_OPEN_MENU_ID, "Open Terminal")
                    .build()?;
                menu.append(&file_menu)?;
                menu.append(&terminal_menu)?;
                app.set_menu(menu)?;
            }
            Ok(())
        })
        .on_menu_event(|app, event| {
            if event.id() == NEW_WINDOW_MENU_ID {
                create_new_window_detached(app);
            } else if event.id() == TERMINAL_OPEN_MENU_ID {
                let focused = app
                    .webview_windows()
                    .into_values()
                    .find(|window| window.is_focused().unwrap_or(false));
                if let Some(window) = focused {
                    let _ = window.emit(TERMINAL_OPEN_EVENT_NAME, ());
                } else {
                    let _ = app.emit(TERMINAL_OPEN_EVENT_NAME, ());
                }
            }
        })
        .manage(state)
        .manage(updater::UpdaterState::new())
        .invoke_handler(tauri::generate_handler![
            workspace::open_workspace,
            workspace::open_new_window,
            workspace::reset_window_title,
            workspace::watch_workspace_command,
            workspace::unwatch_workspace_command,
            workspace::list_workspace_entries_command,
            workspace::list_workspace_files_command,
            workspace::search_workspace_files_command,
            workspace::read_workspace_file_command,
            workspace::write_workspace_file_command,
            workspace::create_workspace_file_command,
            workspace::create_workspace_directory_command,
            workspace::rename_workspace_entry_command,
            workspace::delete_workspace_entry_command,
            workspace::trash_workspace_entry_command,
            workspace::restore_workspace_deleted_entries_command,
            workspace::reveal_workspace_entry_command,
            workspace::reveal_absolute_path_command,
            workspace::resolve_terminal_path_command,
            workspace::read_external_file_command,
            workspace::delete_skill_command,
            workspace::create_skill_command,
            workspace::update_skill_command,
            workspace::open_external_url_command,
            workspace::open_path_with_default_app_command,
            workspace::copy_file_to_path_command,
            workspace::copy_workspace_entries_command,
            workspace::import_workspace_paths_command,
            workspace::save_clipboard_image_attachment_command,
            workspace::read_clipboard_file_paths_command,
            conversations::list_conversations,
            conversations::create_conversation,
            conversations::load_conversation,
            conversations::rename_conversation,
            conversations::delete_conversation,
            conversations::set_conversation_mode,
            conversations::set_conversation_model_preference,
            conversations::list_mcp_settings,
            conversations::save_mcp_settings,
            conversations::list_tool_settings,
            conversations::save_tool_settings,
            conversations::list_sub_agent_settings,
            conversations::save_sub_agent_settings,
            conversations::list_database_settings,
            conversations::save_database_settings,
            conversations::test_database_connection,
            conversations::list_database_source_activity,
            conversations::clear_database_source_activity,
            providers::list_configured_model_providers,
            providers::get_openai_provider_status,
            providers::start_openai_oauth_login,
            providers::cancel_openai_oauth_login,
            providers::disconnect_openai_provider,
            providers::get_anthropic_provider_status,
            providers::start_anthropic_oauth_login,
            providers::cancel_anthropic_oauth_login,
            providers::disconnect_anthropic_provider,
            providers::get_google_provider_status,
            providers::start_google_oauth_login,
            providers::cancel_google_oauth_login,
            providers::disconnect_google_provider,
            providers::get_kimi_provider_status,
            providers::start_kimi_oauth_login,
            providers::cancel_kimi_oauth_login,
            providers::disconnect_kimi_provider,
            providers::get_openrouter_provider_status,
            providers::validate_openrouter_api_key,
            providers::disconnect_openrouter_provider,
            providers::list_openrouter_models,
            providers::search_openrouter_models,
            providers::add_openrouter_model,
            providers::remove_openrouter_model,
            conversations::probe_mcp_tools,
            conversations::list_installed_skills_command,
            conversations::save_skill_settings,
            turns::send_message,
            turns::answer_question,
            turns::reject_question,
            turns::compact_conversation,
            turns::list_active_turns,
            turns::replay_active_turn_events,
            context::estimate_context,
            context::estimate_sub_agent_context,
            turns::cancel_turn,
            swarm::stop_agent_swarm_command,
            terminal::run_terminal_command,
            terminal::spawn_terminal,
            terminal::write_terminal,
            terminal::resize_terminal,
            terminal::kill_terminal,
            git::git_repository_snapshot_command,
            git::git_init_command,
            git::git_create_worktree_command,
            git::git_remove_worktree_command,
            git::git_create_branch_command,
            git::git_commit_command,
            git::git_push_command,
            git::git_pull_command,
            git::git_create_pull_request_command,
            updater::updater_check,
            updater::updater_download_and_install,
            updater::updater_restart,
            updater::updater_current_version,
        ])
        .build(tauri::generate_context!())
        .expect("error while building claakecode desktop")
        .run(|app, event| {
            #[cfg(not(target_os = "macos"))]
            let _ = (&app, &event);

            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                if !focus_existing_window(app) {
                    create_new_window_detached(app);
                }
            }
        })
}
