use std::fs;
use std::path::PathBuf;
use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{Context, Result};
use tracing::{info, error};

mod protocol;

#[cfg(windows)]
use tokio::net::windows::named_pipe::{ServerOptions, NamedPipeServer};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(not(windows))]
use tokio::net::{TcpListener, TcpStream};

fn load_all_providers() -> HashMap<String, Arc<dyn sinew_core::Provider>> {
    let mut providers: HashMap<String, Arc<dyn sinew_core::Provider>> = HashMap::new();
    
    // Load OpenAI
    if let Ok(files) = sinew_openai::all_auth_files() {
        for (key, path) in files {
            if let Ok(p) = sinew_openai::OpenAiProvider::from_file(&path) {
                providers.insert(key, Arc::new(p));
            }
        }
    }
    
    // Load Anthropic
    if let Ok(p) = sinew_anthropic::AnthropicProvider::from_default_sources() {
        let arc = Arc::new(p);
        providers.insert("anthropic".to_string(), arc.clone());
        providers.insert("anthropic:1".to_string(), arc);
    }

    // Load Google
    if let Ok(p) = sinew_google::GoogleProvider::from_default_sources() {
        let arc = Arc::new(p);
        providers.insert("google".to_string(), arc.clone());
        providers.insert("google:1".to_string(), arc);
    }

    // Load Kimi
    if let Ok(p) = sinew_kimi::KimiProvider::from_default_sources() {
        let arc = Arc::new(p);
        providers.insert("kimi".to_string(), arc.clone());
        providers.insert("kimi:1".to_string(), arc);
    }

    // Load OpenRouter
    if let Ok(p) = sinew_openrouter::OpenRouterProvider::from_default_sources(Vec::new()) {
        let arc = Arc::new(p);
        providers.insert("openrouter".to_string(), arc.clone());
        providers.insert("openrouter:1".to_string(), arc);
    }

    // Load DeepSeek
    if let Ok(p) = sinew_deepseek::DeepSeekProvider::from_default_sources() {
        let arc = Arc::new(p);
        providers.insert("deepseek".to_string(), arc.clone());
        providers.insert("deepseek:1".to_string(), arc);
    }

    // Load Cursor
    if let Ok(p) = sinew_cursor::CursorProvider::from_default_sources() {
        let arc = Arc::new(p);
        providers.insert("cursor:1".to_string(), arc.clone());
        providers.insert("cursor".to_string(), arc);
    }

    providers
}

#[allow(unreachable_code)]
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    info!("Starting Sinew Agent Daemon...");

    // Write PID file
    let local_app_data = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    let daemon_dir = local_app_data.join("Sinew").join("Daemon");
    fs::create_dir_all(&daemon_dir).context("Failed to create daemon state directory")?;
    let pid_path = daemon_dir.join("daemon.pid");
    fs::write(&pid_path, std::process::id().to_string().as_bytes())
        .context("Failed to write PID file")?;
    info!("PID written to {}", pid_path.display());

    let providers = Arc::new(tokio::sync::RwLock::new(load_all_providers()));

    // Windows Named Pipe setup
    #[cfg(windows)]
    {
        let pipe_name = r"\\.\pipe\sinew-agent-ipc";
        info!("Listening on Windows Named Pipe: {}", pipe_name);
        
        let mut first = true;
        loop {
            let server = ServerOptions::new()
                .first_pipe_instance(first)
                .create(pipe_name)?;
            first = false;
                
            // Wait for client connection
            server.connect().await?;
            info!("Client connected via Named Pipe!");
            
            let providers_clone = providers.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_client(server, providers_clone).await {
                    error!("Error handling Named Pipe client: {:?}", e);
                }
            });
        }
    }

    #[cfg(not(windows))]
    {
        let addr = "127.0.0.1:47990";
        info!("Listening on TCP: {}", addr);
        let listener = TcpListener::bind(addr).await.context("Failed to bind TCP listener")?;

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    info!("Client connected via TCP!");
                    let providers_clone = providers.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_client_tcp(stream, providers_clone).await {
                            error!("Error handling TCP client: {:?}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting TCP connection: {}", e);
                }
            }
        }
    }

    Ok(())
}

#[cfg(windows)]
async fn handle_client(
    stream: NamedPipeServer,
    providers: Arc<tokio::sync::RwLock<HashMap<String, Arc<dyn sinew_core::Provider>>>>,
) -> Result<()> {
    let (reader, writer) = tokio::io::split(stream);
    handle_client_inner(reader, writer, providers).await
}

#[cfg(not(windows))]
async fn handle_client_tcp(
    stream: TcpStream,
    providers: Arc<tokio::sync::RwLock<HashMap<String, Arc<dyn sinew_core::Provider>>>>,
) -> Result<()> {
    let (reader, writer) = tokio::io::split(stream);
    handle_client_inner(reader, writer, providers).await
}

async fn handle_client_inner<R, W>(
    reader: R,
    mut writer: W,
    providers: Arc<tokio::sync::RwLock<HashMap<String, Arc<dyn sinew_core::Provider>>>>,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            break; // Connection closed
        }
        
        let request: protocol::DaemonRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                let err_resp = protocol::DaemonResponse::Error {
                    message: format!("Invalid JSON request: {}", e),
                };
                let mut resp_str = serde_json::to_string(&err_resp)?;
                resp_str.push('\n');
                writer.write_all(resp_str.as_bytes()).await?;
                continue;
            }
        };
        
        // Process request
        info!("Received request: {:?}", request);
        match request {
            protocol::DaemonRequest::GetStatus => {
                let resp = protocol::DaemonResponse::Status {
                    is_busy: false,
                    active_conversation_id: None,
                };
                let mut resp_str = serde_json::to_string(&resp)?;
                resp_str.push('\n');
                writer.write_all(resp_str.as_bytes()).await?;
            }
            protocol::DaemonRequest::CancelTurn { conversation_id } => {
                info!("Cancel turn requested for: {}", conversation_id);
            }
            protocol::DaemonRequest::ListEntries { workspace_path, relative_path } => {
                info!("ListEntries requested for: {} {:?}", workspace_path, relative_path);
                let root = PathBuf::from(&workspace_path);
                let entries = sinew_app::workspace::list_workspace_entries(&root, relative_path.as_deref()).unwrap_or_default();
                let resp = protocol::DaemonResponse::EntriesList {
                    entries: serde_json::to_value(&entries).unwrap_or(serde_json::Value::Null),
                };
                let mut resp_str = serde_json::to_string(&resp)?;
                resp_str.push('\n');
                writer.write_all(resp_str.as_bytes()).await?;
            }
            protocol::DaemonRequest::ListAllFiles { workspace_path } => {
                info!("ListAllFiles requested for: {}", workspace_path);
                let root = PathBuf::from(&workspace_path);
                let entries = sinew_app::workspace::list_workspace_files(&root).unwrap_or_default();
                let resp = protocol::DaemonResponse::EntriesList {
                    entries: serde_json::to_value(&entries).unwrap_or(serde_json::Value::Null),
                };
                let mut resp_str = serde_json::to_string(&resp)?;
                resp_str.push('\n');
                writer.write_all(resp_str.as_bytes()).await?;
            }
            protocol::DaemonRequest::ReadFile { workspace_path, relative_path } => {
                info!("ReadFile requested for: {}/{}", workspace_path, relative_path);
                let root = PathBuf::from(&workspace_path);
                match sinew_app::workspace::read_workspace_file(&root, &relative_path) {
                    Ok(doc) => {
                        let resp = protocol::DaemonResponse::FileContent {
                            content: doc.content.unwrap_or_default(),
                        };
                        let mut resp_str = serde_json::to_string(&resp)?;
                        resp_str.push('\n');
                        writer.write_all(resp_str.as_bytes()).await?;
                    }
                    Err(e) => {
                        let resp = protocol::DaemonResponse::Error {
                            message: format!("Failed to read file: {}", e),
                        };
                        let mut resp_str = serde_json::to_string(&resp)?;
                        resp_str.push('\n');
                        writer.write_all(resp_str.as_bytes()).await?;
                    }
                }
            }
            protocol::DaemonRequest::WriteFile { workspace_path, relative_path, content } => {
                info!("WriteFile requested for: {}/{}", workspace_path, relative_path);
                let root = PathBuf::from(&workspace_path);
                match sinew_app::workspace::write_workspace_file(&root, &relative_path, &content) {
                    Ok(_) => {
                        let resp = protocol::DaemonResponse::FileWritten;
                        let mut resp_str = serde_json::to_string(&resp)?;
                        resp_str.push('\n');
                        writer.write_all(resp_str.as_bytes()).await?;
                    }
                    Err(e) => {
                        let resp = protocol::DaemonResponse::Error {
                            message: format!("Failed to write file: {}", e),
                        };
                        let mut resp_str = serde_json::to_string(&resp)?;
                        resp_str.push('\n');
                        writer.write_all(resp_str.as_bytes()).await?;
                    }
                }
            }
            protocol::DaemonRequest::StartTurn {
                conversation_id,
                workspace_path,
                system_prompt,
                model_name,
                provider: provider_id,
                history,
                todo_list,
                goal_workflow,
                mcp_settings,
                tool_settings,
                skill_settings,
                sub_agent_settings,
            } => {
                info!("Start turn requested for: {}", conversation_id);
                
                // Get provider from registry
                let provider_instance = {
                    let lock = providers.read().await;
                    lock.get(&provider_id).cloned()
                };
                
                let provider_instance = match provider_instance {
                    Some(p) => p,
                    None => {
                        let err_resp = protocol::DaemonResponse::Error {
                            message: format!("provider `{}` is not configured or missing credentials", provider_id),
                        };
                        let mut resp_str = serde_json::to_string(&err_resp)?;
                        resp_str.push('\n');
                        writer.write_all(resp_str.as_bytes()).await?;
                        continue;
                    }
                };
                
                // Write TurnStarted response
                let resp = protocol::DaemonResponse::TurnStarted {
                    conversation_id: conversation_id.clone(),
                };
                let mut resp_str = serde_json::to_string(&resp)?;
                resp_str.push('\n');
                writer.write_all(resp_str.as_bytes()).await?;
                
                // Prepare TurnContext parameters
                let workspace_root = PathBuf::from(&workspace_path);
                let model = sinew_core::ModelRef {
                    provider: provider_id.clone(),
                    name: model_name.clone(),
                    effort: None,
                };
                
                let tool_settings = tool_settings
                    .and_then(|v| serde_json::from_value::<sinew_app::store::ToolSettings>(v).ok())
                    .unwrap_or_default();
                let mcp_settings = mcp_settings
                    .and_then(|v| serde_json::from_value::<sinew_app::mcp::McpSettings>(v).ok())
                    .unwrap_or_default();
                let skill_settings = skill_settings
                    .and_then(|v| serde_json::from_value::<sinew_app::skill::SkillSettings>(v).ok())
                    .unwrap_or_default();
                let _sub_agent_settings = sub_agent_settings
                    .and_then(|v| serde_json::from_value::<sinew_app::subagent::SubAgentSettings>(v).ok())
                    .unwrap_or_default();
                    
                let history = serde_json::from_value::<Vec<sinew_core::ChatMessage>>(history).unwrap_or_default();
                let todo_list = serde_json::from_value::<sinew_app::todo::TodoListState>(todo_list).unwrap_or_default();
                let goal_workflow = serde_json::from_value::<sinew_app::store::GoalWorkflowState>(goal_workflow).unwrap_or_default();

                // Setup tools
                let bash = Arc::new(sinew_app::bash::BashTool::new(workspace_root.clone()));
                let glob = Arc::new(sinew_app::glob::GlobTool::new(workspace_root.clone()));
                let list_dir = Arc::new(sinew_app::list_dir::ListDirTool::new(workspace_root.clone()));
                let grep = Arc::new(sinew_app::grep::GrepTool::new(workspace_root.clone()));
                let codebase_search = Arc::new(sinew_app::codebase_search::CodebaseSearchTool::new(workspace_root.clone()));
                let check_sota = Arc::new(sinew_app::check_sota::CheckSotaTool::new());
                let computer_use = Arc::new(sinew_app::ComputerUseTool::new());
                let read = Arc::new(sinew_app::read::ReadTool::new(workspace_root.clone()));
                let edit_file = Arc::new(sinew_app::edit::EditFileTool::new(workspace_root.clone()));
                let write_file = Arc::new(sinew_app::write::WriteFileTool::new(workspace_root.clone()));
                let delete_file = Arc::new(sinew_app::delete_file::DeleteFileTool::new(workspace_root.clone()));
                
                let editor_diagnostics = sinew_app::editor_diagnostics::new_editor_diagnostics_store();
                let read_lints = Arc::new(sinew_app::read_lints::ReadLintsTool::new(workspace_root.clone(), editor_diagnostics.clone()));
                
                let create_image = Arc::new(sinew_app::image::CreateImageTool::with_settings(
                    workspace_root.clone(),
                    tool_settings.image_provider,
                    tool_settings.openai_image_use_subscription,
                    tool_settings.gemini_image_use_subscription,
                    Some(tool_settings.openai_image_model.clone()),
                    Some(tool_settings.gemini_image_model.clone()),
                    tool_settings.openai_image_api_key(),
                    tool_settings.nano_banana_api_key(),
                ));
                
                let todo_list_tool = Some(Arc::new(sinew_app::todo::ToDoListTool::new()));
                let question = Some(Arc::new(sinew_app::question::QuestionTool::new()));
                let web_search = Arc::new(sinew_app::web::WebSearchTool::with_settings(
                    tool_settings.web_search_provider,
                    tool_settings.linkup_api_key(),
                ));
                let web_fetch = Arc::new(sinew_app::web::WebFetchTool::new());
                let skill = Arc::new(sinew_app::skill::SkillTool::with_settings(
                    workspace_root.clone(),
                    skill_settings.clone(),
                ));
                let mcp_registry = Arc::new(sinew_app::mcp::McpToolRegistry::new(mcp_settings.clone()));
                
                let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
                let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
                let cancel = sinew_app::TurnCancel::with_steering(cmd_tx, tokio::sync::mpsc::unbounded_channel().0);
                
                let context = sinew_app::TurnContext {
                    provider: provider_instance,
                    workspace_root,
                    model,
                    cache_key: Some(conversation_id.clone()),
                    cache_stable_message_count: history.len(),
                    service_tier: None,
                    auto_compact: true,
                    mode: sinew_app::AgentMode::Act,
                    stop_questions: false,
                    system_prompt,
                    history,
                    todo_list,
                    goal_workflow,
                    bash,
                    glob,
                    list_dir,
                    grep,
                    codebase_search,
                    check_sota,
                    computer_use,
                    read,
                    edit_file,
                    write_file,
                    delete_file,
                    read_lints,
                    create_image,
                    todo_list_tool,
                    question,
                    web_search,
                    web_fetch,
                    skill,
                    mcp: mcp_registry,
                    subagents: None,
                    teams: None,
                    tool_settings,
                    event_scope: None,
                    max_tool_rounds: 30,
                    event_tx,
                    cancel,
                    cmd_rx,
                    steering_rx: None,
                };
                
                // Spawn the turn loop in the daemon
                let conversation_id_clone = conversation_id.clone();
                let event_task = tokio::spawn(async move {
                    sinew_app::run_turn(context).await
                });
                
                // Forward events back to the client
                let _writer_providers = providers.clone();
                let mut writer_clone = writer;
                let conversation_id_events = conversation_id.clone();
                
                let event_forwarder = tokio::spawn(async move {
                    while let Some(event) = event_rx.recv().await {
                        let event_val = serde_json::to_value(&event).unwrap_or(serde_json::Value::Null);
                        let resp = protocol::DaemonResponse::Event {
                            conversation_id: conversation_id_events.clone(),
                            event: event_val,
                        };
                        if let Ok(mut resp_str) = serde_json::to_string(&resp) {
                            resp_str.push('\n');
                            if writer_clone.write_all(resp_str.as_bytes()).await.is_err() {
                                break;
                            }
                        }
                    }
                    writer_clone
                });
                
                // Wait for the turn loop to finish
                let turn_result = event_task.await;
                let mut writer_recovered = event_forwarder.await?;
                
                match turn_result {
                    Ok(output) => {
                        let output_val = serde_json::json!({
                            "history": output.history,
                            "todo_list": output.todo_list,
                            "goal_workflow": output.goal_workflow,
                            "interrupted": output.interrupted,
                            "compacted": output.compacted,
                        });
                        let resp = protocol::DaemonResponse::TurnFinished {
                            conversation_id: conversation_id_clone,
                            success: true,
                            error: None,
                            output: Some(output_val),
                        };
                        let mut resp_str = serde_json::to_string(&resp)?;
                        resp_str.push('\n');
                        writer_recovered.write_all(resp_str.as_bytes()).await?;
                    }
                    Err(e) => {
                        let resp = protocol::DaemonResponse::TurnFinished {
                            conversation_id: conversation_id_clone,
                            success: false,
                            error: Some(format!("Turn panicked or aborted: {:?}", e)),
                            output: None,
                        };
                        let mut resp_str = serde_json::to_string(&resp)?;
                        resp_str.push('\n');
                        writer_recovered.write_all(resp_str.as_bytes()).await?;
                    }
                }
                
                // Recover the writer back to the loop
                writer = writer_recovered;
            }
        }
    }
    Ok(())
}
