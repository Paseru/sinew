use claakecode_core::{EffortMode, ModelCapabilities, ModelRef};

pub const MODEL_ID: &str = "gpt-5.5";
pub const MODEL_WINDOW: u32 = 272_000;
pub const MODEL_MAX_OUTPUT: u32 = 128_000;

struct OpenAiModelInfo {
    id: &'static str,
    context_window: u32,
    preferred_window: u32,
    max_output_tokens: u32,
    supports_images: bool,
}

const MODELS: &[OpenAiModelInfo] = &[
    OpenAiModelInfo {
        id: "gpt-5.5",
        context_window: 272_000,
        preferred_window: 240_000,
        max_output_tokens: 128_000,
        supports_images: true,
    },
    OpenAiModelInfo {
        id: "gpt-5.4",
        context_window: 1_050_000,
        preferred_window: 950_000,
        max_output_tokens: 128_000,
        supports_images: true,
    },
    OpenAiModelInfo {
        id: "gpt-5.4-mini",
        context_window: 400_000,
        preferred_window: 360_000,
        max_output_tokens: 128_000,
        supports_images: true,
    },
    OpenAiModelInfo {
        id: "gpt-5.3-codex",
        context_window: 400_000,
        preferred_window: 360_000,
        max_output_tokens: 128_000,
        supports_images: true,
    },
    OpenAiModelInfo {
        id: "gpt-5.3-codex-spark",
        context_window: 128_000,
        preferred_window: 115_000,
        max_output_tokens: 128_000,
        supports_images: false,
    },
    OpenAiModelInfo {
        id: "gpt-5.2",
        context_window: 400_000,
        preferred_window: 360_000,
        max_output_tokens: 128_000,
        supports_images: true,
    },
];

fn model_info(model_id: &str) -> &'static OpenAiModelInfo {
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
        supports_images: info.supports_images,
        effort_mode: EffortMode::Tier,
    }
}
