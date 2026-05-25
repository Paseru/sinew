use super::*;

fn sample_plan_ready() -> PlanWorkflowState {
    PlanWorkflowState::PlanReady {
        artifact: PlanArtifactState {
            path: ".claakecode/plans/test.md".into(),
            absolute_path: Some("/workspace/.claakecode/plans/test.md".into()),
            title: Some("Test plan".into()),
            updated_at_ms: Some(1),
        },
    }
}

#[test]
fn plan_policy_starts_question_loop_from_idle_plan_mode() {
    let policy = plan_turn_policy(&PlanWorkflowState::Idle, AgentMode::Plan, None).unwrap();

    assert_eq!(policy.mode, AgentMode::Plan);
    assert!(!policy.stop_questions);
    assert!(!policy.attach_plan);
    assert_eq!(policy.next_workflow, PlanWorkflowState::PlanningQuestions);
}

#[test]
fn plan_policy_forces_plan_mode_while_questions_are_active() {
    let policy =
        plan_turn_policy(&PlanWorkflowState::PlanningQuestions, AgentMode::Act, None).unwrap();

    assert_eq!(policy.mode, AgentMode::Plan);
    assert_eq!(policy.next_workflow, PlanWorkflowState::PlanningQuestions);
}

#[test]
fn plan_policy_only_attaches_plan_after_stop_questions() {
    let policy = plan_turn_policy(
        &PlanWorkflowState::PlanningQuestions,
        AgentMode::Plan,
        Some(PlanControlInput::StopQuestions),
    )
    .unwrap();

    assert_eq!(policy.mode, AgentMode::Plan);
    assert!(policy.stop_questions);
    assert!(policy.attach_plan);
    assert_eq!(policy.next_workflow, PlanWorkflowState::PlanningQuestions);
}

#[test]
fn plan_policy_rejects_implementation_before_plan_exists() {
    let err = plan_turn_policy(
        &PlanWorkflowState::PlanningQuestions,
        AgentMode::Act,
        Some(PlanControlInput::ImplementPlan),
    )
    .unwrap_err();

    assert!(err.contains("create the plan"));
}

#[test]
fn plan_policy_allows_card_actions_after_manual_exit() {
    let update_policy = plan_turn_policy(
        &PlanWorkflowState::Idle,
        AgentMode::Act,
        Some(PlanControlInput::UpdatePlan),
    )
    .unwrap();
    assert_eq!(update_policy.mode, AgentMode::Plan);
    assert_eq!(
        update_policy.next_workflow,
        PlanWorkflowState::PlanningQuestions
    );

    let implement_policy = plan_turn_policy(
        &PlanWorkflowState::Idle,
        AgentMode::Act,
        Some(PlanControlInput::ImplementPlan),
    )
    .unwrap();
    assert_eq!(implement_policy.mode, AgentMode::Act);
    assert_eq!(implement_policy.next_workflow, PlanWorkflowState::Idle);
}

#[test]
fn plan_policy_rejects_act_mode_when_plan_is_ready_without_user_action() {
    let err = plan_turn_policy(&sample_plan_ready(), AgentMode::Act, None).unwrap_err();

    assert!(err.contains("choose update plan or implement plan"));
}

#[test]
fn plan_policy_allows_implementation_after_plan_is_ready() {
    let policy = plan_turn_policy(
        &sample_plan_ready(),
        AgentMode::Act,
        Some(PlanControlInput::ImplementPlan),
    )
    .unwrap();

    assert_eq!(policy.mode, AgentMode::Act);
    assert_eq!(policy.next_workflow, PlanWorkflowState::Idle);
    assert!(!policy.attach_plan);
}

#[test]
fn plan_policy_returns_to_question_loop_when_updating_ready_plan() {
    let policy = plan_turn_policy(
        &sample_plan_ready(),
        AgentMode::Plan,
        Some(PlanControlInput::UpdatePlan),
    )
    .unwrap();

    assert_eq!(policy.mode, AgentMode::Plan);
    assert_eq!(policy.next_workflow, PlanWorkflowState::PlanningQuestions);
}

#[test]
fn context_estimate_stays_in_plan_mode_for_active_workflows() {
    assert_eq!(
        plan_estimate_mode(&PlanWorkflowState::PlanningQuestions, AgentMode::Act),
        AgentMode::Plan
    );
    assert_eq!(
        plan_estimate_mode(&sample_plan_ready(), AgentMode::Act),
        AgentMode::Plan
    );
}

#[test]
fn rewritable_user_message_rejects_compaction_and_hidden_messages() {
    let normal = ChatMessage {
        role: Role::User,
        parts: vec![Part::Text {
            text: "change this".into(),
            meta: None,
        }],
    };
    let retained = ChatMessage {
        role: Role::User,
        parts: vec![Part::Text {
            text: "old user message".into(),
            meta: Some(json!({ "compaction_retained_user": true })),
        }],
    };
    let summary = ChatMessage {
        role: Role::User,
        parts: vec![Part::Text {
            text: "summary".into(),
            meta: Some(json!({ "compaction_summary": true })),
        }],
    };
    let reminder = ChatMessage {
        role: Role::User,
        parts: vec![Part::Text {
            text: "continue".into(),
            meta: Some(json!({ "system_reminder": true })),
        }],
    };

    assert!(is_rewritable_user_message(&normal));
    assert!(!is_rewritable_user_message(&retained));
    assert!(!is_rewritable_user_message(&summary));
    assert!(!is_rewritable_user_message(&reminder));
}

#[test]
fn plan_implementation_reminder_uses_ready_plan_artifact() {
    let reminder = plan_implementation_turn_reminder(
        Path::new("/workspace"),
        &sample_plan_ready(),
        &[],
        Some(PlanControlInput::ImplementPlan),
    )
    .unwrap()
    .unwrap();

    assert!(reminder.contains("Plan path: .claakecode/plans/test.md"));
    assert!(reminder.contains("Plan title: Test plan"));
    assert!(reminder.contains("current turn"));
    assert!(reminder.contains("Use the ToDoList tool"));
}

#[test]
fn plan_implementation_reminder_uses_attached_plan_after_context_clear() {
    let attachments = vec![AttachmentInput {
        path: "/workspace/.claakecode/plans/fresh.md".into(),
        name: Some("fresh.md".into()),
    }];
    let reminder = plan_implementation_turn_reminder(
        Path::new("/workspace"),
        &PlanWorkflowState::Idle,
        &attachments,
        Some(PlanControlInput::ImplementPlan),
    )
    .unwrap()
    .unwrap();

    assert!(reminder.contains("Plan path: .claakecode/plans/fresh.md"));
}

#[test]
fn plan_implementation_reminder_is_scoped_to_implement_control() {
    let reminder = plan_implementation_turn_reminder(
        Path::new("/workspace"),
        &sample_plan_ready(),
        &[],
        Some(PlanControlInput::UpdatePlan),
    )
    .unwrap();

    assert!(reminder.is_none());
}

#[test]
fn swarm_completion_event_extracts_structured_responses() {
    let event = AgentEvent::ToolFinished {
        id: "team-run".to_string(),
        output: "Agent Swarm finished".to_string(),
        is_error: false,
        file_changes: Vec::new(),
        images: Vec::new(),
        meta: Some(json!({
            "teamRunStatus": "completed",
            "team": { "name": "team-demo" },
            "agentFinalResponses": [
                {
                    "agent": "builder",
                    "status": "finished",
                    "lastResponse": "Built the feature."
                },
                {
                    "agent": "reviewer",
                    "status": "finished",
                    "lastResponse": "Reviewed the result."
                }
            ]
        })),
    };

    let completion = agent_swarm_completion_from_event(&event)
        .expect("completed TeamRun event should trigger a swarm completion wake");

    assert_eq!(completion.team_name, "team-demo");
    assert_eq!(completion.responses.len(), 2);
    assert_eq!(completion.responses[0].agent, "builder");
    assert_eq!(completion.responses[0].last_response, "Built the feature.");
}

#[test]
fn swarm_completion_wake_text_mentions_finished_and_agent_responses() {
    let completion = AgentSwarmCompletion {
        team_name: "team-demo".to_string(),
        responses: vec![AgentSwarmFinalResponse {
            agent: "builder".to_string(),
            status: "finished".to_string(),
            last_response: "Built the feature.".to_string(),
            last_error: None,
        }],
    };

    let wake_text = agent_swarm_completion_wake_text(&completion);

    assert!(wake_text.contains("<agent_swarm_finished>"));
    assert!(wake_text.contains("agent: @builder"));
    assert!(wake_text.contains("Built the feature."));
    assert!(wake_text.contains("Agent Swarm a terminé"));
}
