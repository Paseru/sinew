import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  startTransition,
} from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Icon } from "@iconify/react";
import { api } from "../lib/ipc";
import { modelRefWithThinking, thinkingFromRef } from "../lib/models";
import { recordRecent } from "../lib/recents";
import { Splitter } from "./Splitter";
import { FileTree, type FileTreeHandle } from "./FileTree";
import { ConversationList } from "./ConversationList";
import { GitPanel, GitMark } from "./GitPanel";
import { EditorPane } from "./EditorPane";
import { SettingsPane } from "./SettingsPane";
import { TerminalPanel } from "./TerminalPanel";
import { SearchPane } from "./SearchPane";
import { ChatPane, type ExternalDropFeed } from "./chat/ChatPane";
import { ClaakeCodeMark } from "./ClaakeCodeMark";
import { useTheme } from "../lib/theme";
import { UpdateBadge } from "./UpdateBadge";
import { WindowControls, isWindowsPlatform } from "./WindowControls";
import type {
  ActiveTurnSummary,
  ActiveTurnsChangedPayload,
  AgentEvent,
  AgentMode,
  ConversationEventPayload,
  ConversationSummary,
  EditorRevealTarget,
  EditorTab,
  FileChange,
  MessageVisibility,
  PlanArtifact,
  PlanControl,
  SavedConversation,
  ServiceTier,
  ThinkingLevel,
  WorkspaceBootstrap,
  WorkspaceEntry,
  WorkspaceFileChangedPayload,
} from "../types";

type Props = {
  bootstrap: WorkspaceBootstrap;
  onSwitchWorkspace: () => void;
  onBootstrapReplace: (b: WorkspaceBootstrap) => void;
};

const INITIAL_LEFT = 280;
const INITIAL_RIGHT = 420;
const MIN_COL = 220;
const MAX_COL_RATIO = 0.6;
const INITIAL_SPLIT_TOP = 0.55;
const INITIAL_TERMINAL_HEIGHT = 240;
const MIN_TERMINAL_HEIGHT = 140;
const MAX_TERMINAL_RATIO = 0.92;
const TERMINAL_OPEN_EVENT = "terminal-open-requested";
const SEND_BUSY_RETRY_DELAYS_MS = [160, 320, 640, 1000, 1400];
const COMPACTION_CONTINUATION_PROMPT =
  "Continue from the compacted context. Do not repeat completed work. Pick up exactly where you left off and proceed with the next useful step.";
const GOAL_COMPACTION_CONTINUATION_PROMPT =
  "Continue working toward the active goal from the compacted context. Do not repeat completed work. If the goal is now truly complete, audit it and call update_goal with status complete.";
const IS_WINDOWS = isWindowsPlatform();

export function Workspace({
  bootstrap,
  onSwitchWorkspace,
  onBootstrapReplace,
}: Props) {
  const workspacePath = bootstrap.workspace.path;
  const { theme, toggleTheme } = useTheme();

  const [conversations, setConversations] = useState<ConversationSummary[]>(
    bootstrap.conversations,
  );
  const [activeConv, setActiveConv] = useState<SavedConversation>(
    bootstrap.activeConversation,
  );
  const [, setGlobalModeModelSettings] = useState(
    bootstrap.modeModelSettings,
  );
  const [streamingConversationIds, setStreamingConversationIds] = useState<
    Set<string>
  >(() => new Set());
  const [streamingModelsByConversation, setStreamingModelsByConversation] =
    useState<Map<string, SavedConversation["model"]>>(() => new Map());
  const lastAgentEventSequenceByConversationRef = useRef<Map<string, number>>(
    new Map(),
  );
  const replayActiveTurnEventsRef = useRef<
    (conversationId: string, afterSequence?: number) => Promise<void>
  >(async () => {});
  const activeConvIdRef = useRef(bootstrap.activeConversation.id);
  const workspacePathRef = useRef(workspacePath);
  const navigationSeqRef = useRef(0);
  // Bottom-sidebar tab — "conversations" preserves the existing default.
  // Both panels stay mounted (display:none on the inactive one) so users
  // can flip between them without losing in-progress form input.
  const [bottomTab, setBottomTab] = useState<"conversations" | "git">(
    "conversations",
  );

  useEffect(() => {
    activeConvIdRef.current = activeConv.id;
  }, [activeConv.id]);

  useEffect(() => {
    workspacePathRef.current = workspacePath;
  }, [workspacePath]);

  useEffect(() => {
    navigationSeqRef.current += 1;
    workspacePathRef.current = bootstrap.workspace.path;
    activeConvIdRef.current = bootstrap.activeConversation.id;
    setConversations(bootstrap.conversations);
    setActiveConv(bootstrap.activeConversation);
    setGlobalModeModelSettings(bootstrap.modeModelSettings);
  }, [bootstrap]);

  useEffect(() => {
    navigationSeqRef.current += 1;
    setStreamingConversationIds(new Set());
    setStreamingModelsByConversation(new Map());
    lastAgentEventSequenceByConversationRef.current.clear();
  }, [workspacePath]);

  const markConversationStreaming = useCallback((id: string, active: boolean) => {
    if (!id) return;
    setStreamingConversationIds((prev) => {
      if (prev.has(id) === active) return prev;
      const next = new Set(prev);
      if (active) {
        next.add(id);
      } else {
        next.delete(id);
      }
      return next;
    });
    if (!active) {
      setStreamingModelsByConversation((prev) => {
        if (!prev.has(id)) return prev;
        const next = new Map(prev);
        next.delete(id);
        return next;
      });
    }
  }, []);

  const markConversationStreamingModel = useCallback(
    (id: string, model: SavedConversation["model"], thinking: ThinkingLevel) => {
      if (!id) return;
      const selected = modelRefWithThinking(model, thinking);
      setStreamingModelsByConversation((prev) => {
        const next = new Map(prev);
        next.set(id, selected);
        return next;
      });
    },
    [],
  );

  const refreshConversationList = useCallback(async () => {
    const workspaceAtRequest = workspacePath;
    try {
      const summaries = await api.listConversations(workspaceAtRequest);
      if (workspacePathRef.current !== workspaceAtRequest) return;
      setConversations(summaries);
    } catch (err) {
      console.error(err);
    }
  }, [workspacePath]);

  const selectConversation = useCallback(
    async (id: string) => {
      if (id === activeConv.id) return;
      const seq = ++navigationSeqRef.current;
      try {
        const loaded = await api.loadConversation(workspacePath, id);
        if (seq !== navigationSeqRef.current) return;
        if (loaded.id !== id || loaded.workspaceId !== workspacePath) return;
        activeConvIdRef.current = loaded.id;
        setActiveConv(loaded);
        const last = lastAgentEventSequenceByConversationRef.current.get(id) ?? 0;
        if (streamingConversationIds.has(id)) {
          void replayActiveTurnEventsRef.current(id, last).catch((err) =>
            console.error(err),
          );
        }
      } catch (err) {
        console.error(err);
      }
    },
    [workspacePath, activeConv.id, streamingConversationIds],
  );

  const createConversation = useCallback(async () => {
    const seq = ++navigationSeqRef.current;
    try {
      const next = await api.createConversation(workspacePath);
      if (seq !== navigationSeqRef.current) return;
      if (next.workspace.path !== workspacePath) return;
      activeConvIdRef.current = next.activeConversation.id;
      setConversations(next.conversations);
      setActiveConv(next.activeConversation);
      setGlobalModeModelSettings(next.modeModelSettings);
    } catch (err) {
      console.error(err);
    }
  }, [workspacePath]);

  const renameConversation = useCallback(
    async (id: string, title: string) => {
      try {
        const next = await api.renameConversation(workspacePath, id, title);
        setConversations(next);
      } catch (err) {
        console.error(err);
      }
    },
    [workspacePath],
  );

  const refreshConversationAfterMessageStart = useCallback(
    async (workspaceAtRequest: string, conversationId: string) => {
      const [loaded, summaries] = await Promise.all([
        api.loadConversation(workspaceAtRequest, conversationId),
        api.listConversations(workspaceAtRequest),
      ]);
      if (workspacePathRef.current !== workspaceAtRequest) return;

      startTransition(() => {
        if (
          loaded.id === conversationId &&
          loaded.workspaceId === workspaceAtRequest &&
          activeConvIdRef.current === conversationId
        ) {
          setActiveConv((current) =>
            current.id === conversationId ? loaded : current,
          );
        }
        setConversations(summaries);
      });
    },
    [],
  );

  const applyOptimisticConversationTitle = useCallback(
    (conversationId: string, title: string) => {
      const updatedAtMs = Date.now();
      setActiveConv((current) =>
        current.id === conversationId ? { ...current, title } : current,
      );
      setConversations((current) =>
        sortConversationSummaries(
          current.map((conversation) =>
            conversation.id === conversationId
              ? {
                  ...conversation,
                  title,
                  updatedAtMs: Math.max(conversation.updatedAtMs, updatedAtMs),
                }
              : conversation,
          ),
        ),
      );
    },
    [],
  );

  const deleteConversation = useCallback(
    async (id: string) => {
      if (streamingConversationIds.has(id)) return;
      const seq = ++navigationSeqRef.current;
      try {
        const next = await api.deleteConversation(workspacePath, id);
        if (seq !== navigationSeqRef.current) return;
        if (next.workspace.path !== workspacePath) return;
        activeConvIdRef.current = next.activeConversation.id;
        setConversations(next.conversations);
        setActiveConv(next.activeConversation);
        setGlobalModeModelSettings(next.modeModelSettings);
        onBootstrapReplace(next);
      } catch (err) {
        console.error(err);
        if (seq === navigationSeqRef.current) {
          navigationSeqRef.current += 1;
        }
      }
    },
    [workspacePath, onBootstrapReplace, streamingConversationIds],
  );

  // ---------------- Editor tabs ----------------
  const [tabs, setTabs] = useState<EditorTab[]>([]);
  const [activeTabIndex, setActiveTabIndex] = useState<number>(-1);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsActive, setSettingsActive] = useState(false);
  const [fileTreeRefreshToken, setFileTreeRefreshToken] = useState(0);
  const [fileSearchOpen, setFileSearchOpen] = useState(false);
  const [pendingRootCreate, setPendingRootCreate] = useState<
    "file" | "directory" | null
  >(null);
  const fileTreeRef = useRef<FileTreeHandle | null>(null);
  const [editorRevealTarget, setEditorRevealTarget] =
    useState<EditorRevealTarget | null>(null);
  const tabsRef = useRef(tabs);
  const fileTreeRefreshTimerRef = useRef<number | null>(null);
  const revealSeqRef = useRef(0);
  tabsRef.current = tabs;

  useEffect(() => {
    if (!pendingRootCreate || fileSearchOpen) return;
    const handle = fileTreeRef.current;
    if (!handle) return;
    handle.startCreateRoot(pendingRootCreate);
    setPendingRootCreate(null);
  }, [pendingRootCreate, fileSearchOpen]);

  const startRootCreate = useCallback(
    (kind: "file" | "directory") => {
      if (fileSearchOpen) {
        setFileSearchOpen(false);
        setPendingRootCreate(kind);
        return;
      }
      const handle = fileTreeRef.current;
      if (handle) {
        handle.startCreateRoot(kind);
      } else {
        setPendingRootCreate(kind);
      }
    },
    [fileSearchOpen],
  );

  useEffect(() => {
    tabsRef.current = tabs;
  }, [tabs]);

  const refreshFileTree = useCallback(() => {
    setFileTreeRefreshToken((value) => value + 1);
  }, []);

  const refreshFileTreeSoon = useCallback(() => {
    if (fileTreeRefreshTimerRef.current !== null) return;
    fileTreeRefreshTimerRef.current = window.setTimeout(() => {
      fileTreeRefreshTimerRef.current = null;
      refreshFileTree();
    }, 120);
  }, [refreshFileTree]);

  useEffect(() => {
    return () => {
      if (fileTreeRefreshTimerRef.current !== null) {
        window.clearTimeout(fileTreeRefreshTimerRef.current);
      }
    };
  }, []);

  const refreshOpenTabFromDisk = useCallback(async (relativePath: string) => {
    if (!relativePath) return;
    const workspaceAtRequest = workspacePathRef.current;
    if (
      !tabsRef.current.some((tab) => tab.relativePath === relativePath)
    ) {
      return;
    }

    try {
      const doc = await api.readFile(workspaceAtRequest, relativePath);
      if (workspacePathRef.current !== workspaceAtRequest) return;
      setTabs((prev) => {
        const idx = prev.findIndex((tab) => tab.relativePath === relativePath);
        if (idx < 0) return prev;
        const next = prev.slice();
        const tab = next[idx];
        next[idx] = tab.dirty
          ? { ...tab, doc }
          : {
              ...tab,
              doc,
              buffer: doc.content ?? "",
            };
        return next;
      });
    } catch (err) {
      console.error(err);
      if (workspacePathRef.current !== workspaceAtRequest) return;
      setTabs((prev) =>
        prev.filter(
          (tab) => tab.dirty || tab.relativePath !== relativePath,
        ),
      );
    }
  }, []);

  const refreshChangedFiles = useCallback(
    (changes: FileChange[]) => {
      if (changes.length === 0) return;
      refreshFileTreeSoon();
      const seen = new Set<string>();
      for (const change of changes) {
        if (!change.relativePath || seen.has(change.relativePath)) continue;
        seen.add(change.relativePath);
        void refreshOpenTabFromDisk(change.relativePath);
      }
    },
    [refreshFileTreeSoon, refreshOpenTabFromDisk],
  );

  useEffect(() => {
    let disposed = false;
    void api.watchWorkspace(workspacePath).catch((err) => {
      if (!disposed) {
        console.warn("workspace watcher unavailable", err);
      }
    });
    return () => {
      disposed = true;
      void api.unwatchWorkspace(workspacePath).catch((err) => {
        console.warn("workspace watcher cleanup failed", err);
      });
    };
  }, [workspacePath]);

  useEffect(() => {
    const onFocus = () => refreshFileTree();
    const onVisibility = () => {
      if (document.visibilityState === "visible") refreshFileTree();
    };
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [refreshFileTree]);

  const hasStreamingConversation = streamingConversationIds.size > 0;
  useEffect(() => {
    if (!hasStreamingConversation) return;
    const interval = window.setInterval(refreshFileTreeSoon, 1000);
    return () => window.clearInterval(interval);
  }, [hasStreamingConversation, refreshFileTreeSoon]);

  const openFile = useCallback(
    async (
      entry: WorkspaceEntry,
      reveal?: Omit<EditorRevealTarget, "id" | "relativePath">,
    ) => {
      if (entry.kind !== "file") return;
      const queueReveal = () => {
        if (!reveal) return;
        setEditorRevealTarget({
          ...reveal,
          id: ++revealSeqRef.current,
          relativePath: entry.relativePath,
        });
      };
      const existing = tabs.findIndex(
        (t) => t.relativePath === entry.relativePath,
      );
      if (existing >= 0) {
        setActiveTabIndex(existing);
        setSettingsActive(false);
        queueReveal();
        return;
      }
      try {
        const doc = await api.readFile(workspacePath, entry.relativePath);
        const newTab: EditorTab = {
          relativePath: entry.relativePath,
          doc,
          buffer: doc.content ?? "",
          dirty: false,
        };
        setTabs((prev) => {
          const existingIndex = prev.findIndex(
            (t) => t.relativePath === entry.relativePath,
          );
          if (existingIndex >= 0) {
            setActiveTabIndex(existingIndex);
            setSettingsActive(false);
            return prev;
          }
          const next = [...prev, newTab];
          setActiveTabIndex(next.length - 1);
          setSettingsActive(false);
          return next;
        });
        queueReveal();
      } catch (err) {
        console.error(err);
      }
    },
    [workspacePath, tabs],
  );

  const activateFileTab = useCallback((index: number) => {
    setActiveTabIndex(index);
    setSettingsActive(false);
  }, []);

  const openSettings = useCallback((section?: "providers") => {
    setSettingsOpen(true);
    setSettingsActive(true);
    // Pickup hook for SettingsPane: when the caller wants to land on a
    // specific section (e.g. the composer's "Connect a provider" CTA),
    // fan it out via a window event so the pane can react regardless of
    // whether it just mounted or was already open.
    if (section) {
      window.dispatchEvent(
        new CustomEvent("claakecode:open-settings-section", {
          detail: { section },
        }),
      );
    }
  }, []);

  const closeSettings = useCallback(() => {
    setSettingsOpen(false);
    setSettingsActive(false);
  }, []);

  const openChatFile = useCallback(
    (rawPath: string) => {
      const relativePath = chatPathToRelative(rawPath, workspacePath);
      if (!relativePath) return;
      void openFile({
        name: basename(relativePath),
        relativePath,
        absolutePath: `${workspacePath}/${relativePath}`,
        kind: "file",
        hasChildren: false,
      });
    },
    [openFile, workspacePath],
  );

  // Open an arbitrary absolute path in a *read-only* Monaco tab. Used when
  // the user cmd+clicks a path in the terminal that points outside of the
  // active workspace.
  const openExternalFile = useCallback(
    async (
      absolutePath: string,
      reveal?: { lineNumber: number; columnStart: number; columnEnd: number },
    ) => {
      const queueReveal = () => {
        if (!reveal) return;
        setEditorRevealTarget({
          ...reveal,
          id: ++revealSeqRef.current,
          relativePath: absolutePath,
          query: "",
        });
      };
      const existing = tabsRef.current.findIndex(
        (t) => t.relativePath === absolutePath,
      );
      if (existing >= 0) {
        setActiveTabIndex(existing);
        setSettingsActive(false);
        queueReveal();
        return;
      }
      try {
        const doc = await api.readExternalFile(absolutePath);
        const newTab: EditorTab = {
          relativePath: absolutePath,
          doc,
          buffer: doc.content ?? "",
          dirty: false,
          external: true,
        };
        setTabs((prev) => {
          const existingIndex = prev.findIndex(
            (t) => t.relativePath === absolutePath,
          );
          if (existingIndex >= 0) {
            setActiveTabIndex(existingIndex);
            setSettingsActive(false);
            return prev;
          }
          const next = [...prev, newTab];
          setActiveTabIndex(next.length - 1);
          setSettingsActive(false);
          return next;
        });
        queueReveal();
      } catch (err) {
        console.error("Unable to open external file", absolutePath, err);
      }
    },
    [],
  );

  // Dispatch a raw path picked from the terminal (cmd+click). Resolves
  // the path on the backend and routes to the right editor / file-tree
  // / Finder helper depending on whether it is a file, a directory, in
  // or out of the active workspace.
  const openTerminalPath = useCallback(
    async (rawPath: string) => {
      const trimmed = rawPath.trim();
      if (!trimmed) return;
      try {
        const resolution = await api.resolveTerminalPath(workspacePath, trimmed);
        if (resolution.kind === "missing") return;

        const buildReveal = () => {
          if (resolution.line == null) return undefined;
          const lineNumber = Math.max(1, resolution.line);
          const columnStart = Math.max(1, resolution.column ?? 1);
          return {
            lineNumber,
            columnStart,
            columnEnd: columnStart + 1,
          };
        };

        if (resolution.kind === "directory") {
          if (!resolution.isOutsideWorkspace && resolution.relativePath != null) {
            void api.revealEntry(workspacePath, resolution.relativePath);
          } else {
            void api.revealAbsolutePath(resolution.absolutePath);
          }
          return;
        }

        // kind === "file"
        if (!resolution.isOutsideWorkspace && resolution.relativePath != null) {
          const reveal = buildReveal();
          await openFile(
            {
              name: basename(resolution.relativePath),
              relativePath: resolution.relativePath,
              absolutePath: resolution.absolutePath,
              kind: "file",
              hasChildren: false,
            },
            reveal ? { ...reveal, query: "" } : undefined,
          );
        } else {
          await openExternalFile(resolution.absolutePath, buildReveal());
        }
      } catch (err) {
        console.error("Unable to resolve terminal path", rawPath, err);
      }
    },
    [openExternalFile, openFile, workspacePath],
  );

  const closeTab = useCallback((index: number) => {
    const tabCount = tabsRef.current.length;
    if (index < 0 || index >= tabCount) return;

    setTabs((prev) => {
      if (index < 0 || index >= prev.length) return prev;
      const next = prev.slice();
      next.splice(index, 1);
      return next;
    });
    setActiveTabIndex((active) => {
      const nextLength = tabCount - 1;
      if (nextLength <= 0) return -1;
      if (active === index) return Math.min(index, nextLength - 1);
      if (active > index) return active - 1;
      return Math.min(active, nextLength - 1);
    });
  }, []);

  const handleTreeEntryRenamed = useCallback(
    (oldRelativePath: string, entry: WorkspaceEntry) => {
      setTabs((prev) =>
        prev.map((tab) => {
          if (tab.external) return tab;
          const nextPath = replaceTreePath(
            tab.relativePath,
            oldRelativePath,
            entry,
          );
          if (!nextPath) return tab;
          return retargetTab(tab, nextPath, workspacePath, entry);
        }),
      );
    },
    [workspacePath],
  );

  const handleTreeEntryDeleted = useCallback((entry: WorkspaceEntry) => {
    setTabs((prev) =>
      prev.filter(
        (tab) =>
          tab.external || tab.dirty || !entryContainsPath(entry, tab.relativePath),
      ),
    );
  }, []);

  const handleTreeEntriesMoved = useCallback(
    (moves: { from: WorkspaceEntry; to: WorkspaceEntry }[]) => {
      setTabs((prev) =>
        prev.map((tab) => {
          if (tab.external) return tab;
          for (const move of moves) {
            const nextPath = replaceTreePath(
              tab.relativePath,
              move.from.relativePath,
              move.to,
            );
            if (nextPath) {
              return retargetTab(tab, nextPath, workspacePath, move.to);
            }
          }
          return tab;
        }),
      );
    },
    [workspacePath],
  );

  useEffect(() => {
    if (activeTabIndex >= tabs.length) {
      setActiveTabIndex(tabs.length - 1);
    }
  }, [tabs.length, activeTabIndex]);

  const updateBuffer = useCallback((index: number, value: string) => {
    setTabs((prev) => {
      const next = prev.slice();
      const tab = next[index];
      if (!tab) return prev;
      // External (read-only) tabs are never dirty and should never have
      // their buffer mutated by Monaco onChange events.
      if (tab.external) return prev;
      next[index] = {
        ...tab,
        buffer: value,
        dirty: value !== (tab.doc.content ?? ""),
      };
      return next;
    });
  }, []);

  const saveTab = useCallback(
    async (index: number) => {
      const tab = tabs[index];
      if (!tab || !tab.dirty || tab.external) return;
      try {
        const updated = await api.writeFile(
          workspacePath,
          tab.relativePath,
          tab.buffer,
        );
        setTabs((prev) => {
          const next = prev.slice();
          if (!next[index]) return prev;
          next[index] = {
            ...next[index],
            doc: updated,
            buffer: updated.content ?? next[index].buffer,
            dirty: false,
          };
          return next;
        });
      } catch (err) {
        console.error(err);
      }
    },
    [workspacePath, tabs],
  );

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
        event.preventDefault();
        if (settingsActive) return;
        if (activeTabIndex >= 0) void saveTab(activeTabIndex);
        return;
      }
      if (
        (event.metaKey || event.ctrlKey) &&
        event.shiftKey &&
        event.key.toLowerCase() === "f"
      ) {
        event.preventDefault();
        setFileSearchOpen(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [activeTabIndex, saveTab, settingsActive]);

  // ---------------- Event subscriptions ----------------

  const agentSubsRef = useRef<
    Set<(conversationId: string, event: AgentEvent, sequence?: number) => void>
  >(new Set());

  useEffect(() => {
    let cancelled = false;
    let unlisten: UnlistenFn | null = null;
    (async () => {
      const u = await listen<ConversationEventPayload>(
        "agent-event",
        (event) => {
          const payload = event.payload;
          if (
            payload.workspaceId &&
            payload.workspaceId !== workspacePathRef.current
          ) {
            return;
          }
          if (typeof payload.sequence === "number") {
            const last =
              lastAgentEventSequenceByConversationRef.current.get(
                payload.conversationId,
              ) ?? 0;
            // The backend's per-turn sequence resets to 1 when a new turn
            // registers. A strictly *lower* sequence than what we cached
            // therefore means a brand-new turn started for this
            // conversation — accept the event and forget the stale "last"
            // from the previous turn. Equal sequences are real duplicates
            // (typically a replay/live overlap) and stay filtered.
            if (payload.sequence === last && last !== 0) return;
            lastAgentEventSequenceByConversationRef.current.set(
              payload.conversationId,
              payload.sequence,
            );
          }
          for (const handler of agentSubsRef.current) {
            handler(payload.conversationId, payload.event, payload.sequence);
          }
        },
      );
      if (cancelled) {
        u();
      } else {
        unlisten = u;
      }
    })();
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  const subscribeEvents = useCallback(
    (
      handler: (
        conversationId: string,
        event: AgentEvent,
        sequence?: number,
      ) => void,
    ) => {
      agentSubsRef.current.add(handler);
      return () => {
        agentSubsRef.current.delete(handler);
      };
    },
    [],
  );

  const replayActiveTurnEvents = useCallback(
    async (conversationId: string, afterSequence = 0) => {
      const workspaceAtRequest = workspacePathRef.current;
      let replay = await api.replayActiveTurnEvents(
        workspaceAtRequest,
        conversationId,
        afterSequence,
      );
      if (workspacePathRef.current !== workspaceAtRequest) return;
      if (!replay.active) {
        markConversationStreaming(conversationId, false);
        return;
      }
      // The backend resets per-turn sequences to 1 on each register_active_turn.
      // If the server's latest sequence is *below* our cached afterSequence, the
      // turn we knew about has ended and a fresh one started — our cached
      // "last" would otherwise filter out every event of the new turn. Drop it
      // and refetch the active turn's buffer from scratch.
      if (replay.latestSequence < afterSequence) {
        lastAgentEventSequenceByConversationRef.current.delete(conversationId);
        replay = await api.replayActiveTurnEvents(
          workspaceAtRequest,
          conversationId,
          0,
        );
        if (workspacePathRef.current !== workspaceAtRequest) return;
        if (!replay.active) {
          markConversationStreaming(conversationId, false);
          return;
        }
      }
      markConversationStreaming(conversationId, true);
      const sortedEvents = [...replay.events].sort(
        (a, b) => a.sequence - b.sequence,
      );
      for (const entry of sortedEvents) {
        const last =
          lastAgentEventSequenceByConversationRef.current.get(conversationId) ?? 0;
        // Same logic as the live `agent-event` handler: only filter true
        // duplicates. A strictly *lower* sequence is impossible inside a
        // single replay batch (events are sorted), but be defensive in case
        // the backend ever interleaves turns.
        if (entry.sequence === last && last !== 0) continue;
        lastAgentEventSequenceByConversationRef.current.set(
          conversationId,
          entry.sequence,
        );
        for (const handler of agentSubsRef.current) {
          handler(conversationId, entry.event, entry.sequence);
        }
      }
    },
    [markConversationStreaming],
  );

  useEffect(() => {
    replayActiveTurnEventsRef.current = replayActiveTurnEvents;
  }, [replayActiveTurnEvents]);

  const syncActiveTurns = useCallback(
    (activeTurns: ActiveTurnSummary[]) => {
      const workspaceTurns = activeTurns.filter(
        (turn) => turn.workspaceId === workspacePathRef.current,
      );
      const activeIds = new Set(workspaceTurns.map((turn) => turn.conversationId));
      setStreamingConversationIds((prev) => {
        let changed = false;
        for (const id of prev) {
          if (!activeIds.has(id)) {
            changed = true;
            break;
          }
        }
        if (!changed) {
          for (const id of activeIds) {
            if (!prev.has(id)) {
              changed = true;
              break;
            }
          }
        }
        return changed ? activeIds : prev;
      });
      setStreamingModelsByConversation((prev) => {
        let changed = false;
        const next = new Map(prev);
        for (const id of Array.from(next.keys())) {
          if (!activeIds.has(id)) {
            next.delete(id);
            changed = true;
          }
        }
        return changed ? next : prev;
      });
      for (const turn of workspaceTurns) {
        let last =
          lastAgentEventSequenceByConversationRef.current.get(turn.conversationId) ?? 0;
        // A freshly-registered turn always starts at latestSequence = 0
        // (next_sequence = 1 on the backend). If our cached "last" from a
        // prior turn is higher, the per-turn counter has rolled over and
        // the cached value would otherwise filter out every live event of
        // the new turn. Drop it so the upcoming sequence=1 event is
        // accepted.
        if (turn.latestSequence < last) {
          lastAgentEventSequenceByConversationRef.current.delete(
            turn.conversationId,
          );
          last = 0;
        }
        if (turn.latestSequence > last) {
          void replayActiveTurnEvents(turn.conversationId, last).catch((err) => {
            console.error(err);
          });
        }
      }
    },
    [replayActiveTurnEvents],
  );

  useEffect(() => {
    const handler = async (conversationId: string, event: AgentEvent) => {
      const fileChanges = fileChangesFromAgentEvent(event);
      if (fileChanges.length > 0) {
        refreshChangedFiles(fileChanges);
      }

      if (event.type === "turn_started") {
        markConversationStreaming(conversationId, true);
        return;
      }
      if (event.type !== "turn_finished") {
        return;
      }
      markConversationStreaming(conversationId, false);
      const workspaceAtRequest = workspacePathRef.current;
      const shouldLoadActive = conversationId === activeConvIdRef.current;
      try {
        const summariesPromise = api.listConversations(workspaceAtRequest);
        const loadedPromise =
          shouldLoadActive
            ? api.loadConversation(workspaceAtRequest, conversationId)
            : Promise.resolve(null);
        const [loaded, summaries] = await Promise.all([
          loadedPromise,
          summariesPromise,
        ]);
        startTransition(() => {
          if (workspacePathRef.current !== workspaceAtRequest) return;
          if (
            loaded &&
            loaded.id === conversationId &&
            loaded.workspaceId === workspaceAtRequest &&
            activeConvIdRef.current === conversationId
          ) {
            setActiveConv(loaded);
          }
          setConversations(summaries);
        });
      } catch (err) {
        console.error(err);
      }
    };
    agentSubsRef.current.add(handler);
    return () => {
      agentSubsRef.current.delete(handler);
    };
  }, [markConversationStreaming, refreshChangedFiles]);

  useEffect(() => {
    let cancelled = false;
    void api
      .listActiveTurns()
      .then((activeTurns) => {
        if (!cancelled) syncActiveTurns(activeTurns);
      })
      .catch((err) => {
        if (!cancelled) console.error(err);
      });
    return () => {
      cancelled = true;
    };
  }, [syncActiveTurns, workspacePath]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: UnlistenFn | null = null;
    (async () => {
      const u = await listen<ActiveTurnsChangedPayload>(
        "active-turns-changed",
        (event) => {
          syncActiveTurns(event.payload.activeTurns);
        },
      );
      if (cancelled) {
        u();
      } else {
        unlisten = u;
      }
    })();
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [syncActiveTurns]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: UnlistenFn | null = null;
    (async () => {
      const u = await listen<WorkspaceFileChangedPayload>(
        "workspace-file-changed",
        async (event) => {
          const payload = event.payload;
          if (payload.workspacePath !== workspacePath) return;
          refreshFileTreeSoon();
          if (!payload.relativePath) return;
          void refreshOpenTabFromDisk(payload.relativePath);
        },
      );
      if (cancelled) {
        u();
      } else {
        unlisten = u;
      }
    })();
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [refreshFileTreeSoon, refreshOpenTabFromDisk, workspacePath]);

  const externalDropFeed = useMemo<ExternalDropFeed>(
    () => ({
      subscribe(handler) {
        dropSubsRef.current.add(handler);
        return () => {
          dropSubsRef.current.delete(handler);
        };
      },
      subscribeDrag(handler) {
        dragSubsRef.current.add(handler);
        return () => {
          dragSubsRef.current.delete(handler);
        };
      },
    }),
    [],
  );
  const dropSubsRef = useRef<
    Set<
      (attachments: { path: string; name: string; origin: "finder" }[]) => void
    >
  >(new Set());
  const dragSubsRef = useRef<Set<(active: boolean) => void>>(new Set());
  const chatDropZoneRef = useRef<HTMLDivElement | null>(null);
  const fileTreeDropZoneRef = useRef<HTMLDivElement | null>(null);
  const [fileTreeDropState, setFileTreeDropState] = useState<{
    active: boolean;
    targetRelative: string | null;
  }>({ active: false, targetRelative: null });
  const [importError, setImportError] = useState<string | null>(null);

  const findFolderTargetAt = useCallback(
    (x: number, y: number): string | null => {
      const el = document.elementFromPoint(x, y);
      if (!el) return null;
      const row = (el as Element).closest?.(
        ".tree-row[data-kind='directory']",
      ) as HTMLElement | null;
      if (!row) return null;
      return row.dataset.dropPath ?? null;
    },
    [],
  );

  useEffect(() => {
    let cancelled = false;
    let unlisten: UnlistenFn | null = null;
    (async () => {
      try {
        const u = await getCurrentWebview().onDragDropEvent(async (event) => {
          const type = event.payload.type;
          const position =
            "position" in event.payload ? event.payload.position : null;
          const chatRect = chatDropZoneRef.current?.getBoundingClientRect();
          const sidebarRect =
            fileTreeDropZoneRef.current?.getBoundingClientRect();
          const overChat = (() => {
            if (!chatRect || !position) return false;
            return (
              position.x >= chatRect.left &&
              position.x <= chatRect.right &&
              position.y >= chatRect.top &&
              position.y <= chatRect.bottom
            );
          })();
          const overSidebar = (() => {
            if (overChat) return false;
            if (!sidebarRect || !position) return false;
            return (
              position.x >= sidebarRect.left &&
              position.x <= sidebarRect.right &&
              position.y >= sidebarRect.top &&
              position.y <= sidebarRect.bottom
            );
          })();

          if (type === "enter" || type === "over") {
            for (const handler of dragSubsRef.current) handler(overChat);
            if (overSidebar && position) {
              const target = findFolderTargetAt(position.x, position.y);
              setFileTreeDropState({ active: true, targetRelative: target });
            } else {
              setFileTreeDropState((prev) =>
                prev.active ? { active: false, targetRelative: null } : prev,
              );
            }
            return;
          }
          if (type === "leave") {
            for (const handler of dragSubsRef.current) handler(false);
            setFileTreeDropState({ active: false, targetRelative: null });
            return;
          }
          if (type === "drop") {
            for (const handler of dragSubsRef.current) handler(false);
            setFileTreeDropState({ active: false, targetRelative: null });
            const paths = event.payload.paths ?? [];
            if (!paths.length) return;
            if (overChat) {
              const attachments = paths.map((path) => ({
                path,
                name: basename(path),
                origin: "finder" as const,
              }));
              for (const handler of dropSubsRef.current) handler(attachments);
              return;
            }
            if (overSidebar && position) {
              const target = findFolderTargetAt(position.x, position.y);
              try {
                setImportError(null);
                await api.importPaths(workspacePath, paths, target ?? undefined);
                refreshFileTree();
              } catch (err) {
                console.error(err);
                setImportError(String(err));
              }
            }
          }
        });
        if (cancelled) {
          u();
        } else {
          unlisten = u;
        }
      } catch (err) {
        console.warn("webview drag-drop unavailable", err);
      }
    })();
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [workspacePath, findFolderTargetAt, refreshFileTree]);

  const sendMessage = useCallback(
    async (
      text: string,
      attachments: { path: string; name?: string }[],
      model: SavedConversation["model"],
      thinking: ThinkingLevel,
      mode: AgentMode,
      serviceTier?: ServiceTier | null,
      rewriteFromHistoryIndex?: number,
      planControl?: PlanControl,
      messageVisibility?: MessageVisibility,
      revertWorkspaceChanges?: boolean,
    ) => {
      const conversationId = activeConv.id;
      const workspaceAtRequest = workspacePath;
      const optimisticTitle = titleFromOutgoingUserText(text);
      const shouldUpdateTitleFromUserMessage =
        messageVisibility !== "systemReminder" &&
        Boolean(optimisticTitle) &&
        (rewriteFromHistoryIndex === 0 ||
          (rewriteFromHistoryIndex === undefined && activeConv.history.length === 0));

      if (shouldUpdateTitleFromUserMessage && optimisticTitle) {
        applyOptimisticConversationTitle(conversationId, optimisticTitle);
      }

      markConversationStreamingModel(conversationId, model, thinking);
      markConversationStreaming(conversationId, true);
      try {
        await sendMessageWithBusyRetry(
          workspaceAtRequest,
          conversationId,
          text,
          attachments,
          model,
          thinking,
          mode,
          serviceTier,
          rewriteFromHistoryIndex,
          planControl,
          messageVisibility,
          revertWorkspaceChanges,
        );
      } catch (err) {
        markConversationStreaming(conversationId, false);
        void refreshConversationAfterMessageStart(
          workspaceAtRequest,
          conversationId,
        ).catch((refreshErr) => console.error(refreshErr));
        throw err;
      }

      void refreshConversationAfterMessageStart(
        workspaceAtRequest,
        conversationId,
      ).catch((err) => console.error(err));
    },
    [
      workspacePath,
      activeConv.id,
      activeConv.history.length,
      applyOptimisticConversationTitle,
      markConversationStreaming,
      markConversationStreamingModel,
      refreshConversationAfterMessageStart,
    ],
  );

  const compactConversation = useCallback(
    async (
      model: SavedConversation["model"],
      thinking: ThinkingLevel,
      serviceTier?: ServiceTier | null,
      options?: { continueAfter?: boolean; instruction?: string },
    ) => {
      const conversationId = activeConv.id;
      const continueAfter = options?.continueAfter ?? true;
      const continuationMode = conversationContinuationMode(activeConv);
      const continuationPrompt =
        continueAfter && continuationMode === "goal"
          ? GOAL_COMPACTION_CONTINUATION_PROMPT
          : COMPACTION_CONTINUATION_PROMPT;
      markConversationStreamingModel(conversationId, model, thinking);
      markConversationStreaming(conversationId, true);
      try {
        await api.compactConversation(
          workspacePath,
          conversationId,
          model,
          thinking,
          serviceTier,
          options?.instruction,
        );

        markConversationStreaming(conversationId, false);

        const [loaded, summaries] = await Promise.all([
          api.loadConversation(workspacePath, conversationId),
          api.listConversations(workspacePath),
        ]);
        if (workspacePathRef.current !== workspacePath) return;

        setConversations(summaries);
        if (activeConvIdRef.current === conversationId) {
          setActiveConv(loaded);
        }

        if (!continueAfter) return;

        await sleep(0);

        markConversationStreamingModel(conversationId, model, thinking);
        markConversationStreaming(conversationId, true);
        await sendMessageWithBusyRetry(
          workspacePath,
          conversationId,
          continuationPrompt,
          [],
          model,
          thinking,
          continuationMode,
          serviceTier,
          undefined,
          undefined,
          "systemReminder",
        );

        const reloaded = await api.loadConversation(workspacePath, conversationId);
        if (
          workspacePathRef.current === workspacePath &&
          activeConvIdRef.current === conversationId
        ) {
          setActiveConv((current) =>
            current.id === conversationId ? reloaded : current,
          );
        }
      } catch (err) {
        markConversationStreaming(conversationId, false);
        throw err;
      }
    },
    [activeConv, markConversationStreaming, markConversationStreamingModel, workspacePath],
  );

  const changeConversationMode = useCallback(
    async (mode: AgentMode) => {
      const conversationId = activeConv.id;
      const updated = await api.setConversationMode(
        workspacePath,
        conversationId,
        mode,
      );
      const summaries = await api.listConversations(workspacePath);
      startTransition(() => {
        setActiveConv((current) =>
          current.id === conversationId ? updated : current,
        );
        setConversations(summaries);
      });
    },
    [activeConv.id, workspacePath],
  );

  const changeConversationModelPreference = useCallback(
    async (
      mode: AgentMode,
      model: SavedConversation["model"],
      thinking: ThinkingLevel,
    ) => {
      const conversationId = activeConv.id;
      const updated = await api.setConversationModelPreference(
        workspacePath,
        conversationId,
        mode,
        model,
        thinking,
      );
      const selected = modelRefWithThinking(model, thinking);
      setGlobalModeModelSettings((current) => ({
        ...current,
        [mode]: selected,
      }));
      startTransition(() => {
        setActiveConv((current) =>
          current.id === conversationId
            ? {
                ...current,
                model: selected,
                modeModelSettings: updated,
              }
            : current,
        );
      });
    },
    [activeConv.id, workspacePath],
  );

  const implementPlanFresh = useCallback(
    async (
      plan: PlanArtifact,
      prompt = "Implement completely this plan. Use the attached markdown plan as the source of truth.",
    ) => {
      const next = await api.createConversation(workspacePath);
      const conversationId = next.activeConversation.id;
      // The new conversation is seeded with the workspace's global default,
      // which represents the most recent model the user picked anywhere. Per
      // the plan, every brand-new conversation must use that seed (not the
      // preference of whatever conversation the user was sitting in when
      // they triggered the action).
      const seedModel = next.activeConversation.modeModelSettings.act;
      const seedThinking = thinkingFromRef(seedModel);
      const title = titleFromPlanImplementation(plan);
      const titledActiveConversation = {
        ...next.activeConversation,
        title,
      };
      const titledConversations = await api.renameConversation(
        workspacePath,
        conversationId,
        title,
      );
      activeConvIdRef.current = conversationId;
      setConversations(titledConversations);
      setActiveConv(titledActiveConversation);
      setGlobalModeModelSettings(next.modeModelSettings);
      onBootstrapReplace({
        ...next,
        conversations: titledConversations,
        activeConversation: titledActiveConversation,
      });
      markConversationStreamingModel(conversationId, seedModel, seedThinking);
      markConversationStreaming(conversationId, true);
      try {
        await sendMessageWithBusyRetry(
          workspacePath,
          conversationId,
          prompt,
          [
            {
              path: plan.absolutePath ?? plan.path,
              name: basename(plan.path),
            },
          ],
          seedModel,
          seedThinking,
          "act",
          null,
          undefined,
          "implementPlan",
          "systemReminder",
        );
        const loaded = await api.loadConversation(workspacePath, conversationId);
        startTransition(() => {
          setActiveConv((current) =>
            current.id === conversationId ? loaded : current,
          );
        });
      } catch (err) {
        markConversationStreaming(conversationId, false);
        throw err;
      }
    },
    [workspacePath, markConversationStreaming, markConversationStreamingModel, onBootstrapReplace],
  );

  const stopTurn = useCallback(async () => {
    try {
      await api.cancelTurn(workspacePath, activeConv.id);
    } catch (err) {
      console.error(err);
    }
  }, [workspacePath, activeConv.id]);

  // Switch this window to another workspace path. Used by the Git
  // panel when the user clicks a worktree row or creates a new one.
  // We refuse the switch while any conversation in the current window
  // is still streaming so the in-flight turn isn't orphaned mid-tool.
  // Throwing here lets the caller surface a contextual error notice
  // instead of swallowing the failure silently.
  const switchWorkspace = useCallback(
    async (targetPath: string): Promise<void> => {
      if (streamingConversationIds.size > 0) {
        throw new Error(
          "A conversation is still streaming. Stop active turns before switching workspace.",
        );
      }
      const nextBootstrap = await api.openWorkspace(targetPath);
      recordRecent(
        nextBootstrap.workspace.path,
        nextBootstrap.workspace.name,
      );
      // Drop editor / settings / search state that belongs to the
      // outgoing workspace so the incoming one boots cleanly. The
      // bootstrap-replace effect refreshes conversations, file tree,
      // etc. on its own.
      setTabs([]);
      setActiveTabIndex(-1);
      setSettingsOpen(false);
      setSettingsActive(false);
      setFileSearchOpen(false);
      setPendingRootCreate(null);
      setEditorRevealTarget(null);
      setImportError(null);
      onBootstrapReplace(nextBootstrap);
      refreshFileTree();
    },
    [
      onBootstrapReplace,
      refreshFileTree,
      streamingConversationIds,
    ],
  );

  // ---------------- Layout state ----------------
  const [leftWidth, setLeftWidth] = useState(INITIAL_LEFT);
  const [rightWidth, setRightWidth] = useState(INITIAL_RIGHT);
  const [topSplit, setTopSplit] = useState(INITIAL_SPLIT_TOP);
  const [terminalAvailable, setTerminalAvailable] = useState(false);
  const [terminalOpen, setTerminalOpen] = useState(false);
  const [terminalFullHeight, setTerminalFullHeight] = useState(false);
  const [terminalHeight, setTerminalHeight] = useState(INITIAL_TERMINAL_HEIGHT);

  const clampColumn = useCallback((v: number) => {
    if (typeof window === "undefined") return v;
    const max = window.innerWidth * MAX_COL_RATIO;
    return Math.max(MIN_COL, Math.min(max, v));
  }, []);

  const clampTerminal = useCallback((v: number) => {
    if (typeof window === "undefined") return v;
    const max = Math.max(MIN_TERMINAL_HEIGHT, window.innerHeight * MAX_TERMINAL_RATIO);
    return Math.max(MIN_TERMINAL_HEIGHT, Math.min(max, v));
  }, []);

  const showTerminal = useCallback(() => {
    setTerminalAvailable(true);
    setTerminalOpen(true);
    setTerminalHeight((value) => clampTerminal(value));
  }, [clampTerminal]);

  const hideTerminal = useCallback(() => {
    setTerminalOpen(false);
    setTerminalFullHeight(false);
  }, []);

  const closeTerminalPanel = useCallback(() => {
    setTerminalOpen(false);
    setTerminalFullHeight(false);
    setTerminalAvailable(false);
  }, []);

  const toggleTerminal = useCallback(() => {
    if (terminalOpen) {
      hideTerminal();
    } else {
      showTerminal();
    }
  }, [hideTerminal, showTerminal, terminalOpen]);

  const toggleTerminalFullHeight = useCallback(() => {
    setTerminalFullHeight((value) => !value);
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: UnlistenFn | null = null;

    void listen(TERMINAL_OPEN_EVENT, () => {
      showTerminal();
    }).then((nextUnlisten) => {
      if (disposed) {
        nextUnlisten();
      } else {
        unlisten = nextUnlisten;
      }
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [showTerminal]);

  const sidebarHeightRef = useRef<HTMLDivElement | null>(null);
  const applyTopDelta = useCallback((delta: number) => {
    const el = sidebarHeightRef.current;
    if (!el) return;
    const h = el.clientHeight;
    if (h <= 0) return;
    setTopSplit((prev) => {
      const nextPx = Math.max(80, Math.min(h - 80, prev * h + delta));
      return nextPx / h;
    });
  }, []);

  const onDragFile = useCallback(
    (entry: WorkspaceEntry, event: React.DragEvent) => {
      if (entry.kind !== "file") return;
      const payload = JSON.stringify({
        relativePath: entry.relativePath,
        absolutePath: entry.absolutePath,
        name: entry.name,
      });
      event.dataTransfer.setData("application/x-claakecode-file", payload);
      event.dataTransfer.setData("text/plain", entry.relativePath);
      event.dataTransfer.effectAllowed = "copy";
    },
    [],
  );

  const activeFilePath =
    !settingsActive && activeTabIndex >= 0 && tabs[activeTabIndex]
      ? tabs[activeTabIndex].relativePath
      : null;
  const terminalVisible = terminalAvailable && terminalOpen;
  const activeConversationIsStreaming = streamingConversationIds.has(
    activeConv.id,
  );
  const activeStreamingModel = activeConversationIsStreaming
    ? streamingModelsByConversation.get(activeConv.id) ?? activeConv.model
    : null;
  const chatModeModelSettings = activeConv.modeModelSettings;

  return (
    <div className="workspace">
      <div
        className="titlebar"
        data-tauri-drag-region
        data-platform={IS_WINDOWS ? "windows" : undefined}
      >
        <div
          className="titlebar__actions"
          data-tauri-drag-region
          style={{ left: leftWidth, right: rightWidth }}
        >
          <button
            className="titlebar__btn"
            data-on={terminalVisible ? "true" : "false"}
            onClick={toggleTerminal}
            title={terminalVisible ? "Hide terminal" : "Show terminal"}
          >
            <Icon
              icon={
                terminalVisible
                  ? "solar:command-bold-duotone"
                  : "solar:command-linear"
              }
              width={12}
              height={12}
            />
            Terminal
          </button>
          <button
            className="titlebar__btn"
            data-on={settingsActive ? "true" : "false"}
            onClick={() => openSettings()}
            title="Settings"
          >
            <Icon icon="solar:settings-linear" width={12} height={12} />
            Settings
          </button>
          <button
            className="titlebar__btn"
            onClick={onSwitchWorkspace}
            title="Switch workspace"
          >
            <Icon icon="solar:folder-with-files-linear" width={12} height={12} />
            Switch
          </button>
          <button
            className="titlebar__btn"
            onClick={toggleTheme}
            title={theme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
            aria-label="Toggle theme"
          >
            <Icon
              icon={theme === "dark" ? "solar:sun-linear" : "solar:moon-linear"}
              width={12}
              height={12}
            />
          </button>
        </div>
        <div className="titlebar__brand" data-tauri-drag-region>
          <span className="titlebar__brand-mark">
            <ClaakeCodeMark size={20} />
          </span>
          <span className="titlebar__brand-name">Claake Code</span>
        </div>
        <UpdateBadge />
        <WindowControls />
      </div>

      <div className="main">
        <div
          className="sidebar"
          style={{ width: leftWidth, flex: `0 0 ${leftWidth}px` }}
          ref={sidebarHeightRef}
        >
          <div
            className="sidebar__section"
            style={{ flex: `0 0 ${topSplit * 100}%` }}
            ref={fileTreeDropZoneRef}
            data-drop-active={fileTreeDropState.active ? "true" : "false"}
          >
            <div className="sidebar__head">
              <span className="sidebar__head-title">
                <Icon icon="solar:folder-bold-duotone" width={16} height={16} />
                <span>{bootstrap.workspace.name}</span>
              </span>
              <span className="sidebar__head-actions">
                <button
                  type="button"
                  className="sidebar__head-btn"
                  title="New file"
                  onClick={() => startRootCreate("file")}
                >
                  <Icon icon="solar:document-add-linear" width={15} height={15} />
                </button>
                <button
                  type="button"
                  className="sidebar__head-btn"
                  title="New folder"
                  onClick={() => startRootCreate("directory")}
                >
                  <Icon
                    icon="solar:add-folder-linear"
                    width={15}
                    height={15}
                  />
                </button>
                <button
                  type="button"
                  className="sidebar__head-btn"
                  data-active={fileSearchOpen ? "true" : "false"}
                  title={fileSearchOpen ? "Show files" : "Search files"}
                  onClick={() => setFileSearchOpen((value) => !value)}
                >
                  <Icon
                    icon={
                      fileSearchOpen
                        ? "solar:folder-open-linear"
                        : "solar:magnifer-linear"
                    }
                    width={15}
                    height={15}
                  />
                </button>
              </span>
            </div>
            {fileSearchOpen ? (
              <SearchPane
                workspacePath={workspacePath}
                onOpenFile={openFile}
                refreshToken={fileTreeRefreshToken}
              />
            ) : (
              <FileTree
                ref={fileTreeRef}
                workspacePath={workspacePath}
                activeFile={activeFilePath}
                onOpenFile={openFile}
                onDragFile={onDragFile}
                onEntryRenamed={handleTreeEntryRenamed}
                onEntryDeleted={handleTreeEntryDeleted}
                onEntriesMoved={handleTreeEntriesMoved}
                refreshToken={fileTreeRefreshToken}
                dropActive={fileTreeDropState.active}
                dropTargetRelative={fileTreeDropState.targetRelative}
              />
            )}
            {importError && (
              <div
                className="sidebar__import-error"
                onClick={() => setImportError(null)}
                title="click to dismiss"
              >
                {importError}
              </div>
            )}
          </div>
          <Splitter orientation="horizontal" onDelta={applyTopDelta} />
          <div
            className="sidebar__section sidebar__section--bottom"
            style={{ flex: "1 1 0" }}
          >
            <div className="sidebar__head sidebar__head--tabs">
              <div className="sidebar-tabs" role="tablist">
                <button
                  type="button"
                  role="tab"
                  className="sidebar-tab"
                  data-active={bottomTab === "conversations" ? "true" : "false"}
                  aria-selected={bottomTab === "conversations"}
                  onClick={() => setBottomTab("conversations")}
                >
                  <Icon
                    icon="solar:chat-round-dots-linear"
                    width={13}
                    height={13}
                  />
                  <span>Conversations</span>
                </button>
                <button
                  type="button"
                  role="tab"
                  className="sidebar-tab"
                  data-active={bottomTab === "git" ? "true" : "false"}
                  aria-selected={bottomTab === "git"}
                  onClick={() => setBottomTab("git")}
                >
                  <GitMark size={13} />
                  <span>Git</span>
                </button>
              </div>
              {bottomTab === "conversations" && (
                <div className="sidebar__head-actions">
                  <button
                    type="button"
                    className="sidebar__head-btn"
                    onClick={createConversation}
                    title="New conversation"
                  >
                    <Icon
                      icon="solar:add-square-linear"
                      width={15}
                      height={15}
                    />
                  </button>
                </div>
              )}
            </div>
            {/*
              Both panels stay mounted so swapping tabs preserves any
              transient state (a half-typed commit message, an open
              "New worktree" form, etc). We toggle visibility with
              display:none to avoid double-mounting their effects.
            */}
            <div
              className="sidebar-tab-pane"
              role="tabpanel"
              aria-hidden={bottomTab !== "conversations"}
              style={{
                display: bottomTab === "conversations" ? "flex" : "none",
              }}
            >
              <ConversationList
                conversations={conversations}
                activeId={activeConv.id}
                streamingIds={streamingConversationIds}
                onSelect={selectConversation}
                onRename={renameConversation}
                onDelete={deleteConversation}
              />
            </div>
            <div
              className="sidebar-tab-pane"
              role="tabpanel"
              aria-hidden={bottomTab !== "git"}
              style={{
                display: bottomTab === "git" ? "flex" : "none",
              }}
            >
              <GitPanel
                workspacePath={workspacePath}
                active={bottomTab === "git"}
                hasStreamingConversation={hasStreamingConversation}
                onSwitchWorkspace={switchWorkspace}
              />
            </div>
          </div>
        </div>
        <Splitter
          orientation="vertical"
          onDelta={(delta) => setLeftWidth((v) => clampColumn(v + delta))}
        />
        <div className="workbench-center">
          <div
            className="editor-shell"
            data-hidden={terminalVisible && terminalFullHeight ? "true" : "false"}
          >
            <EditorPane
              tabs={tabs}
              activeIndex={activeTabIndex}
              onActivate={activateFileTab}
              onClose={closeTab}
              onChange={updateBuffer}
              onSave={saveTab}
              onOpenFile={openChatFile}
              settingsOpen={settingsOpen}
              settingsActive={settingsActive}
              settingsView={<SettingsPane workspacePath={workspacePath} />}
              revealTarget={editorRevealTarget}
              onSettingsActivate={() => setSettingsActive(true)}
              onSettingsClose={closeSettings}
            />
          </div>
          {terminalVisible && !terminalFullHeight && (
            <Splitter
              orientation="horizontal"
              onDelta={(delta) =>
                setTerminalHeight((value) => clampTerminal(value - delta))
              }
            />
          )}
          <div
            className="terminal-shell"
            data-full-height={terminalFullHeight ? "true" : "false"}
            style={{
              display: terminalVisible ? "block" : "none",
              height: terminalVisible
                ? terminalFullHeight
                  ? "auto"
                  : terminalHeight
                : 0,
              flex: terminalVisible
                ? terminalFullHeight
                  ? "1 1 0"
                  : `0 0 ${terminalHeight}px`
                : "0 0 0",
            }}
          >
            {terminalAvailable && (
              <TerminalPanel
                key={workspacePath}
                active={terminalVisible}
                fullHeight={terminalFullHeight}
                workspacePath={workspacePath}
                onClose={hideTerminal}
                onCloseLastSession={closeTerminalPanel}
                onToggleFullHeight={toggleTerminalFullHeight}
                onOpenTerminalPath={openTerminalPath}
              />
            )}
          </div>
          {terminalAvailable && !terminalOpen && (
            <div className="terminal-restore">
              <button
                type="button"
                className="terminal-restore__button"
                onClick={showTerminal}
                title="Show terminal"
              >
                <Icon icon="solar:square-alt-arrow-up-linear" width={14} height={14} />
              </button>
            </div>
          )}
        </div>
        <Splitter
          orientation="vertical"
          onDelta={(delta) => setRightWidth((v) => clampColumn(v - delta))}
        />
        <div
          style={{
            width: rightWidth,
            flex: `0 0 ${rightWidth}px`,
            minWidth: 0,
            display: "flex",
          }}
        >
          <ChatPane
            workspacePath={workspacePath}
            conversationId={activeConv.id}
            activeModel={activeConv.model}
            modeModelSettings={chatModeModelSettings}
            streamingModel={activeStreamingModel}
            planWorkflow={activeConv.planWorkflow}
            goalWorkflow={activeConv.goalWorkflow}
            isStreaming={activeConversationIsStreaming}
            history={activeConv.history}
            subscribeEvents={subscribeEvents}
            onSend={sendMessage}
            onCompact={compactConversation}
            onModeChange={changeConversationMode}
            onModelPreferenceChange={changeConversationModelPreference}
            onImplementPlanFresh={implementPlanFresh}
            onStop={stopTurn}
            onOpenFile={openChatFile}
            onOpenSettings={openSettings}
            externalDrops={externalDropFeed}
            dropZoneRef={chatDropZoneRef}
          />
        </div>
      </div>
    </div>
  );
}

async function sendMessageWithBusyRetry(
  ...args: Parameters<typeof api.sendMessage>
): Promise<void> {
  for (let attempt = 0; ; attempt += 1) {
    try {
      await api.sendMessage(...args);
      return;
    } catch (err) {
      const delayMs = SEND_BUSY_RETRY_DELAYS_MS[attempt];
      if (!isConversationBusyError(err) || delayMs === undefined) {
        throw err;
      }
      await sleep(delayMs);
    }
  }
}

function isConversationBusyError(err: unknown): boolean {
  return String(err).includes("a turn is already running for this conversation");
}

function conversationContinuationMode(conversation: SavedConversation): AgentMode {
  if (conversation.planWorkflow.status !== "idle") return "plan";
  if (conversation.goalWorkflow.status === "active") return "goal";
  return "act";
}

function titleFromOutgoingUserText(text: string): string | null {
  const title = text.trim();
  if (!title) return null;
  const chars = Array.from(title);
  if (chars.length <= 48) return title;
  return `${chars.slice(0, 45).join("")}...`;
}

function titleFromPlanImplementation(plan: PlanArtifact): string {
  const planTitle = plan.title?.trim();
  const fileTitle = basename(plan.path).replace(/\.md$/i, "").trim();
  const base = planTitle || fileTitle || "plan";
  return titleFromOutgoingUserText(`Implement: ${base}`) ?? "Implement plan";
}

function sortConversationSummaries(
  conversations: ConversationSummary[],
): ConversationSummary[] {
  return [...conversations].sort((a, b) => b.updatedAtMs - a.updatedAtMs);
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function basename(path: string): string {
  const idx = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return idx >= 0 ? path.slice(idx + 1) : path;
}

function fileChangesFromAgentEvent(event: AgentEvent): FileChange[] {
  if (event.type === "tool_finished") return event.file_changes;
  if (event.type === "sub_agent_event") {
    return fileChangesFromAgentEvent(event.event);
  }
  return [];
}

function entryContainsPath(entry: WorkspaceEntry, relativePath: string): boolean {
  return (
    relativePath === entry.relativePath ||
    (entry.kind === "directory" &&
      relativePath.startsWith(`${entry.relativePath}/`))
  );
}

function replaceTreePath(
  relativePath: string,
  oldRelativePath: string,
  entry: WorkspaceEntry,
): string | null {
  if (relativePath === oldRelativePath) return entry.relativePath;
  if (relativePath.startsWith(`${oldRelativePath}/`)) {
    return `${entry.relativePath}${relativePath.slice(oldRelativePath.length)}`;
  }
  return null;
}

function retargetTab(
  tab: EditorTab,
  relativePath: string,
  workspacePath: string,
  entry: WorkspaceEntry,
): EditorTab {
  const exactEntry = relativePath === entry.relativePath && entry.kind === "file";
  const absolutePath = exactEntry
    ? entry.absolutePath
    : `${workspacePath}/${relativePath}`;
  const name = basename(relativePath);
  return {
    ...tab,
    relativePath,
    doc: {
      ...tab.doc,
      name,
      relativePath,
      absolutePath,
    },
  };
}

function chatPathToRelative(rawPath: string, workspacePath: string): string | null {
  let path = rawPath
    .trim()
    .replace(/^['"`<]+|['"`>,.;:]+$/g, "")
    .replace(/#L\d+(?:C\d+)?$/i, "")
    .replace(/:\d+(?::\d+)?$/, "")
    .replace(/\\/g, "/");

  if (!path || path.includes("://")) return null;

  const root = workspacePath.replace(/\\/g, "/").replace(/\/+$/, "");
  if (path === root) return null;
  if (path.startsWith(`${root}/`)) {
    path = path.slice(root.length + 1);
  } else if (path.startsWith("/")) {
    return null;
  }

  path = path.replace(/^\.\//, "");
  if (!path || path.startsWith("../") || path.includes("/../")) return null;

  return path;
}
