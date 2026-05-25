use async_trait::async_trait;

use crate::{
    error::Result,
    message::ChatMessage,
    model::{Effort, ModelCapabilities, ModelRef},
    stream::ProviderStream,
    tool::ToolDescriptor,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceTier {
    Fast,
    Flex,
}

#[derive(Debug, Clone)]
pub struct ProviderRequest {
    pub model: ModelRef,
    pub system_prompt: Option<String>,
    pub transcript: Vec<ChatMessage>,
    pub tools: Vec<ToolDescriptor>,
    pub max_output_tokens: Option<u32>,
    pub effort: Option<Effort>,
    pub temperature: Option<f32>,
    pub cache_key: Option<String>,
    pub cache_stable_message_count: Option<usize>,
    pub service_tier: Option<ServiceTier>,
}

#[derive(Debug, Clone, Copy)]
pub struct TokenEstimate {
    pub input_tokens: u32,
    pub exact: bool,
}

impl ProviderRequest {
    pub fn new(model: ModelRef, transcript: Vec<ChatMessage>) -> Self {
        Self {
            model,
            system_prompt: None,
            transcript,
            tools: Vec::new(),
            max_output_tokens: None,
            effort: None,
            temperature: None,
            cache_key: None,
            cache_stable_message_count: None,
            service_tier: None,
        }
    }

    pub fn with_system(mut self, value: impl Into<String>) -> Self {
        self.system_prompt = Some(value.into());
        self
    }

    pub fn with_tools(mut self, tools: Vec<ToolDescriptor>) -> Self {
        self.tools = tools;
        self
    }

    pub fn with_effort(mut self, effort: Effort) -> Self {
        self.effort = Some(effort);
        self
    }

    pub fn with_cache_key(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        if !value.trim().is_empty() {
            self.cache_key = Some(value);
        }
        self
    }

    pub fn with_cache_stable_message_count(mut self, value: usize) -> Self {
        self.cache_stable_message_count = Some(value);
        self
    }

    pub fn with_service_tier(mut self, value: ServiceTier) -> Self {
        self.service_tier = Some(value);
        self
    }

    pub fn effective_effort(&self) -> Option<Effort> {
        self.effort.or(self.model.effort)
    }
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn capabilities(&self, model: &ModelRef) -> Option<ModelCapabilities>;
    async fn estimate_tokens(&self, request: ProviderRequest) -> Result<TokenEstimate>;
    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream>;
}
