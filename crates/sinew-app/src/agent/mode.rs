use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use sinew_core::ToolDescriptor;

use crate::{GoalWorkflowState, ToolRunResult};

use super::context::AgentMode;

const PLAN_MODE_PROMPT: &str = r#"You are in Plan mode.

Rules:
- Build understanding by reading/searching/running diagnostic shell commands as needed.
- Do not edit workspace files and do not use apply_patch.
- You must keep the user in a Question loop until the user explicitly clicks "Send and stop questions".
- If the user message does not contain <plan_mode_control action="stop_questions">, your turn must end by calling the Question tool. Do not write the final plan yet.
- After each normal answer to a Question, inspect/explore more if needed, then ask the next Question.
- If you have no remaining substantive question, ask the user to confirm that you should create the plan now. Still use the Question tool.
- Only when the user message contains <plan_mode_control action="stop_questions">, stop asking questions and write the complete plan now.
- When the plan is ready, respond with only the Markdown plan. Do not implement it.

STRICTLY FORBIDDEN in the plan (unless the user explicitly requests it):
- Code snippets, pseudo-code, or inline code
- File paths, directory structures, or tree views
- Function, class, variable, or module names
- Shell commands or CLI instructions
- Technical configuration details
- Any implementation-specific notation

The plan should read as a clear description of intent and expected behavior that anyone could understand without technical background. Bullet points and paragraphs are both acceptable. The focus is on WHAT the system should do, not HOW the code should be written.

If technical specifics become necessary to avoid ambiguity, the AI may include them at its discretion, integrated naturally into the plan - but this should remain the exception, not the default."#;

const GOAL_MODE_PROMPT: &str = r#"You are in Goal mode.

Rules:
- Work autonomously toward the objective across as many turns as needed.
- Do not treat one answer as the end of the goal unless the objective is genuinely complete.
- Do not repeat completed work. First orient from existing context, then continue from the next useful step.
- Use tools and make edits normally, like Act mode.
- Before deciding the goal is complete, audit the objective against the current workspace state.
- When the objective is truly complete, you MUST call update_goal with status "complete" before your final response.
- If the objective is not complete by the end of this turn, briefly report progress and the next step. The app will continue automatically."#;

pub(super) fn update_goal_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        name: "update_goal".into(),
        description: "Mark the active Goal mode objective complete. Use this only after auditing that the full objective is genuinely finished.".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "enum": ["complete"],
                    "description": "Use complete only when the goal is truly done."
                },
                "summary": {
                    "type": "string",
                    "description": "A concise summary of what was completed."
                }
            },
            "required": ["status"],
            "additionalProperties": false
        }),
    }
}

pub(super) fn run_update_goal(goal_workflow: &mut GoalWorkflowState, input: Value) -> ToolRunResult {
    let status = input
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if status != "complete" {
        return ToolRunResult::err("status must be complete", Vec::new());
    }

    let Some((objective, started_at_ms)) = goal_objective_and_started(goal_workflow) else {
        return ToolRunResult::err("no active goal to update", Vec::new());
    };
    *goal_workflow = GoalWorkflowState::Complete {
        objective,
        started_at_ms,
        completed_at_ms: now_ms(),
    };

    let summary = input
        .get("summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Goal marked complete");
    ToolRunResult::ok(summary.to_string(), Vec::new())
}

fn goal_objective_and_started(goal_workflow: &GoalWorkflowState) -> Option<(String, i64)> {
    match goal_workflow {
        GoalWorkflowState::Active {
            objective,
            started_at_ms,
            ..
        }
        | GoalWorkflowState::Paused {
            objective,
            started_at_ms,
            ..
        }
        | GoalWorkflowState::Complete {
            objective,
            started_at_ms,
            ..
        } => Some((objective.clone(), *started_at_ms)),
        GoalWorkflowState::Idle => None,
    }
}

pub(super) fn goal_objective(goal_workflow: &GoalWorkflowState) -> Option<&str> {
    match goal_workflow {
        GoalWorkflowState::Active { objective, .. }
        | GoalWorkflowState::Paused { objective, .. }
        | GoalWorkflowState::Complete { objective, .. } => Some(objective.as_str()),
        GoalWorkflowState::Idle => None,
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

pub fn system_prompt_for_mode(base: &str, mode: AgentMode) -> String {
    match mode {
        AgentMode::Act => base.to_string(),
        AgentMode::Plan => format!("{base}\n\n<plan_mode>\n{PLAN_MODE_PROMPT}\n</plan_mode>"),
        AgentMode::Goal => format!("{base}\n\n<goal_mode>\n{GOAL_MODE_PROMPT}\n</goal_mode>"),
    }
}

pub(super) fn system_prompt_for_turn(
    base: &str,
    mode: AgentMode,
    goal_workflow: &GoalWorkflowState,
) -> String {
    let prompt = system_prompt_for_mode(base, mode);
    if mode != AgentMode::Goal {
        return prompt;
    }
    let Some(objective) = goal_objective(goal_workflow) else {
        return prompt;
    };
    format!("{prompt}\n\n<goal_objective>\n{objective}\n</goal_objective>")
}
