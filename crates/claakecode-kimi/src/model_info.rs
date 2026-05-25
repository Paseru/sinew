use claakecode_core::{EffortMode, ModelCapabilities, ModelRef};

pub const MODEL_ID: &str = "kimi-for-coding";
pub const MODEL_WINDOW: u32 = 256_000;
pub const MODEL_MAX_OUTPUT: u32 = 32_000;

pub fn capabilities(model: &ModelRef) -> ModelCapabilities {
    ModelCapabilities {
        model: model.clone(),
        context_window: MODEL_WINDOW,
        preferred_window: 230_000,
        max_output_tokens: MODEL_MAX_OUTPUT,
        supports_thinking: true,
        visible_thinking: true,
        supports_tools: true,
        supports_images: true,
        effort_mode: EffortMode::Flag,
    }
}
