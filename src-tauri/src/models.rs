use crate::*;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OpenAiProviderStatus {
    pub(super) connected: bool,
    pub(super) connection_state: String,
    pub(super) email: Option<String>,
    pub(super) account_id: Option<String>,
    pub(super) plan_type: Option<String>,
    pub(super) expires_at_ms: Option<i64>,
    pub(super) last_refresh_ms: Option<i64>,
    pub(super) login_id: Option<String>,
    pub(super) error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StartOpenAiLoginOutput {
    pub(super) login_id: String,
    pub(super) auth_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AnthropicProviderStatus {
    pub(super) connected: bool,
    pub(super) connection_state: String,
    pub(super) expires_at_ms: Option<i64>,
    pub(super) last_refresh_ms: Option<i64>,
    pub(super) login_id: Option<String>,
    pub(super) error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StartAnthropicLoginOutput {
    pub(super) login_id: String,
    pub(super) auth_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GoogleProviderStatus {
    pub(super) connected: bool,
    pub(super) connection_state: String,
    pub(super) email: Option<String>,
    pub(super) project_id: Option<String>,
    pub(super) user_tier: Option<String>,
    pub(super) expires_at_ms: Option<i64>,
    pub(super) last_refresh_ms: Option<i64>,
    pub(super) login_id: Option<String>,
    pub(super) error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StartGoogleLoginOutput {
    pub(super) login_id: String,
    pub(super) auth_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct KimiProviderStatus {
    pub(super) connected: bool,
    pub(super) connection_state: String,
    pub(super) expires_at_ms: Option<i64>,
    pub(super) last_refresh_ms: Option<i64>,
    pub(super) login_id: Option<String>,
    pub(super) error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StartKimiLoginOutput {
    pub(super) login_id: String,
    pub(super) auth_url: String,
    pub(super) user_code: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OpenRouterProviderStatus {
    pub(super) connected: bool,
    pub(super) connection_state: String,
    pub(super) key_preview: Option<String>,
    pub(super) last_validated_ms: Option<i64>,
    pub(super) model_count: usize,
    pub(super) error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkspaceInput {
    pub(super) workspace_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkspaceEntriesInput {
    pub(super) workspace_path: String,
    pub(super) relative_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkspaceFileInput {
    pub(super) workspace_path: String,
    pub(super) relative_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RestoreWorkspaceDeletedEntriesInput {
    pub(super) workspace_path: String,
    pub(super) entries: Vec<WorkspaceDeletedEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkspaceSearchInput {
    pub(super) workspace_path: String,
    pub(super) query: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WriteWorkspaceFileInput {
    pub(super) workspace_path: String,
    pub(super) relative_path: String,
    pub(super) content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CreateWorkspaceEntryInput {
    pub(super) workspace_path: String,
    pub(super) target_relative_path: Option<String>,
    pub(super) name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RenameWorkspaceEntryInput {
    pub(super) workspace_path: String,
    pub(super) relative_path: String,
    pub(super) new_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CopyWorkspaceEntriesInput {
    pub(super) workspace_path: String,
    pub(super) target_relative_path: Option<String>,
    pub(super) sources: Vec<String>,
    pub(super) cut: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ConversationInput {
    pub(super) workspace_path: String,
    pub(super) conversation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AnswerQuestionInput {
    pub(super) workspace_path: String,
    pub(super) conversation_id: String,
    pub(super) tool_call_id: String,
    #[serde(default)]
    pub(super) answers: Vec<Vec<String>>,
    #[serde(default)]
    pub(super) stop_questions: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RejectQuestionInput {
    pub(super) workspace_path: String,
    pub(super) conversation_id: String,
    pub(super) tool_call_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ActiveTurnReplayInput {
    pub(super) workspace_path: String,
    pub(super) conversation_id: String,
    #[serde(default)]
    pub(super) after_sequence: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ActiveTurnSummary {
    pub(super) workspace_id: String,
    pub(super) conversation_id: String,
    pub(super) started_at_ms: i64,
    pub(super) latest_sequence: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ActiveTurnsChangedPayload {
    pub(super) active_turns: Vec<ActiveTurnSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ActiveTurnReplay {
    pub(super) active: bool,
    pub(super) workspace_id: String,
    pub(super) conversation_id: String,
    pub(super) started_at_ms: Option<i64>,
    pub(super) latest_sequence: u64,
    pub(super) events: Vec<SequencedAgentEvent>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StopAgentSwarmInput {
    pub(super) workspace_path: String,
    pub(super) conversation_id: String,
    pub(super) team_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RenameConversationInput {
    pub(super) workspace_path: String,
    pub(super) conversation_id: String,
    pub(super) title: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct AttachmentInput {
    pub(super) path: String,
    pub(super) name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ClipboardImageInput {
    pub(super) workspace_path: String,
    pub(super) name: Option<String>,
    pub(super) media_type: String,
    pub(super) data: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ClipboardImageAttachment {
    pub(super) path: String,
    pub(super) name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SendMessageInput {
    pub(super) workspace_path: String,
    pub(super) conversation_id: String,
    pub(super) text: String,
    #[serde(default)]
    pub(super) attachments: Vec<AttachmentInput>,
    pub(super) model: Option<ModelInput>,
    pub(super) thinking: Option<ThinkingLevelInput>,
    #[serde(default)]
    pub(super) use_1m_context: Option<bool>,
    pub(super) mode: Option<AgentModeInput>,
    pub(super) service_tier: Option<ServiceTierInput>,
    pub(super) plan_control: Option<PlanControlInput>,
    pub(super) message_visibility: Option<MessageVisibilityInput>,
    #[serde(default)]
    pub(super) rewrite_from_history_index: Option<usize>,
    #[serde(default = "default_true")]
    pub(super) revert_workspace_changes: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CompactConversationInput {
    pub(super) workspace_path: String,
    pub(super) conversation_id: String,
    pub(super) model: Option<ModelInput>,
    pub(super) thinking: Option<ThinkingLevelInput>,
    pub(super) service_tier: Option<ServiceTierInput>,
    #[serde(default)]
    pub(super) use_1m_context: Option<bool>,
    #[serde(default)]
    pub(super) instruction: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ContextEstimateInput {
    pub(super) workspace_path: String,
    pub(super) conversation_id: String,
    #[serde(default)]
    pub(super) text: String,
    #[serde(default)]
    pub(super) attachments: Vec<AttachmentInput>,
    pub(super) model: Option<ModelInput>,
    pub(super) thinking: Option<ThinkingLevelInput>,
    #[serde(default)]
    pub(super) use_1m_context: Option<bool>,
    pub(super) mode: Option<AgentModeInput>,
    #[serde(default)]
    pub(super) rewrite_from_history_index: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SubAgentContextEstimateInput {
    pub(super) workspace_path: String,
    pub(super) agent_id: String,
    #[serde(default)]
    pub(super) agent_name: Option<String>,
    pub(super) history: Vec<ChatMessage>,
    pub(super) model: ModelRef,
    pub(super) mode: Option<AgentModeInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ConversationModeInput {
    pub(super) workspace_path: String,
    pub(super) conversation_id: String,
    pub(super) mode: AgentModeInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ConversationModelPreferenceInput {
    pub(super) workspace_path: String,
    pub(super) conversation_id: String,
    pub(super) mode: AgentModeInput,
    pub(super) model: Option<ModelInput>,
    pub(super) thinking: Option<ThinkingLevelInput>,
    #[serde(default)]
    pub(super) use_1m_context: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ContextEstimateOutput {
    pub(super) used_tokens: u32,
    pub(super) context_window: u32,
    pub(super) preferred_window: u32,
    pub(super) max_output_tokens: u32,
    pub(super) input_tokens: u32,
    pub(super) output_tokens: u32,
    pub(super) reasoning_tokens: u32,
    pub(super) cache_read_tokens: u32,
    pub(super) cache_creation_tokens: u32,
    pub(super) exact: bool,
    pub(super) error: Option<String>,
    pub(super) breakdown: Vec<ContextBreakdownItem>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ContextTokenUsage {
    pub(super) input_tokens: u32,
    pub(super) output_tokens: u32,
    pub(super) reasoning_tokens: u32,
    pub(super) cache_read_tokens: u32,
    pub(super) cache_creation_tokens: u32,
    pub(super) total_tokens: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ContextBreakdownItem {
    pub(super) key: String,
    pub(super) label: String,
    pub(super) tokens: u32,
}

#[derive(Debug, Clone)]
pub(super) struct ContextBreakdownWeight {
    pub(super) key: &'static str,
    pub(super) label: &'static str,
    pub(super) weight: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SaveMcpSettingsInput {
    pub(super) settings: McpSettings,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SaveToolSettingsInput {
    pub(super) workspace_path: String,
    pub(super) settings: ToolSettings,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SaveDatabaseSettingsInput {
    pub(super) settings: DatabaseSettings,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TestDatabaseConnectionInput {
    pub(super) source: DatabaseSourceConfig,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ListDatabaseSourceActivityInput {
    pub(super) source_id: String,
    #[serde(default)]
    pub(super) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ClearDatabaseSourceActivityInput {
    pub(super) source_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DatabaseSourcesChangedPayload {
    pub(super) active_count: usize,
    pub(super) source_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SaveSkillSettingsInput {
    pub(super) workspace_path: String,
    pub(super) settings: SkillSettings,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SaveSubAgentSettingsInput {
    pub(super) settings: SubAgentSettings,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ValidateOpenRouterApiKeyInput {
    pub(super) api_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SearchOpenRouterModelsInput {
    pub(super) query: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AddOpenRouterModelInput {
    pub(super) model: OpenRouterModelCandidateInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RemoveOpenRouterModelInput {
    pub(super) id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OpenRouterModelCandidateInput {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) context_window: u32,
    pub(super) max_output_tokens: u32,
    #[serde(default)]
    pub(super) supports_images: bool,
    #[serde(default)]
    pub(super) supports_thinking: bool,
    #[serde(default = "default_true")]
    pub(super) supports_tools: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ModelInput {
    pub(super) provider: String,
    pub(super) name: String,
    #[serde(default)]
    pub(super) use_1m_context: Option<bool>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TerminalCommandInput {
    pub(super) workspace_path: String,
    pub(super) command: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TerminalCommandOutput {
    pub(super) content: String,
    pub(super) is_error: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TerminalSpawnInput {
    pub(super) workspace_path: String,
    pub(super) session_id: String,
    pub(super) token: String,
    pub(super) cols: u16,
    pub(super) rows: u16,
    #[serde(default)]
    pub(super) pixel_width: u16,
    #[serde(default)]
    pub(super) pixel_height: u16,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TerminalSpawnOutput {
    pub(super) session_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TerminalWriteInput {
    pub(super) session_id: String,
    pub(super) token: String,
    pub(super) data: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TerminalResizeInput {
    pub(super) session_id: String,
    pub(super) token: String,
    pub(super) cols: u16,
    pub(super) rows: u16,
    #[serde(default)]
    pub(super) pixel_width: u16,
    #[serde(default)]
    pub(super) pixel_height: u16,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TerminalControlInput {
    pub(super) session_id: String,
    pub(super) token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OpenExternalUrlInput {
    pub(super) url: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct TerminalDataEvent {
    pub(super) session_id: String,
    pub(super) token: String,
    pub(super) data: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct TerminalExitEvent {
    pub(super) session_id: String,
    pub(super) token: String,
    pub(super) exit_code: Option<u32>,
    pub(super) signal: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(super) enum AgentModeInput {
    Act,
    Plan,
    Goal,
}

impl From<AgentModeInput> for AgentMode {
    fn from(value: AgentModeInput) -> Self {
        match value {
            AgentModeInput::Act => AgentMode::Act,
            AgentModeInput::Plan => AgentMode::Plan,
            AgentModeInput::Goal => AgentMode::Goal,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(super) enum ServiceTierInput {
    Fast,
}

impl From<ServiceTierInput> for ServiceTier {
    fn from(value: ServiceTierInput) -> Self {
        match value {
            ServiceTierInput::Fast => ServiceTier::Fast,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) enum PlanControlInput {
    StopQuestions,
    UpdatePlan,
    ImplementPlan,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) enum MessageVisibilityInput {
    Normal,
    SystemReminder,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub(super) enum ThinkingLevelInput {
    Off,
    Low,
    Medium,
    High,
    Max,
    Xhigh,
}

impl ThinkingLevelInput {
    pub(super) fn into_effort(self) -> Effort {
        match self {
            Self::Off => Effort::None,
            Self::Low => Effort::Low,
            Self::Medium => Effort::Medium,
            Self::High => Effort::High,
            Self::Xhigh => Effort::Xhigh,
            Self::Max => Effort::Max,
        }
    }
}
