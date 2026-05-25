use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelRef {
    pub provider: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<Effort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_1m_context: Option<bool>,
}

impl ModelRef {
    pub fn new(provider: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            name: name.into(),
            effort: None,
            use_1m_context: None,
        }
    }

    pub fn with_effort(mut self, effort: Effort) -> Self {
        self.effort = Some(effort);
        self
    }

    pub fn with_use_1m_context(mut self, value: bool) -> Self {
        self.use_1m_context = Some(value);
        self
    }

    pub fn use_1m_context_enabled(&self) -> bool {
        self.use_1m_context.unwrap_or(false)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    None,
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffortMode {
    None,
    Budget,
    Tier,
    Flag,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub model: ModelRef,
    pub context_window: u32,
    pub preferred_window: u32,
    pub max_output_tokens: u32,
    pub supports_thinking: bool,
    pub visible_thinking: bool,
    pub supports_tools: bool,
    pub supports_images: bool,
    pub effort_mode: EffortMode,
}
