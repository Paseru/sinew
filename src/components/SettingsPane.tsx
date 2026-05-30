import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Editor, { type OnMount } from "@monaco-editor/react";
import { Icon } from "@iconify/react";
import { Wrench } from "lucide-react";
import { getAppLocale, setAppLocale, type AppLocale } from "../lib/locale";
import { api } from "../lib/ipc";
import { fetchProviderQuota, quotaColor, getCachedQuota, type QuotaInfo, quotaCache } from "../lib/quotas";
import { canonicalToolName } from "../lib/tools";
import { Markdown } from "./chat/Markdown";
import { SinewMark } from "./SinewMark";
import {
  MODELS,
  PROVIDERS,
  THINKING_LEVELS,
  availableModelsForProviders,
  modelIdFromRef,
  modelRefFromId,
  modelRefWithThinking,
  sanitizeOpenRouterName,
  thinkingFromRef,
  type ModelEntry,
  type ModelId,
} from "../lib/models";
import type {
  AnthropicProviderStatus,
  CursorComposerAuthStatus,
  GoogleProviderStatus,
  ImageProvider,
  InstalledSkill,
  KimiProviderStatus,
  DeepSeekProviderStatus,
  McpEnvVar,
  McpServerConfig,
  McpServerProbe,
  McpSettings,
  OpenAiProviderStatus,
  OpenAiAccountInfo,
  GoogleAccountInfo,
  OpenRouterModel,
  OpenRouterModelSearchResult,
  OpenRouterProviderStatus,
  SkillSettings,
  SubAgentConfig,
  SubAgentSettings,
  ThinkingLevel,
  ToolConfig,
  ToolSettings,
  WebSearchProvider,
} from "../types";

const EMPTY_SETTINGS: McpSettings = { servers: [] };
const FALLBACK_TOOL_SETTINGS: ToolSettings = {
  tools: [],
  planModePrompt: "",
  defaultPlanModePrompt: "",
  imageProvider: "gptImage2",
  openaiImageUseSubscription: false,
  geminiImageUseSubscription: false,
  openaiImageModel: "gpt-image-2",
  geminiImageModel: "gemini-3.1-flash-image-preview",
  openaiImageApiKey: "",
  nanoBananaApiKey: "",
  webSearchProvider: "classic",
  linkupApiKey: "",
};
const PROVIDERS_CHANGED_EVENT = "sinew:providers-changed";
const TOOL_SETTINGS_CHANGED_EVENT = "sinew:tool-settings-changed";

// Global cache to execute SOTA diagnostics exactly once at application startup.
// Prevents re-running diagnostics when entering the Options page.
interface SotaCache {
  data: any;
  error: string | null;
  promise: Promise<any> | null;
}

const sotaCache: SotaCache = {
  data: null,
  error: null,
  promise: null,
};

const triggerSotaDiagnostics = (force = false): Promise<any> => {
  if (!force && sotaCache.promise) {
    return sotaCache.promise;
  }

  const promise = api.checkSotaDiagnostics()
    .then((dataStr) => {
      const parsed = JSON.parse(dataStr);
      sotaCache.data = parsed;
      sotaCache.error = null;
      return parsed;
    })
    .catch((err) => {
      const errMsg = err?.toString() || "Unknown error";
      sotaCache.error = errMsg;
      sotaCache.data = null;
      throw new Error(errMsg);
    });

  sotaCache.promise = promise;
  return promise;
};

// Eagerly trigger in the background at startup
setTimeout(() => {
  triggerSotaDiagnostics().catch(() => {});
}, 500);

type Props = {
  workspacePath: string;
};

type Section = "options" | "about" | "providers" | "tools" | "mcp" | "skills" | "subagents";

export function SettingsPane({ workspacePath }: Props) {
  const [section, setSection] = useState<Section>("options");
  const [locale, setLocaleState] = useState<AppLocale>(() => getAppLocale());
  const [settings, setSettings] = useState<McpSettings>(EMPTY_SETTINGS);
  const [savedJson, setSavedJson] = useState("");
  const [jsonText, setJsonText] = useState("");
  const [selectedServerId, setSelectedServerId] = useState<string | null>(null);
  const [mcpAdvancedOpen, setMcpAdvancedOpen] = useState(false);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [parseError, setParseError] = useState<string | null>(null);
  const [probes, setProbes] = useState<McpServerProbe[]>([]);
  // Last known successful tool count per server id. We keep it across toggles
  // so that disabling a server doesn't make us forget how many tools it had.
  const [knownToolCounts, setKnownToolCounts] = useState<Record<string, number>>({});

  const [probing, setProbing] = useState(false);

  const [skills, setSkills] = useState<InstalledSkill[] | null>(null);
  const [skillsLoading, setSkillsLoading] = useState(false);
  const [skillsSaving, setSkillsSaving] = useState(false);
  const [skillsDeleting, setSkillsDeleting] = useState(false);
  const [skillsError, setSkillsError] = useState<string | null>(null);
  const [skillsStatus, setSkillsStatus] = useState<string | null>(null);
  const [savedSkillsJson, setSavedSkillsJson] = useState("");
  const [selectedSkillName, setSelectedSkillName] = useState<string | null>(null);
  const [skillFilter, setSkillFilter] = useState("");

  const [subAgentSettings, setSubAgentSettings] = useState<SubAgentSettings>({
    agents: [],
  });
  const [savedSubAgentJson, setSavedSubAgentJson] = useState("");
  const [subAgentsLoading, setSubAgentsLoading] = useState(false);
  const [subAgentsSaving, setSubAgentsSaving] = useState(false);
  const [subAgentsStatus, setSubAgentsStatus] = useState<string | null>(null);
  const [selectedSubAgentId, setSelectedSubAgentId] = useState<string | null>(null);

  const [toolSettings, setToolSettings] = useState<ToolSettings | null>(null);
  const [savedToolSettingsJson, setSavedToolSettingsJson] = useState("");
  const [toolsLoading, setToolsLoading] = useState(false);
  const [toolsSaving, setToolsSaving] = useState(false);
  const [toolsStatus, setToolsStatus] = useState<string | null>(null);

  const [openAiStatus, setOpenAiStatus] = useState<OpenAiProviderStatus | null>(null);
  const [openAiAccounts, setOpenAiAccounts] = useState<OpenAiAccountInfo[]>([]);
  const [unconnectedAccounts, setUnconnectedAccounts] = useState<string[]>([]);

  const [anthropicStatus, setAnthropicStatus] = useState<AnthropicProviderStatus | null>(null);
  const [googleStatus, setGoogleStatus] = useState<GoogleProviderStatus | null>(null);
  const [googleAccounts, setGoogleAccounts] = useState<GoogleAccountInfo[]>([]);
  const [unconnectedGoogleAccounts, setUnconnectedGoogleAccounts] = useState<string[]>([]);
  const [kimiStatus, setKimiStatus] = useState<KimiProviderStatus | null>(null);
  const [deepSeekStatus, setDeepSeekStatus] = useState<DeepSeekProviderStatus | null>(null);
  const [cursorComposerStatus, setCursorComposerStatus] = useState<CursorComposerAuthStatus | null>(null);
  const cursorOAuthPendingRef = useRef(false);
  const [openRouterStatus, setOpenRouterStatus] = useState<OpenRouterProviderStatus | null>(null);
  const [openRouterModels, setOpenRouterModels] = useState<OpenRouterModel[]>([]);
  const [providersLoading, setProvidersLoading] = useState(false);
  const [providersBusy, setProvidersBusy] = useState(false);
  const [providersMessage, setProvidersMessage] = useState<string | null>(null);
  const [configuredProviders, setConfiguredProviders] = useState<string[]>([]);

  const setLocale = useCallback((nextLocale: AppLocale) => {
    setAppLocale(nextLocale);
    setLocaleState(nextLocale);
    window.location.reload();
  }, []);

  useEffect(() => {
    setToolSettings(null);
    setSavedToolSettingsJson("");
    setToolsStatus(null);
    setSkills(null);
    setSavedSkillsJson("");
    setSkillsStatus(null);
    setSkillsError(null);
    setSelectedSkillName(null);
  }, [workspacePath]);

  // Outside callers (e.g. the composer's "Connect a provider" CTA) can jump
  // straight to a specific section by dispatching this window event. We
  // listen unconditionally so it works whether or not the pane has been
  // opened before.
  useEffect(() => {
    const handler = (event: Event) => {
      const detail = (event as CustomEvent<{ section?: Section }>).detail;
      if (detail?.section) setSection(detail.section);
    };
    window.addEventListener("sinew:open-settings-section", handler);
    return () =>
      window.removeEventListener("sinew:open-settings-section", handler);
  }, []);

  // ---- MCP load ---------------------------------------------------------
  useEffect(() => {
    let disposed = false;
    setLoading(true);
    setProbing(true);
    (async () => {
      try {
        const loaded = await api.listMcpSettings();
        if (disposed) return;
        const normalized = normalizeSettings(loaded);
        const nextJson = settingsToJson(normalized);
        setSettings(normalized);
        setSavedJson(nextJson);
        setJsonText(nextJson);
        setSelectedServerId(normalized.servers[0]?.id ?? null);
        setLoading(false);

        if (normalized.servers.some((server) => server.enabled)) {
          try {
            const nextProbes = await api.probeMcpTools();
            if (disposed) return;
            setProbes(nextProbes);
            const failures = nextProbes.filter(
              (probe) => probe.enabled && !probe.ok,
            ).length;
            if (failures) {
              setStatus(`${failures} server${failures === 1 ? "" : "s"} failed`);
            }
          } catch (probeErr) {
            if (!disposed) {
              setStatus(
                probeErr instanceof Error ? probeErr.message : String(probeErr),
              );
            }
          }
        }
      } catch (err) {
        if (!disposed) setStatus(String(err));
      } finally {
        if (!disposed) {
          setLoading(false);
          setProbing(false);
        }
      }
    })();
    return () => {
      disposed = true;
    };
  }, []);

  // Remember the latest successful tool count per server so we can keep
  // displaying a count (in a disabled tone) even after the server is toggled
  // off and the probe stops reflecting it.
  useEffect(() => {
    if (probes.length === 0) return;
    setKnownToolCounts((current) => {
      let changed = false;
      const next = { ...current };
      for (const probe of probes) {
        if (probe.enabled && probe.ok) {
          const count = probe.tools.length;
          if (next[probe.serverId] !== count) {
            next[probe.serverId] = count;
            changed = true;
          }
        }
      }
      return changed ? next : current;
    });
  }, [probes]);

  // Re-parse on every JSON edit so cards reflect the latest text.
  useEffect(() => {
    try {
      const parsed = parseMcpJson(jsonText);
      setSettings(parsed);
      setParseError(null);
      setSelectedServerId((current) => {
        if (current && parsed.servers.some((server) => server.id === current)) {
          return current;
        }
        return parsed.servers[0]?.id ?? null;
      });
    } catch (err) {
      setParseError(err instanceof Error ? err.message : String(err));
    }
  }, [jsonText]);

  const dirty = jsonText !== savedJson;
  const selectedServer =
    settings.servers.find((server) => server.id === selectedServerId) ?? null;
  const selectedProbe =
    probes.find((probe) => probe.serverId === selectedServerId) ?? null;

  // ---- Tools load ------------------------------------------------------
  const loadToolSettings = useCallback(async () => {
    setToolsLoading(true);
    setToolsStatus(null);
    try {
      const loaded = normalizeToolSettings(await api.listToolSettings(workspacePath));
      setToolSettings(loaded);
      setSavedToolSettingsJson(toolSettingsFingerprint(loaded));
    } catch (err) {
      setToolsStatus(err instanceof Error ? err.message : String(err));
      const fallback = normalizeToolSettings(FALLBACK_TOOL_SETTINGS);
      setToolSettings(fallback);
      setSavedToolSettingsJson(toolSettingsFingerprint(fallback));
    } finally {
      setToolsLoading(false);
    }
  }, [workspacePath]);

  useEffect(() => {
    if (toolSettings !== null) return;
    void loadToolSettings();
  }, [toolSettings, loadToolSettings]);

  const toolsDirty =
    toolSettings !== null &&
    toolSettingsFingerprint(toolSettings) !== savedToolSettingsJson;

  const saveToolSettings = useCallback(async () => {
    if (!toolSettings) return;
    setToolsSaving(true);
    setToolsStatus(null);
    try {
      const saved = normalizeToolSettings(
        await api.saveToolSettings(workspacePath, toolSettings),
      );
      setToolSettings(saved);
      setSavedToolSettingsJson(toolSettingsFingerprint(saved));
      setToolsStatus("Saved");
      window.dispatchEvent(new CustomEvent(TOOL_SETTINGS_CHANGED_EVENT));
    } catch (err) {
      setToolsStatus(err instanceof Error ? err.message : String(err));
    } finally {
      setToolsSaving(false);
    }
  }, [toolSettings, workspacePath]);

  const updateTool = useCallback((name: string, patch: Partial<ToolConfig>) => {
    setToolSettings((current) => {
      if (!current) return current;
      return {
        ...current,
        tools: current.tools.map((tool) =>
          tool.name === name ? { ...tool, ...patch } : tool,
        ),
      };
    });
  }, []);

  const updatePlanModePrompt = useCallback((planModePrompt: string) => {
    setToolSettings((current) =>
      current ? { ...current, planModePrompt } : current,
    );
  }, []);

  const updateOpenAiImageApiKey = useCallback((openaiImageApiKey: string) => {
    setToolSettings((current) =>
      current ? { ...current, openaiImageApiKey } : current,
    );
  }, []);

  const updateImageProvider = useCallback((imageProvider: ImageProvider) => {
    setToolSettings((current) =>
      current ? { ...current, imageProvider } : current,
    );
  }, []);

  const updateOpenAiImageUseSubscription = useCallback((openaiImageUseSubscription: boolean) => {
    setToolSettings((current) =>
      current ? { ...current, openaiImageUseSubscription } : current,
    );
  }, []);

  const updateGeminiImageUseSubscription = useCallback((geminiImageUseSubscription: boolean) => {
    setToolSettings((current) =>
      current ? { ...current, geminiImageUseSubscription } : current,
    );
  }, []);

  const updateOpenAiImageModel = useCallback((openaiImageModel: string) => {
    setToolSettings((current) =>
      current ? { ...current, openaiImageModel } : current,
    );
  }, []);

  const updateGeminiImageModel = useCallback((geminiImageModel: string) => {
    setToolSettings((current) =>
      current ? { ...current, geminiImageModel } : current,
    );
  }, []);

  const updateNanoBananaApiKey = useCallback((nanoBananaApiKey: string) => {
    setToolSettings((current) =>
      current ? { ...current, nanoBananaApiKey } : current,
    );
  }, []);

  const updateWebSearchProvider = useCallback((webSearchProvider: WebSearchProvider) => {
    setToolSettings((current) =>
      current ? { ...current, webSearchProvider } : current,
    );
  }, []);

  const updateLinkupApiKey = useCallback((linkupApiKey: string) => {
    setToolSettings((current) =>
      current ? { ...current, linkupApiKey } : current,
    );
  }, []);

  const loadConfiguredProviders = useCallback(async () => {
    try {
      const [providers, models] = await Promise.all([
        api.listConfiguredModelProviders(),
        api.listOpenRouterModels().catch(() => []),
      ]);
      setConfiguredProviders(providers);
      setOpenRouterModels(models);
    } catch {
      setConfiguredProviders([]);
      setOpenRouterModels([]);
    }
  }, []);

  useEffect(() => {
    void loadConfiguredProviders();
  }, [loadConfiguredProviders]);

  const availableModels = useMemo(
    () => availableModelsForProviders(configuredProviders, openRouterModels),
    [configuredProviders, openRouterModels],
  );

  const loadOpenAiStatus = useCallback(async () => {
    setProvidersLoading(true);
    try {
      const status = await api.getOpenAiProviderStatus();
      setOpenAiStatus(status);
      setProvidersMessage(status.error ?? null);
      
      const accounts = await api.getAllOpenAiAccounts();
      setOpenAiAccounts(accounts);

      setUnconnectedAccounts((prev) =>
        prev.filter((key) => !accounts.some((acc) => acc.key === key))
      );

      if (status.connectionState !== "connecting") {
        void loadConfiguredProviders();
        window.dispatchEvent(new CustomEvent(PROVIDERS_CHANGED_EVENT));
      }
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersLoading(false);
    }
  }, [loadConfiguredProviders]);



  const loadAnthropicStatus = useCallback(async () => {
    setProvidersLoading(true);
    try {
      const status = await api.getAnthropicProviderStatus();
      setAnthropicStatus(status);
      setProvidersMessage(status.error ?? null);
      if (status.connectionState !== "connecting") {
        void loadConfiguredProviders();
        window.dispatchEvent(new CustomEvent(PROVIDERS_CHANGED_EVENT));
      }
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersLoading(false);
    }
  }, [loadConfiguredProviders]);

  const loadGoogleStatus = useCallback(async () => {
    setProvidersLoading(true);
    try {
      const status = await api.getGoogleProviderStatus();
      setGoogleStatus(status);
      setProvidersMessage(status.error ?? null);

      const accounts = await api.getAllGoogleAccounts();
      setGoogleAccounts(accounts);

      setUnconnectedGoogleAccounts((prev) =>
        prev.filter((key) => !accounts.some((acc) => acc.key === key))
      );

      if (status.connectionState !== "connecting") {
        void loadConfiguredProviders();
        window.dispatchEvent(new CustomEvent(PROVIDERS_CHANGED_EVENT));
      }
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersLoading(false);
    }
  }, [loadConfiguredProviders]);

  const loadKimiStatus = useCallback(async () => {
    setProvidersLoading(true);
    try {
      const status = await api.getKimiProviderStatus();
      setKimiStatus(status);
      setProvidersMessage(status.error ?? null);
      if (status.connectionState !== "connecting") {
        void loadConfiguredProviders();
        window.dispatchEvent(new CustomEvent(PROVIDERS_CHANGED_EVENT));
      }
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersLoading(false);
    }
  }, [loadConfiguredProviders]);

  const loadDeepSeekStatus = useCallback(async () => {
    setProvidersLoading(true);
    try {
      const status = await api.getDeepSeekProviderStatus();
      setDeepSeekStatus(status);
      setProvidersMessage(status.error ?? null);
      if (status.connectionState !== "connecting") {
        void loadConfiguredProviders();
        window.dispatchEvent(new CustomEvent(PROVIDERS_CHANGED_EVENT));
      }
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersLoading(false);
    }
  }, [loadConfiguredProviders]);

  const loadCursorStatus = useCallback(async () => {
    setProvidersLoading(true);
    try {
      const composer = await api.getCursorComposerStatus();
      const wasPending = cursorOAuthPendingRef.current;
      setCursorComposerStatus(composer);
      if (
        wasPending &&
        composer.connected &&
        composer.connectionState === "connected"
      ) {
        cursorOAuthPendingRef.current = false;
        setProvidersMessage(
          "Cursor connecté — vous pouvez fermer l'onglet du navigateur et revenir à Sinew.",
        );
        quotaCache.delete("cursor");
        window.dispatchEvent(new CustomEvent("sinew:quota-updated"));
      } else if (wasPending && composer.connectionState === "error") {
        cursorOAuthPendingRef.current = false;
        setProvidersMessage(composer.error ?? "Connexion Cursor échouée");
      }
      void loadConfiguredProviders();
      window.dispatchEvent(new CustomEvent(PROVIDERS_CHANGED_EVENT));
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersLoading(false);
    }
  }, [loadConfiguredProviders]);

  const loadOpenRouterStatus = useCallback(async () => {
    setProvidersLoading(true);
    try {
      const [status, models] = await Promise.all([
        api.getOpenRouterProviderStatus(),
        api.listOpenRouterModels(),
      ]);
      setOpenRouterStatus(status);
      setOpenRouterModels(models);
      setProvidersMessage(status.error ?? null);
      void loadConfiguredProviders();
      window.dispatchEvent(new CustomEvent(PROVIDERS_CHANGED_EVENT));
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersLoading(false);
    }
  }, [loadConfiguredProviders]);

  const [secondaryModels, setSecondaryModels] = useState<Record<string, string>>(() => {
    const models: Record<string, string> = {};
    try {
      for (let i = 2; i <= 20; i++) {
        const oKey = `openai:${i}`;
        models[oKey] = localStorage.getItem(`sinew.provider-model.${oKey}`) || "gpt-5.5";
        const gKey = `google:${i}`;
        models[gKey] = localStorage.getItem(`sinew.provider-model.${gKey}`) || "gemini-3.5-flash";
      }
    } catch {}
    return models;
  });

  const [secondaryThinking, setSecondaryThinking] = useState<Record<string, string>>(() => {
    const thinking: Record<string, string> = {};
    try {
      for (let i = 2; i <= 20; i++) {
        const oKey = `openai:${i}`;
        thinking[oKey] = localStorage.getItem(`sinew.provider-thinking.${oKey}`) || "medium";
        const gKey = `google:${i}`;
        thinking[gKey] = localStorage.getItem(`sinew.provider-thinking.${gKey}`) || "high";
      }
    } catch {}
    return thinking;
  });

  const [secondaryFast, setSecondaryFast] = useState<Record<string, string>>(() => {
    const fast: Record<string, string> = {};
    try {
      for (let i = 2; i <= 20; i++) {
        const oKey = `openai:${i}`;
        fast[oKey] = localStorage.getItem(`sinew.provider-fast.${oKey}`) || "true";
        const gKey = `google:${i}`;
        fast[gKey] = localStorage.getItem(`sinew.provider-fast.${gKey}`) || "true";
      }
    } catch {}
    return fast;
  });

  const handleUpdateSecondaryModel = useCallback((key: string, model: string) => {
    try {
      localStorage.setItem(`sinew.provider-model.${key}`, model);
    } catch {}
    setSecondaryModels((prev) => ({ ...prev, [key]: model }));
    if (key.startsWith("google:")) {
      void loadGoogleStatus();
    } else {
      void loadOpenAiStatus();
    }
  }, [loadOpenAiStatus, loadGoogleStatus]);

  const handleUpdateSecondaryThinking = useCallback((key: string, thinking: string) => {
    try {
      localStorage.setItem(`sinew.provider-thinking.${key}`, thinking);
    } catch {}
    setSecondaryThinking((prev) => ({ ...prev, [key]: thinking }));
    if (key.startsWith("google:")) {
      void loadGoogleStatus();
    } else {
      void loadOpenAiStatus();
    }
  }, [loadOpenAiStatus, loadGoogleStatus]);

  const handleUpdateSecondaryFast = useCallback((key: string, fast: string) => {
    try {
      localStorage.setItem(`sinew.provider-fast.${key}`, fast);
    } catch {}
    setSecondaryFast((prev) => ({ ...prev, [key]: fast }));
    if (key.startsWith("google:")) {
      void loadGoogleStatus();
    } else {
      void loadOpenAiStatus();
    }
  }, [loadOpenAiStatus, loadGoogleStatus]);

  useEffect(() => {
    if (section !== "providers" && section !== "tools") return;
    if (openAiStatus === null) void loadOpenAiStatus();
    if (section !== "providers") return;
    if (anthropicStatus === null) void loadAnthropicStatus();
    if (googleStatus === null) void loadGoogleStatus();
    if (kimiStatus === null) void loadKimiStatus();
    if (deepSeekStatus === null) void loadDeepSeekStatus();
    if (cursorComposerStatus === null) void loadCursorStatus();
    if (openRouterStatus === null) void loadOpenRouterStatus();
  }, [
    section,
    openAiStatus,
    anthropicStatus,
    googleStatus,
    kimiStatus,
    deepSeekStatus,
    cursorComposerStatus,
    openRouterStatus,
    loadOpenAiStatus,
    loadAnthropicStatus,
    loadGoogleStatus,
    loadKimiStatus,
    loadDeepSeekStatus,
    loadCursorStatus,
    loadOpenRouterStatus,
  ]);

  useEffect(() => {
    if (openAiStatus === null || openAiStatus.connected) return;
    setToolSettings((current) => {
      if (!current?.openaiImageUseSubscription) return current;
      return { ...current, openaiImageUseSubscription: false };
    });
  }, [openAiStatus]);

  useEffect(() => {
    if (googleStatus === null || googleStatus.connected) return;
    setToolSettings((current) => {
      if (!current?.geminiImageUseSubscription) return current;
      return { ...current, geminiImageUseSubscription: false };
    });
  }, [googleStatus]);

  useEffect(() => {
    const openAiConnecting = openAiStatus?.connectionState === "connecting";
    const anthropicConnecting = anthropicStatus?.connectionState === "connecting";
    const googleConnecting = googleStatus?.connectionState === "connecting";
    const kimiConnecting = kimiStatus?.connectionState === "connecting";
    const deepSeekConnecting = deepSeekStatus?.connectionState === "connecting";
    const cursorConnecting = cursorComposerStatus?.connectionState === "connecting";
    if (!openAiConnecting && !anthropicConnecting && !googleConnecting && !kimiConnecting && !deepSeekConnecting && !cursorConnecting) return;
    const timer = window.setInterval(() => {
      if (openAiConnecting) void loadOpenAiStatus();
      if (anthropicConnecting) void loadAnthropicStatus();
      if (googleConnecting) void loadGoogleStatus();
      if (kimiConnecting) void loadKimiStatus();
      if (deepSeekConnecting) void loadDeepSeekStatus();
      if (cursorConnecting) void loadCursorStatus();
    }, 1200);
    return () => window.clearInterval(timer);
  }, [
    openAiStatus?.connectionState,
    anthropicStatus?.connectionState,
    googleStatus?.connectionState,
    kimiStatus?.connectionState,
    deepSeekStatus?.connectionState,
    cursorComposerStatus?.connectionState,
    loadOpenAiStatus,
    loadAnthropicStatus,
    loadGoogleStatus,
    loadKimiStatus,
    loadDeepSeekStatus,
  ]);

  const connectOpenAi = useCallback(async (key?: string) => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      const login = await api.startOpenAiOAuthLogin(key);
      const connecting: OpenAiProviderStatus = {
        connected: openAiStatus?.connected ?? false,
        connectionState: "connecting",
        loginId: login.loginId,
      };
      setOpenAiStatus(connecting);
      await api.openExternalUrl(login.authUrl);
      setProvidersMessage("Waiting for browser confirmation…");
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
      void loadOpenAiStatus();
    } finally {
      setProvidersBusy(false);
    }
  }, [openAiStatus, loadOpenAiStatus]);

  const handleAddOpenAiAccount = useCallback(() => {
    let nextIndex = 2;
    while (true) {
      const key = `openai:${nextIndex}`;
      const isConnected = openAiAccounts.some((acc) => acc.key === key);
      const isUnconnected = unconnectedAccounts.includes(key);
      if (!isConnected && !isUnconnected) {
        setUnconnectedAccounts((prev) => [...prev, key]);
        break;
      }
      nextIndex++;
      if (nextIndex > 100) break;
    }
  }, [openAiAccounts, unconnectedAccounts]);

  const handleRemoveUnconnectedAccount = useCallback((key: string) => {
    setUnconnectedAccounts((prev) => prev.filter((k) => k !== key));
  }, []);

  const cancelOpenAi = useCallback(async () => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      setOpenAiStatus(await api.cancelOpenAiOAuthLogin());
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersBusy(false);
    }
  }, []);

  const disconnectOpenAi = useCallback(async () => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      setOpenAiStatus(await api.disconnectOpenAiProvider());
      setProvidersMessage("Disconnected");
      void loadOpenAiStatus();
      void loadConfiguredProviders();
      window.dispatchEvent(new CustomEvent(PROVIDERS_CHANGED_EVENT));
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersBusy(false);
    }
  }, [loadOpenAiStatus, loadConfiguredProviders]);

  const disconnectOpenAiAccount = useCallback(async (key: string) => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      await api.disconnectOpenAiAccount(key);
      setProvidersMessage("Account disconnected");
      void loadOpenAiStatus();
      void loadConfiguredProviders();
      window.dispatchEvent(new CustomEvent(PROVIDERS_CHANGED_EVENT));
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersBusy(false);
    }
  }, [loadOpenAiStatus, loadConfiguredProviders]);

  const connectAnthropic = useCallback(async () => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      const login = await api.startAnthropicOAuthLogin();
      const connecting: AnthropicProviderStatus = {
        connected: false,
        connectionState: "connecting",
        loginId: login.loginId,
      };
      setAnthropicStatus(connecting);
      await api.openExternalUrl(login.authUrl);
      setProvidersMessage("Waiting for browser confirmation...");
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
      void loadAnthropicStatus();
    } finally {
      setProvidersBusy(false);
    }
  }, [loadAnthropicStatus]);

  const cancelAnthropic = useCallback(async () => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      setAnthropicStatus(await api.cancelAnthropicOAuthLogin());
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersBusy(false);
    }
  }, []);

  const disconnectAnthropic = useCallback(async () => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      setAnthropicStatus(await api.disconnectAnthropicProvider());
      setProvidersMessage("Disconnected");
      void loadConfiguredProviders();
      window.dispatchEvent(new CustomEvent(PROVIDERS_CHANGED_EVENT));
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersBusy(false);
    }
  }, [loadConfiguredProviders]);

  const connectGoogle = useCallback(async (key?: string) => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      const login = await api.startGoogleOAuthLogin(key);
      const connecting: GoogleProviderStatus = {
        connected: googleStatus?.connected ?? false,
        connectionState: "connecting",
        loginId: login.loginId,
      };
      setGoogleStatus(connecting);
      await api.openExternalUrl(login.authUrl);
      setProvidersMessage("Waiting for browser confirmation...");
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
      void loadGoogleStatus();
    } finally {
      setProvidersBusy(false);
    }
  }, [googleStatus, loadGoogleStatus]);

  const disconnectGoogleAccount = useCallback(async (key: string) => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      await api.disconnectGoogleAccount(key);
      setProvidersMessage("Account disconnected");
      void loadGoogleStatus();
      void loadConfiguredProviders();
      window.dispatchEvent(new CustomEvent(PROVIDERS_CHANGED_EVENT));
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersBusy(false);
    }
  }, [loadGoogleStatus, loadConfiguredProviders]);

  const handleAddGoogleAccount = useCallback(() => {
    let nextIndex = 2;
    while (true) {
      const key = `google:${nextIndex}`;
      const isConnected = googleAccounts.some((acc) => acc.key === key);
      const isUnconnected = unconnectedGoogleAccounts.includes(key);
      if (!isConnected && !isUnconnected) {
        setUnconnectedGoogleAccounts((prev) => [...prev, key]);
        break;
      }
      nextIndex++;
      if (nextIndex > 100) break;
    }
  }, [googleAccounts, unconnectedGoogleAccounts]);

  const handleRemoveUnconnectedGoogleAccount = useCallback((key: string) => {
    setUnconnectedGoogleAccounts((prev) => prev.filter((k) => k !== key));
  }, []);

  const cancelGoogle = useCallback(async () => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      setGoogleStatus(await api.cancelGoogleOAuthLogin());
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersBusy(false);
    }
  }, []);

  const disconnectGoogle = useCallback(async () => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      setGoogleStatus(await api.disconnectGoogleProvider());
      setProvidersMessage("Disconnected");
      void loadConfiguredProviders();
      window.dispatchEvent(new CustomEvent(PROVIDERS_CHANGED_EVENT));
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersBusy(false);
    }
  }, [loadConfiguredProviders]);

  const connectCursor = useCallback(async () => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      const login = await api.startCursorOAuthLogin();
      cursorOAuthPendingRef.current = true;
      setCursorComposerStatus({
        connected: false,
        connectionState: "connecting",
        loginId: login.loginId,
      });
      await api.openExternalUrl(login.authUrl);
      setProvidersMessage(
        "Connectez-vous dans le navigateur (Google ou GitHub). La page dira « return to Cursor » — c'est normal : revenez ici, Sinew se connectera automatiquement.",
      );
    } catch (err) {
      cursorOAuthPendingRef.current = false;
      setProvidersMessage(err instanceof Error ? err.message : String(err));
      void loadCursorStatus();
    } finally {
      setProvidersBusy(false);
    }
  }, [loadCursorStatus]);

  const cancelCursorComposer = useCallback(async () => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      cursorOAuthPendingRef.current = false;
      setCursorComposerStatus(await api.cancelCursorOAuthLogin());
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersBusy(false);
    }
  }, []);

  const disconnectCursorComposer = useCallback(async () => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      await api.disconnectCursorComposer();
      setCursorComposerStatus({ connected: false, connectionState: "disconnected" });
      setProvidersMessage("Déconnecté");
      void loadConfiguredProviders();
      window.dispatchEvent(new CustomEvent(PROVIDERS_CHANGED_EVENT));
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersBusy(false);
    }
  }, [loadConfiguredProviders]);

  const connectKimi = useCallback(async () => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      const login = await api.startKimiOAuthLogin();
      const connecting: KimiProviderStatus = {
        connected: false,
        connectionState: "connecting",
        loginId: login.loginId,
      };
      setKimiStatus(connecting);
      await api.openExternalUrl(login.authUrl);
      setProvidersMessage(`Waiting for browser confirmation (${login.userCode})...`);
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
      void loadKimiStatus();
    } finally {
      setProvidersBusy(false);
    }
  }, [loadKimiStatus]);

  const cancelKimi = useCallback(async () => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      setKimiStatus(await api.cancelKimiOAuthLogin());
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersBusy(false);
    }
  }, []);

  const disconnectKimi = useCallback(async () => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      setKimiStatus(await api.disconnectKimiProvider());
      setProvidersMessage("Disconnected");
      void loadConfiguredProviders();
      window.dispatchEvent(new CustomEvent(PROVIDERS_CHANGED_EVENT));
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersBusy(false);
    }
  }, [loadConfiguredProviders]);

  const disconnectOpenRouter = useCallback(async () => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      setOpenRouterStatus(await api.disconnectOpenRouterProvider());
      setProvidersMessage("Disconnected");
      void loadConfiguredProviders();
      window.dispatchEvent(new CustomEvent(PROVIDERS_CHANGED_EVENT));
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersBusy(false);
    }
  }, [loadConfiguredProviders]);

  const handleOpenRouterChanged = useCallback(() => {
    void loadConfiguredProviders();
    window.dispatchEvent(new CustomEvent(PROVIDERS_CHANGED_EVENT));
  }, [loadConfiguredProviders]);

  const disconnectDeepSeek = useCallback(async () => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      setDeepSeekStatus(await api.disconnectDeepSeekProvider());
      setProvidersMessage("Disconnected");
      void loadConfiguredProviders();
      window.dispatchEvent(new CustomEvent(PROVIDERS_CHANGED_EVENT));
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersBusy(false);
    }
  }, [loadConfiguredProviders]);

  const handleDeepSeekChanged = useCallback(() => {
    void loadConfiguredProviders();
    window.dispatchEvent(new CustomEvent(PROVIDERS_CHANGED_EVENT));
  }, [loadConfiguredProviders]);

  const saveAndDetect = useCallback(async () => {
    setSaving(true);
    setStatus(null);
    try {
      const parsed = parseMcpJson(jsonText);
      const saved = await api.saveMcpSettings(parsed);
      const normalized = normalizeSettings(saved);
      const nextJson = settingsToJson(normalized);
      setSettings(normalized);
      setSavedJson(nextJson);
      setJsonText(nextJson);
      setParseError(null);

      const nextProbes = await api.probeMcpTools();
      setProbes(nextProbes);
      const failures = nextProbes.filter((probe) => probe.enabled && !probe.ok).length;
      if (failures) {
        setStatus(`${failures} server${failures === 1 ? "" : "s"} failed`);
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setParseError(message);
      setStatus(message);
    } finally {
      setSaving(false);
    }
  }, [jsonText]);

  const toggleEnabled = useCallback(
    async (id: string) => {
      if (parseError || saving) return;
      const next = normalizeSettings({
        servers: settings.servers.map((server) =>
          server.id === id ? { ...server, enabled: !server.enabled } : server,
        ),
      });
      const optimisticJson = settingsToJson(next);
      setSettings(next);
      setJsonText(optimisticJson);
      setSaving(true);
      setStatus(null);
      try {
        const saved = normalizeSettings(await api.saveMcpSettings(next));
        const nextJson = settingsToJson(saved);
        setSettings(saved);
        setSavedJson(nextJson);
        setJsonText(nextJson);
        setParseError(null);

        const nextProbes = await api.probeMcpTools();
        setProbes(nextProbes);
        const failures = nextProbes.filter((probe) => probe.enabled && !probe.ok).length;
        setStatus(
          failures
            ? `${failures} server${failures === 1 ? "" : "s"} failed`
            : "Saved",
        );
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setParseError(message);
        setStatus(message);
      } finally {
        setSaving(false);
      }
    },
    [parseError, saving, settings],
  );

  const toggleAutoLoad = useCallback(
    async (id: string) => {
      if (parseError || saving) return;
      const next = normalizeSettings({
        servers: settings.servers.map((server) =>
          server.id === id ? { ...server, autoLoad: !server.autoLoad } : server,
        ),
      });
      const optimisticJson = settingsToJson(next);
      setSettings(next);
      setJsonText(optimisticJson);
      setSaving(true);
      setStatus(null);
      try {
        const saved = normalizeSettings(await api.saveMcpSettings(next));
        const nextJson = settingsToJson(saved);
        setSettings(saved);
        setSavedJson(nextJson);
        setJsonText(nextJson);
        setParseError(null);

        const nextProbes = await api.probeMcpTools();
        setProbes(nextProbes);
        const failures = nextProbes.filter((probe) => probe.enabled && !probe.ok).length;
        setStatus(
          failures
            ? `${failures} server${failures === 1 ? "" : "s"} failed`
            : "Saved",
        );
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setParseError(message);
        setStatus(message);
      } finally {
        setSaving(false);
      }
    },
    [parseError, saving, settings],
  );

  const refreshMcpProbes = useCallback(async () => {
    if (!settings.servers.some((server) => server.enabled)) {
      setProbes([]);
      return;
    }
    setProbing(true);
    try {
      const nextProbes = await api.probeMcpTools();
      setProbes(nextProbes);
      const failures = nextProbes.filter((probe) => probe.enabled && !probe.ok).length;
      if (failures) {
        setStatus(`${failures} server${failures === 1 ? "" : "s"} failed`);
      }
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err));
    } finally {
      setProbing(false);
    }
  }, [settings.servers]);

  const autoProbedServersRef = useRef<Set<string>>(new Set());

  // Clear probed servers cache when saving changes or settings are reloaded
  useEffect(() => {
    if (saving || loading) {
      autoProbedServersRef.current.clear();
    }
  }, [saving, loading]);

  useEffect(() => {
    if (loading || saving || probing) return;
    if (!selectedServer?.enabled) return;
    if (selectedServerId && autoProbedServersRef.current.has(selectedServerId)) return;

    if (selectedServerId) {
      autoProbedServersRef.current.add(selectedServerId);
    }
    void refreshMcpProbes();
  }, [
    loading,
    probing,
    refreshMcpProbes,
    saving,
    selectedServer?.enabled,
    selectedServerId,
  ]);

  // ---- Skills load ------------------------------------------------------
  const loadSkills = useCallback(async () => {
    setSkillsLoading(true);
    setSkillsError(null);
    setSkillsStatus(null);
    try {
      const list = await api.listInstalledSkills(workspacePath);
      setSkills(list);
      setSavedSkillsJson(skillsFingerprint(list));
      setSelectedSkillName((current) => {
        if (current && list.some((item) => item.name === current)) return current;
        return list[0]?.name ?? null;
      });
    } catch (err) {
      setSkillsError(err instanceof Error ? err.message : String(err));
      setSkills([]);
      setSavedSkillsJson(skillsFingerprint([]));
    } finally {
      setSkillsLoading(false);
    }
  }, [workspacePath]);

  useEffect(() => {
    if (skills !== null) return;
    void loadSkills();
  }, [skills, loadSkills]);

  const loadSubAgents = useCallback(async () => {
    setSubAgentsLoading(true);
    setSubAgentsStatus(null);
    try {
      const loaded = normalizeSubAgentSettings(await api.listSubAgentSettings());
      setSubAgentSettings(loaded);
      setSavedSubAgentJson(subAgentSettingsFingerprint(loaded));
      setSelectedSubAgentId((current) => {
        if (current && loaded.agents.some((agent) => agent.id === current)) {
          return current;
        }
        return loaded.agents[0]?.id ?? null;
      });
    } catch (err) {
      setSubAgentsStatus(err instanceof Error ? err.message : String(err));
    } finally {
      setSubAgentsLoading(false);
    }
  }, []);

  useEffect(() => {
    if (savedSubAgentJson) return;
    void loadSubAgents();
  }, [savedSubAgentJson, loadSubAgents]);

  const filteredSkills = useMemo(() => {
    if (!skills) return [];
    const needle = skillFilter.trim().toLowerCase();
    if (!needle) return skills;
    return skills.filter((skill) => {
      if (skill.name.toLowerCase().includes(needle)) return true;
      if (skill.description?.toLowerCase().includes(needle)) return true;
      if (skill.rootLabel.toLowerCase().includes(needle)) return true;
      return false;
    });
  }, [skills, skillFilter]);

  const selectedSkill =
    skills?.find((skill) => skill.name === selectedSkillName) ?? null;
  const skillsDirty =
    skills !== null && skillsFingerprint(skills) !== savedSkillsJson;

  const toggleSkillEnabled = useCallback((name: string) => {
    setSkills((current) => {
      if (!current) return current;
      return current.map((skill) =>
        skill.name === name ? { ...skill, enabled: !skill.enabled } : skill,
      );
    });
  }, []);

  const saveSkills = useCallback(async () => {
    if (!skills) return;
    setSkillsSaving(true);
    setSkillsError(null);
    setSkillsStatus(null);
    try {
      const saved = await api.saveSkillSettings(workspacePath, settingsFromSkills(skills));
      setSkills(saved);
      setSavedSkillsJson(skillsFingerprint(saved));
      setSelectedSkillName((current) => {
        if (current && saved.some((skill) => skill.name === current)) return current;
        return saved[0]?.name ?? null;
      });
      setSkillsStatus("Saved");
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setSkillsError(message);
      setSkillsStatus(message);
    } finally {
      setSkillsSaving(false);
    }
  }, [skills, workspacePath]);

  const revealSkill = useCallback(async (skill: InstalledSkill) => {
    setSkillsError(null);
    setSkillsStatus(null);
    try {
      await api.revealAbsolutePath(skill.absolutePath);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setSkillsError(message);
      setSkillsStatus(message);
    }
  }, []);

  const deleteSkill = useCallback(
    async (skill: InstalledSkill) => {
      if (!skills) return;
      const nextSkills = skills.filter(
        (item) => item.absolutePath !== skill.absolutePath,
      );
      setSkillsDeleting(true);
      setSkillsError(null);
      setSkillsStatus(null);
      try {
        await api.deleteSkill(workspacePath, skill.absolutePath);
        if (skillsDirty) {
          setSkills(nextSkills);
        } else {
          const saved = await api.saveSkillSettings(
            workspacePath,
            settingsFromSkills(nextSkills),
          );
          setSkills(saved);
          setSavedSkillsJson(skillsFingerprint(saved));
        }
        setSelectedSkillName((current) => {
          if (current !== skill.name) return current;
          return nextSkills[0]?.name ?? null;
        });
        setSkillsStatus("Deleted");
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setSkillsError(message);
        setSkillsStatus(message);
      } finally {
        setSkillsDeleting(false);
      }
    },
    [skills, skillsDirty, workspacePath],
  );

  const createSkill = useCallback(async () => {
    setSkillsSaving(true);
    setSkillsError(null);
    setSkillsStatus(null);
    try {
      const { name, skills: refreshed } = await api.createSkill(workspacePath);
      setSkills(refreshed);
      setSavedSkillsJson(skillsFingerprint(refreshed));
      setSelectedSkillName(name);
      setSkillFilter("");
      setSkillsStatus("Created");
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setSkillsError(message);
      setSkillsStatus(message);
    } finally {
      setSkillsSaving(false);
    }
  }, [workspacePath]);

  const saveSkillContent = useCallback(
    async (skill: InstalledSkill, content: string) => {
      setSkillsSaving(true);
      setSkillsError(null);
      setSkillsStatus(null);
      try {
        const { name, skills: refreshed } = await api.updateSkillContent(
          workspacePath,
          skill.absolutePath,
          content,
        );
        const enabledByPath = new Map(
          (skills ?? []).map((item) => [item.absolutePath, item.enabled]),
        );
        const merged = refreshed.map((item) => {
          const enabled = enabledByPath.get(item.absolutePath);
          return enabled === undefined ? item : { ...item, enabled };
        });
        const saved = await api.saveSkillSettings(
          workspacePath,
          settingsFromSkills(merged),
        );
        setSkills(saved);
        setSavedSkillsJson(skillsFingerprint(saved));
        setSelectedSkillName(
          name || saved.find((item) => item.absolutePath === skill.absolutePath)?.name || skill.name,
        );
        setSkillFilter("");
        setSkillsStatus("Saved");
        return true;
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setSkillsError(message);
        setSkillsStatus(message);
        return false;
      } finally {
        setSkillsSaving(false);
      }
    },
    [skills, workspacePath],
  );

  const selectedSubAgent =
    subAgentSettings.agents.find((agent) => agent.id === selectedSubAgentId) ??
    null;
  const subAgentsDirty =
    subAgentSettingsFingerprint(subAgentSettings) !== savedSubAgentJson;

  const saveSubAgents = useCallback(async () => {
    setSubAgentsSaving(true);
    setSubAgentsStatus(null);
    try {
      const saved = normalizeSubAgentSettings(
        await api.saveSubAgentSettings(subAgentSettings),
      );
      setSubAgentSettings(saved);
      setSavedSubAgentJson(subAgentSettingsFingerprint(saved));
      setSelectedSubAgentId((current) => {
        if (current && saved.agents.some((agent) => agent.id === current)) {
          return current;
        }
        return saved.agents[0]?.id ?? null;
      });
      setSubAgentsStatus("Saved");
    } catch (err) {
      setSubAgentsStatus(err instanceof Error ? err.message : String(err));
    } finally {
      setSubAgentsSaving(false);
    }
  }, [subAgentSettings]);

  const updateSubAgent = useCallback(
    (id: string, patch: Partial<SubAgentConfig>) => {
      setSubAgentSettings((current) => ({
        agents: current.agents.map((agent) =>
          agent.id === id ? { ...agent, ...patch } : agent,
        ),
      }));
    },
    [],
  );

  const addSubAgent = useCallback(() => {
    const next = createSubAgent(
      subAgentSettings.agents.length + 1,
      availableModels,
    );
    setSubAgentSettings((current) => ({ agents: [...current.agents, next] }));
    setSelectedSubAgentId(next.id);
  }, [availableModels, subAgentSettings.agents.length]);

  const deleteSubAgent = useCallback((id: string) => {
    setSubAgentSettings((current) => {
      const agents = current.agents.filter((agent) => agent.id !== id);
      setSelectedSubAgentId((selected) => {
        if (selected !== id) return selected;
        return agents[0]?.id ?? null;
      });
      return { agents };
    });
  }, []);

  const handleEditorMount: OnMount = useCallback((editor, monaco) => {
    defineSinewCoolTheme(monaco);
    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
      void saveAndDetectRef.current();
    });
  }, []);

  // Mutable ref so the Monaco command picks up the latest closure.
  const saveAndDetectRef = useRef(saveAndDetect);
  useEffect(() => {
    saveAndDetectRef.current = saveAndDetect;
  }, [saveAndDetect]);

  return (
    <div className="settings-pane">
      <nav className="settings-pane__nav" aria-label="Settings sections">
        <button
          type="button"
          className="settings-pane__nav-item"
          data-active={section === "options" ? "true" : "false"}
          onClick={() => setSection("options")}
        >
          <Icon
            icon="solar:tuning-2-linear"
            width={15}
            height={15}
            className="settings-pane__nav-icon"
          />
          <span className="settings-pane__nav-label">Options</span>
        </button>
        <button
          type="button"
          className="settings-pane__nav-item"
          data-active={section === "providers" ? "true" : "false"}
          onClick={() => setSection("providers")}
        >
          <Icon
            icon="solar:cloud-check-linear"
            width={15}
            height={15}
            className="settings-pane__nav-icon"
          />
          <span className="settings-pane__nav-label">Providers</span>
          <span className="settings-pane__nav-count">
            {configuredProviders.length}
          </span>
        </button>
        <button
          type="button"
          className="settings-pane__nav-item"
          data-active={section === "tools" ? "true" : "false"}
          onClick={() => setSection("tools")}
        >
          <WrenchIcon size={15} className="settings-pane__nav-icon" />
          <span className="settings-pane__nav-label">Tools</span>
          <span className="settings-pane__nav-count">
            {toolSettings?.tools.length ?? "·"}
          </span>
        </button>
        <button
          type="button"
          className="settings-pane__nav-item"
          data-active={section === "mcp" ? "true" : "false"}
          onClick={() => setSection("mcp")}
        >
          <Icon
            icon="solar:server-square-cloud-linear"
            width={15}
            height={15}
            className="settings-pane__nav-icon"
          />
          <span className="settings-pane__nav-label">MCP</span>
          <span className="settings-pane__nav-count">{settings.servers.length}</span>
        </button>
        <button
          type="button"
          className="settings-pane__nav-item"
          data-active={section === "skills" ? "true" : "false"}
          onClick={() => setSection("skills")}
        >
          <Icon
            icon="solar:magic-stick-3-linear"
            width={15}
            height={15}
            className="settings-pane__nav-icon"
          />
          <span className="settings-pane__nav-label">Skills</span>
          <span className="settings-pane__nav-count">{skills?.length ?? "·"}</span>
        </button>
        <button
          type="button"
          className="settings-pane__nav-item"
          data-active={section === "subagents" ? "true" : "false"}
          onClick={() => setSection("subagents")}
        >
          <Icon
            icon="solar:branching-paths-down-linear"
            width={15}
            height={15}
            className="settings-pane__nav-icon"
          />
          <span className="settings-pane__nav-label">Agents</span>
          <span className="settings-pane__nav-count">
            {subAgentSettings.agents.length}
          </span>
        </button>
        <button
          type="button"
          className="settings-pane__nav-item"
          data-active={section === "about" ? "true" : "false"}
          onClick={() => setSection("about")}
        >
          <Icon
            icon="solar:info-circle-linear"
            width={15}
            height={15}
            className="settings-pane__nav-icon"
          />
          <span className="settings-pane__nav-label">About</span>
          <span className="settings-pane__nav-count" />
        </button>
      </nav>

      <section className="settings-pane__main">
        {section === "options" ? (
          <OptionsSection
            locale={locale}
            onLocaleChange={setLocale}
            workspacePath={workspacePath}
          />
        ) : section === "about" ? (
          <AboutSection locale={locale} />
        ) : section === "providers" ? (
          <ProvidersSection
            openAiStatus={openAiStatus}
            openAiAccounts={openAiAccounts}
            unconnectedAccounts={unconnectedAccounts}
            secondaryModels={secondaryModels}
            onUpdateSecondaryModel={handleUpdateSecondaryModel}
            secondaryThinking={secondaryThinking}
            onUpdateSecondaryThinking={handleUpdateSecondaryThinking}
            secondaryFast={secondaryFast}
            onUpdateSecondaryFast={handleUpdateSecondaryFast}
            onSaveOpenAiAccessToken={async (token, key) => { await api.saveOpenAiAccessToken(token, key); }}
            onDisconnectOpenAiAccount={disconnectOpenAiAccount}
            anthropicStatus={anthropicStatus}
            googleStatus={googleStatus}
            googleAccounts={googleAccounts}
            unconnectedGoogleAccounts={unconnectedGoogleAccounts}
            onConnectGoogleWithKey={connectGoogle}
            onAddGoogleAccount={handleAddGoogleAccount}
            onRemoveUnconnectedGoogleAccount={handleRemoveUnconnectedGoogleAccount}
            onDisconnectGoogleAccount={disconnectGoogleAccount}
            cursorComposerStatus={cursorComposerStatus}
            kimiStatus={kimiStatus}
            deepSeekStatus={deepSeekStatus}
            openRouterStatus={openRouterStatus}
            openRouterModels={openRouterModels}
            loading={providersLoading}
            busy={providersBusy}
            message={providersMessage}
            onRefresh={() => {
              quotaCache.clear();
              window.dispatchEvent(new CustomEvent("sinew:quota-updated"));
              void loadOpenAiStatus();
              void loadAnthropicStatus();
              void loadGoogleStatus();
              void loadCursorStatus();
              void loadKimiStatus();
              void loadDeepSeekStatus();
              void loadOpenRouterStatus();
            }}
            onConnect={() => void connectOpenAi()}
            onConnectWithKey={(key) => void connectOpenAi(key)}
            onAddOpenAiAccount={handleAddOpenAiAccount}
            onRemoveUnconnectedAccount={handleRemoveUnconnectedAccount}
            onCancel={() => void cancelOpenAi()}
            onDisconnect={() => void disconnectOpenAi()}
            onConnectAnthropic={() => void connectAnthropic()}
            onCancelAnthropic={() => void cancelAnthropic()}
            onDisconnectAnthropic={() => void disconnectAnthropic()}
            onConnectGoogle={() => void connectGoogle()}
            onCancelGoogle={() => void cancelGoogle()}
            onDisconnectGoogle={() => void disconnectGoogle()}
            onConnectCursor={() => void connectCursor()}
            onCancelCursorComposer={() => void cancelCursorComposer()}
            onDisconnectCursorComposer={() => void disconnectCursorComposer()}
            onConnectKimi={() => void connectKimi()}
            onCancelKimi={() => void cancelKimi()}
            onDisconnectKimi={() => void disconnectKimi()}
            onDisconnectOpenRouter={() => void disconnectOpenRouter()}
            onOpenRouterStatusChange={setOpenRouterStatus}
            onOpenRouterModelsChange={setOpenRouterModels}
            onOpenRouterChanged={handleOpenRouterChanged}
            onDisconnectDeepSeek={() => void disconnectDeepSeek()}
            onDeepSeekStatusChange={setDeepSeekStatus}
            onDeepSeekChanged={handleDeepSeekChanged}
          />
        ) : section === "tools" ? (
          <ToolsSection
            settings={toolSettings}
            loading={toolsLoading}
            saving={toolsSaving}
            dirty={toolsDirty}
            status={toolsStatus}
            onSave={() => void saveToolSettings()}
            onUpdate={updateTool}
            onPlanModePromptChange={updatePlanModePrompt}
            onImageProviderChange={updateImageProvider}
            onOpenAiImageUseSubscriptionChange={updateOpenAiImageUseSubscription}
            onGeminiImageUseSubscriptionChange={updateGeminiImageUseSubscription}
            onOpenAiImageModelChange={updateOpenAiImageModel}
            onGeminiImageModelChange={updateGeminiImageModel}
            onOpenAiImageApiKeyChange={updateOpenAiImageApiKey}
            onNanoBananaApiKeyChange={updateNanoBananaApiKey}
            onWebSearchProviderChange={updateWebSearchProvider}
            onLinkupApiKeyChange={updateLinkupApiKey}
            openAiStatus={openAiStatus}
            googleStatus={googleStatus}
          />
        ) : section === "mcp" ? (
          <McpSection
            workspacePath={workspacePath}
            loading={loading}
            saving={saving}
            probing={probing}
            dirty={dirty}
            status={status}
            parseError={parseError}
            jsonText={jsonText}
            onJsonChange={(value) => setJsonText(value)}
            onSave={() => void saveAndDetect()}
            servers={settings.servers}
            probes={probes}
            onSelectServer={(id) => {
              setSelectedServerId(id);
              setMcpAdvancedOpen(false);
            }}
            selectedServer={selectedServer}
            advancedOpen={mcpAdvancedOpen}
            onAdvancedOpenChange={setMcpAdvancedOpen}
            selectedProbe={selectedProbe}
            knownToolCounts={knownToolCounts}
            onToggleEnabled={toggleEnabled}
            onToggleAutoLoad={toggleAutoLoad}
            onRefreshProbes={() => void refreshMcpProbes()}
            onMount={handleEditorMount}
          />
        ) : section === "skills" ? (
          <SkillsSection
            skills={filteredSkills}
            allSkills={skills}
            loading={skillsLoading}
            saving={skillsSaving}
            dirty={skillsDirty}
            error={skillsError}
            status={skillsStatus}
            filter={skillFilter}
            onFilterChange={setSkillFilter}
            selectedSkill={selectedSkill}
            deleting={skillsDeleting}
            onSelectSkill={(name) => setSelectedSkillName(name)}
            onRefresh={() => void loadSkills()}
            onSave={() => void saveSkills()}
            onCreate={() => void createSkill()}
            onToggleSkill={toggleSkillEnabled}
            onRevealSkill={(skill) => void revealSkill(skill)}
            onDeleteSkill={(skill) => void deleteSkill(skill)}
            onSaveSkillContent={saveSkillContent}
          />
        ) : (
          <SubAgentsSection
            settings={subAgentSettings}
            selectedAgent={selectedSubAgent}
            loading={subAgentsLoading}
            saving={subAgentsSaving}
            dirty={subAgentsDirty}
            status={subAgentsStatus}
            availableModels={availableModels}
            onSelect={setSelectedSubAgentId}
            onAdd={addSubAgent}
            onDelete={deleteSubAgent}
            onSave={() => void saveSubAgents()}
            onUpdate={updateSubAgent}
          />
        )}
      </section>
    </div>
  );
}

// ---- Options section ----------------------------------------------------

function OptionsSection({
  locale,
  onLocaleChange,
  workspacePath,
}: {
  locale: AppLocale;
  onLocaleChange: (locale: AppLocale) => void;
  workspacePath: string;
}) {
  const [powerUserMaster, setPowerUserMaster] = useState<"enabled" | "disabled" | "custom">(() => {
    try {
      const saved = localStorage.getItem("sinew.power-user-master");
      if (saved === "enabled" || saved === "disabled" || saved === "custom") {
        return saved;
      }
      const oldPowerUser = localStorage.getItem("sinew.power-user");
      if (oldPowerUser === "false") {
        return "disabled";
      }
      return "enabled";
    } catch {
      return "enabled";
    }
  });

  const [powerUser, setPowerUser] = useState<boolean>(() => {
    try {
      const saved = localStorage.getItem("sinew.power-user");
      if (saved !== null) return saved !== "false";
      const master = localStorage.getItem("sinew.power-user-master") || "enabled";
      if (master === "enabled") return true;
      if (master === "disabled") return false;
      return true;
    } catch {
      return true;
    }
  });

  const [gitAutomation, setGitAutomation] = useState<boolean>(() => {
    try {
      const saved = localStorage.getItem("sinew.git-automation");
      if (saved !== null) return saved !== "false";
      const master = localStorage.getItem("sinew.power-user-master") || "enabled";
      if (master === "enabled") return true;
      if (master === "disabled") return false;
      return true;
    } catch {
      return true;
    }
  });

  const [conciseAnswers, setConciseAnswers] = useState<boolean>(() => {
    try {
      const saved = localStorage.getItem("sinew.concise-answers");
      if (saved !== null) return saved !== "false";
      const master = localStorage.getItem("sinew.power-user-master") || "enabled";
      if (master === "enabled") return true;
      if (master === "disabled") return false;
      return true;
    } catch {
      return true;
    }
  });

  const [compactReasoning, setCompactReasoning] = useState<"disabled" | "compact" | "very-compact">(() => {
    try {
      const val = localStorage.getItem("sinew.compact-reasoning");
      if (val === "very-compact") return "very-compact";
      if (val === "compact" || val === "true") return "compact";
      if (val === "disabled" || val === "false") return "disabled";
      const master = localStorage.getItem("sinew.power-user-master") || "enabled";
      if (master === "enabled") return "very-compact";
      if (master === "disabled") return "disabled";
      return "disabled";
    } catch {
      return "disabled";
    }
  });

  const [multiPcSync, setMultiPcSync] = useState<boolean>(false);

  const [autosave, setAutosave] = useState<boolean>(() => {
    try {
      const saved = localStorage.getItem("sinew.autosave");
      if (saved !== null) return saved === "true";
      const master = localStorage.getItem("sinew.power-user-master") || "enabled";
      if (master === "enabled") return true;
      if (master === "disabled") return false;
      return false;
    } catch {
      return false;
    }
  });

  const [editorFontSize, setEditorFontSize] = useState<number>(() => {
    try {
      const saved = localStorage.getItem("sinew.editor-font-size");
      return saved ? parseInt(saved, 10) : 12;
    } catch {
      return 12;
    }
  });

  const [chatFontSize, setChatFontSize] = useState<number>(() => {
    try {
      const saved = localStorage.getItem("sinew.chat-font-size");
      return saved ? parseInt(saved, 10) : 13;
    } catch {
      return 13;
    }
  });

  const [agentAutonomy, setAgentAutonomy] = useState<boolean>(() => {
    try {
      const saved = localStorage.getItem("sinew.agent-autonomy");
      if (saved !== null) return saved !== "false";
      const master = localStorage.getItem("sinew.power-user-master") || "enabled";
      if (master === "enabled") return true;
      if (master === "disabled") return false;
      return true;
    } catch {
      return true;
    }
  });

  const [forceChangelog, setForceChangelog] = useState<boolean>(() => {
    try {
      const saved = localStorage.getItem("sinew.force-changelog");
      if (saved !== null) return saved === "true";
      const master = localStorage.getItem("sinew.power-user-master") || "enabled";
      if (master === "enabled") return true;
      if (master === "disabled") return false;
      return false;
    } catch {
      return false;
    }
  });

  const [gitFrenchMessages, setGitFrenchMessages] = useState<boolean>(() => {
    try {
      const saved = localStorage.getItem("sinew.git-french-messages");
      if (saved !== null) return saved !== "false";
      const master = localStorage.getItem("sinew.power-user-master") || "enabled";
      if (master === "enabled") return true;
      if (master === "disabled") return false;
      return true;
    } catch {
      return true;
    }
  });

  const [autoMockups, setAutoMockups] = useState<boolean>(() => {
    try {
      const saved = localStorage.getItem("sinew.auto-mockups");
      if (saved !== null) return saved !== "false";
      const master = localStorage.getItem("sinew.power-user-master") || "enabled";
      if (master === "enabled") return true;
      if (master === "disabled") return false;
      return true;
    } catch {
      return true;
    }
  });

  const [autoUpdateCheck, setAutoUpdateCheck] = useState<"blocking" | "notification" | "disabled">(() => {
    try {
      const saved = localStorage.getItem("sinew.auto-update-check");
      if (saved === "blocking" || saved === "notification" || saved === "disabled") {
        return saved;
      }
      if (saved === "false") {
        return "disabled";
      }
      return "blocking";
    } catch {
      return "blocking";
    }
  });

  const changeAutoUpdateCheck = (mode: "blocking" | "notification" | "disabled") => {
    try {
      localStorage.setItem("sinew.auto-update-check", mode);
    } catch {}
    setAutoUpdateCheck(mode);
    window.dispatchEvent(new CustomEvent("sinew:auto-update-check-changed", { detail: mode }));
  };

  const [semanticEmbeddings, setSemanticEmbeddings] = useState<boolean>(() => {
    try {
      return localStorage.getItem("sinew.semantic-embeddings") === "true";
    } catch {
      return false;
    }
  });

  const toggleSemanticEmbeddings = (enabled: boolean) => {
    try {
      localStorage.setItem("sinew.semantic-embeddings", enabled ? "true" : "false");
    } catch {}
    setSemanticEmbeddings(enabled);
    api.setSemanticEmbeddingsEnabled(enabled).catch(() => {});
    window.dispatchEvent(new CustomEvent("sinew:semantic-embeddings-changed", { detail: enabled }));
  };

  const toggleAutosave = (enabled: boolean) => {
    try {
      localStorage.setItem("sinew.autosave", enabled ? "true" : "false");
    } catch {}
    setAutosave(enabled);
    window.dispatchEvent(new CustomEvent("sinew:autosave-changed", { detail: enabled }));
  };

  const changeEditorFontSize = (size: number) => {
    try {
      localStorage.setItem("sinew.editor-font-size", size.toString());
      document.documentElement.style.setProperty("--editor-font-size", `${size}px`);
    } catch {}
    setEditorFontSize(size);
    window.dispatchEvent(new CustomEvent("sinew:editor-font-size-changed", { detail: size }));
  };

  const changeChatFontSize = (size: number) => {
    try {
      localStorage.setItem("sinew.chat-font-size", size.toString());
    } catch {}
    setChatFontSize(size);
    document.documentElement.style.setProperty("--chat-font-size", `${size}px`);
    window.dispatchEvent(new CustomEvent("sinew:chat-font-size-changed", { detail: size }));
  };

  const toggleAgentAutonomy = (enabled: boolean) => {
    try {
      localStorage.setItem("sinew.agent-autonomy", enabled ? "true" : "false");
    } catch {}
    setAgentAutonomy(enabled);
    window.dispatchEvent(new CustomEvent("sinew:agent-autonomy-changed", { detail: enabled }));
  };

  const toggleForceChangelog = (enabled: boolean) => {
    try {
      localStorage.setItem("sinew.force-changelog", enabled ? "true" : "false");
    } catch {}
    setForceChangelog(enabled);
    window.dispatchEvent(new CustomEvent("sinew:force-changelog-changed", { detail: enabled }));
  };

  const [leftWidth, setLeftWidth] = useState<number>(() => {
    try {
      const saved = localStorage.getItem("sinew.left-width");
      return saved ? parseInt(saved, 10) : 280;
    } catch {
      return 280;
    }
  });

  const [rightWidth, setRightWidth] = useState<number>(() => {
    try {
      const saved = localStorage.getItem("sinew.right-width");
      return saved ? parseInt(saved, 10) : 420;
    } catch {
      return 420;
    }
  });

  useEffect(() => {
    const handleLeft = (event: Event) => {
      const w = (event as CustomEvent<number>).detail;
      setLeftWidth(Math.round(w));
    };
    const handleRight = (event: Event) => {
      const w = (event as CustomEvent<number>).detail;
      setRightWidth(Math.round(w));
    };
    window.addEventListener("sinew:left-width-updated", handleLeft);
    window.addEventListener("sinew:right-width-updated", handleRight);
    return () => {
      window.removeEventListener("sinew:left-width-updated", handleLeft);
      window.removeEventListener("sinew:right-width-updated", handleRight);
    };
  }, []);

  const changeLeftWidth = (size: number) => {
    const rounded = Math.round(size);
    setLeftWidth(rounded);
    window.dispatchEvent(new CustomEvent("sinew:left-width-changed", { detail: rounded }));
  };

  const changeRightWidth = (size: number) => {
    const rounded = Math.round(size);
    setRightWidth(rounded);
    window.dispatchEvent(new CustomEvent("sinew:right-width-changed", { detail: rounded }));
  };

  const [largeChatBox, setLargeChatBox] = useState<boolean>(() => {
    try {
      return localStorage.getItem("sinew.large-chat-box") === "true";
    } catch {
      return false;
    }
  });

  const toggleLargeChatBox = (enabled: boolean) => {
    try {
      localStorage.setItem("sinew.large-chat-box", enabled ? "true" : "false");
      document.documentElement.setAttribute("data-large-chat-box", enabled ? "true" : "false");
    } catch {}
    setLargeChatBox(enabled);
    window.dispatchEvent(new CustomEvent("sinew:large-chat-box-changed", { detail: enabled }));
  };

  useEffect(() => {
    const handler = (event: Event) => {
      const enabled = (event as CustomEvent<boolean>).detail;
      setLargeChatBox(enabled);
    };
    window.addEventListener("sinew:large-chat-box-changed", handler as any);
    return () => window.removeEventListener("sinew:large-chat-box-changed", handler as any);
  }, []);

  const [theme, setTheme] = useState<"dark" | "light" | "system" | "ai">(() => {
    try {
      const val = localStorage.getItem("sinew.theme");
      if (val === "light" || val === "system" || val === "ai") return val;
      return "dark";
    } catch {
      return "dark";
    }
  });

  const changeTheme = (newTheme: "dark" | "light" | "system" | "ai") => {
    try {
      localStorage.setItem("sinew.theme", newTheme);
    } catch {}
    setTheme(newTheme);
    document.documentElement.setAttribute("data-theme", newTheme);
    window.dispatchEvent(new CustomEvent("sinew:theme-changed", { detail: newTheme }));
  };

  const [sotaData, setSotaData] = useState<any>(sotaCache.data);
  const [loadingSota, setLoadingSota] = useState<boolean>(!sotaCache.data && !!sotaCache.promise);
  const [sotaError, setSotaError] = useState<string | null>(sotaCache.error);

  // ---- Apprentissage automatique IA ----
  const [autoLearningEnabled, setAutoLearningEnabled] = useState<boolean>(() => {
    try {
      return localStorage.getItem("sinew.auto-learning") === "true";
    } catch {
      return false;
    }
  });
  const [autoLearningProviderId, setAutoLearningProviderId] = useState<string>(() => {
    try {
      return localStorage.getItem("sinew.auto-learning-provider") || "deepseek";
    } catch {
      return "deepseek";
    }
  });
  const [autoLearningLoading, setAutoLearningLoading] = useState(false);
  const [autoLearningStatus, setAutoLearningStatus] = useState<string | null>(null);
  const [configuredProviders, setConfiguredProviders] = useState<string[]>([]);

  // Charger les fournisseurs configurés
  useEffect(() => {
    api.listConfiguredModelProviders()
      .then((providers) => setConfiguredProviders(providers))
      .catch(() => setConfiguredProviders([]));
  }, []);

  const toggleAutoLearning = (enabled: boolean) => {
    try {
      localStorage.setItem("sinew.auto-learning", enabled ? "true" : "false");
    } catch {}
    setAutoLearningEnabled(enabled);
  };

  const runAiConsolidation = async () => {
    setAutoLearningLoading(true);
    setAutoLearningStatus(null);
    try {
      const result = await api.triggerAiRuleConsolidation(autoLearningProviderId);
      setAutoLearningStatus(result);
    } catch (err: any) {
      setAutoLearningStatus(`Erreur: ${err?.toString() || "Inconnue"}`);
    } finally {
      setAutoLearningLoading(false);
    }
  };

  const runSotaDiagnostics = useCallback(async (force = false) => {
    if (!force && sotaCache.data) {
      setSotaData(sotaCache.data);
      setSotaError(sotaCache.error);
      setLoadingSota(false);
      return;
    }
    setLoadingSota(true);
    setSotaError(null);
    try {
      const parsed = await triggerSotaDiagnostics(force);
      setSotaData(parsed);
      setSotaError(null);
    } catch (err: any) {
      setSotaError(sotaCache.error || err.toString());
    } finally {
      setLoadingSota(false);
    }
  }, []);

  useEffect(() => {
    runSotaDiagnostics(false);
  }, [runSotaDiagnostics]);

  useEffect(() => {
    api.isMultiPcSyncEnabled().then((enabled) => {
      setMultiPcSync(enabled);
    }).catch(() => {});

    // Initialiser la variable d'environnement de recherche sémantique côté Rust
    try {
      const saved = localStorage.getItem("sinew.semantic-embeddings") === "true";
      api.setSemanticEmbeddingsEnabled(saved).catch(() => {});
    } catch {}
  }, []);

  useEffect(() => {
    try {
      const saved = localStorage.getItem("sinew.editor-font-size");
      const size = saved ? parseInt(saved, 10) : 12;
      document.documentElement.style.setProperty("--editor-font-size", `${size}px`);
    } catch {}
  }, []);

  useEffect(() => {
    const master = localStorage.getItem("sinew.power-user-master") || "enabled";
    if (master === "enabled") {
      const keysToSet: Record<string, string> = {
        "sinew.power-user": "true",
        "sinew.git-automation": "true",
        "sinew.concise-answers": "true",
        "sinew.force-changelog": "true",
        "sinew.git-french-messages": "true",
        "sinew.auto-mockups": "true",
        "sinew.autosave": "true",
        "sinew.compact-reasoning": "very-compact",
        "sinew.agent-autonomy": "true",
      };
      let changed = false;
      for (const [key, value] of Object.entries(keysToSet)) {
        if (localStorage.getItem(key) !== value) {
          localStorage.setItem(key, value);
          changed = true;
        }
      }
      if (changed) {
        setPowerUser(true);
        setGitAutomation(true);
        setConciseAnswers(true);
        setForceChangelog(true);
        setGitFrenchMessages(true);
        setAutoMockups(true);
        setAutosave(true);
        setCompactReasoning("very-compact");
        setAgentAutonomy(true);
      }
    } else if (master === "disabled") {
      const keysToSet: Record<string, string> = {
        "sinew.power-user": "false",
        "sinew.git-automation": "false",
        "sinew.concise-answers": "false",
        "sinew.force-changelog": "false",
        "sinew.git-french-messages": "false",
        "sinew.auto-mockups": "false",
        "sinew.autosave": "false",
        "sinew.compact-reasoning": "disabled",
        "sinew.agent-autonomy": "false",
      };
      let changed = false;
      for (const [key, value] of Object.entries(keysToSet)) {
        if (localStorage.getItem(key) !== value) {
          localStorage.setItem(key, value);
          changed = true;
        }
      }
      if (changed) {
        setPowerUser(false);
        setGitAutomation(false);
        setConciseAnswers(false);
        setForceChangelog(false);
        setGitFrenchMessages(false);
        setAutoMockups(false);
        setAutosave(false);
        setCompactReasoning("disabled");
        setAgentAutonomy(false);
      }
    }
  }, []);

  const changePowerUserMaster = (value: "enabled" | "disabled" | "custom") => {
    try {
      localStorage.setItem("sinew.power-user-master", value);
    } catch {}
    setPowerUserMaster(value);

    if (value === "enabled") {
      try {
        localStorage.setItem("sinew.power-user", "true");
        localStorage.setItem("sinew.git-automation", "true");
        localStorage.setItem("sinew.concise-answers", "true");
        localStorage.setItem("sinew.force-changelog", "true");
        localStorage.setItem("sinew.git-french-messages", "true");
        localStorage.setItem("sinew.auto-mockups", "true");
        localStorage.setItem("sinew.autosave", "true");
        localStorage.setItem("sinew.compact-reasoning", "very-compact");
        localStorage.setItem("sinew.agent-autonomy", "true");
      } catch {}
      setPowerUser(true);
      setGitAutomation(true);
      setConciseAnswers(true);
      setForceChangelog(true);
      setGitFrenchMessages(true);
      setAutoMockups(true);
      setAutosave(true);
      setCompactReasoning("very-compact");
      setAgentAutonomy(true);

      window.dispatchEvent(new CustomEvent("sinew:power-user-changed", { detail: true }));
      window.dispatchEvent(new CustomEvent("sinew:git-automation-changed", { detail: true }));
      window.dispatchEvent(new CustomEvent("sinew:concise-answers-changed", { detail: true }));
      window.dispatchEvent(new CustomEvent("sinew:force-changelog-changed", { detail: true }));
      window.dispatchEvent(new CustomEvent("sinew:git-french-messages-changed", { detail: true }));
      window.dispatchEvent(new CustomEvent("sinew:auto-mockups-changed", { detail: true }));
      window.dispatchEvent(new CustomEvent("sinew:autosave-changed", { detail: true }));
      window.dispatchEvent(new CustomEvent("sinew:compact-reasoning-changed", { detail: "very-compact" }));
      window.dispatchEvent(new CustomEvent("sinew:agent-autonomy-changed", { detail: true }));
    } else if (value === "disabled") {
      try {
        localStorage.setItem("sinew.power-user", "false");
        localStorage.setItem("sinew.git-automation", "false");
        localStorage.setItem("sinew.concise-answers", "false");
        localStorage.setItem("sinew.force-changelog", "false");
        localStorage.setItem("sinew.git-french-messages", "false");
        localStorage.setItem("sinew.auto-mockups", "false");
        localStorage.setItem("sinew.autosave", "false");
        localStorage.setItem("sinew.compact-reasoning", "disabled");
        localStorage.setItem("sinew.agent-autonomy", "false");
      } catch {}
      setPowerUser(false);
      setGitAutomation(false);
      setConciseAnswers(false);
      setForceChangelog(false);
      setGitFrenchMessages(false);
      setAutoMockups(false);
      setAutosave(false);
      setCompactReasoning("disabled");
      setAgentAutonomy(false);

      window.dispatchEvent(new CustomEvent("sinew:power-user-changed", { detail: false }));
      window.dispatchEvent(new CustomEvent("sinew:git-automation-changed", { detail: false }));
      window.dispatchEvent(new CustomEvent("sinew:concise-answers-changed", { detail: false }));
      window.dispatchEvent(new CustomEvent("sinew:force-changelog-changed", { detail: false }));
      window.dispatchEvent(new CustomEvent("sinew:git-french-messages-changed", { detail: false }));
      window.dispatchEvent(new CustomEvent("sinew:auto-mockups-changed", { detail: false }));
      window.dispatchEvent(new CustomEvent("sinew:autosave-changed", { detail: false }));
      window.dispatchEvent(new CustomEvent("sinew:compact-reasoning-changed", { detail: "disabled" }));
      window.dispatchEvent(new CustomEvent("sinew:agent-autonomy-changed", { detail: false }));
    }
  };

  const toggleGitAutomation = (enabled: boolean) => {
    try {
      localStorage.setItem("sinew.git-automation", enabled ? "true" : "false");
      localStorage.setItem("sinew.power-user", (enabled || conciseAnswers) ? "true" : "false");
    } catch {}
    setGitAutomation(enabled);
    setPowerUser(enabled || conciseAnswers);
    window.dispatchEvent(new CustomEvent("sinew:git-automation-changed", { detail: enabled }));
    window.dispatchEvent(new CustomEvent("sinew:power-user-changed", { detail: enabled || conciseAnswers }));
  };

  const toggleConciseAnswers = (enabled: boolean) => {
    try {
      localStorage.setItem("sinew.concise-answers", enabled ? "true" : "false");
      localStorage.setItem("sinew.power-user", (gitAutomation || enabled) ? "true" : "false");
    } catch {}
    setConciseAnswers(enabled);
    setPowerUser(gitAutomation || enabled);
    window.dispatchEvent(new CustomEvent("sinew:concise-answers-changed", { detail: enabled }));
    window.dispatchEvent(new CustomEvent("sinew:power-user-changed", { detail: gitAutomation || enabled }));
  };

  const toggleGitFrenchMessages = (enabled: boolean) => {
    try {
      localStorage.setItem("sinew.git-french-messages", enabled ? "true" : "false");
    } catch {}
    setGitFrenchMessages(enabled);
    window.dispatchEvent(new CustomEvent("sinew:git-french-messages-changed", { detail: enabled }));
  };

  const toggleAutoMockups = (enabled: boolean) => {
    try {
      localStorage.setItem("sinew.auto-mockups", enabled ? "true" : "false");
    } catch {}
    setAutoMockups(enabled);
    window.dispatchEvent(new CustomEvent("sinew:auto-mockups-changed", { detail: enabled }));
  };

  const changeCompactReasoning = (value: "disabled" | "compact" | "very-compact") => {
    try {
      localStorage.setItem("sinew.compact-reasoning", value);
    } catch {}
    setCompactReasoning(value);
    window.dispatchEvent(new CustomEvent("sinew:compact-reasoning-changed", { detail: value }));
  };

  const toggleMultiPcSync = (enabled: boolean) => {
    api.setMultiPcSyncEnabled(enabled).then(() => {
      setMultiPcSync(enabled);
    }).catch(() => {});
  };

  const activeGitAutomation = powerUserMaster === "enabled" ? true : (powerUserMaster === "disabled" ? false : gitAutomation);
  const activeConciseAnswers = powerUserMaster === "enabled" ? true : (powerUserMaster === "disabled" ? false : conciseAnswers);
  const activeForceChangelog = powerUserMaster === "enabled" ? true : (powerUserMaster === "disabled" ? false : forceChangelog);
  const activeGitFrenchMessages = powerUserMaster === "enabled" ? true : (powerUserMaster === "disabled" ? false : gitFrenchMessages);
  const activeAutoMockups = powerUserMaster === "enabled" ? true : (powerUserMaster === "disabled" ? false : autoMockups);
  const activeAutosave = powerUserMaster === "enabled" ? true : (powerUserMaster === "disabled" ? false : autosave);
  const activeCompactReasoning = powerUserMaster === "enabled" ? "very-compact" : (powerUserMaster === "disabled" ? "disabled" : compactReasoning);
  const activeAgentAutonomy = powerUserMaster === "enabled" ? true : (powerUserMaster === "disabled" ? false : agentAutonomy);

  return (
    <div className="settings-pane__body settings-pane__body--about" style={{ display: "flex", flexDirection: "column", gap: "24px" }}>
      
      {/* ========================================================================= */}
      {/* 🎨 CARTE GENERALE 1 : APPARENCE & INTERFACE                               */}
      {/* ========================================================================= */}
      <div className="options-category-group">
        <h3 className="options-category-title">
          <Icon icon="solar:palette-bold-duotone" className="options-category-icon" />
          {locale === "fr" ? "Apparence & Interface" : "Appearance & Interface"}
        </h3>
        <div className="options-category-grid">
          
          {/* Langue */}
          <div className="settings-pane__about-card">
            <div className="settings-pane__about-card-copy">
              <h2>{locale === "fr" ? "Langue" : "Language"}</h2>
              <p>
                {locale === "fr"
                  ? "Choisissez la langue de l'interface. Sinew se recharge après un changement afin que chaque panneau se mette à jour proprement."
                  : "Choose the interface language. Sinew reloads after a change so every panel updates cleanly."}
              </p>
            </div>
            <div className="settings-pane__locale-switch" role="radiogroup" aria-label="Interface language">
              <button
                type="button"
                role="radio"
                aria-checked={locale === "en"}
                data-active={locale === "en" ? "true" : "false"}
                onClick={() => onLocaleChange("en")}
              >
                English
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={locale === "fr"}
                data-active={locale === "fr" ? "true" : "false"}
                onClick={() => onLocaleChange("fr")}
              >
                Français
              </button>
            </div>
          </div>

          {/* Thème d'affichage */}
          <div className="settings-pane__about-card">
            <div className="settings-pane__about-card-copy">
              <h2>{locale === "fr" ? "Thème d'affichage" : "Theme"}</h2>
              <p>
                {locale === "fr"
                  ? "Basculez entre le mode clair (Jour), sombre (Nuit), système ou l'interface futuriste IA."
                  : "Switch between day, night, system theme, or the futuristic AI interface."}
              </p>
            </div>
            <div className="settings-pane__locale-switch" role="radiogroup" aria-label="Theme">
              <button
                type="button"
                role="radio"
                aria-checked={theme === "light"}
                data-active={theme === "light" ? "true" : "false"}
                onClick={() => changeTheme("light")}
              >
                {locale === "fr" ? "☀️ Jour" : "☀️ Day"}
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={theme === "dark"}
                data-active={theme === "dark" ? "true" : "false"}
                onClick={() => changeTheme("dark")}
              >
                {locale === "fr" ? "🌙 Nuit" : "🌙 Night"}
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={theme === "system"}
                data-active={theme === "system" ? "true" : "false"}
                onClick={() => changeTheme("system")}
              >
                {locale === "fr" ? "💻 Système" : "💻 System"}
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={theme === "ai"}
                data-active={theme === "ai" ? "true" : "false"}
                onClick={() => changeTheme("ai")}
                style={{
                  border: theme === "ai" ? "1px solid var(--accent)" : "none",
                  color: theme === "ai" ? "var(--accent-hi)" : "var(--text-0)"
                }}
              >
                {locale === "fr" ? "✨ IA (Moderne / Verre)" : "✨ AI (Modern / Glass)"}
              </button>
            </div>
          </div>

          {/* Tailles de police (Côte à côte) */}
          <div className="options-subcategory-row">
            {/* Éditeur */}
            <div className="settings-pane__about-card" style={{ flex: 1 }}>
              <div className="settings-pane__about-card-copy">
                <h2>{locale === "fr" ? "Taille du texte (Éditeur)" : "Editor Font Size"}</h2>
                <p>
                  {locale === "fr"
                    ? "Ajustez la taille des caractères dans l'éditeur de code."
                    : "Adjust the text size in the code editor."}
                </p>
              </div>
              <div className="settings-pane__locale-switch" style={{ display: "inline-flex", alignItems: "center", gap: "8px" }} role="group" aria-label="Editor Font Size">
                <button
                  type="button"
                  style={{ width: "28px", height: "28px", display: "inline-flex", alignItems: "center", justifyContent: "center", borderRadius: "6px", backgroundColor: "var(--bg-3)", color: "var(--text-0)", border: "none", cursor: "pointer", fontWeight: "bold" }}
                  onClick={() => changeEditorFontSize(Math.max(10, editorFontSize - 1))}
                >
                  -
                </button>
                <span style={{ minWidth: "32px", textAlign: "center", fontSize: "13px", fontWeight: "600", color: "var(--text-0)" }}>
                  {editorFontSize}px
                </span>
                <button
                  type="button"
                  style={{ width: "28px", height: "28px", display: "inline-flex", alignItems: "center", justifyContent: "center", borderRadius: "6px", backgroundColor: "var(--bg-3)", color: "var(--text-0)", border: "none", cursor: "pointer", fontWeight: "bold" }}
                  onClick={() => changeEditorFontSize(Math.min(22, editorFontSize + 1))}
                >
                  +
                </button>
              </div>
            </div>

            {/* Chat */}
            <div className="settings-pane__about-card" style={{ flex: 1 }}>
              <div className="settings-pane__about-card-copy">
                <h2>{locale === "fr" ? "Taille du texte (Chat)" : "Chat Font Size"}</h2>
                <p>
                  {locale === "fr"
                    ? "Ajustez la taille des caractères dans le panneau de chat."
                    : "Adjust the text size in the chat pane."}
                </p>
              </div>
              <div className="settings-pane__locale-switch" style={{ display: "inline-flex", alignItems: "center", gap: "8px" }} role="group" aria-label="Chat Font Size">
                <button
                  type="button"
                  style={{ width: "28px", height: "28px", display: "inline-flex", alignItems: "center", justifyContent: "center", borderRadius: "6px", backgroundColor: "var(--bg-3)", color: "var(--text-0)", border: "none", cursor: "pointer", fontWeight: "bold" }}
                  onClick={() => changeChatFontSize(Math.max(10, chatFontSize - 1))}
                >
                  -
                </button>
                <span style={{ minWidth: "32px", textAlign: "center", fontSize: "13px", fontWeight: "600", color: "var(--text-0)" }}>
                  {chatFontSize}px
                </span>
                <button
                  type="button"
                  style={{ width: "28px", height: "28px", display: "inline-flex", alignItems: "center", justifyContent: "center", borderRadius: "6px", backgroundColor: "var(--bg-3)", color: "var(--text-0)", border: "none", cursor: "pointer", fontWeight: "bold" }}
                  onClick={() => changeChatFontSize(Math.min(22, chatFontSize + 1))}
                >
                  +
                </button>
              </div>
            </div>
          </div>

          {/* Largeurs (Côte à côte) */}
          <div className="options-subcategory-row">
            {/* Chat Width */}
            <div className="settings-pane__about-card" style={{ flex: 1 }}>
              <div className="settings-pane__about-card-copy">
                <h2>{locale === "fr" ? "Largeur du chat" : "Chat Column Width"}</h2>
                <p>
                  {locale === "fr"
                    ? "Ajustez la largeur par défaut de la colonne de chat de droite."
                    : "Adjust the default width of the right chat column."}
                </p>
              </div>
              <div className="settings-pane__locale-switch" style={{ display: "inline-flex", alignItems: "center", gap: "8px" }} role="group" aria-label="Chat Column Width">
                <button
                  type="button"
                  style={{ width: "28px", height: "28px", display: "inline-flex", alignItems: "center", justifyContent: "center", borderRadius: "6px", backgroundColor: "var(--bg-3)", color: "var(--text-0)", border: "none", cursor: "pointer", fontWeight: "bold" }}
                  onClick={() => changeRightWidth(Math.max(220, rightWidth - 20))}
                >
                  -
                </button>
                <span style={{ minWidth: "48px", textAlign: "center", fontSize: "13px", fontWeight: "600", color: "var(--text-0)" }}>
                  {Math.round(rightWidth)}px
                </span>
                <button
                  type="button"
                  style={{ width: "28px", height: "28px", display: "inline-flex", alignItems: "center", justifyContent: "center", borderRadius: "6px", backgroundColor: "var(--bg-3)", color: "var(--text-0)", border: "none", cursor: "pointer", fontWeight: "bold" }}
                  onClick={() => changeRightWidth(Math.min(1200, rightWidth + 20))}
                >
                  +
                </button>
              </div>
            </div>

            {/* Sidebar Width */}
            <div className="settings-pane__about-card" style={{ flex: 1 }}>
              <div className="settings-pane__about-card-copy">
                <h2>{locale === "fr" ? "Largeur du menu latéral" : "Sidebar Column Width"}</h2>
                <p>
                  {locale === "fr"
                    ? "Ajustez la largeur par défaut de la colonne de gauche (fichiers)."
                    : "Adjust the default width of the left sidebar column."}
                </p>
              </div>
              <div className="settings-pane__locale-switch" style={{ display: "inline-flex", alignItems: "center", gap: "8px" }} role="group" aria-label="Sidebar Column Width">
                <button
                  type="button"
                  style={{ width: "28px", height: "28px", display: "inline-flex", alignItems: "center", justifyContent: "center", borderRadius: "6px", backgroundColor: "var(--bg-3)", color: "var(--text-0)", border: "none", cursor: "pointer", fontWeight: "bold" }}
                  onClick={() => changeLeftWidth(Math.max(220, leftWidth - 20))}
                >
                  -
                </button>
                <span style={{ minWidth: "48px", textAlign: "center", fontSize: "13px", fontWeight: "600", color: "var(--text-0)" }}>
                  {Math.round(leftWidth)}px
                </span>
                <button
                  type="button"
                  style={{ width: "28px", height: "28px", display: "inline-flex", alignItems: "center", justifyContent: "center", borderRadius: "6px", backgroundColor: "var(--bg-3)", color: "var(--text-0)", border: "none", cursor: "pointer", fontWeight: "bold" }}
                  onClick={() => changeLeftWidth(Math.min(800, leftWidth + 20))}
                >
                  +
                </button>
              </div>
            </div>
          </div>

          {/* Recherche automatique de mise à jour */}
          <div className="settings-pane__about-card">
            <div className="settings-pane__about-card-copy">
              <h2>{locale === "fr" ? "Recherche de mise à jour automatique" : "Automatic Update Check"}</h2>
              <p>
                {locale === "fr"
                  ? "Vérifie automatiquement les nouvelles versions au démarrage et périodiquement."
                  : "Automatically checks for new versions on startup and periodically."}
              </p>
            </div>
            <div className="settings-pane__locale-switch" role="radiogroup" aria-label="Automatic Update Check">
              <button
                type="button"
                role="radio"
                aria-checked={autoUpdateCheck === "blocking"}
                data-active={autoUpdateCheck === "blocking" ? "true" : "false"}
                onClick={() => changeAutoUpdateCheck("blocking")}
              >
                {locale === "fr" ? "Bloquant" : "Blocking"}
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={autoUpdateCheck === "notification"}
                data-active={autoUpdateCheck === "notification" ? "true" : "false"}
                onClick={() => changeAutoUpdateCheck("notification")}
              >
                {locale === "fr" ? "Notification uniquement" : "Notification only"}
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={autoUpdateCheck === "disabled"}
                data-active={autoUpdateCheck === "disabled" ? "true" : "false"}
                onClick={() => changeAutoUpdateCheck("disabled")}
              >
                {locale === "fr" ? "Désactivé" : "Disabled"}
              </button>
            </div>
          </div>

          {/* Taille de la boîte de chat */}
          <div className="settings-pane__about-card">
            <div className="settings-pane__about-card-copy">
              <h2>{locale === "fr" ? "Taille de la boîte de chat" : "Chat Box Size"}</h2>
              <p>
                {locale === "fr"
                  ? "Agrandit la zone de saisie de texte en bas du chat pour écrire de longs messages plus facilement."
                  : "Enlarges the text input box at the bottom of the chat for typing long messages more easily."}
              </p>
            </div>
            <div className="settings-pane__locale-switch" role="radiogroup" aria-label={locale === "fr" ? "Taille de la boîte de chat" : "Chat Box Size"}>
              <button
                type="button"
                role="radio"
                aria-checked={!largeChatBox}
                data-active={!largeChatBox ? "true" : "false"}
                onClick={() => toggleLargeChatBox(false)}
              >
                {locale === "fr" ? "Normal" : "Normal"}
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={largeChatBox}
                data-active={largeChatBox ? "true" : "false"}
                onClick={() => toggleLargeChatBox(true)}
              >
                {locale === "fr" ? "Agrandie" : "Enlarged"}
              </button>
            </div>
          </div>

        </div>
      </div>

      {/* ========================================================================= */}
      {/* ⚡ CARTE GENERALE 2 : MODE POWER USER & COMPORTEMENTS DE L'AGENT         */}
      {/* ========================================================================= */}
      <div className="options-category-group">
        <h3 className="options-category-title">
          <Icon icon="solar:bolt-bold-duotone" className="options-category-icon" style={{ color: "#eab308" }} />
          {locale === "fr" ? "Mode Power User & Comportements" : "Power User Mode & Agent Behaviors"}
        </h3>
        <div className="options-category-grid">
          
          {/* Master Toggle */}
          <div className="settings-pane__about-card" style={{ border: "1px solid rgba(234, 179, 8, 0.4)" }}>
            <div className="settings-pane__about-card-copy">
              <h2>{locale === "fr" ? "Mode Power User" : "Power User Mode"}</h2>
              <p>
                {locale === "fr"
                  ? "Active en un clic toutes les fonctionnalités avancées (automatisation Git, réponses ultra-concises, changelog obligatoire, sauvegarde automatique et réflexion très compacte)."
                  : "Activate all advanced options in one click (Git automation, ultra-concise answers, mandatory changelog, auto-save, and very compact display mode)."}
              </p>
            </div>
            <div className="settings-pane__locale-switch" role="radiogroup" aria-label="Power User Mode">
              <button
                type="button"
                role="radio"
                aria-checked={powerUserMaster === "enabled"}
                data-active={powerUserMaster === "enabled" ? "true" : "false"}
                onClick={() => changePowerUserMaster("enabled")}
              >
                {locale === "fr" ? "Activé" : "Enabled"}
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={powerUserMaster === "disabled"}
                data-active={powerUserMaster === "disabled" ? "true" : "false"}
                onClick={() => changePowerUserMaster("disabled")}
              >
                {locale === "fr" ? "Désactivé" : "Disabled"}
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={powerUserMaster === "custom"}
                data-active={powerUserMaster === "custom" ? "true" : "false"}
                onClick={() => changePowerUserMaster("custom")}
              >
                {locale === "fr" ? "Personnalisé" : "Custom"}
              </button>
            </div>
          </div>

          {/* Synchronisation Multi-PC (OneDrive) */}
          <div className="settings-pane__about-card" style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", width: "100%" }}>
              <div className="settings-pane__about-card-copy">
                <h2>{locale === "fr" ? "Synchronisation Multi-PC" : "Multi-PC Sync"}</h2>
                <p>
                  {locale === "fr"
                    ? "Synchronise automatiquement vos conversations et configurations entre vos ordinateurs via OneDrive."
                    : "Automatically synchronize your conversations and configurations between your computers via OneDrive."}
                </p>
              </div>
              <div className="settings-pane__locale-switch" role="radiogroup" aria-label="Multi-PC Sync" style={{ flexShrink: 0 }}>
                <button
                  type="button"
                  role="radio"
                  aria-checked={multiPcSync}
                  data-active={multiPcSync ? "true" : "false"}
                  onClick={() => toggleMultiPcSync(true)}
                >
                  {locale === "fr" ? "Activé" : "Enabled"}
                </button>
                <button
                  type="button"
                  role="radio"
                  aria-checked={!multiPcSync}
                  data-active={!multiPcSync ? "true" : "false"}
                  onClick={() => toggleMultiPcSync(false)}
                >
                  {locale === "fr" ? "Désactivé" : "Disabled"}
                </button>
              </div>
            </div>
          </div>

          {/* Recherche Sémantique Vectorielle (BETA) */}
          <div className="settings-pane__about-card">
            <div className="settings-pane__about-card-copy">
              <h2>
                <span style={{ color: "var(--accent-hi)", marginRight: "6px" }}>🧠</span>
                {locale === "fr" ? "Recherche Sémantique Vectorielle (BETA)" : "Vector Semantic Search (BETA)"}
              </h2>
              <p>
                {locale === "fr"
                  ? "Active l'indexation par concepts (fastembed). Permet de rechercher vos fichiers par leur sens général plutôt que par mot-clé exact (100% local)."
                  : "Enable concept-based indexing (fastembed). Allows searching files by their general meaning instead of exact keywords (100% local)."}
              </p>
            </div>
            <div className="settings-pane__locale-switch" role="radiogroup" aria-label="Semantic Search">
              <button
                type="button"
                role="radio"
                aria-checked={semanticEmbeddings}
                data-active={semanticEmbeddings ? "true" : "false"}
                onClick={() => toggleSemanticEmbeddings(true)}
              >
                {locale === "fr" ? "Activé" : "Enabled"}
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={!semanticEmbeddings}
                data-active={!semanticEmbeddings ? "true" : "false"}
                onClick={() => toggleSemanticEmbeddings(false)}
              >
                {locale === "fr" ? "Désactivé" : "Disabled"}
              </button>
            </div>
          </div>

          {/* Grille des 8 sous-cartes de comportements de l'agent */}
          <div style={{ display: "flex", flexDirection: "column", gap: "12px", marginTop: "4px" }}>
            
            {/* Ligne 1 : Git & Réponses ultra-concises */}
            <div className="options-subcategory-row">
              {/* Automatisation Git */}
              <div className="settings-pane__about-card" style={powerUserMaster !== "custom" ? { opacity: 0.55, pointerEvents: "none" } : undefined}>
                <div className="settings-pane__about-card-copy">
                  <h2>{locale === "fr" ? "Automatisation Git & Arrière-plan" : "Git & Background Automation"}</h2>
                  <p>
                    {locale === "fr"
                      ? "Vérifie, tire (pull) et pousse (push) automatiquement les modifications de code pour vous éviter de gérer Git."
                      : "Automatically checks, pulls, and pushes code changes to automate Git maintenance."}
                  </p>
                </div>
                <div className="settings-pane__locale-switch" role="radiogroup" aria-label="Git Automation">
                  <button
                    type="button"
                    role="radio"
                    aria-checked={activeGitAutomation}
                    data-active={activeGitAutomation ? "true" : "false"}
                    onClick={() => toggleGitAutomation(true)}
                    disabled={powerUserMaster !== "custom"}
                  >
                    {locale === "fr" ? "Activé" : "Enabled"}
                  </button>
                  <button
                    type="button"
                    role="radio"
                    aria-checked={!activeGitAutomation}
                    data-active={!activeGitAutomation ? "true" : "false"}
                    onClick={() => toggleGitAutomation(false)}
                    disabled={powerUserMaster !== "custom"}
                  >
                    {locale === "fr" ? "Désactivé" : "Disabled"}
                  </button>
                </div>
              </div>

              {/* Réponses ultra-concises */}
              <div className="settings-pane__about-card" style={powerUserMaster !== "custom" ? { opacity: 0.55, pointerEvents: "none" } : undefined}>
                <div className="settings-pane__about-card-copy">
                  <h2>{locale === "fr" ? "Réponses Ultra-Concises & Simplifiées" : "Ultra-Concise & Simplified Answers"}</h2>
                  <p>
                    {locale === "fr"
                      ? "Force l'agent à répondre en langage simple, éliminant le jargon technique pour des analogies claires."
                      : "Forces the agent to answer in simple language, replacing jargon with clear analogies."}
                  </p>
                </div>
                <div className="settings-pane__locale-switch" role="radiogroup" aria-label="Concise Answers">
                  <button
                    type="button"
                    role="radio"
                    aria-checked={activeConciseAnswers}
                    data-active={activeConciseAnswers ? "true" : "false"}
                    onClick={() => toggleConciseAnswers(true)}
                    disabled={powerUserMaster !== "custom"}
                  >
                    {locale === "fr" ? "Activé" : "Enabled"}
                  </button>
                  <button
                    type="button"
                    role="radio"
                    aria-checked={!activeConciseAnswers}
                    data-active={!activeConciseAnswers ? "true" : "false"}
                    onClick={() => toggleConciseAnswers(false)}
                    disabled={powerUserMaster !== "custom"}
                  >
                    {locale === "fr" ? "Désactivé" : "Disabled"}
                  </button>
                </div>
              </div>
            </div>

            {/* Ligne 2 : Commits en français & Maquettes visuelles */}
            <div className="options-subcategory-row">
              {/* Commits en français */}
              <div className="settings-pane__about-card" style={powerUserMaster !== "custom" ? { opacity: 0.55, pointerEvents: "none" } : undefined}>
                <div className="settings-pane__about-card-copy">
                  <h2>{locale === "fr" ? "Messages Git en Français Simple" : "Simple French Git Messages"}</h2>
                  <p>
                    {locale === "fr"
                      ? "Force l'agent à rédiger des messages de commit clairs, simples et en français pour vos idées."
                      : "Forces the agent to write clear, simple Git commit messages in French."}
                  </p>
                </div>
                <div className="settings-pane__locale-switch" role="radiogroup" aria-label="Simple French Git Messages">
                  <button
                    type="button"
                    role="radio"
                    aria-checked={activeGitFrenchMessages}
                    data-active={activeGitFrenchMessages ? "true" : "false"}
                    onClick={() => toggleGitFrenchMessages(true)}
                    disabled={powerUserMaster !== "custom"}
                  >
                    {locale === "fr" ? "Activé" : "Enabled"}
                  </button>
                  <button
                    type="button"
                    role="radio"
                    aria-checked={!activeGitFrenchMessages}
                    data-active={!activeGitFrenchMessages ? "true" : "false"}
                    onClick={() => toggleGitFrenchMessages(false)}
                    disabled={powerUserMaster !== "custom"}
                  >
                    {locale === "fr" ? "Désactivé" : "Disabled"}
                  </button>
                </div>
              </div>

              {/* Maquettes visuelles Mermaid */}
              <div className="settings-pane__about-card" style={powerUserMaster !== "custom" ? { opacity: 0.55, pointerEvents: "none" } : undefined}>
                <div className="settings-pane__about-card-copy">
                  <h2>{locale === "fr" ? "Maquettes Visuelles Automatiques" : "Automatic Visual Mockups"}</h2>
                  <p>
                    {locale === "fr"
                      ? "Génère un schéma Mermaid ou un plan visuel uniquement si vous le demandez ou si l'agent l'estime nécessaire."
                      : "Generates a Mermaid diagram or a visual layout only if you ask for it or if the agent deems it necessary."}
                  </p>
                </div>
                <div className="settings-pane__locale-switch" role="radiogroup" aria-label="Automatic Visual Mockups">
                  <button
                    type="button"
                    role="radio"
                    aria-checked={activeAutoMockups}
                    data-active={activeAutoMockups ? "true" : "false"}
                    onClick={() => toggleAutoMockups(true)}
                    disabled={powerUserMaster !== "custom"}
                  >
                    {locale === "fr" ? "Activé" : "Enabled"}
                  </button>
                  <button
                    type="button"
                    role="radio"
                    aria-checked={!activeAutoMockups}
                    data-active={!activeAutoMockups ? "true" : "false"}
                    onClick={() => toggleAutoMockups(false)}
                    disabled={powerUserMaster !== "custom"}
                  >
                    {locale === "fr" ? "Désactivé" : "Disabled"}
                  </button>
                </div>
              </div>
            </div>

            {/* Ligne 3 : Autonomie de l'agent & Changelog obligatoire */}
            <div className="options-subcategory-row">
              {/* Autonomie de l'agent */}
              <div className="settings-pane__about-card" style={powerUserMaster !== "custom" ? { opacity: 0.55, pointerEvents: "none" } : undefined}>
                <div className="settings-pane__about-card-copy">
                  <h2>{locale === "fr" ? "Autonomie de l'Agent" : "Agent Autonomy"}</h2>
                  <p>
                    {locale === "fr"
                      ? "Oblige l'agent à exécuter les tâches lui-même (coder, lire, tester) au lieu de vous lister des instructions textuelles."
                      : "Forces the agent to perform tasks itself (write code, read, test) instead of giving you instructions to do them."}
                  </p>
                </div>
                <div className="settings-pane__locale-switch" role="radiogroup" aria-label="Agent Autonomy">
                  <button
                    type="button"
                    role="radio"
                    aria-checked={activeAgentAutonomy}
                    data-active={activeAgentAutonomy ? "true" : "false"}
                    onClick={() => toggleAgentAutonomy(true)}
                    disabled={powerUserMaster !== "custom"}
                  >
                    {locale === "fr" ? "Activé" : "Enabled"}
                  </button>
                  <button
                    type="button"
                    role="radio"
                    aria-checked={!activeAgentAutonomy}
                    data-active={!activeAgentAutonomy ? "true" : "false"}
                    onClick={() => toggleAgentAutonomy(false)}
                    disabled={powerUserMaster !== "custom"}
                  >
                    {locale === "fr" ? "Désactivé" : "Disabled"}
                  </button>
                </div>
              </div>

              {/* Changelog obligatoire */}
              <div className="settings-pane__about-card" style={powerUserMaster !== "custom" ? { opacity: 0.55, pointerEvents: "none" } : undefined}>
                <div className="settings-pane__about-card-copy">
                  <h2>{locale === "fr" ? "Changelog obligatoire" : "Mandatory Changelog"}</h2>
                  <p>
                    {locale === "fr"
                      ? "Oblige l'agent à noter chaque modification dans un journal (CHANGELOG.md) daté."
                      : "Forces the agent to log every change in a dated changelog file (CHANGELOG.md)."}
                  </p>
                </div>
                <div className="settings-pane__locale-switch" role="radiogroup" aria-label="Mandatory Changelog">
                  <button
                    type="button"
                    role="radio"
                    aria-checked={activeForceChangelog}
                    data-active={activeForceChangelog ? "true" : "false"}
                    onClick={() => toggleForceChangelog(true)}
                    disabled={powerUserMaster !== "custom"}
                  >
                    {locale === "fr" ? "Activé" : "Enabled"}
                  </button>
                  <button
                    type="button"
                    role="radio"
                    aria-checked={!activeForceChangelog}
                    data-active={!activeForceChangelog ? "true" : "false"}
                    onClick={() => toggleForceChangelog(false)}
                    disabled={powerUserMaster !== "custom"}
                  >
                    {locale === "fr" ? "Désactivé" : "Disabled"}
                  </button>
                </div>
              </div>
            </div>

            {/* Ligne 4 : Sauvegarde automatique & Mode d'affichage de réflexion */}
            <div className="options-subcategory-row">
              {/* Sauvegarde automatique */}
              <div className="settings-pane__about-card" style={powerUserMaster !== "custom" ? { opacity: 0.55, pointerEvents: "none" } : undefined}>
                <div className="settings-pane__about-card-copy">
                  <h2>{locale === "fr" ? "Sauvegarde automatique" : "Auto-Save"}</h2>
                  <p>
                    {locale === "fr"
                      ? "Sauvegarde automatiquement vos fichiers modifiés après un court instant d'inactivité."
                      : "Automatically save your modified files after a brief period of inactivity."}
                  </p>
                </div>
                <div className="settings-pane__locale-switch" role="radiogroup" aria-label="Auto-Save">
                  <button
                    type="button"
                    role="radio"
                    aria-checked={activeAutosave}
                    data-active={activeAutosave ? "true" : "false"}
                    onClick={() => toggleAutosave(true)}
                    disabled={powerUserMaster !== "custom"}
                  >
                    {locale === "fr" ? "Activé (1.5s)" : "Enabled (1.5s)"}
                  </button>
                  <button
                    type="button"
                    role="radio"
                    aria-checked={!activeAutosave}
                    data-active={!activeAutosave ? "true" : "false"}
                    onClick={() => toggleAutosave(false)}
                    disabled={powerUserMaster !== "custom"}
                  >
                    {locale === "fr" ? "Désactivé" : "Disabled"}
                  </button>
                </div>
              </div>

              {/* Mode d'affichage / Réflexion */}
              <div className="settings-pane__about-card" style={powerUserMaster !== "custom" ? { opacity: 0.55, pointerEvents: "none" } : undefined}>
                <div className="settings-pane__about-card-copy">
                  <h2>{locale === "fr" ? "Mode d'affichage" : "Display Mode"}</h2>
                  <p>
                    {locale === "fr"
                      ? "Choisissez le niveau de détails techniques et de réflexion affichés dans le chat."
                      : "Choose the level of technical details and reasoning displayed in the chat."}
                  </p>
                </div>
                <div className="settings-pane__locale-switch" role="radiogroup" aria-label="Display Mode">
                  <button
                    type="button"
                    role="radio"
                    aria-checked={activeCompactReasoning === "very-compact"}
                    data-active={activeCompactReasoning === "very-compact" ? "true" : "false"}
                    onClick={() => changeCompactReasoning("very-compact")}
                    disabled={powerUserMaster !== "custom"}
                  >
                    {locale === "fr" ? "Très compact" : "Very compact"}
                  </button>
                  <button
                    type="button"
                    role="radio"
                    aria-checked={activeCompactReasoning === "compact"}
                    data-active={activeCompactReasoning === "compact" ? "true" : "false"}
                    onClick={() => changeCompactReasoning("compact")}
                    disabled={powerUserMaster !== "custom"}
                  >
                    {locale === "fr" ? "Compact" : "Compact"}
                  </button>
                  <button
                    type="button"
                    role="radio"
                    aria-checked={activeCompactReasoning === "disabled"}
                    data-active={activeCompactReasoning === "disabled" ? "true" : "false"}
                    onClick={() => changeCompactReasoning("disabled")}
                    disabled={powerUserMaster !== "custom"}
                  >
                    {locale === "fr" ? "Détaillé" : "Detailed"}
                  </button>
                </div>
              </div>
            </div>

          </div>

        </div>
      </div>

      {/* ========================================================================= */}
      {/* 🔄 CARTE GENERALE 3 : SYSTEME & DIAGNOSTICS                               */}
      {/* ========================================================================= */}
      <div className="options-category-group">
        <h3 className="options-category-title">
          <Icon icon="solar:settings-bold-duotone" className="options-category-icon" style={{ color: "#3b82f6" }} />
          {locale === "fr" ? "Diagnostics & Outils Système" : "Diagnostics & System Tools"}
        </h3>
        <div className="options-category-grid">
          
          {/* Diagnostic Système SOTA */}
          <div className="settings-pane__about-card" style={{ flexDirection: "column", gap: "16px", alignItems: "stretch" }}>
            <div className="settings-pane__about-card-header-flex">
              <div className="settings-pane__about-card-copy">
                <h2>{locale === "fr" ? "Diagnostic Système SOTA" : "SOTA System Diagnostics"}</h2>
                <p>
                  {locale === "fr"
                    ? "Vérifiez en temps réel le statut et la version de vos outils système indispensables."
                    : "Check the real-time status and version of your essential system tools."}
                </p>
              </div>
              <button
                type="button"
                className="settings-pane__button"
                onClick={() => runSotaDiagnostics(true)}
                disabled={loadingSota}
                style={{
                  padding: "6px 12px",
                  borderRadius: "6px",
                  backgroundColor: "var(--bg-3, rgba(255, 255, 255, 0.08))",
                  color: "var(--text-0, #fff)",
                  border: "1px solid var(--line-1, rgba(255, 255, 255, 0.12))",
                  cursor: "pointer",
                  fontSize: "12px",
                  fontWeight: 500,
                  display: "flex",
                  alignItems: "center",
                  gap: "6px"
                }}
              >
                {loadingSota ? (
                  <Icon icon="eos-icons:loading" width={14} height={14} />
                ) : (
                  <Icon icon="solar:refresh-linear" width={14} height={14} />
                )}
                {locale === "fr" ? "Actualiser" : "Refresh"}
              </button>
            </div>

            {sotaError && (
              <div style={{ color: "#ef4444", fontSize: "13px", padding: "8px", borderRadius: "6px", backgroundColor: "rgba(239, 68, 68, 0.1)" }}>
                Error: {sotaError}
              </div>
            )}

            {sotaData && (
              <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
                <div style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "8px",
                  padding: "10px 12px",
                  borderRadius: "6px",
                  backgroundColor: sotaData.status === "ok" ? "var(--bg-2, rgba(255, 255, 255, 0.04))" : "rgba(234, 179, 8, 0.1)",
                  border: `1px solid ${sotaData.status === "ok" ? "var(--line-1, rgba(255, 255, 255, 0.08))" : "rgba(234, 179, 8, 0.2)"}`,
                  fontSize: "13px",
                  color: sotaData.status === "ok" ? "var(--text-1, rgba(255, 255, 255, 0.85))" : "#eab308"
                }}>
                  <Icon
                    icon={sotaData.status === "ok" ? "solar:check-circle-bold" : "solar:danger-bold"}
                    width={18}
                    height={18}
                  />
                  <span>
                    {locale === "fr"
                      ? (sotaData.status === "ok"
                          ? "Tous les outils système SOTA sont installés et configurés."
                          : "Certains outils système SOTA ou dépendances sont manquants.")
                      : sotaData.message}
                  </span>
                </div>

                <div style={{
                  display: "grid",
                  gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))",
                  gap: "10px"
                }}>
                  {Object.entries(sotaData.tools || {}).map((entry) => {
                    const name = entry[0];
                    const info = entry[1] as any;
                    const isAvailable = info.available;
                    let displayName = name;
                    if (name === "sinew-extension") {
                      displayName = locale === "fr" ? "Extension Sinew Browser" : "Sinew Browser Extension";
                    } else if (name === "ripgrep") {
                      displayName = "Ripgrep";
                    } else if (name === "rustc") {
                      displayName = "Rustc";
                    } else {
                      displayName = name.charAt(0).toUpperCase() + name.slice(1);
                    }
                    return (
                      <div
                        key={name}
                        style={{
                          padding: "12px",
                          borderRadius: "8px",
                          backgroundColor: "var(--bg-card, rgba(255, 255, 255, 0.03))",
                          border: "1px solid var(--border, rgba(255, 255, 255, 0.08))",
                          display: "flex",
                          flexDirection: "column",
                          gap: "6px"
                        }}
                      >
                        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                          <span style={{ fontWeight: 600, fontSize: "14px" }}>
                            {displayName}
                          </span>
                          <span style={{
                            fontSize: "11px",
                            padding: "2px 6px",
                            borderRadius: "4px",
                            fontWeight: 600,
                            backgroundColor: isAvailable ? "var(--bg-3, rgba(255, 255, 255, 0.08))" : "rgba(239, 68, 68, 0.15)",
                            color: isAvailable ? "var(--text-2, rgba(255, 255, 255, 0.65))" : "#ef4444"
                          }}>
                            {isAvailable ? (locale === "fr" ? "Disponible" : "Available") : (locale === "fr" ? "Manquant" : "Missing")}
                          </span>
                        </div>

                        {isAvailable ? (
                          <div style={{ fontSize: "11px", opacity: 0.8, display: "flex", flexDirection: "column", gap: "2px" }}>
                            <div style={{ textOverflow: "ellipsis", overflow: "hidden", whiteSpace: "nowrap" }} title={info.version || ""}>
                              <strong>Version:</strong> {info.version || "Unknown"}
                            </div>
                            <div style={{ textOverflow: "ellipsis", overflow: "hidden", whiteSpace: "nowrap" }} title={info.path || ""}>
                              <strong>Path:</strong> {info.path || "N/A"}
                            </div>
                          </div>
                        ) : (
                          <div style={{ fontSize: "11px", color: "#ef4444", fontStyle: "italic" }}>
                            {info.error || (locale === "fr" ? "Exécutable introuvable dans le PATH" : "Executable not found in PATH")}
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>
            )}
          </div>

          {/* Apprentissage Automatique IA */}
          <div className="settings-pane__about-card" style={{ flexDirection: "column", gap: "16px", alignItems: "stretch" }}>
            <div className="settings-pane__about-card-header-flex">
              <div className="settings-pane__about-card-copy">
                <h2>
                  <span style={{ color: "var(--accent-hi)", marginRight: "6px" }}>🧠</span>
                  {locale === "fr" ? "Apprentissage Automatique IA" : "AI Auto-Learning"}
                </h2>
                <p>
                  {locale === "fr"
                    ? "Analyse les erreurs répétitives avec une IA pour les fusionner intelligemment en règles globales (remplace le script de consolidation simple)."
                    : "Analyzes repetitive errors with AI to intelligently merge them into global rules (replaces the simple consolidation script)."}
                </p>
              </div>
              <div className="settings-pane__locale-switch" role="radiogroup" aria-label="AI Auto-Learning">
                <button
                  type="button"
                  role="radio"
                  aria-checked={autoLearningEnabled}
                  data-active={autoLearningEnabled ? "true" : "false"}
                  onClick={() => toggleAutoLearning(true)}
                >
                  {locale === "fr" ? "Activé" : "Enabled"}
                </button>
                <button
                  type="button"
                  role="radio"
                  aria-checked={!autoLearningEnabled}
                  data-active={!autoLearningEnabled ? "true" : "false"}
                  onClick={() => toggleAutoLearning(false)}
                >
                  {locale === "fr" ? "Désactivé" : "Disabled"}
                </button>
              </div>
            </div>

            {configuredProviders.length > 0 && (
              <div style={{ display: "flex", alignItems: "center", gap: "8px", marginTop: "4px" }}>
                <span style={{ fontSize: "12px", color: "var(--text-2, rgba(255, 255, 255, 0.65))" }}>
                  {locale === "fr" ? "Fournisseur:" : "Provider:"}
                </span>
                <select
                  value={autoLearningProviderId}
                  onChange={(e) => {
                    const val = e.target.value;
                    setAutoLearningProviderId(val);
                    try { localStorage.setItem("sinew.auto-learning-provider", val); } catch {}
                  }}
                  style={{
                    padding: "4px 8px",
                    borderRadius: "6px",
                    backgroundColor: "var(--bg-3, rgba(255, 255, 255, 0.08))",
                    color: "var(--text-0, #fff)",
                    border: "1px solid var(--line-1, rgba(255, 255, 255, 0.12))",
                    fontSize: "12px",
                    cursor: "pointer"
                  }}
                >
                  {configuredProviders.map((p) => (
                    <option key={p} value={p}>{p}</option>
                  ))}
                </select>
              </div>
            )}

            <div style={{ display: "flex", alignItems: "center", gap: "10px", marginTop: "4px" }}>
              <button
                type="button"
                className="settings-pane__button"
                onClick={runAiConsolidation}
                disabled={autoLearningLoading || !autoLearningEnabled}
                style={{
                  padding: "6px 12px",
                  borderRadius: "6px",
                  backgroundColor: autoLearningEnabled ? "var(--accent-lo, rgba(59, 130, 246, 0.15))" : "var(--bg-3, rgba(255, 255, 255, 0.08))",
                  color: autoLearningEnabled ? "var(--accent-hi, #3b82f6)" : "var(--text-2, rgba(255, 255, 255, 0.45))",
                  border: `1px solid ${autoLearningEnabled ? "var(--accent-lo, rgba(59, 130, 246, 0.3))" : "var(--line-1, rgba(255, 255, 255, 0.08))"}`,
                  cursor: autoLearningEnabled ? "pointer" : "not-allowed",
                  fontSize: "12px",
                  fontWeight: 500,
                  display: "flex",
                  alignItems: "center",
                  gap: "6px",
                  opacity: autoLearningEnabled ? 1 : 0.5
                }}
              >
                {autoLearningLoading ? (
                  <Icon icon="eos-icons:loading" width={14} height={14} />
                ) : (
                  <Icon icon="solar:refresh-linear" width={14} height={14} />
                )}
                {locale === "fr" ? "Analyser maintenant" : "Analyze Now"}
              </button>
              <span style={{ fontSize: "11px", color: "var(--text-3, rgba(255, 255, 255, 0.45))" }}>
                {locale === "fr"
                  ? "L'IA lira errors_raw.json + instructions_consolidated.md, dédoublonnera les règles similaires et produira un fichier optimisé."
                  : "The AI reads errors_raw.json + instructions_consolidated.md, deduplicates similar rules, and produces an optimized file."}
              </span>
            </div>

            {autoLearningStatus && (
              <div style={{
                fontSize: "12px",
                padding: "8px 10px",
                borderRadius: "6px",
                backgroundColor: autoLearningStatus.startsWith("Erreur")
                  ? "rgba(239, 68, 68, 0.1)"
                  : "rgba(34, 197, 94, 0.1)",
                color: autoLearningStatus.startsWith("Erreur") ? "#ef4444" : "#22c55e",
                border: `1px solid ${autoLearningStatus.startsWith("Erreur") ? "rgba(239, 68, 68, 0.2)" : "rgba(34, 197, 94, 0.2)"}`
              }}>
                {autoLearningStatus}
              </div>
            )}
          </div>

        </div>
      </div>

    </div>
  );
}

// ---- About section -----------------------------------------------------

function AboutSection({ locale }: { locale: AppLocale }) {
  return (
    <div className="settings-pane__body settings-pane__body--about">
      <div className="settings-pane__about-hero">
        <span className="settings-pane__about-mark" aria-hidden>
          <SinewMark size={26} />
        </span>
        <div className="settings-pane__about-title">
          <h1>Sinew</h1>
        </div>
      </div>

      <p className="settings-pane__about-line">
        {locale === "fr"
          ? "Sinew est un harnais de codage IA flexible. Vous le façonnez : ajustez la description de chaque outil, désactivez ceux dont vous n'avez pas besoin, et l'assistant ne verra que ce que vous conservez."
          : "Sinew is a flexible AI coding harness. You shape it: tweak the description of every tool, turn the ones you don't need off, and the assistant only sees what you keep."}
      </p>
      <p className="settings-pane__about-line">
        {locale === "fr"
          ? "Lancez-le en mode minimal avec quelques outils, ou débloquez l'ensemble complet : terminal, recherche, MCP, web, images, sous-agents. Multi-fournisseur par défaut."
          : "Run it minimal with a couple of tools, or unlock the full set : shell, search, MCP, web, images, sub-agents. Multi-provider by default."}
      </p>

      <div className="settings-pane__about-links">
        <a
          className="settings-pane__about-link"
          href="https://discord.gg/MADQNHtZW"
          target="_blank"
          rel="noreferrer"
        >
          <Icon icon="simple-icons:discord" width={13} height={13} />
          <span>Discord</span>
        </a>
        <a
          className="settings-pane__about-link"
          href="https://github.com/Paseru/sinew"
          target="_blank"
          rel="noreferrer"
        >
          <Icon icon="simple-icons:github" width={13} height={13} />
          <span>GitHub</span>
        </a>
      </div>

      <div className="settings-pane__fork-section" style={{
        marginTop: "16px",
        paddingTop: "16px",
        borderTop: "1px solid var(--line-1)",
        display: "grid",
        gap: "12px",
      }}>
        <h2 style={{
          margin: 0,
          fontSize: "14px",
          fontWeight: 600,
          color: "var(--text-0)",
          display: "flex",
          alignItems: "center",
          gap: "6px"
        }}>
          <Icon icon="solar:widget-bold" width={16} height={16} style={{ color: "var(--primary)" }} />
          {locale === "fr" ? "Fork JulienPiron.fr — Améliorations Clés" : "JulienPiron.fr Fork — Key Enhancements"}
        </h2>
        
        <p className="settings-pane__about-line" style={{ fontSize: "var(--fs-xs)", color: "var(--text-3)" }}>
          {locale === "fr" 
            ? "Ce fork de JulienPiron.fr enrichit Sinew avec des fonctionnalités avancées optimisées pour un flux de travail quotidien rapide, autonome et ultra-résilient."
            : "This fork by JulienPiron.fr enriches Sinew with advanced features optimized for a fast, autonomous, and ultra-resilient daily workflow."}
        </p>

        <div style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))",
          gap: "10px",
          marginTop: "4px"
        }}>
          {/* Item 1 */}
          <div style={{
            background: "var(--bg-1)",
            border: "1px solid var(--line-1)",
            borderRadius: "var(--r-med)",
            padding: "10px 12px",
            display: "grid",
            gap: "4px"
          }}>
            <div style={{ display: "flex", alignItems: "center", gap: "6px", fontWeight: 600, fontSize: "var(--fs-xs)", color: "var(--text-0)" }}>
              <Icon icon="solar:play-circle-bold-duotone" width={14} height={14} style={{ color: "#3b82f6" }} />
              {locale === "fr" ? "Démarrage & Sandbox" : "Startup & Sandbox"}
            </div>
            <div style={{ fontSize: "11px", color: "var(--text-2)", lineHeight: 1.4 }}>
              {locale === "fr" 
                ? "Démarrage instantané en un clic (Mode Sandbox) sans ouvrir de projet pour tester l'IA ou utiliser les outils MCP de manière isolée."
                : "Instant one-click startup (Sandbox Mode) without opening a project to test AI or use MCP tools in isolation."}
            </div>
          </div>

          {/* Item 2 */}
          <div style={{
            background: "var(--bg-1)",
            border: "1px solid var(--line-1)",
            borderRadius: "var(--r-med)",
            padding: "10px 12px",
            display: "grid",
            gap: "4px"
          }}>
            <div style={{ display: "flex", alignItems: "center", gap: "6px", fontWeight: 600, fontSize: "var(--fs-xs)", color: "var(--text-0)" }}>
              <Icon icon="solar:chat-round-line-bold-duotone" width={14} height={14} style={{ color: "#10b981" }} />
              {locale === "fr" ? "Expérience Chat & Ergonomie" : "Chat Experience & Ergonomics"}
            </div>
            <div style={{ fontSize: "11px", color: "var(--text-2)", lineHeight: 1.4 }}>
              {locale === "fr" 
                ? "Clic droit interactif sur les onglets (fermeture) et fichiers du chat (ouvrir, révéler, exécuter). Question collante et copie libre du chat débloquée."
                : "Interactive right-click on tabs and chat files. Sticky question and unlocked chat text copying."}
            </div>
          </div>

          {/* Item 3 */}
          <div style={{
            background: "var(--bg-1)",
            border: "1px solid var(--line-1)",
            borderRadius: "var(--r-med)",
            padding: "10px 12px",
            display: "grid",
            gap: "4px"
          }}>
            <div style={{ display: "flex", alignItems: "center", gap: "6px", fontWeight: 600, fontSize: "var(--fs-xs)", color: "var(--text-0)" }}>
              <Icon icon="solar:settings-bold-duotone" width={14} height={14} style={{ color: "#f59e0b" }} />
              {locale === "fr" ? "Confort Monaco & Polices" : "Monaco Comfort & Fonts"}
            </div>
            <div style={{ fontSize: "11px", color: "var(--text-2)", lineHeight: 1.4 }}>
              {locale === "fr" 
                ? "Ajustement dynamique de la taille du texte (+/-) pour Monaco et le chat. Découpage du bundle Vite (-80% taille) pour chargement instantané."
                : "Dynamic font size buttons (+/-) for Monaco Editor and chat. Vite bundle splitting (-80% size) for instant UI load."}
            </div>
          </div>

          {/* Item 4 */}
          <div style={{
            background: "var(--bg-1)",
            border: "1px solid var(--line-1)",
            borderRadius: "var(--r-med)",
            padding: "10px 12px",
            display: "grid",
            gap: "4px"
          }}>
            <div style={{ display: "flex", alignItems: "center", gap: "6px", fontWeight: 600, fontSize: "var(--fs-xs)", color: "var(--text-0)" }}>
              <Icon icon="solar:disk-bold-duotone" width={14} height={14} style={{ color: "#eab308" }} />
              {locale === "fr" ? "Sauvegarde Auto & Mises à Jour" : "Auto-Save & Updates"}
            </div>
            <div style={{ fontSize: "11px", color: "var(--text-2)", lineHeight: 1.4 }}>
              {locale === "fr" 
                ? "Sauvegarde automatique SOTA 1.5s après la frappe. Écran de mises à jour sécurisé et synchronisation OneDrive & SQLite automatique."
                : "SOTA auto-save 1.5s after typing. Safe updates screen and automatic OneDrive & SQLite synchronization."}
            </div>
          </div>

          {/* Item 5 */}
          <div style={{
            background: "var(--bg-1)",
            border: "1px solid var(--line-1)",
            borderRadius: "var(--r-med)",
            padding: "10px 12px",
            display: "grid",
            gap: "4px"
          }}>
            <div style={{ display: "flex", alignItems: "center", gap: "6px", fontWeight: 600, fontSize: "var(--fs-xs)", color: "var(--text-0)" }}>
              <Icon icon="solar:shield-check-bold-duotone" width={14} height={14} style={{ color: "#14b8a6" }} />
              {locale === "fr" ? "Zéro Popup & Robustesse" : "Zero Popup & Resilience"}
            </div>
            <div style={{ fontSize: "11px", color: "var(--text-2)", lineHeight: 1.4 }}>
              {locale === "fr" 
                ? "Lancement invisible des outils sans popup cmd.exe noire. Diagnostic réseau OAuth Windows résilient (erreur 10013, WinNAT/HNS)."
                : "Invisible launch of sidecars with zero black cmd popups. Resilient Windows OAuth network check (error 10013, WinNAT/HNS)."}
            </div>
          </div>

          {/* Item 6 */}
          <div style={{
            background: "var(--bg-1)",
            border: "1px solid var(--line-1)",
            borderRadius: "var(--r-med)",
            padding: "10px 12px",
            display: "grid",
            gap: "4px"
          }}>
            <div style={{ display: "flex", alignItems: "center", gap: "6px", fontWeight: 600, fontSize: "var(--fs-xs)", color: "var(--text-0)" }}>
              <Icon icon="solar:history-bold-duotone" width={14} height={14} style={{ color: "#6366f1" }} />
              {locale === "fr" ? "Active Turn & Préfixe PC" : "Active Turn & PC Prefix"}
            </div>
            <div style={{ fontSize: "11px", color: "var(--text-2)", lineHeight: 1.4 }}>
              {locale === "fr" 
                ? "Active Turn Registry (Rust) pour reprise de streaming après coupure. Détection automatique du nom PC réel pour les fichiers multi-PC."
                : "Rust Active Turn Registry for streaming recovery on restart. Automatic PC name prefixing for secure multi-PC files."}
            </div>
          </div>

          {/* Item 7 */}
          <div style={{
            background: "var(--bg-1)",
            border: "1px solid var(--line-1)",
            borderRadius: "var(--r-med)",
            padding: "10px 12px",
            display: "grid",
            gap: "4px"
          }}>
            <div style={{ display: "flex", alignItems: "center", gap: "6px", fontWeight: 600, fontSize: "var(--fs-xs)", color: "var(--text-0)" }}>
              <Icon icon="solar:users-group-two-rounded-bold-duotone" width={14} height={14} style={{ color: "#a855f7" }} />
              {locale === "fr" ? "Multi-comptes & Quotas" : "Multi-accounts & Quotas"}
            </div>
            <div style={{ fontSize: "11px", color: "var(--text-2)", lineHeight: 1.4 }}>
              {locale === "fr" 
                ? "Connexion simultanée de plusieurs clés/comptes (OpenAI/Gemini). Pastilles live et barres de progression de consommation crédit dans le chat."
                : "Simultaneous multi-account keys (OpenAI/Gemini). Live progression bars and credit balance dots directly in the chat."}
            </div>
          </div>

          {/* Item 8 */}
          <div style={{
            background: "var(--bg-1)",
            border: "1px solid var(--line-1)",
            borderRadius: "var(--r-med)",
            padding: "10px 12px",
            display: "grid",
            gap: "4px"
          }}>
            <div style={{ display: "flex", alignItems: "center", gap: "6px", fontWeight: 600, fontSize: "var(--fs-xs)", color: "var(--text-0)" }}>
              <Icon icon="solar:cpu-bold-duotone" width={14} height={14} style={{ color: "#8b5cf6" }} />
              {locale === "fr" ? "Google Antigravity & DeepSeek" : "Google Antigravity & DeepSeek"}
            </div>
            <div style={{ fontSize: "11px", color: "var(--text-2)", lineHeight: 1.4 }}>
              {locale === "fr" 
                ? "Routage intelligent Gemini Ultra et streaming optimisé. Intégration de Claude Opus 4.6 via les abonnements professionnels Google."
                : "Gemini Ultra smart routing and optimized streaming. Claude Opus 4.6 integration via professional Google accounts."}
            </div>
          </div>

          {/* Item 9 */}
          <div style={{
            background: "var(--bg-1)",
            border: "1px solid var(--line-1)",
            borderRadius: "var(--r-med)",
            padding: "10px 12px",
            display: "grid",
            gap: "4px"
          }}>
            <div style={{ display: "flex", alignItems: "center", gap: "6px", fontWeight: 600, fontSize: "var(--fs-xs)", color: "var(--text-0)" }}>
              <Icon icon="solar:square-academic-cap-bold-duotone" width={14} height={14} style={{ color: "#d946ef" }} />
              {locale === "fr" ? "Cursor Composer & WebSocket" : "Cursor Composer & WebSocket"}
            </div>
            <div style={{ fontSize: "11px", color: "var(--text-2)", lineHeight: 1.4 }}>
              {locale === "fr" 
                ? "Pont Cursor Composer 2.5 (agent.v1) HTTP/2 autonome. WebSocket natif OpenAI et spoofing d'empreinte anti-blocage ChatGPT."
                : "Cursor Composer 2.5 (agent.v1) HTTP/2 bridge. Native OpenAI WebSockets and advanced anti-blocking fingerprint spoofing."}
            </div>
          </div>

          {/* Item 10 */}
          <div style={{
            background: "var(--bg-1)",
            border: "1px solid var(--line-1)",
            borderRadius: "var(--r-med)",
            padding: "10px 12px",
            display: "grid",
            gap: "4px"
          }}>
            <div style={{ display: "flex", alignItems: "center", gap: "6px", fontWeight: 600, fontSize: "var(--fs-xs)", color: "var(--text-0)" }}>
              <Icon icon="solar:compass-bold-duotone" width={14} height={14} style={{ color: "#ec4899" }} />
              {locale === "fr" ? "Steering & Influence SOTA" : "Steering & SOTA Influence"}
            </div>
            <div style={{ fontSize: "11px", color: "var(--text-2)", lineHeight: 1.4 }}>
              {locale === "fr" 
                ? "Bouton d'interception « Influencer » (Steering) pour orienter, corriger ou ajouter des instructions en direct pendant que l'IA génère."
                : "Smart 'Steering' button to guide, correct, or add instructions on the fly during generation."}
            </div>
          </div>

          {/* Item 11 */}
          <div style={{
            background: "var(--bg-1)",
            border: "1px solid var(--line-1)",
            borderRadius: "var(--r-med)",
            padding: "10px 12px",
            display: "grid",
            gap: "4px"
          }}>
            <div style={{ display: "flex", alignItems: "center", gap: "6px", fontWeight: 600, fontSize: "var(--fs-xs)", color: "var(--text-0)" }}>
              <Icon icon="solar:global-bold-duotone" width={14} height={14} style={{ color: "#ec4899" }} />
              {locale === "fr" ? "Pont Chrome & Réparation" : "Chrome Bridge & Repair"}
            </div>
            <div style={{ fontSize: "11px", color: "var(--text-2)", lineHeight: 1.4 }}>
              {locale === "fr" 
                ? "Pilotage Chrome Rust ultra-stable (clics et courbes Beziers physiques). Bouton bleu de réparation en un clic en cas d'erreur de pont local."
                : "Native Rust Chrome control with human Bézier curves. One-click blue repair button for the local bridge."}
            </div>
          </div>

          {/* Item 12 */}
          <div style={{
            background: "var(--bg-1)",
            border: "1px solid var(--line-1)",
            borderRadius: "var(--r-med)",
            padding: "10px 12px",
            display: "grid",
            gap: "4px"
          }}>
            <div style={{ display: "flex", alignItems: "center", gap: "6px", fontWeight: 600, fontSize: "var(--fs-xs)", color: "var(--text-0)" }}>
              <Icon icon="solar:magnifier-bold-duotone" width={14} height={14} style={{ color: "#06b6d4" }} />
              {locale === "fr" ? "Recherche Vectorielle & 8 Couches" : "Vector Search & 8-Layer"}
            </div>
            <div style={{ fontSize: "11px", color: "var(--text-2)", lineHeight: 1.4 }}>
              {locale === "fr" 
                ? "Indexation vectorielle sémantique locale BGE-Small. Moteur de remplacement Search/Replace intelligent à 8 couches résilient aux espaces."
                : "Local vector index search with interactive badge. 8-layer smart Search/Replace engine resilient to minor whitespace errors."}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
// ---- Providers section -------------------------------------------------

type ProvidersSectionProps = {
  openAiStatus: OpenAiProviderStatus | null;
  openAiAccounts: OpenAiAccountInfo[];
  unconnectedAccounts: string[];
  secondaryModels: Record<string, string>;
  onUpdateSecondaryModel: (key: string, model: string) => void;
  secondaryThinking: Record<string, string>;
  onUpdateSecondaryThinking: (key: string, thinking: string) => void;
  secondaryFast: Record<string, string>;
  onUpdateSecondaryFast: (key: string, fast: string) => void;
  onSaveOpenAiAccessToken: (token: string, key?: string) => Promise<void>;
  onDisconnectOpenAiAccount: (key: string) => void;
  anthropicStatus: AnthropicProviderStatus | null;
  googleStatus: GoogleProviderStatus | null;
  googleAccounts: GoogleAccountInfo[];
  unconnectedGoogleAccounts: string[];
  onConnectGoogleWithKey: (key: string) => void;
  onAddGoogleAccount: () => void;
  onRemoveUnconnectedGoogleAccount: (key: string) => void;
  onDisconnectGoogleAccount: (key: string) => void;
  cursorComposerStatus: CursorComposerAuthStatus | null;
  kimiStatus: KimiProviderStatus | null;
  deepSeekStatus: DeepSeekProviderStatus | null;
  openRouterStatus: OpenRouterProviderStatus | null;
  openRouterModels: OpenRouterModel[];
  loading: boolean;
  busy: boolean;
  message: string | null;
  onRefresh: () => void;
  onConnect: () => void;
  onConnectWithKey: (key: string) => void;
  onAddOpenAiAccount: () => void;
  onRemoveUnconnectedAccount: (key: string) => void;
  onCancel: () => void;
  onDisconnect: () => void;
  onConnectAnthropic: () => void;
  onCancelAnthropic: () => void;
  onDisconnectAnthropic: () => void;
  onConnectGoogle: () => void;
  onCancelGoogle: () => void;
  onDisconnectGoogle: () => void;
  onConnectCursor: () => void;
  onCancelCursorComposer: () => void;
  onDisconnectCursorComposer: () => void;
  onConnectKimi: () => void;
  onCancelKimi: () => void;
  onDisconnectKimi: () => void;
  onDisconnectOpenRouter: () => void;
  onOpenRouterStatusChange: (status: OpenRouterProviderStatus) => void;
  onOpenRouterModelsChange: (models: OpenRouterModel[]) => void;
  onOpenRouterChanged: () => void;
  onDisconnectDeepSeek: () => void;
  onDeepSeekStatusChange: (status: DeepSeekProviderStatus) => void;
  onDeepSeekChanged: () => void;
};

function ProvidersSection({
  openAiStatus,
  openAiAccounts,
  unconnectedAccounts,
  secondaryModels,
  onUpdateSecondaryModel,
  secondaryThinking,
  onUpdateSecondaryThinking,
  secondaryFast,
  onUpdateSecondaryFast,
  onSaveOpenAiAccessToken,
  onDisconnectOpenAiAccount,
  anthropicStatus,
  googleStatus,
  googleAccounts,
  unconnectedGoogleAccounts,
  onConnectGoogleWithKey,
  onAddGoogleAccount,
  onRemoveUnconnectedGoogleAccount,
  onDisconnectGoogleAccount,
  cursorComposerStatus,
  kimiStatus,
  deepSeekStatus,
  openRouterStatus,
  openRouterModels,
  loading,
  busy,
  message,
  onRefresh,
  onConnect,
  onConnectWithKey,
  onAddOpenAiAccount,
  onRemoveUnconnectedAccount,
  onCancel,
  onDisconnect,
  onConnectAnthropic,
  onCancelAnthropic,
  onDisconnectAnthropic,
  onConnectGoogle,
  onCancelGoogle,
  onDisconnectGoogle,
  onConnectCursor,
  onCancelCursorComposer,
  onDisconnectCursorComposer,
  onConnectKimi,
  onCancelKimi,
  onDisconnectKimi,
  onDisconnectOpenRouter,
  onOpenRouterStatusChange,
  onOpenRouterModelsChange,
  onOpenRouterChanged,
  onDisconnectDeepSeek,
  onDeepSeekStatusChange,
  onDeepSeekChanged,
}: ProvidersSectionProps) {
  const cursorStatus: CursorComposerAuthStatus = {
    connected: Boolean(cursorComposerStatus?.connected),
    connectionState:
      cursorComposerStatus?.connectionState ??
      (cursorComposerStatus?.connected ? "connected" : "disconnected"),
    email: cursorComposerStatus?.email ?? undefined,
    membershipType: cursorComposerStatus?.membershipType ?? undefined,
    error: cursorComposerStatus?.error ?? undefined,
  };

  return (
    <>
      <header className="settings-pane__header">
        <div className="settings-pane__header-text">
          <h1 className="settings-pane__title">Providers</h1>
          <p className="settings-pane__subtitle">
            Connect model providers for Sinew.
          </p>
        </div>
        <div className="settings-pane__actions">
          {message && (
            <span
              className="settings-pane__status"
              data-tone={message === "Disconnected" ? "ok" : "pending"}
            >
              {message}
            </span>
          )}
          <button
            type="button"
            className="settings-pane__btn"
            onClick={onRefresh}
            disabled={loading || busy}
          >
            <Icon icon="solar:refresh-linear" width={13} height={13} />
            <span>{loading ? "Refreshing…" : "Refresh"}</span>
          </button>
        </div>
      </header>

      <div className="settings-pane__body settings-pane__body--providers">
        <ProviderCard
          name="Anthropic"
          icon="simple-icons:anthropic"
          description="Use OAuth to connect your Claude account for Anthropic models."
          status={anthropicStatus}
          connectedMeta={["Claude OAuth"]}
          loading={loading}
          busy={busy}
          onConnect={onConnectAnthropic}
          onCancel={onCancelAnthropic}
          onDisconnect={onDisconnectAnthropic}
          providerId="anthropic"
        />
        <ProviderCard
          name="OpenAI"
          icon="simple-icons:openai"
          description="Use OAuth to connect your ChatGPT account for OpenAI models."
          status={openAiStatus}
          connectedMeta={[
            openAiStatus?.email || "Signed in",
            openAiStatus?.planType ?? null,
          ]}
          loading={loading}
          busy={busy}
          onConnect={onConnect}
          onCancel={onCancel}
          onDisconnect={onDisconnect}
          showPlus={true}
          onPlus={onAddOpenAiAccount}
          providerId="openai"
        >
          {!openAiStatus?.connected && (
            <div style={{ display: "grid", gap: "8px", marginTop: "12px", padding: "10px", background: "var(--bg-2)", borderRadius: "6px", border: "1px solid var(--line-1)" }}>
              <span style={{ fontSize: "11px", fontWeight: "bold", color: "var(--text-2)", textTransform: "uppercase", letterSpacing: "0.05em" }}>
                Or paste a business access token:
              </span>
              <div style={{ display: "flex", gap: "8px" }}>
                <input
                  type="password"
                  placeholder="eyJhbGciOi..."
                  id="token-input-openai-main"
                  style={{
                    flex: 1,
                    background: "var(--bg-3)",
                    color: "var(--text-1)",
                    border: "1px solid var(--line-2)",
                    borderRadius: "4px",
                    padding: "6px 10px",
                    fontSize: "12px",
                    outline: "none"
                  }}
                />
                <button
                  type="button"
                  onClick={async () => {
                    const input = document.getElementById("token-input-openai-main") as HTMLInputElement;
                    const val = input?.value || "";
                    if (val.trim()) {
                      await onSaveOpenAiAccessToken(val, "openai");
                      onRefresh();
                    }
                  }}
                  style={{
                    background: "var(--accent-1, #3b82f6)",
                    color: "#fff",
                    border: "none",
                    borderRadius: "4px",
                    padding: "6px 12px",
                    fontSize: "12px",
                    fontWeight: 500,
                    cursor: "pointer"
                  }}
                >
                  Save
                </button>
              </div>
            </div>
          )}
        </ProviderCard>
        {(openAiAccounts.some((account) => account.key.startsWith("openai:")) || unconnectedAccounts.length > 0) && (
          <div className="settings-pane__secondary-grid">
            {[...openAiAccounts]
              .filter((account) => account.key.startsWith("openai:"))
              .sort((a, b) => a.key.localeCompare(b.key, undefined, { numeric: true, sensitivity: "base" }))
              .map((account) => {
                const suffix = account.key.slice("openai:".length);
            const displayName = `OpenAI ${suffix}`;
            const accountStatus: ProviderCardStatus = {
              connected: true,
              connectionState: "connected",
              email: account.email,
              planType: account.planType,
            };
            const currentModel = secondaryModels[account.key] || "gpt-5.5";
            
            const selectStyle = {
              background: "var(--bg-3)",
              color: "var(--text-1)",
              border: "1px solid var(--line-2)",
              borderRadius: "4px",
              padding: "2px 6px",
              fontSize: "11px",
              cursor: "pointer",
              outline: "none",
              flex: "1 1 auto",
              minWidth: 0,
              textOverflow: "ellipsis",
              overflow: "hidden",
              whiteSpace: "nowrap" as const
            };
            
            return (
              <ProviderCard
                key={account.key}
                name={displayName}
                icon="simple-icons:openai"
                description={`Connected OpenAI account ${suffix}.`}
                status={accountStatus}
                connectedMeta={[
                  account.email || "Signed in",
                  account.planType ?? null,
                ]}
                loading={loading}
                busy={busy}
                onConnect={() => {}}
                onCancel={() => {}}
                onDisconnect={() => void onDisconnectOpenAiAccount(account.key)}
                showMinus={true}
                onMinus={() => void onDisconnectOpenAiAccount(account.key)}
                providerId={account.key}
                compact={true}
              >
                <div style={{ display: "flex", flexDirection: "column", gap: "6px", marginTop: "4px", minWidth: 0 }}>
                  <div style={{ display: "flex", gap: "6px", alignItems: "center", minWidth: 0 }}>
                    <span style={{ fontSize: "10px", fontWeight: 500, color: "var(--text-2)", textTransform: "uppercase", flex: "0 0 auto", width: "55px" }}>Model:</span>
                    <select
                      value={currentModel}
                      onChange={(e) => onUpdateSecondaryModel(account.key, e.target.value)}
                      style={selectStyle}
                    >
                      <option value="gpt-5.5">GPT-5.5</option>
                      <option value="gpt-5.4">GPT-5.4</option>
                      <option value="gpt-5.4-mini">GPT-5.4 Mini</option>
                      <option value="gpt-5.3-codex">GPT-5.3 Codex</option>
                      <option value="gpt-5.3-codex-spark">GPT-5.3 Codex Spark</option>
                      <option value="gpt-5.2">GPT-5.2</option>
                    </select>
                  </div>
                </div>
              </ProviderCard>
            );
          })}
        {[...unconnectedAccounts]
          .sort((a, b) => a.localeCompare(b, undefined, { numeric: true, sensitivity: "base" }))
          .map((key) => {
          const suffix = key.slice("openai:".length);
          const displayName = `OpenAI ${suffix}`;
          const accountStatus: ProviderCardStatus = {
            connected: false,
            connectionState: "disconnected",
          };
          return (
            <ProviderCard
              key={key}
              name={displayName}
              icon="simple-icons:openai"
              description={`Connect a secondary OpenAI ChatGPT account.`}
              status={accountStatus}
              connectedMeta={[]}
              loading={loading}
              busy={busy}
              onConnect={() => onConnectWithKey(key)}
              onCancel={() => {}}
              onDisconnect={() => {}}
              showMinus={true}
              onMinus={() => onRemoveUnconnectedAccount(key)}
              compact={true}
            >
              <div style={{ display: "grid", gap: "8px", marginTop: "8px" }}>
                <span style={{ fontSize: "11px", fontWeight: "bold", color: "var(--text-2)", textTransform: "uppercase", letterSpacing: "0.05em" }}>
                  Or paste a business access token:
                </span>
                <div style={{ display: "flex", gap: "8px" }}>
                  <input
                    type="password"
                    placeholder="eyJhbGciOi..."
                    id={`token-input-${key}`}
                    style={{
                      flex: 1,
                      background: "var(--bg-3)",
                      color: "var(--text-1)",
                      border: "1px solid var(--line-2)",
                      borderRadius: "4px",
                      padding: "6px 10px",
                      fontSize: "12px",
                      outline: "none"
                    }}
                  />
                  <button
                    type="button"
                    onClick={async () => {
                      const input = document.getElementById(`token-input-${key}`) as HTMLInputElement;
                      const val = input?.value || "";
                      if (val.trim()) {
                        await onSaveOpenAiAccessToken(val, key);
                        onRefresh();
                      }
                    }}
                    style={{
                      background: "var(--accent-1, #3b82f6)",
                      color: "#fff",
                      border: "none",
                      borderRadius: "4px",
                      padding: "6px 12px",
                      fontSize: "12px",
                      fontWeight: 500,
                      cursor: "pointer"
                    }}
                  >
                    Save
                  </button>
                </div>
              </div>
            </ProviderCard>
            );
          })}
        </div>
        )}
        <ProviderCard
          name="Cursor"
          icon="local:cursor"
          description="Connectez votre compte Cursor (Google/GitHub) pour utiliser Auto + Composer via votre abonnement."
          status={cursorStatus}
          connectedMeta={[
            cursorComposerStatus?.email || "Session Composer",
            cursorComposerStatus?.membershipType && cursorComposerStatus.membershipType !== "pro_plus"
              ? cursorComposerStatus.membershipType
              : null,
          ]}
          loading={loading}
          busy={busy}
          onConnect={onConnectCursor}
          onCancel={onCancelCursorComposer}
          onDisconnect={onDisconnectCursorComposer}
          providerId="cursor"
          connectLabel="Connecter"
          busyConnectLabel="Ouverture..."
        />
        <ProviderCard
          name="Google"
          icon="simple-icons:google"
          description="Use OAuth to connect your Google account for Gemini models."
          status={googleStatus}
          connectedMeta={[
            googleStatus?.email || "Signed in",
            googleStatus?.userTier ?? null,
            googleStatus?.projectId
              ? `Project ${googleStatus.projectId}`
              : null,
          ]}
          loading={loading}
          busy={busy}
          onConnect={onConnectGoogle}
          onCancel={onCancelGoogle}
          onDisconnect={onDisconnectGoogle}
          showPlus={true}
          onPlus={onAddGoogleAccount}
          providerId="google"
        />
        {(googleAccounts.some((account) => account.key.startsWith("google:")) || unconnectedGoogleAccounts.length > 0) && (
          <div className="settings-pane__secondary-grid">
            {[...googleAccounts]
              .filter((account) => account.key.startsWith("google:"))
              .sort((a, b) => a.key.localeCompare(b.key, undefined, { numeric: true, sensitivity: "base" }))
              .map((account) => {
                const suffix = account.key.slice("google:".length);
                const displayName = `Google ${suffix}`;
                const accountStatus: ProviderCardStatus = {
                  connected: true,
                  connectionState: "connected",
                  email: account.email,
                  userTier: account.userTier,
                  projectId: account.projectId,
                };
                const currentModel = secondaryModels[account.key] || "gemini-3.5-flash";

                const selectStyle = {
                  background: "var(--bg-3)",
                  color: "var(--text-1)",
                  border: "1px solid var(--line-2)",
                  borderRadius: "4px",
                  padding: "2px 6px",
                  fontSize: "11px",
                  cursor: "pointer",
                  outline: "none",
                  flex: "1 1 auto",
                  minWidth: 0,
                  textOverflow: "ellipsis",
                  overflow: "hidden",
                  whiteSpace: "nowrap" as const
                };

                return (
                  <ProviderCard
                    key={account.key}
                    name={displayName}
                    icon="simple-icons:google"
                    description={`Connected Google account ${suffix}.`}
                    status={accountStatus}
                    connectedMeta={[
                      account.email || "Signed in",
                      account.userTier ?? null,
                      account.projectId ? `Project ${account.projectId}` : null,
                    ]}
                    loading={loading}
                    busy={busy}
                    onConnect={() => {}}
                    onCancel={() => {}}
                    onDisconnect={() => void onDisconnectGoogleAccount(account.key)}
                    showMinus={true}
                    onMinus={() => void onDisconnectGoogleAccount(account.key)}
                    providerId={account.key}
                    compact={true}
                  >
                    <div style={{ display: "flex", flexDirection: "column", gap: "6px", marginTop: "4px", minWidth: 0 }}>
                      <div style={{ display: "flex", gap: "6px", alignItems: "center", minWidth: 0 }}>
                        <span style={{ fontSize: "10px", fontWeight: 500, color: "var(--text-2)", textTransform: "uppercase", flex: "0 0 auto", width: "55px" }}>Model:</span>
                        <select
                          value={currentModel}
                          onChange={(e) => onUpdateSecondaryModel(account.key, e.target.value)}
                          style={selectStyle}
                        >
                          <option value="gemini-3.5-flash">Gemini 3.5 Flash</option>
                          <option value="gemini-3.1-pro">Gemini 3.1 Pro</option>
                          <option value="claude-sonnet-4.6">Claude Sonnet 4.6</option>
                          <option value="claude-opus-4.6">Claude Opus 4.6</option>
                          <option value="gpt-oss-120b">GPT-OSS 120B</option>
                        </select>
                      </div>
                    </div>
                  </ProviderCard>
                );
              })}
            {[...unconnectedGoogleAccounts]
              .sort((a, b) => a.localeCompare(b, undefined, { numeric: true, sensitivity: "base" }))
              .map((key) => {
                const suffix = key.slice("google:".length);
                const displayName = `Google ${suffix}`;
                const accountStatus: ProviderCardStatus = {
                  connected: false,
                  connectionState: "disconnected",
                };
                return (
                  <ProviderCard
                    key={key}
                    name={displayName}
                    icon="simple-icons:google"
                    description={`Connect a secondary Google account.`}
                    status={accountStatus}
                    connectedMeta={[]}
                    loading={loading}
                    busy={busy}
                    onConnect={() => onConnectGoogleWithKey(key)}
                    onCancel={() => {}}
                    onDisconnect={() => {}}
                    showMinus={true}
                    onMinus={() => onRemoveUnconnectedGoogleAccount(key)}
                    compact={true}
                  />
                );
              })}
          </div>
        )}
        <ProviderCard
          name="Kimi"
          icon="local:kimi"
          description="Use OAuth to connect your Kimi account for Kimi 2.6."
          status={kimiStatus}
          connectedMeta={["Kimi OAuth"]}
          loading={loading}
          busy={busy}
          onConnect={onConnectKimi}
          onCancel={onCancelKimi}
          onDisconnect={onDisconnectKimi}
          providerId="kimi"
        />
        <DeepSeekProviderCard
          status={deepSeekStatus}
          loading={loading}
          busy={busy}
          onDisconnect={onDisconnectDeepSeek}
          onStatusChange={onDeepSeekStatusChange}
          onChanged={onDeepSeekChanged}
        />
        <OpenRouterProviderCard
          status={openRouterStatus}
          models={openRouterModels}
          loading={loading}
          busy={busy}
          onDisconnect={onDisconnectOpenRouter}
          onStatusChange={onOpenRouterStatusChange}
          onModelsChange={onOpenRouterModelsChange}
          onChanged={onOpenRouterChanged}
        />
      </div>
    </>
  );
}

type ProviderCardStatus =
  | OpenAiProviderStatus
  | AnthropicProviderStatus
  | GoogleProviderStatus
  | KimiProviderStatus
  | CursorComposerAuthStatus
  | null;

type ProviderCardProps = {
  name: string;
  icon: string;
  description: string;
  status: ProviderCardStatus;
  connectedMeta: (string | null | undefined)[];
  loading: boolean;
  busy: boolean;
  onConnect: () => void;
  onCancel: () => void;
  onDisconnect: () => void;
  children?: React.ReactNode;
  showPlus?: boolean;
  onPlus?: () => void;
  showMinus?: boolean;
  onMinus?: () => void;
  providerId?: string;
  compact?: boolean;
  connectLabel?: string;
  busyConnectLabel?: string;
};

function ProviderCard({
  name,
  icon,
  description,
  status,
  connectedMeta,
  loading,
  busy,
  onConnect,
  onCancel,
  onDisconnect,
  children,
  showPlus,
  onPlus,
  showMinus,
  onMinus,
  providerId,
  compact,
  connectLabel = "Connect",
  busyConnectLabel = "Opening...",
}: ProviderCardProps) {
  const state = status?.connectionState ?? "disconnected";
  const connected = Boolean(status?.connected);
  const connecting = state === "connecting";
  const error = state === "error" ? status?.error : null;
  const [quota, setQuota] = useState<QuotaInfo | null>(null);

  const isQuotaExhausted = connected && quota && quota.kind !== "unavailable" && (
    quota.kind === "credits"
      ? (quota.creditRemaining != null && quota.creditRemaining <= 0)
      : (quota.kind === "groups" ? quota.groups ?? [] : quota.windows ?? []).some(
          (w) => w.remainingPercent !== null && w.remainingPercent <= 0
        )
  );

  const statusLabel = connecting
    ? "Connecting"
    : isQuotaExhausted
      ? "Limit reached"
      : connected
        ? "Connected"
        : state === "error"
          ? "Needs attention"
          : "Not connected";

  const statusTone = connecting
    ? "pending"
    : isQuotaExhausted
      ? "error"
      : connected
        ? "ok"
        : state === "error"
          ? "error"
          : "off";

  useEffect(() => {
    if (!connected || !providerId) {
      setQuota(null);
      return;
    }
    let active = true;
    const update = async () => {
      const q = await fetchProviderQuota(providerId);
      if (active) setQuota(q);
    };
    update();
    const handleUpdate = () => {
      const cached = getCachedQuota(providerId);
      if (cached && active) {
        setQuota(cached);
      } else {
        update();
      }
    };
    window.addEventListener("sinew:quota-updated", handleUpdate);
    return () => {
      active = false;
      window.removeEventListener("sinew:quota-updated", handleUpdate);
    };
  }, [connected, providerId]);

  const rawMeta = [...connectedMeta];
  if (connected && quota && quota.label) {
    if (quota.label.startsWith("Projet ")) {
      // Check if project is already in connectedMeta to avoid duplicate
      const alreadyHasProject = connectedMeta.some(m => m && (m.includes("Project ") || m.includes("Projet ")));
      if (!alreadyHasProject) {
        rawMeta.push(quota.label);
      }
    } else if (!quota.label.includes("Codex")) {
      rawMeta.push(quota.label);
    }
  }
  const meta = rawMeta.filter((item): item is string => Boolean(item));

  return (
    <section className={`settings-pane__provider-card ${compact ? 'settings-pane__provider-card--compact' : ''}`}>
      <div className="settings-pane__provider-main">
        <div className="settings-pane__provider-mark" aria-hidden>
          <Icon icon={icon} width={compact ? 16 : 24} height={compact ? 16 : 24} />
        </div>
        <div className="settings-pane__provider-copy">
          <div className="settings-pane__provider-title-row">
            <h2>{name}</h2>
            <span className="settings-pane__chip" data-tone={statusTone}>
              <span className="settings-pane__chip-dot" />
              {statusLabel}
            </span>
          </div>
          {!compact && <p>{description}</p>}
          <div className="settings-pane__provider-meta" style={{ marginTop: compact ? "4px" : "8px", alignItems: "center" }}>
            {connected && meta.map((item) => (
              <span key={item}>{item}</span>
            ))}
            {connected && quota && quota.kind !== "unavailable" && (
              <>
                {quota.kind === "credits" ? (
                  <>
                    <QuotaBar inline item={{ label: quota.creditLimit == null ? "Limite" : `Limite $${quota.creditLimit.toFixed(2)}`, remainingPercent: 100 }} />
                    <QuotaBar inline item={{ label: quota.creditRemaining == null ? "Restant" : `Restant $${quota.creditRemaining.toFixed(2)}`, remainingPercent: quota.percentage }} />
                  </>
                ) : (
                  (quota.kind === "groups" ? quota.groups ?? [] : quota.windows ?? []).map((item) => (
                    <QuotaBar inline key={item.label} item={item} />
                  ))
                )}
              </>
            )}
          </div>
          {connected && quota && quota.kind === "unavailable" && (
            <div style={{ marginTop: compact ? "4px" : "8px", color: "var(--text-3)", fontSize: compact ? "10px" : "11px", opacity: 0.7 }}>
              {quota.label ?? "Quota non disponible"}
            </div>
          )}
          {error && <div className="settings-pane__provider-error">{error}</div>}
          {children}
        </div>
      </div>
      <div className="settings-pane__provider-actions">
        {connecting ? (
          <button
            type="button"
            className="settings-pane__btn"
            onClick={onCancel}
            disabled={busy}
          >
            <Icon icon="solar:close-circle-linear" width={13} height={13} />
            <span>Cancel</span>
          </button>
        ) : connected ? (
          <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
            {showPlus && (
              <button
                type="button"
                className="settings-pane__btn"
                onClick={onPlus}
                disabled={loading || busy}
                title="Add account"
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  padding: "6px 8px",
                  minWidth: "auto",
                }}
              >
                <Icon icon="solar:add-circle-linear" width={16} height={16} />
              </button>
            )}
            {showMinus && (
              <button
                type="button"
                className="settings-pane__btn"
                onClick={onMinus}
                disabled={loading || busy}
                title="Remove account"
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  padding: "6px 8px",
                  minWidth: "auto",
                  color: "var(--text-error, #f87171)"
                }}
              >
                <Icon icon="solar:minus-circle-linear" width={16} height={16} />
              </button>
            )}
            <button
              type="button"
              className="settings-pane__btn"
              onClick={onDisconnect}
              disabled={busy}
              style={compact ? { padding: "6px 8px", minWidth: "auto" } : undefined}
              title="Disconnect"
            >
              <Icon icon="solar:logout-2-linear" width={compact ? 16 : 13} height={compact ? 16 : 13} />
              {!compact && <span>{busy ? "Disconnecting..." : "Disconnect"}</span>}
            </button>
          </div>
        ) : (
          <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
            {showMinus && (
              <button
                type="button"
                className="settings-pane__btn"
                onClick={onMinus}
                disabled={loading || busy}
                title="Remove account"
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  padding: "6px 8px",
                  minWidth: "auto",
                  color: "var(--text-error, #f87171)"
                }}
              >
                <Icon icon="solar:minus-circle-linear" width={16} height={16} />
              </button>
            )}
            <button
              type="button"
              className="settings-pane__btn"
              data-primary="true"
              onClick={onConnect}
              disabled={loading || busy}
            >
              <Icon
                icon={busy ? "solar:refresh-linear" : "solar:login-2-linear"}
                width={13}
                height={13}
              />
              <span>{busy ? busyConnectLabel : connectLabel}</span>
            </button>
          </div>
        ) }
      </div>
    </section>
  );
}

function formatResetLabelForWindow(item: { label: string; resetAt?: number | null; resetTime?: string | null }) {
  const value = item.resetAt ?? item.resetTime ?? null;
  if (value == null) return null;
  const targetMs = typeof value === "number" ? value * 1000 : Date.parse(value);
  if (!Number.isFinite(targetMs)) return null;
  const deltaMs = Math.max(0, targetMs - Date.now());
  const lowerLabel = item.label.toLowerCase();
  
  if (lowerLabel.includes("fenetre courte") || lowerLabel.includes("fenêtre courte")) {
    const totalMinutes = Math.max(0, Math.round(deltaMs / 60_000));
    return `reset ${totalMinutes}m`;
  }
  
  if (lowerLabel.includes("fenetre longue") || lowerLabel.includes("fenêtre longue")) {
    const totalHours = Math.max(0, Math.round(deltaMs / 3_600_000));
    return `reset ${totalHours}h`;
  }
  
  const totalMinutes = Math.max(0, Math.round(deltaMs / 60_000));
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  if (hours > 0) return `reset ${hours}h${minutes ? ` ${minutes}m` : ""}`;
  return `reset ${minutes}m`;
}

function formatWindowLabel(window: { label: string; windowMinutes?: number | null }) {
  const lowerLabel = window.label.toLowerCase();
  if (lowerLabel.includes("fenetre courte") || lowerLabel.includes("fenêtre courte")) {
    return "5h";
  }
  if (lowerLabel.includes("fenetre longue") || lowerLabel.includes("fenêtre longue")) {
    return "Semaine";
  }
  if (!window.windowMinutes) return window.label;
  const hours = Math.round(window.windowMinutes / 60);
  if (hours >= 1 && window.windowMinutes % 60 === 0) return `${window.label} (${hours}h)`;
  return `${window.label} (${window.windowMinutes}m)`;
}

function QuotaBar({ item, inline }: { item: { label: string; remainingPercent: number | null; windowMinutes?: number | null; resetAt?: number | null; resetTime?: string | null }; inline?: boolean }) {
  const percent = item.remainingPercent;
  const reset = formatResetLabelForWindow(item);
  
  if (inline) {
    return (
      <span style={{ display: "inline-flex", alignItems: "center", gap: "6px", background: "var(--bg-2)", padding: "3px 7px", borderRadius: "var(--r-med)", color: "var(--text-2)", fontSize: "10px", border: "1px solid var(--line-1)" }}>
        <span style={{ whiteSpace: "nowrap" }}>{formatWindowLabel(item)}</span>
        <div style={{ width: "30px", height: "4px", background: "var(--bg-3)", borderRadius: "2px", overflow: "hidden", flexShrink: 0 }}>
          <div
            style={{
              width: `${percent ?? 0}%`,
              height: "100%",
              background: quotaColor(percent),
              borderRadius: "2px",
            }}
          />
        </div>
        <span style={{ fontWeight: 600, whiteSpace: "nowrap" }}>
          {percent == null ? "—" : `${percent.toFixed(0)}%`}{reset ? ` - ${reset}` : ""}
        </span>
      </span>
    );
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "4px", minWidth: 0 }}>
      <div style={{ display: "flex", justifyContent: "space-between", gap: "8px", fontSize: "11px", minWidth: 0 }}>
        <span style={{
          color: "var(--text-3)",
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
          minWidth: 0,
          flex: "1 1 auto"
        }}>
          {formatWindowLabel(item)}
        </span>
        <span style={{
          color: "var(--text-2)",
          fontWeight: 600,
          whiteSpace: "nowrap",
          minWidth: 0,
          flex: "0 0 auto"
        }} title={percent == null ? "—" : `${percent.toFixed(0)}%${reset ? ` - ${reset}` : ""}`}>
          {percent == null ? "—" : `${percent.toFixed(0)}%`}{reset ? ` - ${reset}` : ""}
        </span>
      </div>
      <div style={{ width: "100%", height: "4px", background: "var(--bg-3)", borderRadius: "2px", overflow: "hidden" }}>
        <div
          style={{
            width: `${percent ?? 0}%`,
            height: "100%",
            background: quotaColor(percent),
            borderRadius: "2px",
            transition: "width 0.3s ease",
          }}
        />
      </div>
    </div>
  );
}

function QuotaInlinePanel({ quota, compact, showLabel = true }: { quota: QuotaInfo; compact?: boolean; showLabel?: boolean }) {
  if (quota.kind === "unavailable") {
    return (
      <div style={{ marginTop: compact ? "4px" : "8px", color: "var(--text-3)", fontSize: compact ? "10px" : "11px", opacity: compact ? 0.7 : 1 }}>
        {quota.label ?? "Quota non disponible"}
      </div>
    );
  }

  const creditLimit = quota.creditLimit;
  const creditRemaining = quota.creditRemaining;
  const windows = quota.kind === "groups" ? quota.groups ?? [] : quota.windows ?? [];

  return (
    <div
      style={{
        marginTop: "12px",
        padding: "10px",
        background: "rgba(0, 0, 0, 0.15)",
        border: "1px solid rgba(255, 255, 255, 0.04)",
        borderRadius: "6px",
        display: "flex",
        flexDirection: "column",
        gap: "9px",
        width: "100%",
      }}
    >
      {showLabel && quota.label && !quota.label.startsWith("Projet") && !quota.label.includes("Codex") && <div style={{ color: "var(--text-3)", fontSize: "11px" }}>{quota.label}</div>}
      {quota.kind === "credits" ? (
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "16px" }}>
          <QuotaBar item={{ label: creditLimit == null ? "Limite" : `Limite $${creditLimit.toFixed(2)}`, remainingPercent: 100 }} />
          <QuotaBar item={{ label: creditRemaining == null ? "Restant" : `Restant $${creditRemaining.toFixed(2)}`, remainingPercent: quota.percentage }} />
        </div>
      ) : (
        <div style={{ display: "grid", gridTemplateColumns: compact ? "1fr" : (windows.length > 1 ? "repeat(auto-fill, minmax(220px, 1fr))" : "1fr"), gap: "16px" }}>
          {windows.map((item) => (
            <QuotaBar key={item.label} item={item} />
          ))}
        </div>
      )}
    </div>
  );
}

function OpenRouterQuotaPanel({ quota, compact }: { quota: QuotaInfo; compact?: boolean }) {
  if (quota.kind === "unavailable") {
    return (
      <div style={{ marginTop: compact ? "4px" : "8px", color: "var(--text-3)", fontSize: compact ? "10px" : "11px", opacity: compact ? 0.7 : 1 }}>
        {quota.label ?? "Quota non disponible"}
      </div>
    );
  }

  const creditLimit = quota.creditLimit;
  const creditRemaining = quota.creditRemaining;
  const windows = quota.kind === "groups" ? quota.groups ?? [] : quota.windows ?? [];

  return (
    <div style={{ display: "flex", flexWrap: "wrap", gap: "16px", alignItems: "center", marginTop: "8px", paddingTop: "8px", borderTop: "1px solid var(--line-1)" }}>
      {quota.kind === "credits" ? (
        <>
          <div style={{ width: "160px" }}><QuotaBar item={{ label: creditLimit == null ? "Limite" : `Limite $${creditLimit.toFixed(2)}`, remainingPercent: 100 }} /></div>
          <div style={{ width: "160px" }}><QuotaBar item={{ label: creditRemaining == null ? "Restant" : `Restant $${creditRemaining.toFixed(2)}`, remainingPercent: quota.percentage }} /></div>
        </>
      ) : (
        <>
          {windows.map((item) => (
            <div key={item.label} style={{ width: "160px" }}><QuotaBar item={item} /></div>
          ))}
        </>
      )}
    </div>
  );
}

type OpenRouterProviderCardProps = {
  status: OpenRouterProviderStatus | null;
  models: OpenRouterModel[];
  loading: boolean;
  busy: boolean;
  onDisconnect: () => void;
  onStatusChange: (status: OpenRouterProviderStatus) => void;
  onModelsChange: (models: OpenRouterModel[]) => void;
  onChanged: () => void;
};

function OpenRouterProviderCard({
  status,
  models,
  loading,
  busy,
  onDisconnect,
  onStatusChange,
  onModelsChange,
  onChanged,
}: OpenRouterProviderCardProps) {
  const [apiKey, setApiKey] = useState("");
  const [revealed, setRevealed] = useState(false);
  const [validating, setValidating] = useState(false);
  const [validationError, setValidationError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<OpenRouterModelSearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [mutatingModelId, setMutatingModelId] = useState<string | null>(null);
  const validationSeq = useRef(0);
  const searchSeq = useRef(0);

  const [quota, setQuota] = useState<QuotaInfo | null>(null);

  const displayStatus: OpenRouterProviderStatus = validating
    ? {
        connected: false,
        connectionState: "connecting",
        modelCount: models.length,
      }
    : status ?? {
        connected: false,
        connectionState: "disconnected",
        modelCount: models.length,
      };
  const state = displayStatus.connectionState;
  const connected = Boolean(displayStatus.connected);
  
  useEffect(() => {
    if (!connected) {
      setQuota(null);
      return;
    }
    let active = true;
    const update = async () => {
      const q = await fetchProviderQuota("openrouter");
      if (active) setQuota(q);
    };
    update();
    const handleUpdate = () => {
      const cached = getCachedQuota("openrouter");
      if (cached && active) {
        setQuota(cached);
      } else {
        update();
      }
    };
    window.addEventListener("sinew:quota-updated", handleUpdate);
    return () => {
      active = false;
      window.removeEventListener("sinew:quota-updated", handleUpdate);
    };
  }, [connected]);

  const connecting = state === "connecting";
  const error = validationError ?? (state === "error" ? displayStatus.error : null);

  const isQuotaExhausted = connected && quota && quota.kind !== "unavailable" && (
    quota.kind === "credits"
      ? (quota.creditRemaining != null && quota.creditRemaining <= 0)
      : (quota.kind === "groups" ? quota.groups ?? [] : quota.windows ?? []).some(
          (w) => w.remainingPercent !== null && w.remainingPercent <= 0
        )
  );

  const statusLabel = connecting
    ? "Connecting"
    : isQuotaExhausted
      ? "Limit reached"
      : connected
        ? "Connected"
        : state === "error"
          ? "Needs attention"
          : "Not connected";

  const statusTone = connecting
    ? "pending"
    : isQuotaExhausted
      ? "error"
      : connected
        ? "ok"
        : state === "error"
          ? "error"
          : "off";
  const modelIds = useMemo(() => new Set(models.map((model) => model.id)), [models]);
  const searchEnabled = connected && !validating;

  useEffect(() => {
    const key = apiKey.trim();
    validationSeq.current += 1;
    const seq = validationSeq.current;
    setValidationError(null);
    if (!key) {
      setValidating(false);
      return;
    }
    setValidating(true);
    const timer = window.setTimeout(() => {
      void (async () => {
        try {
          const next = await api.validateOpenRouterApiKey(key);
          if (validationSeq.current !== seq) return;
          onStatusChange(next);
          setApiKey("");
          setValidationError(null);
          onChanged();
        } catch (err) {
          if (validationSeq.current !== seq) return;
          const message = err instanceof Error ? err.message : String(err);
          setValidationError(message);
          onStatusChange({
            connected: false,
            connectionState: "error",
            modelCount: models.length,
            error: message,
          });
        } finally {
          if (validationSeq.current === seq) setValidating(false);
        }
      })();
    }, 650);
    return () => window.clearTimeout(timer);
  }, [apiKey, models.length, onChanged, onStatusChange]);

  useEffect(() => {
    const trimmed = query.trim();
    searchSeq.current += 1;
    const seq = searchSeq.current;
    setSearchError(null);
    if (!trimmed || !searchEnabled) {
      setSearching(false);
      setResults([]);
      return;
    }
    setSearching(true);
    const timer = window.setTimeout(() => {
      void (async () => {
        try {
          const found = await api.searchOpenRouterModels(trimmed);
          if (searchSeq.current !== seq) return;
          setResults(found);
        } catch (err) {
          if (searchSeq.current !== seq) return;
          const message = err instanceof Error ? err.message : String(err);
          setSearchError(message);
          setResults([]);
          onStatusChange({
            connected: false,
            connectionState: "error",
            modelCount: models.length,
            error: message,
          });
        } finally {
          if (searchSeq.current === seq) setSearching(false);
        }
      })();
    }, 250);
    return () => window.clearTimeout(timer);
  }, [models.length, onStatusChange, query, searchEnabled]);

  const addModel = async (model: OpenRouterModelSearchResult) => {
    setMutatingModelId(model.id);
    setSearchError(null);
    try {
      const cleanName = sanitizeOpenRouterName(model.name) || model.id;
      const next = await api.addOpenRouterModel({ ...model, name: cleanName });
      onModelsChange(next);
      onStatusChange({
        ...(status ?? displayStatus),
        connected: true,
        connectionState: "connected",
        modelCount: next.length,
        error: null,
      });
      onChanged();
      setQuery("");
      setSearchError(null);
    } catch (err) {
      setSearchError(err instanceof Error ? err.message : String(err));
    } finally {
      setMutatingModelId(null);
    }
  };

  const removeModel = async (id: string) => {
    setMutatingModelId(id);
    setSearchError(null);
    try {
      const next = await api.removeOpenRouterModel(id);
      onModelsChange(next);
      if (status) {
        onStatusChange({ ...status, modelCount: next.length });
      }
      onChanged();
    } catch (err) {
      setSearchError(err instanceof Error ? err.message : String(err));
    } finally {
      setMutatingModelId(null);
    }
  };

  return (
    <section className="settings-pane__provider-card settings-pane__provider-card--openrouter">
      <div className="settings-pane__provider-main">
        <div className="settings-pane__provider-mark" aria-hidden>
          <Icon icon="simple-icons:openrouter" width={24} height={24} />
        </div>
        <div className="settings-pane__provider-copy">
          <div className="settings-pane__provider-title-row">
            <h2>OpenRouter</h2>
            <span className="settings-pane__chip" data-tone={statusTone}>
              <span className="settings-pane__chip-dot" />
              {statusLabel}
            </span>
          </div>
          <p>Use any OpenRouter model with your own API key.</p>
          {error && <div className="settings-pane__provider-error">{error}</div>}
          {connected && quota && <OpenRouterQuotaPanel quota={quota} />}
        </div>
      </div>

      <div className="settings-pane__provider-detail">
        <label className="settings-pane__tool-credential">
          <span className="settings-pane__tool-credential-label">API key</span>
          <div className="settings-pane__tool-credential-field">
            <input
              type={revealed ? "text" : "password"}
              value={apiKey}
              placeholder={connected ? displayStatus.keyPreview ?? "Key saved" : "sk-or-..."}
              onChange={(event) => setApiKey(event.target.value)}
              autoComplete="off"
              spellCheck={false}
            />
            <div className="settings-pane__tool-credential-actions">
              <button
                type="button"
                className="settings-pane__icon-btn"
                onClick={() => setRevealed((value) => !value)}
                title={revealed ? "Hide key" : "Show key"}
                aria-label={revealed ? "Hide key" : "Show key"}
              >
                <Icon
                  icon={revealed ? "solar:eye-closed-linear" : "solar:eye-linear"}
                  width={13}
                  height={13}
                />
              </button>
              {connected && (
                <button
                  type="button"
                  className="settings-pane__icon-btn"
                  onClick={onDisconnect}
                  disabled={busy}
                  title="Remove API key"
                  aria-label="Remove API key"
                >
                  <Icon icon="solar:trash-bin-trash-linear" width={13} height={13} />
                </button>
              )}
            </div>
          </div>
        </label>

        <label className="settings-pane__tool-credential">
          <span className="settings-pane__tool-credential-label">Search</span>
          <div className="settings-pane__tool-credential-field">
            <input
              type="text"
              value={query}
              disabled={!searchEnabled}
              placeholder={searchEnabled ? "Type a model name…" : "Save a valid key first"}
              onChange={(event) => setQuery(event.target.value)}
            />
          </div>
        </label>

        {searchEnabled && query.trim() !== "" && (
          <div className="settings-pane__openrouter-results" aria-live="polite">
            {searching ? (
              <div className="settings-pane__openrouter-hint">Searching…</div>
            ) : searchError ? (
              <div className="settings-pane__provider-error">{searchError}</div>
            ) : results.length === 0 ? (
              <div className="settings-pane__openrouter-hint">No matching model.</div>
            ) : (
              results.map((model) => {
                const added = modelIds.has(model.id);
                const label = sanitizeOpenRouterName(model.name) || model.id;
                return (
                  <div key={model.id} className="settings-pane__openrouter-row">
                    <span title={model.id}>{label}</span>
                    {added ? (
                      <span className="settings-pane__openrouter-added">Added</span>
                    ) : (
                      <button
                        type="button"
                        className="settings-pane__btn"
                        onClick={() => void addModel(model)}
                        disabled={mutatingModelId === model.id}
                      >
                        <Icon icon="solar:add-circle-linear" width={13} height={13} />
                        <span>{mutatingModelId === model.id ? "Adding…" : "Add"}</span>
                      </button>
                    )}
                  </div>
                );
              })
            )}
          </div>
        )}

        {models.length > 0 && (
          <div className="settings-pane__openrouter-list">
            {models.map((model) => {
              const label = sanitizeOpenRouterName(model.name) || model.id;
              return (
                <div key={model.id} className="settings-pane__openrouter-row">
                  <span title={model.id}>{label}</span>
                  <button
                    type="button"
                    className="settings-pane__icon-btn"
                    onClick={() => void removeModel(model.id)}
                    disabled={mutatingModelId === model.id}
                    title="Remove model"
                    aria-label={`Remove ${label}`}
                  >
                    <Icon icon="solar:trash-bin-trash-linear" width={13} height={13} />
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </section>
  );
}

// ---- Tools section -----------------------------------------------------

type ToolsSectionProps = {
  settings: ToolSettings | null;
  loading: boolean;
  saving: boolean;
  dirty: boolean;
  status: string | null;
  onSave: () => void;
  onUpdate: (name: string, patch: Partial<ToolConfig>) => void;
  onPlanModePromptChange: (value: string) => void;
  onImageProviderChange: (value: ImageProvider) => void;
  onOpenAiImageUseSubscriptionChange: (value: boolean) => void;
  onGeminiImageUseSubscriptionChange: (value: boolean) => void;
  onOpenAiImageModelChange: (value: string) => void;
  onGeminiImageModelChange: (value: string) => void;
  onOpenAiImageApiKeyChange: (value: string) => void;
  onNanoBananaApiKeyChange: (value: string) => void;
  onWebSearchProviderChange: (value: WebSearchProvider) => void;
  onLinkupApiKeyChange: (value: string) => void;
  openAiStatus: OpenAiProviderStatus | null;
  googleStatus: GoogleProviderStatus | null;
};

const TOOL_GROUPS = [
  { id: "main", label: "Main Agent" },
  { id: "swarm", label: "Swarm Agents" },
] as const;

type ToolGroupId = (typeof TOOL_GROUPS)[number]["id"];

const SWARM_TOOL_NAMES = new Set([
  "send_message",
  "task_list",
]);

function ToolsSection({
  settings,
  loading,
  saving,
  dirty,
  status,
  onSave,
  onUpdate,
  onPlanModePromptChange,
  onImageProviderChange,
  onOpenAiImageUseSubscriptionChange,
  onGeminiImageUseSubscriptionChange,
  onOpenAiImageModelChange,
  onGeminiImageModelChange,
  onOpenAiImageApiKeyChange,
  onNanoBananaApiKeyChange,
  onWebSearchProviderChange,
  onLinkupApiKeyChange,
  openAiStatus,
  googleStatus,
}: ToolsSectionProps) {
  const tools = settings?.tools ?? [];
  const planModePrompt = settings?.planModePrompt ?? "";
  const defaultPlanModePrompt = settings?.defaultPlanModePrompt ?? "";
  const imageProvider = settings?.imageProvider ?? "gptImage2";
  const openaiImageUseSubscription = settings?.openaiImageUseSubscription ?? false;
  const geminiImageUseSubscription = settings?.geminiImageUseSubscription ?? false;
  const openaiImageModel = settings?.openaiImageModel ?? "gpt-image-2";
  const geminiImageModel = settings?.geminiImageModel ?? "gemini-3.1-flash-image-preview";
  const openaiImageApiKey = settings?.openaiImageApiKey ?? "";
  const nanoBananaApiKey = settings?.nanoBananaApiKey ?? "";
  const webSearchProvider = settings?.webSearchProvider ?? "classic";
  const linkupApiKey = settings?.linkupApiKey ?? "";
  const openAiConnected = openAiStatus?.connected === true;
  const googleConnected = googleStatus?.connected === true;
  const openaiSubscriptionActive =
    imageProvider === "gptImage2" && openAiConnected && openaiImageUseSubscription;
  const geminiSubscriptionActive =
    imageProvider === "nanoBanana2" && googleConnected && geminiImageUseSubscription;
  const subscriptionActive =
    imageProvider === "gptImage2" ? openaiSubscriptionActive : geminiSubscriptionActive;
  const showImageKeyField = !subscriptionActive;
  const activeImageKey =
    imageProvider === "nanoBanana2" ? nanoBananaApiKey : openaiImageApiKey;
  const hasImageTool = tools.some((tool) => canonicalToolName(tool.name) === "create_image");
  const hasWebSearchTool = tools.some((tool) => canonicalToolName(tool.name) === "web_search");
  const enabledCount = tools.filter((tool) => tool.enabled).length;
  const groups = TOOL_GROUPS.map((group) => {
    const groupTools = tools.filter((tool) => toolGroupId(tool) === group.id);
    return {
      ...group,
      tools: groupTools,
      enabled: groupTools.filter((tool) => tool.enabled).length,
    };
  }).filter((group) => group.tools.length > 0);

  return (
    <>
      <header className="settings-pane__header">
        <div className="settings-pane__header-text">
          <h1 className="settings-pane__title">Tools</h1>
          <p className="settings-pane__subtitle">
            {loading ? "Loading…" : `${enabledCount}/${tools.length} enabled`}
          </p>
        </div>
        <div className="settings-pane__actions">
          {status && (
            <span
              className="settings-pane__status"
              data-tone={status === "Saved" || status === "Deleted" ? "ok" : "error"}
            >
              {status}
            </span>
          )}
          <button
            type="button"
            className="settings-pane__btn"
            data-primary="true"
            onClick={onSave}
            disabled={loading || saving || !dirty}
          >
            <Icon
              icon={saving ? "solar:refresh-linear" : "solar:diskette-linear"}
              width={13}
              height={13}
            />
            <span>{saving ? "Saving…" : "Save"}</span>
          </button>
        </div>
      </header>

      <div className="settings-pane__body settings-pane__body--tools">
        <div className="settings-pane__tool-settings-list">
          <section className="settings-pane__tool-group">
            <div className="settings-pane__tool-group-head">
              <h2>Plan mode prompt</h2>
              <span>
                {planModePrompt === defaultPlanModePrompt ? "Default" : "Custom"}
              </span>
            </div>
            <PlanModePromptSettingsItem
              value={planModePrompt}
              defaultValue={defaultPlanModePrompt}
              onChange={onPlanModePromptChange}
            />
          </section>
          {hasImageTool && (
            <section className="settings-pane__tool-group">
              <div className="settings-pane__tool-group-head">
                <h2>Génération d&apos;images</h2>
                <span>Outil create_image (OpenAI / Google)</span>
              </div>
              <p className="settings-pane__tool-group-note">
                Les images générées passent par ChatGPT (OpenAI) ou Gemini (Google).
                Avec Composer, utilise l&apos;outil natif « generate image » (branché sur ce provider).
              </p>
              <div
                className="settings-pane__tool-provider-switch"
                role="group"
                aria-label="Fournisseur d'images"
              >
                <button
                  type="button"
                  data-active={imageProvider === "gptImage2" ? "true" : "false"}
                  onClick={() => onImageProviderChange("gptImage2")}
                >
                  ChatGPT
                </button>
                <button
                  type="button"
                  data-active={imageProvider === "nanoBanana2" ? "true" : "false"}
                  onClick={() => onImageProviderChange("nanoBanana2")}
                >
                  Gemini
                </button>
              </div>
              <div style={{ display: "flex", gap: "12px", marginTop: "12px", marginBottom: "12px" }}>
                {imageProvider === "gptImage2" ? (
                  <label className="settings-pane__field" style={{ flex: 1 }}>
                    <span style={{ marginBottom: "4px", display: "block", fontSize: "11px", fontWeight: 500, color: "var(--text-2)" }}>Modèle d&apos;image</span>
                    <select
                      value={openaiImageModel}
                      onChange={(e) => onOpenAiImageModelChange(e.target.value)}
                      style={{ background: "var(--bg-1)", border: "1px solid var(--border-0)", color: "var(--text-0)" }}
                    >
                      <option value="gpt-image-2">gpt-image-2 (ChatGPT Images 2.0 - Le plus performant)</option>
                      <option value="gpt-image-1.5">gpt-image-1.5 (ChatGPT Images 1.5 - Rapide et stable)</option>
                      <option value="dall-e-3">dall-e-3 (DALL-E 3 - Classique)</option>
                    </select>
                  </label>
                ) : (
                  <label className="settings-pane__field" style={{ flex: 1 }}>
                    <span style={{ marginBottom: "4px", display: "block", fontSize: "11px", fontWeight: 500, color: "var(--text-2)" }}>Modèle d&apos;image</span>
                    <select
                      value={geminiImageModel}
                      onChange={(e) => onGeminiImageModelChange(e.target.value)}
                      style={{ background: "var(--bg-1)", border: "1px solid var(--border-0)", color: "var(--text-0)" }}
                    >
                      <option value="gemini-3.1-flash-image-preview">gemini-3.1-flash-image-preview (Nano Banana 2 - Le plus rapide)</option>
                      <option value="gemini-3-pro-image-preview">gemini-3-pro-image-preview (Nano Banana Pro - Précision / Thinking)</option>
                      <option value="gemini-2.5-flash-image">gemini-2.5-flash-image (Nano Banana - Classique)</option>
                    </select>
                  </label>
                )}
              </div>
              {imageProvider === "gptImage2" && (
                <div
                  className="settings-pane__tool-toggle-row"
                  data-disabled={openAiConnected ? "false" : "true"}
                >
                  <div className="settings-pane__tool-toggle-text">
                    <span className="settings-pane__tool-toggle-label">
                      Utiliser l&apos;abonnement OpenAI
                    </span>
                    <span className="settings-pane__tool-toggle-hint">
                      {openAiConnected
                        ? "Authentifie les requêtes image avec ton compte OpenAI connecté, sans clé API."
                        : "Connecte OpenAI dans Paramètres → Providers pour utiliser ton abonnement."}
                    </span>
                  </div>
                  <button
                    type="button"
                    className="settings-pane__switch"
                    role="switch"
                    aria-checked={openaiSubscriptionActive}
                    aria-label={
                      openaiSubscriptionActive
                        ? "Disable OpenAI subscription mode"
                        : "Enable OpenAI subscription mode"
                    }
                    data-on={openaiSubscriptionActive ? "true" : "false"}
                    disabled={!openAiConnected}
                    onClick={() =>
                      onOpenAiImageUseSubscriptionChange(!openaiImageUseSubscription)
                    }
                  >
                    <span className="settings-pane__switch-thumb" />
                  </button>
                </div>
              )}
              {imageProvider === "nanoBanana2" && (
                <div
                  className="settings-pane__tool-toggle-row"
                  data-disabled={googleConnected ? "false" : "true"}
                >
                  <div className="settings-pane__tool-toggle-text">
                    <span className="settings-pane__tool-toggle-label">
                      Utiliser l&apos;abonnement Gemini
                    </span>
                    <span className="settings-pane__tool-toggle-hint">
                      {googleConnected
                        ? "Authentifie les requêtes image avec ton compte Gemini/Google connecté, sans clé API."
                        : "Connecte Google dans Paramètres → Providers pour utiliser ton abonnement."}
                    </span>
                  </div>
                  <button
                    type="button"
                    className="settings-pane__switch"
                    role="switch"
                    aria-checked={geminiSubscriptionActive}
                    aria-label={
                      geminiSubscriptionActive
                        ? "Disable Gemini subscription mode"
                        : "Enable Gemini subscription mode"
                    }
                    data-on={geminiSubscriptionActive ? "true" : "false"}
                    disabled={!googleConnected}
                    onClick={() =>
                      onGeminiImageUseSubscriptionChange(!geminiImageUseSubscription)
                    }
                  >
                    <span className="settings-pane__switch-thumb" />
                  </button>
                </div>
              )}
              {showImageKeyField && (
                <ApiKeyField
                  label={
                    imageProvider === "nanoBanana2"
                      ? "Clé API Gemini"
                      : "Clé API OpenAI"
                  }
                  value={activeImageKey}
                  placeholder={imageProvider === "nanoBanana2" ? "AIza..." : "sk-..."}
                  onChange={
                    imageProvider === "nanoBanana2"
                      ? onNanoBananaApiKeyChange
                      : onOpenAiImageApiKeyChange
                  }
                />
              )}
            </section>
          )}
          {hasWebSearchTool && (
            <section className="settings-pane__tool-group">
              <div className="settings-pane__tool-group-head">
                <h2>Web search</h2>
              </div>
              <div
                className="settings-pane__tool-provider-switch"
                role="group"
                aria-label="Web search provider"
              >
                <button
                  type="button"
                  data-active={webSearchProvider === "classic" ? "true" : "false"}
                  onClick={() => onWebSearchProviderChange("classic")}
                >
                  Classic
                </button>
                <button
                  type="button"
                  data-active={webSearchProvider === "linkup" ? "true" : "false"}
                  onClick={() => onWebSearchProviderChange("linkup")}
                >
                  LinkUp
                </button>
              </div>
              {webSearchProvider === "linkup" && (
                <ApiKeyField
                  label="LinkUp API key"
                  value={linkupApiKey}
                  placeholder="linkup key"
                  onChange={onLinkupApiKeyChange}
                />
              )}
            </section>
          )}
          {groups.map((group) => (
            <section className="settings-pane__tool-group" key={group.id}>
              <div className="settings-pane__tool-group-head">
                <h2>{group.label}</h2>
                <span>
                  {group.enabled}/{group.tools.length}
                </span>
              </div>
              <div className="settings-pane__tool-group-list">
                {group.tools.map((tool) => (
                  <ToolSettingsItem
                    key={tool.name}
                    tool={tool}
                    onUpdate={(patch) => onUpdate(tool.name, patch)}
                  />
                ))}
              </div>
            </section>
          ))}
          {!loading && tools.length === 0 && (
            <div className="settings-pane__empty settings-pane__empty--main">
              <WrenchIcon size={22} />
              <span className="settings-pane__empty-title">No tools</span>
            </div>
          )}
        </div>
      </div>
    </>
  );
}

function toolGroupId(tool: ToolConfig): ToolGroupId {
  return SWARM_TOOL_NAMES.has(canonicalToolName(tool.name)) ? "swarm" : "main";
}

function PlanModePromptSettingsItem({
  value,
  defaultValue,
  onChange,
}: {
  value: string;
  defaultValue: string;
  onChange: (value: string) => void;
}) {
  const canReset = value !== defaultValue;
  const rows = Math.min(18, Math.max(10, value.split("\n").length + 1));

  return (
    <div className="settings-pane__tool-config" data-on="true">
      <div className="settings-pane__tool-config-head">
        <span className="settings-pane__tool-config-name">
          <span className="settings-pane__tool-config-glyph" aria-hidden>
            <Icon icon="solar:document-text-linear" width={15} height={15} />
          </span>
          <span className="settings-pane__tool-config-label">
            Prompt injected into Plan mode
          </span>
        </span>
        <div className="settings-pane__tool-config-actions">
          <button
            type="button"
            className="settings-pane__icon-btn"
            aria-label="Reset Plan mode prompt"
            title="Reset prompt"
            disabled={!canReset}
            onClick={() => onChange(defaultValue)}
          >
            <Icon icon="solar:refresh-linear" width={14} height={14} />
          </button>
        </div>
      </div>
      <p className="settings-pane__tool-config-help">
        This text is appended to the system prompt only when the conversation is in Plan mode.
      </p>
      <textarea
        className="settings-pane__tool-config-desc settings-pane__tool-config-desc--prompt"
        aria-label="Plan mode prompt"
        value={value}
        rows={rows}
        placeholder="Plan mode instructions…"
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}

function ToolSettingsItem({
  tool,
  onUpdate,
}: {
  tool: ToolConfig;
  onUpdate: (patch: Partial<ToolConfig>) => void;
}) {
  const canReset = tool.description !== tool.defaultDescription;
  const rows = Math.min(8, Math.max(3, tool.description.split("\n").length + 1));

  return (
    <div
      className="settings-pane__tool-config"
      data-on={tool.enabled ? "true" : "false"}
    >
      <div className="settings-pane__tool-config-head">
        <span className="settings-pane__tool-config-name">
          <span
            className="settings-pane__tool-config-glyph"
            aria-hidden
          >
            <ToolGlyph name={tool.name} />
          </span>
          <span className="settings-pane__tool-config-label">
            {labelForTool(tool)}
          </span>
        </span>
        <div className="settings-pane__tool-config-actions">
          <button
            type="button"
            className="settings-pane__icon-btn"
            aria-label={`Reset ${tool.name} description`}
            title="Reset description"
            disabled={!canReset}
            onClick={() => onUpdate({ description: tool.defaultDescription })}
          >
            <Icon icon="solar:refresh-linear" width={14} height={14} />
          </button>
          <button
            type="button"
            className="settings-pane__switch"
            role="switch"
            aria-checked={tool.enabled}
            aria-label={`${tool.enabled ? "Disable" : "Enable"} ${tool.name}`}
            data-on={tool.enabled ? "true" : "false"}
            onClick={() => onUpdate({ enabled: !tool.enabled })}
          >
            <span className="settings-pane__switch-thumb" />
          </button>
        </div>
      </div>
      <textarea
        className="settings-pane__tool-config-desc"
        aria-label={`${tool.name} description`}
        value={tool.description}
        rows={rows}
        onChange={(event) => onUpdate({ description: event.target.value })}
      />
    </div>
  );
}

// ---- MCP section component ---------------------------------------------

type McpSectionProps = {
  workspacePath: string;
  loading: boolean;
  saving: boolean;
  probing: boolean;
  dirty: boolean;
  status: string | null;
  parseError: string | null;
  jsonText: string;
  onJsonChange: (value: string) => void;
  onSave: () => void;
  servers: McpServerConfig[];
  probes: McpServerProbe[];
  onSelectServer: (id: string) => void;
  selectedServer: McpServerConfig | null;
  advancedOpen: boolean;
  onAdvancedOpenChange: (open: boolean) => void;
  selectedProbe: McpServerProbe | null;
  knownToolCounts: Record<string, number>;
  onToggleEnabled: (id: string) => void;
  onToggleAutoLoad: (id: string) => void;
  onRefreshProbes: () => void;
  onMount?: (editor: any, monaco: any) => void;
};

function McpSection({
  workspacePath,
  loading,
  saving,
  probing,
  dirty,
  status,
  parseError,
  jsonText,
  onJsonChange,
  onSave,
  servers,
  probes,
  onSelectServer,
  selectedServer,
  advancedOpen,
  onAdvancedOpenChange,
  selectedProbe,
  knownToolCounts,
  onToggleEnabled,
  onToggleAutoLoad,
  onRefreshProbes,
  onMount,
}: McpSectionProps) {
  const enabledCount = servers.filter((server) => server.enabled).length;
  const failedCount = probes.filter((probe) => probe.enabled && !probe.ok).length;

  return (
    <>
      <header className="settings-pane__header">
        <div className="settings-pane__header-text">
          <h1 className="settings-pane__title">MCP servers</h1>
          <p className="settings-pane__subtitle">
            {loading
              ? "Loading servers…"
              : servers.length === 0
                ? "Add servers in advanced config, then turn them on here."
                : `${enabledCount}/${servers.length} enabled${failedCount ? ` - ${failedCount} need attention` : ""}`}
          </p>
        </div>
        <div className="settings-pane__actions">
          {status && (
            <span className="settings-pane__status" data-tone={parseError ? "error" : "ok"}>
              {status}
            </span>
          )}
          <button
            type="button"
            className="settings-pane__btn"
            onClick={onRefreshProbes}
            disabled={probing || enabledCount === 0}
            title="Reconnecter et rafraîchir tous les serveurs MCP"
          >
            <Icon icon="solar:restart-linear" width={13} height={13} />
            <span>{probing ? "Scan…" : "Refresh"}</span>
          </button>
          <button
            type="button"
            className="settings-pane__btn"
            onClick={() => onAdvancedOpenChange(!advancedOpen)}
            disabled={loading}
          >
            <Icon icon="solar:code-square-linear" width={13} height={13} />
            <span>{advancedOpen ? "Hide config" : "Advanced config"}</span>
          </button>
          <button
            type="button"
            className="settings-pane__btn"
            data-primary="true"
            onClick={onSave}
            disabled={loading || saving || !dirty}
          >
            <Icon
              icon={saving ? "solar:refresh-linear" : "solar:diskette-linear"}
              width={13}
              height={13}
            />
            <span>{saving ? "Checking…" : dirty ? "Save changes" : "Saved"}</span>
          </button>
        </div>
      </header>

      <div className="settings-pane__body settings-pane__body--mcp">
        <aside className="settings-pane__nav-list">
          <div className="settings-pane__nav-list-head">
            <span>Servers</span>
            {probing && (
              <span className="settings-pane__servers-meta">probing…</span>
            )}
          </div>
          <div className="settings-pane__nav-list-items">
            {servers.map((server) => {
              const probe = probes.find((item) => item.serverId === server.id);
              const tone = !server.enabled
                ? "off"
                : !probe
                  ? "pending"
                  : probe.ok
                    ? "ok"
                    : "error";
              const isActive = selectedServer?.id === server.id;
              const knownCount = knownToolCounts[server.id];
              const toolCount = probe?.ok
                ? probe.tools.length
                : knownCount ?? null;
              const toggleDisabled =
                loading || saving || Boolean(parseError);
              const displayName = server.name || "Untitled";
              return (
                <div
                  key={server.id}
                  className="settings-pane__nav-list-item"
                  data-active={isActive ? "true" : "false"}
                  data-on={server.enabled ? "true" : "false"}
                >
                  <button
                    type="button"
                    className="settings-pane__nav-list-item-main"
                    onClick={() => onSelectServer(server.id)}
                  >
                    <span
                      className="settings-pane__nav-list-item-dot"
                      data-tone={tone}
                      aria-hidden
                    />
                    <span className="settings-pane__nav-list-item-name">
                      {displayName}
                    </span>
                    {toolCount !== null && (
                      <span
                        className="settings-pane__nav-list-item-count"
                        data-on={server.enabled ? "true" : "false"}
                        title={`${toolCount} tool${toolCount === 1 ? "" : "s"}`}
                        aria-label={`${toolCount} tool${toolCount === 1 ? "" : "s"}`}
                      >
                        {toolCount}
                      </span>
                    )}
                  </button>
                  <button
                    type="button"
                    className="settings-pane__switch"
                    role="switch"
                    aria-checked={server.enabled}
                    aria-label={`${server.enabled ? "Disable" : "Enable"} ${displayName}`}
                    data-on={server.enabled ? "true" : "false"}
                    disabled={toggleDisabled}
                    onClick={(event) => {
                      event.stopPropagation();
                      onToggleEnabled(server.id);
                    }}
                  >
                    <span className="settings-pane__switch-thumb" />
                  </button>
                </div>
              );
            })}
            {servers.length === 0 && (
              <div className="settings-pane__nav-list-empty">
                No servers yet — add one in the raw config.
              </div>
            )}
          </div>
        </aside>

        <main className="settings-pane__detail-pane">
          {advancedOpen ? (
            <div className="settings-pane__editor-card">
              <div className="settings-pane__editor-bar">
                <div className="settings-pane__editor-bar-left">
                  <Icon icon="solar:code-square-linear" width={13} height={13} />
                  <span>mcp.json</span>
                </div>
                <div className="settings-pane__editor-bar-right">
                  {dirty ? (
                    <span className="settings-pane__pill" data-tone="dirty">
                      <span className="settings-pane__pill-dot" />
                      Unsaved
                    </span>
                  ) : (
                    <span className="settings-pane__pill" data-tone="ok">
                      Synced
                    </span>
                  )}
                </div>
              </div>
              <div className="settings-pane__editor-host">
                <Editor
                  value={jsonText}
                  language="json"
                  theme="sinew-cool"
                  onChange={(value) => onJsonChange(value ?? "")}
                  onMount={onMount}
                  options={{
                    fontFamily:
                      '"Geist Mono", ui-monospace, "SF Mono", Menlo, monospace',
                    fontSize: 12,
                    lineHeight: 18,
                    minimap: { enabled: false },
                    scrollBeyondLastLine: false,
                    smoothScrolling: true,
                    renderLineHighlight: "line",
                    padding: { top: 12, bottom: 12 },
                    tabSize: 2,
                    wordWrap: "off",
                    automaticLayout: true,
                    lineNumbers: "on",
                    lineNumbersMinChars: 3,
                    folding: true,
                    bracketPairColorization: { enabled: true },
                    scrollbar: {
                      verticalScrollbarSize: 9,
                      horizontalScrollbarSize: 9,
                    },
                  }}
                />
              </div>
              {parseError && (
                <div className="settings-pane__editor-error">
                  <Icon icon="solar:danger-triangle-linear" width={13} height={13} />
                  <span>{parseError}</span>
                </div>
              )}
            </div>
          ) : selectedServer ? (
            <ServerDetail
              workspacePath={workspacePath}
              server={selectedServer}
              probe={selectedProbe}
              probing={probing}
              knownToolCount={knownToolCounts[selectedServer.id]}
              onToggleAutoLoad={onToggleAutoLoad}
              onRefreshProbes={onRefreshProbes}
            />
          ) : (
            <div className="settings-pane__empty-state">
              <Icon icon="solar:server-square-cloud-linear" width={18} height={18} />
              <div>
                <strong>No MCP servers configured yet.</strong>
                <span>Use Advanced config to paste an MCP server block.</span>
              </div>
            </div>
          )}
        </main>
      </div>
    </>
  );
}

type ServerDetailProps = {
  workspacePath: string;
  server: McpServerConfig;
  probe: McpServerProbe | null;
  probing: boolean;
  knownToolCount: number | undefined;
  onToggleAutoLoad?: (id: string) => void;
  onRefreshProbes?: () => void;
};

function ServerDetail({ workspacePath, server, probe, probing, knownToolCount, onToggleAutoLoad, onRefreshProbes }: ServerDetailProps) {
  const [expandedTools, setExpandedTools] = useState<Set<string>>(
    () => new Set<string>(),
  );
  const toggleTool = useCallback((toolName: string) => {
    setExpandedTools((prev) => {
      const next = new Set(prev);
      if (next.has(toolName)) {
        next.delete(toolName);
      } else {
        next.add(toolName);
      }
      return next;
    });
  }, []);
  const tone = !server.enabled
    ? "off"
    : !probe
      ? "pending"
      : probe.ok
        ? "ok"
        : "error";
  const statusLabel = !server.enabled
    ? knownToolCount != null
      ? `${knownToolCount} tool${knownToolCount === 1 ? "" : "s"}`
      : "disabled"
    : !probe
      ? probing
        ? "probing…"
        : "pending"
      : !probe.ok
        ? "failed"
        : `${probe.tools.length} tool${probe.tools.length === 1 ? "" : "s"}`;
  const command = [server.command, ...server.args].join(" ").trim();

  return (
    <div className="settings-pane__detail">
      <div className="settings-pane__detail-head">
        <span className="settings-pane__detail-title">{server.name}</span>
        <span className="settings-pane__chip" data-tone={tone}>
          <span className="settings-pane__chip-dot" />
          {statusLabel}
        </span>
      </div>

      <div className="settings-pane__detail-body">
        {command && (
          <code className="settings-pane__detail-cmd" title={command}>
            {command}
          </code>
        )}
        <div className="settings-pane__detail-row">
          <span>Exposer tous les outils au démarrage</span>
          <button
            type="button"
            className="settings-pane__switch"
            role="switch"
            aria-checked={server.autoLoad ?? false}
            aria-label={`Toujours exposer les outils de ${server.name}`}
            data-on={server.autoLoad ? "true" : "false"}
            onClick={() => onToggleAutoLoad?.(server.id)}
          >
            <span className="settings-pane__switch-thumb" />
          </button>
        </div>
        {server.cwd && (
          <div className="settings-pane__detail-meta">
            <span className="settings-pane__detail-key">cwd</span>
            <code>{server.cwd}</code>
          </div>
        )}
        {server.env.length > 0 && (
          <div className="settings-pane__detail-meta">
            <span className="settings-pane__detail-key">env</span>
            <code>{server.env.map((item) => item.key).join(", ")}</code>
          </div>
        )}

        {probe?.error && (
          <div className="settings-pane__tools-error">{probe.error}</div>
        )}

        {server.id === "sinew-chrome" && (
          <div className="settings-pane__chrome-bridge-repair" style={{
            marginTop: "12px",
            padding: "12px",
            border: "1px solid var(--border-color)",
            borderRadius: "6px",
            background: "var(--bg-card)",
            display: "flex",
            flexDirection: "column",
            gap: "8px"
          }}>
            <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
              <Icon icon="solar:info-circle-linear" width={16} height={16} style={{ color: "#3b82f6" }} />
              <strong style={{ fontSize: "13px" }}>Synchronisation et configuration locale</strong>
            </div>
            <p style={{ margin: 0, fontSize: "12px", opacity: 0.8, lineHeight: "1.4" }}>
              Le pont Chrome nécessite des fichiers système locaux (`AppData`) et une clé de registre Windows.
              Si vous êtes sur un nouveau PC, ou si le pont ne répond pas, vous pouvez l'enregistrer à nouveau en un clic.
            </p>
            <div style={{
              fontSize: "12px",
              lineHeight: "1.4",
              background: "rgba(239, 68, 68, 0.15)",
              border: "1px solid rgba(239, 68, 68, 0.3)",
              padding: "8px",
              borderRadius: "4px",
              color: "#f87171",
              marginTop: "4px",
              fontWeight: "500"
            }}>
              ⚠️ IMPORTANT : Pour que l'automatisation fonctionne, vous devez également charger l'extension une première fois dans Google Chrome. 
              Allez sur <code style={{color: "#fff", background: "rgba(0,0,0,0.3)", padding: "2px 4px", borderRadius: "3px"}}>chrome://extensions</code>, activez le "Mode développeur", cliquez sur "Charger l'extension non empaquetée" et sélectionnez le dossier <code style={{color: "#fff", background: "rgba(0,0,0,0.3)", padding: "2px 4px", borderRadius: "3px"}}>sinew-chrome-bridge</code> de ce projet.
            </div>
            <button
              type="button"
              className="settings-pane__btn"
              style={{
                alignSelf: "flex-start",
                background: "#2563eb",
                color: "#fff",
                border: "none",
                padding: "6px 12px",
                borderRadius: "4px",
                cursor: "pointer",
                display: "flex",
                alignItems: "center",
                gap: "6px",
                fontSize: "12px",
                fontWeight: 500,
                marginTop: "4px"
              }}
              onClick={async () => {
                try {
                  await api.registerChromeBridge(workspacePath);
                  onRefreshProbes?.();
                  alert("Le pont Chrome a été enregistré avec succès sur cet ordinateur ! Veuillez redémarrer l'application pour appliquer les changements.");
                } catch (e) {
                  alert("Erreur lors de l'enregistrement : " + e);
                }
              }}
            >
              <Icon icon="solar:settings-linear" width={14} height={14} />
              <span>Configurer/Réparer le pont local</span>
            </button>
          </div>
        )}

        <div className="settings-pane__detail-section">Tools</div>
        <div className="settings-pane__tool-list">
          {probe?.tools.map((tool) => {
            const isOpen = expandedTools.has(tool.toolName);
            const hasDescription = Boolean(tool.description?.trim());
            return (
              <div className="settings-pane__tool" key={tool.toolName}>
                <button
                  type="button"
                  className="settings-pane__tool-head"
                  onClick={() => toggleTool(tool.toolName)}
                  aria-expanded={isOpen}
                  data-open={isOpen ? "true" : "false"}
                >
                  <span className="settings-pane__tool-head-text">
                    <span className="settings-pane__tool-name">
                      {tool.title || tool.name}
                    </span>
                    <code className="settings-pane__tool-id">
                      {tool.toolName}
                    </code>
                  </span>
                  <Icon
                    icon="solar:alt-arrow-down-linear"
                    width={12}
                    height={12}
                    className="settings-pane__tool-caret"
                  />
                </button>
                {isOpen && (
                  <div className="settings-pane__tool-body">
                    {hasDescription ? (
                      <Markdown
                        text={tool.description ?? ""}
                        onOpenFile={noop}
                      />
                    ) : (
                      <div className="settings-pane__muted">
                        No description provided.
                      </div>
                    )}
                  </div>
                )}
              </div>
            );
          })}
          {probe?.ok && probe.tools.length === 0 && (
            <div className="settings-pane__muted">Server returned no tools.</div>
          )}
          {!probe && (
            <div className="settings-pane__muted">
              {probing ? "Probing server…" : "No probe data yet."}
            </div>
          )}
          {!probe && server.enabled && onRefreshProbes && !probing ? (
            <button
              type="button"
              className="settings-pane__btn"
              onClick={onRefreshProbes}
            >
              <Icon icon="solar:refresh-linear" width={13} height={13} />
              <span>Probe now</span>
            </button>
          ) : null}
        </div>
      </div>
    </div>
  );
}

// ---- Sub-agent section -------------------------------------------------

type SubAgentsSectionProps = {
  settings: SubAgentSettings;
  selectedAgent: SubAgentConfig | null;
  loading: boolean;
  saving: boolean;
  dirty: boolean;
  status: string | null;
  availableModels: readonly ModelEntry[];
  onSelect: (id: string) => void;
  onAdd: () => void;
  onDelete: (id: string) => void;
  onSave: () => void;
  onUpdate: (id: string, patch: Partial<SubAgentConfig>) => void;
};

function SubAgentsSection({
  settings,
  selectedAgent,
  loading,
  saving,
  dirty,
  status,
  availableModels,
  onSelect,
  onAdd,
  onDelete,
  onSave,
  onUpdate,
}: SubAgentsSectionProps) {
  const enabledCount = settings.agents.filter((agent) => agent.enabled).length;

  return (
    <>
      <header className="settings-pane__header">
        <div className="settings-pane__header-text">
          <h1 className="settings-pane__title">Sub-agents</h1>
          <p className="settings-pane__subtitle">
            {settings.agents.length === 0
              ? "Create focused agents the main agent can call as tools."
              : `${enabledCount}/${settings.agents.length} available to the main agent`}
          </p>
        </div>
        <div className="settings-pane__actions">
          {status && (
            <span
              className="settings-pane__status"
              data-tone={
                status === "Saved" || status === "Created" || status === "Deleted"
                  ? "ok"
                  : "error"
              }
            >
              {status}
            </span>
          )}
          <button
            type="button"
            className="settings-pane__btn"
            data-primary="true"
            onClick={onSave}
            disabled={loading || saving || !dirty}
          >
            <Icon
              icon={saving ? "solar:refresh-linear" : "solar:diskette-linear"}
              width={13}
              height={13}
            />
            <span>{saving ? "Saving…" : "Save"}</span>
          </button>
        </div>
      </header>

      <div className="settings-pane__body settings-pane__body--subagents">
        <aside className="settings-pane__nav-list">
          <div className="settings-pane__nav-list-head">
            <span>Agents</span>
            <button
              type="button"
              className="settings-pane__nav-list-add"
              onClick={onAdd}
              aria-label="New agent"
              title="New agent"
            >
              <Icon icon="solar:add-circle-linear" width={13} height={13} />
            </button>
          </div>
          <div className="settings-pane__nav-list-items">
            {settings.agents.map((agent) => (
              <button
                type="button"
                key={agent.id}
                className="settings-pane__nav-list-item"
                data-active={selectedAgent?.id === agent.id ? "true" : "false"}
                data-on={agent.enabled ? "true" : "false"}
                onClick={() => onSelect(agent.id)}
              >
                <span
                  className="settings-pane__nav-list-item-dot"
                  data-tone={agent.enabled ? "ok" : "off"}
                  aria-hidden
                />
                <span className="settings-pane__nav-list-item-name">
                  {agent.name || "Untitled"}
                </span>
              </button>
            ))}
            {!loading && settings.agents.length === 0 && (
              <div className="settings-pane__nav-list-empty">
                No sub-agents yet — click + to start.
              </div>
            )}
          </div>
        </aside>

        <main className="settings-pane__detail-pane">
          {selectedAgent ? (
            <SubAgentEditor
              agent={selectedAgent}
              availableModels={availableModels}
              onUpdate={(patch) => onUpdate(selectedAgent.id, patch)}
              onDelete={() => onDelete(selectedAgent.id)}
            />
          ) : (
            <div className="settings-pane__empty settings-pane__empty--main">
              <Icon
                icon="solar:branching-paths-down-linear"
                width={22}
                height={22}
              />
              <span className="settings-pane__empty-title">
                Select or create an agent
              </span>
            </div>
          )}
        </main>
      </div>
    </>
  );
}

function SettingsPicker({
  value,
  options,
  onSelect,
}: {
  value: string;
  options: { value: string; label: string; icon?: string }[];
  onSelect: (value: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (event: MouseEvent) => {
      if (
        ref.current &&
        event.target instanceof Node &&
        !ref.current.contains(event.target)
      ) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  const selected = options.find((opt) => opt.value === value);

  return (
    <div className="settings-pane__picker" ref={ref}>
      <button
        type="button"
        className="settings-pane__picker-btn"
        data-open={open ? "true" : "false"}
        onClick={() => setOpen((value) => !value)}
      >
        <span className="settings-pane__picker-label">
          {selected?.icon && (
            <Icon icon={selected.icon} width={12} height={12} />
          )}
          <span>{selected?.label ?? "—"}</span>
        </span>
        <Icon icon="solar:alt-arrow-down-linear" width={11} height={11} />
      </button>
      {open && (
        <div className="settings-pane__picker-pop" role="menu">
          {options.map((opt) => {
            const isSelected = opt.value === value;
            return (
              <button
                type="button"
                key={opt.value}
                className="settings-pane__picker-row"
                data-selected={isSelected ? "true" : "false"}
                onClick={() => {
                  onSelect(opt.value);
                  setOpen(false);
                }}
              >
                <span className="settings-pane__picker-row-label">
                  {opt.icon && (
                    <Icon icon={opt.icon} width={12} height={12} />
                  )}
                  <span>{opt.label}</span>
                </span>
                {isSelected && (
                  <Icon
                    icon="solar:check-read-linear"
                    width={12}
                    height={12}
                  />
                )}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

function settingsThinkingLabel(
  level: (typeof THINKING_LEVELS)[number],
  model: ModelEntry | null,
): string {
  if (model?.provider === "kimi" && level.value !== "off") return "Thinking";
  return level.label;
}

function SubAgentEditor({
  agent,
  availableModels,
  onUpdate,
  onDelete,
}: {
  agent: SubAgentConfig;
  availableModels: readonly ModelEntry[];
  onUpdate: (patch: Partial<SubAgentConfig>) => void;
  onDelete: () => void;
}) {
  const rawModelId = modelIdFromRef(agent.model);
  const thinking = thinkingFromRef(agent.model);
  const modelEntry =
    availableModels.find((model) => model.value === rawModelId) ??
    availableModels[0] ??
    null;
  const modelId = modelEntry?.value ?? rawModelId;
  const thinkingOptions = modelEntry
    ? THINKING_LEVELS.filter((level) => modelEntry.thinking.includes(level.value))
    : [];

  const updateModel = (nextModelId: ModelId) => {
    const nextEntry =
      availableModels.find((model) => model.value === nextModelId) ??
      availableModels[0];
    if (!nextEntry) return;
    const nextThinking = nextEntry.thinking.includes(thinking)
      ? thinking
      : nextEntry.defaultThinking;
    onUpdate({
      model: modelRefWithThinking(modelRefFromId(nextModelId), nextThinking),
    });
  };

  const updateThinking = (nextThinking: ThinkingLevel) => {
    if (!modelEntry) return;
    onUpdate({
      model: modelRefWithThinking(modelRefFromId(modelId), nextThinking),
    });
  };

  const [confirmDelete, setConfirmDelete] = useState(false);
  useEffect(() => {
    if (!confirmDelete) return;
    const id = window.setTimeout(() => setConfirmDelete(false), 3000);
    return () => window.clearTimeout(id);
  }, [confirmDelete]);
  useEffect(() => {
    setConfirmDelete(false);
  }, [agent.id]);

  return (
    <div className="settings-pane__subagent-editor">
      <div className="settings-pane__detail-head">
        <input
          className="settings-pane__detail-title-input"
          value={agent.name}
          onChange={(event) => onUpdate({ name: event.target.value })}
          placeholder="Untitled agent"
          aria-label="Agent name"
        />
        <div className="settings-pane__detail-head-actions">
          <button
            type="button"
            className="settings-pane__icon-btn"
            data-confirm={confirmDelete ? "true" : "false"}
            onClick={() => {
              if (confirmDelete) {
                onDelete();
              } else {
                setConfirmDelete(true);
              }
            }}
            title={confirmDelete ? "Click again to confirm" : "Delete agent"}
            aria-label={confirmDelete ? "Confirm delete" : "Delete agent"}
          >
            {confirmDelete ? (
              <span className="settings-pane__icon-btn-confirm">Delete?</span>
            ) : (
              <Icon icon="solar:trash-bin-trash-linear" width={13} height={13} />
            )}
          </button>
          <button
            type="button"
            className="settings-pane__switch"
            role="switch"
            aria-checked={agent.enabled}
            aria-label={`${agent.enabled ? "Disable" : "Enable"} ${agent.name}`}
            data-on={agent.enabled ? "true" : "false"}
            onClick={() => onUpdate({ enabled: !agent.enabled })}
          >
            <span className="settings-pane__switch-thumb" />
          </button>
        </div>
      </div>

      <div className="settings-pane__subagent-form">
        <label className="settings-pane__field settings-pane__field--grow">
          <span>Description seen by the main agent</span>
          <textarea
            value={agent.description}
            onChange={(event) => onUpdate({ description: event.target.value })}
          />
        </label>

        <div className="settings-pane__subagent-row">
          <label className="settings-pane__field">
            <span>Model</span>
            <SettingsPicker
              value={modelId}
              options={availableModels.map((model) => ({
                value: model.value,
                label: model.label,
                icon: model.provider.startsWith("openai:")
                  ? "simple-icons:openai"
                  : PROVIDERS.find((p) => p.value === model.provider)?.icon,
              }))}
              onSelect={(value) => updateModel(value as ModelId)}
            />
          </label>
          <label className="settings-pane__field">
            <span>Thinking</span>
            <SettingsPicker
              value={thinking}
              options={thinkingOptions.map((level) => ({
                value: level.value,
                label: settingsThinkingLabel(level, modelEntry),
              }))}
              onSelect={(value) => updateThinking(value as ThinkingLevel)}
            />
          </label>
        </div>

        <label className="settings-pane__field settings-pane__field--grow settings-pane__field--code">
          <span>Internal prompt</span>
          <textarea
            value={agent.prompt}
            onChange={(event) => onUpdate({ prompt: event.target.value })}
          />
        </label>
      </div>
    </div>
  );
}

// ---- Skills section ----------------------------------------------------

type SkillsSectionProps = {
  skills: InstalledSkill[];
  allSkills: InstalledSkill[] | null;
  loading: boolean;
  saving: boolean;
  dirty: boolean;
  error: string | null;
  status: string | null;
  filter: string;
  onFilterChange: (value: string) => void;
  selectedSkill: InstalledSkill | null;
  deleting: boolean;
  onSelectSkill: (name: string) => void;
  onRefresh: () => void;
  onSave: () => void;
  onCreate: () => void;
  onToggleSkill: (name: string) => void;
  onRevealSkill: (skill: InstalledSkill) => void;
  onDeleteSkill: (skill: InstalledSkill) => void;
  onSaveSkillContent: (skill: InstalledSkill, content: string) => Promise<boolean>;
};

function SkillsSection({
  skills,
  allSkills,
  loading,
  saving,
  dirty,
  error,
  status,
  filter,
  onFilterChange,
  selectedSkill,
  deleting,
  onSelectSkill,
  onRefresh,
  onSave,
  onCreate,
  onToggleSkill,
  onRevealSkill,
  onDeleteSkill,
  onSaveSkillContent,
}: SkillsSectionProps) {
  const total = allSkills?.length ?? 0;
  const visible = skills.length;
  const enabled = allSkills?.filter((skill) => skill.enabled).length ?? 0;

  return (
    <>
      <header className="settings-pane__header">
        <div className="settings-pane__header-text">
          <h1 className="settings-pane__title">Skills</h1>
          <p className="settings-pane__subtitle">
            {loading
              ? "Scanning…"
              : total === 0
                ? "Drop SKILL.md files in .agents/skills or ~/.agents/skills."
                : `${enabled}/${total} available to the agent`}
          </p>
        </div>
        <div className="settings-pane__actions">
          {status && (
            <span
              className="settings-pane__status"
              data-tone={
                status === "Saved" || status === "Created" || status === "Deleted"
                  ? "ok"
                  : "error"
              }
            >
              {status}
            </span>
          )}
          <button
            type="button"
            className="settings-pane__btn"
            onClick={onCreate}
            disabled={loading || saving || deleting}
          >
            <Icon icon="solar:add-circle-linear" width={13} height={13} />
            <span>Add</span>
          </button>
          <button
            type="button"
            className="settings-pane__btn"
            onClick={onRefresh}
            disabled={loading || deleting}
          >
            <Icon icon="solar:refresh-linear" width={13} height={13} />
            <span>Rescan</span>
          </button>
          <button
            type="button"
            className="settings-pane__btn"
            data-primary="true"
            onClick={onSave}
            disabled={loading || saving || deleting || !dirty}
          >
            <Icon
              icon={saving ? "solar:refresh-linear" : "solar:diskette-linear"}
              width={13}
              height={13}
            />
            <span>{saving ? "Saving…" : "Save"}</span>
          </button>
        </div>
      </header>

      <div className="settings-pane__body settings-pane__body--skills">
        <aside className="settings-pane__skill-list">
          <div className="settings-pane__search">
            <Icon icon="solar:magnifer-linear" width={13} height={13} />
            <input
              type="search"
              value={filter}
              onChange={(event) => onFilterChange(event.target.value)}
              placeholder={total ? `Search ${total} skills` : "Search skills"}
            />
          </div>

          {error && (
            <div className="settings-pane__editor-error">
              <Icon icon="solar:danger-triangle-linear" width={13} height={13} />
              <span>{error}</span>
            </div>
          )}

          <div className="settings-pane__skill-scroll">
            {skills.map((skill) => (
              <div
                key={skill.name}
                className="settings-pane__skill-item"
                data-active={selectedSkill?.name === skill.name ? "true" : "false"}
                role="button"
                tabIndex={0}
                onClick={() => onSelectSkill(skill.name)}
                onKeyDown={(event) => {
                  if (event.key !== "Enter" && event.key !== " ") return;
                  event.preventDefault();
                  onSelectSkill(skill.name);
                }}
              >
                <div className="settings-pane__skill-row">
                  <div className="settings-pane__subagent-list-head">
                    <span className="settings-pane__skill-name">{skill.name}</span>
                    <span
                      className="settings-pane__skill-source"
                      data-source={skill.source}
                    >
                      {skill.source === "workspace" ? "workspace" : "global"}
                    </span>
                    <span
                      className="settings-pane__skill-state"
                      data-enabled={skill.enabled ? "true" : "false"}
                    >
                      {skill.enabled ? "enabled" : "off"}
                    </span>
                  </div>
                  <button
                    type="button"
                    className="settings-pane__switch"
                    role="switch"
                    aria-checked={skill.enabled}
                    aria-label={`${skill.enabled ? "Disable" : "Enable"} ${skill.name}`}
                    data-on={skill.enabled ? "true" : "false"}
                    onKeyDown={(event) => {
                      event.stopPropagation();
                    }}
                    onClick={(event) => {
                      event.stopPropagation();
                      onToggleSkill(skill.name);
                    }}
                  >
                    <span className="settings-pane__switch-thumb" />
                  </button>
                </div>
                {skill.description && (
                  <span className="settings-pane__skill-desc">
                    {skill.description}
                  </span>
                )}
              </div>
            ))}
            {!loading && visible === 0 && total > 0 && (
              <div className="settings-pane__muted settings-pane__muted--center">
                No skills match.
              </div>
            )}
            {!loading && total === 0 && (
              <div className="settings-pane__empty">
                <Icon icon="solar:magic-stick-3-linear" width={22} height={22} />
                <span className="settings-pane__empty-title">No skills yet</span>
                <span className="settings-pane__empty-sub">
                  Create a folder under <code>.agents/skills/&lt;name&gt;/</code>{" "}
                  with a <code>SKILL.md</code> file.
                </span>
              </div>
            )}
          </div>
        </aside>

        <div className="settings-pane__skill-preview">
          {selectedSkill ? (
            <SkillPreview
              skill={selectedSkill}
              saving={saving}
              deleting={deleting}
              onReveal={() => onRevealSkill(selectedSkill)}
              onDelete={() => onDeleteSkill(selectedSkill)}
              onSaveContent={(content) => onSaveSkillContent(selectedSkill, content)}
            />
          ) : (
            <div className="settings-pane__empty settings-pane__empty--main">
              <Icon icon="solar:document-text-linear" width={22} height={22} />
              <span className="settings-pane__empty-title">
                {total === 0 ? "Nothing to preview" : "Select a skill"}
              </span>
            </div>
          )}
        </div>
      </div>
    </>
  );
}

function SkillPreview({
  skill,
  saving,
  deleting,
  onReveal,
  onDelete,
  onSaveContent,
}: {
  skill: InstalledSkill;
  saving: boolean;
  deleting: boolean;
  onReveal: () => void;
  onDelete: () => void;
  onSaveContent: (content: string) => Promise<boolean>;
}) {
  const body = stripFrontmatter(skill.content);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [editing, setEditing] = useState(false);
  const [draftName, setDraftName] = useState(skill.name);
  const [draftDescription, setDraftDescription] = useState(skill.description ?? "");
  const [draftBody, setDraftBody] = useState(body);
  const nameValid = draftName.trim().length > 0;

  const resetDraft = useCallback(() => {
    setDraftName(skill.name);
    setDraftDescription(skill.description ?? "");
    setDraftBody(stripFrontmatter(skill.content));
  }, [skill.content, skill.description, skill.name]);

  useEffect(() => {
    if (!confirmDelete) return;
    const id = window.setTimeout(() => setConfirmDelete(false), 3000);
    return () => window.clearTimeout(id);
  }, [confirmDelete]);

  useEffect(() => {
    setConfirmDelete(false);
    setEditing(false);
    resetDraft();
  }, [resetDraft, skill.absolutePath]);

  const saveDraft = useCallback(async () => {
    if (!nameValid || saving || deleting) return;
    const ok = await onSaveContent(
      buildSkillContent(draftName, draftDescription, draftBody),
    );
    if (ok) setEditing(false);
  }, [deleting, draftBody, draftDescription, draftName, nameValid, onSaveContent, saving]);

  return (
    <article className="settings-pane__skill-doc">
      <header className="settings-pane__skill-doc-head">
        <div className="settings-pane__skill-doc-top">
          <div className="settings-pane__skill-doc-title">
            {editing ? (
              <input
                className="settings-pane__skill-doc-title-input"
                value={draftName}
                onChange={(event) => setDraftName(event.target.value)}
                placeholder="skill-name"
                aria-invalid={!nameValid}
                spellCheck={false}
                autoFocus
              />
            ) : (
              <h2>{skill.name}</h2>
            )}
            <span
              className="settings-pane__skill-source"
              data-source={skill.source}
            >
              {skill.source === "workspace" ? "workspace" : "global"}
            </span>
          </div>
          <div className="settings-pane__skill-doc-actions">
            {editing ? (
              <>
                <button
                  type="button"
                  className="settings-pane__skill-doc-action"
                  onClick={() => {
                    resetDraft();
                    setEditing(false);
                  }}
                  disabled={saving || deleting}
                >
                  <Icon icon="solar:close-circle-linear" width={13} height={13} />
                  <span>Cancel</span>
                </button>
                <button
                  type="button"
                  className="settings-pane__skill-doc-action"
                  data-primary="true"
                  onClick={() => void saveDraft()}
                  disabled={saving || deleting || !nameValid}
                >
                  <Icon
                    icon={saving ? "solar:refresh-linear" : "solar:diskette-linear"}
                    width={13}
                    height={13}
                  />
                  <span>{saving ? "Saving…" : "Save"}</span>
                </button>
              </>
            ) : (
              <button
                type="button"
                className="settings-pane__skill-doc-action"
                onClick={() => setEditing(true)}
                disabled={saving || deleting}
                title="Edit skill content"
              >
                <Icon icon="solar:code-square-linear" width={13} height={13} />
                <span>Raw</span>
              </button>
            )}
            {!editing && (
              <>
                <button
                  type="button"
                  className="settings-pane__skill-doc-action"
                  onClick={onReveal}
                  disabled={saving || deleting}
                  title="Reveal in Finder"
                  aria-label={`Reveal ${skill.name} in Finder`}
                >
                  <Icon icon="solar:folder-open-linear" width={13} height={13} />
                  <span>Reveal in Finder</span>
                </button>
                <button
                  type="button"
                  className="settings-pane__skill-doc-action"
                  data-danger="true"
                  data-confirm={confirmDelete ? "true" : "false"}
                  disabled={saving || deleting}
                  title={confirmDelete ? "Click again to confirm" : "Delete skill"}
                  aria-label={confirmDelete ? "Confirm skill delete" : `Delete ${skill.name}`}
                  onClick={() => {
                    if (confirmDelete) {
                      onDelete();
                    } else {
                      setConfirmDelete(true);
                    }
                  }}
                >
                  <Icon
                    icon={
                      deleting
                        ? "solar:refresh-linear"
                        : "solar:trash-bin-trash-linear"
                    }
                    width={13}
                    height={13}
                  />
                  <span>
                    {deleting
                      ? "Deleting..."
                      : confirmDelete
                        ? "Confirm delete"
                        : "Delete"}
                  </span>
                </button>
              </>
            )}
          </div>
        </div>
        <code className="settings-pane__skill-path">{skill.absolutePath}</code>
        {editing ? (
          <input
            className="settings-pane__skill-doc-desc-input"
            value={draftDescription}
            onChange={(event) => setDraftDescription(event.target.value)}
            placeholder="When should the agent reach for this skill?"
            spellCheck={false}
          />
        ) : (
          skill.description && (
            <p className="settings-pane__skill-doc-desc">{skill.description}</p>
          )
        )}
      </header>
      {editing ? (
        <div className="settings-pane__skill-doc-editor">
          {!nameValid && (
            <div className="settings-pane__editor-error">
              <Icon icon="solar:danger-triangle-linear" width={13} height={13} />
              <span>Name is required.</span>
            </div>
          )}
          <div className="settings-pane__skill-doc-editor-host">
            <Editor
              value={draftBody}
              language="markdown"
              theme="sinew-cool"
              onChange={(value) => setDraftBody(value ?? "")}
              onMount={(_editor, monaco) => defineSinewCoolTheme(monaco)}
              options={{
                fontFamily:
                  '"Geist Mono", ui-monospace, "SF Mono", Menlo, monospace',
                fontSize: 12,
                lineHeight: 18,
                minimap: { enabled: false },
                scrollBeyondLastLine: false,
                smoothScrolling: true,
                renderLineHighlight: "line",
                padding: { top: 12, bottom: 12 },
                tabSize: 2,
                wordWrap: "on",
                automaticLayout: true,
                lineNumbers: "on",
                lineNumbersMinChars: 3,
                folding: true,
                scrollbar: {
                  verticalScrollbarSize: 9,
                  horizontalScrollbarSize: 9,
                },
              }}
            />
          </div>
        </div>
      ) : (
        <div className="settings-pane__skill-doc-body">
          <Markdown text={body || "_(empty SKILL.md)_"} onOpenFile={noop} />
        </div>
      )}
    </article>
  );
}

function noop(): void {}

function WrenchIcon({
  size,
  className,
}: {
  size: number;
  className?: string;
}) {
  return (
    <Wrench
      size={size}
      strokeWidth={2}
      className={className}
      aria-hidden
    />
  );
}

function stripFrontmatter(content: string): string {
  if (!content.startsWith("---")) return content.trim();
  const end = content.indexOf("\n---", 3);
  if (end === -1) return content.trim();
  return content.slice(end + 4).trim();
}

type MonacoNs = Parameters<OnMount>[1];

function defineSinewCoolTheme(monaco: MonacoNs): void {
  monaco.editor.defineTheme("sinew-cool", {
    base: "vs-dark",
    inherit: true,
    rules: [
      { token: "comment", foreground: "52555c" },
      { token: "keyword", foreground: "c4b5fd" },
      { token: "string", foreground: "86efac" },
      { token: "number", foreground: "f5a683" },
      { token: "type", foreground: "e8bb6a" },
      { token: "function", foreground: "9fc2ff" },
      { token: "variable", foreground: "e8e9ec" },
      { token: "constant", foreground: "f5a683" },
      { token: "regexp", foreground: "86efac" },
      { token: "tag", foreground: "f5a1ab" },
      { token: "attribute.name", foreground: "c4b5fd" },
    ],
    colors: {
      "editor.background": "#0b0b0d",
      "editor.foreground": "#e8e9ec",
      "editor.lineHighlightBackground": "#0f1013",
      "editorLineNumber.foreground": "#3a3d44",
      "editorLineNumber.activeForeground": "#9aa0a8",
      "editorCursor.foreground": "#3b82f6",
      "editor.selectionBackground": "#1e2b4a",
      "editor.inactiveSelectionBackground": "#141518",
      "editorIndentGuide.background1": "#141518",
      "editorIndentGuide.activeBackground1": "#23252b",
      "editorGutter.background": "#0b0b0d",
      "editorWidget.background": "#0f1013",
      "editorWidget.border": "#23252b",
      "editorHoverWidget.background": "#0f1013",
      "editorHoverWidget.border": "#23252b",
      "editorSuggestWidget.background": "#0f1013",
      "editorSuggestWidget.border": "#23252b",
      "editorSuggestWidget.selectedBackground": "#1e2b4a",
      "editorBracketMatch.background": "#1e2b4a",
      "editorBracketMatch.border": "#3b82f6",
      "scrollbarSlider.background": "#23252bcc",
      "scrollbarSlider.hoverBackground": "#2b2e35cc",
      "scrollbarSlider.activeBackground": "#3a3d44cc",
    },
  });
  monaco.editor.setTheme("sinew-cool");
}

function buildSkillContent(name: string, description: string, body: string): string {
  const normalizedBody = body.replace(/\r\n/g, "\n").trimEnd();
  return [
    "---",
    `name: ${cleanFrontmatterValue(name)}`,
    `description: ${cleanFrontmatterValue(description)}`,
    "---",
    "",
    normalizedBody,
  ].join("\n");
}

function cleanFrontmatterValue(value: string): string {
  return value.replace(/\r?\n/g, " ").trim();
}

function createSubAgent(
  index: number,
  availableModels: readonly ModelEntry[] = MODELS,
): SubAgentConfig {
  const id =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : `agent-${Date.now()}-${index}`;
  const model = availableModels[0] ?? MODELS[0];
  return {
    id,
    name: `Sub-agent ${index}`,
    description: "Use this agent for focused research or implementation tasks.",
    prompt: "",
    model: modelRefWithThinking(modelRefFromId(model.value), model.defaultThinking),
    enabled: true,
  };
}

function normalizeSubAgentSettings(settings: SubAgentSettings): SubAgentSettings {
  return {
    agents: (settings.agents ?? []).map((agent, index) => ({
      id: agent.id || `agent-${index + 1}`,
      name: agent.name || `Sub-agent ${index + 1}`,
      description: agent.description ?? "",
      prompt: agent.prompt ?? "",
      model:
        agent.model ??
        modelRefWithThinking(modelRefFromId(MODELS[0].value), MODELS[0].defaultThinking),
      enabled: agent.enabled !== false,
    })),
  };
}

function subAgentSettingsFingerprint(settings: SubAgentSettings): string {
  return JSON.stringify(normalizeSubAgentSettings(settings));
}

function settingsFromSkills(skills: InstalledSkill[]): SkillSettings {
  return {
    skills: skills.map((skill) => ({
      name: skill.name,
      enabled: skill.enabled,
    })),
  };
}

function skillsFingerprint(skills: InstalledSkill[]): string {
  return JSON.stringify(settingsFromSkills(skills));
}

function normalizeToolSettings(settings: ToolSettings): ToolSettings {
  const seen = new Set<string>();
  const imageProvider =
    settings.imageProvider === "nanoBanana2" ? "nanoBanana2" : "gptImage2";
  const defaultPlanModePrompt = settings.defaultPlanModePrompt ?? "";
  const planModePrompt = settings.planModePrompt ?? defaultPlanModePrompt;
  return {
    imageProvider,
    openaiImageUseSubscription: settings.openaiImageUseSubscription === true,
    geminiImageUseSubscription: settings.geminiImageUseSubscription === true,
    openaiImageModel: settings.openaiImageModel ?? "gpt-image-2",
    geminiImageModel: settings.geminiImageModel ?? "gemini-3.1-flash-image-preview",
    planModePrompt,
    defaultPlanModePrompt,
    openaiImageApiKey: settings.openaiImageApiKey ?? "",
    nanoBananaApiKey: settings.nanoBananaApiKey ?? "",
    webSearchProvider:
      settings.webSearchProvider === "linkup" ? "linkup" : "classic",
    linkupApiKey: settings.linkupApiKey ?? "",
    tools: (settings.tools ?? []).flatMap((tool) => {
      const name = canonicalToolName(tool.name?.trim() ?? "");
      if (!name || seen.has(name)) return [];
      seen.add(name);
      const defaultDescription = tool.defaultDescription ?? tool.description ?? "";
      return [
        {
          name,
          displayName: tool.displayName?.trim() || undefined,
          description: tool.description ?? defaultDescription,
          defaultDescription,
          enabled: tool.enabled !== false,
        },
      ];
    }),
  };
}

function toolSettingsFingerprint(settings: ToolSettings): string {
  return JSON.stringify(normalizeToolSettings(settings));
}

// ---- JSON parsing helpers (unchanged) ----------------------------------

function parseMcpJson(source: string): McpSettings {
  const trimmed = source.trim();
  if (!trimmed) return EMPTY_SETTINGS;

  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch (err) {
    throw new Error(err instanceof Error ? err.message : "Invalid JSON");
  }

  return normalizeSettings(settingsFromUnknown(parsed));
}

function settingsFromUnknown(value: unknown): McpSettings {
  if (Array.isArray(value)) {
    return {
      servers: value.map((item, index) => serverFromUnknown(item, `server-${index + 1}`)),
    };
  }

  if (!isRecord(value)) {
    throw new Error("JSON must be an object or an array");
  }

  if (isRecord(value.mcpServers)) {
    return {
      servers: Object.entries(value.mcpServers).map(([name, config]) =>
        serverFromUnknown(config, name),
      ),
    };
  }

  if (Array.isArray(value.servers)) {
    return {
      servers: value.servers.map((item, index) =>
        serverFromUnknown(item, `server-${index + 1}`),
      ),
    };
  }

  throw new Error('Use {"mcpServers": {...}} or {"servers": [...]}');
}

function serverFromUnknown(value: unknown, fallbackName: string): McpServerConfig {
  if (!isRecord(value)) {
    throw new Error(`Invalid MCP config for ${fallbackName}`);
  }

  const name = stringValue(value.name) || fallbackName;
  const command = stringValue(value.command);
  if (!command) throw new Error(`Missing command for ${name}`);

  return {
    id: stringValue(value.id) || deterministicId(name),
    name,
    command,
    args: arrayOfStrings(value.args),
    env: envFromUnknown(value.env),
    cwd: stringValue(value.cwd) || null,
    enabled: value.enabled === false || value.disabled === true ? false : true,
    autoLoad: value.autoLoad === true || value.auto_load === true,
  };
}

function normalizeSettings(settings: McpSettings): McpSettings {
  const seen = new Map<string, number>();
  return {
    servers: (settings.servers ?? []).map((server, index) => {
      const name = server.name || `server-${index + 1}`;
      const baseId = server.id || deterministicId(name);
      const count = seen.get(baseId) ?? 0;
      seen.set(baseId, count + 1);
      const id = count ? `${baseId}-${count + 1}` : baseId;
      return {
        id,
        name,
        command: server.command ?? "",
        args: server.args ?? [],
        env: server.env ?? [],
        cwd: server.cwd ?? null,
        enabled: server.enabled ?? true,
        autoLoad: server.autoLoad ?? false,
      };
    }),
  };
}

function settingsToJson(settings: McpSettings): string {
  const mcpServers: Record<string, unknown> = {};
  for (const server of settings.servers) {
    const entry: Record<string, unknown> = {
      command: server.command,
    };
    if (server.args.length) entry.args = server.args;
    if (server.cwd) entry.cwd = server.cwd;
    if (server.env.length) entry.env = envToObject(server.env);
    if (!server.enabled) entry.disabled = true;
    if (server.autoLoad) entry.autoLoad = true;
    mcpServers[server.name || server.id] = entry;
  }

  return `${JSON.stringify({ mcpServers }, null, 2)}\n`;
}

function deterministicId(name: string): string {
  return `mcp_${slug(name) || "server"}`;
}

function slug(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
}

function stringValue(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function arrayOfStrings(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

function envFromUnknown(value: unknown): McpEnvVar[] {
  if (!value) return [];
  if (Array.isArray(value)) {
    return value
      .filter(isRecord)
      .map((item) => ({ key: stringValue(item.key), value: stringValue(item.value) }))
      .filter((item) => item.key);
  }
  if (isRecord(value)) {
    return Object.entries(value).map(([key, item]) => ({
      key,
      value: typeof item === "string" ? item : JSON.stringify(item),
    }));
  }
  return [];
}

function envToObject(env: McpEnvVar[]): Record<string, string> {
  return Object.fromEntries(env.map((item) => [item.key, item.value]));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function ApiKeyField({
  label,
  value,
  placeholder,
  onChange,
}: {
  label: string;
  value: string;
  placeholder?: string;
  onChange: (value: string) => void;
}) {
  const [revealed, setRevealed] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const id = window.setTimeout(() => setCopied(false), 1400);
    return () => window.clearTimeout(id);
  }, [copied]);

  const handleCopy = useCallback(async () => {
    if (!value) return;
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
    } catch {
      // clipboard unavailable; ignore silently
    }
  }, [value]);

  return (
    <label className="settings-pane__tool-credential">
      <span className="settings-pane__tool-credential-label">{label}</span>
      <div className="settings-pane__tool-credential-field">
        <input
          type={revealed ? "text" : "password"}
          value={value}
          placeholder={placeholder}
          autoComplete="off"
          spellCheck={false}
          onChange={(event) => onChange(event.target.value)}
        />
        <div className="settings-pane__tool-credential-actions">
          <button
            type="button"
            className="settings-pane__icon-btn"
            onClick={() => setRevealed((v) => !v)}
            title={revealed ? "Hide key" : "Show key"}
            aria-label={revealed ? "Hide key" : "Show key"}
            disabled={!value}
          >
            <Icon
              icon={revealed ? "solar:eye-closed-linear" : "solar:eye-linear"}
              width={13}
              height={13}
            />
          </button>
          <button
            type="button"
            className="settings-pane__icon-btn"
            onClick={handleCopy}
            title={copied ? "Copied" : "Copy key"}
            aria-label={copied ? "Copied" : "Copy key"}
            disabled={!value}
          >
            <Icon
              icon={
                copied
                  ? "solar:check-read-linear"
                  : "solar:copy-linear"
              }
              width={13}
              height={13}
            />
          </button>
        </div>
      </div>
    </label>
  );
}

function TerminalGlyph() {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <rect x="1.5" y="2.5" width="11" height="9" rx="1.6" />
      <path d="M4 6l1.6 1.3L4 8.6" />
      <path d="M7.3 8.8h3" />
    </svg>
  );
}

function AsteriskGlyph() {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.45"
      strokeLinecap="round"
      aria-hidden
    >
      <path d="M7 2.4v9.2" />
      <path d="M3 4.7l8 4.6" />
      <path d="M11 4.7 3 9.3" />
    </svg>
  );
}

function SwarmGlyph() {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <circle cx="5" cy="5" r="2" />
      <circle cx="12" cy="5" r="2" />
      <circle cx="19" cy="5" r="2" />
      <circle cx="12" cy="19" r="2" />
      <path d="M5 7 L5 12 L19 12 L19 7" />
      <path d="M12 7 L12 17" />
    </svg>
  );
}

function McpGlyph() {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 180 180"
      fill="none"
      aria-hidden
    >
      <path
        d="M18 84.8528L85.8822 16.9706C95.2548 7.59798 110.451 7.59798 119.823 16.9706C129.196 26.3431 129.196 41.5391 119.823 50.9117L68.5581 102.177"
        stroke="currentColor"
        strokeWidth="12"
        strokeLinecap="round"
      />
      <path
        d="M69.2652 101.47L119.823 50.9117C129.196 41.5391 144.392 41.5391 153.765 50.9117L154.118 51.2652C163.491 60.6378 163.491 75.8338 154.118 85.2063L92.7248 146.6C89.6006 149.724 89.6006 154.789 92.7248 157.913L105.331 170.52"
        stroke="currentColor"
        strokeWidth="12"
        strokeLinecap="round"
      />
      <path
        d="M102.853 33.9411L52.6482 84.1457C43.2756 93.5183 43.2756 108.714 52.6482 118.087C62.0208 127.459 77.2167 127.459 86.5893 118.087L136.794 67.8822"
        stroke="currentColor"
        strokeWidth="12"
        strokeLinecap="round"
      />
    </svg>
  );
}

function SkillGlyph() {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.45"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d="M4 2.5v4.2c0 1.5 1.2 2.7 2.7 2.7H11" />
      <path d="M8.8 7.2 11 9.4l-2.2 2.2" />
    </svg>
  );
}

function BroomGlyph() {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d="M11.5 2.5 L6.5 7.5" />
      <path d="M4.5 6.5 L7.5 9.5" />
      <path d="M3.8 7.6 L2 12" />
      <path d="M5 8.7 L4.2 12.4" />
      <path d="M6.2 9.8 L6.4 12.5" />
      <path d="M7.3 10.5 L8.6 12.2" />
    </svg>
  );
}

function ToolGlyph({ name }: { name: string }) {
  const canonicalName = canonicalToolName(name);
  if (canonicalName === "bash" || canonicalName === "bash_input") {
    return <TerminalGlyph />;
  }
  if (canonicalName === "glob" || canonicalName === "grep") {
    return <AsteriskGlyph />;
  }
  if (canonicalName === "team_run" || canonicalName === "team_status" || canonicalName === "team_stop") {
    return <SwarmGlyph />;
  }
  if (canonicalName === "load_mcp_tool") {
    return <McpGlyph />;
  }
  if (canonicalName === "skill") {
    return <SkillGlyph />;
  }
  if (canonicalName === "clean_context") {
    return <BroomGlyph />;
  }
  const icon = TOOL_ICON[canonicalName] ?? "solar:tuning-2-linear";
  return <Icon icon={icon} width={13} height={13} />;
}

const TOOL_LABEL: Record<string, string> = {
  bash: "Shell",
  bash_input: "Shell input",
  read: "Read",
  edit_file: "Edit file",
  write_file: "Write file",
  glob: "Glob",
  grep: "Grep",
  check_sota: "Check SOTA status",
  web_search: "Web search",
  web_fetch: "Web fetch",
  create_image: "Create image",
  question: "Question",
  todo_list: "To-do list",
  load_mcp_tool: "Load MCP tool",
  skill: "Load skill",
  team_run: "Team run",
  team_status: "Team status",
  team_stop: "Team stop",
  send_message: "Send message",
  clean_context: "Clean context",
  update_goal: "Update goal",
  context_compaction: "Compact context",
};

const TOOL_ICON: Record<string, string> = {
  read: "solar:document-text-linear",
  edit_file: "solar:pen-2-linear",
  write_file: "solar:file-text-linear",
  web_search: "solar:magnifer-linear",
  check_sota: "solar:health-state-linear",
  web_fetch: "solar:link-round-linear",
  create_image: "solar:gallery-wide-linear",
  question: "solar:question-circle-linear",
  todo_list: "solar:checklist-linear",
  send_message: "solar:chat-round-dots-linear",
  update_goal: "solar:flag-2-linear",
  context_compaction: "solar:archive-linear",
};

function labelForTool(tool: ToolConfig | string): string {
  if (typeof tool !== "string") {
    const displayName = tool.displayName?.trim();
    if (displayName) return displayName;
    return TOOL_LABEL[canonicalToolName(tool.name)] ?? humanizeToolName(tool.name);
  }
  const name = tool;
  return TOOL_LABEL[canonicalToolName(name)] ?? humanizeToolName(name);
}

function humanizeToolName(name: string): string {
  const spaced = name
    .replace(/[_-]+/g, " ")
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/\s+/g, " ")
    .trim();
  if (!spaced) return name;
  return spaced.charAt(0).toUpperCase() + spaced.slice(1).toLowerCase();
}

type DeepSeekProviderCardProps = {
  status: DeepSeekProviderStatus | null;
  loading: boolean;
  busy: boolean;
  onDisconnect: () => void;
  onStatusChange: (status: DeepSeekProviderStatus) => void;
  onChanged: () => void;
};

function DeepSeekProviderCard({
  status,
  loading,
  busy,
  onDisconnect,
  onStatusChange,
  onChanged,
}: DeepSeekProviderCardProps) {
  const [apiKey, setApiKey] = useState("");
  const [revealed, setRevealed] = useState(false);
  const [validating, setValidating] = useState(false);
  const [validationError, setValidationError] = useState<string | null>(null);
  const [quota, setQuota] = useState<QuotaInfo | null>(null);
  const validationSeq = useRef(0);

  const displayStatus: DeepSeekProviderStatus = validating
    ? {
        connected: false,
        connectionState: "connecting",
      }
    : status ?? {
        connected: false,
        connectionState: "disconnected",
      };
  const state = displayStatus.connectionState;
  const connected = Boolean(displayStatus.connected);

  useEffect(() => {
    if (!connected) {
      setQuota(null);
      return;
    }
    let active = true;
    const update = async () => {
      try {
        const q = await fetchProviderQuota("deepseek");
        if (active) setQuota(q);
      } catch (err) {
        console.error("Failed to fetch DeepSeek quota:", err);
      }
    };
    update();
    const handleUpdate = () => {
      const cached = getCachedQuota("deepseek");
      if (cached && active) {
        setQuota(cached);
      } else {
        update();
      }
    };
    window.addEventListener("sinew:quota-updated", handleUpdate);
    return () => {
      active = false;
      window.removeEventListener("sinew:quota-updated", handleUpdate);
    };
  }, [connected]);

  const connecting = state === "connecting";
  const error = validationError ?? (state === "error" ? displayStatus.error : null);

  const isQuotaExhausted = connected && quota && quota.kind !== "unavailable" && (
    quota.kind === "credits"
      ? (quota.creditRemaining != null && quota.creditRemaining <= 0)
      : (quota.kind === "groups" ? quota.groups ?? [] : quota.windows ?? []).some(
          (w) => w.remainingPercent !== null && w.remainingPercent <= 0
        )
  );

  const statusLabel = connecting
    ? "Connecting"
    : isQuotaExhausted
      ? "Limit reached"
      : connected
        ? "Connected"
        : state === "error"
          ? "Needs attention"
          : "Not connected";

  const statusTone = connecting
    ? "pending"
    : isQuotaExhausted
      ? "error"
      : connected
        ? "ok"
        : state === "error"
          ? "error"
          : "off";

  useEffect(() => {
    const key = apiKey.trim();
    validationSeq.current += 1;
    const seq = validationSeq.current;
    setValidationError(null);
    if (!key) {
      setValidating(false);
      return;
    }
    setValidating(true);
    const timer = window.setTimeout(() => {
      void (async () => {
        try {
          const next = await api.validateDeepSeekApiKey(key);
          if (validationSeq.current !== seq) return;
          onStatusChange(next);
          setApiKey("");
          setValidationError(null);
          onChanged();
        } catch (err) {
          if (validationSeq.current !== seq) return;
          const message = err instanceof Error ? err.message : String(err);
          setValidationError(message);
          onStatusChange({
            connected: false,
            connectionState: "error",
            error: message,
          });
        } finally {
          if (validationSeq.current === seq) setValidating(false);
        }
      })();
    }, 650);
    return () => window.clearTimeout(timer);
  }, [apiKey, onChanged, onStatusChange]);

  return (
    <section className="settings-pane__provider-card">
      <div className="settings-pane__provider-main">
        <div className="settings-pane__provider-mark" aria-hidden>
          <Icon icon="simple-icons:deepseek" width={24} height={24} />
        </div>
        <div className="settings-pane__provider-copy">
          <div className="settings-pane__provider-title-row">
            <h2>DeepSeek</h2>
            <span className="settings-pane__chip" data-tone={statusTone}>
              <span className="settings-pane__chip-dot" />
              {statusLabel}
            </span>
          </div>
          <p>Use DeepSeek models (V3 & R1) with your own API key.</p>
          {error && <div className="settings-pane__provider-error">{error}</div>}
          {connected && quota && quota.kind !== "unavailable" && (
            <div className="settings-pane__provider-meta" style={{ marginTop: "8px", alignItems: "center" }}>
              {quota.kind === "credits" ? (
                <>
                  <QuotaBar inline item={{ label: quota.creditLimit == null ? "Limite" : `Limite $${quota.creditLimit.toFixed(2)}`, remainingPercent: 100 }} />
                  <QuotaBar inline item={{ label: quota.creditRemaining == null ? "Restant" : `Restant $${quota.creditRemaining.toFixed(2)}`, remainingPercent: quota.percentage }} />
                </>
              ) : (
                (quota.kind === "groups" ? quota.groups ?? [] : quota.windows ?? []).map((item) => (
                  <QuotaBar inline key={item.label} item={item} />
                ))
              )}
            </div>
          )}
          {connected && quota && quota.kind === "unavailable" && (
            <div style={{ marginTop: "8px", color: "var(--text-3)", fontSize: "11px", opacity: 0.7 }}>
              {quota.label ?? "Quota non disponible"}
            </div>
          )}
          {connected && displayStatus.models && displayStatus.models.length > 0 && (
            <div style={{ fontSize: "11px", opacity: 0.9, color: "var(--color-success)", marginTop: "10px" }}>
              <strong>✓ Modèles vérifiés sur votre clé :</strong>{" "}
              {displayStatus.models.map(id => id === "deepseek-chat" ? "DeepSeek V3" : id === "deepseek-reasoner" ? "DeepSeek R1" : id).join(", ")}
            </div>
          )}
        </div>
      </div>

      <div className="settings-pane__provider-detail">
        <label className="settings-pane__tool-credential">
          <span className="settings-pane__tool-credential-label">API key</span>
          <div className="settings-pane__tool-credential-field">
            <input
              type={revealed ? "text" : "password"}
              value={apiKey}
              placeholder={connected ? displayStatus.keyPreview ?? "Key saved" : "sk-..."}
              onChange={(event) => setApiKey(event.target.value)}
              autoComplete="off"
              spellCheck={false}
            />
            <div className="settings-pane__tool-credential-actions">
              <button
                type="button"
                className="settings-pane__icon-btn"
                onClick={() => setRevealed((value) => !value)}
                title={revealed ? "Hide key" : "Show key"}
                aria-label={revealed ? "Hide key" : "Show key"}
              >
                <Icon
                  icon={revealed ? "solar:eye-closed-linear" : "solar:eye-linear"}
                  width={13}
                  height={13}
                />
              </button>
              {connected && (
                <button
                  type="button"
                  className="settings-pane__icon-btn"
                  onClick={onDisconnect}
                  disabled={busy}
                  title="Remove API key"
                  aria-label="Remove API key"
                >
                  <Icon icon="solar:trash-bin-trash-linear" width={13} height={13} />
                </button>
              )}
            </div>
          </div>
        </label>
      </div>
    </section>
  );
}
