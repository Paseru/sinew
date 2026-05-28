use sinew_core::{EffortMode, ModelCapabilities, ModelRef};

pub const PROVIDER_ID: &str = "deepseek";
pub const DEEPSEEK_V4_FLASH_MODEL: &str = "deepseek-v4-flash";
pub const DEEPSEEK_V4_PRO_MODEL: &str = "deepseek-v4-pro";
pub const DEEPSEEK_CHAT_MODEL: &str = "deepseek-chat";
pub const DEEPSEEK_REASONER_MODEL: &str = "deepseek-reasoner";

pub fn capabilities(model: &ModelRef) -> ModelCapabilities {
    if model.name == DEEPSEEK_V4_FLASH_MODEL || model.name == DEEPSEEK_V4_PRO_MODEL {
        ModelCapabilities {
            model: model.clone(),
            context_window: 1_000_000,
            preferred_window: 800_000,
            max_output_tokens: 384_000,
            supports_thinking: true,
            visible_thinking: true,
            supports_tools: true,
            supports_images: false,
            effort_mode: EffortMode::Flag,
        }
    } else if model.name == DEEPSEEK_REASONER_MODEL {
        ModelCapabilities {
            model: model.clone(),
            context_window: 128_000,
            preferred_window: 120_000,
            max_output_tokens: 8192,
            supports_thinking: true,
            visible_thinking: true,
            supports_tools: false,
            supports_images: false,
            effort_mode: EffortMode::Flag,
        }
    } else {
        ModelCapabilities {
            model: model.clone(),
            context_window: 128_000,
            preferred_window: 120_000,
            max_output_tokens: 8192,
            supports_thinking: false,
            visible_thinking: false,
            supports_tools: true,
            supports_images: false,
            effort_mode: EffortMode::None,
        }
    }
}
