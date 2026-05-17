import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Editor, { type OnMount } from "@monaco-editor/react";
import { Icon } from "@iconify/react";
import { Languages, Wrench } from "lucide-react";
import { api } from "../lib/ipc";
import { setLanguage, useLanguage, type AppLanguage } from "../lib/i18n";
import { replayOnboarding } from "../lib/onboarding";
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
  GoogleProviderStatus,
  ImageProvider,
  InstalledSkill,
  KimiProviderStatus,
  McpEnvVar,
  McpServerConfig,
  McpServerProbe,
  McpSettings,
  OpenAiProviderStatus,
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
  openaiImageApiKey: "",
  nanoBananaApiKey: "",
  webSearchProvider: "classic",
  linkupApiKey: "",
};
const PROVIDERS_CHANGED_EVENT = "sinew:providers-changed";
const TOOL_SETTINGS_CHANGED_EVENT = "sinew:tool-settings-changed";

type Section = "about" | "providers" | "tools" | "mcp" | "skills" | "subagents";

type Props = {
  workspacePath: string;
  initialSection?: Section;
};

export function SettingsPane({ workspacePath, initialSection = "about" }: Props) {
  const language = useLanguage();
  const copy = settingsPaneCopy[language];
  const [section, setSection] = useState<Section>(initialSection);
  const [settings, setSettings] = useState<McpSettings>(EMPTY_SETTINGS);
  const [savedJson, setSavedJson] = useState("");
  const [jsonText, setJsonText] = useState("");
  const [selectedServerId, setSelectedServerId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [parseError, setParseError] = useState<string | null>(null);
  const [probes, setProbes] = useState<McpServerProbe[]>([]);

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
  const [anthropicStatus, setAnthropicStatus] = useState<AnthropicProviderStatus | null>(null);
  const [googleStatus, setGoogleStatus] = useState<GoogleProviderStatus | null>(null);
  const [kimiStatus, setKimiStatus] = useState<KimiProviderStatus | null>(null);
  const [openRouterStatus, setOpenRouterStatus] = useState<OpenRouterProviderStatus | null>(null);
  const [openRouterModels, setOpenRouterModels] = useState<OpenRouterModel[]>([]);
  const [providersLoading, setProvidersLoading] = useState(false);
  const [providersBusy, setProvidersBusy] = useState(false);
  const [providersMessage, setProvidersMessage] = useState<string | null>(null);
  const [configuredProviders, setConfiguredProviders] = useState<string[]>([]);

  useEffect(() => {
    setSection(initialSection);
  }, [initialSection]);

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
          const nextProbes = await api.probeMcpTools();
          if (disposed) return;
          setProbes(nextProbes);
          const failures = nextProbes.filter(
            (probe) => probe.enabled && !probe.ok,
          ).length;
          if (failures) {
            setStatus(`${failures} server${failures === 1 ? "" : "s"} failed`);
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

  useEffect(() => {
    if (section !== "providers" && section !== "tools") return;
    if (openAiStatus === null) void loadOpenAiStatus();
    if (section !== "providers") return;
    if (anthropicStatus === null) void loadAnthropicStatus();
    if (googleStatus === null) void loadGoogleStatus();
    if (kimiStatus === null) void loadKimiStatus();
    if (openRouterStatus === null) void loadOpenRouterStatus();
  }, [
    section,
    openAiStatus,
    anthropicStatus,
    googleStatus,
    kimiStatus,
    openRouterStatus,
    loadOpenAiStatus,
    loadAnthropicStatus,
    loadGoogleStatus,
    loadKimiStatus,
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
    const openAiConnecting = openAiStatus?.connectionState === "connecting";
    const anthropicConnecting = anthropicStatus?.connectionState === "connecting";
    const googleConnecting = googleStatus?.connectionState === "connecting";
    const kimiConnecting = kimiStatus?.connectionState === "connecting";
    if (!openAiConnecting && !anthropicConnecting && !googleConnecting && !kimiConnecting) return;
    const timer = window.setInterval(() => {
      if (openAiConnecting) void loadOpenAiStatus();
      if (anthropicConnecting) void loadAnthropicStatus();
      if (googleConnecting) void loadGoogleStatus();
      if (kimiConnecting) void loadKimiStatus();
    }, 1200);
    return () => window.clearInterval(timer);
  }, [
    openAiStatus?.connectionState,
    anthropicStatus?.connectionState,
    googleStatus?.connectionState,
    kimiStatus?.connectionState,
    loadOpenAiStatus,
    loadAnthropicStatus,
    loadGoogleStatus,
    loadKimiStatus,
  ]);

  const connectOpenAi = useCallback(async () => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      const login = await api.startOpenAiOAuthLogin();
      const connecting: OpenAiProviderStatus = {
        connected: false,
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
  }, [loadOpenAiStatus]);

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
      void loadConfiguredProviders();
      window.dispatchEvent(new CustomEvent(PROVIDERS_CHANGED_EVENT));
    } catch (err) {
      setProvidersMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setProvidersBusy(false);
    }
  }, [loadConfiguredProviders]);

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

  const connectGoogle = useCallback(async () => {
    setProvidersBusy(true);
    setProvidersMessage(null);
    try {
      const login = await api.startGoogleOAuthLogin();
      const connecting: GoogleProviderStatus = {
        connected: false,
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
  }, [loadGoogleStatus]);

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
    (id: string) => {
      if (parseError) return;
      const next: McpSettings = {
        servers: settings.servers.map((server) =>
          server.id === id ? { ...server, enabled: !server.enabled } : server,
        ),
      };
      setJsonText(settingsToJson(next));
    },
    [parseError, settings],
  );

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
        "editor.background": "#08090b",
        "editor.foreground": "#e8e9ec",
        "editor.lineHighlightBackground": "#0f1013",
        "editorLineNumber.foreground": "#3a3d44",
        "editorLineNumber.activeForeground": "#9aa0a8",
        "editorCursor.foreground": "#3b82f6",
        "editor.selectionBackground": "#1e2b4a",
        "editor.inactiveSelectionBackground": "#141518",
        "editorIndentGuide.background1": "#141518",
        "editorIndentGuide.activeBackground1": "#23252b",
        "editorGutter.background": "#08090b",
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
      <nav className="settings-pane__nav" aria-label={copy.sectionsLabel}>
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
          <span className="settings-pane__nav-label">{copy.about}</span>
          <span className="settings-pane__nav-count" />
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
          <span className="settings-pane__nav-label">{copy.providers}</span>
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
          <span className="settings-pane__nav-label">{copy.tools}</span>
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
          <span className="settings-pane__nav-label">{copy.skills}</span>
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
          <span className="settings-pane__nav-label">{copy.agents}</span>
          <span className="settings-pane__nav-count">
            {subAgentSettings.agents.length}
          </span>
        </button>
      </nav>

      <section className="settings-pane__main">
        {section === "about" ? (
          <AboutSection />
        ) : section === "providers" ? (
          <ProvidersSection
            openAiStatus={openAiStatus}
            anthropicStatus={anthropicStatus}
            googleStatus={googleStatus}
            kimiStatus={kimiStatus}
            openRouterStatus={openRouterStatus}
            openRouterModels={openRouterModels}
            loading={providersLoading}
            busy={providersBusy}
            message={providersMessage}
            onRefresh={() => {
              void loadOpenAiStatus();
              void loadAnthropicStatus();
              void loadGoogleStatus();
              void loadKimiStatus();
              void loadOpenRouterStatus();
            }}
            onConnect={() => void connectOpenAi()}
            onCancel={() => void cancelOpenAi()}
            onDisconnect={() => void disconnectOpenAi()}
            onConnectAnthropic={() => void connectAnthropic()}
            onCancelAnthropic={() => void cancelAnthropic()}
            onDisconnectAnthropic={() => void disconnectAnthropic()}
            onConnectGoogle={() => void connectGoogle()}
            onCancelGoogle={() => void cancelGoogle()}
            onDisconnectGoogle={() => void disconnectGoogle()}
            onConnectKimi={() => void connectKimi()}
            onCancelKimi={() => void cancelKimi()}
            onDisconnectKimi={() => void disconnectKimi()}
            onDisconnectOpenRouter={() => void disconnectOpenRouter()}
            onOpenRouterStatusChange={setOpenRouterStatus}
            onOpenRouterModelsChange={setOpenRouterModels}
            onOpenRouterChanged={handleOpenRouterChanged}
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
            onOpenAiImageApiKeyChange={updateOpenAiImageApiKey}
            onNanoBananaApiKeyChange={updateNanoBananaApiKey}
            onWebSearchProviderChange={updateWebSearchProvider}
            onLinkupApiKeyChange={updateLinkupApiKey}
            openAiStatus={openAiStatus}
          />
        ) : section === "mcp" ? (
          <McpSection
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
            onSelectServer={setSelectedServerId}
            onClearSelection={() => setSelectedServerId(null)}
            selectedServer={selectedServer}
            selectedProbe={selectedProbe}
            onToggleEnabled={toggleEnabled}
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
            onToggleSkill={toggleSkillEnabled}
            onRevealSkill={(skill) => void revealSkill(skill)}
            onDeleteSkill={(skill) => void deleteSkill(skill)}
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

const settingsPaneCopy = {
  en: {
    sectionsLabel: "Settings sections",
    about: "About",
    providers: "Providers",
    tools: "Tools",
    skills: "Skills",
    agents: "Agents",
  },
  fr: {
    sectionsLabel: "Sections des paramètres",
    about: "À propos",
    providers: "Modèles",
    tools: "Outils",
    skills: "Compétences",
    agents: "Agents",
  },
} as const;
// ---- About section -----------------------------------------------------

function AboutSection() {
  const language = useLanguage();
  const copy = aboutCopy[language];
  const nextLanguage = language === "fr" ? "en" : "fr";

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

      <p className="settings-pane__about-line">{copy.description}</p>
      <p className="settings-pane__about-line">{copy.capabilities}</p>

      <div className="settings-pane__about-actions">
        <button
          type="button"
          className="settings-pane__about-action"
          onClick={() => setLanguage(nextLanguage)}
        >
          <Languages size={13} aria-hidden />
          <span>{copy.languageAction}</span>
        </button>
        <button
          type="button"
          className="settings-pane__about-action"
          onClick={replayOnboarding}
        >
          <Icon icon="solar:play-circle-linear" width={13} height={13} />
          <span>{copy.replayOnboarding}</span>
        </button>
      </div>

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
    </div>
  );
}

const aboutCopy = {
  en: {
    description:
      "Sinew is a flexible AI coding harness. You shape it: tweak the description of every tool, turn the ones you don't need off, and the assistant only sees what you keep.",
    capabilities:
      "Run it minimal with a couple of tools, or unlock the full set: shell, search, MCP, web, images, sub-agents. Multi-provider by default.",
    languageAction: "Passer en français",
    replayOnboarding: "Replay onboarding",
  },
  fr: {
    description:
      "Sinew est un environnement de code assisté par IA. Vous le façonnez: ajustez les outils, désactivez ceux dont vous n'avez pas besoin, et l'assistant ne voit que ce que vous gardez.",
    capabilities:
      "Utilisez-le simplement avec quelques outils, ou activez l'ensemble complet: shell, recherche, MCP, web, images et sous-agents. Multi-fournisseur par défaut.",
    languageAction: "Passer en anglais",
    replayOnboarding: "Rejouer l'onboarding",
  },
} as const;
const providersCopy = {
  en: {
    title: "Providers",
    subtitle: "Connect model providers for Sinew.",
    refresh: "Refresh",
    refreshing: "Refreshing...",
    signedIn: "Signed in",
    project: "Project",
    anthropicDescription: "Use OAuth to connect your Claude account for Anthropic models.",
    openAiDescription: "Use OAuth to connect your ChatGPT account for OpenAI models.",
    googleDescription: "Use OAuth to connect your Google account for Gemini models.",
    kimiDescription: "Use OAuth to connect your Kimi account for Kimi 2.6.",
  },
  fr: {
    title: "Modèles",
    subtitle: "Connectez les fournisseurs de modèles pour Sinew.",
    refresh: "Actualiser",
    refreshing: "Actualisation...",
    signedIn: "Connecté",
    project: "Projet",
    anthropicDescription: "Connectez votre compte Claude avec OAuth pour les modèles Anthropic.",
    openAiDescription: "Connectez votre compte ChatGPT avec OAuth pour les modèles OpenAI.",
    googleDescription: "Connectez votre compte Google avec OAuth pour les modèles Gemini.",
    kimiDescription: "Connectez votre compte Kimi avec OAuth pour Kimi 2.6.",
  },
} as const;

const providerCardCopy = {
  en: {
    connecting: "Connecting",
    connected: "Connected",
    needsAttention: "Needs attention",
    notConnected: "Not connected",
    cancel: "Cancel",
    disconnecting: "Disconnecting...",
    disconnect: "Disconnect",
    opening: "Opening...",
    connect: "Connect",
  },
  fr: {
    connecting: "Connexion",
    connected: "Connecté",
    needsAttention: "À vérifier",
    notConnected: "Non connecté",
    cancel: "Annuler",
    disconnecting: "Déconnexion...",
    disconnect: "Déconnecter",
    opening: "Ouverture...",
    connect: "Connecter",
  },
} as const;

const settingsDetailCopy = {
  en: {
    openRouter: {
      description: "Use any OpenRouter model with your own API key.",
      keySaved: "Key saved",
      hideKey: "Hide key",
      showKey: "Show key",
      removeKey: "Remove API key",
      search: "Search",
      typeModel: "Type a model name...",
      saveKeyFirst: "Save a valid key first",
      searching: "Searching...",
      noMatchingModel: "No matching model.",
      added: "Added",
      adding: "Adding...",
      add: "Add",
      removeModel: "Remove model",
    },
    tools: {
      title: "Tools",
      loading: "Loading...",
      enabled: "enabled",
      save: "Save",
      saving: "Saving...",
      planPromptTitle: "Plan mode prompt",
      default: "Default",
      custom: "Custom",
      imageGeneration: "Image generation",
      imageProvider: "Image provider",
      webSearch: "Web search",
      webSearchProvider: "Web search provider",
      openAiSubscription: "Use OpenAI subscription",
      openAiSubscriptionConnected: "Authenticate image requests with your connected OpenAI account instead of an API key.",
      openAiSubscriptionDisconnected: "Connect OpenAI in Settings -> Providers to use your subscription.",
      disableOpenAiSubscription: "Disable OpenAI subscription mode",
      enableOpenAiSubscription: "Enable OpenAI subscription mode",
      mainAgent: "Main Agent",
      swarmAgents: "Swarm Agents",
      noTools: "No tools",
      promptInjected: "Prompt injected into Plan mode",
      resetPlanPrompt: "Reset Plan mode prompt",
      resetPrompt: "Reset prompt",
      planPromptHelp: "This text is appended to the system prompt only when the conversation is in Plan mode.",
      planInstructions: "Plan mode instructions...",
      resetDescription: "Reset description",
      disable: "Disable",
      enable: "Enable",
      description: "description",
    },
    mcp: {
      title: "MCP servers",
      subtitle: "Add a server in the JSON config to extend the agent.",
      checking: "Checking...",
      saveProbe: "Save & probe",
      servers: "Servers",
      probing: "probing...",
      rawConfig: "Raw config",
      unsaved: "Unsaved",
      synced: "Synced",
      untitled: "Untitled",
      noServers: "No servers yet - add one in the raw config.",
      disabled: "disabled",
      pending: "pending",
      failed: "failed",
      error: "error",
      tools: "Tools",
      tool: "tool",
      noDescription: "No description provided.",
      noTools: "Server returned no tools.",
      probingServer: "Probing server...",
      noProbe: "No probe data yet.",
      disable: "Disable",
      enable: "Enable",
    },
    agents: {
      title: "Sub-agents",
      emptySubtitle: "Create focused agents the main agent can call as tools.",
      available: "available to the main agent",
      save: "Save",
      saving: "Saving...",
      agents: "Agents",
      newAgent: "New agent",
      untitled: "Untitled",
      noAgents: "No sub-agents yet - click + to start.",
      selectOrCreate: "Select or create an agent",
      untitledAgent: "Untitled agent",
      agentName: "Agent name",
      clickConfirm: "Click again to confirm",
      deleteAgent: "Delete agent",
      confirmDelete: "Confirm delete",
      deleteConfirmShort: "Delete?",
      disable: "Disable",
      enable: "Enable",
      description: "Description seen by the main agent",
      model: "Model",
      thinking: "Thinking",
      internalPrompt: "Internal prompt",
    },
    skills: {
      title: "Skills",
      scanning: "Scanning...",
      drop: "Drop SKILL.md files in .agents/skills or ~/.agents/skills.",
      available: "available to the agent",
      rescan: "Rescan",
      save: "Save",
      saving: "Saving...",
      search: "Search skills",
      searchCount: "Search {count} skills",
      workspace: "workspace",
      global: "global",
      enabled: "enabled",
      off: "off",
      disable: "Disable",
      enable: "Enable",
      noMatch: "No skills match.",
      noSkills: "No skills yet",
      createFolder: "Create a folder under",
      withSkill: "with a SKILL.md file.",
      nothingPreview: "Nothing to preview",
      selectSkill: "Select a skill",
      revealFinder: "Reveal in Finder",
      deleteSkill: "Delete skill",
      confirmSkillDelete: "Confirm skill delete",
      deleting: "Deleting...",
      confirmDelete: "Confirm delete",
      delete: "Delete",
    },
  },
  fr: {
    openRouter: {
      description: "Utilisez n'importe quel modèle OpenRouter avec votre propre clé API.",
      keySaved: "Clé enregistrée",
      hideKey: "Masquer la clé",
      showKey: "Afficher la clé",
      removeKey: "Supprimer la clé API",
      search: "Rechercher",
      typeModel: "Tapez un nom de modèle...",
      saveKeyFirst: "Enregistrez d'abord une clé valide",
      searching: "Recherche...",
      noMatchingModel: "Aucun modèle correspondant.",
      added: "Ajouté",
      adding: "Ajout...",
      add: "Ajouter",
      removeModel: "Retirer le modèle",
    },
    tools: {
      title: "Outils",
      loading: "Chargement...",
      enabled: "activés",
      save: "Enregistrer",
      saving: "Enregistrement...",
      planPromptTitle: "Prompt du mode Plan",
      default: "Par défaut",
      custom: "Personnalisé",
      imageGeneration: "Génération d'image",
      imageProvider: "Fournisseur d'images",
      webSearch: "Recherche web",
      webSearchProvider: "Fournisseur de recherche web",
      openAiSubscription: "Utiliser l'abonnement OpenAI",
      openAiSubscriptionConnected: "Authentifie les requêtes d'image avec votre compte OpenAI connecté au lieu d'une clé API.",
      openAiSubscriptionDisconnected: "Connectez OpenAI dans Paramètres -> Modèles pour utiliser votre abonnement.",
      disableOpenAiSubscription: "Désactiver le mode abonnement OpenAI",
      enableOpenAiSubscription: "Activer le mode abonnement OpenAI",
      mainAgent: "Agent principal",
      swarmAgents: "Agents parallèles",
      noTools: "Aucun outil",
      promptInjected: "Prompt injecté dans le mode Plan",
      resetPlanPrompt: "Réinitialiser le prompt du mode Plan",
      resetPrompt: "Réinitialiser le prompt",
      planPromptHelp: "Ce texte est ajouté au prompt système uniquement quand la conversation est en mode Plan.",
      planInstructions: "Instructions du mode Plan...",
      resetDescription: "Réinitialiser la description",
      disable: "Désactiver",
      enable: "Activer",
      description: "description",
    },
    mcp: {
      title: "Serveurs MCP",
      subtitle: "Ajoutez un serveur dans la configuration JSON pour étendre l'agent.",
      checking: "Vérification...",
      saveProbe: "Enregistrer et tester",
      servers: "Serveurs",
      probing: "test...",
      rawConfig: "Configuration brute",
      unsaved: "Non enregistré",
      synced: "Synchronisé",
      untitled: "Sans titre",
      noServers: "Aucun serveur pour le moment - ajoutez-en un dans la configuration brute.",
      disabled: "désactivé",
      pending: "en attente",
      failed: "échec",
      error: "erreur",
      tools: "Outils",
      tool: "outil",
      noDescription: "Aucune description fournie.",
      noTools: "Le serveur n'a retourné aucun outil.",
      probingServer: "Test du serveur...",
      noProbe: "Aucune donnée de test pour le moment.",
      disable: "Désactiver",
      enable: "Activer",
    },
    agents: {
      title: "Agents",
      emptySubtitle: "Créez des agents spécialisés que l'agent principal peut appeler comme outils.",
      available: "disponibles pour l'agent principal",
      save: "Enregistrer",
      saving: "Enregistrement...",
      agents: "Agents",
      newAgent: "Nouvel agent",
      untitled: "Sans titre",
      noAgents: "Aucun agent pour le moment - cliquez sur + pour commencer.",
      selectOrCreate: "Sélectionnez ou créez un agent",
      untitledAgent: "Agent sans titre",
      agentName: "Nom de l'agent",
      clickConfirm: "Cliquez encore pour confirmer",
      deleteAgent: "Supprimer l'agent",
      confirmDelete: "Confirmer la suppression",
      deleteConfirmShort: "Supprimer ?",
      disable: "Désactiver",
      enable: "Activer",
      description: "Description vue par l'agent principal",
      model: "Modèle",
      thinking: "Raisonnement",
      internalPrompt: "Prompt interne",
    },
    skills: {
      title: "Compétences",
      scanning: "Analyse...",
      drop: "Déposez des fichiers SKILL.md dans .agents/skills ou ~/.agents/skills.",
      available: "disponibles pour l'agent",
      rescan: "Réanalyser",
      save: "Enregistrer",
      saving: "Enregistrement...",
      search: "Rechercher des compétences",
      searchCount: "Rechercher parmi {count} compétences",
      workspace: "workspace",
      global: "global",
      enabled: "activée",
      off: "désactivée",
      disable: "Désactiver",
      enable: "Activer",
      noMatch: "Aucune compétence ne correspond.",
      noSkills: "Aucune compétence",
      createFolder: "Créez un dossier dans",
      withSkill: "avec un fichier SKILL.md.",
      nothingPreview: "Rien à prévisualiser",
      selectSkill: "Sélectionnez une compétence",
      revealFinder: "Afficher dans l'explorateur",
      deleteSkill: "Supprimer la compétence",
      confirmSkillDelete: "Confirmer la suppression de la compétence",
      deleting: "Suppression...",
      confirmDelete: "Confirmer la suppression",
      delete: "Supprimer",
    },
  },
} as const;
// ---- Providers section -------------------------------------------------

type ProvidersSectionProps = {
  openAiStatus: OpenAiProviderStatus | null;
  anthropicStatus: AnthropicProviderStatus | null;
  googleStatus: GoogleProviderStatus | null;
  kimiStatus: KimiProviderStatus | null;
  openRouterStatus: OpenRouterProviderStatus | null;
  openRouterModels: OpenRouterModel[];
  loading: boolean;
  busy: boolean;
  message: string | null;
  onRefresh: () => void;
  onConnect: () => void;
  onCancel: () => void;
  onDisconnect: () => void;
  onConnectAnthropic: () => void;
  onCancelAnthropic: () => void;
  onDisconnectAnthropic: () => void;
  onConnectGoogle: () => void;
  onCancelGoogle: () => void;
  onDisconnectGoogle: () => void;
  onConnectKimi: () => void;
  onCancelKimi: () => void;
  onDisconnectKimi: () => void;
  onDisconnectOpenRouter: () => void;
  onOpenRouterStatusChange: (status: OpenRouterProviderStatus) => void;
  onOpenRouterModelsChange: (models: OpenRouterModel[]) => void;
  onOpenRouterChanged: () => void;
};

function ProvidersSection({
  openAiStatus,
  anthropicStatus,
  googleStatus,
  kimiStatus,
  openRouterStatus,
  openRouterModels,
  loading,
  busy,
  message,
  onRefresh,
  onConnect,
  onCancel,
  onDisconnect,
  onConnectAnthropic,
  onCancelAnthropic,
  onDisconnectAnthropic,
  onConnectGoogle,
  onCancelGoogle,
  onDisconnectGoogle,
  onConnectKimi,
  onCancelKimi,
  onDisconnectKimi,
  onDisconnectOpenRouter,
  onOpenRouterStatusChange,
  onOpenRouterModelsChange,
  onOpenRouterChanged,
}: ProvidersSectionProps) {
  const language = useLanguage();
  const copy = providersCopy[language];
  return (
    <>
      <header className="settings-pane__header">
        <div className="settings-pane__header-text">
          <h1 className="settings-pane__title">{copy.title}</h1>
          <p className="settings-pane__subtitle">
            {copy.subtitle}
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
            <span>{loading ? copy.refreshing : copy.refresh}</span>
          </button>
        </div>
      </header>

      <div className="settings-pane__body settings-pane__body--providers">
        <ProviderCard
          name="Anthropic"
          icon="simple-icons:anthropic"
          description={copy.anthropicDescription}
          status={anthropicStatus}
          connectedMeta={["Claude OAuth"]}
          loading={loading}
          busy={busy}
          onConnect={onConnectAnthropic}
          onCancel={onCancelAnthropic}
          onDisconnect={onDisconnectAnthropic}
        />
        <ProviderCard
          name="OpenAI"
          icon="simple-icons:openai"
          description={copy.openAiDescription}
          status={openAiStatus}
          connectedMeta={[
            openAiStatus?.email || copy.signedIn,
            openAiStatus?.planType ?? null,
          ]}
          loading={loading}
          busy={busy}
          onConnect={onConnect}
          onCancel={onCancel}
          onDisconnect={onDisconnect}
        />
        <ProviderCard
          name="Google"
          icon="simple-icons:google"
          description={copy.googleDescription}
          status={googleStatus}
          connectedMeta={[
            googleStatus?.email || copy.signedIn,
            googleStatus?.userTier ?? null,
            googleStatus?.projectId
              ? `${copy.project} ${googleStatus.projectId}`
              : null,
          ]}
          loading={loading}
          busy={busy}
          onConnect={onConnectGoogle}
          onCancel={onCancelGoogle}
          onDisconnect={onDisconnectGoogle}
        />
        <ProviderCard
          name="Kimi"
          icon="local:kimi"
          description={copy.kimiDescription}
          status={kimiStatus}
          connectedMeta={["Kimi OAuth"]}
          loading={loading}
          busy={busy}
          onConnect={onConnectKimi}
          onCancel={onCancelKimi}
          onDisconnect={onDisconnectKimi}
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
}: ProviderCardProps) {
  const language = useLanguage();
  const copy = providerCardCopy[language];
  const state = status?.connectionState ?? "disconnected";
  const connected = Boolean(status?.connected);
  const connecting = state === "connecting";
  const error = state === "error" ? status?.error : null;
  const statusLabel = connecting
    ? copy.connecting
    : connected
      ? copy.connected
      : state === "error"
        ? copy.needsAttention
        : copy.notConnected;
  const statusTone = connecting
    ? "pending"
    : connected
      ? "ok"
      : state === "error"
        ? "error"
        : "off";
  const meta = connectedMeta.filter((item): item is string => Boolean(item));

  return (
    <section className="settings-pane__provider-card">
      <div className="settings-pane__provider-main">
        <div className="settings-pane__provider-mark" aria-hidden>
          <Icon icon={icon} width={24} height={24} />
        </div>
        <div className="settings-pane__provider-copy">
          <div className="settings-pane__provider-title-row">
            <h2>{name}</h2>
            <span className="settings-pane__chip" data-tone={statusTone}>
              <span className="settings-pane__chip-dot" />
              {statusLabel}
            </span>
          </div>
          <p>{description}</p>
          {connected && meta.length > 0 && (
            <div className="settings-pane__provider-meta">
              {meta.map((item) => (
                <span key={item}>{item}</span>
              ))}
            </div>
          )}
          {error && <div className="settings-pane__provider-error">{error}</div>}
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
            <span>{copy.cancel}</span>
          </button>
        ) : connected ? (
          <button
            type="button"
            className="settings-pane__btn"
            onClick={onDisconnect}
            disabled={busy}
          >
            <Icon icon="solar:logout-2-linear" width={13} height={13} />
            <span>{busy ? copy.disconnecting : copy.disconnect}</span>
          </button>
        ) : (
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
            <span>{busy ? copy.opening : copy.connect}</span>
          </button>
        )}
      </div>
    </section>
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
  const language = useLanguage();
  const copy = providerCardCopy[language];
  const openRouterCopy = settingsDetailCopy[language].openRouter;
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
  const connecting = state === "connecting";
  const error = validationError ?? (state === "error" ? displayStatus.error : null);
  const statusLabel = connecting
    ? copy.connecting
    : connected
      ? copy.connected
      : state === "error"
        ? copy.needsAttention
        : copy.notConnected;
  const statusTone = connecting
    ? "pending"
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
          <p>{openRouterCopy.description}</p>
          {error && <div className="settings-pane__provider-error">{error}</div>}
        </div>
      </div>

      <div className="settings-pane__provider-detail">
        <label className="settings-pane__tool-credential">
          <span className="settings-pane__tool-credential-label">API key</span>
          <div className="settings-pane__tool-credential-field">
            <input
              type={revealed ? "text" : "password"}
              value={apiKey}
              placeholder={connected ? displayStatus.keyPreview ?? openRouterCopy.keySaved : "sk-or-..."}
              onChange={(event) => setApiKey(event.target.value)}
              autoComplete="off"
              spellCheck={false}
            />
            <div className="settings-pane__tool-credential-actions">
              <button
                type="button"
                className="settings-pane__icon-btn"
                onClick={() => setRevealed((value) => !value)}
                title={revealed ? openRouterCopy.hideKey : openRouterCopy.showKey}
                aria-label={revealed ? openRouterCopy.hideKey : openRouterCopy.showKey}
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
                  title={openRouterCopy.removeKey}
                  aria-label={openRouterCopy.removeKey}
                >
                  <Icon icon="solar:trash-bin-trash-linear" width={13} height={13} />
                </button>
              )}
            </div>
          </div>
        </label>

        <label className="settings-pane__tool-credential">
          <span className="settings-pane__tool-credential-label">{openRouterCopy.search}</span>
          <div className="settings-pane__tool-credential-field">
            <input
              type="text"
              value={query}
              disabled={!searchEnabled}
              placeholder={searchEnabled ? openRouterCopy.typeModel : openRouterCopy.saveKeyFirst}
              onChange={(event) => setQuery(event.target.value)}
            />
          </div>
        </label>

        {searchEnabled && query.trim() !== "" && (
          <div className="settings-pane__openrouter-results" aria-live="polite">
            {searching ? (
              <div className="settings-pane__openrouter-hint">{openRouterCopy.searching}</div>
            ) : searchError ? (
              <div className="settings-pane__provider-error">{searchError}</div>
            ) : results.length === 0 ? (
              <div className="settings-pane__openrouter-hint">{openRouterCopy.noMatchingModel}</div>
            ) : (
              results.map((model) => {
                const added = modelIds.has(model.id);
                const label = sanitizeOpenRouterName(model.name) || model.id;
                return (
                  <div key={model.id} className="settings-pane__openrouter-row">
                    <span title={model.id}>{label}</span>
                    {added ? (
                      <span className="settings-pane__openrouter-added">{openRouterCopy.added}</span>
                    ) : (
                      <button
                        type="button"
                        className="settings-pane__btn"
                        onClick={() => void addModel(model)}
                        disabled={mutatingModelId === model.id}
                      >
                        <Icon icon="solar:add-circle-linear" width={13} height={13} />
                        <span>{mutatingModelId === model.id ? openRouterCopy.adding : openRouterCopy.add}</span>
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
                    title={openRouterCopy.removeModel}
                    aria-label={`${openRouterCopy.removeModel}: ${label}`}
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
  onOpenAiImageApiKeyChange: (value: string) => void;
  onNanoBananaApiKeyChange: (value: string) => void;
  onWebSearchProviderChange: (value: WebSearchProvider) => void;
  onLinkupApiKeyChange: (value: string) => void;
  openAiStatus: OpenAiProviderStatus | null;
};

const TOOL_GROUPS = [
  { id: "main", label: "Main Agent" },
  { id: "swarm", label: "Swarm Agents" },
] as const;

type ToolGroupId = (typeof TOOL_GROUPS)[number]["id"];

const SWARM_TOOL_NAMES = new Set([
  "SendMessage",
  "TaskList",
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
  onOpenAiImageApiKeyChange,
  onNanoBananaApiKeyChange,
  onWebSearchProviderChange,
  onLinkupApiKeyChange,
  openAiStatus,
}: ToolsSectionProps) {
  const language = useLanguage();
  const copy = settingsDetailCopy[language].tools;
  const tools = settings?.tools ?? [];
  const planModePrompt = settings?.planModePrompt ?? "";
  const defaultPlanModePrompt = settings?.defaultPlanModePrompt ?? "";
  const imageProvider = settings?.imageProvider ?? "gptImage2";
  const openaiImageUseSubscription = settings?.openaiImageUseSubscription ?? false;
  const openaiImageApiKey = settings?.openaiImageApiKey ?? "";
  const nanoBananaApiKey = settings?.nanoBananaApiKey ?? "";
  const webSearchProvider = settings?.webSearchProvider ?? "classic";
  const linkupApiKey = settings?.linkupApiKey ?? "";
  const openAiConnected = openAiStatus?.connected === true;
  const subscriptionActive =
    imageProvider === "gptImage2" && openAiConnected && openaiImageUseSubscription;
  const showImageKeyField =
    imageProvider === "nanoBanana2" || !subscriptionActive;
  const activeImageKey =
    imageProvider === "nanoBanana2" ? nanoBananaApiKey : openaiImageApiKey;
  const hasImageTool = tools.some((tool) => tool.name === "CreateImage");
  const hasWebSearchTool = tools.some((tool) => tool.name === "WebSearch");
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
          <h1 className="settings-pane__title">{copy.title}</h1>
          <p className="settings-pane__subtitle">
            {loading ? copy.loading : `${enabledCount}/${tools.length} ${copy.enabled}`}
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
            <span>{saving ? copy.saving : copy.save}</span>
          </button>
        </div>
      </header>

      <div className="settings-pane__body settings-pane__body--tools">
        <div className="settings-pane__tool-settings-list">
          <section className="settings-pane__tool-group">
            <div className="settings-pane__tool-group-head">
              <h2>{copy.planPromptTitle}</h2>
              <span>
                {planModePrompt === defaultPlanModePrompt ? copy.default : copy.custom}
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
                <h2>{copy.imageGeneration}</h2>
              </div>
              <div
                className="settings-pane__tool-provider-switch"
                role="group"
                aria-label={copy.imageProvider}
              >
                <button
                  type="button"
                  data-active={imageProvider === "gptImage2" ? "true" : "false"}
                  onClick={() => onImageProviderChange("gptImage2")}
                >
                  GPT Image 2
                </button>
                <button
                  type="button"
                  data-active={imageProvider === "nanoBanana2" ? "true" : "false"}
                  onClick={() => onImageProviderChange("nanoBanana2")}
                >
                  Nano Banana 2
                </button>
              </div>
              {imageProvider === "gptImage2" && (
                <div
                  className="settings-pane__tool-toggle-row"
                  data-disabled={openAiConnected ? "false" : "true"}
                >
                  <div className="settings-pane__tool-toggle-text">
                    <span className="settings-pane__tool-toggle-label">
                      {copy.openAiSubscription}
                    </span>
                    <span className="settings-pane__tool-toggle-hint">
                      {openAiConnected
                        ? copy.openAiSubscriptionConnected
                        : copy.openAiSubscriptionDisconnected}
                    </span>
                  </div>
                  <button
                    type="button"
                    className="settings-pane__switch"
                    role="switch"
                    aria-checked={subscriptionActive}
                    aria-label={
                      subscriptionActive
                        ? copy.disableOpenAiSubscription
                        : copy.enableOpenAiSubscription
                    }
                    data-on={subscriptionActive ? "true" : "false"}
                    disabled={!openAiConnected}
                    onClick={() =>
                      onOpenAiImageUseSubscriptionChange(!openaiImageUseSubscription)
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
                      ? "Gemini API key"
                      : "OpenAI API key"
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
                <h2>{copy.webSearch}</h2>
              </div>
              <div
                className="settings-pane__tool-provider-switch"
                role="group"
                aria-label={copy.webSearchProvider}
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
                <h2>{group.id === "swarm" ? copy.swarmAgents : copy.mainAgent}</h2>
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
              <span className="settings-pane__empty-title">{copy.noTools}</span>
            </div>
          )}
        </div>
      </div>
    </>
  );
}

function toolGroupId(tool: ToolConfig): ToolGroupId {
  return SWARM_TOOL_NAMES.has(tool.name) ? "swarm" : "main";
}

const DEFAULT_PLAN_MODE_PROMPT_FR = [
  "Vous \u00eates en mode Plan.",
  "",
  "R\u00e8gles:",
  "- Construisez votre compr\u00e9hension en lisant, recherchant ou lan\u00e7ant des commandes de diagnostic si n\u00e9cessaire.",
  "- Ne modifiez pas les fichiers de l'espace de travail et n'utilisez pas apply_patch.",
  "- Gardez l'utilisateur dans une boucle de questions jusqu'\u00e0 ce qu'il clique explicitement sur \"Send and stop questions\".",
  "- Si le message utilisateur ne contient pas <plan_mode_control action=\"stop_questions\">, terminez votre tour avec l'outil Question. N'\u00e9crivez pas encore le plan final.",
  "- Apr\u00e8s chaque r\u00e9ponse normale \u00e0 une Question, inspectez ou explorez davantage si besoin, puis posez la question suivante.",
  "- S'il ne reste aucune question substantielle, demandez \u00e0 l'utilisateur de confirmer que vous devez cr\u00e9er le plan maintenant. Utilisez quand m\u00eame l'outil Question.",
  "- Uniquement quand le message utilisateur contient <plan_mode_control action=\"stop_questions\">, arr\u00eatez les questions et \u00e9crivez le plan complet.",
  "- Quand le plan est pr\u00eat, r\u00e9pondez uniquement avec le plan en Markdown. Ne l'impl\u00e9mentez pas.",
  "",
  "STRICTEMENT INTERDIT dans le plan, sauf demande explicite de l'utilisateur:",
  "- Extraits de code, pseudo-code ou code inline",
  "- Chemins de fichiers, structures de dossiers ou vues en arbre",
  "- Noms de fonctions, classes, variables ou modules",
  "- Commandes shell ou instructions CLI",
  "- D\u00e9tails de configuration technique",
  "- Toute notation sp\u00e9cifique \u00e0 l'impl\u00e9mentation",
  "",
  "Le plan doit se lire comme une description claire de l'intention et du comportement attendu, compr\u00e9hensible sans bagage technique. Les listes \u00e0 puces et les paragraphes sont accept\u00e9s. L'objectif est d'expliquer CE que le syst\u00e8me doit faire, pas COMMENT le code doit \u00eatre \u00e9crit.",
  "",
  "Si des pr\u00e9cisions techniques deviennent n\u00e9cessaires pour lever une ambigu\u00eft\u00e9, l'IA peut les inclure \u00e0 sa discr\u00e9tion, naturellement dans le plan, mais cela doit rester l'exception.",
].join("\n");

function planModePromptForDisplay(
  value: string,
  defaultValue: string,
  language: AppLanguage,
): string {
  if (language === "fr" && value === defaultValue) {
    return DEFAULT_PLAN_MODE_PROMPT_FR;
  }
  return value;
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
  const language = useLanguage();
  const copy = settingsDetailCopy[language].tools;
  const canReset = value !== defaultValue;
  const displayValue = planModePromptForDisplay(value, defaultValue, language);
  const rows = Math.min(18, Math.max(10, displayValue.split("\n").length + 1));

  return (
    <div className="settings-pane__tool-config" data-on="true">
      <div className="settings-pane__tool-config-head">
        <span className="settings-pane__tool-config-name">
          <span className="settings-pane__tool-config-glyph" aria-hidden>
            <Icon icon="solar:document-text-linear" width={15} height={15} />
          </span>
          <span className="settings-pane__tool-config-label">
            {copy.promptInjected}
          </span>
        </span>
        <div className="settings-pane__tool-config-actions">
          <button
            type="button"
            className="settings-pane__icon-btn"
            aria-label={copy.resetPlanPrompt}
            title={copy.resetPrompt}
            disabled={!canReset}
            onClick={() => onChange(defaultValue)}
          >
            <Icon icon="solar:refresh-linear" width={14} height={14} />
          </button>
        </div>
      </div>
      <p className="settings-pane__tool-config-help">
        {copy.planPromptHelp}
      </p>
      <textarea
        className="settings-pane__tool-config-desc settings-pane__tool-config-desc--prompt"
        aria-label={copy.planPromptTitle}
        value={displayValue}
        rows={rows}
        placeholder={copy.planInstructions}
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
  const language = useLanguage();
  const copy = settingsDetailCopy[language].tools;
  const canReset = tool.description !== tool.defaultDescription;
  const displayDescription = descriptionForTool(tool, language);
  const rows = Math.min(8, Math.max(3, displayDescription.split("\n").length + 1));

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
            {labelForTool(tool, language)}
          </span>
        </span>
        <div className="settings-pane__tool-config-actions">
          <button
            type="button"
            className="settings-pane__icon-btn"
            aria-label={`${copy.resetDescription}: ${tool.name}`}
            title={copy.resetDescription}
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
            aria-label={`${tool.enabled ? copy.disable : copy.enable} ${tool.name}`}
            data-on={tool.enabled ? "true" : "false"}
            onClick={() => onUpdate({ enabled: !tool.enabled })}
          >
            <span className="settings-pane__switch-thumb" />
          </button>
        </div>
      </div>
      <textarea
        className="settings-pane__tool-config-desc"
        aria-label={`${tool.name} ${copy.description}`}
        value={displayDescription}
        rows={rows}
        onChange={(event) => onUpdate({ description: event.target.value })}
      />
    </div>
  );
}

// ---- MCP section component ---------------------------------------------

type McpSectionProps = {
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
  onClearSelection: () => void;
  selectedServer: McpServerConfig | null;
  selectedProbe: McpServerProbe | null;
  onToggleEnabled: (id: string) => void;
  onMount: OnMount;
};

function McpSection({
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
  onClearSelection,
  selectedServer,
  selectedProbe,
  onToggleEnabled,
  onMount,
}: McpSectionProps) {
  const language = useLanguage();
  const copy = settingsDetailCopy[language].mcp;
  const detailOpen = Boolean(selectedServer);

  return (
    <>
      <header className="settings-pane__header">
        <div className="settings-pane__header-text">
          <h1 className="settings-pane__title">{copy.title}</h1>
          {servers.length === 0 && (
            <p className="settings-pane__subtitle">
              {copy.subtitle}
            </p>
          )}
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
            data-primary="true"
            onClick={onSave}
            disabled={loading || saving || !dirty}
          >
            <Icon
              icon={saving ? "solar:refresh-linear" : "solar:diskette-linear"}
              width={13}
              height={13}
            />
            <span>{saving ? copy.checking : copy.saveProbe}</span>
          </button>
        </div>
      </header>

      <div className="settings-pane__body settings-pane__body--mcp">
        <aside className="settings-pane__nav-list">
          <div className="settings-pane__nav-list-head">
            <span>{copy.servers}</span>
            {probing && (
              <span className="settings-pane__servers-meta">{copy.probing}</span>
            )}
          </div>
          <div className="settings-pane__nav-list-items">
            <button
              type="button"
              className="settings-pane__nav-list-item"
              data-active={!detailOpen ? "true" : "false"}
              onClick={onClearSelection}
            >
              <Icon
                icon="solar:code-square-linear"
                width={12}
                height={12}
                className="settings-pane__nav-list-item-glyph"
              />
              <span className="settings-pane__nav-list-item-name">
                {copy.rawConfig}
              </span>
              {dirty && (
                <span
                  className="settings-pane__nav-list-item-dot"
                  data-tone="dirty"
                  aria-label={copy.unsaved}
                />
              )}
            </button>
            {servers.length > 0 && (
              <div className="settings-pane__nav-list-divider" />
            )}
            {servers.map((server) => {
              const probe = probes.find((item) => item.serverId === server.id);
              const tone = !server.enabled
                ? "off"
                : !probe
                  ? "pending"
                  : probe.ok
                    ? "ok"
                    : "error";
              const isActive =
                detailOpen && selectedServer?.id === server.id;
              return (
                <button
                  type="button"
                  key={server.id}
                  className="settings-pane__nav-list-item"
                  data-active={isActive ? "true" : "false"}
                  data-on={server.enabled ? "true" : "false"}
                  onClick={() => onSelectServer(server.id)}
                >
                  <span
                    className="settings-pane__nav-list-item-dot"
                    data-tone={tone}
                    aria-hidden
                  />
                  <span className="settings-pane__nav-list-item-name">
                    {server.name || copy.untitled}
                  </span>
                </button>
              );
            })}
            {servers.length === 0 && (
              <div className="settings-pane__nav-list-empty">
                {copy.noServers}
              </div>
            )}
          </div>
        </aside>

        <main className="settings-pane__detail-pane">
          {detailOpen && selectedServer ? (
            <ServerDetail
              server={selectedServer}
              probe={selectedProbe}
              probing={probing}
            />
          ) : (
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
                      {copy.unsaved}
                    </span>
                  ) : (
                    <span className="settings-pane__pill" data-tone="ok">
                      {copy.synced}
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
          )}
        </main>
      </div>
    </>
  );
}

type ServerCardProps = {
  server: McpServerConfig;
  probe: McpServerProbe | undefined;
  probing: boolean;
  disabled: boolean;
  onOpen: () => void;
  onToggle: () => void;
};

function ServerCard({
  server,
  probe,
  probing,
  disabled,
  onOpen,
  onToggle,
}: ServerCardProps) {
  const language = useLanguage();
  const copy = settingsDetailCopy[language].mcp;
  const tone = !server.enabled
    ? "off"
    : !probe
      ? "pending"
      : probe.ok
        ? "ok"
        : "error";

  const label = !server.enabled
    ? copy.disabled
    : !probe
      ? probing
        ? copy.probing
        : copy.pending
      : probe.ok
        ? `${probe.tools.length} ${copy.tool}${probe.tools.length === 1 ? "" : "s"}`
        : "error";

  const command = [server.command, ...server.args].join(" ").trim();

  return (
    <div
      className="settings-pane__server-card"
      data-on={server.enabled ? "true" : "false"}
      onClick={onOpen}
    >
      <div className="settings-pane__server-row">
        <span className="settings-pane__server-name">{server.name || copy.untitled}</span>
        <button
          type="button"
          className="settings-pane__switch"
          role="switch"
          aria-checked={server.enabled}
          aria-label={`${server.enabled ? copy.disable : copy.enable} ${server.name}`}
          data-on={server.enabled ? "true" : "false"}
          disabled={disabled}
          onClick={(event) => {
            event.stopPropagation();
            onToggle();
          }}
        >
          <span className="settings-pane__switch-thumb" />
        </button>
      </div>
      <div className="settings-pane__server-meta">
        <span className="settings-pane__chip" data-tone={tone}>
          <span className="settings-pane__chip-dot" />
          {label}
        </span>
        {command && (
          <code className="settings-pane__server-cmd" title={command}>
            {command}
          </code>
        )}
      </div>
    </div>
  );
}

type ServerDetailProps = {
  server: McpServerConfig;
  probe: McpServerProbe | null;
  probing: boolean;
};

function ServerDetail({ server, probe, probing }: ServerDetailProps) {
  const language = useLanguage();
  const copy = settingsDetailCopy[language].mcp;
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
    ? copy.disabled
    : !probe
      ? probing
        ? copy.probing
        : copy.pending
      : !probe.ok
        ? copy.failed
        : `${probe.tools.length} ${copy.tool}${probe.tools.length === 1 ? "" : "s"}`;
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

        <div className="settings-pane__detail-section">{copy.tools}</div>
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
                        {copy.noDescription}
                      </div>
                    )}
                  </div>
                )}
              </div>
            );
          })}
          {probe?.ok && probe.tools.length === 0 && (
            <div className="settings-pane__muted">{copy.noTools}</div>
          )}
          {!probe && (
            <div className="settings-pane__muted">
              {probing ? copy.probingServer : copy.noProbe}
            </div>
          )}
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
  const language = useLanguage();
  const copy = settingsDetailCopy[language].agents;
  const enabledCount = settings.agents.filter((agent) => agent.enabled).length;

  return (
    <>
      <header className="settings-pane__header">
        <div className="settings-pane__header-text">
          <h1 className="settings-pane__title">{copy.title}</h1>
          <p className="settings-pane__subtitle">
            {settings.agents.length === 0
              ? copy.emptySubtitle
              : `${enabledCount}/${settings.agents.length} ${copy.available}`}
          </p>
        </div>
        <div className="settings-pane__actions">
          {status && (
            <span
              className="settings-pane__status"
              data-tone={status === "Saved" ? "ok" : "error"}
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
            <span>{saving ? copy.saving : copy.save}</span>
          </button>
        </div>
      </header>

      <div className="settings-pane__body settings-pane__body--subagents">
        <aside className="settings-pane__nav-list">
          <div className="settings-pane__nav-list-head">
            <span>{copy.agents}</span>
            <button
              type="button"
              className="settings-pane__nav-list-add"
              onClick={onAdd}
              aria-label={copy.newAgent}
              title={copy.newAgent}
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
                  {agent.name || copy.untitled}
                </span>
              </button>
            ))}
            {!loading && settings.agents.length === 0 && (
              <div className="settings-pane__nav-list-empty">
                {copy.noAgents}
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
                {copy.selectOrCreate}
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
  const language = useLanguage();
  const copy = settingsDetailCopy[language].agents;
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
          placeholder={copy.untitledAgent}
          aria-label={copy.agentName}
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
            title={confirmDelete ? copy.clickConfirm : copy.deleteAgent}
            aria-label={confirmDelete ? copy.confirmDelete : copy.deleteAgent}
          >
            {confirmDelete ? (
              <span className="settings-pane__icon-btn-confirm">{copy.deleteConfirmShort}</span>
            ) : (
              <Icon icon="solar:trash-bin-trash-linear" width={13} height={13} />
            )}
          </button>
          <button
            type="button"
            className="settings-pane__switch"
            role="switch"
            aria-checked={agent.enabled}
            aria-label={`${agent.enabled ? copy.disable : copy.enable} ${agent.name}`}
            data-on={agent.enabled ? "true" : "false"}
            onClick={() => onUpdate({ enabled: !agent.enabled })}
          >
            <span className="settings-pane__switch-thumb" />
          </button>
        </div>
      </div>

      <div className="settings-pane__subagent-form">
        <label className="settings-pane__field settings-pane__field--grow">
          <span>{copy.description}</span>
          <textarea
            value={agent.description}
            onChange={(event) => onUpdate({ description: event.target.value })}
          />
        </label>

        <div className="settings-pane__subagent-row">
          <label className="settings-pane__field">
            <span>{copy.model}</span>
            <SettingsPicker
              value={modelId}
              options={availableModels.map((model) => ({
                value: model.value,
                label: model.label,
                icon: PROVIDERS.find((p) => p.value === model.provider)?.icon,
              }))}
              onSelect={(value) => updateModel(value as ModelId)}
            />
          </label>
          <label className="settings-pane__field">
            <span>{copy.thinking}</span>
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
          <span>{copy.internalPrompt}</span>
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
  onToggleSkill: (name: string) => void;
  onRevealSkill: (skill: InstalledSkill) => void;
  onDeleteSkill: (skill: InstalledSkill) => void;
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
  onToggleSkill,
  onRevealSkill,
  onDeleteSkill,
}: SkillsSectionProps) {
  const language = useLanguage();
  const copy = settingsDetailCopy[language].skills;
  const total = allSkills?.length ?? 0;
  const visible = skills.length;
  const enabled = allSkills?.filter((skill) => skill.enabled).length ?? 0;

  return (
    <>
      <header className="settings-pane__header">
        <div className="settings-pane__header-text">
          <h1 className="settings-pane__title">{copy.title}</h1>
          <p className="settings-pane__subtitle">
            {loading
              ? copy.scanning
              : total === 0
                ? copy.drop
                : `${enabled}/${total} ${copy.available}`}
          </p>
        </div>
        <div className="settings-pane__actions">
          {status && (
            <span
              className="settings-pane__status"
              data-tone={status === "Saved" ? "ok" : "error"}
            >
              {status}
            </span>
          )}
          <button
            type="button"
            className="settings-pane__btn"
            onClick={onRefresh}
            disabled={loading || deleting}
          >
            <Icon icon="solar:refresh-linear" width={13} height={13} />
            <span>{copy.rescan}</span>
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
            <span>{saving ? copy.saving : copy.save}</span>
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
              placeholder={total ? copy.searchCount.replace("{count}", String(total)) : copy.search}
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
                      {skill.source === "workspace" ? copy.workspace : copy.global}
                    </span>
                    <span
                      className="settings-pane__skill-state"
                      data-enabled={skill.enabled ? "true" : "false"}
                    >
                      {skill.enabled ? copy.enabled : copy.off}
                    </span>
                  </div>
                  <button
                    type="button"
                    className="settings-pane__switch"
                    role="switch"
                    aria-checked={skill.enabled}
                    aria-label={`${skill.enabled ? copy.disable : copy.enable} ${skill.name}`}
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
                {copy.noMatch}
              </div>
            )}
            {!loading && total === 0 && (
              <div className="settings-pane__empty">
                <Icon icon="solar:magic-stick-3-linear" width={22} height={22} />
                <span className="settings-pane__empty-title">{copy.noSkills}</span>
                <span className="settings-pane__empty-sub">
                  {copy.createFolder} <code>.agents/skills/&lt;name&gt;/</code>{" "}
                  {copy.withSkill}
                </span>
              </div>
            )}
          </div>
        </aside>

        <div className="settings-pane__skill-preview">
          {selectedSkill ? (
            <SkillPreview
              skill={selectedSkill}
              deleting={deleting}
              onReveal={() => onRevealSkill(selectedSkill)}
              onDelete={() => onDeleteSkill(selectedSkill)}
            />
          ) : (
            <div className="settings-pane__empty settings-pane__empty--main">
              <Icon icon="solar:document-text-linear" width={22} height={22} />
              <span className="settings-pane__empty-title">
                {total === 0 ? copy.nothingPreview : copy.selectSkill}
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
  deleting,
  onReveal,
  onDelete,
}: {
  skill: InstalledSkill;
  deleting: boolean;
  onReveal: () => void;
  onDelete: () => void;
}) {
  const language = useLanguage();
  const copy = settingsDetailCopy[language].skills;
  const body = stripFrontmatter(skill.content);
  const [confirmDelete, setConfirmDelete] = useState(false);

  useEffect(() => {
    if (!confirmDelete) return;
    const id = window.setTimeout(() => setConfirmDelete(false), 3000);
    return () => window.clearTimeout(id);
  }, [confirmDelete]);

  useEffect(() => {
    setConfirmDelete(false);
  }, [skill.absolutePath]);

  return (
    <article className="settings-pane__skill-doc">
      <header className="settings-pane__skill-doc-head">
        <div className="settings-pane__skill-doc-top">
          <div className="settings-pane__skill-doc-title">
            <h2>{skill.name}</h2>
            <span
              className="settings-pane__skill-source"
              data-source={skill.source}
            >
              {skill.source === "workspace" ? copy.workspace : copy.global}
            </span>
          </div>
          <div className="settings-pane__skill-doc-actions">
            <button
              type="button"
              className="settings-pane__skill-doc-action"
              onClick={onReveal}
              disabled={deleting}
              title={copy.revealFinder}
              aria-label={`${copy.revealFinder}: ${skill.name}`}
            >
              <Icon icon="solar:folder-open-linear" width={13} height={13} />
              <span>{copy.revealFinder}</span>
            </button>
            <button
              type="button"
              className="settings-pane__skill-doc-action"
              data-danger="true"
              data-confirm={confirmDelete ? "true" : "false"}
              disabled={deleting}
              title={confirmDelete ? copy.confirmDelete : copy.deleteSkill}
              aria-label={confirmDelete ? copy.confirmSkillDelete : `${copy.deleteSkill}: ${skill.name}`}
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
                  ? copy.deleting
                  : confirmDelete
                    ? copy.confirmDelete
                    : copy.delete}
              </span>
            </button>
          </div>
        </div>
        <code className="settings-pane__skill-path">{skill.absolutePath}</code>
        {skill.description && (
          <p className="settings-pane__skill-doc-desc">{skill.description}</p>
        )}
      </header>
      <div className="settings-pane__skill-doc-body">
        <Markdown text={body || "_(empty SKILL.md)_"} onOpenFile={noop} />
      </div>
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
    planModePrompt,
    defaultPlanModePrompt,
    openaiImageApiKey: settings.openaiImageApiKey ?? "",
    nanoBananaApiKey: settings.nanoBananaApiKey ?? "",
    webSearchProvider:
      settings.webSearchProvider === "linkup" ? "linkup" : "classic",
    linkupApiKey: settings.linkupApiKey ?? "",
    tools: (settings.tools ?? []).flatMap((tool) => {
      const name = tool.name?.trim();
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
  const language = useLanguage();
  const copy = settingsDetailCopy[language].openRouter;
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
            title={revealed ? copy.hideKey : copy.showKey}
            aria-label={revealed ? copy.hideKey : copy.showKey}
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
  if (name === "bash" || name === "bash_input") {
    return <TerminalGlyph />;
  }
  if (name === "Glob" || name === "Grep") {
    return <AsteriskGlyph />;
  }
  if (name === "TeamRun" || name === "TeamStatus" || name === "TeamStop") {
    return <SwarmGlyph />;
  }
  if (name === "LoadMcpTool") {
    return <McpGlyph />;
  }
  if (name === "LoadSkill") {
    return <SkillGlyph />;
  }
  if (name === "clean_context") {
    return <BroomGlyph />;
  }
  const icon = TOOL_ICON[name] ?? "solar:tuning-2-linear";
  return <Icon icon={icon} width={13} height={13} />;
}

const TOOL_LABELS: Record<AppLanguage, Record<string, string>> = {
  en: {
    bash: "Shell",
    bash_input: "Shell input",
    read: "Read",
    apply_patch: "Patch",
    Glob: "Glob",
    Grep: "Grep",
    WebSearch: "Web search",
    WebFetch: "Web fetch",
    CreateImage: "Create image",
    Question: "Question",
    ToDoList: "To-do list",
    TaskList: "Task list",
    LoadMcpTool: "Load MCP tool",
    LoadSkill: "Load skill",
    TeamRun: "Team run",
    TeamStatus: "Team status",
    TeamStop: "Team stop",
    SendMessage: "Send message",
    clean_context: "Clean context",
    update_goal: "Update goal",
    context_compaction: "Compact context",
  },
  fr: {
    bash: "Shell",
    bash_input: "Entr\u00e9e shell",
    read: "Lecture",
    apply_patch: "Patch",
    Glob: "Recherche glob",
    Grep: "Recherche grep",
    WebSearch: "Recherche web",
    WebFetch: "Lecture web",
    CreateImage: "Cr\u00e9er une image",
    Question: "Question",
    ToDoList: "Liste de t\u00e2ches",
    TaskList: "Liste des t\u00e2ches",
    LoadMcpTool: "Charger un outil MCP",
    LoadSkill: "Charger une comp\u00e9tence",
    TeamRun: "Lancer l'\u00e9quipe",
    TeamStatus: "Statut de l'\u00e9quipe",
    TeamStop: "Arr\u00eater l'\u00e9quipe",
    SendMessage: "Envoyer un message",
    clean_context: "Nettoyer le contexte",
    update_goal: "Mettre \u00e0 jour l'objectif",
    context_compaction: "Compacter le contexte",
  },
};
const TOOL_DESCRIPTION_FR: Record<string, string> = {
  bash: "Ex\u00e9cute une commande shell dans l'espace de travail.",
  bash_input: "Envoie du texte \u00e0 une session shell interactive, r\u00e9cup\u00e8re sa sortie ou l'arr\u00eate.",
  read: "Lit des fichiers texte ou joint visuellement les fichiers image pris en charge.",
  apply_patch: "Applique une modification cibl\u00e9e aux fichiers de l'espace de travail.",
  Glob: "Trouve des fichiers de l'espace de travail avec un motif glob.",
  Grep: "Recherche du texte ou des expressions r\u00e9guli\u00e8res dans les fichiers avec ripgrep.",
  WebSearch: "Recherche sur le web, consulte la documentation ou r\u00e9cup\u00e8re des informations r\u00e9centes.",
  WebFetch: "R\u00e9cup\u00e8re une URL pr\u00e9cise, souvent une source issue de la recherche web, et retourne un texte lisible pour l'inspecter.",
  CreateImage: "G\u00e9n\u00e8re ou cr\u00e9e une nouvelle image quand l'utilisateur le demande, puis affiche le r\u00e9sultat visuellement.",
  Question: "Pose une ou plusieurs questions \u00e0 l'utilisateur dans l'interface de chat quand une clarification est n\u00e9cessaire avant de continuer.",
  ToDoList: "Met \u00e0 jour la liste de t\u00e2ches courante avec un petit patch ligne par ligne.",
  TaskList: "Modifie le tableau de t\u00e2ches partag\u00e9 de l'\u00e9quipe d'agents: cr\u00e9er, mettre \u00e0 jour, supprimer ou prendre une t\u00e2che.",
  LoadMcpTool: "Charge un outil MCP par serveur et par nom avant de l'appeler.",
  LoadSkill: "Charge une comp\u00e9tence par son nom exact avant de l'utiliser.",
  TeamRun: "Lance une \u00e9quipe d'agents pour travailler en parall\u00e8le.",
  TeamStatus: "Inspecte l'\u00e9quipe d'agents active, les co\u00e9quipiers, les messages, les t\u00e2ches et les derniers r\u00e9sum\u00e9s.",
  TeamStop: "Arr\u00eate un co\u00e9quipier ou toute l'\u00e9quipe d'agents.",
  SendMessage: "Envoie un message asynchrone \u00e0 un co\u00e9quipier par nom, ou diffuse \u00e0 toute l'\u00e9quipe avec le destinataire \"*\". Les co\u00e9quipiers re\u00e7oivent le message \u00e0 leur prochain tour.",
  clean_context: "Nettoie les r\u00e9sultats d'outils inutiles dans le contexte de l'agent.",
  update_goal: "Marque l'objectif actif du mode Goal comme termin\u00e9 apr\u00e8s v\u00e9rification.",
  context_compaction: "Compacte le contexte courant pour continuer avec un historique plus l\u00e9ger.",
};
const TOOL_ICON: Record<string, string> = {
  read: "solar:document-text-linear",
  apply_patch: "solar:pen-new-square-linear",
  WebSearch: "solar:magnifer-linear",
  WebFetch: "solar:link-round-linear",
  CreateImage: "solar:gallery-wide-linear",
  Question: "solar:question-circle-linear",
  ToDoList: "solar:checklist-linear",
  SendMessage: "solar:chat-round-dots-linear",
  update_goal: "solar:flag-2-linear",
  context_compaction: "solar:archive-linear",
};

function labelForTool(
  tool: ToolConfig | string,
  language: AppLanguage = "en",
): string {
  const name = typeof tool === "string" ? tool : tool.name;
  const translatedLabel = TOOL_LABELS[language][name];
  if (translatedLabel) return translatedLabel;

  if (typeof tool !== "string") {
    const displayName = tool.displayName?.trim();
    if (displayName) return displayName;
  }
  return TOOL_LABELS.en[name] ?? humanizeToolName(name);
}

function descriptionForTool(tool: ToolConfig, language: AppLanguage): string {
  if (language !== "fr") return tool.description;
  if (tool.description !== tool.defaultDescription) return tool.description;
  return TOOL_DESCRIPTION_FR[tool.name] ?? tool.description;
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
