use super::*;

fn sample_plan_ready() -> PlanWorkflowState {
    PlanWorkflowState::PlanReady {
        artifact: PlanArtifactState {
            path: ".sinew/plans/test.md".into(),
            absolute_path: Some("/workspace/.sinew/plans/test.md".into()),
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
fn display_mode_prompt_is_added_only_for_compact_modes() {
    let base = "base prompt";

    assert_eq!(
        with_display_mode_prompt(base, DisplayModeInput::Disabled),
        base
    );

    let compact = with_display_mode_prompt(base, DisplayModeInput::Compact);
    assert!(compact.contains("Display mode: Compact"));
    assert!(compact.contains("concise"));

    let very_compact = with_display_mode_prompt(base, DisplayModeInput::VeryCompact);
    assert!(very_compact.contains("Display mode: Very compact"));
    assert!(very_compact.contains("Before the final answer"));
    assert!(very_compact.contains("ultra-concise"));
}

#[test]
fn minimal_thinking_level_maps_to_none_effort() {
    assert_eq!(ThinkingLevelInput::Minimal.into_effort(), Effort::None);
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

    assert!(reminder.contains("Plan path: .sinew/plans/test.md"));
    assert!(reminder.contains("Plan title: Test plan"));
    assert!(reminder.contains("current turn"));
    assert!(reminder.contains("Use the ToDoList tool"));
}

#[test]
fn plan_implementation_reminder_uses_attached_plan_after_context_clear() {
    let attachments = vec![AttachmentInput {
        path: "/workspace/.sinew/plans/fresh.md".into(),
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

    assert!(reminder.contains("Plan path: .sinew/plans/fresh.md"));
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

// Test de latence réel (réseau) : compare DeepSeek V4 Flash et Gemini 3.5 Flash
// sur la tâche d'optimisation de prompt. Ignoré par défaut car il appelle les
// vraies API avec les identifiants locaux.
// Lancer avec : cargo test -p Sinew flash_optimizer_race -- --ignored --nocapture
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn flash_optimizer_race() {
    use futures::StreamExt;
    use sinew_core::provider::{Provider, ProviderRequest};

    let draft = "ajoute un bouton pour exporter mes conversations en pdf et previens moi quand cest fini";
    let system = "Tu es un Prompt Engineer. Réécris le brouillon de l'utilisateur en une consigne claire. Réponds par: MODE: act puis une nouvelle ligne ===PROMPT=== puis le texte réécrit.";

    async fn time_model(
        provider: std::sync::Arc<dyn Provider>,
        model: sinew_core::ModelRef,
        system: &str,
        draft: &str,
    ) -> (String, std::time::Duration, std::time::Duration, usize) {
        let label = format!("{}:{}", model.provider, model.name);
        let messages = vec![sinew_core::message::ChatMessage::user_text(draft.to_string())];
        let request = ProviderRequest::new(model, messages).with_system(system.to_string());
        let start = std::time::Instant::now();
        let mut first_token: Option<std::time::Duration> = None;
        let mut chars = 0usize;
        match provider.stream(request).await {
            Ok(mut stream) => {
                while let Some(event) = stream.next().await {
                    if let Ok(sinew_core::stream::StreamEvent::TextDelta { delta, .. }) = event {
                        if first_token.is_none() {
                            first_token = Some(start.elapsed());
                        }
                        chars += delta.chars().count();
                    }
                }
            }
            Err(e) => {
                eprintln!("[{label}] ERREUR stream: {e}");
            }
        }
        (
            label,
            first_token.unwrap_or_else(|| start.elapsed()),
            start.elapsed(),
            chars,
        )
    }

    let deepseek = std::sync::Arc::new(
        tokio::task::spawn_blocking(|| {
            sinew_deepseek::DeepSeekProvider::from_default_sources()
                .expect("DeepSeek non configuré (deepseek-auth.json manquant)")
        })
        .await
        .unwrap(),
    ) as std::sync::Arc<dyn Provider>;
    let google = std::sync::Arc::new(
        tokio::task::spawn_blocking(|| {
            sinew_google::GoogleProvider::from_default_sources()
                .expect("Google non configuré (google-auth.json manquant)")
        })
        .await
        .unwrap(),
    ) as std::sync::Arc<dyn Provider>;

    let ds_model = sinew_core::ModelRef {
        provider: "deepseek".to_string(),
        name: "deepseek-v4-flash".to_string(),
        effort: Some(sinew_core::model::Effort::None),
    };
    let g_model = sinew_core::ModelRef {
        provider: "google".to_string(),
        name: "gemini-3.5-flash".to_string(),
        effort: Some(sinew_core::model::Effort::None),
    };

    // Course concurrente : les deux modèles partent en même temps.
    let (ds, g) = tokio::join!(
        time_model(deepseek, ds_model, system, draft),
        time_model(google, g_model, system, draft),
    );

    for (label, ttft, total, chars) in [&ds, &g] {
        println!(
            "[{label}] 1er token: {:>7.0} ms | total: {:>7.0} ms | {chars} caractères",
            ttft.as_secs_f64() * 1000.0,
            total.as_secs_f64() * 1000.0,
        );
    }

    let winner_ttft = if ds.1 <= g.1 { &ds.0 } else { &g.0 };
    let winner_total = if ds.2 <= g.2 { &ds.0 } else { &g.0 };
    println!("==> 1er token le plus rapide : {winner_ttft}");
    println!("==> réponse complète la plus rapide : {winner_total}");
}
