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
use sinew_anthropic::{
    delete_default_auth as delete_default_anthropic_auth,
    exchange_oauth_code as exchange_anthropic_oauth_code, generate_pkce as generate_anthropic_pkce,
    generate_state as generate_anthropic_state,
    load_default_auth_status as load_default_anthropic_auth_status,
    oauth_authorize_url as anthropic_oauth_authorize_url, AnthropicAuthStatus, AnthropicProvider,
    PkceCodes as AnthropicPkceCodes, MODEL_ID as ANTHROPIC_MODEL_ID,
};
use sinew_app::{
    checkpoint_from_snapshots, clean_context_descriptor, compact_conversation_history,
    copy_workspace_entries, create_installed_skill, create_workspace_directory,
    create_workspace_file, delete_workspace_entry, import_workspace_paths, list_installed_skills,
    list_workspace_entries, list_workspace_files, normalize_workspace_root, probe_mcp_servers,
    read_external_file, read_workspace_file, rename_workspace_entry, resolve_terminal_path,
    restore_turn_checkpoints, restore_workspace_deleted_entries, run_turn, search_workspace_files,
    shell_system_prompt, snapshot_workspace_for_checkpoint, subagent_system_prompt,
    system_prompt_for_mode_with_plan_prompt, system_prompt_with_todo, todo_list_from_history,
    tool_settings_view, trash_workspace_entry, write_workspace_file, AgentEvent, AgentMode,
    AppStore, BashTool, CheckSotaTool, ConversationEvent, ConversationSummary, CreateImageTool,
    EditFileTool, GlobTool, GoalWorkflowState, GrepTool, ImportedEntry, InstalledSkill,
    McpSettings, McpToolRegistry, ModeModelSettings, OpenRouterModelRecord, PlanArtifactState,
    PlanWorkflowState, QuestionTool, ReadTool, SavedConversation, SkillSettings, SkillTool,
    SubAgentConfig, SubAgentSettings, SubAgentTool, TeamRuntime, TeamTool, TerminalPathResolution,
    ToDoListTool, TodoListState, ToolSettings, ToolSettingsView, TurnCancel, TurnContext,
    WebFetchTool, WebSearchTool, WorkspaceBootstrap, WorkspaceCopyOperation, WorkspaceDeletedEntry,
    WorkspaceFileChangeEvent, WorkspaceSearchResult, WriteFileTool,
};
use sinew_core::{
    ChatMessage, Effort, ModelCapabilities, ModelRef, Part, Provider, ProviderRequest, Role,
    ServiceTier, ToolDescriptor,
};
use sinew_google::{
    delete_default_auth as delete_default_google_auth,
    exchange_oauth_code as exchange_google_oauth_code, generate_pkce as generate_google_pkce,
    generate_state as generate_google_state,
    load_default_auth_status as load_default_google_auth_status,
    oauth_authorize_url as google_oauth_authorize_url,
    purge_legacy_oauth_if_needed as purge_legacy_google_oauth, Credential as GoogleCredential,
    GoogleAuthStatus, GoogleProvider, PkceCodes as GooglePkceCodes, MODEL_ID as GOOGLE_MODEL_ID,
};
use sinew_kimi::{
    delete_default_auth as delete_default_kimi_auth, generate_state as generate_kimi_state,
    load_default_auth_status as load_default_kimi_auth_status,
    request_device_authorization as request_kimi_device_authorization,
    wait_for_device_token as wait_for_kimi_device_token,
    DeviceAuthorization as KimiDeviceAuthorization, KimiAuthStatus, KimiProvider,
    MODEL_ID as KIMI_MODEL_ID,
};
use sinew_openai::{
    all_auth_files, default_auth_path, delete_default_auth, exchange_oauth_code, generate_pkce,
    generate_state, load_auth_status, load_default_auth_status, oauth_authorize_url,
    Credential as OpenAiCredential, OpenAiAuthStatus, OpenAiProvider, PkceCodes,
    MODEL_ID as OPENAI_MODEL_ID,
};
use sinew_openrouter::{
    delete_default_auth as delete_default_openrouter_auth,
    fetch_model_catalog as fetch_openrouter_model_catalog,
    load_default_api_key as load_default_openrouter_api_key,
    load_default_auth_status as load_default_openrouter_auth_status,
    save_default_api_key as save_default_openrouter_api_key,
    touch_default_auth_validation as touch_default_openrouter_auth_validation,
    validate_api_key as validate_openrouter_api_key_remote, OpenRouterAuthStatus,
    OpenRouterCatalogModel, OpenRouterProvider, PROVIDER_ID as OPENROUTER_PROVIDER_ID,
};
use sinew_cursor::{
    create_login_challenge, delete_composer_auth, delete_default_api_auth, load_composer_auth_status,
    load_default_api_auth_status, save_default_api_key as persist_cursor_api_key,
    sync_composer_auth_from_ide, wait_for_oauth_login, CursorApiAuthStatus,
    CursorComposerAuthStatus, CursorLoginChallenge, CursorProvider, CursorIdeIdentity,
    PROVIDER_ID as CURSOR_PROVIDER_ID,
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

struct LogWriter {
    file: Arc<StdMutex<Option<fs::File>>>,
}

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = std::io::stderr().write_all(buf);
        if let Ok(mut guard) = self.file.lock() {
            if let Some(ref mut f) = *guard {
                let _ = f.write_all(buf);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let _ = std::io::stderr().flush();
        if let Ok(mut guard) = self.file.lock() {
            if let Some(ref mut f) = *guard {
                let _ = f.flush();
            }
        }
        Ok(())
    }
}

struct MakeLogWriter {
    file: Arc<StdMutex<Option<fs::File>>>,
}

impl<'a> tracing_subscriber::fmt::writer::MakeWriter<'a> for MakeLogWriter {
    type Writer = LogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LogWriter {
            file: self.file.clone(),
        }
    }
}

fn merge_databases(
    local_path: &std::path::Path,
    onedrive_path: &std::path::Path,
) -> Result<(), rusqlite::Error> {
    #[cfg(target_os = "windows")]
    {
        let conn = rusqlite::Connection::open(local_path)?;
        let onedrive_str = onedrive_path.to_string_lossy();

        // Attach OneDrive database
        conn.execute(
            &format!("ATTACH DATABASE '{}' AS onedrive", onedrive_str),
            [],
        )?;

        // Ensure tombstones table exists in both main and onedrive to prevent errors
        let _ = conn.execute(
            "create table if not exists main.tombstones (
                id text primary key,
                deleted_at_ms integer not null
            )",
            [],
        );
        let _ = conn.execute(
            "create table if not exists onedrive.tombstones (
                id text primary key,
                deleted_at_ms integer not null
            )",
            [],
        );

        // Enable foreign keys
        let _ = conn.execute("PRAGMA foreign_keys = ON", []);

        // 1. Merge tombstones
        let _ = conn.execute(
            "INSERT OR REPLACE INTO main.tombstones SELECT * FROM onedrive.tombstones",
            [],
        );

        // 2. Delete conversations/messages that are in tombstones
        let _ = conn.execute(
            "DELETE FROM main.conversations WHERE id IN (SELECT id FROM main.tombstones)",
            [],
        );
        let _ = conn.execute(
            "DELETE FROM main.messages WHERE conversation_id IN (SELECT id FROM main.tombstones)",
            [],
        );
        let _ = conn.execute(
            "DELETE FROM main.turn_checkpoints WHERE conversation_id IN (SELECT id FROM main.tombstones)",
            [],
        );

        // 3. Merge conversations (excluding those with tombstones)
        let _ = conn.execute(
            "INSERT OR IGNORE INTO main.conversations \
             SELECT * FROM onedrive.conversations \
             WHERE id NOT IN (SELECT id FROM main.tombstones)",
            [],
        );
        let _ = conn.execute(
            "INSERT OR REPLACE INTO main.conversations \
             SELECT * FROM onedrive.conversations AS o \
             WHERE EXISTS ( \
                 SELECT 1 FROM main.conversations AS m \
                 WHERE m.id = o.id AND o.updated_at_ms > m.updated_at_ms \
             ) AND o.id NOT IN (SELECT id FROM main.tombstones)",
            [],
        );

        // 4. Merge messages (excluding those with tombstones)
        let _ = conn.execute(
            "INSERT OR IGNORE INTO main.messages \
             SELECT * FROM onedrive.messages \
             WHERE conversation_id NOT IN (SELECT id FROM main.tombstones)",
            [],
        );
        let _ = conn.execute(
            "INSERT OR REPLACE INTO main.messages \
             SELECT * FROM onedrive.messages AS o \
             WHERE EXISTS ( \
                 SELECT 1 FROM main.conversations AS mc \
                 JOIN onedrive.conversations AS oc ON mc.id = oc.id \
                 WHERE mc.id = o.conversation_id AND oc.updated_at_ms > mc.updated_at_ms \
             ) AND o.conversation_id NOT IN (SELECT id FROM main.tombstones)",
            [],
        );

        // 5. Merge app_settings
        let _ = conn.execute(
            "INSERT OR IGNORE INTO main.app_settings SELECT * FROM onedrive.app_settings",
            [],
        );
        let _ = conn.execute(
            "INSERT OR REPLACE INTO main.app_settings \
             SELECT * FROM onedrive.app_settings AS o \
             WHERE EXISTS ( \
                 SELECT 1 FROM main.app_settings AS m \
                 WHERE m.key = o.key AND o.updated_at_ms > m.updated_at_ms \
             )",
            [],
        );

        // Detach OneDrive database
        let _ = conn.execute("DETACH DATABASE onedrive", []);
    }
    let _ = (local_path, onedrive_path);
    Ok(())
}

fn sync_onedrive_db_on_startup() {
    #[cfg(target_os = "windows")]
    {
        use std::fs;
        use std::path::PathBuf;

        // 1. Local AppData path
        let localappdata = match std::env::var("LOCALAPPDATA") {
            Ok(val) => val,
            Err(_) => return,
        };

        // Check if Multi-PC sync is enabled
        let sync_enabled_file = PathBuf::from(&localappdata)
            .join("hyrak")
            .join("sinew")
            .join("data")
            .join("multi_pc_enabled.txt");
        if !sync_enabled_file.exists() {
            return;
        }

        let local_db = PathBuf::from(localappdata)
            .join("hyrak")
            .join("sinew")
            .join("data")
            .join("desktop-state.sqlite3");

        // 2. OneDrive path
        let onedrive = std::env::var("ONEDRIVE").unwrap_or_else(|_| {
            std::env::var("USERPROFILE")
                .map(|u| format!("{}\\OneDrive", u))
                .unwrap_or_default()
        });
        if onedrive.is_empty() {
            return;
        }
        let onedrive_db = PathBuf::from(onedrive)
            .join("Documents")
            .join("Sinew")
            .join("desktop-state.sqlite3");

        if onedrive_db.exists() {
            if let Some(parent) = local_db.parent() {
                let _ = fs::create_dir_all(parent);
            }

            if local_db.exists() {
                let backup_path = local_db.with_extension("sqlite3.bak");
                let _ = fs::copy(&local_db, &backup_path);

                // Perform differential merge
                if let Err(e) = merge_databases(&local_db, &onedrive_db) {
                    tracing::error!("Differential merge on startup failed: {:?}", e);
                    // Fallback to direct overwrite if local database is empty or corrupt
                    if let Ok(meta) = fs::metadata(&local_db) {
                        if meta.len() == 0 {
                            let _ = fs::copy(&onedrive_db, &local_db);
                        }
                    }
                }
            } else {
                let _ = fs::copy(&onedrive_db, &local_db);
            }
        }
    }
}

pub(crate) fn backup_onedrive_db_on_exit() {
    #[cfg(target_os = "windows")]
    {
        use std::fs;
        use std::path::PathBuf;

        // 1. Local AppData path
        let localappdata = match std::env::var("LOCALAPPDATA") {
            Ok(val) => val,
            Err(_) => return,
        };

        // Check if Multi-PC sync is enabled
        let sync_enabled_file = PathBuf::from(&localappdata)
            .join("hyrak")
            .join("sinew")
            .join("data")
            .join("multi_pc_enabled.txt");
        if !sync_enabled_file.exists() {
            return;
        }

        let local_db = PathBuf::from(localappdata)
            .join("hyrak")
            .join("sinew")
            .join("data")
            .join("desktop-state.sqlite3");

        // 2. OneDrive path
        let onedrive = std::env::var("ONEDRIVE").unwrap_or_else(|_| {
            std::env::var("USERPROFILE")
                .map(|u| format!("{}\\OneDrive", u))
                .unwrap_or_default()
        });
        if onedrive.is_empty() {
            return;
        }
        let onedrive_db_dir = PathBuf::from(onedrive).join("Documents").join("Sinew");
        let onedrive_db = onedrive_db_dir.join("desktop-state.sqlite3");

        if local_db.exists() {
            let _ = fs::create_dir_all(&onedrive_db_dir);
            if onedrive_db.exists() {
                // Perform differential merge into OneDrive so no data is lost
                if let Err(e) = merge_databases(&onedrive_db, &local_db) {
                    tracing::error!("Differential merge on exit failed: {:?}", e);
                    let _ = fs::copy(&local_db, &onedrive_db);
                }
            } else {
                let _ = fs::copy(&local_db, &onedrive_db);
            }
        }
    }
}

#[tauri::command]
fn is_multi_pc_sync_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
            let file = std::path::PathBuf::from(localappdata)
                .join("hyrak")
                .join("sinew")
                .join("data")
                .join("multi_pc_enabled.txt");
            return file.exists();
        }
    }
    false
}

#[tauri::command]
fn set_multi_pc_sync_enabled(enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
            let dir = std::path::PathBuf::from(localappdata)
                .join("hyrak")
                .join("sinew")
                .join("data");
            let _ = std::fs::create_dir_all(&dir);
            let file = dir.join("multi_pc_enabled.txt");
            if enabled {
                if let Err(e) = std::fs::write(&file, b"1") {
                    return Err(e.to_string());
                }
            } else {
                let _ = std::fs::remove_file(&file);
            }
            return Ok(());
        }
    }
    Err("Not supported on this platform".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    sync_onedrive_db_on_startup();

    let file = directories::ProjectDirs::from("dev", "hyrak", "sinew").and_then(|dirs| {
        let log_dir = dirs.data_local_dir();
        let _ = fs::create_dir_all(log_dir);
        let log_path = log_dir.join("desktop-app.log");
        fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(log_path)
            .ok()
    });

    let make_writer = MakeLogWriter {
        file: Arc::new(StdMutex::new(file)),
    };

    let _ = tracing_subscriber::fmt()
        .with_writer(make_writer)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    let store = AppStore::open_default().expect("unable to open app store");
    let openrouter_models = store.load_openrouter_models().unwrap_or_default();
    let mut providers: HashMap<String, Arc<dyn Provider>> = HashMap::new();
    if let Ok(provider) = AnthropicProvider::from_default_sources() {
        providers.insert("anthropic".into(), Arc::new(provider) as Arc<dyn Provider>);
    }
    if let Ok(files) = all_auth_files() {
        for (key, path) in files {
            if let Ok(provider) = OpenAiProvider::from_file(&path) {
                providers.insert(key, Arc::new(provider) as Arc<dyn Provider>);
            }
        }
    }
    if let Ok(files) = sinew_google::auth::all_auth_files() {
        for (key, path) in files {
            if let Ok(provider) = GoogleProvider::from_file(&path) {
                providers.insert(key, Arc::new(provider) as Arc<dyn Provider>);
            }
        }
    } else if let Ok(provider) = GoogleProvider::from_default_sources() {
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
    if let Ok(provider) = CursorProvider::from_default_sources() {
        providers.insert(
            CURSOR_PROVIDER_ID.into(),
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
        active_turn_inputs: Arc::new(Mutex::new(HashMap::new())),
        active_turn_details: Arc::new(StdMutex::new(HashMap::new())),
        team_runtime: Arc::new(RwLock::new(TeamRuntime::default())),
        file_watchers: Arc::new(Mutex::new(HashMap::new())),
        terminal_sessions: Arc::new(Mutex::new(HashMap::new())),
        openai_login: Arc::new(Mutex::new(None)),
        anthropic_login: Arc::new(Mutex::new(None)),
        google_login: Arc::new(Mutex::new(None)),
        kimi_login: Arc::new(Mutex::new(None)),
        cursor_login: Arc::new(Mutex::new(None)),
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
                install_desktop_menu(app.handle())?;
            }
            Ok(())
        })
        .on_menu_event(|app, event| {
            if event.id() == CLOSE_ACTIVE_TAB_MENU_ID {
                let windows = app.webview_windows();
                let target = windows
                    .values()
                    .find(|window| window.is_focused().unwrap_or(false))
                    .or_else(|| windows.values().next());
                if let Some(window) = target {
                    let _ = window.emit(CLOSE_ACTIVE_TAB_EVENT_NAME, ());
                }
            } else if event.id() == NEW_WINDOW_MENU_ID {
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
            workspace::get_or_create_sandbox_workspace,
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
            workspace::update_skill_content_command,
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
            providers::list_configured_model_providers,
            providers::get_openai_provider_status,
            providers::start_openai_oauth_login,
            providers::cancel_openai_oauth_login,
            providers::disconnect_openai_provider,
            providers::get_all_openai_accounts,
            providers::get_openai_codex_rate_limits,
            providers::disconnect_openai_account,
            providers::save_openai_access_token,
            providers::get_anthropic_provider_status,
            providers::start_anthropic_oauth_login,
            providers::cancel_anthropic_oauth_login,
            providers::disconnect_anthropic_provider,
            providers::get_google_provider_status,
            providers::get_antigravity_quota,
            providers::start_google_oauth_login,
            providers::cancel_google_oauth_login,
            providers::disconnect_google_provider,
            providers::get_all_google_accounts,
            providers::disconnect_google_account,
            providers::get_kimi_provider_status,
            providers::start_kimi_oauth_login,
            providers::cancel_kimi_oauth_login,
            providers::disconnect_kimi_provider,
            providers::get_openrouter_provider_status,
            providers::get_openrouter_key_details,
            providers::validate_openrouter_api_key,
            providers::disconnect_openrouter_provider,
            providers::list_openrouter_models,
            providers::search_openrouter_models,
            providers::add_openrouter_model,
            providers::remove_openrouter_model,
            providers::get_cursor_composer_status,
            providers::start_cursor_oauth_login,
            providers::cancel_cursor_oauth_login,
            providers::sync_cursor_composer_auth,
            providers::disconnect_cursor_composer,
            providers::get_cursor_api_status,
            providers::save_cursor_api_key,
            providers::disconnect_cursor_api,
            providers::get_cursor_usage,
            conversations::probe_mcp_tools,
            conversations::list_installed_skills_command,
            conversations::save_skill_settings,
            turns::send_message,
            turns::steer_turn,
            turns::answer_question,
            turns::reject_question,
            turns::compact_conversation,
            turns::list_active_turns,
            turns::replay_active_turn_events,
            context::estimate_context,
            context::estimate_sub_agent_context,
            turns::cancel_turn,
            turns::check_sota_diagnostics,
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
            git::git_delete_branch_command,
            git::git_rename_branch_command,
            git::git_commit_command,
            git::git_push_command,
            git::git_pull_command,
            git::git_create_pull_request_command,
            updater::updater_check,
            updater::updater_download_and_install,
            updater::updater_restart,
            updater::updater_current_version,
            is_multi_pc_sync_enabled,
            set_multi_pc_sync_enabled,
        ])
        .build(tauri::generate_context!())
        .expect("error while building sinew desktop")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                backup_onedrive_db_on_exit();
            }

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
