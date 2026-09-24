use sinew_core::{EffortMode, ModelCapabilities, ModelRef};

pub const MODEL_ID: &str = "gpt-6-astra";
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
        id: "gpt-6-astra",
        context_window: 272_000,
        preferred_window: 240_000,
        max_output_tokens: 128_000,
        supports_images: true,
    },
    OpenAiModelInfo {
        id: "gpt-6-sol",
        context_window: 1_050_000,
        preferred_window: 950_000,
        max_output_tokens: 128_000,
        supports_images: true,
    },
    OpenAiModelInfo {
        id: "gpt-6-luna",
        context_window: 1_050_000,
        preferred_window: 950_000,
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

#[cfg(test)]
mod tests {
    use sinew_core::ModelRef;

    use super::capabilities;

    #[test]
    fn gpt_6_astra_uses_codex_account_capabilities() {
        let capabilities = capabilities(&ModelRef::new("openai", "gpt-6-astra"));

        assert_eq!(capabilities.context_window, 272_000);
        assert_eq!(capabilities.preferred_window, 240_000);
        assert_eq!(capabilities.max_output_tokens, 128_000);
        assert!(capabilities.supports_images);
    }

    #[test]
    fn gpt_6_sol_and_luna_use_expected_capabilities() {
        for model_id in ["gpt-6-sol", "gpt-6-luna"] {
            let capabilities = capabilities(&ModelRef::new("openai", model_id));

            assert_eq!(capabilities.context_window, 1_050_000, "{model_id}");
            assert_eq!(capabilities.preferred_window, 950_000, "{model_id}");
            assert_eq!(capabilities.max_output_tokens, 128_000, "{model_id}");
            assert!(capabilities.supports_images, "{model_id}");
        }
    }
}
