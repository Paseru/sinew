use crate::*;

pub(super) fn conversation_active_mode(conversation: &SavedConversation) -> AgentMode {
    workflow_active_mode(&conversation.plan_workflow, &conversation.goal_workflow)
}

pub(super) fn workflow_active_mode(
    plan_workflow: &PlanWorkflowState,
    goal_workflow: &GoalWorkflowState,
) -> AgentMode {
    let mode = match plan_workflow {
        PlanWorkflowState::Idle => AgentMode::Act,
        PlanWorkflowState::PlanningQuestions | PlanWorkflowState::PlanReady { .. } => {
            AgentMode::Plan
        }
    };

    if mode == AgentMode::Act && matches!(goal_workflow, GoalWorkflowState::Active { .. }) {
        AgentMode::Goal
    } else {
        mode
    }
}

#[derive(Debug, Clone)]
pub(super) struct PlanTurnPolicy {
    pub(super) mode: AgentMode,
    pub(super) stop_questions: bool,
    pub(super) next_workflow: PlanWorkflowState,
    pub(super) attach_plan: bool,
}

pub(super) fn plan_turn_policy(
    current: &PlanWorkflowState,
    requested_mode: AgentMode,
    control: Option<PlanControlInput>,
) -> std::result::Result<PlanTurnPolicy, String> {
    match current {
        PlanWorkflowState::Idle => match control {
            Some(PlanControlInput::StopQuestions) => {
                Err("no active plan workflow for this action".into())
            }
            Some(PlanControlInput::UpdatePlan) => Ok(PlanTurnPolicy {
                mode: AgentMode::Plan,
                stop_questions: false,
                next_workflow: PlanWorkflowState::PlanningQuestions,
                attach_plan: false,
            }),
            Some(PlanControlInput::ImplementPlan) => Ok(PlanTurnPolicy {
                mode: AgentMode::Act,
                stop_questions: false,
                next_workflow: PlanWorkflowState::Idle,
                attach_plan: false,
            }),
            None if requested_mode == AgentMode::Plan => Ok(PlanTurnPolicy {
                mode: AgentMode::Plan,
                stop_questions: false,
                next_workflow: PlanWorkflowState::PlanningQuestions,
                attach_plan: false,
            }),
            None if requested_mode == AgentMode::Goal => Ok(PlanTurnPolicy {
                mode: AgentMode::Goal,
                stop_questions: false,
                next_workflow: PlanWorkflowState::Idle,
                attach_plan: false,
            }),
            None => Ok(PlanTurnPolicy {
                mode: AgentMode::Act,
                stop_questions: false,
                next_workflow: PlanWorkflowState::Idle,
                attach_plan: false,
            }),
        },
        PlanWorkflowState::PlanningQuestions => match control {
            Some(PlanControlInput::ImplementPlan) => {
                Err("create the plan before implementing it".into())
            }
            Some(PlanControlInput::UpdatePlan) => Ok(PlanTurnPolicy {
                mode: AgentMode::Plan,
                stop_questions: false,
                next_workflow: PlanWorkflowState::PlanningQuestions,
                attach_plan: false,
            }),
            Some(PlanControlInput::StopQuestions) => Ok(PlanTurnPolicy {
                mode: AgentMode::Plan,
                stop_questions: true,
                next_workflow: PlanWorkflowState::PlanningQuestions,
                attach_plan: true,
            }),
            None => Ok(PlanTurnPolicy {
                mode: AgentMode::Plan,
                stop_questions: false,
                next_workflow: PlanWorkflowState::PlanningQuestions,
                attach_plan: false,
            }),
        },
        PlanWorkflowState::PlanReady { .. } => match control {
            Some(PlanControlInput::ImplementPlan) => Ok(PlanTurnPolicy {
                mode: AgentMode::Act,
                stop_questions: false,
                next_workflow: PlanWorkflowState::Idle,
                attach_plan: false,
            }),
            Some(PlanControlInput::UpdatePlan) => Ok(PlanTurnPolicy {
                mode: AgentMode::Plan,
                stop_questions: false,
                next_workflow: PlanWorkflowState::PlanningQuestions,
                attach_plan: false,
            }),
            Some(PlanControlInput::StopQuestions) => Ok(PlanTurnPolicy {
                mode: AgentMode::Plan,
                stop_questions: true,
                next_workflow: PlanWorkflowState::PlanningQuestions,
                attach_plan: true,
            }),
            None if requested_mode == AgentMode::Plan => Ok(PlanTurnPolicy {
                mode: AgentMode::Plan,
                stop_questions: false,
                next_workflow: PlanWorkflowState::PlanningQuestions,
                attach_plan: false,
            }),
            None => Err("plan is ready; choose update plan or implement plan".into()),
        },
    }
}

pub(super) fn plan_estimate_mode(
    current: &PlanWorkflowState,
    requested_mode: AgentMode,
) -> AgentMode {
    match current {
        PlanWorkflowState::Idle => requested_mode,
        PlanWorkflowState::PlanningQuestions | PlanWorkflowState::PlanReady { .. } => {
            AgentMode::Plan
        }
    }
}

pub(super) fn start_goal_workflow(objective: &str) -> GoalWorkflowState {
    let now = now_ms();
    GoalWorkflowState::Active {
        objective: objective.trim().to_string(),
        started_at_ms: now,
        updated_at_ms: now,
    }
}

pub(super) fn resume_goal_workflow(workflow: GoalWorkflowState) -> GoalWorkflowState {
    match workflow {
        GoalWorkflowState::Paused {
            objective,
            started_at_ms,
            ..
        } => GoalWorkflowState::Active {
            objective,
            started_at_ms,
            updated_at_ms: now_ms(),
        },
        current => current,
    }
}

pub(super) fn pause_goal_workflow(workflow: GoalWorkflowState) -> GoalWorkflowState {
    match workflow {
        GoalWorkflowState::Active {
            objective,
            started_at_ms,
            ..
        } => GoalWorkflowState::Paused {
            objective,
            started_at_ms,
            updated_at_ms: now_ms(),
        },
        current => current,
    }
}

pub(super) fn goal_workflow_duration_ms(workflow: &GoalWorkflowState) -> Option<i64> {
    match workflow {
        GoalWorkflowState::Active { started_at_ms, .. }
        | GoalWorkflowState::Paused { started_at_ms, .. } => {
            Some(now_ms().saturating_sub(*started_at_ms))
        }
        GoalWorkflowState::Complete {
            started_at_ms,
            completed_at_ms,
            ..
        } => Some(completed_at_ms.saturating_sub(*started_at_ms)),
        GoalWorkflowState::Idle => None,
    }
}
