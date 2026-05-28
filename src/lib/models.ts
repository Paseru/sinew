import type {
  AgentMode,
  ModeModelSettings,
  ModelRef,
  OpenRouterModel,
  ThinkingLevel,
} from "../types";

export type ModelId = string;
export type ProviderId = "anthropic" | "openai" | "google" | "kimi" | "openrouter" | "cursor" | "deepseek";
export type ModeModelSelection = { model: ModelId; thinking: ThinkingLevel };
export type ModeModelSelections = Record<AgentMode, ModeModelSelection>;

export type ModelEntry = {
  value: ModelId;
  provider: ProviderId;
  label: string;
  thinking: readonly ThinkingLevel[];
  defaultThinking: ThinkingLevel;
  supportsFast?: boolean;
  defaultFast?: boolean;
};

export const PROVIDERS: {
  value: ProviderId;
  label: string;
  icon: string;
}[] = [
  {
    value: "anthropic",
    label: "Anthropic",
    icon: "simple-icons:anthropic",
  },
  {
    value: "openai",
    label: "OpenAI",
    icon: "simple-icons:openai",
  },
  {
    value: "google",
    label: "Google",
    icon: "simple-icons:google",
  },
  {
    value: "kimi",
    label: "Kimi",
    icon: "local:kimi",
  },
  {
    value: "cursor",
    label: "Cursor",
    icon: "local:cursor",
  },
  {
    value: "deepseek",
    label: "DeepSeek",
    icon: "simple-icons:deepseek",
  },
  {
    value: "openrouter",
    label: "OpenRouter",
    icon: "simple-icons:openrouter",
  },
];

export const THINKING_LEVELS: { value: ThinkingLevel; label: string }[] = [
  { value: "off", label: "Off" },
  { value: "minimal", label: "Minimal" },
  { value: "low", label: "Low" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High" },
  { value: "xhigh", label: "XHigh" },
  { value: "max", label: "Max" },
];

export const MODELS: ModelEntry[] = [
  {
    value: "anthropic:claude-opus-4-7",
    provider: "anthropic",
    label: "Opus 4.7",
    thinking: ["off", "low", "medium", "high", "xhigh", "max"],
    defaultThinking: "medium",
  },
  {
    value: "anthropic:claude-opus-4-6",
    provider: "anthropic",
    label: "Opus 4.6",
    thinking: ["off", "low", "medium", "high", "max"],
    defaultThinking: "medium",
  },
  {
    value: "anthropic:claude-sonnet-4-6",
    provider: "anthropic",
    label: "Sonnet 4.6",
    thinking: ["off", "low", "medium", "high", "max"],
    defaultThinking: "medium",
  },
  {
    value: "anthropic:claude-haiku-4-5",
    provider: "anthropic",
    label: "Haiku 4.5",
    thinking: ["off", "low", "medium", "high"],
    defaultThinking: "medium",
  },
  {
    value: "openai:gpt-5.5",
    provider: "openai",
    label: "GPT-5.5",
    thinking: ["off", "low", "medium", "high", "xhigh"],
    defaultThinking: "medium",
    supportsFast: true,
  },
  {
    value: "openai:gpt-5.4",
    provider: "openai",
    label: "GPT-5.4",
    thinking: ["off", "low", "medium", "high", "xhigh"],
    defaultThinking: "medium",
    supportsFast: true,
  },
  {
    value: "openai:gpt-5.4-mini",
    provider: "openai",
    label: "GPT-5.4 Mini",
    thinking: ["off", "low", "medium", "high", "xhigh"],
    defaultThinking: "medium",
    supportsFast: true,
  },
  {
    value: "openai:gpt-5.3-codex",
    provider: "openai",
    label: "GPT-5.3 Codex",
    thinking: ["off", "low", "medium", "high", "xhigh"],
    defaultThinking: "medium",
    supportsFast: true,
  },
  {
    value: "openai:gpt-5.3-codex-spark",
    provider: "openai",
    label: "GPT-5.3 Codex Spark",
    thinking: ["low", "medium", "high", "xhigh"],
    defaultThinking: "low",
    supportsFast: true,
  },
  {
    value: "openai:gpt-5.2",
    provider: "openai",
    label: "GPT-5.2",
    thinking: ["off", "low", "medium", "high", "xhigh"],
    defaultThinking: "medium",
    supportsFast: true,
  },
  {
    value: "google:claude-opus-4.6",
    provider: "google",
    label: "Claude Opus 4.6",
    thinking: ["high"],
    defaultThinking: "high",
  },
  {
    value: "google:claude-sonnet-4.6",
    provider: "google",
    label: "Claude Sonnet 4.6",
    thinking: ["high"],
    defaultThinking: "high",
  },
  {
    value: "google:gpt-oss-120b",
    provider: "google",
    label: "GPT-OSS 120B",
    thinking: ["off"],
    defaultThinking: "off",
  },
  {
    value: "google:gemini-3.1-pro",
    provider: "google",
    label: "Gemini 3.1 Pro",
    thinking: ["low", "high"],
    defaultThinking: "high",
  },
  {
    value: "google:gemini-3.5-flash",
    provider: "google",
    label: "Gemini 3.5 Flash",
    thinking: ["low", "medium", "high"],
    defaultThinking: "high",
  },
  {
    value: "kimi:kimi-for-coding",
    provider: "kimi",
    label: "Kimi 2.6",
    thinking: ["off", "high"],
    defaultThinking: "high",
  },
  {
    value: "deepseek:deepseek-v4-flash",
    provider: "deepseek",
    label: "DeepSeek V4 Flash",
    thinking: ["off", "high"],
    defaultThinking: "high",
  },
  {
    value: "deepseek:deepseek-v4-pro",
    provider: "deepseek",
    label: "DeepSeek V4 Pro",
    thinking: ["off", "high"],
    defaultThinking: "high",
  },
  {
    value: "cursor:composer-2.5",
    provider: "cursor",
    label: "Composer 2.5",
    thinking: [],
    defaultThinking: "off",
    supportsFast: true,
    defaultFast: true,
  },
];

const OPENROUTER_THINKING: readonly ThinkingLevel[] = ["off", "low", "medium", "high"];
const OPENROUTER_NO_THINKING: readonly ThinkingLevel[] = ["off"];

export function sanitizeOpenRouterName(name: string | null | undefined): string {
  const raw = (name ?? "").trim();
  if (!raw) return "";
  // OpenRouter prefixes most names with the underlying provider, e.g. "OpenAI: GPT-4o".
  // The provider icon already conveys that information in Sinew, so drop the prefix.
  const colon = raw.indexOf(":");
  if (colon <= 0) return raw;
  const tail = raw.slice(colon + 1).trim();
  return tail || raw;
}

export function modelsWithOpenRouter(
  openRouterModels: readonly OpenRouterModel[] = [],
  deepSeekModels: readonly string[] = [],
): ModelEntry[] {
  const dsEntries = deepSeekModelEntries(deepSeekModels);
  const staticDsValues = new Set(MODELS.filter(m => m.provider === "deepseek").map(m => m.value));
  const filteredDs = dsEntries.filter(e => !staticDsValues.has(e.value));
  return [...MODELS, ...openRouterModelEntries(openRouterModels), ...filteredDs];
}

export function availableModelsForProviders(
  configuredProviders: readonly string[],
  openRouterModels: readonly OpenRouterModel[] = [],
  deepSeekModels: readonly string[] = [],
): ModelEntry[] {
  const configured = new Set(configuredProviders);
  const entries: ModelEntry[] = [
    ...MODELS.filter((model) => configured.has(model.provider)),
  ];

  const sortedProviders = [...configuredProviders].sort((a, b) =>
    a.localeCompare(b, undefined, { numeric: true, sensitivity: "base" })
  );

  for (const provider of sortedProviders) {
    if (provider.startsWith("openai:")) {
      const suffix = provider.slice("openai:".length);
      let modelName = "gpt-5.5";
      try {
        modelName = localStorage.getItem(`sinew.provider-model.${provider}`) || "gpt-5.5";
      } catch {}

      const matchingModel = MODELS.find((m) => m.value === `openai:${modelName}`);
      const label = matchingModel ? matchingModel.label : "GPT-5.5";
      const thinking = (matchingModel ? matchingModel.thinking : ["off", "low", "medium", "high", "xhigh"]) as readonly ThinkingLevel[];
      
      let defaultThinking = matchingModel ? matchingModel.defaultThinking : "medium";
      try {
        const storedThinking = localStorage.getItem(`sinew.provider-thinking.${provider}`);
        if (storedThinking) {
          defaultThinking = storedThinking as any;
        }
      } catch {}

      let defaultFast = matchingModel ? Boolean(matchingModel.supportsFast) : true;
      try {
        const storedFast = localStorage.getItem(`sinew.provider-fast.${provider}`);
        if (storedFast !== null) {
          defaultFast = storedFast === "true";
        }
      } catch {}

      entries.push({
        value: modelId(provider, modelName),
        provider: provider as any,
        label: `OpenAI ${suffix}`,
        thinking,
        defaultThinking,
        supportsFast: true,
        defaultFast,
      });
    } else if (provider.startsWith("google:")) {
      const suffix = provider.slice("google:".length);
      let modelName = "gemini-3.5-flash";
      try {
        modelName = localStorage.getItem(`sinew.provider-model.${provider}`) || "gemini-3.5-flash";
      } catch {}

      const matchingModel = MODELS.find((m) => m.value === `google:${modelName}`);
      const label = matchingModel ? matchingModel.label : "Gemini 3.5 Flash";
      const thinking = (matchingModel ? matchingModel.thinking : ["low", "medium", "high"]) as readonly ThinkingLevel[];
      
      let defaultThinking = matchingModel ? matchingModel.defaultThinking : "high";
      try {
        const storedThinking = localStorage.getItem(`sinew.provider-thinking.${provider}`);
        if (storedThinking) {
          defaultThinking = storedThinking as any;
        }
      } catch {}

      entries.push({
        value: modelId(provider, modelName),
        provider: provider as any,
        label: `Google ${suffix}`,
        thinking,
        defaultThinking,
      });
    }
  }

  if (configured.has("openrouter")) {
    entries.push(...openRouterModelEntries(openRouterModels));
  }

  if (configured.has("deepseek") && deepSeekModels.length > 0) {
    const dsEntries = deepSeekModelEntries(deepSeekModels);
    const existingValues = new Set(entries.map(e => e.value));
    for (const e of dsEntries) {
      if (!existingValues.has(e.value)) {
        entries.push(e);
      }
    }
  }

  return entries;
}

function deepSeekModelEntries(
  deepSeekModels: readonly string[],
): ModelEntry[] {
  return deepSeekModels.map((modelName) => {
    const isReasoner = modelName.includes("reasoner") || modelName.includes("r1");
    const supportsThinking = modelName.includes("v4") || isReasoner;
    return {
      value: modelId("deepseek", modelName),
      provider: "deepseek",
      label: modelName.replace(/-/g, " ").replace(/\b\w/g, c => c.toUpperCase()),
      thinking: supportsThinking ? ["off", "high"] as readonly ThinkingLevel[] : ["off"] as readonly ThinkingLevel[],
      defaultThinking: supportsThinking ? "high" : "off",
    };
  });
}

function openRouterModelEntries(
  openRouterModels: readonly OpenRouterModel[],
): ModelEntry[] {
  return openRouterModels.map((model) => ({
    value: modelId("openrouter", model.id),
    provider: "openrouter",
    label: sanitizeOpenRouterName(model.name) || model.id,
    thinking: model.supportsThinking ? OPENROUTER_THINKING : OPENROUTER_NO_THINKING,
    defaultThinking: model.supportsThinking ? "medium" : "off",
  }));
}

export function modelIdFromRef(model: ModelRef | null | undefined): ModelId {
  if (model?.provider && model.name) {
    return modelId(model.provider, normalizedModelName(model.provider, model.name));
  }
  return MODELS[0].value;
}

export function modelRefFromId(model: ModelId): ModelRef {
  const separator = model.lastIndexOf(":");
  if (separator < 0) return { provider: "anthropic", name: model };
  const provider = model.slice(0, separator);
  const name = model.slice(separator + 1);
  return { provider, name };
}

export function thinkingFromRef(
  model: ModelRef | null | undefined,
): ThinkingLevel {
  if (model?.provider === "google" || model?.provider?.startsWith("google:")) {
    if (model.name.endsWith("-low")) return "low";
    if (model.name.endsWith("-medium")) return "medium";
    if (model.name.endsWith("-high")) return "high";
    if (model.effort === "low" || model.effort === "medium" || model.effort === "high") {
      return model.effort;
    }
    if (model.effort === "none") {
      // Pro variants don't support `minimal`; clamp to low so we never send
      // an invalid thinking level for those models.
      return model.name.includes("-pro") ? "low" : "minimal";
    }
    return "high";
  }
  if (model?.provider === "kimi") {
    if (model.effort === "none") return "off";
    return "high";
  }
  if (model?.provider === "deepseek") {
    if (model.name === "deepseek-reasoner") return "high";
    if (model.name === "deepseek-v4-flash" || model.name === "deepseek-v4-pro") {
      if (model.effort === "none") return "off";
      return "high";
    }
    return "off";
  }
  if (model?.provider === "openrouter") {
    if (model.effort === "none") return "off";
    if (
      model.effort === "low" ||
      model.effort === "medium" ||
      model.effort === "high"
    ) {
      return model.effort;
    }
    return "medium";
  }
  if (
    model?.provider === "openai" &&
    model.name === "gpt-5.3-codex-spark" &&
    model.effort === "none"
  ) {
    return "low";
  }
  if (model?.effort === "none") return "off";
  if (model?.effort === "xhigh") return "xhigh";
  if (model?.provider === "openai" && model.effort === "max") return "xhigh";
  if (
    model?.effort === "low" ||
    model?.effort === "medium" ||
    model?.effort === "high" ||
    model?.effort === "max"
  ) {
    return model.effort;
  }
  return "medium";
}

export function modelRefWithThinking(
  model: ModelRef,
  thinking: ThinkingLevel,
): ModelRef {
  if (model.provider === "google") {
    const name = normalizedGoogleModelName(model.name);
    if (thinking === "off") return { ...model, name, effort: "low" };
    if (thinking === "minimal") {
      // Pro variants reject `minimal` server-side; clamp to low.
      return name.includes("-pro")
        ? { ...model, name, effort: "low" }
        : { ...model, name, effort: "none" };
    }
    if (thinking === "xhigh" || thinking === "max") return { ...model, name, effort: "high" };
    return { ...model, name, effort: thinking };
  }
  if (
    model.provider === "openai" &&
    model.name === "gpt-5.3-codex-spark" &&
    thinking === "off"
  ) {
    return { ...model, effort: "low" };
  }
  if (thinking === "off") return { ...model, effort: "none" };
  if (model.provider === "kimi") return { ...model, effort: "high" };
  if (model.provider === "deepseek") {
    if (model.name === "deepseek-reasoner") return { ...model, effort: "high" };
    if (model.name === "deepseek-v4-flash" || model.name === "deepseek-v4-pro") {
      return { ...model, effort: "high" };
    }
    return { ...model, effort: "none" };
  }
  if (model.provider === "openrouter" && (thinking === "xhigh" || thinking === "max")) {
    return { ...model, effort: "high" };
  }
  // `minimal` is Gemini-only on the backend. The Google branch above already
  // handled it; for any other provider that ever surfaces it, clamp to low.
  if (thinking === "minimal") return { ...model, effort: "low" };
  return { ...model, effort: thinking };
}

export function selectionFromRef(
  model: ModelRef | null | undefined,
): ModeModelSelection {
  return {
    model: modelIdFromRef(model),
    thinking: thinkingFromRef(model),
  };
}

function modelId(provider: string, name: string): ModelId {
  return `${provider}:${name}`;
}

function normalizedModelName(provider: string, name: string): string {
  if (provider === "google" || provider.startsWith("google:")) return normalizedGoogleModelName(name);
  return name;
}

function normalizedGoogleModelName(name: string): string {
  if (name === "gemini-3.1-pro-preview") return "gemini-3.1-pro";
  if (name === "claude-opus-4-6-thinking") return "claude-opus-4.6";
  if (name === "claude-sonnet-4-6") return "claude-sonnet-4.6";
  if (name === "gpt-oss-120b-medium") return "gpt-oss-120b";
  if (name === "gemini-3-flash-preview") return "gemini-3-flash";
  if (name === "gemini-3.1-pro-low" || name === "gemini-3.1-pro-high") {
    return "gemini-3.1-pro";
  }
  if (
    name === "gemini-3.5-flash-low" ||
    name === "gemini-3.5-flash-medium" ||
    name === "gemini-3.5-flash-high" ||
    name === "gemini-3.5-flash-extra-low" ||
    name === "gemini-3-flash-agent"
  ) {
    return "gemini-3.5-flash";
  }
  return name;
}

export function selectionsFromSettings(
  settings: ModeModelSettings | null | undefined,
  fallback: ModelRef,
): ModeModelSelections {
  return {
    act: selectionFromRef(settings?.act ?? fallback),
    plan: selectionFromRef(settings?.plan ?? fallback),
    goal: selectionFromRef(settings?.goal ?? settings?.act ?? fallback),
  };
}
