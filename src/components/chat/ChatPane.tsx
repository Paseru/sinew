import {
  useCallback,
  useDeferredValue,
  useEffect,
  useLayoutEffect,
  useMemo,
  type CSSProperties,
  type ReactNode,
  type RefObject,
  useRef,
  useState,
} from "react";
import { Icon } from "@iconify/react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { AIThinkingBlock } from "./AIThinkingBlock";
import { FileChangeBlock } from "./FileChangeBlock";
import { FileLinkedText, Markdown } from "./Markdown";
import { PlanningNextMoveBlock } from "./PlanningNextMoveBlock";
import { Questionnaire, type QuestionItem } from "./Questionnaire";
import {
  AiAgentGlyph,
  subAgentToolTitle,
  ToolCard,
  type ToolCardTeamAgent,
} from "./ToolCard";
import { TodoStrip, type QueuedPromptStripItem } from "./TodoStrip";
import { fileIcon } from "../../lib/fileIcon";
import { api } from "../../lib/ipc";
import {
  MODELS,
  PROVIDERS,
  THINKING_LEVELS,
  availableModelsForProviders,
  modelRefFromId,
  modelRefWithUse1mContext,
  modelSupports1mContextBeta,
  modelsWithOpenRouter,
  selectionFromRef,
  selectionsFromSettings,
  thinkingFromRef,
  type ModelEntry,
  type ModeModelSelection,
  type ModeModelSelections,
  type ModelId,
} from "../../lib/models";
import type {
  AgentMode,
  AttachmentInput,
  ChatMessage,
  ContextEstimate,
  FileChange,
  GoalWorkflowState,
  MessageVisibility,
  ModeModelSettings,
  ModelRef,
  OpenRouterModel,
  Part,
  PlanArtifact,
  PlanControl,
  PlanWorkflowState,
  QuestionAnswer,
  ServiceTier,
  StreamTokenUsage,
  ThinkingLevel,
  WorkspaceEntry,
} from "../../types";
import {
  applyEvent,
  appendUserMessage,
  beginTurn,
  initialStateFromHistory,
  type ChatBlock,
  type ChatViewState,
  type SubAgentBlock,
} from "./stream";
import type { AgentEvent } from "../../types";

type Attachment = {
  path: string;
  name: string;
  origin: "sidebar" | "finder" | "manual";
};

type RewriteState = {
  historyIndex: number;
  originalText: string;
  originalAttachments: Attachment[];
  revertWorkspaceChanges: boolean;
};

type QueuedPrompt = QueuedPromptStripItem & {
  model: ModelRef;
  thinking: ThinkingLevel;
  mode: AgentMode;
  serviceTier: ServiceTier | null;
  createdAtMs: number;
};

type EditingQueuedPrompt = {
  conversationId: string;
  id: string;
  index: number;
  createdAtMs: number;
};

type ComposerDraft = {
  text: string;
  attachments: Attachment[];
  inlineMentions: InlineMention[];
  editingQueuedPrompt: EditingQueuedPrompt | null;
};

const MODES: {
  value: AgentMode;
  label: string;
  icon: string;
}[] = [
  { value: "act", label: "Act", icon: "solar:bolt-circle-linear" },
  { value: "plan", label: "Plan", icon: "solar:clipboard-list-linear" },
  { value: "goal", label: "Goal", icon: "solar:flag-2-linear" },
];

const FAST_SERVICE_TIER_STORAGE_KEY = "sinew.fastServiceTier";

const CONTEXT_BREAKDOWN_COLORS: Record<string, string> = {
  system: "#85888f",
  tools: "#8b5cf6",
  rules: "#2da47d",
  skills: "#d19935",
  mcp: "#c44c96",
  subagents: "#4f9bcb",
  conversation: "#e57e56",
  input: "#e57e56",
  output: "#4f9bcb",
  reasoning: "#8b5cf6",
  cache: "#2da47d",
  cache_write: "#d19935",
};

type Props = {
  workspacePath: string;
  conversationId: string;
  activeModel: ModelRef;
  modeModelSettings: ModeModelSettings;
  streamingModel: ModelRef | null;
  planWorkflow: PlanWorkflowState;
  goalWorkflow: GoalWorkflowState;
  isStreaming: boolean;
  history: ChatMessage[];
  subscribeEvents: (
    handler: (conversationId: string, event: AgentEvent, sequence?: number) => void,
  ) => () => void;
  onSend: (
    text: string,
    attachments: AttachmentInput[],
    model: ModelRef,
    thinking: ThinkingLevel,
    mode: AgentMode,
    serviceTier?: ServiceTier | null,
    rewriteFromHistoryIndex?: number,
    planControl?: PlanControl,
    messageVisibility?: MessageVisibility,
    revertWorkspaceChanges?: boolean,
  ) => Promise<void>;
  onCompact: (
    model: ModelRef,
    thinking: ThinkingLevel,
    serviceTier?: ServiceTier | null,
    options?: { continueAfter?: boolean; instruction?: string },
  ) => Promise<void>;
  onModeChange: (mode: AgentMode) => Promise<void>;
  onModelPreferenceChange: (
    mode: AgentMode,
    model: ModelRef,
    thinking: ThinkingLevel,
  ) => Promise<void>;
  onImplementPlanFresh: (
    plan: PlanArtifact,
    prompt?: string,
  ) => Promise<void>;
  onStop: () => Promise<void>;
  onOpenFile: (path: string) => void;
  onOpenSettings: (section?: "providers") => void;
  externalDrops: ExternalDropFeed;
  dropZoneRef: RefObject<HTMLDivElement>;
};

function preserveTrailingTurnDuration(
  next: ChatViewState,
  current: ChatViewState,
): ChatViewState {
  const trailing = current.blocks[current.blocks.length - 1];
  if (trailing?.kind !== "turn-duration") return next;
  if (next.blocks[next.blocks.length - 1]?.kind === "turn-duration") return next;
  return { ...next, blocks: [...next.blocks, trailing] };
}

function isHistoryViewBehindCurrentTurn(
  current: ChatViewState,
  nextFromHistory: ChatViewState,
): boolean {
  const currentComparable = current.blocks.filter(isHistoryComparableBlock);
  const nextComparable = nextFromHistory.blocks.filter(isHistoryComparableBlock);
  if (nextComparable.length >= currentComparable.length) return false;
  return hasAssistantContentAfterLatestUser(currentComparable);
}

function isHistoryComparableBlock(block: ChatBlock): boolean {
  return block.kind !== "turn-duration" && block.kind !== "plan-writing";
}

function hasAssistantContentAfterLatestUser(blocks: ChatBlock[]): boolean {
  for (let index = blocks.length - 1; index >= 0; index -= 1) {
    const block = blocks[index];
    if (block.kind === "user-text") return false;
    if (isAssistantHistoryContentBlock(block)) return true;
  }
  return false;
}

function isAssistantHistoryContentBlock(block: ChatBlock): boolean {
  if (block.kind === "tool" && block.hidden) return false;
  return (
    block.kind === "assistant-text" ||
    block.kind === "thinking" ||
    block.kind === "tool" ||
    block.kind === "plan" ||
    block.kind === "agent-status"
  );
}

function optimisticNextUserHistoryIndex(
  history: ChatMessage[],
  view: ChatViewState,
): number {
  let next = history.length;
  let latestUserBlockIndex = -1;
  let latestUserHistoryIndex = -1;
  view.blocks.forEach((block, index) => {
    if (block.kind !== "user-text" && block.kind !== "compaction-summary") return;
    next = Math.max(next, block.historyIndex + 1);
    if (block.kind === "user-text" && block.historyIndex >= latestUserHistoryIndex) {
      latestUserBlockIndex = index;
      latestUserHistoryIndex = block.historyIndex;
    }
  });
  if (
    latestUserBlockIndex >= 0 &&
    view.blocks
      .slice(latestUserBlockIndex + 1)
      .some(isAssistantHistoryContentBlock)
  ) {
    next = Math.max(next, latestUserHistoryIndex + 2);
  }
  return next;
}

type MentionState = {
  query: string;
  tokenStart: number;
  index: number;
};

type InlineMention = {
  path: string;
  absolutePath: string;
  name: string;
};

type ContextEstimateState =
  | { status: "loading"; estimate: ContextEstimate | null }
  | { status: "ready"; estimate: ContextEstimate }
  | { status: "error"; estimate: ContextEstimate | null };

type ConversationContextEstimateState = ContextEstimateState & {
  conversationId: string;
};

type SubAgentViewRecord = {
  id: string;
  agentId?: string;
  name: string;
  title: string;
  model?: ModelRef;
  history?: ChatMessage[];
  view: ChatViewState;
};

type TeamAgentRosterItem = SubAgentViewRecord & {
  color: string;
  status: "running" | "idle" | "finished" | "error" | "stopped";
  task?: string;
};

type PartialModeModelSelections = Partial<Record<AgentMode, ModeModelSelection>>;

const MENTION_MAX_RESULTS = 10;
const EMPTY_ACTIVE_TEAM_NAMES: ReadonlySet<string> = new Set();
const EMPTY_QUEUED_PROMPTS: QueuedPrompt[] = [];
const AUTO_COMPACT_OUTPUT_TOKEN_MAX = 32_000;
const GOAL_CONTINUATION_PROMPT =
  "Continue working toward the active goal. Do not repeat completed work. If the goal is now truly complete, audit it and call update_goal with status complete.";
const PROVIDERS_CHANGED_EVENT = "claakecode:providers-changed";
const TOOL_SETTINGS_CHANGED_EVENT = "claakecode:tool-settings-changed";
const AGENT_TEAMS_TOOL_NAME = "TeamRun";
const AGENT_TEAMS_DISABLED_TITLE = "Please activate Agent teams in settings.";
const IMPLEMENT_PLAN_PROMPT =
  "Implement completely this plan. Use the attached plan as the source of truth.";
const IMPLEMENT_PLAN_WITH_SWARM_PROMPT = `${IMPLEMENT_PLAN_PROMPT} Launch with agent swarm only, let them build.`;

function selectionForAvailableModels(
  selection: ModeModelSelection,
  availableModels: readonly ModelEntry[],
): ModeModelSelection {
  const entry =
    availableModels.find((model) => model.value === selection.model) ??
    availableModels[0];
  if (!entry) return selection;
  const supports1m = modelSupports1mContextBeta(entry.value);
  return {
    model: entry.value,
    thinking: entry.thinking.includes(selection.thinking)
      ? selection.thinking
      : entry.defaultThinking,
    use1mContext: supports1m ? selection.use1mContext : false,
  };
}

function serviceTierForSelection(
  selection: ModeModelSelection,
  availableModels: readonly ModelEntry[],
  fastEnabled: boolean,
): ServiceTier | null {
  const entry = availableModels.find((model) => model.value === selection.model);
  return fastEnabled && entry?.supportsFast ? "fast" : null;
}

function mergeModeSelections(
  base: ModeModelSelections,
  override: PartialModeModelSelections | undefined,
): ModeModelSelections {
  if (!override) return base;
  return {
    act: override.act ?? base.act,
    plan: override.plan ?? base.plan,
    goal: override.goal ?? base.goal,
  };
}

function sameModeSelection(
  a: ModeModelSelection | undefined,
  b: ModeModelSelection | undefined,
): boolean {
  return (
    a?.model === b?.model &&
    a?.thinking === b?.thinking &&
    a?.use1mContext === b?.use1mContext
  );
}

function thinkingLevelLabel(
  level: (typeof THINKING_LEVELS)[number] | undefined,
  model: ModelEntry | null,
): string | undefined {
  if (!level) return undefined;
  if (model?.provider === "kimi" && level.value !== "off") return "Thinking";
  return level.label;
}

function loadFastServiceTierPreference(): boolean {
  try {
    return window.localStorage.getItem(FAST_SERVICE_TIER_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

function saveFastServiceTierPreference(enabled: boolean) {
  try {
    if (enabled) {
      window.localStorage.setItem(FAST_SERVICE_TIER_STORAGE_KEY, "1");
    } else {
      window.localStorage.removeItem(FAST_SERVICE_TIER_STORAGE_KEY);
    }
  } catch {
    // Preference persistence is best-effort.
  }
}

export type ExternalDropFeed = {
  subscribe(handler: (attachments: Attachment[]) => void): () => void;
  subscribeDrag(handler: (active: boolean) => void): () => void;
};

export function ChatPane({
  workspacePath,
  conversationId,
  activeModel,
  modeModelSettings,
  streamingModel,
  planWorkflow,
  goalWorkflow,
  isStreaming,
  history,
  subscribeEvents,
  onSend,
  onCompact,
  onModeChange,
  onModelPreferenceChange,
  onImplementPlanFresh,
  onStop,
  onOpenFile,
  onOpenSettings,
  externalDrops,
  dropZoneRef,
}: Props) {
  const conversationViewsRef = useRef<Map<string, ChatViewState>>(new Map());
  const composerDraftsRef = useRef<Map<string, ComposerDraft>>(new Map());
  const [view, setView] = useState<ChatViewState>(() => {
    const initial = initialStateFromHistory(history);
    return isStreaming ? beginTurn(initial) : initial;
  });
  const viewRef = useRef(view);
  const appliedHistoryRef = useRef(history);
  const viewConversationIdRef = useRef(conversationId);
  const [subAgentViews, setSubAgentViews] = useState<
    Map<string, SubAgentViewRecord>
  >(() => subAgentViewsFromHistory(history));
  const subAgentViewsRef = useRef(subAgentViews);
  const subAgentViewsByConversationRef = useRef<
    Map<string, Map<string, SubAgentViewRecord>>
  >(new Map([[conversationId, subAgentViews]]));
  const [activeTeamNamesByConversation, setActiveTeamNamesByConversation] =
    useState<Map<string, Set<string>>>(() => new Map());
  const [promptQueuesByConversation, setPromptQueuesByConversation] = useState<
    Map<string, QueuedPrompt[]>
  >(() => new Map());
  const [editingQueuedPrompt, setEditingQueuedPrompt] =
    useState<EditingQueuedPrompt | null>(null);
  const [activeSubAgentId, setActiveSubAgentId] = useState<string | null>(null);
  const activeSubAgentIdRef = useRef<string | null>(null);
  const [autoCloseSubAgentId, setAutoCloseSubAgentId] = useState<string | null>(null);
  const [text, setText] = useState("");
  const [attachments, setAttachments] = useState<Attachment[]>([]);
  const [dropActive, setDropActive] = useState(false);
  const [optimisticModeSelectionsByConversation, setOptimisticModeSelectionsByConversation] =
    useState<Map<string, PartialModeModelSelections>>(() => new Map());
  const [mode, setMode] = useState<AgentMode>(() =>
    planWorkflow.status !== "idle"
      ? "plan"
      : goalWorkflow.status === "active"
        ? "goal"
        : "act",
  );
  const [rewriteState, setRewriteState] = useState<RewriteState | null>(null);
  const [compactInstructionOpen, setCompactInstructionOpen] = useState(false);
  const [compactInstruction, setCompactInstruction] = useState("");
  const rewindFileChanges = useMemo(
    () =>
      rewriteState
        ? aggregateFileChanges(
            fileChangesAfterHistoryIndex(history, rewriteState.historyIndex),
          )
        : [],
    [history, rewriteState],
  );
  const [modelOpen, setModelOpen] = useState(false);
  const [thinkingOpen, setThinkingOpen] = useState(false);
  const [modeOpen, setModeOpen] = useState(false);
  const [fastServiceTierEnabled, setFastServiceTierEnabled] = useState(
    loadFastServiceTierPreference,
  );
  const [configuredProviders, setConfiguredProviders] = useState<string[]>([]);
  const [openRouterModels, setOpenRouterModels] = useState<OpenRouterModel[]>([]);
  const [agentTeamsEnabled, setAgentTeamsEnabled] = useState(false);
  const modelRef = useRef<HTMLDivElement | null>(null);
  const thinkingRef = useRef<HTMLDivElement | null>(null);
  const modeRef = useRef<HTMLDivElement | null>(null);
  const composerRef = useRef<HTMLDivElement | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const [composerHeight, setComposerHeight] = useState<number | null>(null);
  const composerResizeStateRef = useRef<
    { startY: number; startHeight: number } | null
  >(null);
  const handleComposerResizeMouseDown = useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      if (event.button !== 0) return;
      const ta = textareaRef.current;
      if (!ta) return;
      event.preventDefault();
      const startHeight =
        composerHeight ?? Math.round(ta.getBoundingClientRect().height);
      composerResizeStateRef.current = {
        startY: event.clientY,
        startHeight,
      };
      const previousCursor = document.body.style.cursor;
      const previousUserSelect = document.body.style.userSelect;
      document.body.style.cursor = "ns-resize";
      document.body.style.userSelect = "none";
      const handleMove = (ev: MouseEvent) => {
        const state = composerResizeStateRef.current;
        if (!state) return;
        const delta = state.startY - ev.clientY; // drag up = grow
        const next = Math.max(36, Math.min(800, state.startHeight + delta));
        setComposerHeight(next);
      };
      const handleUp = () => {
        composerResizeStateRef.current = null;
        document.body.style.cursor = previousCursor;
        document.body.style.userSelect = previousUserSelect;
        window.removeEventListener("mousemove", handleMove);
        window.removeEventListener("mouseup", handleUp);
      };
      window.addEventListener("mousemove", handleMove);
      window.addEventListener("mouseup", handleUp);
    },
    [composerHeight],
  );
  const handleComposerResizeDoubleClick = useCallback(() => {
    setComposerHeight(null);
  }, []);
  const compactInstructionInputRef = useRef<HTMLInputElement | null>(null);
  const compactPopoverRef = useRef<HTMLDivElement | null>(null);
  const pendingCaretRef = useRef<number | null>(null);
  const mentionLoadingRef = useRef(false);
  const [mentionFiles, setMentionFiles] = useState<WorkspaceEntry[] | null>(
    null,
  );
  const [mention, setMention] = useState<MentionState | null>(null);
  const [inlineMentions, setInlineMentions] = useState<InlineMention[]>([]);
  const overlayRef = useRef<HTMLDivElement | null>(null);
  const overlayInnerRef = useRef<HTMLDivElement | null>(null);
  const planWritingRef = useRef<{ conversationId: string; placeholderId: string } | null>(null);
  const pendingPlanWriteModeRef = useRef<"update" | null>(null);
  const dequeueInFlightRef = useRef<string | null>(null);
  const blockedQueueItemIdsRef = useRef<Set<string>>(new Set());
  const contextEstimateConversationIdRef = useRef(conversationId);
  const contextEstimateSignatureRef = useRef<string | null>(null);
  const autoCompactAttemptKeysRef = useRef<Set<string>>(new Set());
  const goalContinuationKeysRef = useRef<Set<string>>(new Set());
  const [contextEstimate, setContextEstimate] =
    useState<ConversationContextEstimateState>({
      conversationId,
      status: "loading",
      estimate: null,
    });
  const subAgentContextEstimateIdRef = useRef<string | null>(null);
  const [subAgentContextEstimate, setSubAgentContextEstimate] =
    useState<ContextEstimateState>({
      status: "loading",
      estimate: null,
    });

  const [previewImage, setPreviewImage] = useState<string | null>(null);

  useEffect(() => {
    activeSubAgentIdRef.current = activeSubAgentId;
  }, [activeSubAgentId]);

  useLayoutEffect(() => {
    contextEstimateConversationIdRef.current = conversationId;
    contextEstimateSignatureRef.current = null;
    setContextEstimate((previous) =>
      previous.conversationId === conversationId
        ? previous
        : { conversationId, status: "loading", estimate: null },
    );
  }, [conversationId]);

  useEffect(() => {
    if (!previewImage) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setPreviewImage(null);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [previewImage]);

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

  useEffect(() => {
    if (!modelOpen) return;
    void loadConfiguredProviders();
  }, [modelOpen, loadConfiguredProviders]);

  useEffect(() => {
    const refresh = () => void loadConfiguredProviders();
    window.addEventListener("focus", refresh);
    window.addEventListener(PROVIDERS_CHANGED_EVENT, refresh);
    return () => {
      window.removeEventListener("focus", refresh);
      window.removeEventListener(PROVIDERS_CHANGED_EVENT, refresh);
    };
  }, [loadConfiguredProviders]);

  const loadAgentTeamsEnabled = useCallback(async () => {
    try {
      const settings = await api.listToolSettings(workspacePath);
      setAgentTeamsEnabled(
        settings.tools.some(
          (tool) => tool.name === AGENT_TEAMS_TOOL_NAME && tool.enabled,
        ),
      );
    } catch {
      setAgentTeamsEnabled(false);
    }
  }, [workspacePath]);

  useEffect(() => {
    void loadAgentTeamsEnabled();
  }, [loadAgentTeamsEnabled]);

  useEffect(() => {
    const refresh = () => void loadAgentTeamsEnabled();
    window.addEventListener(TOOL_SETTINGS_CHANGED_EVENT, refresh);
    return () => window.removeEventListener(TOOL_SETTINGS_CHANGED_EVENT, refresh);
  }, [loadAgentTeamsEnabled]);

  const allModels = useMemo(
    () => modelsWithOpenRouter(openRouterModels),
    [openRouterModels],
  );
  const availableModels = useMemo(
    () => availableModelsForProviders(configuredProviders, openRouterModels),
    [configuredProviders, openRouterModels],
  );

  const baseModeSelections = useMemo(
    () => selectionsFromSettings(modeModelSettings, activeModel),
    [activeModel, modeModelSettings],
  );
  const optimisticModeSelections = optimisticModeSelectionsByConversation.get(conversationId);
  const modeSelections = useMemo(
    () => mergeModeSelections(baseModeSelections, optimisticModeSelections),
    [baseModeSelections, optimisticModeSelections],
  );

  const planWorkflowActive = planWorkflow.status !== "idle";
  const goalWorkflowActive = goalWorkflow.status === "active";
  const effectiveMode: AgentMode = planWorkflowActive
    ? "plan"
    : goalWorkflowActive
      ? "goal"
      : mode;
  const selectorLocked = isStreaming || view.status === "streaming";
  const rawCurrentSelection = selectorLocked
    ? selectionFromRef(streamingModel ?? activeModel)
    : modeSelections[effectiveMode] ?? selectionFromRef(activeModel);
  const currentSelection = selectorLocked
    ? rawCurrentSelection
    : selectionForAvailableModels(rawCurrentSelection, availableModels);
  const model = currentSelection.model;
  const thinking = currentSelection.thinking;
  const use1mContext = currentSelection.use1mContext;
  const supports1mContext = modelSupports1mContextBeta(model);
  const modelEntry = availableModels.find((m) => m.value === model) ?? null;
  const displayModelEntry =
    modelEntry ?? allModels.find((m) => m.value === model) ?? null;
  const availableThinking = modelEntry
    ? THINKING_LEVELS.filter((l) => modelEntry.thinking.includes(l.value))
    : [];
  const fastSupported = Boolean(modelEntry?.supportsFast);
  const serviceTier = serviceTierForSelection(
    currentSelection,
    availableModels,
    fastServiceTierEnabled,
  );
  // Surface a clear "connect a provider" affordance when nothing is wired up
  // yet. The selectors stay hidden in that case — there's nothing meaningful
  // to pick from until the user signs into at least one provider.
  const noProvidersConfigured =
    !selectorLocked && configuredProviders.length === 0;
  const thinkingLabel =
    thinkingLevelLabel(
      THINKING_LEVELS.find((l) => l.value === thinking),
      displayModelEntry,
    ) ??
    "Medium";
  const modeEntry = MODES.find((entry) => entry.value === effectiveMode) ?? MODES[0];
  const composerAttachments = useMemo(
    () => collectComposerAttachments(attachments, inlineMentions),
    [attachments, inlineMentions],
  );
  const currentModelRef = useMemo(
    () =>
      modelRefWithUse1mContext(modelRefFromId(model), supports1mContext && use1mContext),
    [model, supports1mContext, use1mContext],
  );
  const queuedPrompts =
    promptQueuesByConversation.get(conversationId) ?? EMPTY_QUEUED_PROMPTS;
  const visibleContextEstimate: ContextEstimateState =
    contextEstimate.conversationId === conversationId
      ? contextEstimate
      : { status: "loading", estimate: null };

  const setSubAgentViewsForConversation = useCallback(
    (
      cid: string,
      updater: (
        current: Map<string, SubAgentViewRecord>,
      ) => Map<string, SubAgentViewRecord>,
    ) => {
      if (cid === viewConversationIdRef.current) {
        setSubAgentViews((current) => {
          const next = updater(current);
          subAgentViewsRef.current = next;
          subAgentViewsByConversationRef.current.set(cid, next);
          return next;
        });
        return;
      }

      const current =
        subAgentViewsByConversationRef.current.get(cid) ??
        new Map<string, SubAgentViewRecord>();
      const next = updater(current);
      subAgentViewsByConversationRef.current.set(cid, next);
    },
    [],
  );

  useLayoutEffect(() => {
    subAgentViewsRef.current = subAgentViews;
    subAgentViewsByConversationRef.current.set(
      viewConversationIdRef.current,
      subAgentViews,
    );
  }, [subAgentViews]);

  useEffect(() => {
    viewRef.current = view;
    conversationViewsRef.current.set(viewConversationIdRef.current, view);
  }, [view]);

  useLayoutEffect(() => {
    const previousConversationId = viewConversationIdRef.current;
    if (previousConversationId !== conversationId) {
      conversationViewsRef.current.set(previousConversationId, viewRef.current);
      subAgentViewsByConversationRef.current.set(
        previousConversationId,
        subAgentViewsRef.current,
      );
      composerDraftsRef.current.set(
        previousConversationId,
        buildComposerDraft(text, attachments, inlineMentions, editingQueuedPrompt),
      );
      viewConversationIdRef.current = conversationId;
    }
    const composerDraft = composerDraftsRef.current.get(conversationId);
    const cached = conversationViewsRef.current.get(conversationId);
    if (cached && isStreaming) {
      const next: ChatViewState =
        cached.status === "streaming"
          ? cached
          : beginTurn(cached);
      appliedHistoryRef.current = history;
      viewRef.current = next;
      conversationViewsRef.current.set(conversationId, next);
      setView(next);
    } else {
      const next = initialStateFromHistory(history);
      const nextView: ChatViewState = isStreaming
        ? beginTurn(next)
        : next;
      appliedHistoryRef.current = history;
      viewRef.current = nextView;
      conversationViewsRef.current.set(conversationId, nextView);
      setView(nextView);
    }
    const storedSubAgentViews = subAgentViewsFromHistory(history);
    const cachedSubAgentViews =
      subAgentViewsByConversationRef.current.get(conversationId);
    const nextSubAgentViews = cachedSubAgentViews
      ? mergeSubAgentViews(cachedSubAgentViews, storedSubAgentViews)
      : storedSubAgentViews;
    subAgentViewsRef.current = nextSubAgentViews;
    subAgentViewsByConversationRef.current.set(conversationId, nextSubAgentViews);
    setSubAgentViews(nextSubAgentViews);
    setActiveSubAgentId(null);
    setAutoCloseSubAgentId(null);
    subAgentContextEstimateIdRef.current = null;
    setSubAgentContextEstimate({ status: "loading", estimate: null });
    planWritingRef.current = null;
    pendingPlanWriteModeRef.current = null;
    setText(composerDraft?.text ?? "");
    setAttachments(cloneComposerAttachments(composerDraft?.attachments ?? []));
    setInlineMentions(cloneInlineMentions(composerDraft?.inlineMentions ?? []));
    setRewriteState(null);
    setCompactInstructionOpen(false);
    setCompactInstruction("");
    setEditingQueuedPrompt(composerDraft?.editingQueuedPrompt ?? null);
    setMention(null);
    setMode(
      planWorkflow.status !== "idle"
        ? "plan"
        : goalWorkflow.status === "active"
          ? "goal"
          : "act",
    );
    // Re-init only when switching conversations; within the same
    // conversation the streaming reducer is authoritative (history reloads
    // after turn_finished don't always preserve thinking parts).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [conversationId]);

  useEffect(() => {
    if (planWorkflow.status !== "idle") {
      setMode("plan");
    } else if (goalWorkflow.status === "active") {
      setMode("goal");
    }
  }, [goalWorkflow.status, planWorkflow.status]);

  useEffect(() => {
    if (view.status === "streaming" || isStreaming) return;
    if (appliedHistoryRef.current === history) return;
    const nextFromHistory = preserveTrailingTurnDuration(
      initialStateFromHistory(history),
      viewRef.current,
    );
    const current = viewRef.current;
    if (isHistoryViewBehindCurrentTurn(current, nextFromHistory)) {
      appliedHistoryRef.current = history;
      return;
    }
    const next =
      current.lastError && current.status === "stopped"
        ? {
            ...nextFromHistory,
            status: current.status,
            lastError: current.lastError,
          }
        : nextFromHistory;
    appliedHistoryRef.current = history;
    viewRef.current = next;
    conversationViewsRef.current.set(conversationId, next);
    setView(next);
    const storedSubAgentViews = subAgentViewsFromHistory(history);
    setSubAgentViewsForConversation(conversationId, (current) =>
      mergeSubAgentViews(current, storedSubAgentViews),
    );
  }, [conversationId, history, isStreaming, setSubAgentViewsForConversation, view.status]);

  useEffect(() => {
    if (!isStreaming) return;
    setView((prev) =>
      prev.status === "idle" ? beginTurn(prev) : prev,
    );
  }, [isStreaming]);

  useEffect(() => {
    setOptimisticModeSelectionsByConversation((current) => {
      const override = current.get(conversationId);
      if (!override) return current;

      let changed = false;
      const nextOverride: PartialModeModelSelections = { ...override };
      for (const targetMode of MODES.map((entry) => entry.value)) {
        if (sameModeSelection(nextOverride[targetMode], baseModeSelections[targetMode])) {
          delete nextOverride[targetMode];
          changed = true;
        }
      }
      if (!changed) return current;

      const next = new Map(current);
      if (Object.keys(nextOverride).length === 0) {
        next.delete(conversationId);
      } else {
        next.set(conversationId, nextOverride);
      }
      return next;
    });
  }, [baseModeSelections, conversationId]);

  useEffect(() => {
    if (!selectorLocked) return;
    setModelOpen(false);
    setThinkingOpen(false);
    setModeOpen(false);
  }, [selectorLocked]);

  useEffect(() => {
    setInlineMentions((prev) => {
      const next = prev.filter((m) => text.includes(`@${m.path}`));
      return next.length === prev.length ? prev : next;
    });
  }, [text]);

  useEffect(() => {
    setMentionFiles(null);
    mentionLoadingRef.current = false;
  }, [workspacePath]);

  useLayoutEffect(() => {
    const target = pendingCaretRef.current;
    if (target === null) return;
    pendingCaretRef.current = null;
    const ta = textareaRef.current;
    if (!ta) return;
    ta.focus();
    ta.setSelectionRange(target, target);
  }, [text]);

  // Mirror the textarea's internal scroll offset onto the overlay so that the
  // (invisible) caret drawn by the textarea always sits exactly on top of the
  // glyphs rendered by the overlay. We translate an inner wrapper instead of
  // writing to `overlay.scrollTop` because the overlay uses `overflow: hidden`
  // and programmatic scrolling on a hidden-overflow element is unreliable
  // across engines, and because a transform is always honoured even when the
  // overlay's scrollable area is zero.
  const syncOverlayScroll = useCallback(() => {
    const ta = textareaRef.current;
    const inner = overlayInnerRef.current;
    if (!ta || !inner) return;
    const x = -ta.scrollLeft;
    const y = -ta.scrollTop;
    inner.style.transform = `translate(${x}px, ${y}px)`;
  }, []);

  useLayoutEffect(() => {
    const ta = textareaRef.current;
    const overlay = overlayRef.current;
    if (!ta || !overlay) return;
    syncOverlayScroll();
    // Re-sync once on the next frame: when text shrinks, the browser may
    // clamp `ta.scrollTop` *after* the React commit, so a single sync misses
    // the final value and the caret ends up above the visible text.
    const raf = requestAnimationFrame(() => syncOverlayScroll());
    return () => cancelAnimationFrame(raf);
  }, [text, inlineMentions]);

  const ensureMentionFiles = useCallback(async () => {
    if (mentionFiles !== null || mentionLoadingRef.current) return;
    mentionLoadingRef.current = true;
    try {
      const files = await api.listAllFiles(workspacePath);
      setMentionFiles(files);
    } catch (err) {
      console.error(err);
      setMentionFiles([]);
    } finally {
      mentionLoadingRef.current = false;
    }
  }, [workspacePath, mentionFiles]);

  const detectMention = useCallback(
    (value: string, caret: number) => {
      let i = caret - 1;
      while (i >= 0) {
        const ch = value[i];
        if (ch === "@") {
          const prev = i === 0 ? "" : value[i - 1];
          if (prev === "" || /\s/.test(prev)) {
            const query = value.slice(i + 1, caret);
            if (!/\s/.test(query) && query.length <= 80) {
              setMention((prev) =>
                prev && prev.tokenStart === i && prev.query === query
                  ? prev
                  : { tokenStart: i, query, index: 0 },
              );
              void ensureMentionFiles();
              return;
            }
          }
          break;
        }
        if (/\s/.test(ch)) break;
        i--;
      }
      setMention(null);
    },
    [ensureMentionFiles],
  );

  const matches = useMemo<WorkspaceEntry[]>(() => {
    if (!mention) return [];
    const files = mentionFiles ?? [];
    const q = mention.query.toLowerCase();
    if (!q) return files.slice(0, MENTION_MAX_RESULTS);
    const scored: { file: WorkspaceEntry; score: number }[] = [];
    for (const file of files) {
      const name = file.name.toLowerCase();
      const path = file.relativePath.toLowerCase();
      let score = 0;
      if (name === q) score = 1000;
      else if (name.startsWith(q)) score = 800;
      else if (name.includes(q)) score = 600;
      else if (path.includes(q)) score = 300;
      else continue;
      score -= path.length * 0.1;
      scored.push({ file, score });
    }
    scored.sort((a, b) => b.score - a.score);
    return scored.slice(0, MENTION_MAX_RESULTS).map((s) => s.file);
  }, [mention, mentionFiles]);

  useEffect(() => {
    if (!mention) return;
    if (matches.length === 0) {
      if (mention.index !== 0) {
        setMention({ ...mention, index: 0 });
      }
      return;
    }
    if (mention.index >= matches.length) {
      setMention({ ...mention, index: matches.length - 1 });
    }
  }, [mention, matches]);

  const selectMention = useCallback(
    (file: WorkspaceEntry) => {
      if (!mention) return;
      const ta = textareaRef.current;
      const tokenEnd = ta ? ta.selectionStart : mention.tokenStart + 1 + mention.query.length;
      const insertion = `@${file.relativePath} `;
      const next =
        text.slice(0, mention.tokenStart) + insertion + text.slice(tokenEnd);
      pendingCaretRef.current = mention.tokenStart + insertion.length;
      setText(next);
      setMention(null);
      setInlineMentions((prev) => {
        if (prev.some((m) => m.path === file.relativePath)) return prev;
        return [
          ...prev,
          {
            path: file.relativePath,
            absolutePath: file.absolutePath || file.relativePath,
            name: file.name,
          },
        ];
      });
    },
    [mention, text],
  );

  useEffect(() => {
    if (!rewriteState) return;
    const onDoc = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (!target) return;
      if (composerRef.current?.contains(target)) return;
      if (
        target instanceof Element &&
        target.closest('[data-rewindable="true"]')
      ) {
        return;
      }
      setText(rewriteState.originalText);
      setAttachments(rewriteState.originalAttachments);
      setRewriteState(null);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [rewriteState]);

  useEffect(() => {
    if (!compactInstructionOpen) return;
    window.setTimeout(() => compactInstructionInputRef.current?.focus(), 0);
  }, [compactInstructionOpen]);

  useEffect(() => {
    if (!compactInstructionOpen) return;
    const onDoc = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (!target) return;
      if (compactPopoverRef.current?.contains(target)) return;
      setCompactInstructionOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [compactInstructionOpen]);

  const reduceEventForConversation = useCallback(
    (
      cid: string,
      current: ChatViewState,
      event: AgentEvent,
    ): ChatViewState => {
      const planWriting =
        planWritingRef.current?.conversationId === cid
          ? planWritingRef.current
          : null;
      if (
        planWriting &&
        (event.type === "text_started" ||
          event.type === "text_chunk" ||
          event.type === "text_finished")
      ) {
        if (event.type === "text_chunk") {
          return {
            ...current,
            blocks: current.blocks.map((block) =>
              block.kind === "plan-writing" &&
              block.id === planWriting.placeholderId
                ? { ...block, text: block.text + event.delta }
                : block,
            ),
          };
        }
        return current;
      }
      if (
        planWriting &&
        (event.type === "turn_finished" ||
          event.type === "interrupted" ||
          event.type === "error")
      ) {
        planWritingRef.current = null;
        pendingPlanWriteModeRef.current = null;
        if (event.type !== "turn_finished") {
          return applyEvent(
            {
              ...current,
              blocks: current.blocks.filter(
                (block) =>
                  !(
                    block.kind === "plan-writing" &&
                    block.id === planWriting.placeholderId
                  ),
              ),
            },
            event,
          );
        }
      }
      return applyEvent(current, event);
    },
    [],
  );

  const applySubAgentEvent = useCallback(
    (cid: string, event: AgentEvent) => {
      if (event.type !== "sub_agent_event") return;
      setSubAgentViewsForConversation(cid, (current) =>
        applySubAgentEventToViews(current, event),
      );
    },
    [setSubAgentViewsForConversation],
  );

  const applySubAgentToolMeta = useCallback(
    (cid: string, event: AgentEvent) => {
      if (event.type !== "tool_finished") return;
      const stored = subAgentViewsFromToolMeta(event.id, event.meta);
      if (stored.size === 0) return;
      setSubAgentViewsForConversation(cid, (current) =>
        mergeSubAgentViews(current, stored),
      );
    },
    [setSubAgentViewsForConversation],
  );

  const applyTokenUsageEvent = useCallback(
    (cid: string, event: AgentEvent) => {
      if (event.type === "token_usage") {
        const estimate = contextEstimateFromTokenUsageEvent(event);
        if (cid === contextEstimateConversationIdRef.current) {
          setContextEstimate({ conversationId: cid, status: "ready", estimate });
        }
        return;
      }

      if (event.type !== "sub_agent_event") return;
      const inner = event.event;
      if (inner.type !== "token_usage") return;
      if (cid !== contextEstimateConversationIdRef.current) return;
      if (!subAgentEventMatchesActiveView(event, activeSubAgentIdRef.current)) {
        return;
      }
      setSubAgentContextEstimate({
        status: "ready",
        estimate: contextEstimateFromTokenUsageEvent(inner),
      });
    },
    [],
  );

  const applyEventToConversationView = useCallback(
    (cid: string, event: AgentEvent) => {
      const viewBeforeEvent =
        cid === viewConversationIdRef.current
          ? viewRef.current
          : conversationViewsRef.current.get(cid) ?? initialStateFromHistory([]);
      setActiveTeamNamesByConversation((current) =>
        updateActiveTeamNamesForEvent(current, cid, event, viewBeforeEvent),
      );
      if (event.type === "sub_agent_event") applySubAgentEvent(cid, event);
      if (event.type === "tool_finished") applySubAgentToolMeta(cid, event);
      applyTokenUsageEvent(cid, event);
      if (cid === viewConversationIdRef.current) {
        setView((prev) => {
          const next = reduceEventForConversation(cid, prev, event);
          viewRef.current = next;
          conversationViewsRef.current.set(cid, next);
          return next;
        });
        return;
      }

      const current =
        conversationViewsRef.current.get(cid) ?? initialStateFromHistory([]);
      const next = reduceEventForConversation(cid, current, event);
      conversationViewsRef.current.set(cid, next);
    },
    [
      applySubAgentEvent,
      applySubAgentToolMeta,
      applyTokenUsageEvent,
      reduceEventForConversation,
    ],
  );

  useEffect(() => {
    const unsubscribe = subscribeEvents((cid, event) => {
      applyEventToConversationView(cid, event);
    });
    return unsubscribe;
  }, [subscribeEvents, applyEventToConversationView]);

  useEffect(() => {
    const unsubscribe = externalDrops.subscribe((incoming) => {
      setAttachments((prev) => mergeAttachments(prev, incoming));
      setDropActive(false);
    });
    return unsubscribe;
  }, [externalDrops]);

  useEffect(() => {
    const unsubscribe = externalDrops.subscribeDrag((active) => {
      setDropActive(active);
    });
    return unsubscribe;
  }, [externalDrops]);

  useEffect(() => {
    if (activeSubAgentId !== null) {
      setDropActive(false);
    }
  }, [activeSubAgentId]);

  const bodyRef = useRef<HTMLDivElement | null>(null);
  const bodyContentRef = useRef<HTMLDivElement | null>(null);
  const scrollAnimationRef = useRef<number | null>(null);
  const autoScrollingRef = useRef(false);
  const stickToBottomRef = useRef(true);
  const pendingForceScrollRef = useRef(true);
  const [sendTick, setSendTick] = useState(0);
  const scrollViewStatus =
    activeSubAgentId !== null
      ? subAgentViews.get(activeSubAgentId)?.view.status ?? view.status
      : view.status;

  const stopAutoScroll = useCallback(() => {
    if (scrollAnimationRef.current !== null) {
      cancelAnimationFrame(scrollAnimationRef.current);
      scrollAnimationRef.current = null;
    }
    autoScrollingRef.current = false;
  }, []);

  const scheduleStickToBottom = useCallback(
    (options: { force?: boolean; animated?: boolean } = {}) => {
      const el = bodyRef.current;
      if (!el) return;
      if (options.force) stickToBottomRef.current = true;
      if (!stickToBottomRef.current) return;

      const target = () => Math.max(0, el.scrollHeight - el.clientHeight);
      const prefersReducedMotion = window.matchMedia(
        "(prefers-reduced-motion: reduce)",
      ).matches;
      const animated = options.animated && !prefersReducedMotion;

      if (!animated) {
        stopAutoScroll();
        autoScrollingRef.current = true;
        el.scrollTop = target();
        requestAnimationFrame(() => {
          autoScrollingRef.current = false;
        });
        return;
      }

      if (scrollAnimationRef.current !== null) return;
      autoScrollingRef.current = true;

      const tick = () => {
        const nextTarget = target();
        const distance = nextTarget - el.scrollTop;
        if (!stickToBottomRef.current) {
          stopAutoScroll();
          return;
        }
        if (Math.abs(distance) < 1) {
          el.scrollTop = nextTarget;
          scrollAnimationRef.current = null;
          autoScrollingRef.current = false;
          return;
        }
        const step = Math.sign(distance) * Math.max(1, Math.abs(distance) * 0.28);
        el.scrollTop += step;
        scrollAnimationRef.current = requestAnimationFrame(tick);
      };

      scrollAnimationRef.current = requestAnimationFrame(tick);
    },
    [stopAutoScroll],
  );

  useEffect(() => {
    const el = bodyRef.current;
    if (!el) return;
    const updateStickiness = () => {
      if (autoScrollingRef.current) return;
      stickToBottomRef.current =
        el.scrollHeight - el.scrollTop - el.clientHeight < 120;
    };
    const cancelOnUpwardWheel = (event: WheelEvent) => {
      if (event.deltaY >= 0) return;
      stopAutoScroll();
      stickToBottomRef.current = false;
    };
    updateStickiness();
    el.addEventListener("scroll", updateStickiness, { passive: true });
    el.addEventListener("wheel", cancelOnUpwardWheel, { passive: true });
    return () => {
      el.removeEventListener("scroll", updateStickiness);
      el.removeEventListener("wheel", cancelOnUpwardWheel);
    };
  }, [stopAutoScroll]);

  useLayoutEffect(() => {
    if (pendingForceScrollRef.current) {
      pendingForceScrollRef.current = false;
      stickToBottomRef.current = true;
      scheduleStickToBottom({ force: true, animated: false });
    } else {
      scheduleStickToBottom({ animated: scrollViewStatus === "streaming" });
    }
  }, [view, subAgentViews, activeSubAgentId, scrollViewStatus, scheduleStickToBottom]);

  useLayoutEffect(() => {
    const content = bodyContentRef.current;
    if (!content) return;
    const observer = new ResizeObserver(() => {
      scheduleStickToBottom({ animated: scrollViewStatus === "streaming" });
    });
    observer.observe(content);
    return () => observer.disconnect();
  }, [scrollViewStatus, scheduleStickToBottom]);

  useLayoutEffect(() => {
    pendingForceScrollRef.current = true;
    stickToBottomRef.current = true;
    scheduleStickToBottom({ force: true, animated: false });
  }, [conversationId, activeSubAgentId, scheduleStickToBottom]);

  useLayoutEffect(() => {
    if (sendTick === 0) return;
    scheduleStickToBottom({ force: true, animated: false });
  }, [sendTick, scheduleStickToBottom]);

  useEffect(() => stopAutoScroll, [stopAutoScroll]);

  useEffect(() => {
    let cancelled = false;
    const sameConversation = contextEstimate.conversationId === conversationId;
    const previous = sameConversation ? contextEstimate.estimate : null;
    const estimateSignature = autoCompactHistorySignature(history);
    contextEstimateConversationIdRef.current = conversationId;
    contextEstimateSignatureRef.current = null;

    if (view.status === "streaming") {
      if (!sameConversation) {
        setContextEstimate({ conversationId, status: "loading", estimate: null });
      }
      return () => {
        cancelled = true;
      };
    }
    if (!modelEntry) {
      setContextEstimate({ conversationId, status: "error", estimate: previous });
      return () => {
        cancelled = true;
      };
    }

    setContextEstimate({ conversationId, status: "loading", estimate: previous });
    const timer = window.setTimeout(() => {
      void api
        .estimateContext(
          workspacePath,
          conversationId,
          text,
          composerAttachments,
          currentModelRef,
          thinking,
          effectiveMode,
          rewriteState?.historyIndex,
        )
        .then((estimate) => {
          if (
            !cancelled &&
            contextEstimateConversationIdRef.current === conversationId
          ) {
            contextEstimateSignatureRef.current = estimateSignature;
            setContextEstimate({ conversationId, status: "ready", estimate });
          }
        })
        .catch(() => {
          if (
            !cancelled &&
            contextEstimateConversationIdRef.current === conversationId
          ) {
            setContextEstimate({ conversationId, status: "error", estimate: previous });
          }
        });
    }, 700);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
    // We intentionally key on history identity so the meter refreshes after
    // stored turns reload.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    workspacePath,
    conversationId,
    history,
    text,
    composerAttachments,
    model,
    modelEntry,
    thinking,
    effectiveMode,
    rewriteState?.historyIndex,
    view.status,
  ]);

  const handleSend = useCallback(async () => {
    if (!modelEntry) return;
    const currentAttachments = composerAttachments;
    const value = text.trim() || attachmentOnlyMessage(currentAttachments);
    if (!value && currentAttachments.length === 0) {
      return;
    }
    if (view.status === "streaming" || isStreaming) {
      const editing =
        editingQueuedPrompt?.conversationId === conversationId
          ? editingQueuedPrompt
          : null;
      const queuedPrompt = buildQueuedPrompt({
        id: editing?.id,
        text: value,
        attachments: currentAttachments,
        model: currentModelRef,
        thinking,
        mode: effectiveMode,
        serviceTier,
        createdAtMs: editing?.createdAtMs,
      });
      blockedQueueItemIdsRef.current.delete(queuedPrompt.id);
      setPromptQueuesByConversation((current) =>
        updatePromptQueue(current, conversationId, (queue) =>
          insertQueuedPrompt(queue, queuedPrompt, editing?.index),
        ),
      );
      setText("");
      setAttachments([]);
      setInlineMentions([]);
      setRewriteState(null);
      setEditingQueuedPrompt(null);
      setSendTick((t) => t + 1);
      return;
    }
    const rewriteFromHistoryIndex = rewriteState?.historyIndex;
    const revertWorkspaceChanges = rewriteState?.revertWorkspaceChanges ?? true;
    const nextHistoryIndex = rewriteFromHistoryIndex ?? history.length;
    setView((prev) => {
      const base =
        rewriteFromHistoryIndex === undefined
          ? prev
          : initialStateFromHistory(history.slice(0, rewriteFromHistoryIndex));
      return beginTurn(
        appendUserMessage(
          base,
          value,
          nextHistoryIndex,
          currentAttachments,
        ),
      );
    });
    setText("");
    setAttachments([]);
    setInlineMentions([]);
    setRewriteState(null);
    setSendTick((t) => t + 1);
    try {
      await onSend(
        value,
        currentAttachments,
        currentModelRef,
        thinking,
        effectiveMode,
        serviceTier,
        rewriteFromHistoryIndex,
        undefined,
        undefined,
        revertWorkspaceChanges,
      );
    } catch (err) {
      setView((prev) => ({
        ...prev,
        status: "stopped",
        streamPhase: "idle",
        lastError: String(err),
        turnStartedAtMs: null,
      }));
    }
  }, [
    text,
    view.status,
    isStreaming,
    composerAttachments,
    conversationId,
    editingQueuedPrompt,
    history.length,
    history,
    onSend,
    rewriteState,
    model,
    modelEntry,
    thinking,
    effectiveMode,
    serviceTier,
  ]);

  useEffect(() => {
    if (view.status === "streaming" || isStreaming) return;
    if (activeSubAgentId !== null) return;
    if (queuedPrompts.length === 0) return;
    if (editingQueuedPrompt?.conversationId === conversationId) return;
    const nextPrompt = queuedPrompts[0];
    if (!nextPrompt) return;
    if (blockedQueueItemIdsRef.current.has(nextPrompt.id)) return;
    if (dequeueInFlightRef.current === nextPrompt.id) return;

    dequeueInFlightRef.current = nextPrompt.id;
    setPromptQueuesByConversation((current) =>
      updatePromptQueue(current, conversationId, (queue) =>
        queue.filter((prompt) => prompt.id !== nextPrompt.id),
      ),
    );
    setView((prev) =>
      beginTurn(
        appendUserMessage(
          prev,
          nextPrompt.text,
          optimisticNextUserHistoryIndex(history, prev),
          userAttachmentsFromQueue(nextPrompt.attachments),
        ),
      ),
    );
    setSendTick((t) => t + 1);
    void onSend(
      nextPrompt.text,
      nextPrompt.attachments,
      nextPrompt.model,
      nextPrompt.thinking,
      nextPrompt.mode,
      nextPrompt.serviceTier,
    )
      .catch((err) => {
        blockedQueueItemIdsRef.current.add(nextPrompt.id);
        setPromptQueuesByConversation((current) =>
          updatePromptQueue(current, conversationId, (queue) =>
            queue.some((prompt) => prompt.id === nextPrompt.id)
              ? queue
              : [nextPrompt, ...queue],
          ),
        );
        setView((prev) => ({
          ...prev,
          status: "stopped",
          streamPhase: "idle",
          lastError: String(err),
          turnStartedAtMs: null,
        }));
      })
      .finally(() => {
        if (dequeueInFlightRef.current === nextPrompt.id) {
          dequeueInFlightRef.current = null;
        }
      });
  }, [
    activeSubAgentId,
    conversationId,
    editingQueuedPrompt,
    history.length,
    isStreaming,
    onSend,
    queuedPrompts,
    view.status,
  ]);

  const compactDisabled =
    view.status === "streaming" ||
    activeSubAgentId !== null ||
    history.length === 0 ||
    !modelEntry;

  const runManualCompact = useCallback(
    async (instruction?: string) => {
      if (compactDisabled) {
        return;
      }
      setSendTick((t) => t + 1);
      try {
        await onCompact(currentModelRef, thinking, serviceTier, {
          continueAfter: false,
          instruction,
        });
        setCompactInstructionOpen(false);
        setCompactInstruction("");
      } catch (err) {
        setView((prev) => ({
          ...prev,
          status: "stopped",
          streamPhase: "idle",
          lastError: String(err),
          turnStartedAtMs: null,
        }));
      }
    },
    [compactDisabled, model, onCompact, serviceTier, thinking],
  );

  const handleCompact = useCallback(() => {
    if (compactDisabled) {
      return;
    }
    setCompactInstructionOpen((open) => !open);
  }, [compactDisabled]);

  const handleCompactInstructionSubmit = useCallback(async () => {
    const instruction = compactInstruction.trim();
    await runManualCompact(instruction || undefined);
  }, [compactInstruction, runManualCompact]);

  const handleCompactInstructionKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLInputElement>) => {
      if (event.key === "Enter") {
        event.preventDefault();
        void handleCompactInstructionSubmit();
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        setCompactInstructionOpen(false);
      }
    },
    [handleCompactInstructionSubmit],
  );

  useEffect(() => {
    if (compactDisabled && compactInstructionOpen) {
      setCompactInstructionOpen(false);
    }
  }, [compactDisabled, compactInstructionOpen]);

  useEffect(() => {
    if (contextEstimate.conversationId !== conversationId) return;
    if (contextEstimate.status !== "ready") return;
    if (contextEstimateSignatureRef.current !== autoCompactHistorySignature(history)) return;
    if (view.status === "streaming" || isStreaming) return;
    if (activeSubAgentId !== null) return;
    if (history.length === 0) return;
    if (!modelEntry) return;
    if (rewriteState !== null) return;
    if (text.trim() || composerAttachments.length > 0) return;
    if (!hasContentAfterLatestCompaction(history)) return;

    const estimate = contextEstimate.estimate;
    if (!estimate.exact) return;
    const compactWindow = autoCompactWindow(estimate);
    if (compactWindow <= 0) return;
    if (estimate.usedTokens < compactWindow) return;

    const key = `${conversationId}:${autoCompactHistorySignature(history)}`;
    if (autoCompactAttemptKeysRef.current.has(key)) return;
    autoCompactAttemptKeysRef.current.add(key);

    setSendTick((t) => t + 1);
    void onCompact(currentModelRef, thinking, serviceTier, {
      continueAfter: true,
    }).catch(
      (err) => {
        autoCompactAttemptKeysRef.current.delete(key);
        setView((prev) => ({
          ...prev,
          status: "stopped",
          streamPhase: "idle",
          lastError: String(err),
          turnStartedAtMs: null,
        }));
      },
    );
  }, [
    activeSubAgentId,
    composerAttachments.length,
    contextEstimate,
    conversationId,
    history,
    isStreaming,
    model,
    modelEntry,
    onCompact,
    rewriteState,
    serviceTier,
    text,
    thinking,
    view.status,
  ]);

  useEffect(() => {
    if (goalWorkflow.status !== "active") return;
    if (planWorkflow.status !== "idle") return;
    if (view.status === "streaming" || isStreaming) return;
    if (activeSubAgentId !== null) return;
    if (history.length === 0) return;
    if (rewriteState !== null) return;
    if (text.trim() || composerAttachments.length > 0) return;
    if (!hasContentAfterLatestCompaction(history)) return;
    if (contextEstimate.conversationId !== conversationId) return;
    if (contextEstimate.status !== "ready") return;
    if (contextEstimateSignatureRef.current !== autoCompactHistorySignature(history)) return;

    const estimate = contextEstimate.estimate;
    if (
      estimate.exact &&
      autoCompactWindow(estimate) > 0 &&
      estimate.usedTokens >= autoCompactWindow(estimate) &&
      hasContentAfterLatestCompaction(history)
    ) {
      return;
    }

    const key = `${conversationId}:${history.length}:${goalWorkflow.updatedAtMs}`;
    if (goalContinuationKeysRef.current.has(key)) return;
    goalContinuationKeysRef.current.add(key);

    const goalSelection = selectionForAvailableModels(
      modeSelections.goal ?? modeSelections.act ?? selectionFromRef(activeModel),
      availableModels,
    );
    const goalServiceTier = serviceTierForSelection(
      goalSelection,
      availableModels,
      fastServiceTierEnabled,
    );
    setView((prev) => beginTurn(prev));
    setSendTick((t) => t + 1);
    void onSend(
      GOAL_CONTINUATION_PROMPT,
      [],
      modelRefWithUse1mContext(
        modelRefFromId(goalSelection.model),
        modelSupports1mContextBeta(goalSelection.model) && goalSelection.use1mContext,
      ),
      goalSelection.thinking,
      "goal",
      goalServiceTier,
      undefined,
      undefined,
      "systemReminder",
    ).catch((err) => {
      goalContinuationKeysRef.current.delete(key);
      setView((prev) => ({
        ...prev,
        status: "stopped",
        streamPhase: "idle",
        lastError: String(err),
        turnStartedAtMs: null,
      }));
    });
  }, [
    activeModel,
    activeSubAgentId,
    availableModels,
    composerAttachments.length,
    contextEstimate,
    conversationId,
    fastServiceTierEnabled,
    goalWorkflow,
    history,
    isStreaming,
    modeSelections,
    onSend,
    planWorkflow.status,
    rewriteState,
    text,
    view.status,
  ]);

  const handleQuestionAnswer = useCallback(
    async (
      toolCallId: string,
      answers: QuestionAnswer[],
      options?: { stopQuestions?: boolean },
    ) => {
      if (!modelEntry) return;
      if (answers.length === 0) return;
      const placeholderId = options?.stopQuestions
        ? `plan-writing-${Date.now()}`
        : null;
      const placeholderLabel =
        placeholderId !== null
          ? pendingPlanWriteModeRef.current === "update"
            ? "Updating the plan"
            : "Writing plan"
          : null;
      if (placeholderId) {
        planWritingRef.current = { conversationId, placeholderId };
      }
      setView((prev) => {
        const next = beginTurn(prev);
        if (!placeholderId) return next;
        return {
          ...next,
          blocks: [
            ...next.blocks,
            {
              kind: "plan-writing",
              id: placeholderId,
              label: placeholderLabel ?? "Writing plan",
              text: "",
            },
          ],
        };
      });
      setSendTick((t) => t + 1);
      try {
        const answered = await api.answerQuestion(
          workspacePath,
          conversationId,
          toolCallId,
          answers,
          options?.stopQuestions === true,
        );
        if (!answered) {
          throw new Error("question is no longer waiting for an answer");
        }
      } catch (err) {
        if (placeholderId) {
          planWritingRef.current = null;
        }
        setView((prev) => ({
          ...prev,
          blocks: placeholderId
            ? prev.blocks.filter(
                (block) =>
                  !(
                    block.kind === "plan-writing" &&
                    block.id === placeholderId
                  ),
              )
            : prev.blocks,
          status: "stopped",
          streamPhase: "idle",
          lastError: String(err),
          turnStartedAtMs: null,
        }));
      }
    },
    [conversationId, modelEntry, workspacePath],
  );

  const persistModeSelection = useCallback(
    async (targetMode: AgentMode, next: ModeModelSelection) => {
      const targetConversationId = conversationId;
      const previous = modeSelections[targetMode];
      setOptimisticModeSelectionsByConversation((current) => {
        const currentOverride = current.get(targetConversationId) ?? {};
        const updatedOverride: PartialModeModelSelections = {
          ...currentOverride,
          [targetMode]: next,
        };
        const updated = new Map(current);
        updated.set(targetConversationId, updatedOverride);
        return updated;
      });
      try {
        await onModelPreferenceChange(
          targetMode,
          modelRefWithUse1mContext(
            modelRefFromId(next.model),
            modelSupports1mContextBeta(next.model) && next.use1mContext,
          ),
          next.thinking,
        );
      } catch (err) {
        setOptimisticModeSelectionsByConversation((current) => {
          const currentOverride = current.get(targetConversationId) ?? {};
          const updatedOverride: PartialModeModelSelections = {
            ...currentOverride,
            [targetMode]: previous,
          };
          const updated = new Map(current);
          updated.set(targetConversationId, updatedOverride);
          return updated;
        });
        setView((prev) => ({
          ...prev,
          status: "stopped",
          streamPhase: "idle",
          lastError: String(err),
          turnStartedAtMs: null,
        }));
      }
    },
    [conversationId, modeSelections, onModelPreferenceChange],
  );

  const handleModelSelect = useCallback(
    (nextModel: ModelId) => {
      if (selectorLocked) return;
      const nextEntry =
        availableModels.find((m) => m.value === nextModel) ?? availableModels[0];
      if (!nextEntry) return;
      const nextThinking = nextEntry.thinking.includes(thinking)
        ? thinking
        : nextEntry.defaultThinking;
      setModelOpen(false);
      void persistModeSelection(effectiveMode, {
        model: nextModel,
        thinking: nextThinking,
        use1mContext: modelSupports1mContextBeta(nextModel) ? use1mContext : false,
      });
    },
    [
      availableModels,
      effectiveMode,
      persistModeSelection,
      selectorLocked,
      thinking,
      use1mContext,
    ],
  );

  const handleThinkingSelect = useCallback(
    (nextThinking: ThinkingLevel) => {
      if (selectorLocked) return;
      setThinkingOpen(false);
      void persistModeSelection(effectiveMode, {
        model,
        thinking: nextThinking,
        use1mContext,
      });
    },
    [effectiveMode, model, persistModeSelection, selectorLocked, use1mContext],
  );

  const handle1mContextToggle = useCallback(
    (enabled: boolean) => {
      if (selectorLocked) return;
      if (!supports1mContext) return;
      void persistModeSelection(effectiveMode, {
        model,
        thinking,
        use1mContext: enabled,
      });
    },
    [
      effectiveMode,
      model,
      persistModeSelection,
      selectorLocked,
      supports1mContext,
      thinking,
    ],
  );

  const handleFastServiceTierToggle = useCallback(() => {
    if (selectorLocked || !fastSupported) return;
    setFastServiceTierEnabled((enabled) => {
      const next = !enabled;
      saveFastServiceTierPreference(next);
      return next;
    });
  }, [fastSupported, selectorLocked]);

  const handleModeSelect = useCallback(
    async (nextMode: AgentMode) => {
      if (selectorLocked) return;
      const previousMode = effectiveMode;
      if (nextMode !== "plan") {
        pendingPlanWriteModeRef.current = null;
      }
      setMode(nextMode);
      setModeOpen(false);
      try {
        await onModeChange(nextMode);
      } catch (err) {
        setMode(previousMode);
        setView((prev) => ({
          ...prev,
          status: "stopped",
          streamPhase: "idle",
          lastError: String(err),
          turnStartedAtMs: null,
        }));
      }
    },
    [effectiveMode, onModeChange, selectorLocked],
  );

  const commandSelectionForMode = useCallback(
    (targetMode: AgentMode): ModeModelSelection => {
      const rawSelection =
        modeSelections[targetMode] ??
        selectionFromRef(modeModelSettings[targetMode] ?? activeModel);
      return selectionForAvailableModels(rawSelection, availableModels);
    },
    [activeModel, availableModels, modeModelSettings, modeSelections],
  );

  const sendPlanCommand = useCallback(
    async (
      plan: PlanArtifact,
      value: string,
      nextMode: AgentMode,
      planControl: PlanControl,
      messageVisibility: MessageVisibility = "normal",
    ) => {
      if (view.status === "streaming" || !modelEntry) return;
      const planAttachment = {
        path: plan.absolutePath ?? plan.path,
        name: basename(plan.path),
      };
      const commandSelection = commandSelectionForMode(nextMode);
      const commandServiceTier = serviceTierForSelection(
        commandSelection,
        availableModels,
        fastServiceTierEnabled,
      );
      setView((prev) => {
        const next = beginTurn(prev);
        if (messageVisibility === "systemReminder") return next;
        return appendUserMessage(next, value, history.length, [planAttachment]);
      });
      setSendTick((t) => t + 1);
      try {
        await onSend(
          value,
          [planAttachment],
          modelRefWithUse1mContext(
            modelRefFromId(commandSelection.model),
            modelSupports1mContextBeta(commandSelection.model) &&
              commandSelection.use1mContext,
          ),
          commandSelection.thinking,
          nextMode,
          commandServiceTier,
          undefined,
          planControl,
          messageVisibility,
        );
      } catch (err) {
        setView((prev) => ({
          ...prev,
          status: "stopped",
          streamPhase: "idle",
          lastError: String(err),
          turnStartedAtMs: null,
        }));
      }
    },
    [
      availableModels,
      commandSelectionForMode,
      fastServiceTierEnabled,
      history.length,
      modelEntry,
      onSend,
      view.status,
    ],
  );

  const handlePlanKeepUpdating = useCallback(
    (plan: PlanArtifact) => {
      setMode("plan");
      pendingPlanWriteModeRef.current = "update";
      void sendPlanCommand(
        plan,
        "No, keep updating the plan. Use the attached plan as the current draft, ask any useful follow-up questions, then rewrite the plan when ready.",
        "plan",
        "updatePlan",
        "systemReminder",
      );
    },
    [sendPlanCommand],
  );

  const handlePlanImplement = useCallback(
    (plan: PlanArtifact) => {
      pendingPlanWriteModeRef.current = null;
      setMode("act");
      void sendPlanCommand(
        plan,
        IMPLEMENT_PLAN_PROMPT,
        "act",
        "implementPlan",
        "systemReminder",
      );
    },
    [sendPlanCommand],
  );

  const handlePlanImplementWithSwarm = useCallback(
    (plan: PlanArtifact) => {
      if (!agentTeamsEnabled) return;
      pendingPlanWriteModeRef.current = null;
      setMode("act");
      void sendPlanCommand(
        plan,
        IMPLEMENT_PLAN_WITH_SWARM_PROMPT,
        "act",
        "implementPlan",
        "systemReminder",
      );
    },
    [agentTeamsEnabled, sendPlanCommand],
  );

  const handlePlanImplementFresh = useCallback(
    async (plan: PlanArtifact) => {
      if (view.status === "streaming") return;
      pendingPlanWriteModeRef.current = null;
      setMode("act");
      setSendTick((t) => t + 1);
      try {
        // "Implement plan and clear context" creates a brand new
        // conversation, so the parent will seed it from the workspace's
        // global default (the user's most recent model choice anywhere).
        // We deliberately do NOT pass the current conversation's selection
        // here — that would contaminate the fresh conversation with the
        // old conversation's preference.
        await onImplementPlanFresh(plan);
      } catch (err) {
        setView((prev) => ({
          ...prev,
          status: "stopped",
          streamPhase: "idle",
          lastError: String(err),
          turnStartedAtMs: null,
        }));
      }
    },
    [onImplementPlanFresh, view.status],
  );

  const handlePlanImplementFreshWithSwarm = useCallback(
    async (plan: PlanArtifact) => {
      if (view.status === "streaming" || !agentTeamsEnabled) return;
      pendingPlanWriteModeRef.current = null;
      setMode("act");
      setSendTick((t) => t + 1);
      try {
        // Same reasoning as handlePlanImplementFresh: this spawns a new
        // conversation, which the parent seeds from the workspace's global
        // default.
        await onImplementPlanFresh(plan, IMPLEMENT_PLAN_WITH_SWARM_PROMPT);
      } catch (err) {
        setView((prev) => ({
          ...prev,
          status: "stopped",
          streamPhase: "idle",
          lastError: String(err),
          turnStartedAtMs: null,
        }));
      }
    },
    [agentTeamsEnabled, onImplementPlanFresh, view.status],
  );

  const handleRewindToMessage = useCallback(
    (block: Extract<ChatBlock, { kind: "user-text" }>) => {
      if (view.status === "streaming") return;
      const rewound = block.attachments ?? [];
      const inline: InlineMention[] = [];
      const chips: Attachment[] = [];
      for (const att of rewound) {
        const rel = relativizePath(workspacePath, att.path);
        if (rel && block.text.includes(`@${rel}`)) {
          inline.push({ path: rel, absolutePath: att.path, name: att.name });
        } else {
          chips.push({ ...att, origin: "manual" });
        }
      }
      pendingCaretRef.current = block.text.length;
      setText(block.text);
      setAttachments(chips);
      setInlineMentions(inline);
      setCompactInstructionOpen(false);
      setRewriteState((prev) => ({
        historyIndex: block.historyIndex,
        originalText: prev?.originalText ?? text,
        originalAttachments: prev?.originalAttachments ?? attachments,
        revertWorkspaceChanges: prev?.revertWorkspaceChanges ?? true,
      }));
    },
    [attachments, text, view.status, workspacePath],
  );

  const handleKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (mention) {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        if (matches.length > 0) {
          setMention({
            ...mention,
            index: (mention.index + 1) % matches.length,
          });
        }
        return;
      }
      if (event.key === "ArrowUp") {
        event.preventDefault();
        if (matches.length > 0) {
          setMention({
            ...mention,
            index: (mention.index - 1 + matches.length) % matches.length,
          });
        }
        return;
      }
      if (event.key === "Enter" || event.key === "Tab") {
        if (matches.length > 0) {
          event.preventDefault();
          selectMention(matches[mention.index] ?? matches[0]);
          return;
        }
      }
      if (event.key === "Escape") {
        event.preventDefault();
        setMention(null);
        return;
      }
    }
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      void handleSend();
    }
  };

  const handleTextChange = (
    event: React.ChangeEvent<HTMLTextAreaElement>,
  ) => {
    const value = event.target.value;
    setText(value);
    detectMention(value, event.target.selectionStart ?? value.length);
  };

  const handleTextSelect = (
    event: React.SyntheticEvent<HTMLTextAreaElement>,
  ) => {
    const ta = event.currentTarget;
    detectMention(ta.value, ta.selectionStart ?? ta.value.length);
  };

  const handlePaste = useCallback(
    (event: React.ClipboardEvent<HTMLTextAreaElement>) => {
      const files = clipboardImageFiles(event.clipboardData);
      if (files.length === 0) return;

      event.preventDefault();
      setMention(null);
      void (async () => {
        const next: Attachment[] = [];
        for (const [index, file] of files.entries()) {
          const mediaType = clipboardImageMediaType(file);
          if (!mediaType) continue;
          try {
            const saved = await api.saveClipboardImage(
              workspacePath,
              pastedImageName(file, index, mediaType),
              mediaType,
              await readFileAsBase64(file),
            );
            next.push({
              path: saved.path,
              name: saved.name,
              origin: "manual",
            });
          } catch (err) {
            console.error(err);
          }
        }
        if (next.length > 0) {
          setAttachments((prev) => mergeAttachments(prev, next));
          textareaRef.current?.focus();
        }
      })();
    },
    [workspacePath],
  );

  const pickAttachments = useCallback(async () => {
    try {
      const selected = await open({ multiple: true, directory: false });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      if (!paths.length) return;
      const next: Attachment[] = paths.map((p) => ({
        path: p,
        name: basename(p),
        origin: "manual",
      }));
      setAttachments((prev) => mergeAttachments(prev, next));
    } catch {
      // user cancelled or platform error
    }
  }, []);

  useEffect(() => {
    if (!modelOpen && !thinkingOpen && !modeOpen) return;
    const onDoc = (event: MouseEvent) => {
      const target = event.target as Node;
      if (modelOpen && modelRef.current && !modelRef.current.contains(target)) {
        setModelOpen(false);
      }
      if (
        thinkingOpen &&
        thinkingRef.current &&
        !thinkingRef.current.contains(target)
      ) {
        setThinkingOpen(false);
      }
      if (modeOpen && modeRef.current && !modeRef.current.contains(target)) {
        setModeOpen(false);
      }
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setModelOpen(false);
        setThinkingOpen(false);
        setModeOpen(false);
      }
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [modelOpen, thinkingOpen, modeOpen]);

  const onDragOver = (event: React.DragEvent) => {
    if (activeSubAgentId !== null) return;
    if (
      event.dataTransfer.types.includes("application/x-claakecode-file") ||
      event.dataTransfer.types.includes("Files")
    ) {
      event.preventDefault();
      setDropActive(true);
    }
  };
  const onDragLeave = () => setDropActive(false);
  const onDrop = (event: React.DragEvent) => {
    event.preventDefault();
    setDropActive(false);
    if (activeSubAgentId !== null) return;
    const payload = event.dataTransfer.getData("application/x-claakecode-file");
    if (payload) {
      try {
        const parsed = JSON.parse(payload) as {
          relativePath: string;
          absolutePath: string;
          name: string;
        };
        setAttachments((prev) =>
          mergeAttachments(prev, [
            {
              path: parsed.absolutePath || parsed.relativePath,
              name: parsed.name,
              origin: "sidebar",
            },
          ]),
        );
      } catch {
        // ignore
      }
    }
    if (event.dataTransfer.files && event.dataTransfer.files.length > 0) {
      const files = Array.from(event.dataTransfer.files);
      const next: Attachment[] = [];
      for (const file of files) {
        const path = (file as File & { path?: string }).path;
        if (!path) continue;
        next.push({ path, name: file.name, origin: "finder" });
      }
      if (next.length) {
        setAttachments((prev) => mergeAttachments(prev, next));
      }
    }
  };

  const handleOpenSubAgent = useCallback((block: Extract<ChatBlock, { kind: "tool" }>) => {
    const subAgent = block.subAgent;
    const recordId = subAgentViewId(block.id, subAgent?.agentId);
    const initialMessage = subAgentInitialMessageFromToolBlock(block);
    const fallbackBaseView: ChatViewState = subAgent?.history
      ? initialSubAgentViewFromHistory(subAgent.history)
      : {
          blocks: [],
          status: block.status === "running" ? "streaming" : "idle",
          streamPhase: block.status === "running" ? "waiting" : "idle",
          lastError: null,
          turnStartedAtMs: null,
        };
    const name =
      subAgentNameFromSummary(block.summary) ||
      subAgent?.name ||
      "Sub-agent";
    const fallbackView = appendQueuedMessagesToView(
      seedInitialSubAgentMessage(fallbackBaseView, block.id, initialMessage),
      subAgent?.queuedMessages,
      name,
    );
    const title = subAgentToolTitle(block.summary, subAgent?.name);
    setSubAgentViewsForConversation(conversationId, (current) => {
      const existing = current.get(recordId);
      if (existing) {
        const seededView = seedInitialSubAgentMessage(
          appendQueuedMessagesToView(existing.view, subAgent?.queuedMessages, name),
          block.id,
          initialMessage,
        );
        const nextModel = existing.model ?? subAgent?.model;
        const nextHistory = existing.history ?? subAgent?.history;
        const nextAgentId = existing.agentId ?? subAgent?.agentId;
        const nextName = isGenericSubAgentName(existing.name) ? name : existing.name;
        const nextTitle =
          isGenericSubAgentName(existing.name) || /agent/i.test(existing.title)
            ? title
            : existing.title;
        if (
          seededView === existing.view &&
          nextAgentId === existing.agentId &&
          nextModel === existing.model &&
          nextHistory === existing.history &&
          nextName === existing.name &&
          nextTitle === existing.title
        ) {
          return current;
        }
        const next = new Map(current);
        next.set(recordId, {
          ...existing,
          name: nextName,
          title: nextTitle,
          agentId: nextAgentId,
          model: nextModel,
          history: nextHistory,
          view: seededView,
        });
        return next;
      }
      const next = new Map(current);
      next.set(recordId, {
        id: recordId,
        agentId: subAgent?.agentId,
        name,
        title,
        model: subAgent?.model,
        history: subAgent?.history,
        view: fallbackView,
      });
      return next;
    });
    setActiveSubAgentId(recordId);
    setAutoCloseSubAgentId(block.status === "running" ? recordId : null);
  }, [conversationId, setSubAgentViewsForConversation]);

  const handleOpenSubAgentRecord = useCallback((record: SubAgentViewRecord) => {
    setSubAgentViewsForConversation(conversationId, (current) => {
      const existing = current.get(record.id);
      if (existing) {
        const next = new Map(current);
        next.set(record.id, {
          ...existing,
          ...record,
          agentId: existing.agentId ?? record.agentId,
          model: existing.model ?? record.model,
          history: existing.history ?? record.history,
          view: existing.view.blocks.length > 0 ? existing.view : record.view,
        });
        return next;
      }
      const next = new Map(current);
      next.set(record.id, record);
      return next;
    });
    setActiveSubAgentId(record.id);
    setAutoCloseSubAgentId(record.view.status === "streaming" ? record.id : null);
  }, [conversationId, setSubAgentViewsForConversation]);

  const handleStopAgentSwarm = useCallback(
    async (teamName?: string) => {
      await api.stopAgentSwarm(workspacePath, conversationId, teamName);
      setActiveTeamNamesByConversation((current) =>
        removeActiveTeamName(current, conversationId, teamName),
      );
      setSubAgentViewsForConversation(conversationId, (current) => {
        const next = new Map(current);
        for (const [id, record] of current) {
          const recordTeamName = teamNameFromAgentId(record.agentId);
          if (recordTeamName && (!teamName || recordTeamName === teamName)) {
            next.delete(id);
          }
        }
        return next;
      });
    },
    [conversationId, setSubAgentViewsForConversation, workspacePath],
  );

  const handleQueuedPromptEdit = useCallback(
    (id: string) => {
      const item = queuedPrompts.find((prompt) => prompt.id === id);
      if (!item) return;
      const itemIndex = queuedPrompts.findIndex((prompt) => prompt.id === id);
      const currentAttachments = composerAttachments;
      const currentValue = text.trim() || attachmentOnlyMessage(currentAttachments);
      const editing =
        editingQueuedPrompt?.conversationId === conversationId
          ? editingQueuedPrompt
          : null;
      const stashedDraft =
        currentValue || currentAttachments.length > 0
          ? buildQueuedPrompt({
              id: editing?.id,
              text: currentValue,
              attachments: currentAttachments,
              model: currentModelRef,
              thinking,
              mode: effectiveMode,
              serviceTier,
              createdAtMs: editing?.createdAtMs,
            })
          : null;

      if (stashedDraft) {
        blockedQueueItemIdsRef.current.delete(stashedDraft.id);
      }
      setPromptQueuesByConversation((current) =>
        updatePromptQueue(current, conversationId, (queue) => {
          const index = queue.findIndex((prompt) => prompt.id === id);
          if (index < 0) return queue;
          let next = queue.filter((prompt) => prompt.id !== id);
          if (stashedDraft) {
            const insertAtIndex = editing ? editing.index : index;
            next = insertQueuedPrompt(next, stashedDraft, insertAtIndex);
          }
          return next;
        }),
      );
      blockedQueueItemIdsRef.current.delete(item.id);
      setText(item.text);
      setAttachments(composerAttachmentsFromQueue(item.attachments));
      setInlineMentions([]);
      setRewriteState(null);
      setMention(null);
      setEditingQueuedPrompt({
        conversationId,
        id: item.id,
        index: itemIndex >= 0 ? itemIndex : 0,
        createdAtMs: item.createdAtMs,
      });
      pendingCaretRef.current = item.text.length;
    },
    [
      composerAttachments,
      conversationId,
      editingQueuedPrompt,
      effectiveMode,
      model,
      queuedPrompts,
      serviceTier,
      text,
      thinking,
    ],
  );

  const handleQueuedPromptDelete = useCallback(
    (id: string) => {
      blockedQueueItemIdsRef.current.delete(id);
      if (dequeueInFlightRef.current === id) {
        dequeueInFlightRef.current = null;
      }
      setPromptQueuesByConversation((current) =>
        updatePromptQueue(current, conversationId, (queue) =>
          queue.filter((prompt) => prompt.id !== id),
        ),
      );
    },
    [conversationId],
  );

  const handleQueuedPromptMove = useCallback(
    (draggedId: string, targetId: string) => {
      setPromptQueuesByConversation((current) =>
        updatePromptQueue(current, conversationId, (queue) =>
          moveQueuedPrompt(queue, draggedId, targetId),
        ),
      );
    },
    [conversationId],
  );

  const activeSubAgent =
    activeSubAgentId !== null ? subAgentViews.get(activeSubAgentId) ?? null : null;
  const viewingSubAgent = activeSubAgent !== null;
  const displayView = activeSubAgent?.view ?? view;
  const showPlanningNextMove = shouldShowPlanningNextMove(displayView);
  const teamAgentRoster = useMemo(
    () => buildTeamAgentRoster(view.blocks, subAgentViews),
    [view.blocks, subAgentViews],
  );
  const activeTeamNames =
    activeTeamNamesByConversation.get(conversationId) ??
    EMPTY_ACTIVE_TEAM_NAMES;
  const teamTaskBlocks = useMemo(() => {
    const blocks = view.blocks.slice();
    for (const record of subAgentViews.values()) {
      blocks.push(...record.view.blocks);
    }
    return blocks;
  }, [view.blocks, subAgentViews]);
  const activeTeamAgentRoster = useMemo(
    () =>
      markFinishedTeamAgents(
        filterActiveTeamAgentRoster(teamAgentRoster, activeTeamNames),
        teamTaskBlocks,
      ),
    [activeTeamNames, teamAgentRoster, teamTaskBlocks],
  );
  const teamAgentColors = useMemo(() => {
    const colors: Record<string, string> = {};
    for (const agent of activeTeamAgentRoster) {
      colors[agent.name.trim().toLowerCase()] = agent.color;
    }
    return colors;
  }, [activeTeamAgentRoster]);
  const teamCompletionByTeam = useMemo(
    () => buildTeamCompletionByTeam(teamTaskBlocks),
    [teamTaskBlocks],
  );

  useEffect(() => {
    let cancelled = false;
    if (!activeSubAgent) {
      subAgentContextEstimateIdRef.current = null;
      setSubAgentContextEstimate({ status: "loading", estimate: null });
      return () => {
        cancelled = true;
      };
    }

    const sameSubAgent =
      subAgentContextEstimateIdRef.current === activeSubAgent.id;
    subAgentContextEstimateIdRef.current = activeSubAgent.id;

    if (activeSubAgent.view.status === "streaming") {
      if (!sameSubAgent) {
        setSubAgentContextEstimate({ status: "loading", estimate: null });
      }
      return () => {
        cancelled = true;
      };
    }

    const estimateModel = activeSubAgent.model ?? activeModel;
    if (!activeSubAgent.agentId || !estimateModel || !activeSubAgent.history) {
      setSubAgentContextEstimate((previous) => ({
        status: "loading",
        estimate: sameSubAgent ? previous.estimate : null,
      }));
      return () => {
        cancelled = true;
      };
    }

    setSubAgentContextEstimate((previous) => ({
      status: "loading",
      estimate: sameSubAgent ? previous.estimate : null,
    }));
    const timer = window.setTimeout(() => {
      void api
        .estimateSubAgentContext(
          workspacePath,
          activeSubAgent.agentId!,
          activeSubAgent.name,
          activeSubAgent.history!,
          estimateModel,
          effectiveMode,
        )
        .then((estimate) => {
          if (
            !cancelled &&
            subAgentContextEstimateIdRef.current === activeSubAgent.id
          ) {
            setSubAgentContextEstimate({ status: "ready", estimate });
          }
        })
        .catch(() => {
          if (
            !cancelled &&
            subAgentContextEstimateIdRef.current === activeSubAgent.id
          ) {
            setSubAgentContextEstimate((previous) => ({
              status: "error",
              estimate: previous.estimate,
            }));
          }
        });
    }, 700);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [
    activeSubAgent,
    activeModel,
    effectiveMode,
    workspacePath,
  ]);

  const handleCloseSubAgent = useCallback(() => {
    setActiveSubAgentId(null);
    setAutoCloseSubAgentId(null);
  }, []);

  useEffect(() => {
    if (!activeSubAgent || activeSubAgent.id !== autoCloseSubAgentId) return;
    if (activeSubAgent.view.status === "streaming") return;
    if (activeSubAgent.view.blocks.length === 0 && !activeSubAgent.view.lastError) return;
    setActiveSubAgentId(null);
    setAutoCloseSubAgentId(null);
  }, [activeSubAgent, autoCloseSubAgentId]);

  return (
    <div
      className="chat-col"
      ref={dropZoneRef}
      data-drop={dropActive && !viewingSubAgent ? "true" : "false"}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
    >
      {previewImage && (
        <div
          className="img-preview"
          onClick={() => setPreviewImage(null)}
          role="dialog"
          aria-modal="true"
        >
          <img
            src={convertFileSrc(previewImage)}
            alt={basename(previewImage)}
            onClick={(event) => event.stopPropagation()}
          />
        </div>
      )}
      {dropActive && !viewingSubAgent && (
        <div className="chat-drop-overlay" aria-hidden>
          <div className="chat-drop-overlay__card">
            <span className="chat-drop-overlay__mark">
              <Icon icon="solar:paperclip-bold" width={24} height={24} />
            </span>
            <div className="chat-drop-overlay__title">Drop files to attach</div>
            <div className="chat-drop-overlay__sub">
              They&rsquo;ll be included as context in your next message.
            </div>
          </div>
        </div>
      )}
      <div className="chat-head">
        {viewingSubAgent && (
          <button
            type="button"
            className="chat-head__back"
            onClick={handleCloseSubAgent}
            aria-label="Back to main chat"
            title="Back"
          >
            <Icon icon="solar:alt-arrow-left-linear" width={16} height={16} />
          </button>
        )}
        <span className="chat-head__title">
          {viewingSubAgent ? (
            <span style={{ display: "inline-flex", color: "var(--text-3)" }}>
              <AiAgentGlyph />
            </span>
          ) : (
            <Icon
              icon="solar:chat-square-code-bold-duotone"
              width={16}
              height={16}
              style={{ color: "var(--text-3)" }}
            />
          )}
          <span>{activeSubAgent?.title ?? "Chat"}</span>
        </span>
        <span className="chat-head__dot" data-status={displayView.status} />
      </div>
      <div className="chat-body" ref={bodyRef}>
        <div className="chat-body__content" ref={bodyContentRef}>
          {displayView.blocks.length === 0 && !showPlanningNextMove ? (
            <div className="chat-empty">
              <span className="chat-empty__mark">
                {viewingSubAgent ? (
                  <span className="tool-card__spinner" />
                ) : (
                  <Icon
                    icon="solar:magic-stick-3-bold-duotone"
                    width={22}
                    height={22}
                  />
                )}
              </span>
              <span className="chat-empty__title">
                {viewingSubAgent ? "Starting sub-agent" : "Say something"}
              </span>
              {!viewingSubAgent && (
                <span className="chat-empty__sub">
                  Enter to send · Shift+Enter for newline
                </span>
              )}
            </div>
          ) : (
            <>
              {displayView.blocks.length > 0 && (
                <ChatBlocks
                  blocks={displayView.blocks}
                  onPreviewImage={setPreviewImage}
                  onRewindMessage={viewingSubAgent ? () => {} : handleRewindToMessage}
                  rewindDisabled={viewingSubAgent || view.status === "streaming"}
                  rewriteHistoryIndex={
                    viewingSubAgent ? null : rewriteState?.historyIndex ?? null
                  }
                  onOpenFile={onOpenFile}
                  onAnswerQuestion={viewingSubAgent ? () => {} : handleQuestionAnswer}
                  answerQuestionDisabled={viewingSubAgent}
                  allowStopQuestions={!viewingSubAgent && effectiveMode === "plan"}
                  onPlanKeepUpdating={viewingSubAgent ? () => {} : handlePlanKeepUpdating}
                  onPlanImplement={viewingSubAgent ? () => {} : handlePlanImplement}
                  onPlanImplementWithSwarm={
                    viewingSubAgent ? () => {} : handlePlanImplementWithSwarm
                  }
                  onPlanImplementFresh={viewingSubAgent ? () => {} : handlePlanImplementFresh}
                  onPlanImplementFreshWithSwarm={
                    viewingSubAgent ? () => {} : handlePlanImplementFreshWithSwarm
                  }
                  planActionDisabled={viewingSubAgent || view.status === "streaming"}
                  agentTeamsEnabled={!viewingSubAgent && agentTeamsEnabled}
                  onOpenSubAgent={viewingSubAgent ? () => {} : handleOpenSubAgent}
                  onStopAgentSwarm={viewingSubAgent ? undefined : handleStopAgentSwarm}
                  teamAgents={activeTeamAgentRoster}
                  teamCompletionByTeam={teamCompletionByTeam}
                  activeTeamNames={activeTeamNames}
                  activeAgentName={viewingSubAgent ? activeSubAgent?.name : undefined}
                />
              )}
              {showPlanningNextMove && (
                <div className="msg" data-role="assistant">
                  <PlanningNextMoveBlock />
                </div>
              )}
            </>
          )}
          {displayView.lastError && (
            <div
              className="tool-card__pre"
              data-error="true"
              style={{ margin: 0 }}
            >
              {displayView.lastError}
            </div>
          )}
        </div>
      </div>
      <TodoStrip
        blocks={displayView.blocks}
        teamBlocks={teamTaskBlocks}
        queuedPrompts={viewingSubAgent ? [] : queuedPrompts}
        showTeamTasks={activeTeamNames.size > 0}
        teamAgentColors={teamAgentColors}
        teamMessageRecipient={viewingSubAgent ? activeSubAgent?.name : undefined}
        onOpenFile={onOpenFile}
        onQueuedPromptEdit={viewingSubAgent ? undefined : handleQueuedPromptEdit}
        onQueuedPromptDelete={viewingSubAgent ? undefined : handleQueuedPromptDelete}
        onQueuedPromptMove={viewingSubAgent ? undefined : handleQueuedPromptMove}
      />
      {viewingSubAgent && activeSubAgent && (
        <SubAgentRuntimeCard
          subAgent={activeSubAgent}
          contextState={subAgentContextEstimate}
          fallbackModel={activeModel}
          allModels={allModels}
        />
      )}
      {!viewingSubAgent && (
        <>
          <div
            className={`composer${selectorLocked ? " composer--selector-locked" : ""}`}
            ref={composerRef}
          >
            {rewriteState && rewindFileChanges.length > 0 && (
              <RewindChangesPreview
                changes={rewindFileChanges}
                revertWorkspaceChanges={rewriteState.revertWorkspaceChanges}
                onRevertWorkspaceChangesChange={(revertWorkspaceChanges) => {
                  setRewriteState((current) =>
                    current ? { ...current, revertWorkspaceChanges } : current,
                  );
                }}
                onClose={() => {
                  setText(rewriteState.originalText);
                  setAttachments(rewriteState.originalAttachments);
                  setRewriteState(null);
                }}
              />
            )}
            {attachments.length > 0 && (
              <div className="chips">
                {attachments.map((att) => {
                  const image = isImagePath(att.name);
                  return (
                    <span
                      className="chip"
                      key={att.path}
                      data-image={image ? "true" : "false"}
                      onClick={
                        image ? () => setPreviewImage(att.path) : undefined
                      }
                      title={image ? "Click to preview" : att.path}
                    >
                      <span className="chip__icon">
                        <Icon
                          icon={fileIcon(att.name)}
                          width={14}
                          height={14}
                        />
                      </span>
                      <span className="chip__name">{att.name}</span>
                      <button
                        className="chip__close"
                        onClick={(event) => {
                          event.stopPropagation();
                          setAttachments((prev) =>
                            prev.filter((x) => x.path !== att.path),
                          );
                        }}
                        title="Remove attachment"
                      >
                        <Icon
                          icon="solar:close-circle-linear"
                          width={12}
                          height={12}
                        />
                      </button>
                    </span>
                  );
                })}
              </div>
            )}
        <TeamAgentRail
          agents={activeTeamAgentRoster}
          activeId={activeSubAgentId}
          fallbackModel={activeModel}
          allModels={allModels}
          onOpen={handleOpenSubAgentRecord}
        />
        <div
          className="composer__box"
          data-drop={dropActive ? "true" : "false"}
        >
          {mention && (
            <div
              className="mention-popover"
              role="listbox"
              aria-label="File mentions"
            >
              {matches.length === 0 ? (
                <div className="mention-popover__empty">
                  {mentionFiles === null ? "Loading files…" : "No matches"}
                </div>
              ) : (
                matches.map((file, idx) => {
                  const selected = idx === mention.index;
                  return (
                    <button
                      key={file.relativePath}
                      type="button"
                      role="option"
                      aria-selected={selected}
                      className="mention-popover__row"
                      data-selected={selected ? "true" : "false"}
                      onMouseDown={(event) => event.preventDefault()}
                      onClick={() => selectMention(file)}
                      onMouseEnter={() =>
                        setMention((prev) =>
                          prev ? { ...prev, index: idx } : prev,
                        )
                      }
                    >
                      <span className="mention-popover__icon">
                        <Icon
                          icon={fileIcon(file.name)}
                          width={14}
                          height={14}
                        />
                      </span>
                      <span className="mention-popover__name">
                        {file.name}
                      </span>
                      <span className="mention-popover__path">
                        {file.relativePath}
                      </span>
                    </button>
                  );
                })
              )}
            </div>
          )}
          <div
            className="composer__resize-handle"
            role="separator"
            aria-orientation="horizontal"
            aria-label="Resize composer (double-click to reset)"
            onMouseDown={handleComposerResizeMouseDown}
            onDoubleClick={handleComposerResizeDoubleClick}
          />
          <div className="composer__input-wrap">
            <div
              className="composer__overlay"
              ref={overlayRef}
              aria-hidden="true"
            >
              <div
                className="composer__overlay-inner"
                ref={overlayInnerRef}
              >
                {renderMentionHighlights(text, inlineMentions)}
              </div>
            </div>
            <textarea
              ref={textareaRef}
              className="composer__input"
              style={
                composerHeight !== null
                  ? { height: composerHeight, maxHeight: "none" }
                  : undefined
              }
              value={text}
              placeholder={
                view.status === "streaming" || isStreaming
                  ? "Queue next prompt..."
                  : "Message the agent… (type @ to mention a file)"
              }
              onChange={handleTextChange}
              onKeyDown={handleKeyDown}
              onPaste={handlePaste}
              onSelect={(event) => {
                handleTextSelect(event);
                // Arrow keys / Home / End can scroll the textarea internally
                // without firing `onScroll`; resync here too.
                syncOverlayScroll();
              }}
              onClick={(event) => {
                handleTextSelect(event);
                syncOverlayScroll();
              }}
              onScroll={syncOverlayScroll}
              onFocus={syncOverlayScroll}
              onBlur={() => {
                window.setTimeout(() => {
                  if (
                    !composerRef.current?.contains(document.activeElement)
                  ) {
                    setMention(null);
                  }
                }, 80);
              }}
            />
          </div>
          <div
            className="composer__actions"
            data-compacting={compactInstructionOpen ? "true" : "false"}
          >
            {compactInstructionOpen ? (
              <div className="composer__compacting" ref={compactPopoverRef}>
                <div
                  className="compact-pill"
                  role="dialog"
                  aria-label="Compaction instruction"
                >
                  <span className="compact-pill__icon" aria-hidden="true">
                    <Icon icon="solar:archive-linear" width={14} height={14} />
                  </span>
                  <input
                    ref={compactInstructionInputRef}
                    className="compact-pill__input"
                    value={compactInstruction}
                    onChange={(event) =>
                      setCompactInstruction(event.target.value)
                    }
                    onKeyDown={handleCompactInstructionKeyDown}
                    placeholder="Optional focus, e.g. keep only X…"
                    aria-label="Compaction instruction"
                  />
                  <button
                    type="button"
                    className="compact-pill__submit"
                    onClick={() => void handleCompactInstructionSubmit()}
                    aria-label="Compact conversation"
                  >
                    <Icon
                      icon="solar:arrow-right-linear"
                      width={13}
                      height={13}
                    />
                    <span
                      className="compact-pill__tip"
                      role="tooltip"
                      aria-hidden="true"
                    >
                      Compact conversation
                    </span>
                  </button>
                </div>
                <button
                  type="button"
                  className="composer__iconbtn composer__compact-cancel"
                  onClick={() => setCompactInstructionOpen(false)}
                  aria-label="Cancel compaction"
                >
                  <Icon icon="solar:close-circle-linear" width={18} height={18} />
                </button>
              </div>
            ) : (
              <>
            <div className="composer__actions-left">
              <button
                type="button"
                className="composer__iconbtn"
                onClick={() => void pickAttachments()}
                title="Attach files"
                aria-label="Attach files"
              >
                <Icon icon="solar:add-circle-linear" width={18} height={18} />
              </button>
              {noProvidersConfigured ? (
                <button
                  type="button"
                  className="composer__connect-cta"
                  onClick={() => onOpenSettings("providers")}
                >
                  <Icon icon="solar:plug-circle-linear" width={14} height={14} />
                  <span>Connect a provider</span>
                </button>
              ) : (
                <>
              <div className="composer__picker" data-kind="mode" ref={modeRef}>
                <button
                  type="button"
                  className="composer__picker-btn"
                  data-open={modeOpen ? "true" : "false"}
                  data-mode={effectiveMode}
                  data-locked={selectorLocked ? "true" : "false"}
                  disabled={selectorLocked}
                  onClick={() => {
                    if (selectorLocked) return;
                    setModeOpen((o) => !o);
                    setModelOpen(false);
                    setThinkingOpen(false);
                  }}
                  title={selectorLocked ? "Mode locked while streaming" : "Mode"}
                >
                  <span className="composer__picker-label">{modeEntry.label}</span>
                  <Icon
                    icon="solar:alt-arrow-down-linear"
                    width={11}
                    height={11}
                  />
                </button>
                {modeOpen && !selectorLocked && (
                  <div
                    className="composer__popover"
                    role="menu"
                    aria-label="Mode"
                  >
                    {MODES.map((entry) => {
                      const selected = entry.value === effectiveMode;
                      const disabled = selectorLocked;
                      return (
                        <button
                          key={entry.value}
                          type="button"
                          className="composer__popover-row"
                          data-mode={entry.value}
                          data-selected={selected ? "true" : "false"}
                          disabled={disabled}
                          onClick={() => {
                            if (disabled) return;
                            void handleModeSelect(entry.value);
                          }}
                        >
                          <span className="composer__popover-label">
                            <Icon icon={entry.icon} width={14} height={14} />
                            <span>{entry.label}</span>
                          </span>
                          {selected && (
                            <Icon
                              icon="solar:check-read-linear"
                              width={13}
                              height={13}
                              className="composer__popover-check"
                            />
                          )}
                        </button>
                      );
                    })}
                  </div>
                )}
              </div>
              <div className="composer__picker" data-kind="model" ref={modelRef}>
                <button
                  type="button"
                  className="composer__picker-btn"
                  data-open={modelOpen ? "true" : "false"}
                  data-locked={selectorLocked ? "true" : "false"}
                  disabled={selectorLocked || availableModels.length === 0}
                  onClick={() => {
                    if (selectorLocked) return;
                    setModelOpen((o) => !o);
                    setThinkingOpen(false);
                    setModeOpen(false);
                  }}
                  title={selectorLocked ? "Model locked while streaming" : "Model"}
                >
                  <span className="composer__picker-label">
                    {displayModelEntry?.label ?? "No models"}
                  </span>
                  <Icon
                    icon="solar:alt-arrow-down-linear"
                    width={11}
                    height={11}
                  />
                </button>
                {modelOpen && !selectorLocked && (
                  <div
                    className="composer__popover"
                    role="menu"
                    aria-label="Model"
                  >
                    {availableModels.map((m) => {
                      const selected = m.value === model;
                      const providerIcon =
                        PROVIDERS.find((p) => p.value === m.provider)?.icon;
                      return (
                        <button
                          key={m.value}
                          type="button"
                          className="composer__popover-row"
                          data-selected={selected ? "true" : "false"}
                          onClick={() => {
                            handleModelSelect(m.value);
                          }}
                        >
                          <span className="composer__popover-label">
                            {providerIcon && (
                              <Icon icon={providerIcon} width={13} height={13} />
                            )}
                            <span>{m.label}</span>
                          </span>
                          {selected && (
                            <Icon
                              icon="solar:check-read-linear"
                              width={13}
                              height={13}
                              className="composer__popover-check"
                            />
                          )}
                        </button>
                      );
                    })}
                  </div>
                )}
              </div>
              <div className="composer__picker" data-kind="thinking" ref={thinkingRef}>
                <button
                  type="button"
                  className="composer__picker-btn"
                  data-open={thinkingOpen ? "true" : "false"}
                  data-locked={selectorLocked ? "true" : "false"}
                  disabled={
                    selectorLocked ||
                    availableModels.length === 0 ||
                    availableThinking.length === 0
                  }
                  onClick={() => {
                    if (selectorLocked) return;
                    setThinkingOpen((o) => !o);
                    setModelOpen(false);
                    setModeOpen(false);
                  }}
                  title={selectorLocked ? "Thinking locked while streaming" : "Thinking"}
                >
                  <span className="composer__picker-label">{thinkingLabel}</span>
                  <Icon
                    icon="solar:alt-arrow-down-linear"
                    width={11}
                    height={11}
                  />
                </button>
                {thinkingOpen && !selectorLocked && (
                  <div
                    className="composer__popover"
                    role="menu"
                    aria-label="Thinking"
                  >
                    {availableThinking.map((level) => {
                      const selected = level.value === thinking;
                      return (
                        <button
                          key={level.value}
                          type="button"
                          className="composer__popover-row"
                          data-selected={selected ? "true" : "false"}
                          onClick={() => {
                            handleThinkingSelect(level.value);
                          }}
                        >
                          <span>{thinkingLevelLabel(level, modelEntry)}</span>
                          {selected && (
                            <Icon
                              icon="solar:check-read-linear"
                              width={13}
                              height={13}
                              className="composer__popover-check"
                            />
                          )}
                        </button>
                      );
                    })}
                  </div>
                )}
              </div>
              {supports1mContext && (
                <button
                  type="button"
                  className="composer__picker-btn"
                  data-open={use1mContext ? "true" : "false"}
                  data-locked={selectorLocked ? "true" : "false"}
                  disabled={selectorLocked}
                  onClick={() => {
                    handle1mContextToggle(!use1mContext);
                  }}
                  title={
                    use1mContext
                      ? "1M context window (beta) enabled — requires Anthropic tier 4"
                      : "Enable 1M context window (beta) — requires Anthropic tier 4"
                  }
                  aria-pressed={use1mContext}
                >
                  <Icon
                    icon={
                      use1mContext
                        ? "solar:checkbox-bold"
                        : "solar:checkbox-linear"
                    }
                    width={12}
                    height={12}
                  />
                  <span className="composer__picker-label">1M</span>
                </button>
              )}
              {fastSupported && (
                <button
                  type="button"
                  className="composer__iconbtn composer__fast-btn"
                  data-active={serviceTier === "fast" ? "true" : "false"}
                  disabled={selectorLocked}
                  onClick={handleFastServiceTierToggle}
                  aria-label={
                    serviceTier === "fast" ? "Disable Fast" : "Enable Fast"
                  }
                  aria-pressed={serviceTier === "fast"}
                  title={
                    selectorLocked
                      ? "Fast locked while streaming"
                      : serviceTier === "fast"
                        ? "Fast on (uses more tokens)"
                        : "Fast (uses more tokens)"
                  }
                >
                  <Icon icon="solar:bolt-bold-duotone" width={15} height={15} />
                  <span
                    className="composer__iconbtn-tip"
                    role="tooltip"
                    aria-hidden="true"
                  >
                    {serviceTier === "fast" ? "Fast on (uses more tokens)" : "Fast (uses more tokens)"}
                  </span>
                </button>
              )}
                </>
              )}
            </div>
            <div className="composer__actions-right">
              <button
                type="button"
                className="composer__iconbtn composer__compact"
                onClick={handleCompact}
                disabled={compactDisabled}
                aria-label="Compact context"
                aria-expanded={compactInstructionOpen}
              >
                <Icon icon="solar:archive-linear" width={16} height={16} />
                <span className="composer__iconbtn-tip" role="tooltip" aria-hidden="true">
                  Compaction
                </span>
              </button>
              <ContextMeter state={visibleContextEstimate} />
              {view.status === "streaming" ? (
                <button
                  className="composer__send"
                  data-variant="stop"
                  onClick={() => void onStop()}
                >
                  <span className="composer__send-label">Stop</span>
                </button>
              ) : (
                <button
                  className="composer__send"
                  onClick={() => void handleSend()}
                  disabled={
                    (!text.trim() && composerAttachments.length === 0) ||
                    !modelEntry
                  }
                >
                  <span className="composer__send-label">Send</span>
                </button>
              )}
            </div>
              </>
            )}
          </div>
        </div>
      </div>
        </>
      )}
    </div>
  );
}

function TeamAgentRail({
  agents,
  activeId,
  fallbackModel,
  allModels,
  onOpen,
}: {
  agents: TeamAgentRosterItem[];
  activeId: string | null;
  fallbackModel: ModelRef;
  allModels: readonly ModelEntry[];
  onOpen: (record: SubAgentViewRecord) => void;
}) {
  if (agents.length === 0) return null;
  return (
    <div className="team-agent-rail" aria-label="Agent Swarm">
      <div className="team-agent-rail__track" role="list">
        {agents.map((agent) => {
          const label = compactModelLabel(agent.model ?? fallbackModel, allModels);
          return (
            <button
              key={agent.id}
              type="button"
              role="listitem"
              className="team-agent-chip"
              data-status={agent.status}
              data-active={activeId === agent.id ? "true" : "false"}
              style={
                {
                  "--agent-color": agent.color,
                  "--agent-color-soft": `${agent.color}22`,
                } as CSSProperties
              }
              onClick={() => onOpen(agent)}
              title={agent.task ? `${agent.name}: ${agent.task}` : agent.name}
            >
              <span className="team-agent-chip__glyph">
                <AiAgentGlyph />
              </span>
              <span className="team-agent-chip__body">
                <span className="team-agent-chip__name">{agent.name}</span>
                <span className="team-agent-chip__meta">
                  {agentStatusLabel(agent.status)} · {label}
                </span>
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}

function SubAgentRuntimeCard({
  subAgent,
  contextState,
  fallbackModel,
  allModels,
}: {
  subAgent: SubAgentViewRecord;
  contextState: ContextEstimateState;
  fallbackModel: ModelRef;
  allModels: readonly ModelEntry[];
}) {
  const model = subAgent.model ?? fallbackModel;
  const modelEntry = model ? modelEntryFromRef(model, allModels) : null;
  const thinkingLabel = model
    ? thinkingLevelLabel(
        THINKING_LEVELS.find((level) => level.value === thinkingFromRef(model)),
        modelEntry,
      ) ?? "Medium"
    : "Unknown";
  const modelLabel = modelEntry?.label ?? model?.name ?? "Unknown";
  return (
    <div className="composer composer--readonly">
      <div className="composer__actions">
        <div className="composer__actions-left">
          <div className="composer__picker" data-kind="model">
            <button type="button" className="composer__picker-btn" disabled tabIndex={-1}>
              <span className="composer__picker-label">{modelLabel}</span>
              <Icon icon="solar:alt-arrow-down-linear" width={11} height={11} />
            </button>
          </div>
          <div className="composer__picker" data-kind="thinking">
            <button type="button" className="composer__picker-btn" disabled tabIndex={-1}>
              <span className="composer__picker-label">{thinkingLabel}</span>
              <Icon icon="solar:alt-arrow-down-linear" width={11} height={11} />
            </button>
          </div>
        </div>
        <div className="composer__actions-right">
          <ContextMeter state={contextState} />
        </div>
      </div>
    </div>
  );
}

function ContextMeter({ state }: { state: ContextEstimateState }) {
  const estimate = state.estimate;
  const hasEstimate = estimate !== null;
  const ratio = estimate
    ? Math.min(1, estimate.usedTokens / Math.max(estimate.contextWindow, 1))
    : 0;
  const percent = Math.round(ratio * 100);
  const status =
    state.status === "loading"
      ? "loading"
      : state.status === "error" && hasEstimate
        ? "error"
        : !hasEstimate
          ? "loading"
          : ratio >= 0.95
          ? "danger"
          : ratio >= 0.8
            ? "warn"
            : "ok";
  const breakdown = estimate?.breakdown?.filter((item) => item.tokens > 0) ?? [];
  return (
    <div
      className="context-meter"
      data-status={status}
      style={
        {
          "--context-fill": `${Math.round(ratio * 360)}deg`,
        } as CSSProperties
      }
      tabIndex={hasEstimate ? 0 : -1}
      aria-hidden={hasEstimate ? undefined : true}
      aria-label={estimate ? `Context ${percent}%` : undefined}
    >
      <span className="context-meter__ring" aria-hidden="true" />
      {estimate ? (
        <div className="context-meter__popover" role="tooltip">
          <div className="context-meter__head">
            <span className="context-meter__title">Context</span>
            <span className="context-meter__token-total">
              {estimate.exact ? "" : "~"}
              {formatFullTokenCount(estimate.usedTokens)} /{" "}
              {formatFullTokenCount(estimate.contextWindow)}
            </span>
          </div>
          <div className="context-meter__summary">
            <span>{percent}% Full</span>
          </div>
          {breakdown.length > 0 && (
            <>
              <div className="context-meter__segments" aria-hidden="true">
                {breakdown.map((item) => (
                  <span
                    key={item.key}
                    className="context-meter__segment"
                    style={
                      {
                        "--segment-color": contextBreakdownColor(item.key),
                        "--segment-width": `${Math.max(
                          1,
                          (item.tokens / Math.max(estimate.contextWindow, 1)) *
                            100,
                        )}%`,
                      } as CSSProperties
                    }
                  />
                ))}
              </div>
              <div className="context-meter__rows">
                {breakdown.map((item) => (
                  <div className="context-meter__row" key={item.key}>
                    <span className="context-meter__legend">
                      <span
                        className="context-meter__swatch"
                        style={
                          {
                            "--swatch-color": contextBreakdownColor(item.key),
                          } as CSSProperties
                        }
                        aria-hidden="true"
                      />
                      <span className="context-meter__label">{item.label}</span>
                    </span>
                    <span className="context-meter__value">
                      {formatCompactTokenCount(item.tokens)}
                    </span>
                  </div>
                ))}
              </div>
            </>
          )}
        </div>
      ) : null}
    </div>
  );
}

function autoCompactWindow(estimate: ContextEstimate): number {
  if (estimate.contextWindow <= 0) return 0;
  const reservedOutput =
    estimate.maxOutputTokens > 0
      ? Math.min(estimate.maxOutputTokens, AUTO_COMPACT_OUTPUT_TOKEN_MAX)
      : AUTO_COMPACT_OUTPUT_TOKEN_MAX;
  return Math.max(0, estimate.contextWindow - reservedOutput);
}

type TokenUsageEvent = Extract<AgentEvent, { type: "token_usage" }>;

function contextEstimateFromTokenUsageEvent(
  event: TokenUsageEvent,
): ContextEstimate {
  const usage = event.usage;
  const inputTokens = safeTokenCount(usage.input_tokens);
  const outputTokens = safeTokenCount(usage.output_tokens);
  const reasoningTokens = safeTokenCount(usage.reasoning_tokens);
  const cacheReadTokens = safeTokenCount(usage.cache_read_tokens);
  const cacheCreationTokens = safeTokenCount(usage.cache_creation_tokens);
  const explicitTotal = safeTokenCount(usage.total_tokens);
  const summedTotal =
    inputTokens +
    outputTokens +
    reasoningTokens +
    cacheReadTokens +
    cacheCreationTokens;
  const usedTokens = explicitTotal > 0 ? explicitTotal : summedTotal;

  return {
    usedTokens,
    contextWindow: safeTokenCount(event.context_window),
    preferredWindow: safeTokenCount(event.preferred_window),
    maxOutputTokens: safeTokenCount(event.max_output_tokens),
    inputTokens,
    outputTokens,
    reasoningTokens,
    cacheReadTokens,
    cacheCreationTokens,
    exact: true,
    error: null,
    breakdown: contextBreakdownFromTokenUsage(usage),
  };
}

function contextBreakdownFromTokenUsage(
  usage: StreamTokenUsage,
): ContextEstimate["breakdown"] {
  const items: ContextEstimate["breakdown"] = [];
  pushTokenBreakdown(items, "input", "Input", usage.input_tokens);
  pushTokenBreakdown(items, "output", "Output", usage.output_tokens);
  pushTokenBreakdown(items, "reasoning", "Reasoning", usage.reasoning_tokens);
  pushTokenBreakdown(items, "cache", "Cache read", usage.cache_read_tokens);
  pushTokenBreakdown(
    items,
    "cache_write",
    "Cache write",
    usage.cache_creation_tokens,
  );
  return items;
}

function pushTokenBreakdown(
  items: ContextEstimate["breakdown"],
  key: string,
  label: string,
  rawTokens: number,
) {
  const tokens = safeTokenCount(rawTokens);
  if (tokens > 0) items.push({ key, label, tokens });
}

function safeTokenCount(value: number): number {
  return Number.isFinite(value) ? Math.max(0, Math.round(value)) : 0;
}

function hasContentAfterLatestCompaction(history: ChatMessage[]): boolean {
  let latestBoundary = -1;
  for (let i = 0; i < history.length; i++) {
    if (history[i].parts.some(isAutoCompactBoundaryPart)) {
      latestBoundary = i;
    }
  }
  return history
    .slice(latestBoundary + 1)
    .some((message) => message.parts.some(isAutoCompactMeaningfulPart));
}

function autoCompactHistorySignature(history: ChatMessage[]): string {
  const last = history[history.length - 1];
  if (!last) return "empty";
  const parts = last.parts.map(autoCompactPartSignature).join("|");
  return `${history.length}:${last.role}:${hashString(parts)}`;
}

function autoCompactPartSignature(part: Part): string {
  switch (part.type) {
    case "text":
    case "thinking":
      return `${part.type}:${part.text.length}:${part.text.slice(0, 32)}:${part.text.slice(-32)}`;
    case "tool_call":
      return `tool_call:${part.id}:${part.name}:${hashString(JSON.stringify(part.input ?? null))}`;
    case "tool_result":
      return `tool_result:${part.tool_call_id}:${part.content.length}:${part.is_error ? "1" : "0"}`;
    case "image":
      return `image:${part.media_type}:${part.data.length}`;
  }
}

function isAutoCompactBoundaryPart(part: Part): boolean {
  if (part.type !== "text") return false;
  const meta = part.meta;
  if (!meta || typeof meta !== "object") return false;
  const record = meta as Record<string, unknown>;
  return record.compaction_summary === true || record.compaction_marker === true;
}

function isAutoCompactMeaningfulPart(part: Part): boolean {
  if (part.type !== "text") return true;
  if (!part.text.trim()) return false;
  const meta = part.meta;
  if (!meta || typeof meta !== "object") return true;
  const record = meta as Record<string, unknown>;
  return !(
    record.attachment_context === true ||
    record.compaction_marker === true ||
    record.compaction_retained_user === true ||
    record.compaction_summary === true ||
    record.plan_control === "stop_questions" ||
    record.system_reminder === true ||
    record.ui_only === true
  );
}

function hashString(value: string): string {
  let hash = 2166136261;
  for (let i = 0; i < value.length; i++) {
    hash ^= value.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(36);
}

function shouldShowPlanningNextMove(view: ChatViewState): boolean {
  return (
    view.status === "streaming" &&
    view.streamPhase === "waiting" &&
    !view.blocks.some(
      (block) =>
        block.kind === "tool" &&
        block.name === "Question" &&
        block.status === "running",
    ) &&
    !view.blocks.some((block) => block.kind === "plan-writing")
  );
}

type TeamTaskCompletionSnapshot = {
  status: string;
  updatedAtMs?: number;
  sourceOrder: number;
};

const REWIND_DIFF_LINE_LIMIT = 220;

function RewindChangesPreview({
  changes,
  revertWorkspaceChanges,
  onRevertWorkspaceChangesChange,
  onClose,
}: {
  changes: FileChange[];
  revertWorkspaceChanges: boolean;
  onRevertWorkspaceChangesChange: (value: boolean) => void;
  onClose: () => void;
}) {
  const fileLabel =
    changes.length === 1
      ? "1 file will roll back"
      : `${changes.length} files will roll back`;
  const detail = revertWorkspaceChanges
    ? fileLabel
    : changes.length === 1
      ? "1 file will stay as-is"
      : `${changes.length} files will stay as-is`;
  return (
    <div className="rewind-preview">
      <div className="rewind-preview__head">
        <Icon icon="solar:rewind-back-bold-duotone" width={15} height={15} />
        <span className="rewind-preview__title">Rollback</span>
        <button
          type="button"
          className="rewind-preview__toggle"
          data-on={revertWorkspaceChanges ? "true" : "false"}
          role="switch"
          aria-checked={revertWorkspaceChanges}
          aria-label="Revert workspace changes on rollback"
          title={
            revertWorkspaceChanges
              ? "File changes will be reverted"
              : "File changes will be kept"
          }
          onClick={() =>
            onRevertWorkspaceChangesChange(!revertWorkspaceChanges)
          }
        >
          <span className="rewind-preview__toggle-thumb" />
        </button>
        <span className="rewind-preview__detail">{detail}</span>
        <button
          type="button"
          className="rewind-preview__close"
          onClick={onClose}
          aria-label="Cancel rewind"
          title="Cancel"
        >
          <Icon icon="solar:close-circle-linear" width={14} height={14} />
        </button>
      </div>
      <div className="rewind-preview__changes">
        {changes.map((change) => (
          <FileChangeBlock key={change.relativePath} change={change} />
        ))}
      </div>
    </div>
  );
}

function fileChangesAfterHistoryIndex(
  history: ChatMessage[],
  historyIndex: number,
): FileChange[] {
  const changes: FileChange[] = [];
  for (const message of history.slice(historyIndex)) {
    for (const part of message.parts) {
      if (part.type !== "tool_result") continue;
      const raw = part.meta?.file_changes;
      if (!Array.isArray(raw)) continue;
      for (const change of raw) {
        if (isFileChange(change)) changes.push(change);
      }
    }
  }
  return changes;
}

function aggregateFileChanges(changes: FileChange[]): FileChange[] {
  const byPath = new Map<string, FileChange>();
  for (const change of changes) {
    const existing = byPath.get(change.relativePath);
    const added = change.addedLines ?? countDiffLines(change, "added");
    const removed = change.removedLines ?? countDiffLines(change, "removed");
    if (!existing) {
      byPath.set(change.relativePath, {
        ...change,
        addedLines: added,
        removedLines: removed,
        lines: change.lines.slice(0, REWIND_DIFF_LINE_LIMIT),
        truncated:
          change.truncated || change.lines.length > REWIND_DIFF_LINE_LIMIT,
      });
      continue;
    }

    const nextLines = existing.lines.concat(
      change.lines.slice(
        0,
        Math.max(0, REWIND_DIFF_LINE_LIMIT - existing.lines.length),
      ),
    );
    byPath.set(change.relativePath, {
      ...existing,
      kind: existing.kind === change.kind ? existing.kind : "modified",
      binary: existing.binary || change.binary,
      addedLines: (existing.addedLines ?? 0) + added,
      removedLines: (existing.removedLines ?? 0) + removed,
      truncated:
        existing.truncated ||
        change.truncated ||
        existing.lines.length + change.lines.length > REWIND_DIFF_LINE_LIMIT,
      lines: nextLines,
    });
  }
  return Array.from(byPath.values());
}

function countDiffLines(change: FileChange, kind: "added" | "removed"): number {
  return change.lines.reduce(
    (count, line) => count + (line.kind === kind ? 1 : 0),
    0,
  );
}

function isFileChange(value: unknown): value is FileChange {
  if (!value || typeof value !== "object") return false;
  const record = value as Partial<FileChange>;
  return (
    typeof record.relativePath === "string" &&
    (record.kind === "added" ||
      record.kind === "modified" ||
      record.kind === "deleted") &&
    typeof record.binary === "boolean" &&
    Array.isArray(record.lines)
  );
}

function buildTeamCompletionByTeam(blocks: ChatBlock[]): Record<string, boolean> {
  const byTeam = new Map<string, Map<string, TeamTaskCompletionSnapshot>>();
  for (const [sourceOrder, block] of blocks.entries()) {
    if (block.kind !== "tool" || block.status === "error") continue;
    const team = block.meta?.team;
    if (!team || typeof team !== "object" || Array.isArray(team)) continue;
    const record = team as Record<string, unknown>;
    const teamName = typeof record.name === "string" ? record.name.trim() : "";
    const rawTasks = Array.isArray(record.tasks) ? record.tasks : [];
    if (!teamName || rawTasks.length === 0) continue;
    const tasks = byTeam.get(teamName) ?? new Map<string, TeamTaskCompletionSnapshot>();
    byTeam.set(teamName, tasks);
    for (const raw of rawTasks) {
      if (!raw || typeof raw !== "object" || Array.isArray(raw)) continue;
      const task = raw as Record<string, unknown>;
      const id =
        typeof task.id === "number" || typeof task.id === "string"
          ? String(task.id)
          : "";
      const status = typeof task.status === "string" ? task.status.trim() : "";
      if (!id || !status) continue;
      const incoming = {
        status,
        updatedAtMs: numberFromUnknown(task.updatedAtMs),
        sourceOrder,
      };
      const current = tasks.get(id);
      if (!current || shouldReplaceCompletionSnapshot(current, incoming)) {
        tasks.set(id, incoming);
      }
    }
  }
  return Object.fromEntries(
    Array.from(byTeam.entries()).map(([teamName, tasks]) => [
      teamName,
      tasks.size > 0 &&
        Array.from(tasks.values()).every((task) => task.status === "completed"),
    ]),
  );
}

function shouldReplaceCompletionSnapshot(
  current: TeamTaskCompletionSnapshot,
  incoming: TeamTaskCompletionSnapshot,
): boolean {
  if (incoming.updatedAtMs !== undefined && current.updatedAtMs !== undefined) {
    return incoming.updatedAtMs >= current.updatedAtMs;
  }
  if (incoming.updatedAtMs !== undefined) return true;
  if (current.updatedAtMs !== undefined) return false;
  return incoming.sourceOrder >= current.sourceOrder;
}

function numberFromUnknown(value: unknown): number | undefined {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim()) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : undefined;
  }
  return undefined;
}

function buildTeamAgentRoster(
  blocks: ChatBlock[],
  subAgentViews: Map<string, SubAgentViewRecord>,
): TeamAgentRosterItem[] {
  const byAgent = new Map<string, TeamAgentRosterItem>();

  const put = (record: SubAgentViewRecord, task?: string, status?: TeamAgentRosterItem["status"]) => {
    const key = subAgentRosterKey(record);
    const existing = byAgent.get(key);
    const nextStatus = status ?? statusFromSubAgentView(record.view);
    const color = fallbackAgentColor(record.agentId ?? record.name ?? record.id);
    const next: TeamAgentRosterItem = {
      ...record,
      color,
      task: task || existing?.task,
      status: mergeAgentStatus(existing?.status, nextStatus),
    };
    if (!existing) {
      byAgent.set(key, { ...next, title: next.title || subAgentToolTitle(undefined, next.name) });
      return;
    }
    const shouldReplace =
      nextStatus === "running" ||
      existing.status !== "running" ||
      record.view.blocks.length >= existing.view.blocks.length;
    byAgent.set(key, {
      ...(shouldReplace ? next : existing),
      id: shouldReplace ? next.id : existing.id,
      agentId: existing.agentId ?? next.agentId,
      model: next.model ?? existing.model,
      history: next.history ?? existing.history,
      task: next.task ?? existing.task,
      color,
      status: mergeAgentStatus(existing.status, nextStatus),
      title: next.title || existing.title,
    });
  };

  for (const record of subAgentViews.values()) {
    put(record);
  }

  for (const block of blocks) {
    if (block.kind !== "tool" || !isSubAgentToolName(block.name)) continue;
    const record = recordFromSubAgentToolBlock(block);
    if (!record) continue;
    put(record, subAgentTaskFromToolBlock(block), statusFromToolBlock(block));
  }

  const sorted = Array.from(byAgent.values()).sort((left, right) => {
    if (left.status === "running" && right.status !== "running") return -1;
    if (right.status === "running" && left.status !== "running") return 1;
    return orderKey(left).localeCompare(orderKey(right));
  });
  return assignUniqueTeamAgentColors(sorted);
}

function filterActiveTeamAgentRoster(
  agents: TeamAgentRosterItem[],
  activeTeamNames: ReadonlySet<string>,
): TeamAgentRosterItem[] {
  if (activeTeamNames.size === 0) return [];
  return agents.filter((agent) => {
    const teamName = teamNameFromAgentId(agent.agentId);
    return teamName ? activeTeamNames.has(teamName) : false;
  });
}

type OwnedTaskState = {
  owned: number;
  open: number;
};

type LatestOwnedTaskSnapshot = TeamTaskCompletionSnapshot & {
  owner?: string;
  teamName: string;
};

function markFinishedTeamAgents(
  agents: TeamAgentRosterItem[],
  blocks: ChatBlock[],
): TeamAgentRosterItem[] {
  const taskStateByAgent = buildOwnedTaskStateByAgent(blocks);
  return agents.map((agent) => {
    if (agent.status !== "idle") return agent;
    const teamName = teamNameFromAgentId(agent.agentId);
    if (!teamName) return agent;
    const taskState = taskStateByAgent.get(teamAgentTaskStateKey(teamName, agent.name));
    if (!taskState || taskState.owned === 0 || taskState.open > 0) return agent;
    return { ...agent, status: "finished" };
  });
}

function buildOwnedTaskStateByAgent(blocks: ChatBlock[]): Map<string, OwnedTaskState> {
  const latestByTask = new Map<string, LatestOwnedTaskSnapshot>();
  for (const [sourceOrder, block] of blocks.entries()) {
    if (block.kind !== "tool" || block.status === "error") continue;
    const team = block.meta?.team;
    if (!team || typeof team !== "object" || Array.isArray(team)) continue;
    const record = team as Record<string, unknown>;
    const teamName = typeof record.name === "string" ? record.name.trim() : "";
    const rawTasks = Array.isArray(record.tasks) ? record.tasks : [];
    if (!teamName || rawTasks.length === 0) continue;
    for (const raw of rawTasks) {
      if (!raw || typeof raw !== "object" || Array.isArray(raw)) continue;
      const task = raw as Record<string, unknown>;
      const id =
        typeof task.id === "number" || typeof task.id === "string"
          ? String(task.id)
          : "";
      const status = typeof task.status === "string" ? task.status.trim() : "";
      if (!id || !status) continue;
      const incoming: LatestOwnedTaskSnapshot = {
        teamName,
        owner: typeof task.owner === "string" ? task.owner.trim() : undefined,
        status,
        updatedAtMs: numberFromUnknown(task.updatedAtMs),
        sourceOrder,
      };
      const key = `${teamName}:${id}`;
      const current = latestByTask.get(key);
      if (!current || shouldReplaceCompletionSnapshot(current, incoming)) {
        latestByTask.set(key, incoming);
      }
    }
  }

  const stateByAgent = new Map<string, OwnedTaskState>();
  for (const task of latestByTask.values()) {
    if (!task.owner) continue;
    const key = teamAgentTaskStateKey(task.teamName, task.owner);
    const state = stateByAgent.get(key) ?? { owned: 0, open: 0 };
    state.owned += 1;
    if (task.status !== "completed") state.open += 1;
    stateByAgent.set(key, state);
  }
  return stateByAgent;
}

function teamAgentTaskStateKey(teamName: string, agentName: string): string {
  return `${teamName.trim().toLowerCase()}:${agentName
    .trim()
    .replace(/^@/, "")
    .toLowerCase()}`;
}

function teamNameFromAgentId(agentId?: string): string | undefined {
  const at = agentId?.lastIndexOf("@") ?? -1;
  if (at < 0) return undefined;
  return agentId?.slice(at + 1).trim() || undefined;
}

function recordFromSubAgentToolBlock(
  block: Extract<ChatBlock, { kind: "tool" }>,
): SubAgentViewRecord | null {
  const subAgent = block.subAgent;
  const name = subAgentNameFromToolBlock(block);
  if (!name) return null;
  const initialMessage = subAgentInitialMessageFromToolBlock(block);
  const baseView: ChatViewState = subAgent?.history
    ? initialSubAgentViewFromHistory(subAgent.history)
    : {
        blocks: [],
        status: block.status === "running" ? "streaming" : "idle",
        streamPhase: block.status === "running" ? "waiting" : "idle",
        lastError: block.status === "error" ? block.output ?? "Agent error" : null,
        turnStartedAtMs: null,
      };
  return {
    id: subAgentViewId(block.id, subAgent?.agentId),
    agentId: subAgent?.agentId,
    name,
    title: subAgentToolTitle(block.summary, name),
    model: subAgent?.model,
    history: subAgent?.history,
    view: appendQueuedMessagesToView(
      seedInitialSubAgentMessage(baseView, block.id, initialMessage),
      subAgent?.queuedMessages,
      name,
    ),
  };
}

function subAgentNameFromToolBlock(
  block: Extract<ChatBlock, { kind: "tool" }>,
): string | null {
  const input = parseJsonRecord(block.argsPretty) ?? parseJsonRecord(block.argsRaw);
  if (block.name === "Agent") {
    const fromArgs = typeof input?.name === "string" ? input.name.trim() : "";
    if (fromArgs) return fromArgs;
  }
  return (
    subAgentNameFromSummary(block.summary) ||
    block.subAgent?.name ||
    (block.name.startsWith("subagent_") ? "Sub-agent" : null)
  );
}

function subAgentTaskFromToolBlock(
  block: Extract<ChatBlock, { kind: "tool" }>,
): string | undefined {
  const input = parseJsonRecord(block.argsPretty) ?? parseJsonRecord(block.argsRaw);
  const value =
    typeof input?.description === "string" && input.description.trim()
      ? input.description
      : typeof input?.prompt === "string" && input.prompt.trim()
        ? input.prompt
        : typeof input?.task === "string"
          ? input.task
          : "";
  return value.trim() || undefined;
}

function subAgentRosterKey(record: SubAgentViewRecord): string {
  if (record.agentId) return `id:${record.agentId}`;
  return `name:${record.name.trim().toLowerCase() || record.id}`;
}

function orderKey(record: SubAgentViewRecord): string {
  return `${record.name.toLowerCase()}-${record.id}`;
}

function statusFromToolBlock(
  block: Extract<ChatBlock, { kind: "tool" }>,
): TeamAgentRosterItem["status"] {
  if (block.status === "running") return "running";
  if (block.status === "error") return "error";
  return "idle";
}

function statusFromSubAgentView(view: ChatViewState): TeamAgentRosterItem["status"] {
  if (view.status === "streaming") return "running";
  if (view.status === "stopped") return view.lastError ? "error" : "stopped";
  if (view.lastError) return "error";
  return "idle";
}

function mergeAgentStatus(
  current: TeamAgentRosterItem["status"] | undefined,
  next: TeamAgentRosterItem["status"],
): TeamAgentRosterItem["status"] {
  if (current === "running" || next === "running") return "running";
  if (current === "error" || next === "error") return "error";
  if (current === "stopped" || next === "stopped") return "stopped";
  if (current === "finished" || next === "finished") return "finished";
  return "idle";
}

function isSubAgentToolName(name: string): boolean {
  return name.startsWith("subagent_") || name === "Agent";
}

function isGenericSubAgentName(value: string): boolean {
  return /^(agent|sub-agent|teammate)$/i.test(value.trim());
}

const TEAM_AGENT_COLORS = [
  "#f72585",
  "#a3e635",
  "#60a5fa",
  "#5eead4",
  "#7bd88f",
  "#ffd166",
  "#b388ff",
  "#ff8a5b",
  "#f472b6",
  "#4cc9f0",
];

function assignUniqueTeamAgentColors(
  agents: TeamAgentRosterItem[],
): TeamAgentRosterItem[] {
  const byTeam = new Map<string, TeamAgentRosterItem[]>();
  for (const agent of agents) {
    const teamName = teamNameFromAgentId(agent.agentId);
    if (!teamName) continue;
    const teamAgents = byTeam.get(teamName) ?? [];
    teamAgents.push(agent);
    byTeam.set(teamName, teamAgents);
  }
  if (byTeam.size === 0) return agents;

  const colorByAgent = new Map<string, string>();
  for (const teamAgents of byTeam.values()) {
    const ordered = [...teamAgents].sort((left, right) =>
      teamColorOrderKey(left).localeCompare(teamColorOrderKey(right)),
    );
    ordered.forEach((agent, index) => {
      colorByAgent.set(subAgentRosterKey(agent), teamAgentColorAt(index));
    });
  }
  return agents.map((agent) => {
    const color = colorByAgent.get(subAgentRosterKey(agent));
    return color ? { ...agent, color } : agent;
  });
}

function teamColorOrderKey(agent: SubAgentViewRecord): string {
  return `${agent.name.trim().toLowerCase() || "agent"}-${
    agent.agentId ?? agent.id
  }`;
}

function teamAgentColorAt(index: number): string {
  if (index < TEAM_AGENT_COLORS.length) return TEAM_AGENT_COLORS[index];
  const hue = Math.round((index * 137.508) % 360);
  return `hsl(${hue} 82% 68%)`;
}

function fallbackAgentColor(value: string): string {
  let hash = 0;
  for (const char of value) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  }
  return TEAM_AGENT_COLORS[hash % TEAM_AGENT_COLORS.length];
}

function agentStatusLabel(status: TeamAgentRosterItem["status"]): string {
  switch (status) {
    case "running":
      return "Running";
    case "error":
      return "Error";
    case "stopped":
      return "Stopped";
    case "finished":
      return "Finished";
    case "idle":
      return "Slept";
  }
}

function compactModelLabel(
  model: ModelRef,
  allModels: readonly ModelEntry[] = MODELS,
): string {
  const entry = modelEntryFromRef(model, allModels);
  return entry?.label ?? model.name;
}

function isTeamAgentId(agentId?: string): boolean {
  return !!agentId && agentId.includes("@");
}

function subAgentViewId(turnId: string, agentId?: string): string {
  return isTeamAgentId(agentId) ? `agent:${agentId}` : turnId;
}

function modelEntryFromRef(
  model: ModelRef,
  allModels: readonly ModelEntry[] = MODELS,
) {
  return (
    allModels.find((entry) => {
      const ref = modelRefFromId(entry.value);
      return ref.provider === model.provider && ref.name === model.name;
    }) ?? null
  );
}

function initialSubAgentViewFromHistory(history: ChatMessage[]): ChatViewState {
  return cleanSwarmUserMessages(initialStateFromHistory(history));
}

function cleanSwarmUserMessages(view: ChatViewState): ChatViewState {
  let changed = false;
  const blocks = view.blocks.map((block) => {
    if (block.kind !== "user-text") return block;
    const text = cleanSwarmInitialMessage(block.text);
    if (text === block.text) return block;
    changed = true;
    return { ...block, text };
  });
  return changed ? { ...view, blocks } : view;
}

function cleanSwarmInitialMessage(text: string): string {
  const trimmed = text.trim();
  const match = trimmed.match(
    /^<agent_team_(kickoff|restart)\b[^>]*>\s*([\s\S]*?)\s*<\/agent_team_\1>$/,
  );
  if (!match) return text;
  return decodeXmlEntities(match[2] ?? "").trim();
}

function seedInitialSubAgentMessage(
  view: ChatViewState,
  id: string,
  initialMessage?: string,
): ChatViewState {
  const text = cleanSwarmInitialMessage(initialMessage ?? "").trim();
  if (!text) return view;
  if (hasTeamMessages(text)) {
    return appendTeamMessageTextToView(view, `subagent-message-${id}`, text);
  }
  const blockId = `subagent-task-${id}`;
  if (
    view.blocks.some(
      (block) =>
        block.kind === "user-text" &&
        (block.id === blockId || block.text.trim() === text),
    )
  ) {
    return view;
  }
  return {
    ...view,
    blocks: [
      ...view.blocks,
      {
        kind: "user-text",
        id: blockId,
        text,
        historyIndex: 0,
      },
    ],
  };
}

function applySubAgentEventToViews(
  current: Map<string, SubAgentViewRecord>,
  event: Extract<AgentEvent, { type: "sub_agent_event" }>,
): Map<string, SubAgentViewRecord> {
  const next = new Map(current);
  const recordId = subAgentViewId(event.id, event.agent_id);
  const existing = next.get(recordId);
  const baseView =
    existing?.view ??
    ({
      blocks: [],
      status: "idle",
      streamPhase: "idle",
      lastError: null,
      turnStartedAtMs: null,
    } satisfies ChatViewState);
  const name = event.agent_name || existing?.name || "Sub-agent";
  const title =
    event.agent_name || !existing || isGenericSubAgentName(existing.name)
      ? subAgentToolTitle(undefined, name)
      : existing.title;
  const baseViewWithTask = seedInitialSubAgentMessage(
    baseView,
    event.id,
    event.initial_message,
  );
  const view =
    event.event.type === "peer_message_received"
      ? appendPeerMessageToView(baseViewWithTask, event.event)
      : appendAgentSleptToView(
          applyEvent(baseViewWithTask, event.event),
          event.event,
          name,
          event.team_name ?? undefined,
        );
  next.set(recordId, {
    id: recordId,
    agentId: event.agent_id || existing?.agentId,
    name,
    title,
    model: event.model ?? existing?.model,
    history: existing?.history,
    view,
  });
  return next;
}

function subAgentEventMatchesActiveView(
  event: Extract<AgentEvent, { type: "sub_agent_event" }>,
  activeSubAgentId: string | null,
): boolean {
  if (!activeSubAgentId) return false;
  return activeSubAgentId === subAgentViewId(event.id, event.agent_id);
}

function subAgentInitialMessageFromToolBlock(
  block: Extract<ChatBlock, { kind: "tool" }>,
): string | undefined {
  const input = parseJsonRecord(block.argsPretty) ?? parseJsonRecord(block.argsRaw);
  if (block.name === "Agent") {
    const prompt = typeof input?.prompt === "string" ? input.prompt.trim() : "";
    const description =
      typeof input?.description === "string" ? input.description.trim() : "";
    if (prompt && description) return `${description}\n\n${prompt}`;
    return prompt || description || undefined;
  }
  const prompt = typeof input?.prompt === "string" ? input.prompt.trim() : "";
  if (prompt) return prompt;
  const legacyTask = typeof input?.task === "string" ? input.task.trim() : "";
  return legacyTask || undefined;
}

function parseJsonRecord(value?: string): Record<string, unknown> | null {
  if (!value) return null;
  try {
    const parsed = JSON.parse(value);
    return parsed && typeof parsed === "object"
      ? (parsed as Record<string, unknown>)
      : null;
  } catch {
    return null;
  }
}

function updateActiveTeamNamesForEvent(
  current: Map<string, Set<string>>,
  conversationId: string,
  event: AgentEvent,
  viewBeforeEvent: ChatViewState,
): Map<string, Set<string>> {
  const change = activeTeamNameChange(event, viewBeforeEvent);
  if (!change) return current;
  return change.action === "add"
    ? addActiveTeamName(current, conversationId, change.teamName)
    : removeActiveTeamName(current, conversationId, change.teamName);
}

function activeTeamNameChange(
  event: AgentEvent,
  viewBeforeEvent: ChatViewState,
): { action: "add" | "remove"; teamName: string } | null {
  if (event.type !== "tool_finished") return null;
  const block = viewBeforeEvent.blocks.find(
    (candidate): candidate is Extract<ChatBlock, { kind: "tool" }> =>
      candidate.kind === "tool" && candidate.id === event.id,
  );
  const teamRunStatus = teamRunStatusFromMeta(event.meta);
  const isTeamStop = block?.name === "TeamStop";
  const isTeamRunSpawn =
    block?.name === "TeamRun" && !teamRunAgentFromArgs(block.argsPretty ?? block.argsRaw);
  const teamName =
    teamNameFromMeta(event.meta) ??
    (isTeamRunSpawn || isTeamStop
      ? teamNameFromTeamRunArgs(block?.argsPretty ?? block?.argsRaw)
      : undefined);
  if (!teamName) return null;
  if (isTeamStop && !event.is_error) {
    return { action: "remove", teamName };
  }
  if (teamRunStatus && teamRunStatus !== "running") {
    return { action: "remove", teamName };
  }
  if (event.is_error && isTeamRunSpawn) {
    return { action: "remove", teamName };
  }
  if (teamRunStatus === "running" || (isTeamRunSpawn && !event.is_error)) {
    return { action: "add", teamName };
  }
  return null;
}

function addActiveTeamName(
  current: Map<string, Set<string>>,
  conversationId: string,
  teamName: string,
): Map<string, Set<string>> {
  const existing = current.get(conversationId);
  if (existing?.has(teamName)) return current;
  const next = new Map(current);
  const names = new Set(existing ?? []);
  names.add(teamName);
  next.set(conversationId, names);
  return next;
}

function removeActiveTeamName(
  current: Map<string, Set<string>>,
  conversationId: string,
  teamName?: string,
): Map<string, Set<string>> {
  const existing = current.get(conversationId);
  if (!existing) return current;
  const next = new Map(current);
  if (!teamName) {
    next.delete(conversationId);
    return next;
  }
  if (!existing.has(teamName)) return current;
  const names = new Set(existing);
  names.delete(teamName);
  if (names.size > 0) {
    next.set(conversationId, names);
  } else {
    next.delete(conversationId);
  }
  return next;
}

function teamRunStatusFromMeta(meta: unknown): string | undefined {
  if (!meta || typeof meta !== "object") return undefined;
  const status = (meta as Record<string, unknown>).teamRunStatus;
  return typeof status === "string" ? status.trim() || undefined : undefined;
}

function teamNameFromMeta(meta: unknown): string | undefined {
  if (!meta || typeof meta !== "object") return undefined;
  const team = (meta as Record<string, unknown>).team;
  if (!team || typeof team !== "object" || Array.isArray(team)) return undefined;
  const name = (team as Record<string, unknown>).name;
  return typeof name === "string" ? name.trim() || undefined : undefined;
}

function teamNameFromTeamRunArgs(args?: string): string | undefined {
  const input = parseJsonRecord(args);
  const teamName = input?.team_name;
  return typeof teamName === "string" ? teamName.trim() || undefined : undefined;
}

function teamRunAgentFromArgs(args?: string): string | undefined {
  const input = parseJsonRecord(args);
  const agent = input?.agent;
  return typeof agent === "string"
    ? agent.replace(/^@/, "").trim() || undefined
    : undefined;
}

function subAgentViewsFromHistory(
  history: ChatMessage[],
): Map<string, SubAgentViewRecord> {
  const next = new Map<string, SubAgentViewRecord>();
  for (const message of history) {
    if (message.role !== "user") continue;
    for (const part of message.parts) {
      if (part.type !== "tool_result") continue;
      for (const [id, record] of subAgentViewsFromToolMeta(
        part.tool_call_id,
        part.meta,
      )) {
        next.set(id, record);
      }
    }
  }
  return next;
}

function subAgentViewsFromToolMeta(
  id: string,
  meta: unknown,
): Map<string, SubAgentViewRecord> {
  const next = new Map<string, SubAgentViewRecord>();
  for (const subAgent of subAgentsFromMeta(id, meta)) {
    const baseView: ChatViewState = subAgent.history
      ? initialSubAgentViewFromHistory(subAgent.history)
      : {
          blocks: [],
          status: "idle",
          streamPhase: "idle",
          lastError: null,
          turnStartedAtMs: null,
        };
    next.set(subAgent.id, {
      id: subAgent.id,
      agentId: subAgent.agentId,
      name: subAgent.name,
      title: subAgentToolTitle(undefined, subAgent.name),
      model: subAgent.model,
      history: subAgent.history,
      view: appendQueuedMessagesToView(baseView, subAgent.queuedMessages, subAgent.name),
    });
  }
  return next;
}

function subAgentsFromMeta(
  id: string,
  meta: unknown,
): SubAgentBlock[] {
  const out: SubAgentBlock[] = [];
  const single = subAgentFromMeta(id, meta);
  if (single) out.push(single);
  if (meta && typeof meta === "object") {
    const rawList = (meta as Record<string, unknown>).subagents;
    if (Array.isArray(rawList)) {
      for (const raw of rawList) {
        if (!raw || typeof raw !== "object") continue;
        const record = raw as Record<string, unknown>;
        const agentId = typeof record.id === "string" ? record.id : undefined;
        const name = typeof record.name === "string" ? record.name : "Sub-agent";
        out.push({
          id: subAgentViewId(`${id}:${agentId ?? name}`, agentId),
          agentId,
          name,
          model:
            record.model && typeof record.model === "object"
              ? (record.model as ModelRef)
              : undefined,
          history: Array.isArray(record.history)
            ? (record.history as ChatMessage[])
            : undefined,
          queuedMessages: queuedMessagesFromRecord(record),
        });
      }
    }
  }
  const seen = new Set<string>();
  return out.filter((agent) => {
    const key = agent.agentId ?? agent.name;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function mergeSubAgentViews(
  current: Map<string, SubAgentViewRecord>,
  stored: Map<string, SubAgentViewRecord>,
): Map<string, SubAgentViewRecord> {
  if (stored.size === 0) return current;
  const next = new Map(current);
  for (const [id, record] of stored) {
    const existing = next.get(id);
    next.set(id, existing ? mergeSubAgentView(existing, record) : record);
  }
  return next;
}

function mergeSubAgentView(
  existing: SubAgentViewRecord,
  stored: SubAgentViewRecord,
): SubAgentViewRecord {
  const useStoredView = shouldUseStoredSubAgentView(existing, stored);
  const primary = useStoredView ? stored : existing;
  const secondary = useStoredView ? existing : stored;
  return {
    ...primary,
    agentId: primary.agentId ?? secondary.agentId,
    model: primary.model ?? secondary.model,
    history: longerSubAgentHistory(primary.history, secondary.history),
    name: isGenericSubAgentName(primary.name) ? secondary.name : primary.name,
    title:
      isGenericSubAgentName(primary.name) || isGenericSubAgentTitle(primary.title)
        ? secondary.title || primary.title
        : primary.title,
  };
}

function shouldUseStoredSubAgentView(
  existing: SubAgentViewRecord,
  stored: SubAgentViewRecord,
): boolean {
  if (existing.view.status === "streaming" && stored.view.status !== "streaming") {
    return false;
  }

  const existingHistoryLength = existing.history?.length ?? 0;
  const storedHistoryLength = stored.history?.length ?? 0;
  if (storedHistoryLength !== existingHistoryLength) {
    return storedHistoryLength > existingHistoryLength;
  }

  const existingHasContent = hasRenderableSubAgentContent(existing.view);
  const storedHasContent = hasRenderableSubAgentContent(stored.view);
  if (storedHasContent !== existingHasContent) {
    return storedHasContent;
  }

  const existingBlockCount = existing.view.blocks.length;
  const storedBlockCount = stored.view.blocks.length;
  if (storedBlockCount !== existingBlockCount) {
    return storedBlockCount > existingBlockCount;
  }

  return existing.view.status !== "streaming";
}

function hasRenderableSubAgentContent(view: ChatViewState): boolean {
  return view.blocks.length > 0 || !!view.lastError;
}

function longerSubAgentHistory(
  left?: ChatMessage[],
  right?: ChatMessage[],
): ChatMessage[] | undefined {
  if (!left?.length) return right?.length ? right : left;
  if (!right?.length) return left;
  return right.length > left.length ? right : left;
}

function isGenericSubAgentTitle(value: string): boolean {
  const normalized = value.trim().toLowerCase();
  return (
    normalized === "agent" ||
    normalized === "sub-agent" ||
    normalized === "agent · agent" ||
    normalized === "agent · sub-agent"
  );
}

function subAgentFromMeta(
  id: string,
  meta: unknown,
): SubAgentBlock | null {
  if (!meta || typeof meta !== "object") return null;
  const raw = (meta as Record<string, unknown>).subagent;
  if (!raw || typeof raw !== "object") return null;
  const record = raw as Record<string, unknown>;
  const agentId = typeof record.id === "string" ? record.id : undefined;
  return {
    id: subAgentViewId(id, agentId),
    agentId,
    name: typeof record.name === "string" ? record.name : "Sub-agent",
    model:
      record.model && typeof record.model === "object"
        ? (record.model as ModelRef)
        : undefined,
    history: Array.isArray(record.history)
      ? (record.history as ChatMessage[])
      : undefined,
    queuedMessages: queuedMessagesFromRecord(record),
  };
}

function subAgentNameFromSummary(summary?: string): string | null {
  if (!summary) return null;
  const parts = summary.split("·").map((part) => part.trim()).filter(Boolean);
  if (parts.length >= 2 && /^sub-agent$/i.test(parts[0])) {
    return parts.slice(1).join(" · ");
  }
  if (parts.length >= 2 && /^agent$/i.test(parts[0])) {
    return parts[1]?.replace(/^@/, "") ?? null;
  }
  return null;
}

type TeamMessageItem = {
  id?: string;
  from?: string;
  to?: string;
  message: string;
};

function appendPeerMessageToView(
  view: ChatViewState,
  event: Extract<AgentEvent, { type: "peer_message_received" }>,
): ChatViewState {
  return appendTeamMessageTextToView(view, `peer-message-${event.id}`, formatTeamMessageXml({
    id: event.id,
    from: event.from,
    to: event.to ?? undefined,
    message: event.message,
  }));
}

function appendAgentSleptToView(
  view: ChatViewState,
  event: AgentEvent,
  agentName: string,
  teamName?: string,
): ChatViewState {
  if (event.type !== "agent_slept") return view;
  if (!teamName) return view;
  const trimmedName = agentName.trim() || "agent";
  const previous = view.blocks[view.blocks.length - 1];
  if (
    previous?.kind === "agent-status" &&
    previous.agentName === trimmedName &&
    previous.status === "slept"
  ) {
    return view;
  }
  return {
    ...view,
    blocks: [
      ...view.blocks,
      {
        kind: "agent-status",
        id: `agent-slept-${trimmedName}-${Date.now()}`,
        agentName: trimmedName,
        status: "slept",
        teamName,
      },
    ],
  };
}

function appendQueuedMessagesToView(
  view: ChatViewState,
  queuedMessages?: SubAgentBlock["queuedMessages"],
  recipient?: string,
): ChatViewState {
  if (!queuedMessages?.length) return view;
  return appendTeamMessageTextToView(
    view,
    "queued-peer-messages",
    queuedMessages
      .map((message) =>
        formatTeamMessageXml({
          ...message,
          to: message.to ?? recipient,
        }),
      )
      .join("\n\n"),
  );
}

function appendTeamMessageTextToView(
  view: ChatViewState,
  id: string,
  text: string,
): ChatViewState {
  const incoming = teamMessagesFromText(text);
  if (!incoming?.length) return view;

  const known = new Set<string>();
  for (const block of view.blocks) {
    if (block.kind !== "user-text") continue;
    for (const message of teamMessagesFromText(block.text) ?? []) {
      for (const key of teamMessageKeys(message)) known.add(key);
    }
  }

  const fresh: TeamMessageItem[] = [];
  for (const message of incoming) {
    const keys = teamMessageKeys(message);
    if (keys.some((key) => known.has(key))) continue;
    fresh.push(message);
    for (const key of keys) known.add(key);
  }
  if (fresh.length === 0) return view;

  return {
    ...view,
    blocks: [
      ...view.blocks,
      {
        kind: "user-text",
        id: fresh.length === 1 && fresh[0].id ? `${id}-${fresh[0].id}` : `${id}-${view.blocks.length}`,
        text: fresh.map(formatTeamMessageXml).join("\n\n"),
        historyIndex: 0,
      },
    ],
  };
}

function teamMessageFromSendMessageTool(
  block: Extract<ChatBlock, { kind: "tool" }>,
  activeAgentName?: string,
): TeamMessageItem | null {
  if (block.name !== "SendMessage" || !activeAgentName?.trim()) return null;
  const input = parseJsonRecord(block.argsPretty) ?? parseJsonRecord(block.argsRaw);
  const message =
    typeof input?.message === "string" ? input.message.trim() : "";
  if (!message) return null;
  const to = typeof input?.to === "string" ? input.to.trim() : "";
  return {
    id: block.id,
    from: activeAgentName.trim(),
    to: to || undefined,
    message,
  };
}

function filterTeamMessagesForAgent(
  messages: TeamMessageItem[] | null,
  agentName?: string,
): TeamMessageItem[] {
  if (!messages?.length) return [];
  const agentKey = normalizedAgentName(agentName);
  if (!agentKey) return messages;
  return messages.filter((message) => teamMessageSentByAgent(message, agentKey));
}

function teamMessageSentByAgent(
  message: TeamMessageItem,
  agentKey: string,
): boolean {
  return normalizedAgentName(message.from) === agentKey;
}

function normalizedAgentName(value?: string): string {
  return value?.trim().replace(/^@/, "").toLowerCase() ?? "";
}

function TeamMessageStack({
  messages,
  onOpenFile,
}: {
  messages: TeamMessageItem[];
  onOpenFile: (path: string) => void;
}) {
  return (
    <div className="team-message-stack">
      {messages.map((message, index) => (
        <div className="team-message" key={message.id ?? `${message.from ?? "peer"}-${index}`}>
          <span className="team-message__icon">
            <Icon icon="solar:chat-round-dots-linear" width={14} height={14} />
          </span>
          <div className="team-message__body">
            <span className="team-message__meta">
              {teamMessageMeta(message)}
            </span>
            <div className="team-message__text">
              <Markdown text={message.message} onOpenFile={onOpenFile} />
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

function teamMessageMeta(message: TeamMessageItem): string {
  const from = message.from?.trim();
  const to = message.to?.trim();
  const fromLabel = from ? `@${from.replace(/^@/, "")}` : "teammate";
  if (!to) return `Message from ${fromLabel}`;
  const toLabel =
    to === "*" ? "all agents" : `@${to.replace(/^@/, "")}`;
  return `Message ${fromLabel} -> ${toLabel}`;
}

function teamMessagesFromText(text: string): TeamMessageItem[] | null {
  if (!hasTeamMessages(text)) return null;
  const messages: TeamMessageItem[] = [];
  for (const match of text.matchAll(/<team_message\b([^>]*)>([\s\S]*?)<\/team_message>/g)) {
    const attrs = match[1] ?? "";
    const message = decodeXmlEntities(match[2].trim());
    if (!message) continue;
    messages.push({
      id: attrValue(attrs, "id"),
      from: attrValue(attrs, "from"),
      to: attrValue(attrs, "to"),
      message,
    });
  }
  for (const match of text.matchAll(/<teammate-message\b([^>]*)>([\s\S]*?)<\/teammate-message>/g)) {
    const attrs = match[1] ?? "";
    const message = decodeXmlEntities(match[2].trim());
    if (!message) continue;
    messages.push({
      id: attrValue(attrs, "id"),
      from: attrValue(attrs, "teammate_id") ?? attrValue(attrs, "from"),
      to: attrValue(attrs, "to"),
      message,
    });
  }
  return messages.length > 0 ? messages : null;
}

function hasTeamMessages(text: string): boolean {
  return text.includes("<team_message") || text.includes("<teammate-message");
}

function queuedMessagesFromRecord(
  record: Record<string, unknown>,
): SubAgentBlock["queuedMessages"] | undefined {
  const raw = record.queuedMessages;
  if (!Array.isArray(raw)) return undefined;
  const messages = raw
    .map((item) => {
      if (!item || typeof item !== "object") return null;
      const message = item as Record<string, unknown>;
      const text = typeof message.message === "string" ? message.message.trim() : "";
      if (!text) return null;
      return {
        id:
          typeof message.id === "string" || typeof message.id === "number"
            ? String(message.id)
            : undefined,
        from: typeof message.from === "string" ? message.from : undefined,
        to: typeof message.to === "string" ? message.to : undefined,
        message: text,
      };
    })
    .filter((item): item is NonNullable<typeof item> => item !== null);
  return messages.length > 0 ? messages : undefined;
}

function attrValue(attrs: string, name: string): string | undefined {
  const match = attrs.match(new RegExp(`${name}="([^"]*)"`));
  return match ? decodeXmlEntities(match[1]).trim() : undefined;
}

function formatTeamMessageXml(message: TeamMessageItem): string {
  const from = message.from?.trim() || "teammate";
  const id = message.id?.trim();
  const to = message.to?.trim();
  const idAttr = id ? ` id="${escapeXmlAttr(id)}"` : "";
  const toAttr = to ? ` to="${escapeXmlAttr(to)}"` : "";
  return `<teammate-message${idAttr} teammate_id="${escapeXmlAttr(from)}"${toAttr}>\n${escapeXmlText(message.message)}\n</teammate-message>`;
}

function teamMessageKeys(message: TeamMessageItem): string[] {
  const fallback = [
    "team-msg",
    message.from?.trim().toLowerCase() ?? "",
    message.to?.trim().toLowerCase() ?? "",
    message.message.trim(),
  ].join(":");
  return message.id ? [`id:${message.id}`, fallback] : [fallback];
}

function escapeXmlText(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function escapeXmlAttr(value: string): string {
  return escapeXmlText(value).replace(/"/g, "&quot;").replace(/'/g, "&apos;");
}

function decodeXmlEntities(value: string): string {
  return value
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&amp;/g, "&");
}

function basename(path: string): string {
  const idx = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return idx >= 0 ? path.slice(idx + 1) : path;
}

function isImagePath(name: string): boolean {
  return /\.(png|jpe?g|gif|webp|svg|bmp|avif|heic|heif)$/i.test(name);
}

function clipboardImageFiles(dataTransfer: DataTransfer): File[] {
  const fromItems: File[] = [];
  for (const item of Array.from(dataTransfer.items)) {
    if (item.kind !== "file") continue;
    if (item.type && !item.type.toLowerCase().startsWith("image/")) continue;
    const file = item.getAsFile();
    if (file && clipboardImageMediaType(file)) fromItems.push(file);
  }
  if (fromItems.length > 0) return fromItems;

  return Array.from(dataTransfer.files).filter((file) =>
    Boolean(clipboardImageMediaType(file)),
  );
}

function clipboardImageMediaType(file: File): string | null {
  const normalized = file.type.split(";")[0]?.trim().toLowerCase();
  if (
    normalized === "image/png" ||
    normalized === "image/jpeg" ||
    normalized === "image/gif" ||
    normalized === "image/webp"
  ) {
    return normalized;
  }
  if (normalized === "image/jpg") return "image/jpeg";

  if (/\.png$/i.test(file.name)) return "image/png";
  if (/\.jpe?g$/i.test(file.name)) return "image/jpeg";
  if (/\.gif$/i.test(file.name)) return "image/gif";
  if (/\.webp$/i.test(file.name)) return "image/webp";
  return null;
}

function pastedImageName(file: File, index: number, mediaType: string): string {
  if (file.name.trim()) return file.name;
  const suffix = index === 0 ? "" : `-${index + 1}`;
  return `pasted-image${suffix}.${extensionForImageMediaType(mediaType)}`;
}

function extensionForImageMediaType(mediaType: string): string {
  if (mediaType === "image/jpeg") return "jpg";
  if (mediaType === "image/gif") return "gif";
  if (mediaType === "image/webp") return "webp";
  return "png";
}

function readFileAsBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () =>
      reject(reader.error ?? new Error("Unable to read pasted image"));
    reader.onload = () => {
      const result = typeof reader.result === "string" ? reader.result : "";
      const comma = result.indexOf(",");
      resolve(comma >= 0 ? result.slice(comma + 1) : result);
    };
    reader.readAsDataURL(file);
  });
}

function attachmentOnlyMessage(attachments: AttachmentInput[]): string {
  if (attachments.length === 0) return "";
  const onlyImages = attachments.every((att) =>
    isImagePath(att.name ?? basename(att.path)),
  );
  if (attachments.length === 1) {
    return onlyImages
      ? "Please inspect the attached image."
      : "Please inspect the attached file.";
  }
  return onlyImages
    ? "Please inspect the attached images."
    : "Please inspect the attached files.";
}

function buildQueuedPrompt({
  id,
  text,
  attachments,
  model,
  thinking,
  mode,
  serviceTier,
  createdAtMs,
}: {
  id?: string;
  text: string;
  attachments: AttachmentInput[];
  model: ModelRef;
  thinking: ThinkingLevel;
  mode: AgentMode;
  serviceTier: ServiceTier | null;
  createdAtMs?: number;
}): QueuedPrompt {
  return {
    id: id ?? nextQueuedPromptId(),
    text,
    attachments: cloneAttachmentInputs(attachments),
    model: cloneModelRef(model),
    thinking,
    mode,
    serviceTier,
    createdAtMs: createdAtMs ?? Date.now(),
  };
}

function buildComposerDraft(
  text: string,
  attachments: Attachment[],
  inlineMentions: InlineMention[],
  editingQueuedPrompt: EditingQueuedPrompt | null,
): ComposerDraft {
  return {
    text,
    attachments: cloneComposerAttachments(attachments),
    inlineMentions: cloneInlineMentions(inlineMentions),
    editingQueuedPrompt,
  };
}

function cloneComposerAttachments(attachments: Attachment[]): Attachment[] {
  return attachments.map((attachment) => ({
    path: attachment.path,
    name: attachment.name,
    origin: attachment.origin,
  }));
}

function cloneInlineMentions(mentions: InlineMention[]): InlineMention[] {
  return mentions.map((mention) => ({
    path: mention.path,
    absolutePath: mention.absolutePath,
    name: mention.name,
  }));
}

function nextQueuedPromptId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return `queue-${crypto.randomUUID()}`;
  }
  return `queue-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function updatePromptQueue(
  current: Map<string, QueuedPrompt[]>,
  conversationId: string,
  updater: (queue: QueuedPrompt[]) => QueuedPrompt[],
): Map<string, QueuedPrompt[]> {
  const existing = current.get(conversationId) ?? EMPTY_QUEUED_PROMPTS;
  const updated = updater(existing);
  const next = new Map(current);
  if (updated.length === 0) {
    next.delete(conversationId);
  } else {
    next.set(conversationId, updated);
  }
  return next;
}

function insertQueuedPrompt(
  queue: QueuedPrompt[],
  prompt: QueuedPrompt,
  index?: number,
): QueuedPrompt[] {
  const withoutExisting = queue.filter((item) => item.id !== prompt.id);
  const targetIndex =
    index === undefined
      ? withoutExisting.length
      : Math.min(Math.max(index, 0), withoutExisting.length);
  return [
    ...withoutExisting.slice(0, targetIndex),
    prompt,
    ...withoutExisting.slice(targetIndex),
  ];
}

function moveQueuedPrompt(
  queue: QueuedPrompt[],
  draggedId: string,
  targetId: string,
): QueuedPrompt[] {
  if (draggedId === targetId) return queue;
  const fromIndex = queue.findIndex((prompt) => prompt.id === draggedId);
  const targetIndex = queue.findIndex((prompt) => prompt.id === targetId);
  if (fromIndex < 0 || targetIndex < 0) return queue;
  const next = queue.slice();
  const [dragged] = next.splice(fromIndex, 1);
  if (!dragged) return queue;
  const targetAfterRemoval = next.findIndex((prompt) => prompt.id === targetId);
  const insertIndex =
    fromIndex < targetIndex ? targetAfterRemoval + 1 : targetAfterRemoval;
  next.splice(Math.max(0, insertIndex), 0, dragged);
  return next;
}

function cloneAttachmentInputs(
  attachments: AttachmentInput[],
): AttachmentInput[] {
  return attachments.map((attachment) => ({
    path: attachment.path,
    name: attachment.name,
  }));
}

function cloneModelRef(model: ModelRef): ModelRef {
  return {
    provider: model.provider,
    name: model.name,
    effort: model.effort,
  };
}

function composerAttachmentsFromQueue(
  attachments: AttachmentInput[],
): Attachment[] {
  return attachments.map((attachment) => ({
    path: attachment.path,
    name: attachment.name ?? basename(attachment.path),
    origin: "manual",
  }));
}

function userAttachmentsFromQueue(
  attachments: AttachmentInput[],
): { path: string; name: string }[] {
  return attachments.map((attachment) => ({
    path: attachment.path,
    name: attachment.name ?? basename(attachment.path),
  }));
}

type QuestionToolBlock = Extract<ChatBlock, { kind: "tool" }>;

type RenderItem =
  | { kind: "block"; block: ChatBlock }
  | { kind: "questionnaire"; id: string; question: QuestionToolBlock };

function renderItemsFromBlocks(blocks: ChatBlock[]): RenderItem[] {
  return blocks.flatMap((block): RenderItem[] => {
    if (block.kind === "tool" && block.hidden) return [];
    if (
      block.kind === "tool" &&
      block.name === "Question" &&
      (block.status === "running" || block.status === "done")
    ) {
      return [{
        kind: "questionnaire",
        id: `qs-${block.id}`,
        question: block,
      }];
    }
    return [{ kind: "block", block }];
  });
}

function toQuestionItem(block: QuestionToolBlock): QuestionItem {
  return {
    id: block.id,
    argsPretty: block.argsPretty,
    status: block.status,
    isError: block.isError,
    answered: block.answered,
    answer: block.answer,
    summary: block.summary,
  };
}

function ChatBlocks({
  blocks,
  onPreviewImage,
  onRewindMessage,
  rewindDisabled,
  rewriteHistoryIndex,
  onOpenFile,
  onAnswerQuestion,
  answerQuestionDisabled,
  allowStopQuestions,
  onPlanKeepUpdating,
  onPlanImplement,
  onPlanImplementWithSwarm,
  onPlanImplementFresh,
  onPlanImplementFreshWithSwarm,
  planActionDisabled,
  agentTeamsEnabled,
  onOpenSubAgent,
  onStopAgentSwarm,
  teamAgents,
  teamCompletionByTeam,
  activeTeamNames,
  activeAgentName,
}: {
  blocks: ChatBlock[];
  onPreviewImage: (path: string) => void;
  onRewindMessage: (block: Extract<ChatBlock, { kind: "user-text" }>) => void;
  rewindDisabled: boolean;
  rewriteHistoryIndex: number | null;
  onOpenFile: (path: string) => void;
  onAnswerQuestion: (
    toolCallId: string,
    answers: QuestionAnswer[],
    options?: { stopQuestions?: boolean },
  ) => void | Promise<void>;
  answerQuestionDisabled: boolean;
  allowStopQuestions: boolean;
  onPlanKeepUpdating: (plan: PlanArtifact) => void;
  onPlanImplement: (plan: PlanArtifact) => void;
  onPlanImplementWithSwarm: (plan: PlanArtifact) => void;
  onPlanImplementFresh: (plan: PlanArtifact) => void;
  onPlanImplementFreshWithSwarm: (plan: PlanArtifact) => void;
  planActionDisabled: boolean;
  agentTeamsEnabled: boolean;
  onOpenSubAgent: (block: Extract<ChatBlock, { kind: "tool" }>) => void;
  onStopAgentSwarm?: (teamName?: string) => void | Promise<void>;
  teamAgents?: ToolCardTeamAgent[];
  teamCompletionByTeam?: Record<string, boolean>;
  activeTeamNames?: ReadonlySet<string>;
  activeAgentName?: string;
}) {
  const deferred = useDeferredValue(blocks);
  const items = useMemo(() => renderItemsFromBlocks(deferred), [deferred]);
  return (
    <>
      {items.map((item, index) => {
        if (item.kind === "questionnaire") {
          return (
            <div
              className="msg"
              data-role="assistant"
              key={item.id + ":" + index}
            >
              <Questionnaire
                questions={[toQuestionItem(item.question)]}
                onAnswerQuestion={onAnswerQuestion}
                disabled={answerQuestionDisabled}
                allowStopQuestions={allowStopQuestions}
              />
            </div>
          );
        }
        return (
          <BlockView
            key={item.block.id + ":" + index}
            block={item.block}
            onPreviewImage={onPreviewImage}
            onRewindMessage={onRewindMessage}
            rewindDisabled={rewindDisabled}
            rewriteHistoryIndex={rewriteHistoryIndex}
            onOpenFile={onOpenFile}
            onPlanKeepUpdating={onPlanKeepUpdating}
            onPlanImplement={onPlanImplement}
            onPlanImplementWithSwarm={onPlanImplementWithSwarm}
            onPlanImplementFresh={onPlanImplementFresh}
            onPlanImplementFreshWithSwarm={onPlanImplementFreshWithSwarm}
            planActionDisabled={planActionDisabled}
            agentTeamsEnabled={agentTeamsEnabled}
            onOpenSubAgent={onOpenSubAgent}
            onStopAgentSwarm={onStopAgentSwarm}
            teamAgents={teamAgents}
            teamCompletionByTeam={teamCompletionByTeam}
            activeTeamNames={activeTeamNames}
            activeAgentName={activeAgentName}
          />
        );
      })}
    </>
  );
}

function BlockView({
  block,
  onPreviewImage,
  onRewindMessage,
  rewindDisabled,
  rewriteHistoryIndex,
  onOpenFile,
  onPlanKeepUpdating,
  onPlanImplement,
  onPlanImplementWithSwarm,
  onPlanImplementFresh,
  onPlanImplementFreshWithSwarm,
  planActionDisabled,
  agentTeamsEnabled,
  onOpenSubAgent,
  onStopAgentSwarm,
  teamAgents,
  teamCompletionByTeam,
  activeTeamNames,
  activeAgentName,
}: {
  block: ChatBlock;
  onPreviewImage: (path: string) => void;
  onRewindMessage: (block: Extract<ChatBlock, { kind: "user-text" }>) => void;
  rewindDisabled: boolean;
  rewriteHistoryIndex: number | null;
  onOpenFile: (path: string) => void;
  onPlanKeepUpdating: (plan: PlanArtifact) => void;
  onPlanImplement: (plan: PlanArtifact) => void;
  onPlanImplementWithSwarm: (plan: PlanArtifact) => void;
  onPlanImplementFresh: (plan: PlanArtifact) => void;
  onPlanImplementFreshWithSwarm: (plan: PlanArtifact) => void;
  planActionDisabled: boolean;
  agentTeamsEnabled: boolean;
  onOpenSubAgent: (block: Extract<ChatBlock, { kind: "tool" }>) => void;
  onStopAgentSwarm?: (teamName?: string) => void | Promise<void>;
  teamAgents?: ToolCardTeamAgent[];
  teamCompletionByTeam?: Record<string, boolean>;
  activeTeamNames?: ReadonlySet<string>;
  activeAgentName?: string;
}) {
  switch (block.kind) {
    case "user-text":
      const teamMessages = teamMessagesFromText(block.text);
      const visibleTeamMessages = filterTeamMessagesForAgent(
        teamMessages,
        activeAgentName,
      );
      if (teamMessages && visibleTeamMessages.length === 0) return null;
      return (
        <div className="msg" data-role="user">
          <div
            className="msg__body user-text"
            data-rewindable={rewindDisabled ? "false" : "true"}
            data-rewriting={
              rewriteHistoryIndex !== null &&
              rewriteHistoryIndex === block.historyIndex
                ? "true"
                : "false"
            }
            role={rewindDisabled ? undefined : "button"}
            tabIndex={rewindDisabled ? undefined : 0}
            title={rewindDisabled ? undefined : "Click to edit from here"}
            aria-label={rewindDisabled ? undefined : "Edit from this message"}
            onClick={rewindDisabled ? undefined : () => onRewindMessage(block)}
            onKeyDown={
              rewindDisabled
                ? undefined
                : (event) => {
                    if (event.key !== "Enter" && event.key !== " ") return;
                    event.preventDefault();
                    onRewindMessage(block);
                  }
            }
          >
            {teamMessages ? (
              <TeamMessageStack
                messages={visibleTeamMessages}
                onOpenFile={onOpenFile}
              />
            ) : (
              <FileLinkedText text={block.text} onOpenFile={onOpenFile} />
            )}
          </div>
          {block.attachments && block.attachments.length > 0 && (
            <div className="msg-attachments">
              {block.attachments.map((att) => {
                const image = isImagePath(att.name);
                return image ? (
                  <button
                    key={att.path}
                    type="button"
                    className="msg-attachment-img"
                    onClick={() => onPreviewImage(att.path)}
                    title={att.name}
                  >
                    <img src={convertFileSrc(att.path)} alt={att.name} />
                  </button>
                ) : (
                  <button
                    type="button"
                    className="msg-attachment-file"
                    key={att.path}
                    title={att.path}
                    onClick={() => onOpenFile(att.path)}
                  >
                    <Icon
                      icon={fileIcon(att.name)}
                      width={14}
                      height={14}
                    />
                    <span>{att.name}</span>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      );
    case "assistant-text":
      return (
        <div className="msg" data-role="assistant">
          <div className="msg__body">
            <Markdown text={block.text} onOpenFile={onOpenFile} />
          </div>
        </div>
      );
    case "compaction-summary":
      return (
        <div className="msg" data-role="user">
          <CompactionSummaryBlock
            text={block.text}
            streaming={block.streaming}
            onOpenFile={onOpenFile}
          />
        </div>
      );
    case "compaction-marker":
      return (
        <div className="msg" data-role="assistant">
          <div className="compaction-marker">
            <Icon icon="solar:archive-check-linear" width={13} height={13} />
            <span>Conversation compacted</span>
            {formatCompactionMarkerTime(block.compactedAtMs) ? (
              <span className="compaction-marker__time">
                {formatCompactionMarkerTime(block.compactedAtMs)}
              </span>
            ) : null}
          </div>
        </div>
      );
    case "plan":
      return (
        <div className="msg" data-role="assistant">
          <PlanCard
            artifact={block.artifact}
            disabled={planActionDisabled}
            agentTeamsEnabled={agentTeamsEnabled}
            onOpenFile={onOpenFile}
            onKeepUpdating={onPlanKeepUpdating}
            onImplement={onPlanImplement}
            onImplementWithSwarm={onPlanImplementWithSwarm}
            onImplementFresh={onPlanImplementFresh}
            onImplementFreshWithSwarm={onPlanImplementFreshWithSwarm}
          />
        </div>
      );
    case "plan-writing":
      return (
        <div className="msg" data-role="assistant">
          <div className="plan-writing-card">
            <div className="plan-writing-card__head">
              <span className="tool-card__spinner" />
              <span className="plan-writing-card__title">{block.label}</span>
            </div>
            {block.text.trim() ? (
              <div className="plan-writing-card__body">
                <Markdown text={block.text} onOpenFile={onOpenFile} />
              </div>
            ) : null}
          </div>
        </div>
      );
    case "thinking":
      return (
        <div className="msg" data-role="assistant">
          <AIThinkingBlock
            content={block.text}
            isStreaming={block.streaming}
            durationMs={block.durationMs}
            onOpenFile={onOpenFile}
          />
        </div>
      );
    case "tool":
      if (
        !block.isError &&
        (block.name === "ToDoList" ||
          block.name === "TaskCreate" ||
          block.name === "TaskList" ||
          block.name === "TaskUpdate")
      ) {
        return null;
      }
      const preparingQuestion =
        block.name === "Question" && block.status === "running";
      const sentTeamMessage = teamMessageFromSendMessageTool(
        block,
        activeAgentName,
      );
      if (sentTeamMessage && !block.isError && block.status !== "running") {
        return (
          <div className="msg" data-role="assistant">
            <div className="msg__body user-text">
              <TeamMessageStack
                messages={[sentTeamMessage]}
                onOpenFile={onOpenFile}
              />
            </div>
          </div>
        );
      }
      return (
        <div className="msg" data-role="assistant">
          <ToolCard
            name={block.name}
            status={block.status}
            summary={preparingQuestion ? "Preparing question" : block.summary}
            argsPretty={preparingQuestion ? undefined : block.argsPretty}
            output={preparingQuestion ? undefined : block.output}
            isError={block.isError}
            cleaned={block.cleaned}
            fileChanges={block.fileChanges}
            liveFileChange={block.liveFileChange}
            images={block.images}
            meta={block.meta}
            onOpenFile={onOpenFile}
            onStopTeam={
              block.name === "TeamRun" ? onStopAgentSwarm : undefined
            }
            teamAgents={teamAgents}
            teamCompletionByTeam={teamCompletionByTeam}
            activeTeamNames={activeTeamNames}
            onOpenSubAgent={
              block.name.startsWith("subagent_") || block.name === "Agent"
                ? () => onOpenSubAgent(block)
                : undefined
            }
            subAgentName={block.subAgent?.name}
          />
        </div>
      );
    case "turn-duration":
      return (
        <div className="msg" data-role="assistant">
          <div className="turn-duration">
            <Icon icon="solar:clock-circle-linear" width={13} height={13} />
            <span>Worked for {formatTurnDuration(block.durationMs)}</span>
          </div>
        </div>
      );
    case "agent-status":
      return (
        <div className="msg" data-role="assistant">
          <div className="agent-status-line">
            <span className="agent-status-line__name">@{block.agentName}</span>
            <span className="agent-status-line__text">{block.status}</span>
          </div>
        </div>
      );
  }
}

type PlanImplementMode = "continue" | "fresh";

function PlanSwarmGlyph() {
  return (
    <svg
      width="18"
      height="18"
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

function PlanCard({
  artifact,
  disabled,
  agentTeamsEnabled,
  onOpenFile,
  onKeepUpdating,
  onImplement,
  onImplementWithSwarm,
  onImplementFresh,
  onImplementFreshWithSwarm,
}: {
  artifact: PlanArtifact;
  disabled: boolean;
  agentTeamsEnabled: boolean;
  onOpenFile: (path: string) => void;
  onKeepUpdating: (plan: PlanArtifact) => void;
  onImplement: (plan: PlanArtifact) => void;
  onImplementWithSwarm: (plan: PlanArtifact) => void;
  onImplementFresh: (plan: PlanArtifact) => void;
  onImplementFreshWithSwarm: (plan: PlanArtifact) => void;
}) {
  const [step, setStep] = useState<"choose" | "runner">("choose");
  const [implementMode, setImplementMode] =
    useState<PlanImplementMode>("continue");

  const startImplement = (mode: PlanImplementMode) => {
    if (disabled) return;
    setImplementMode(mode);
    setStep("runner");
  };

  const launchNormal = () => {
    if (disabled) return;
    if (implementMode === "continue") onImplement(artifact);
    else onImplementFresh(artifact);
  };

  const launchSwarm = () => {
    if (disabled || !agentTeamsEnabled) return;
    if (implementMode === "continue") onImplementWithSwarm(artifact);
    else onImplementFreshWithSwarm(artifact);
  };

  return (
    <div className="plan-card" data-step={step}>
      <div className="plan-card__head">
        <span className="plan-card__title">
          <Icon
            icon="solar:clipboard-list-bold-duotone"
            width={15}
            height={15}
          />
          <span>{artifact.title ?? "Plan created"}</span>
        </span>
        <button
          type="button"
          className="plan-card__view"
          onClick={() => onOpenFile(artifact.path)}
        >
          View
        </button>
      </div>

      {step === "choose" ? (
        <div className="plan-card__actions">
          <button
            type="button"
            className="plan-card__button"
            onClick={() => onKeepUpdating(artifact)}
            disabled={disabled}
          >
            <span>Keep updating</span>
          </button>
          <button
            type="button"
            className="plan-card__button"
            data-primary="true"
            onClick={() => startImplement("continue")}
            disabled={disabled}
          >
            <span>Implement the plan</span>
          </button>
          <button
            type="button"
            className="plan-card__button"
            data-primary="true"
            onClick={() => startImplement("fresh")}
            disabled={disabled}
          >
            <span>Implement the plan &amp; clear context</span>
          </button>
        </div>
      ) : (
        <div className="plan-card__runner">
          <div className="plan-card__runner-head">
            <button
              type="button"
              className="plan-card__back"
              onClick={() => setStep("choose")}
              aria-label="Back"
              title="Back"
            >
              <Icon icon="solar:alt-arrow-left-linear" width={13} height={13} />
            </button>
            <span className="plan-card__runner-label">
              {implementMode === "continue"
                ? "Implement the plan · choose runner"
                : "Implement the plan & clear context · choose runner"}
            </span>
          </div>
          <div className="plan-card__tiles">
            <button
              type="button"
              className="plan-card__tile"
              onClick={launchNormal}
              disabled={disabled}
            >
              <span className="plan-card__tile-icon">
                <Icon
                  icon="solar:bolt-circle-bold-duotone"
                  width={20}
                  height={20}
                />
              </span>
              <span className="plan-card__tile-text">
                <span className="plan-card__tile-title">Normal</span>
                <span className="plan-card__tile-sub">
                  Single agent works through the plan.
                </span>
              </span>
            </button>
            <span
              className="plan-card__tile-wrap"
              title={
                agentTeamsEnabled
                  ? undefined
                  : AGENT_TEAMS_DISABLED_TITLE
              }
            >
              <button
                type="button"
                className="plan-card__tile"
                data-variant="swarm"
                onClick={launchSwarm}
                disabled={disabled || !agentTeamsEnabled}
              >
                <span className="plan-card__tile-icon">
                  <PlanSwarmGlyph />
                </span>
                <span className="plan-card__tile-text">
                  <span className="plan-card__tile-title">Agent swarm</span>
                  <span className="plan-card__tile-sub">
                    Multiple teammates split the work in parallel.
                  </span>
                </span>
              </button>
            </span>
          </div>
        </div>
      )}
    </div>
  );
}

function CompactionSummaryBlock({
  text,
  streaming,
  onOpenFile,
}: {
  text: string;
  streaming?: boolean;
  onOpenFile: (path: string) => void;
}) {
  const [open, setOpen] = useState(streaming ?? false);
  const wasStreamingRef = useRef(streaming ?? false);
  useEffect(() => {
    const wasStreaming = wasStreamingRef.current;
    const isStreamingNow = streaming ?? false;
    wasStreamingRef.current = isStreamingNow;
    if (isStreamingNow && !wasStreaming) {
      setOpen(true);
      return;
    }
    if (wasStreaming && !isStreamingNow && text.trim()) setOpen(false);
  }, [streaming, text]);
  return (
    <div className="tool-card">
      <div
        className="tool-card__head"
        data-cleaned="false"
        data-clickable="true"
        onClick={() => setOpen((value) => !value)}
      >
        {streaming ? (
          <span className="tool-card__spinner" />
        ) : (
          <span className="tool-card__glyph">
            <Icon icon="solar:archive-linear" width={13} height={13} />
          </span>
        )}
        <span className="tool-card__title">Compacted context</span>
        <span className="tool-card__caret" data-open={open ? "true" : "false"}>
          <Icon
            icon={
              open
                ? "solar:alt-arrow-down-linear"
                : "solar:alt-arrow-right-linear"
            }
            width={12}
            height={12}
          />
        </span>
      </div>
      {open && (
        <div className="tool-card__body">
          <div className="compaction-summary__body">
            {text.trim() ? (
              <Markdown text={text} onOpenFile={onOpenFile} />
            ) : (
              <span>Compacting context…</span>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function formatTurnDuration(durationMs: number): string {
  const totalSeconds = Math.max(0, Math.round(durationMs / 1000));
  if (totalSeconds < 1) return "<1s";
  if (totalSeconds < 60) return `${totalSeconds}s`;

  const totalMinutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (totalMinutes < 60) {
    return seconds === 0 ? `${totalMinutes}m` : `${totalMinutes}m ${seconds}s`;
  }

  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return minutes === 0 ? `${hours}h` : `${hours}h ${minutes}m`;
}

function formatCompactionMarkerTime(value?: number): string | null {
  if (typeof value !== "number" || !Number.isFinite(value)) return null;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  const now = new Date();
  const sameDay =
    date.getFullYear() === now.getFullYear() &&
    date.getMonth() === now.getMonth() &&
    date.getDate() === now.getDate();
  return date.toLocaleString(undefined, {
    month: sameDay ? undefined : "short",
    day: sameDay ? undefined : "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatFullTokenCount(value: number): string {
  const digits = String(Math.max(0, Math.round(value)));
  return digits.replace(/\B(?=(\d{3})+(?!\d))/g, " ");
}

function formatCompactTokenCount(value: number): string {
  const rounded = Math.max(0, Math.round(value));
  if (rounded < 1_000) return rounded.toLocaleString("en-US");
  const compact = rounded / 1_000;
  const digits = compact < 10 ? 1 : 0;
  return `${compact.toFixed(digits).replace(/\.0$/, "")}K`;
}

function contextBreakdownColor(key: string): string {
  return CONTEXT_BREAKDOWN_COLORS[key] ?? "#85888f";
}

function collectComposerAttachments(
  attachments: Attachment[],
  inlineMentions: InlineMention[],
): { path: string; name: string }[] {
  const seenPaths = new Set<string>();
  const currentAttachments: { path: string; name: string }[] = [];
  for (const att of attachments) {
    if (seenPaths.has(att.path)) continue;
    seenPaths.add(att.path);
    currentAttachments.push({ path: att.path, name: att.name });
  }
  for (const mention of inlineMentions) {
    if (seenPaths.has(mention.absolutePath)) continue;
    seenPaths.add(mention.absolutePath);
    currentAttachments.push({
      path: mention.absolutePath,
      name: mention.name,
    });
  }
  return currentAttachments;
}

function mergeAttachments(
  prev: Attachment[],
  incoming: Attachment[],
): Attachment[] {
  const byPath = new Map(prev.map((a) => [a.path, a]));
  for (const item of incoming) {
    byPath.set(item.path, item);
  }
  return Array.from(byPath.values());
}

function relativizePath(workspacePath: string, candidate: string): string | null {
  if (!workspacePath || !candidate) return null;
  const root = workspacePath.replace(/[\\/]+$/, "");
  const norm = candidate.replace(/\\/g, "/");
  const rootSlash = root.replace(/\\/g, "/");
  if (norm === rootSlash) return "";
  if (norm.startsWith(rootSlash + "/")) {
    return norm.slice(rootSlash.length + 1);
  }
  return null;
}

function renderMentionHighlights(
  text: string,
  mentions: { path: string }[],
): ReactNode {
  if (mentions.length === 0) return text + "\u200B";
  const regions: { start: number; end: number }[] = [];
  for (const m of mentions) {
    if (!m.path) continue;
    const needle = `@${m.path}`;
    let idx = 0;
    while (idx < text.length) {
      const found = text.indexOf(needle, idx);
      if (found < 0) break;
      const after = text[found + needle.length] ?? "";
      const isBoundary = after === "" || /[\s.,;:!?)\]}'"`]/.test(after);
      if (isBoundary) {
        regions.push({ start: found, end: found + needle.length });
      }
      idx = found + needle.length;
    }
  }
  if (regions.length === 0) return text + "\u200B";
  regions.sort((a, b) => a.start - b.start);
  const merged: typeof regions = [];
  for (const r of regions) {
    if (merged.length === 0 || r.start >= merged[merged.length - 1].end) {
      merged.push(r);
    }
  }
  const parts: ReactNode[] = [];
  let cursor = 0;
  for (const r of merged) {
    if (r.start > cursor) parts.push(text.slice(cursor, r.start));
    parts.push(
      <span className="mention-pill" key={`m-${r.start}`}>
        {text.slice(r.start, r.end)}
      </span>,
    );
    cursor = r.end;
  }
  if (cursor < text.length) parts.push(text.slice(cursor));
  parts.push("\u200B");
  return parts;
}
