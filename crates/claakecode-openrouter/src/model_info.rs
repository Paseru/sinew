use claakecode_core::{EffortMode, ModelCapabilities, ModelRef};

use crate::client::OpenRouterCatalogModel;

pub const PROVIDER_ID: &str = "openrouter";

pub fn capabilities_from_catalog_model(model: &OpenRouterCatalogModel) -> ModelCapabilities {
    capabilities_from_parts(
        &model.id,
        model.context_window,
        model.max_output_tokens,
        model.supports_images,
        model.supports_thinking,
        model.supports_tools,
    )
}

pub fn capabilities_from_parts(
    id: &str,
    context_window: u32,
    max_output_tokens: u32,
    supports_images: bool,
    supports_thinking: bool,
    supports_tools: bool,
) -> ModelCapabilities {
    let context_window = context_window.max(1);
    let max_output_tokens = max_output_tokens.max(1).min(context_window);
    ModelCapabilities {
        model: ModelRef::new(PROVIDER_ID, id),
        context_window,
        preferred_window: preferred_window(context_window),
        max_output_tokens,
        supports_thinking,
        visible_thinking: supports_thinking,
        supports_tools,
        supports_images,
        effort_mode: if supports_thinking {
            EffortMode::Tier
        } else {
            EffortMode::None
        },
    }
}

fn preferred_window(context_window: u32) -> u32 {
    ((context_window as u64 * 9) / 10)
        .max(1)
        .min(u32::MAX as u64) as u32
}
