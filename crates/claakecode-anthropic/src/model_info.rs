use claakecode_core::{EffortMode, ModelCapabilities, ModelRef};

pub const MODEL_ID: &str = "claude-opus-4-7";
pub const MODEL_WINDOW: u32 = 1_000_000;
pub const MODEL_MAX_OUTPUT: u32 = 128_000;

struct AnthropicModelInfo {
    id: &'static str,
    context_window: u32,
    preferred_window: u32,
    max_output_tokens: u32,
    beta_1m_context_window: Option<u32>,
    beta_1m_preferred_window: Option<u32>,
}

const MODELS: &[AnthropicModelInfo] = &[
    AnthropicModelInfo {
        id: "claude-opus-4-7",
        context_window: 1_000_000,
        preferred_window: 900_000,
        max_output_tokens: 128_000,
        beta_1m_context_window: None,
        beta_1m_preferred_window: None,
    },
    AnthropicModelInfo {
        id: "claude-opus-4-6",
        context_window: 1_000_000,
        preferred_window: 900_000,
        max_output_tokens: 128_000,
        beta_1m_context_window: None,
        beta_1m_preferred_window: None,
    },
    AnthropicModelInfo {
        id: "claude-sonnet-4-6",
        context_window: 200_000,
        preferred_window: 180_000,
        max_output_tokens: 128_000,
        beta_1m_context_window: Some(1_000_000),
        beta_1m_preferred_window: Some(900_000),
    },
    AnthropicModelInfo {
        id: "claude-haiku-4-5",
        context_window: 200_000,
        preferred_window: 180_000,
        max_output_tokens: 64_000,
        beta_1m_context_window: None,
        beta_1m_preferred_window: None,
    },
];

fn model_info(model_id: &str) -> &'static AnthropicModelInfo {
    MODELS
        .iter()
        .find(|info| info.id == model_id)
        .unwrap_or(&MODELS[0])
}

pub fn supports_1m_context_beta(model_name: &str) -> bool {
    model_info(model_name).beta_1m_context_window.is_some()
}

pub fn capabilities(model: &ModelRef) -> ModelCapabilities {
    let info = model_info(&model.name);
    let use_1m = model.use_1m_context_enabled();
    let (context_window, preferred_window) = if use_1m {
        (
            info.beta_1m_context_window.unwrap_or(info.context_window),
            info.beta_1m_preferred_window.unwrap_or(info.preferred_window),
        )
    } else {
        (info.context_window, info.preferred_window)
    };
    ModelCapabilities {
        model: model.clone(),
        context_window,
        preferred_window,
        max_output_tokens: info.max_output_tokens,
        supports_thinking: true,
        visible_thinking: true,
        supports_tools: true,
        supports_images: true,
        effort_mode: EffortMode::Tier,
    }
}
