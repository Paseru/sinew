use serde_json::{json, Map, Value};

#[derive(Debug, Clone)]
pub struct ParsedToolCall {
    pub id: String,
    pub cursor_tool: String,
    pub sinew_name: String,
    pub input: Value,
}

pub const SUPPORTED_TOOLS: &[&str] = &[
    "CLIENT_SIDE_TOOL_V2_READ_FILE_V2",
    "CLIENT_SIDE_TOOL_V2_EDIT_FILE_V2",
    "CLIENT_SIDE_TOOL_V2_LIST_DIR_V2",
    "CLIENT_SIDE_TOOL_V2_RIPGREP_RAW_SEARCH",
    "CLIENT_SIDE_TOOL_V2_GLOB_FILE_SEARCH",
    "CLIENT_SIDE_TOOL_V2_RUN_TERMINAL_COMMAND_V2",
    "CLIENT_SIDE_TOOL_V2_WEB_SEARCH",
    "CLIENT_SIDE_TOOL_V2_WEB_FETCH",
    "CLIENT_SIDE_TOOL_V2_SEMANTIC_SEARCH_FULL",
    "CLIENT_SIDE_TOOL_V2_TODO_READ",
    "CLIENT_SIDE_TOOL_V2_TODO_WRITE",
    "CLIENT_SIDE_TOOL_V2_ASK_QUESTION",
    "CLIENT_SIDE_TOOL_V2_CALL_MCP_TOOL",
    "CLIENT_SIDE_TOOL_V2_GENERATE_IMAGE",
    "CLIENT_SIDE_TOOL_V2_DELETE_FILE",
];

pub fn sinew_tool_name(cursor_tool: &str) -> Option<&'static str> {
    match cursor_tool {
        "CLIENT_SIDE_TOOL_V2_READ_FILE_V2" | "CLIENT_SIDE_TOOL_V2_READ_FILE" => Some("read"),
        "CLIENT_SIDE_TOOL_V2_EDIT_FILE_V2" | "CLIENT_SIDE_TOOL_V2_EDIT_FILE" => Some("edit_file"),
        "CLIENT_SIDE_TOOL_V2_LIST_DIR_V2" | "CLIENT_SIDE_TOOL_V2_LIST_DIR" => Some("glob"),
        "CLIENT_SIDE_TOOL_V2_RIPGREP_RAW_SEARCH" | "CLIENT_SIDE_TOOL_V2_RIPGREP_SEARCH" => {
            Some("grep")
        }
        "CLIENT_SIDE_TOOL_V2_GLOB_FILE_SEARCH" | "CLIENT_SIDE_TOOL_V2_FILE_SEARCH" => Some("glob"),
        "CLIENT_SIDE_TOOL_V2_RUN_TERMINAL_COMMAND_V2" => Some("bash"),
        "CLIENT_SIDE_TOOL_V2_WEB_SEARCH" => Some("web_search"),
        "CLIENT_SIDE_TOOL_V2_WEB_FETCH" => Some("web_fetch"),
        "CLIENT_SIDE_TOOL_V2_TODO_READ" | "CLIENT_SIDE_TOOL_V2_TODO_WRITE" => Some("todo_list"),
        "CLIENT_SIDE_TOOL_V2_ASK_QUESTION" => Some("question"),
        "CLIENT_SIDE_TOOL_V2_CALL_MCP_TOOL" | "CLIENT_SIDE_TOOL_V2_MCP" => Some("load_mcp_tool"),
        "CLIENT_SIDE_TOOL_V2_GENERATE_IMAGE" => Some("create_image"),
        "CLIENT_SIDE_TOOL_V2_SEMANTIC_SEARCH_FULL" => Some("codebase_search"),
        "CLIENT_SIDE_TOOL_V2_DELETE_FILE" => Some("bash"),
        _ => None,
    }
}

pub fn is_mappable_sinew_tool(name: &str) -> bool {
    cursor_tool_name(name) != "CLIENT_SIDE_TOOL_V2_UNSPECIFIED"
}

pub fn cursor_tool_name(sinew_tool: &str) -> &'static str {
    match sinew_tool {
        "read" => "CLIENT_SIDE_TOOL_V2_READ_FILE_V2",
        "edit_file" => "CLIENT_SIDE_TOOL_V2_EDIT_FILE_V2",
        "write_file" => "CLIENT_SIDE_TOOL_V2_EDIT_FILE_V2",
        "glob" => "CLIENT_SIDE_TOOL_V2_GLOB_FILE_SEARCH",
        "grep" => "CLIENT_SIDE_TOOL_V2_RIPGREP_RAW_SEARCH",
        "codebase_search" => "CLIENT_SIDE_TOOL_V2_SEMANTIC_SEARCH_FULL",
        "bash" => "CLIENT_SIDE_TOOL_V2_RUN_TERMINAL_COMMAND_V2",
        "web_search" => "CLIENT_SIDE_TOOL_V2_WEB_SEARCH",
        "web_fetch" => "CLIENT_SIDE_TOOL_V2_WEB_FETCH",
        "todo_list" => "CLIENT_SIDE_TOOL_V2_TODO_WRITE",
        "question" => "CLIENT_SIDE_TOOL_V2_ASK_QUESTION",
        "load_mcp_tool" => "CLIENT_SIDE_TOOL_V2_CALL_MCP_TOOL",
        "create_image" => "CLIENT_SIDE_TOOL_V2_GENERATE_IMAGE",
        _ => "CLIENT_SIDE_TOOL_V2_UNSPECIFIED",
    }
}

pub fn parse_tool_call(value: &Value) -> Option<ParsedToolCall> {
    let tool = value
        .get("tool")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("tool")
                .and_then(Value::as_i64)
                .and_then(tool_name_from_number)
        })?;
    let sinew_name = sinew_tool_name(tool)?.to_string();
    let id = value
        .get("toolCallId")
        .or_else(|| value.get("tool_call_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let mut input = if let Some(raw) = value
        .get("rawArgs")
        .or_else(|| value.get("raw_args"))
        .and_then(Value::as_str)
        .filter(|raw| !raw.trim().is_empty())
    {
        serde_json::from_str(raw).unwrap_or_else(|_| json!({ "raw_args": raw }))
    } else {
        map_tool_params(tool, value)
    };
    let mut sinew_name = sinew_name;
    if sinew_name == "load_mcp_tool" {
        if let Some((name, mapped_input)) = map_builtin_mcp_call(&input) {
            sinew_name = name;
            input = mapped_input;
        }
    }
    if sinew_name == "edit_file"
        && input.get("content").is_some()
        && input.get("path").is_some()
        && input.get("files").is_none()
    {
        sinew_name = "write_file".to_string();
    }
    Some(ParsedToolCall {
        id,
        cursor_tool: tool.to_string(),
        sinew_name,
        input,
    })
}

pub fn build_client_tool_result(
    tool_call_id: &str,
    sinew_name: &str,
    cursor_tool: &str,
    content: &str,
    is_error: bool,
) -> Value {
    let mut result = json!({
        "toolCallId": tool_call_id,
        "tool": cursor_tool,
    });
    if is_error {
        result["error"] = json!({ "message": crate::sanitize::sanitize_outbound_text(content) });
        return result;
    }
    let content = crate::sanitize::sanitize_outbound_text(content);
    match sinew_name {
        "read" => {
            result["readFileV2Result"] = json!({ "contents": content });
        }
        "write_file" => {
            result["editFileV2Result"] = json!({ "resultForModel": content });
        }
        "edit_file" => {
            result["editFileV2Result"] = json!({ "resultForModel": content });
        }
        "bash" => {
            result["runTerminalCommandV2Result"] = json!({ "output": content });
        }
        "grep" => {
            result["ripgrepRawSearchResult"] = json!({ "output": content });
        }
        "glob" => {
            result["globFileSearchResult"] = json!({ "directories": [] , "output": content });
        }
        "web_search" => {
            result["webSearchResult"] = json!({ "references": [], "output": content });
        }
        "web_fetch" => {
            result["webFetchResult"] = json!({ "markdown": content });
        }
        "todo_list" => {
            result["todoWriteResult"] = json!({ "content": content });
        }
        "question" => {
            result["askQuestionResult"] = json!({ "content": content });
        }
        "load_mcp_tool" => {
            result["callMcpToolResult"] = json!({ "result": content });
        }
        "create_image" => {
            result["generateImageResult"] = json!({ "resultForModel": content });
        }
        _ => {
            result["resultForModel"] = json!(content);
        }
    }
    crate::sanitize::sanitize_outbound_json(result)
}

fn tool_name_from_number(value: i64) -> Option<&'static str> {
    match value {
        40 => Some("CLIENT_SIDE_TOOL_V2_READ_FILE_V2"),
        38 => Some("CLIENT_SIDE_TOOL_V2_EDIT_FILE_V2"),
        39 => Some("CLIENT_SIDE_TOOL_V2_LIST_DIR_V2"),
        41 => Some("CLIENT_SIDE_TOOL_V2_RIPGREP_RAW_SEARCH"),
        42 => Some("CLIENT_SIDE_TOOL_V2_GLOB_FILE_SEARCH"),
        15 => Some("CLIENT_SIDE_TOOL_V2_RUN_TERMINAL_COMMAND_V2"),
        18 => Some("CLIENT_SIDE_TOOL_V2_WEB_SEARCH"),
        57 => Some("CLIENT_SIDE_TOOL_V2_WEB_FETCH"),
        34 => Some("CLIENT_SIDE_TOOL_V2_TODO_READ"),
        35 => Some("CLIENT_SIDE_TOOL_V2_TODO_WRITE"),
        51 => Some("CLIENT_SIDE_TOOL_V2_ASK_QUESTION"),
        49 => Some("CLIENT_SIDE_TOOL_V2_CALL_MCP_TOOL"),
        53 => Some("CLIENT_SIDE_TOOL_V2_GENERATE_IMAGE"),
        _ => None,
    }
}

fn map_tool_params(tool: &str, value: &Value) -> Value {
    match tool {
        "CLIENT_SIDE_TOOL_V2_READ_FILE_V2" => {
            let params = params(value, "readFileV2Params", "read_file_v2_params");
            json!({
                "path": field(params, &["targetFile", "target_file"]),
                "offset": field(params, &["offset"]).unwrap_or(json!(0)),
                "limit": field(params, &["limit"]).unwrap_or(json!(500)),
            })
        }
        "CLIENT_SIDE_TOOL_V2_RIPGREP_RAW_SEARCH" | "CLIENT_SIDE_TOOL_V2_RIPGREP_SEARCH" => {
            let params = params(value, "ripgrepRawSearchParams", "ripgrep_raw_search_params")
                .or_else(|| params(value, "ripgrepSearchParams", "ripgrep_search_params"));
            json!({
                "pattern": field(params, &["pattern", "query", "searchTerm", "search_term"]),
                "path": field(params, &["path", "targetDirectory", "target_directory"]).unwrap_or(json!(".")),
                "include": field(params, &["glob", "include"]),
                "limit": field(params, &["limit"]).unwrap_or(json!(100)),
            })
        }
        "CLIENT_SIDE_TOOL_V2_GLOB_FILE_SEARCH" | "CLIENT_SIDE_TOOL_V2_FILE_SEARCH" => {
            let params = params(value, "globFileSearchParams", "glob_file_search_params")
                .or_else(|| params(value, "fileSearchParams", "file_search_params"));
            json!({
                "pattern": field(params, &["globPattern", "glob_pattern"]).unwrap_or(json!("**/*")),
                "path": field(params, &["targetDirectory", "target_directory"]).unwrap_or(json!(".")),
                "limit": field(params, &["limit"]).unwrap_or(json!(200)),
            })
        }
        "CLIENT_SIDE_TOOL_V2_SEMANTIC_SEARCH_FULL" => {
            let params = params(value, "semanticSearchFullParams", "semantic_search_full_params")
                .or_else(|| params(value, "semanticSearchParams", "semantic_search_params"));
            json!({
                "query": field(params, &["query", "searchQuery", "search_query", "pattern"])
                    .unwrap_or(json!("")),
                "path": field(params, &["targetDirectory", "target_directory", "path"]).unwrap_or(json!(".")),
                "limit": field(params, &["limit", "maxResults", "max_results"]).unwrap_or(json!(20)),
            })
        }
        "CLIENT_SIDE_TOOL_V2_DELETE_FILE" => {
            let params = params(value, "deleteFileParams", "delete_file_params")
                .or_else(|| params(value, "deleteFileV2Params", "delete_file_v2_params"));
            let path = field(
                params,
                &[
                    "relativeWorkspacePath",
                    "relative_workspace_path",
                    "targetFile",
                    "target_file",
                ],
            )
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_default();
            json!({
                "command": delete_file_command(&path),
            })
        }
        "CLIENT_SIDE_TOOL_V2_RUN_TERMINAL_COMMAND_V2" => {
            let params = params(value, "runTerminalCommandV2Params", "run_terminal_command_v2_params");
            json!({
                "command": field(params, &["command"]),
                "cwd": field(params, &["cwd"]),
            })
        }
        "CLIENT_SIDE_TOOL_V2_WEB_SEARCH" => {
            let params = params(value, "webSearchParams", "web_search_params");
            json!({
                "query": field(params, &["searchTerm", "search_term"]).unwrap_or(json!("")),
            })
        }
        "CLIENT_SIDE_TOOL_V2_WEB_FETCH" => {
            let params = params(value, "webFetchParams", "web_fetch_params");
            json!({ "url": field(params, &["url"]) })
        }
        "CLIENT_SIDE_TOOL_V2_GENERATE_IMAGE" => {
            map_generate_image_params(value)
        }
        "CLIENT_SIDE_TOOL_V2_EDIT_FILE_V2" | "CLIENT_SIDE_TOOL_V2_EDIT_FILE" => {
            map_edit_params(value)
        }
        "CLIENT_SIDE_TOOL_V2_LIST_DIR_V2" | "CLIENT_SIDE_TOOL_V2_LIST_DIR" => {
            let params = params(value, "listDirV2Params", "list_dir_v2_params")
                .or_else(|| params(value, "listDirParams", "list_dir_params"));
            json!({
                "pattern": "*",
                "path": field(params, &["targetDirectory", "target_directory"]).unwrap_or(json!(".")),
                "limit": field(params, &["limit"]).unwrap_or(json!(200)),
            })
        }
        _ => flatten_params(value),
    }
}

fn map_generate_image_params(value: &Value) -> Value {
    let params = params(value, "generateImageParams", "generate_image_params")
        .or_else(|| params(value, "generateImageV2Params", "generate_image_v2_params"));
    let mut mapped = json!({
        "prompt": field(params, &["prompt", "description", "text", "query"]).unwrap_or(json!("")),
    });
    if let Some(size) = field(params, &["size", "imageSize", "image_size"]) {
        mapped["size"] = size;
    }
    if let Some(n) = field(params, &["n", "numImages", "num_images", "count"]) {
        mapped["n"] = n;
    }
    if let Some(format) = field(params, &["outputFormat", "output_format", "format"]) {
        mapped["output_format"] = format;
    }
    if let Some(ratio) = field(params, &["aspectRatio", "aspect_ratio"]) {
        mapped["aspect_ratio"] = ratio;
    }
    if let Some(image_size) = field(params, &["imageSizeTier", "image_size_tier", "resolution"]) {
        mapped["image_size"] = image_size;
    }
    mapped
}

fn map_builtin_mcp_call(input: &Value) -> Option<(String, Value)> {
    const MCP_CREATE_IMAGE: &str = "mcp__sinew__create_image";
    let tool_name = input
        .get("toolName")
        .or_else(|| input.get("tool_name"))
        .or_else(|| input.get("name"))
        .and_then(Value::as_str)?;
    if tool_name != MCP_CREATE_IMAGE {
        return None;
    }
    let args = input
        .get("args")
        .or_else(|| input.get("arguments"))
        .or_else(|| input.get("input"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    Some(("create_image".into(), map_create_image_input(&args)))
}

fn map_create_image_input(value: &Value) -> Value {
    if value.get("prompt").is_some() {
        return value.clone();
    }
    map_generate_image_params(value)
}

fn map_edit_params(value: &Value) -> Value {
    let params = params(value, "editFileV2Params", "edit_file_v2_params")
        .or_else(|| params(value, "editFileParams", "edit_file_params"));
    let path = field(params, &["relativeWorkspacePath", "relative_workspace_path"])
        .unwrap_or_else(|| json!(""));
    if let Some(contents) = field(params, &["contentsAfterEdit", "contents_after_edit"]) {
        return json!({
            "path": path,
            "content": contents,
        });
    }
    if let Some(diff) = params
        .and_then(|params| params.get("diff"))
        .or_else(|| params.and_then(|params| params.get("Diff")))
    {
        if let Some(mapped) = map_diff_edits(&path, diff) {
            return mapped;
        }
    }
    if let Some(text) = params
        .and_then(|params| params.get("streamingContent"))
        .or_else(|| params.and_then(|params| params.get("streaming_content")))
        .or_else(|| params.and_then(|params| params.get("resultForModel")))
        .or_else(|| params.and_then(|params| params.get("result_for_model")))
        .and_then(Value::as_str)
    {
        if let Some(mapped) = map_search_replace_blocks(&path, text) {
            return mapped;
        }
    }
    json!({
        "files": [{
            "path": path,
            "edits": [{
                "oldContent": "",
                "newContent": field(params, &["streamingContent", "streaming_content", "resultForModel", "result_for_model"]).unwrap_or(json!("")),
                "replaceAll": false
            }]
        }]
    })
}

fn map_diff_edits(path: &Value, diff: &Value) -> Option<Value> {
    let chunks = diff
        .get("chunks")
        .or_else(|| diff.get("edits"))
        .and_then(Value::as_array)?;
    let mut edits = Vec::new();
    for chunk in chunks {
        let old_content = field(Some(chunk), &["originalText", "original_text", "before", "oldContent", "old_content"]);
        let new_content = field(Some(chunk), &["newText", "new_text", "after", "newContent", "new_content"]);
        if old_content.is_some() || new_content.is_some() {
            edits.push(json!({
                "oldContent": old_content.unwrap_or(json!("")),
                "newContent": new_content.unwrap_or(json!("")),
                "replaceAll": false
            }));
        }
    }
    if edits.is_empty() {
        return None;
    }
    Some(json!({ "files": [{ "path": path, "edits": edits }] }))
}

fn map_search_replace_blocks(path: &Value, text: &str) -> Option<Value> {
    let mut edits = Vec::new();
    let mut search = None;
    let mut replace = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("<<<<<<< SEARCH") {
            search = Some(rest.trim_start_matches('\n').to_string());
            replace = None;
        } else if let Some(rest) = line.strip_prefix("=======") {
            replace = Some(rest.trim_start_matches('\n').to_string());
        } else if line.starts_with(">>>>>>> REPLACE") {
            if let (Some(old_content), Some(new_content)) = (search.take(), replace.take()) {
                edits.push(json!({
                    "oldContent": old_content,
                    "newContent": new_content,
                    "replaceAll": false
                }));
            }
        } else if let Some(current) = replace.as_mut() {
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        } else if let Some(current) = search.as_mut() {
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        }
    }
    if edits.is_empty() {
        return None;
    }
    Some(json!({ "files": [{ "path": path, "edits": edits }] }))
}

fn delete_file_command(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    let escaped = path.replace('\'', "''");
    #[cfg(windows)]
    {
        format!("Remove-Item -LiteralPath '{escaped}' -Force")
    }
    #[cfg(not(windows))]
    {
        format!("rm -- {escaped}")
    }
}

fn params<'a>(value: &'a Value, camel: &str, snake: &str) -> Option<&'a Value> {
    value.get(camel).or_else(|| value.get(snake))
}

fn field(value: Option<&Value>, keys: &[&str]) -> Option<Value> {
    let obj = value?;
    for key in keys {
        if let Some(found) = obj.get(*key) {
            return Some(found.clone());
        }
    }
    None
}

fn flatten_params(value: &Value) -> Value {
    let mut mapped = Map::new();
    if let Some(obj) = value.as_object() {
        for (key, val) in obj {
            if key.ends_with("Params") || key.ends_with("_params") {
                if let Some(inner) = val.as_object() {
                    for (inner_key, inner_val) in inner {
                        mapped.insert(inner_key.clone(), inner_val.clone());
                    }
                }
            }
        }
    }
    Value::Object(mapped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_search_replace_blocks_to_edit_file_input() {
        let text = "<<<<<<< SEARCH\nold line\n=======\nnew line\n>>>>>>> REPLACE";
        let mapped = map_search_replace_blocks(&json!("src/a.rs"), text).expect("mapped");
        assert_eq!(mapped["files"][0]["edits"][0]["oldContent"], "old line");
        assert_eq!(mapped["files"][0]["edits"][0]["newContent"], "new line");
    }

    #[test]
    fn maps_list_dir_to_shallow_glob() {
        let value = json!({
            "tool": "CLIENT_SIDE_TOOL_V2_LIST_DIR_V2",
            "toolCallId": "call_1",
            "listDirV2Params": {
                "targetDirectory": "src"
            }
        });
        let parsed = parse_tool_call(&value).expect("parsed");
        assert_eq!(parsed.input["pattern"], "*");
        assert_eq!(parsed.input["path"], "src");
        assert_eq!(parsed.input["limit"], 200);
    }

    #[test]
    fn maps_delete_file_to_shell_command() {
        let value = json!({
            "tool": "CLIENT_SIDE_TOOL_V2_DELETE_FILE",
            "toolCallId": "call_1",
            "deleteFileParams": {
                "relativeWorkspacePath": "tmp/old.txt"
            }
        });
        let parsed = parse_tool_call(&value).expect("parsed");
        assert_eq!(parsed.sinew_name, "bash");
        assert!(parsed.input["command"]
            .as_str()
            .unwrap_or("")
            .contains("tmp/old.txt"));
    }

    #[test]
    fn maps_semantic_search_to_codebase_search() {
        let value = json!({
            "tool": "CLIENT_SIDE_TOOL_V2_SEMANTIC_SEARCH_FULL",
            "toolCallId": "call_1",
            "semanticSearchFullParams": {
                "query": "auth token",
                "targetDirectory": "src"
            }
        });
        let parsed = parse_tool_call(&value).expect("parsed");
        assert_eq!(parsed.sinew_name, "codebase_search");
        assert_eq!(parsed.input["query"], "auth token");
        assert_eq!(parsed.input["path"], "src");
        assert_eq!(parsed.input["limit"], 20);
    }

    #[test]
    fn maps_generate_image_to_create_image() {
        let value = json!({
            "tool": "CLIENT_SIDE_TOOL_V2_GENERATE_IMAGE",
            "toolCallId": "call_img",
            "generateImageParams": {
                "prompt": "A minimal blue logo"
            }
        });
        let parsed = parse_tool_call(&value).expect("parsed");
        assert_eq!(parsed.sinew_name, "create_image");
        assert_eq!(parsed.input["prompt"], "A minimal blue logo");
    }

    #[test]
    fn maps_builtin_mcp_create_image_call() {
        let value = json!({
            "tool": "CLIENT_SIDE_TOOL_V2_CALL_MCP_TOOL",
            "toolCallId": "call_mcp_img",
            "callMcpToolParams": {
                "toolName": "mcp__sinew__create_image",
                "args": { "prompt": "Sunset over mountains" }
            }
        });
        let parsed = parse_tool_call(&value).expect("parsed");
        assert_eq!(parsed.sinew_name, "create_image");
        assert_eq!(parsed.input["prompt"], "Sunset over mountains");
    }

    #[test]
    fn builds_generate_image_tool_result() {
        let result = build_client_tool_result(
            "call_img",
            "create_image",
            "CLIENT_SIDE_TOOL_V2_GENERATE_IMAGE",
            "saved: assets/logo.png",
            false,
        );
        assert_eq!(
            result["generateImageResult"]["resultForModel"],
            "saved: assets/logo.png"
        );
    }
}
