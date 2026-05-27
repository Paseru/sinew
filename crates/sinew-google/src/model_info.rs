use sinew_core::{Effort, EffortMode, ModelCapabilities, ModelRef};

pub const MODEL_ID: &str = "gemini-3.1-pro";
pub const GEMINI_WINDOW: u32 = 1_048_576;
pub const GEMINI_MAX_OUTPUT: u32 = 65_535;

struct GoogleModelInfo {
    id: &'static str,
    context_window: u32,
    preferred_window: u32,
    max_output_tokens: u32,
    supports_images: bool,
}

const MODELS: &[GoogleModelInfo] = &[
    GoogleModelInfo {
        id: "gemini-3.1-pro",
        context_window: GEMINI_WINDOW,
        preferred_window: 950_000,
        max_output_tokens: GEMINI_MAX_OUTPUT,
        supports_images: true,
    },
    GoogleModelInfo {
        id: "gemini-3-flash",
        context_window: GEMINI_WINDOW,
        preferred_window: 950_000,
        max_output_tokens: GEMINI_MAX_OUTPUT,
        supports_images: true,
    },
    GoogleModelInfo {
        id: "gemini-3.5-flash",
        context_window: GEMINI_WINDOW,
        preferred_window: 950_000,
        max_output_tokens: GEMINI_MAX_OUTPUT,
        supports_images: true,
    },
    GoogleModelInfo {
        id: "gemini-3.1-flash-lite",
        context_window: GEMINI_WINDOW,
        preferred_window: 950_000,
        max_output_tokens: GEMINI_MAX_OUTPUT,
        supports_images: true,
    },
    GoogleModelInfo {
        id: "claude-opus-4.6",
        context_window: 200_000,
        preferred_window: 180_000,
        max_output_tokens: 8192,
        supports_images: true,
    },
    GoogleModelInfo {
        id: "claude-sonnet-4.6",
        context_window: 200_000,
        preferred_window: 180_000,
        max_output_tokens: 8192,
        supports_images: true,
    },
    GoogleModelInfo {
        id: "gpt-oss-120b",
        context_window: 128_000,
        preferred_window: 100_000,
        max_output_tokens: 4096,
        supports_images: false,
    },
    GoogleModelInfo {
        id: "gemini-2.5-pro",
        context_window: GEMINI_WINDOW,
        preferred_window: 950_000,
        max_output_tokens: GEMINI_MAX_OUTPUT,
        supports_images: true,
    },
];

fn model_info(model_id: &str) -> &'static GoogleModelInfo {
    MODELS
        .iter()
        .find(|info| info.id == model_id)
        .unwrap_or(&MODELS[0])
}

fn is_known_model(model_id: &str) -> bool {
    MODELS.iter().any(|info| info.id == model_id)
}

pub fn canonical_model(model: &ModelRef) -> ModelRef {
    let mut canonical = model.clone();
    if !is_known_model(&canonical.name) {
        canonical.name = MODEL_ID.into();
    }
    canonical
}

pub fn antigravity_model_and_thinking(
    model: &ModelRef,
    effort: Option<Effort>,
) -> (String, Option<&'static str>) {
    let base = canonical_model(model).name;
    let requested = effort.or(model.effort).unwrap_or(Effort::High);
    let is_pro = is_gemini_pro_model(&base);
    let thinking_level = match requested {
        // Antigravity's pro models do not accept `minimal`; clamp them to low.
        Effort::None => {
            if is_pro {
                "low"
            } else {
                "minimal"
            }
        }
        Effort::Low => "low",
        Effort::Medium => "medium",
        Effort::High | Effort::Xhigh | Effort::Max => "high",
    };

    // Antigravity exposes 3.5-flash with separate low/medium/high IDs matching your plan's quotas.
    if base == "gemini-3.5-flash" {
        let model_id = match thinking_level {
            "minimal" | "low" => "gemini-3.5-flash-low",
            "medium" => "gemini-3.5-flash-medium",
            _ => "gemini-3.5-flash-high",
        };
        return (model_id.into(), Some(thinking_level));
    }
    // Gemini 3.1 Pro on Antigravity is routed to either -low or -high to leverage your active quotas.
    if base == "gemini-3.1-pro" {
        let model_id = match thinking_level {
            "minimal" | "low" => "gemini-3.1-pro-low",
            _ => "gemini-3.1-pro-high",
        };
        return (model_id.into(), Some(thinking_level));
    }
    if is_pro {
        (format!("{base}-{thinking_level}"), Some(thinking_level))
    } else {
        (base, Some(thinking_level))
    }
}

pub fn capabilities(model: &ModelRef) -> ModelCapabilities {
    let model = canonical_model(model);
    let info = model_info(&model.name);
    ModelCapabilities {
        model,
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

pub fn is_gemini3_model(model_id: &str) -> bool {
    let lower = model_id.to_ascii_lowercase();
    // Antigravity exposes several aliases for Gemini 3.x family. They all
    // share the same thought_signature / multimodal function response
    // requirements, so treat them uniformly.
    lower.contains("gemini-3")
        || lower == "gemini-pro-agent"
        || lower.starts_with("gemini-pro-agent")
}

fn is_gemini_pro_model(model_id: &str) -> bool {
    model_id.to_ascii_lowercase().contains("-pro")
}
