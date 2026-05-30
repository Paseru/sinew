const fs = require('fs');
let code = fs.readFileSync('crates/sinew-app/src/agent/tool_dispatch.rs', 'utf8');

const editFileReplacement = \    } else if canonical_name == tool_names::EDIT_FILE {
        if mode == AgentMode::Plan {
            return ToolRunResult::err("edit_file is unavailable in Plan mode", Vec::new());
        }
        let mut result = edit_file.run(input, read_fingerprints).await;
        if !result.is_error {
            let lints = read_lints.run(serde_json::json!({})).await;
            if !lints.is_error && !lints.output.trim().is_empty() && !lints.output.contains("No linter errors found") {
                result.output.push_str("\\n\\n[Auto-Lint Diagnostics (Self-Healing)]:\\n");
                result.output.push_str(&lints.output);
            }
        }
        result
    } else if canonical_name == tool_names::WRITE_FILE {
        if mode == AgentMode::Plan {
            return ToolRunResult::err("write_file is unavailable in Plan mode", Vec::new());
        }
        let mut result = write_file.run(input, read_fingerprints).await;
        if !result.is_error {
            let lints = read_lints.run(serde_json::json!({})).await;
            if !lints.is_error && !lints.output.trim().is_empty() && !lints.output.contains("No linter errors found") {
                result.output.push_str("\\n\\n[Auto-Lint Diagnostics (Self-Healing)]:\\n");
                result.output.push_str(&lints.output);
            }
        }
        result\;

const regex = /\}\s*else if canonical_name == tool_names::EDIT_FILE\s*\{\s*if mode == AgentMode::Plan\s*\{\s*return ToolRunResult::err\("edit_file is unavailable in Plan mode", Vec::new\(\)\);\s*\}\s*edit_file\.run\(input, read_fingerprints\)\.await\s*\}\s*else if canonical_name == tool_names::WRITE_FILE\s*\{\s*if mode == AgentMode::Plan\s*\{\s*return ToolRunResult::err\("write_file is unavailable in Plan mode", Vec::new\(\)\);\s*\}\s*write_file\.run\(input, read_fingerprints\)\.await/;

code = code.replace(regex, editFileReplacement);

fs.writeFileSync('crates/sinew-app/src/agent/tool_dispatch.rs', code);
