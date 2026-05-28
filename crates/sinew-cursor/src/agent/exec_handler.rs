//! Handle `ExecServerMessage` locally (read/ls/write/delete/context/MCP).

use std::path::{Path, PathBuf};

use bytes::Bytes;
use prost_reflect::{DynamicMessage, Value as ProtoValue};
use serde_json::Value as JsonValue;
use sinew_core::Result;

use super::client_proto::encode_exec_client_message;
use super::proto_dynamic::{
    get_message_field, get_string_field, get_u32_field, message_desc, oneof_case, setf,
};
use super::server_decode::decode_mcp_args_from_message;
use super::tools::execute_tool;

const READ_LIMIT: usize = 512 * 1024;
const REJECT: &str = "Tool not available in Sinew Rust agent bridge.";

pub struct ExecContext<'a> {
    pub workspace_root: &'a str,
    pub tools: &'a [JsonValue],
    pub workspace_snapshot: Option<&'a JsonValue>,
}

#[derive(Debug, Clone)]
pub struct PendingToolRequest {
    pub exec_id: String,
    pub exec_msg_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub args: JsonValue,
}

pub enum ExecOutcome {
    Frame(Vec<u8>),
    ToolRequest(PendingToolRequest),
}

fn exec_args(exec: &DynamicMessage, field: &str, type_name: &str) -> Result<DynamicMessage> {
    Ok(match get_message_field(exec, field) {
        Some(m) => m,
        None => DynamicMessage::new(message_desc(type_name)?),
    })
}

pub async fn handle_exec_server_message(
    exec: &DynamicMessage,
    ctx: &ExecContext<'_>,
) -> Result<Option<ExecOutcome>> {
    let exec_id = get_string_field(exec, "exec_id").unwrap_or_default();
    let id = get_u32_field(exec, "id").unwrap_or(0);
    let case = oneof_case(exec).unwrap_or_default();

    match case.as_str() {
        "request_context_args" => {
            let result = build_request_context_result(ctx)?;
            Ok(Some(ExecOutcome::Frame(encode_exec_client_message(
                &exec_id,
                id,
                "request_context_result",
                result,
            )?)))
        }
        "read_args" => {
            let args = exec_args(exec, "read_args", "agent.v1.ReadArgs")?;
            let result = handle_read_args(&args, ctx.workspace_root)?;
            Ok(Some(ExecOutcome::Frame(encode_exec_client_message(
                &exec_id, id, "read_result", result,
            )?)))
        }
        "ls_args" => {
            let args = exec_args(exec, "ls_args", "agent.v1.LsArgs")?;
            let result = handle_ls_args(&args, ctx.workspace_root)?;
            Ok(Some(ExecOutcome::Frame(encode_exec_client_message(
                &exec_id, id, "ls_result", result,
            )?)))
        }
        "write_args" | "edit_args" => {
            let field = if case == "write_args" {
                "write_args"
            } else {
                "edit_args"
            };
            let args = exec_args(exec, field, "agent.v1.WriteArgs")?;
            let result = handle_write_args(&args, ctx.workspace_root)?;
            Ok(Some(ExecOutcome::Frame(encode_exec_client_message(
                &exec_id, id, "write_result", result,
            )?)))
        }
        "delete_args" => {
            let args = exec_args(exec, "delete_args", "agent.v1.DeleteArgs")?;
            let result = handle_delete_args(&args, ctx.workspace_root)?;
            Ok(Some(ExecOutcome::Frame(encode_exec_client_message(
                &exec_id, id, "delete_result", result,
            )?)))
        }
        "grep_args" => Ok(Some(ExecOutcome::Frame(encode_exec_client_message(
            &exec_id,
            id,
            "grep_result",
            grep_error_result(REJECT)?,
        )?))),
        "shell_args" | "shell_stream_args" | "background_shell_spawn_args" => {
            Ok(Some(ExecOutcome::Frame(encode_exec_client_message(
                &exec_id,
                id,
                "shell_result",
                shell_rejected_result("")?,
            )?)))
        }
        "write_shell_stdin_args" => Ok(Some(ExecOutcome::Frame(encode_exec_client_message(
            &exec_id,
            id,
            "write_shell_stdin_result",
            write_shell_stdin_error(REJECT)?,
        )?))),
        "fetch_args" => Ok(Some(ExecOutcome::Frame(encode_exec_client_message(
            &exec_id,
            id,
            "fetch_result",
            fetch_error_result("")?,
        )?))),
        "diagnostics_args" => Ok(Some(ExecOutcome::Frame(encode_exec_client_message(
            &exec_id,
            id,
            "diagnostics_result",
            DynamicMessage::new(message_desc("agent.v1.DiagnosticsResult")?),
        )?))),
        "list_mcp_resources_exec_args"
        | "read_mcp_resource_exec_args"
        | "record_screen_args"
        | "computer_use_args"
        | "setup_vm_environment_args" => Ok(Some(ExecOutcome::Frame(encode_exec_client_message(
            &exec_id,
            id,
            "mcp_result",
            DynamicMessage::new(message_desc("agent.v1.McpResult")?),
        )?))),
        "mcp_args" => {
            let mcp = get_message_field(exec, "mcp_args").ok_or_else(|| {
                sinew_core::AppError::Provider("mcp_args missing".into())
            })?;
            let tool_name = get_string_field(&mcp, "tool_name")
                .or_else(|| get_string_field(&mcp, "name"))
                .unwrap_or_default();
            let tool_call_id = get_string_field(&mcp, "tool_call_id")
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let args = decode_mcp_args_from_message(&mcp);
            Ok(Some(ExecOutcome::ToolRequest(PendingToolRequest {
                exec_id,
                exec_msg_id: id.to_string(),
                tool_call_id,
                tool_name,
                args,
            })))
        }
        _ => Ok(None),
    }
}

pub fn encode_mcp_result(
    exec_id: &str,
    id: u32,
    content: &str,
    is_error: bool,
) -> Result<Vec<u8>> {
    let mcp_result_desc = message_desc("agent.v1.McpResult")?;
    let mut mcp_result = DynamicMessage::new(mcp_result_desc);
    if is_error {
        let err_desc = message_desc("agent.v1.McpError")?;
        let mut err = DynamicMessage::new(err_desc);
        setf(&mut err, "error", ProtoValue::String(content.to_string()))?;
        setf(&mut mcp_result, "error", ProtoValue::Message(err))?;
    } else {
        let ok_desc = message_desc("agent.v1.McpSuccess")?;
        let text_desc = message_desc("agent.v1.McpTextContent")?;
        let item_desc = message_desc("agent.v1.McpToolResultContentItem")?;
        let mut text = DynamicMessage::new(text_desc);
        setf(&mut text, "text", ProtoValue::String(content.to_string()))?;
        let mut item = DynamicMessage::new(item_desc);
        setf(&mut item, "text", ProtoValue::Message(text))?;
        let mut ok = DynamicMessage::new(ok_desc);
        setf(&mut ok, "is_error", ProtoValue::Bool(false))?;
        setf(&mut ok, "content", ProtoValue::List(vec![ProtoValue::Message(item)]))?;
        setf(&mut mcp_result, "success", ProtoValue::Message(ok))?;
    }
    encode_exec_client_message(exec_id, id, "mcp_result", mcp_result)
}

fn resolve_path(root: &str, raw: &str) -> Result<PathBuf> {
    let base = PathBuf::from(root);
    let target = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        base.join(raw)
    };
    let normalized = std::fs::canonicalize(&target).unwrap_or(target);
    let normalized_root = std::fs::canonicalize(&base).unwrap_or(base);
    if !normalized.starts_with(&normalized_root) {
        return Err(sinew_core::AppError::Provider(
            "path outside workspace".into(),
        ));
    }
    Ok(normalized)
}

fn handle_read_args(args: &DynamicMessage, workspace_root: &str) -> Result<DynamicMessage> {
    let path = get_string_field(args, "path")
        .or_else(|| get_string_field(args, "file_path"))
        .unwrap_or_default();
    let result_desc = message_desc("agent.v1.ReadResult")?;
    let mut result = DynamicMessage::new(result_desc);
    let read_outcome = (|| -> Result<Vec<u8>> {
        let full = resolve_path(workspace_root, &path)?;
        std::fs::read(&full).map_err(|e| sinew_core::AppError::Provider(e.to_string()))
    })();
    match read_outcome {
        Ok(buf) => {
            let truncated = buf.len() > READ_LIMIT;
            let slice = if truncated {
                &buf[..READ_LIMIT]
            } else {
                &buf[..]
            };
            let content = String::from_utf8_lossy(slice).into_owned();
            let total_lines = content.lines().count() as i32;
            let ok_desc = message_desc("agent.v1.ReadSuccess")?;
            let mut ok = DynamicMessage::new(ok_desc);
            setf(&mut ok, "path", ProtoValue::String(path))?;
            setf(&mut ok, "total_lines", ProtoValue::I32(total_lines))?;
            setf(&mut ok, "file_size", ProtoValue::I64(buf.len() as i64))?;
            setf(&mut ok, "truncated", ProtoValue::Bool(truncated))?;
            setf(&mut ok, "content", ProtoValue::String(content))?;
            setf(&mut result, "success", ProtoValue::Message(ok))?;
        }
        Err(err) => {
            let err_desc = message_desc("agent.v1.ReadError")?;
            let mut err_msg = DynamicMessage::new(err_desc);
            setf(&mut err_msg, "path", ProtoValue::String(path))?;
            setf(&mut err_msg, "error", ProtoValue::String(err.to_string()))?;
            setf(&mut result, "error", ProtoValue::Message(err_msg))?;
        }
    }
    Ok(result)
}

fn handle_ls_args(args: &DynamicMessage, workspace_root: &str) -> Result<DynamicMessage> {
    let path = get_string_field(args, "path")
        .or_else(|| get_string_field(args, "target_directory"))
        .unwrap_or_else(|| ".".to_string());
    let result_desc = message_desc("agent.v1.LsResult")?;
    let mut result = DynamicMessage::new(result_desc);
    match resolve_path(workspace_root, &path) {
        Ok(full) => {
            let content = execute_tool("list_dir", &serde_json::json!({ "path": path }), workspace_root);
            let ok_desc = message_desc("agent.v1.LsSuccess")?;
            let node_desc = message_desc("agent.v1.LsDirectoryTreeNode")?;
            let mut root_node = DynamicMessage::new(node_desc);
            setf(
                &mut root_node,
                "abs_path",
                ProtoValue::String(full.display().to_string()),
            )?;
            setf(&mut root_node, "children_were_processed", ProtoValue::Bool(true))?;
            let mut ok = DynamicMessage::new(ok_desc);
            setf(&mut ok, "directory_tree_root", ProtoValue::Message(root_node))?;
            setf(&mut result, "success", ProtoValue::Message(ok))?;
            let _ = content;
        }
        Err(err) => {
            let rej_desc = message_desc("agent.v1.LsRejected")?;
            let mut rej = DynamicMessage::new(rej_desc);
            setf(&mut rej, "path", ProtoValue::String(path))?;
            setf(&mut rej, "reason", ProtoValue::String(err.to_string()))?;
            setf(&mut result, "rejected", ProtoValue::Message(rej))?;
        }
    }
    Ok(result)
}

fn handle_write_args(args: &DynamicMessage, workspace_root: &str) -> Result<DynamicMessage> {
    let path = get_string_field(args, "path")
        .or_else(|| get_string_field(args, "file_path"))
        .or_else(|| get_string_field(args, "target_file"))
        .unwrap_or_default();
    let tool_args = serde_json::json!({
        "path": path,
        "old_string": get_string_field(args, "old_string").or_else(|| get_string_field(args, "oldString")),
        "new_string": get_string_field(args, "new_string")
            .or_else(|| get_string_field(args, "newString"))
            .or_else(|| get_string_field(args, "contents"))
            .or_else(|| get_string_field(args, "content")),
    });
    let content = execute_tool("write", &tool_args, workspace_root);
    let result_desc = message_desc("agent.v1.WriteResult")?;
    let mut result = DynamicMessage::new(result_desc);
    if content.starts_with("Error:") {
        let rej_desc = message_desc("agent.v1.WriteRejected")?;
        let mut rej = DynamicMessage::new(rej_desc);
        setf(&mut rej, "path", ProtoValue::String(path))?;
        setf(&mut rej, "reason", ProtoValue::String(content))?;
        setf(&mut result, "rejected", ProtoValue::Message(rej))?;
    } else {
        let ok_desc = message_desc("agent.v1.WriteSuccess")?;
        let mut ok = DynamicMessage::new(ok_desc);
        setf(&mut ok, "path", ProtoValue::String(path))?;
        setf(&mut result, "success", ProtoValue::Message(ok))?;
    }
    Ok(result)
}

fn handle_delete_args(args: &DynamicMessage, workspace_root: &str) -> Result<DynamicMessage> {
    let path = get_string_field(args, "path")
        .or_else(|| get_string_field(args, "file_path"))
        .unwrap_or_default();
    let content = execute_tool("delete", &serde_json::json!({ "path": path }), workspace_root);
    let result_desc = message_desc("agent.v1.DeleteResult")?;
    let mut result = DynamicMessage::new(result_desc);
    if content.starts_with("Error:") {
        let rej_desc = message_desc("agent.v1.DeleteRejected")?;
        let mut rej = DynamicMessage::new(rej_desc);
        setf(&mut rej, "path", ProtoValue::String(path))?;
        setf(&mut rej, "reason", ProtoValue::String(content))?;
        setf(&mut result, "rejected", ProtoValue::Message(rej))?;
    } else {
        let ok_desc = message_desc("agent.v1.DeleteSuccess")?;
        let mut ok = DynamicMessage::new(ok_desc);
        setf(&mut ok, "path", ProtoValue::String(path))?;
        setf(&mut result, "success", ProtoValue::Message(ok))?;
    }
    Ok(result)
}

fn build_request_context_result(ctx: &ExecContext<'_>) -> Result<DynamicMessage> {
    let root = ctx.workspace_root.trim();
    let root = if root.is_empty() { "." } else { root };
    let result_desc = message_desc("agent.v1.RequestContextResult")?;
    let mut result = DynamicMessage::new(result_desc);
    let ok_desc = message_desc("agent.v1.RequestContextSuccess")?;
    let ctx_desc = message_desc("agent.v1.RequestContext")?;
    let env_desc = message_desc("agent.v1.RequestContextEnv")?;
    let mut env = DynamicMessage::new(env_desc);
    setf(
        &mut env,
        "os_version",
        ProtoValue::String(format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)),
    )?;
    setf(
        &mut env,
        "workspace_paths",
        ProtoValue::List(vec![ProtoValue::String(root.to_string())]),
    )?;
    setf(
        &mut env,
        "shell",
        ProtoValue::String(std::env::var("ComSpec").or_else(|_| std::env::var("SHELL")).unwrap_or_default()),
    )?;
    setf(&mut env, "sandbox_enabled", ProtoValue::Bool(false))?;
    setf(&mut env, "time_zone", ProtoValue::String("UTC".to_string()))?;

    let mut request_context = DynamicMessage::new(ctx_desc);
    setf(&mut request_context, "env", ProtoValue::Message(env))?;
    for tool in ctx.tools {
        if let Some(def) = build_mcp_tool_definition(tool) {
            if let Some(existing) = request_context.get_field_by_name("tools") {
                if let ProtoValue::List(mut list) = existing.as_ref().clone() {
                    list.push(ProtoValue::Message(def));
                    setf(&mut request_context, "tools", ProtoValue::List(list))?;
                    continue;
                }
            }
            setf(&mut request_context, "tools", ProtoValue::List(vec![ProtoValue::Message(def)]))?;
        }
    }

    let mut ok = DynamicMessage::new(ok_desc);
    setf(&mut ok, "request_context", ProtoValue::Message(request_context))?;
    setf(&mut result, "success", ProtoValue::Message(ok))?;
    Ok(result)
}

fn build_mcp_tool_definition(tool: &JsonValue) -> Option<DynamicMessage> {
    let name = tool.get("name")?.as_str()?;
    let desc = message_desc("agent.v1.McpToolDefinition").ok()?;
    let mut msg = DynamicMessage::new(desc);
    let schema = tool
        .get("parameters")
        .cloned()
        .unwrap_or(serde_json::json!({"type":"object","properties":{},"required":[]}));
    setf(&mut msg, "name", ProtoValue::String(name.to_string())).ok()?;
    setf(&mut msg, "tool_name", ProtoValue::String(name.to_string())).ok()?;
    setf(
        &mut msg,
        "description",
        ProtoValue::String(tool.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string()),
    )
    .ok()?;
    setf(&mut msg, "provider_identifier", ProtoValue::String("sinew".to_string())).ok()?;
    setf(
        &mut msg,
        "input_schema",
        ProtoValue::Bytes(Bytes::from(schema.to_string())),
    )
    .ok()?;
    Some(msg)
}

fn grep_error_result(msg: &str) -> Result<DynamicMessage> {
    let result_desc = message_desc("agent.v1.GrepResult")?;
    let err_desc = message_desc("agent.v1.GrepError")?;
    let mut err = DynamicMessage::new(err_desc);
    setf(&mut err, "error", ProtoValue::String(msg.to_string()))?;
    let mut result = DynamicMessage::new(result_desc);
    setf(&mut result, "error", ProtoValue::Message(err))?;
    Ok(result)
}

fn shell_rejected_result(command: &str) -> Result<DynamicMessage> {
    let result_desc = message_desc("agent.v1.ShellResult")?;
    let rej_desc = message_desc("agent.v1.ShellRejected")?;
    let mut rej = DynamicMessage::new(rej_desc);
    setf(&mut rej, "command", ProtoValue::String(command.to_string()))?;
    setf(&mut rej, "reason", ProtoValue::String(REJECT.to_string()))?;
    setf(&mut rej, "is_readonly", ProtoValue::Bool(false))?;
    let mut result = DynamicMessage::new(result_desc);
    setf(&mut result, "rejected", ProtoValue::Message(rej))?;
    Ok(result)
}

fn write_shell_stdin_error(msg: &str) -> Result<DynamicMessage> {
    let result_desc = message_desc("agent.v1.WriteShellStdinResult")?;
    let err_desc = message_desc("agent.v1.WriteShellStdinError")?;
    let mut err = DynamicMessage::new(err_desc);
    setf(&mut err, "error", ProtoValue::String(msg.to_string()))?;
    let mut result = DynamicMessage::new(result_desc);
    setf(&mut result, "error", ProtoValue::Message(err))?;
    Ok(result)
}

fn fetch_error_result(url: &str) -> Result<DynamicMessage> {
    let result_desc = message_desc("agent.v1.FetchResult")?;
    let err_desc = message_desc("agent.v1.FetchError")?;
    let mut err = DynamicMessage::new(err_desc);
    setf(&mut err, "url", ProtoValue::String(url.to_string()))?;
    setf(&mut err, "error", ProtoValue::String(REJECT.to_string()))?;
    let mut result = DynamicMessage::new(result_desc);
    setf(&mut result, "error", ProtoValue::Message(err))?;
    Ok(result)
}
