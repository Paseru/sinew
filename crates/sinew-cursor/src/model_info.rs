use sinew_core::{EffortMode, ModelCapabilities, ModelRef};

pub const PROVIDER_ID: &str = "cursor";
pub const MODEL_COMPOSER_25: &str = "composer-2.5";
pub const MODEL_COMPOSER_25_FAST: &str = "composer-2.5-fast";

struct CursorModelInfo {
    id: &'static str,
    context_window: u32,
    preferred_window: u32,
    max_output_tokens: u32,
}

const MODELS: &[CursorModelInfo] = &[CursorModelInfo {
    id: MODEL_COMPOSER_25_FAST,
    context_window: 272_000,
    preferred_window: 240_000,
    max_output_tokens: 128_000,
}, CursorModelInfo {
    id: MODEL_COMPOSER_25,
    context_window: 272_000,
    preferred_window: 240_000,
    max_output_tokens: 128_000,
}];

fn model_info(model_id: &str) -> &'static CursorModelInfo {
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
