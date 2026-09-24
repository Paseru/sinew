use sinew_core::{EffortMode, ModelCapabilities, ModelRef};

pub const MODEL_ID: &str = "claude-opus-5-5";
pub const MODEL_WINDOW: u32 = 1_000_000;
pub const MODEL_MAX_OUTPUT: u32 = 128_000;

struct AnthropicModelInfo {
    id: &'static str,
    context_window: u32,
    preferred_window: u32,
    max_output_tokens: u32,
}

const MODELS: &[AnthropicModelInfo] = &[
    AnthropicModelInfo {
        id: "claude-opus-5-5",
        context_window: 1_000_000,
        preferred_window: 900_000,
        max_output_tokens: 128_000,
    },
    AnthropicModelInfo {
        id: "claude-fable-5-1",
        context_window: 1_000_000,
        preferred_window: 900_000,
        max_output_tokens: 128_000,
    },
    AnthropicModelInfo {
        id: "claude-fable-5",
        context_window: 1_000_000,
        preferred_window: 900_000,
        max_output_tokens: 128_000,
    },
    AnthropicModelInfo {
        id: "claude-sonnet-5",
        context_window: 1_000_000,
        preferred_window: 900_000,
        max_output_tokens: 128_000,
    },
];

fn model_info(model_id: &str) -> &'static AnthropicModelInfo {
    MODELS
        .iter()
        .find(|info| info.id == model_id)
        .unwrap_or(&MODELS[0])
}

pub fn capabilities(model: &ModelRef) -> ModelCapabilities {
    let info = model_info(&model.name);
    ModelCapabilities {
        model: model.clone(),
        context_window: info.context_window,
        preferred_window: info.preferred_window,
        max_output_tokens: info.max_output_tokens,
        supports_thinking: true,
        visible_thinking: true,
        supports_tools: true,
        supports_images: true,
        effort_mode: EffortMode::Tier,
    }
}
