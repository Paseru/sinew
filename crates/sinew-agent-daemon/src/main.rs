use std::fs;
use std::path::PathBuf;
use anyhow::{Context, Result};
use tracing::{info, error};

mod protocol;

#[cfg(windows)]
use tokio::net::windows::named_pipe::{ServerOptions, NamedPipeServer};
#[cfg(windows)]
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

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
            
            tokio::spawn(async move {
                if let Err(e) = handle_client(server).await {
                    error!("Error handling Named Pipe client: {:?}", e);
                }
            });
        }
    }

    #[cfg(not(windows))]
    {
        info!("Daemon placeholder on Unix (no named pipe)");
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }

    Ok(())
}

#[cfg(windows)]
async fn handle_client(stream: NamedPipeServer) -> Result<()> {
    let (reader, mut writer) = tokio::io::split(stream);
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
            protocol::DaemonRequest::StartTurn { conversation_id, .. } => {
                info!("Start turn requested for: {}", conversation_id);
                // Placeholder response
                let resp = protocol::DaemonResponse::TurnStarted {
                    conversation_id: conversation_id.clone(),
                };
                let mut resp_str = serde_json::to_string(&resp)?;
                resp_str.push('\n');
                writer.write_all(resp_str.as_bytes()).await?;
            }
        }
    }
    Ok(())
}
