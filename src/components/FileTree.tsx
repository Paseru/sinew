import {
  memo,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  forwardRef,
} from "react";
import { Icon } from "@iconify/react";
import { api } from "../lib/ipc";
import { fileIcon, folderIcon } from "../lib/fileIcon";
import type { WorkspaceDeletedEntry, WorkspaceEntry } from "../types";

type NodeState = {
  entry: WorkspaceEntry;
  expanded: boolean;
  loading: boolean;
  error?: string;
  children?: WorkspaceEntry[];
};

type ClipboardState = {
  mode: "copy" | "cut";
  entries: WorkspaceEntry[];
};

type DeletedEntryBatch = WorkspaceDeletedEntry[];

type EditState =
  | {
      mode: "create";
      kind: WorkspaceEntry["kind"];
      parentRelativePath: string | null;
      draft: string;
    }
  | {
      mode: "rename";
      entry: WorkspaceEntry;
      draft: string;
    };

type ContextMenuState = {
  x: number;
  y: number;
  entry: WorkspaceEntry | null;
};

type InternalDragPayload = {
  entries?: WorkspaceEntry[];
};

type PointerDragState = {
  pointerId: number;
  startX: number;
  startY: number;
  entry: WorkspaceEntry;
  entries: WorkspaceEntry[];
  active: boolean;
};

type DropPointTarget = {
  targetRelativePath: string | null;
  hoverRelativePath: string | null;
};

type DragPreviewState = {
  x: number;
  y: number;
  entries: WorkspaceEntry[];
};

const INTERNAL_DRAG_MIME = "application/x-claakecode-files";
const POINTER_DRAG_THRESHOLD_PX = 4;

type Props = {
  workspacePath: string;
  activeFile: string | null;
  onOpenFile: (entry: WorkspaceEntry) => void;
  onDragFile: (entry: WorkspaceEntry, event: React.DragEvent) => void;
  onEntryCreated?: (entry: WorkspaceEntry) => void;
  onEntryRenamed?: (oldRelativePath: string, entry: WorkspaceEntry) => void;
  onEntryDeleted?: (entry: WorkspaceEntry) => void;
  onEntriesMoved?: (
    moves: { from: WorkspaceEntry; to: WorkspaceEntry }[],
  ) => void;
  refreshToken?: number;
  dropActive?: boolean;
  dropTargetRelative?: string | null;
};

export type FileTreeHandle = {
  startCreateRoot: (kind: WorkspaceEntry["kind"]) => void;
};

export const FileTree = forwardRef<FileTreeHandle, Props>(function FileTree(
  {
    workspacePath,
    activeFile,
    onOpenFile,
    onDragFile,
    onEntryCreated,
    onEntryRenamed,
    onEntryDeleted,
    onEntriesMoved,
    refreshToken,
    dropActive,
    dropTargetRelative,
  },
  ref,
) {
  const [roots, setRoots] = useState<WorkspaceEntry[]>([]);
  const [expanded, setExpanded] = useState<Record<string, NodeState>>({});
  const [rootError, setRootError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [selectedEntry, setSelectedEntry] = useState<WorkspaceEntry | null>(null);
  const [selectedEntries, setSelectedEntries] = useState<
    Record<string, WorkspaceEntry>
  >({});
  const [menu, setMenu] = useState<ContextMenuState | null>(null);
  const [editState, setEditState] = useState<EditState | null>(null);
  const [clipboard, setClipboard] = useState<ClipboardState | null>(null);
  const [localRefreshToken, setLocalRefreshToken] = useState(0);
  const [internalDragActive, setInternalDragActive] = useState(false);
  const [internalDropTargetRelative, setInternalDropTargetRelative] =
    useState<string | null>(null);
  const [internalDropHoverRelative, setInternalDropHoverRelative] =
    useState<string | null>(null);
  const [dragPreview, setDragPreview] = useState<DragPreviewState | null>(null);
  const expandedRef = useRef(expanded);
  const workspacePathRef = useRef(workspacePath);
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const submittingEditRef = useRef(false);
  const draggedEntriesRef = useRef<WorkspaceEntry[] | null>(null);
  const hoverExpandTimerRef = useRef<number | null>(null);
  const hoverExpandPathRef = useRef<string | null>(null);
  const pointerDragRef = useRef<PointerDragState | null>(null);
  const suppressNextClickRef = useRef(false);
  const deletedEntryBatchesRef = useRef<DeletedEntryBatch[]>([]);
  const restoringDeletedEntriesRef = useRef(false);

  useEffect(() => {
    expandedRef.current = expanded;
  }, [expanded]);

  useEffect(() => {
    const workspaceChanged = workspacePathRef.current !== workspacePath;
    workspacePathRef.current = workspacePath;
    const expandedSnapshot = workspaceChanged ? {} : expandedRef.current;
    const expandedNodes = Object.values(expandedSnapshot).filter(
      (state) => state.expanded || state.children,
    );

    if (workspaceChanged) {
      expandedRef.current = {};
      setExpanded({});
      setRoots([]);
      setSelectedEntry(null);
      setSelectedEntries({});
      setMenu(null);
      setEditState(null);
      setClipboard(null);
      setInternalDragActive(false);
      setInternalDropTargetRelative(null);
      setInternalDropHoverRelative(null);
      setDragPreview(null);
      draggedEntriesRef.current = null;
      pointerDragRef.current = null;
      suppressNextClickRef.current = false;
      deletedEntryBatchesRef.current = [];
      restoringDeletedEntriesRef.current = false;
      if (hoverExpandTimerRef.current !== null) {
        window.clearTimeout(hoverExpandTimerRef.current);
        hoverExpandTimerRef.current = null;
      }
      hoverExpandPathRef.current = null;
    }

    let cancelled = false;
    setRootError(null);
    (async () => {
      try {
        const [rootEntries, childResults] = await Promise.all([
          api.listEntries(workspacePath),
          Promise.all(
            expandedNodes.map(async (state) => {
              try {
                return {
                  state,
                  children: await api.listEntries(
                    workspacePath,
                    state.entry.relativePath,
                  ),
                  error: undefined,
                };
              } catch (err) {
                return {
                  state,
                  children: undefined,
                  error: String(err),
                };
              }
            }),
          ),
        ]);
        if (cancelled) return;
        setRoots(rootEntries);
        if (childResults.length) {
          setExpanded((prev) => {
            const next = { ...prev };
            for (const result of childResults) {
              const current =
                next[result.state.entry.relativePath] ?? result.state;
              next[result.state.entry.relativePath] = {
                ...current,
                loading: false,
                children: result.children,
                error: result.error,
              };
            }
            return next;
          });
        }
      } catch (err) {
        if (cancelled) return;
        setRootError(String(err));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [workspacePath, refreshToken, localRefreshToken]);

  useEffect(() => {
    return () => {
      if (hoverExpandTimerRef.current !== null) {
        window.clearTimeout(hoverExpandTimerRef.current);
      }
    };
  }, []);

  const refresh = useCallback(() => {
    setLocalRefreshToken((value) => value + 1);
  }, []);

  const visibleEntries = useMemo(
    () => flattenVisibleEntries(roots, expanded),
    [expanded, roots],
  );

  const selectedRelativePaths = useMemo(
    () => new Set(Object.keys(selectedEntries)),
    [selectedEntries],
  );

  const visibleEntryByPath = useMemo(() => {
    const byPath = new Map<string, WorkspaceEntry>();
    for (const entry of visibleEntries) byPath.set(entry.relativePath, entry);
    return byPath;
  }, [visibleEntries]);

  const selectOnly = useCallback((entry: WorkspaceEntry | null) => {
    setSelectedEntry(entry);
    setSelectedEntries(entry ? { [entry.relativePath]: entry } : {});
  }, []);

  const selectEntry = useCallback(
    (entry: WorkspaceEntry, event?: React.MouseEvent) => {
      bodyRef.current?.focus();
      const addRange = Boolean(event?.shiftKey && selectedEntry);
      const toggleEntry = Boolean(event?.metaKey || event?.ctrlKey);

      if (addRange && selectedEntry) {
        const entries = entriesInVisibleRange(
          visibleEntries,
          selectedEntry.relativePath,
          entry.relativePath,
        );
        setSelectedEntries(selectionMap(entries));
        setSelectedEntry(entry);
        return;
      }

      if (toggleEntry) {
        setSelectedEntries((prev) => {
          const next = { ...prev };
          if (next[entry.relativePath]) {
            delete next[entry.relativePath];
          } else {
            next[entry.relativePath] = entry;
          }
          return next;
        });
        setSelectedEntry(entry);
        return;
      }

      selectOnly(entry);
    },
    [selectOnly, selectedEntry, visibleEntries],
  );

  const toggleKeyboardSelection = useCallback((entry: WorkspaceEntry) => {
    setSelectedEntries((prev) => {
      const next = { ...prev };
      if (next[entry.relativePath] && Object.keys(next).length > 1) {
        delete next[entry.relativePath];
      } else {
        next[entry.relativePath] = entry;
      }
      return next;
    });
    setSelectedEntry(entry);
  }, []);

  const selectedScopeFor = useCallback(
    (entry: WorkspaceEntry | null) => {
      if (entry && selectedEntries[entry.relativePath]) {
        return compactSelection(Object.values(selectedEntries));
      }
      if (entry) return [entry];
      return compactSelection(Object.values(selectedEntries));
    },
    [selectedEntries],
  );

  const expandDirectory = useCallback(
    async (entry: WorkspaceEntry) => {
      if (entry.kind !== "directory") return;
      const key = entry.relativePath;
      const current = expandedRef.current[key];
      if (current?.expanded && current.children) return;

      setExpanded((prev) => ({
        ...prev,
        [key]: {
          entry,
          expanded: true,
          loading: !current?.children,
          children: current?.children,
          error: undefined,
        },
      }));

      if (current?.children) return;
      try {
        const children = await api.listEntries(workspacePath, key);
        setExpanded((prev) => ({
          ...prev,
          [key]: {
            entry,
            expanded: true,
            loading: false,
            children,
          },
        }));
      } catch (err) {
        setExpanded((prev) => ({
          ...prev,
          [key]: {
            entry,
            expanded: true,
            loading: false,
            error: String(err),
          },
        }));
      }
    },
    [workspacePath],
  );

  const toggle = useCallback(
    async (entry: WorkspaceEntry) => {
      const key = entry.relativePath;
      const current = expandedRef.current[key];
      if (current?.expanded) {
        setExpanded((prev) => ({
          ...prev,
          [key]: { ...current, expanded: false },
        }));
        return;
      }
      if (current?.children) {
        setExpanded((prev) => ({
          ...prev,
          [key]: { ...current, expanded: true },
        }));
        return;
      }
      await expandDirectory(entry);
    },
    [expandDirectory],
  );

  const closeMenu = useCallback(() => setMenu(null), []);

  useEffect(() => {
    if (!menu) return;
    const onPointerDown = () => closeMenu();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") closeMenu();
    };
    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [menu, closeMenu]);

  const openContextMenu = useCallback(
    (event: React.MouseEvent, entry: WorkspaceEntry | null) => {
      event.preventDefault();
      event.stopPropagation();
      if (entry && selectedEntries[entry.relativePath]) {
        setSelectedEntry(entry);
      } else {
        selectOnly(entry);
      }
      setActionError(null);
      const width = 196;
      const height = entry ? 392 : 138;
      const x =
        typeof window === "undefined"
          ? event.clientX
          : Math.min(event.clientX, window.innerWidth - width - 8);
      const y =
        typeof window === "undefined"
          ? event.clientY
          : Math.min(event.clientY, window.innerHeight - height - 8);
      setMenu({
        x: Math.max(8, x),
        y: Math.max(8, y),
        entry,
      });
      bodyRef.current?.focus();
    },
    [selectOnly, selectedEntries],
  );

  const startCreate = useCallback(
    async (kind: WorkspaceEntry["kind"], entry: WorkspaceEntry | null) => {
      closeMenu();
      setActionError(null);
      const parentRelativePath = targetDirectoryFor(entry);
      if (entry?.kind === "directory") {
        await expandDirectory(entry);
      }
      setEditState({
        mode: "create",
        kind,
        parentRelativePath,
        draft: kind === "file" ? "untitled.txt" : "new-folder",
      });
    },
    [closeMenu, expandDirectory],
  );

  useImperativeHandle(
    ref,
    () => ({
      startCreateRoot: (kind: WorkspaceEntry["kind"]) => {
        void startCreate(kind, null);
      },
    }),
    [startCreate],
  );

  const startRename = useCallback(
    (entry: WorkspaceEntry | null) => {
      if (!entry) return;
      closeMenu();
      setActionError(null);
      setSelectedEntry(entry);
      setEditState({ mode: "rename", entry, draft: entry.name });
    },
    [closeMenu],
  );

  const cancelEdit = useCallback(() => {
    submittingEditRef.current = false;
    setEditState(null);
  }, []);

  const updateDraft = useCallback((draft: string) => {
    setEditState((prev) => (prev ? { ...prev, draft } : prev));
  }, []);

  const submitEdit = useCallback(async () => {
    if (!editState) return;
    if (submittingEditRef.current) return;
    const name = editState.draft.trim();
    if (!name) {
      setEditState(null);
      return;
    }

    submittingEditRef.current = true;
    try {
      setActionError(null);
      if (editState.mode === "create") {
        const entry =
          editState.kind === "file"
            ? await api.createFile(
                workspacePath,
                editState.parentRelativePath,
                name,
              )
            : await api.createDirectory(
                workspacePath,
                editState.parentRelativePath,
                name,
              );
        setEditState(null);
        setSelectedEntry(entry);
        setSelectedEntries({ [entry.relativePath]: entry });
        refresh();
        onEntryCreated?.(entry);
        if (entry.kind === "file") onOpenFile(entry);
        return;
      }

      const oldRelativePath = editState.entry.relativePath;
      const entry = await api.renameEntry(workspacePath, oldRelativePath, name);
      setEditState(null);
      setSelectedEntry(entry);
      setSelectedEntries({ [entry.relativePath]: entry });
      refresh();
      onEntryRenamed?.(oldRelativePath, entry);
    } catch (err) {
      setActionError(String(err));
    } finally {
      submittingEditRef.current = false;
    }
  }, [
    editState,
    onEntryCreated,
    onEntryRenamed,
    onOpenFile,
    refresh,
    workspacePath,
  ]);

  const copySelection = useCallback(
    (entry: WorkspaceEntry | null, cut: boolean) => {
      const entries = selectedScopeFor(entry);
      if (!entries.length) return;
      setClipboard({ mode: cut ? "cut" : "copy", entries });
      setSelectedEntry(entry ?? entries[0] ?? null);
      setSelectedEntries(selectionMap(entries));
      closeMenu();
      void navigator.clipboard
        ?.writeText(entries.map((item) => item.absolutePath).join("\n"))
        .catch(() => {});
    },
    [closeMenu, selectedScopeFor],
  );

  const rememberDeletedEntries = useCallback((entries: DeletedEntryBatch) => {
    if (!entries.length) return;
    deletedEntryBatchesRef.current = [
      ...deletedEntryBatchesRef.current.slice(-19),
      entries,
    ];
  }, []);

  const restoreLastDeletedEntries = useCallback(async () => {
    if (restoringDeletedEntriesRef.current) return;
    const batch = deletedEntryBatchesRef.current.pop();
    if (!batch?.length) return;

    restoringDeletedEntriesRef.current = true;
    closeMenu();
    try {
      setActionError(null);
      const restored = await api.restoreDeletedEntries(workspacePath, batch);
      setSelectedEntry(restored[0] ?? null);
      setSelectedEntries(selectionMap(restored));
      refresh();
      window.setTimeout(() => bodyRef.current?.focus(), 0);
    } catch (err) {
      deletedEntryBatchesRef.current.push(batch);
      setActionError(String(err));
    } finally {
      restoringDeletedEntriesRef.current = false;
    }
  }, [closeMenu, refresh, workspacePath]);

  const deleteEntries = useCallback(
    async (entry: WorkspaceEntry | null) => {
      const entries = selectedScopeFor(entry);
      if (!entries.length) return;
      closeMenu();
      const confirmed = window.confirm(deleteConfirmationText(entries));
      if (!confirmed) return;
      const deleted: DeletedEntryBatch = [];
      try {
        setActionError(null);
        for (const item of entries) {
          const trashed = await api.trashEntry(workspacePath, item.relativePath);
          deleted.push(trashed);
          onEntryDeleted?.(item);
        }
        rememberDeletedEntries(deleted);
        setSelectedEntry(null);
        setSelectedEntries((prev) => {
          const next = { ...prev };
          for (const item of entries) {
            delete next[item.relativePath];
          }
          return next;
        });
        refresh();
        window.setTimeout(() => bodyRef.current?.focus(), 0);
      } catch (err) {
        rememberDeletedEntries(deleted);
        refresh();
        setActionError(String(err));
      }
    },
    [
      closeMenu,
      onEntryDeleted,
      refresh,
      rememberDeletedEntries,
      selectedScopeFor,
      workspacePath,
    ],
  );

  const revealEntry = useCallback(
    async (entry: WorkspaceEntry | null) => {
      if (!entry) return;
      closeMenu();
      try {
        setActionError(null);
        await api.revealEntry(workspacePath, entry.relativePath);
      } catch (err) {
        setActionError(String(err));
      }
    },
    [closeMenu, workspacePath],
  );

  const copyEntryPaths = useCallback(
    async (entry: WorkspaceEntry | null, mode: "absolute" | "relative") => {
      const entries = selectedScopeFor(entry);
      if (!entries.length) return;
      closeMenu();
      setSelectedEntry(entry ?? entries[0] ?? null);
      setSelectedEntries(selectionMap(entries));
      try {
        setActionError(null);
        if (!navigator.clipboard) {
          throw new Error("Clipboard is unavailable.");
        }
        const text = entries
          .map((item) =>
            mode === "absolute" ? item.absolutePath : item.relativePath,
          )
          .join("\n");
        await navigator.clipboard.writeText(text);
      } catch (err) {
        setActionError(String(err));
      }
    },
    [closeMenu, selectedScopeFor],
  );

  const pasteInto = useCallback(
    async (entry: WorkspaceEntry | null) => {
      closeMenu();
      const targetRelativePath = targetDirectoryFor(entry);
      try {
        setActionError(null);
        if (clipboard?.entries.length) {
          const sources = clipboard.entries.map((item) => item.relativePath);
          const pasted = await api.copyEntries(
            workspacePath,
            sources,
            targetRelativePath,
            clipboard.mode === "cut",
          );
          if (clipboard.mode === "cut") {
            onEntriesMoved?.(
              clipboard.entries.map((from, index) => ({
                from,
                to: pasted[index] ?? from,
              })),
            );
            setClipboard(null);
          }
          setSelectedEntry(pasted[0] ?? entry);
          setSelectedEntries(selectionMap(pasted));
          refresh();
          return;
        }

        const paths = await readExternalClipboardPaths();
        if (!paths.length) {
          setActionError("Clipboard has no files to paste.");
          return;
        }
        await api.importPaths(
          workspacePath,
          paths,
          targetRelativePath ?? undefined,
        );
        refresh();
      } catch (err) {
        setActionError(String(err));
      }
    },
    [clipboard, closeMenu, onEntriesMoved, refresh, workspacePath],
  );

  const clearInternalDragState = useCallback(() => {
    draggedEntriesRef.current = null;
    setInternalDragActive(false);
    setInternalDropTargetRelative(null);
    setInternalDropHoverRelative(null);
    setDragPreview(null);
    if (hoverExpandTimerRef.current !== null) {
      window.clearTimeout(hoverExpandTimerRef.current);
      hoverExpandTimerRef.current = null;
    }
    hoverExpandPathRef.current = null;
  }, []);

  const dropTargetAtPoint = useCallback(
    (
      x: number,
      y: number,
      dragged: WorkspaceEntry[],
    ): DropPointTarget | null => {
      const body = bodyRef.current;
      const element = document.elementFromPoint(x, y);
      if (!body || !element || !body.contains(element)) return null;

      const row = (element as Element).closest(
        ".tree-row[data-entry-path]",
      ) as HTMLElement | null;
      if (row && body.contains(row)) {
        const relativePath = row.dataset.entryPath;
        const kind = row.dataset.entryKind;
        if (!relativePath || (kind !== "file" && kind !== "directory")) {
          return null;
        }
        const entry = visibleEntryByPath.get(relativePath);
        if (!entry) return null;
        const target = dropTargetForEntry(entry, dragged);
        if (target === undefined) return null;
        return {
          targetRelativePath: target,
          hoverRelativePath: relativePath,
        };
      }

      if (!canMoveEntriesToTarget(dragged, null)) return null;
      return { targetRelativePath: null, hoverRelativePath: null };
    },
    [visibleEntryByPath],
  );

  const moveEntriesInto = useCallback(
    async (entries: WorkspaceEntry[], targetRelativePath: string | null) => {
      const movable = compactSelection(entries);
      if (!movable.length) return;
      if (!canMoveEntriesToTarget(movable, targetRelativePath)) return;

      try {
        setActionError(null);
        const moved = await api.copyEntries(
          workspacePath,
          movable.map((entry) => entry.relativePath),
          targetRelativePath,
          true,
        );
        onEntriesMoved?.(
          movable.map((from, index) => ({
            from,
            to: moved[index] ?? from,
          })),
        );
        setSelectedEntry(moved[0] ?? null);
        setSelectedEntries(selectionMap(moved));
        refresh();
      } catch (err) {
        setActionError(String(err));
      }
    },
    [onEntriesMoved, refresh, workspacePath],
  );

  const draggedEntriesFromEvent = useCallback(
    (event: React.DragEvent): WorkspaceEntry[] => {
      const parsed = parseInternalDragEntries(event.dataTransfer);
      return parsed.length ? parsed : draggedEntriesRef.current ?? [];
    },
    [],
  );

  const hasActiveInternalDrag = useCallback((event: React.DragEvent) => {
    return (
      Boolean(draggedEntriesRef.current?.length) ||
      hasInternalDragPayload(event.dataTransfer)
    );
  }, []);

  const queueHoverExpand = useCallback(
    (entry: WorkspaceEntry) => {
      if (entry.kind !== "directory") return;
      const current = expandedRef.current[entry.relativePath];
      if (current?.expanded) return;
      if (hoverExpandPathRef.current === entry.relativePath) return;

      if (hoverExpandTimerRef.current !== null) {
        window.clearTimeout(hoverExpandTimerRef.current);
      }
      hoverExpandPathRef.current = entry.relativePath;
      hoverExpandTimerRef.current = window.setTimeout(() => {
        hoverExpandTimerRef.current = null;
        hoverExpandPathRef.current = null;
        void expandDirectory(entry);
      }, 650);
    },
    [expandDirectory],
  );

  const updatePointerDropTarget = useCallback(
    (x: number, y: number, dragged: WorkspaceEntry[]) => {
      const target = dropTargetAtPoint(x, y, dragged);
      if (!target) {
        setInternalDropTargetRelative(null);
        setInternalDropHoverRelative(null);
        return null;
      }

      setInternalDropTargetRelative(target.targetRelativePath);
      setInternalDropHoverRelative(target.hoverRelativePath);

      if (target.hoverRelativePath && target.targetRelativePath === target.hoverRelativePath) {
        const hovered = visibleEntryByPath.get(target.hoverRelativePath);
        if (hovered?.kind === "directory") queueHoverExpand(hovered);
      }

      return target;
    },
    [dropTargetAtPoint, queueHoverExpand, visibleEntryByPath],
  );

  useEffect(() => {
    const onPointerMove = (event: PointerEvent) => {
      const state = pointerDragRef.current;
      if (!state || event.pointerId !== state.pointerId) return;

      const distance = Math.hypot(
        event.clientX - state.startX,
        event.clientY - state.startY,
      );
      if (!state.active && distance < POINTER_DRAG_THRESHOLD_PX) return;

      if (!state.active) {
        state.active = true;
        suppressNextClickRef.current = true;
        draggedEntriesRef.current = state.entries;
        setSelectedEntry(state.entry);
        setSelectedEntries(selectionMap(state.entries));
        setInternalDragActive(true);
      }

      event.preventDefault();
      setDragPreview({ x: event.clientX, y: event.clientY, entries: state.entries });
      updatePointerDropTarget(event.clientX, event.clientY, state.entries);
    };

    const onPointerUp = (event: PointerEvent) => {
      const state = pointerDragRef.current;
      if (!state || event.pointerId !== state.pointerId) return;
      pointerDragRef.current = null;

      if (!state.active) return;
      event.preventDefault();
      suppressNextClickRef.current = true;
      const target = dropTargetAtPoint(event.clientX, event.clientY, state.entries);
      clearInternalDragState();
      if (!target) return;
      void moveEntriesInto(state.entries, target.targetRelativePath);
    };

    const onPointerCancel = (event: PointerEvent) => {
      const state = pointerDragRef.current;
      if (!state || event.pointerId !== state.pointerId) return;
      pointerDragRef.current = null;
      clearInternalDragState();
    };

    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", onPointerCancel);
    return () => {
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
      window.removeEventListener("pointercancel", onPointerCancel);
    };
  }, [
    clearInternalDragState,
    dropTargetAtPoint,
    moveEntriesInto,
    updatePointerDropTarget,
  ]);

  const handleEntryPointerDown = useCallback(
    (entry: WorkspaceEntry, event: React.PointerEvent) => {
      if (event.button !== 0) return;
      if (editState) return;
      const entries = selectedScopeFor(entry);
      pointerDragRef.current = {
        pointerId: event.pointerId,
        startX: event.clientX,
        startY: event.clientY,
        entry,
        entries: entries.length ? entries : [entry],
        active: false,
      };
    },
    [editState, selectedScopeFor],
  );

  const suppressClickAfterDrag = useCallback((event: React.MouseEvent) => {
    if (!suppressNextClickRef.current) return false;
    suppressNextClickRef.current = false;
    event.preventDefault();
    event.stopPropagation();
    return true;
  }, []);

  const handleEntryDragStart = useCallback(
    (entry: WorkspaceEntry, event: React.DragEvent) => {
      bodyRef.current?.focus();
      closeMenu();

      const entries = selectedScopeFor(entry);
      const dragged = entries.length ? entries : [entry];
      draggedEntriesRef.current = dragged;
      setInternalDragActive(true);
      setInternalDropTargetRelative(null);
      setSelectedEntry(entry);
      setSelectedEntries(selectionMap(dragged));

      if (entry.kind === "file" && dragged.length === 1) {
        onDragFile(entry, event);
      }

      event.dataTransfer.setData(
        INTERNAL_DRAG_MIME,
        JSON.stringify({ entries: dragged } satisfies InternalDragPayload),
      );
      event.dataTransfer.setData(
        "text/plain",
        dragged.map((item) => item.relativePath).join("\n"),
      );
      event.dataTransfer.effectAllowed = "copyMove";
    },
    [closeMenu, onDragFile, selectedScopeFor],
  );

  const handleEntryDragEnd = useCallback(() => {
    clearInternalDragState();
  }, [clearInternalDragState]);

  const handleEntryDragOver = useCallback(
    (entry: WorkspaceEntry, event: React.DragEvent) => {
      if (!hasActiveInternalDrag(event)) return;
      const dragged = draggedEntriesFromEvent(event);
      if (!dragged.length) return;
      const target = dropTargetForEntry(entry, dragged);
      if (target === undefined) return;

      event.preventDefault();
      event.stopPropagation();
      event.dataTransfer.dropEffect = "move";
      setInternalDragActive(true);
      setInternalDropTargetRelative(target);
      setInternalDropHoverRelative(entry.relativePath);
      if (target === entry.relativePath) queueHoverExpand(entry);
    },
    [draggedEntriesFromEvent, hasActiveInternalDrag, queueHoverExpand],
  );

  const handleEntryDrop = useCallback(
    async (entry: WorkspaceEntry, event: React.DragEvent) => {
      if (!hasActiveInternalDrag(event)) return;
      event.preventDefault();
      event.stopPropagation();
      const dragged = draggedEntriesFromEvent(event);
      const target = dropTargetForEntry(entry, dragged);
      clearInternalDragState();
      if (target === undefined) return;
      await moveEntriesInto(dragged, target);
    },
    [clearInternalDragState, draggedEntriesFromEvent, hasActiveInternalDrag, moveEntriesInto],
  );

  const handleRootDragOver = useCallback(
    (event: React.DragEvent<HTMLDivElement>) => {
      if (!hasActiveInternalDrag(event)) return;
      if ((event.target as Element).closest(".tree-row")) return;
      const dragged = draggedEntriesFromEvent(event);
      if (!dragged.length) return;
      if (!canMoveEntriesToTarget(dragged, null)) return;

      event.preventDefault();
      event.dataTransfer.dropEffect = "move";
      setInternalDragActive(true);
      setInternalDropTargetRelative(null);
      setInternalDropHoverRelative(null);
    },
    [draggedEntriesFromEvent, hasActiveInternalDrag],
  );

  const handleRootDrop = useCallback(
    async (event: React.DragEvent<HTMLDivElement>) => {
      if (!hasActiveInternalDrag(event)) return;
      if ((event.target as Element).closest(".tree-row")) return;
      event.preventDefault();
      const dragged = draggedEntriesFromEvent(event);
      clearInternalDragState();
      await moveEntriesInto(dragged, null);
    },
    [clearInternalDragState, draggedEntriesFromEvent, hasActiveInternalDrag, moveEntriesInto],
  );

  const handleRootDragLeave = useCallback(
    (event: React.DragEvent<HTMLDivElement>) => {
      const next = event.relatedTarget;
      if (next instanceof Node && bodyRef.current?.contains(next)) return;
      setInternalDragActive(false);
      setInternalDropTargetRelative(null);
      setInternalDropHoverRelative(null);
    },
    [],
  );

  const handleBodyKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (editState) return;
      const entry = selectedEntry;
      const command = event.metaKey || event.ctrlKey;

      if (
        command &&
        !event.shiftKey &&
        !event.altKey &&
        event.key.toLowerCase() === "z"
      ) {
        event.preventDefault();
        void restoreLastDeletedEntries();
        return;
      }
      if (command && event.key.toLowerCase() === "c") {
        event.preventDefault();
        copySelection(entry, false);
        return;
      }
      if (command && event.key.toLowerCase() === "x") {
        event.preventDefault();
        copySelection(entry, true);
        return;
      }
      if (command && event.key.toLowerCase() === "v") {
        event.preventDefault();
        void pasteInto(entry);
        return;
      }
      if (event.shiftKey && event.key === "Tab") {
        event.preventDefault();
        if (entry) toggleKeyboardSelection(entry);
        return;
      }
      if (event.key === "F2") {
        event.preventDefault();
        startRename(entry);
        return;
      }
      if (event.key === "Delete" || event.key === "Backspace") {
        event.preventDefault();
        void deleteEntries(entry);
        return;
      }
      if (event.key === "Enter" && entry) {
        event.preventDefault();
        if (entry.kind === "directory") void toggle(entry);
        else onOpenFile(entry);
      }
    },
    [
      copySelection,
      deleteEntries,
      editState,
      onOpenFile,
      pasteInto,
      restoreLastDeletedEntries,
      selectedEntry,
      startRename,
      toggle,
      toggleKeyboardSelection,
    ],
  );

  const cutRelativePaths = useMemo(() => {
    if (clipboard?.mode !== "cut") return new Set<string>();
    return new Set(clipboard.entries.map((entry) => entry.relativePath));
  }, [clipboard]);

  const effectiveDropActive = Boolean(dropActive || internalDragActive);
  const effectiveDropTargetRelative = internalDragActive
    ? internalDropTargetRelative
    : dropActive
      ? dropTargetRelative ?? null
      : null;
  const rootTarget =
    effectiveDropActive &&
    (effectiveDropTargetRelative === null || effectiveDropTargetRelative === "");
  const rootCreate =
    editState?.mode === "create" && editState.parentRelativePath === null
      ? editState
      : null;

  return (
    <div
      ref={bodyRef}
      className="sidebar__body"
      data-drop-active={effectiveDropActive ? "true" : "false"}
      data-drop-root={rootTarget ? "true" : "false"}
      tabIndex={0}
      onKeyDown={handleBodyKeyDown}
      onDragOver={handleRootDragOver}
      onDragLeave={handleRootDragLeave}
      onDrop={(event) => void handleRootDrop(event)}
      onContextMenu={(event) => {
        if ((event.target as Element).closest(".tree-row")) return;
        openContextMenu(event, null);
      }}
      onMouseDown={() => bodyRef.current?.focus()}
    >
      {(rootError || actionError) && (
        <div className="tree-error" onClick={() => setActionError(null)}>
          {actionError ?? rootError}
        </div>
      )}
      {rootCreate && (
        <InlineEditRow
          depth={0}
          kind={rootCreate.kind}
          value={rootCreate.draft}
          onChange={updateDraft}
          onSubmit={submitEdit}
          onCancel={cancelEdit}
        />
      )}
      {roots.map((entry) => (
        <TreeNode
          key={entry.relativePath}
          entry={entry}
          depth={0}
          expanded={expanded}
          editState={editState}
          selectedRelativePaths={selectedRelativePaths}
          cutRelativePaths={cutRelativePaths}
          toggle={toggle}
          activeFile={activeFile}
          onOpenFile={onOpenFile}
          onDragStart={handleEntryDragStart}
          onDragEnd={handleEntryDragEnd}
          onPointerDown={handleEntryPointerDown}
          suppressClick={suppressClickAfterDrag}
          onEntryDragOver={handleEntryDragOver}
          onEntryDrop={(entry, event) => void handleEntryDrop(entry, event)}
          onSelect={selectEntry}
          onContextMenu={openContextMenu}
          onChangeDraft={updateDraft}
          onSubmitEdit={submitEdit}
          onCancelEdit={cancelEdit}
          dropTargetRelative={
            effectiveDropActive ? effectiveDropTargetRelative : null
          }
          dropHoverRelative={
            effectiveDropActive ? internalDropHoverRelative : null
          }
        />
      ))}
      {!roots.length && !rootCreate && !rootError && (
        <div className="tree-empty">No files</div>
      )}
      {menu && (
        <TreeContextMenu
          menu={menu}
          hasClipboard={Boolean(clipboard?.entries.length)}
          onNewFile={() => void startCreate("file", menu.entry)}
          onNewFolder={() => void startCreate("directory", menu.entry)}
          onOpen={() => {
            if (!menu.entry) return;
            closeMenu();
            if (menu.entry.kind === "directory") void toggle(menu.entry);
            else onOpenFile(menu.entry);
          }}
          onRename={() => startRename(menu.entry)}
          onReveal={() => void revealEntry(menu.entry)}
          onCopy={() => copySelection(menu.entry, false)}
          onCut={() => copySelection(menu.entry, true)}
          onCopyPath={() => void copyEntryPaths(menu.entry, "absolute")}
          onCopyRelativePath={() => void copyEntryPaths(menu.entry, "relative")}
          onPaste={() => void pasteInto(menu.entry)}
          onDelete={() => void deleteEntries(menu.entry)}
          onRefresh={() => {
            closeMenu();
            refresh();
          }}
        />
      )}
      {dragPreview && <DragPreview preview={dragPreview} />}
    </div>
  );
});

function DragPreview({ preview }: { preview: DragPreviewState }) {
  const first = preview.entries[0];
  if (!first) return null;
  const count = preview.entries.length;
  return (
    <div
      className="tree-drag-preview"
      style={{
        transform: `translate3d(${preview.x + 12}px, ${preview.y + 12}px, 0)`,
      }}
    >
      <span className="tree-drag-preview__icon">
        <EntryIcon entry={first} open={false} />
      </span>
      <span className="tree-drag-preview__name">{first.name}</span>
      {count > 1 && <span className="tree-drag-preview__count">{count}</span>}
    </div>
  );
}

type NodeProps = {
  entry: WorkspaceEntry;
  depth: number;
  expanded: Record<string, NodeState>;
  editState: EditState | null;
  selectedRelativePaths: Set<string>;
  cutRelativePaths: Set<string>;
  toggle: (entry: WorkspaceEntry) => void;
  activeFile: string | null;
  onOpenFile: (entry: WorkspaceEntry) => void;
  onDragStart: (entry: WorkspaceEntry, event: React.DragEvent) => void;
  onDragEnd: () => void;
  onPointerDown: (entry: WorkspaceEntry, event: React.PointerEvent) => void;
  suppressClick: (event: React.MouseEvent) => boolean;
  onEntryDragOver: (entry: WorkspaceEntry, event: React.DragEvent) => void;
  onEntryDrop: (entry: WorkspaceEntry, event: React.DragEvent) => void;
  onSelect: (entry: WorkspaceEntry, event?: React.MouseEvent) => void;
  onContextMenu: (event: React.MouseEvent, entry: WorkspaceEntry) => void;
  onChangeDraft: (draft: string) => void;
  onSubmitEdit: () => void;
  onCancelEdit: () => void;
  dropTargetRelative: string | null;
  dropHoverRelative: string | null;
};

const TreeNode = memo(function TreeNode({
  entry,
  depth,
  expanded,
  editState,
  selectedRelativePaths,
  cutRelativePaths,
  toggle,
  activeFile,
  onOpenFile,
  onDragStart,
  onDragEnd,
  onPointerDown,
  suppressClick,
  onEntryDragOver,
  onEntryDrop,
  onSelect,
  onContextMenu,
  onChangeDraft,
  onSubmitEdit,
  onCancelEdit,
  dropTargetRelative,
  dropHoverRelative,
}: NodeProps) {
  const isDir = entry.kind === "directory";
  const state = expanded[entry.relativePath];
  const open = Boolean(state?.expanded);
  const childCreate =
    editState?.mode === "create" &&
    editState.parentRelativePath === entry.relativePath
      ? editState
      : null;
  const rename =
    editState?.mode === "rename" &&
    editState.entry.relativePath === entry.relativePath
      ? editState
      : null;

  const handleClick = (event: React.MouseEvent) => {
    if (suppressClick(event)) return;
    onSelect(entry, event);
    if (event.shiftKey || event.metaKey || event.ctrlKey) return;
    if (isDir) toggle(entry);
    else onOpenFile(entry);
  };

  const iconName = isDir ? folderIcon(entry.name, open) : fileIcon(entry.name);
  const showCaret =
    isDir && (entry.hasChildren || state?.children?.length || open || childCreate);
  const dropTarget =
    dropHoverRelative !== null
      ? dropHoverRelative === entry.relativePath
      : isDir &&
        dropTargetRelative !== null &&
        dropTargetRelative === entry.relativePath;

  return (
    <>
      {rename ? (
        <InlineEditRow
          depth={depth}
          kind={entry.kind}
          iconName={iconName}
          value={rename.draft}
          onChange={onChangeDraft}
          onSubmit={onSubmitEdit}
          onCancel={onCancelEdit}
        />
      ) : (
        <div
          className="tree-row"
          data-kind={entry.kind}
          data-active={
            !isDir && activeFile === entry.relativePath ? "true" : "false"
          }
          data-selected={
            selectedRelativePaths.has(entry.relativePath) ? "true" : "false"
          }
          data-cut={cutRelativePaths.has(entry.relativePath) ? "true" : "false"}
          data-drop-target={dropTarget ? "true" : "false"}
          data-drop-path={isDir ? entry.relativePath : undefined}
          data-entry-path={entry.relativePath}
          data-entry-kind={entry.kind}
          style={{ paddingLeft: 8 + depth * 14 }}
          onClick={handleClick}
          onContextMenu={(event) => onContextMenu(event, entry)}
          draggable={false}
          onPointerDown={(event) => onPointerDown(entry, event)}
          onDragStart={(event) => onDragStart(entry, event)}
          onDragEnd={onDragEnd}
          onDragOver={(event) => onEntryDragOver(entry, event)}
          onDrop={(event) => onEntryDrop(entry, event)}
        >
          <span
            className="tree-row__caret"
            data-empty={!isDir || !showCaret ? "true" : "false"}
          >
            {isDir && showCaret ? (
              <Icon
                icon={
                  open
                    ? "solar:alt-arrow-down-linear"
                    : "solar:alt-arrow-right-linear"
                }
                width={12}
                height={12}
              />
            ) : null}
          </span>
          <span className="tree-row__icon">
            <EntryIcon entry={entry} open={open} iconName={iconName} />
          </span>
          <span className="tree-row__name">{entry.name}</span>
        </div>
      )}
      {isDir && open && (
        <>
          {childCreate && (
            <InlineEditRow
              depth={depth + 1}
              kind={childCreate.kind}
              value={childCreate.draft}
              onChange={onChangeDraft}
              onSubmit={onSubmitEdit}
              onCancel={onCancelEdit}
            />
          )}
          {state.loading && (
            <div
              className="tree-row tree-row--sub"
              style={{ paddingLeft: 8 + (depth + 1) * 14 }}
            >
              <span className="tree-row__caret" data-empty="true" />
              <span className="tree-row__icon">
                <Icon icon="solar:refresh-linear" width={14} height={14} />
              </span>
              <span className="tree-row__name" style={{ color: "var(--text-3)" }}>
                loading...
              </span>
            </div>
          )}
          {state.error && !state.loading && (
            <div
              className="tree-row tree-row--sub"
              style={{ paddingLeft: 8 + (depth + 1) * 14 }}
            >
              <span className="tree-row__caret" data-empty="true" />
              <span className="tree-row__icon">
                <Icon icon="solar:danger-triangle-linear" width={14} height={14} />
              </span>
              <span className="tree-row__name" style={{ color: "var(--danger)" }}>
                {state.error}
              </span>
            </div>
          )}
          {state.children?.map((child) => (
            <TreeNode
              key={child.relativePath}
              entry={child}
              depth={depth + 1}
              expanded={expanded}
              editState={editState}
              selectedRelativePaths={selectedRelativePaths}
              cutRelativePaths={cutRelativePaths}
              toggle={toggle}
              activeFile={activeFile}
              onOpenFile={onOpenFile}
              onDragStart={onDragStart}
              onDragEnd={onDragEnd}
              onPointerDown={onPointerDown}
              suppressClick={suppressClick}
              onEntryDragOver={onEntryDragOver}
              onEntryDrop={onEntryDrop}
              onSelect={onSelect}
              onContextMenu={onContextMenu}
              onChangeDraft={onChangeDraft}
              onSubmitEdit={onSubmitEdit}
              onCancelEdit={onCancelEdit}
              dropTargetRelative={dropTargetRelative}
              dropHoverRelative={dropHoverRelative}
            />
          ))}
        </>
      )}
    </>
  );
});

function InlineEditRow({
  depth,
  kind,
  iconName,
  value,
  onChange,
  onSubmit,
  onCancel,
}: {
  depth: number;
  kind: WorkspaceEntry["kind"];
  iconName?: string;
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
  onCancel: () => void;
}) {
  const inputRef = useRef<HTMLInputElement | null>(null);
  const ignoreNextBlurRef = useRef(false);

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  return (
    <div
      className="tree-row tree-row--editing"
      data-kind={kind}
      style={{ paddingLeft: 8 + depth * 14 }}
      onMouseDown={(event) => event.stopPropagation()}
      onClick={(event) => event.stopPropagation()}
    >
      <span className="tree-row__caret" data-empty="true" />
      <span className="tree-row__icon">
        <Icon
          icon={
            iconName ??
            (kind === "directory"
              ? "vscode-icons:default-folder"
              : "vscode-icons:default-file")
          }
          width={16}
          height={16}
        />
      </span>
      <input
        ref={inputRef}
        className="tree-row__input"
        value={value}
        spellCheck={false}
        onChange={(event) => onChange(event.target.value)}
        onBlur={() => {
          if (ignoreNextBlurRef.current) {
            ignoreNextBlurRef.current = false;
            return;
          }
          onSubmit();
        }}
        onKeyDown={(event) => {
          event.stopPropagation();
          if (event.key === "Enter") {
            event.preventDefault();
            onSubmit();
          }
          if (event.key === "Escape") {
            event.preventDefault();
            ignoreNextBlurRef.current = true;
            onCancel();
          }
        }}
      />
    </div>
  );
}

function EntryIcon({
  entry,
  open,
  iconName,
}: {
  entry: WorkspaceEntry;
  open: boolean;
  iconName?: string;
}) {
  if (entry.kind === "file" && isDesignMarkdown(entry.name)) {
    return <DesignMarkdownIcon />;
  }
  if (entry.kind === "file" && isAgentsMarkdown(entry.name)) {
    return <AgentsMarkdownIcon />;
  }
  if (entry.kind === "file" && isClaudeMarkdown(entry.name)) {
    return <ClaudeMarkdownIcon />;
  }
  return (
    <Icon
      icon={iconName ?? (entry.kind === "directory" ? folderIcon(entry.name, open) : fileIcon(entry.name))}
      width={16}
      height={16}
    />
  );
}

function DesignMarkdownIcon() {
  return (
    <svg
      className="design-file-icon"
      width="16"
      height="16"
      viewBox="0 0 16 16"
      aria-hidden="true"
      focusable="false"
    >
      <rect x="1" y="1" width="14" height="14" rx="3" fill="#18181b" />
      <rect x="2.2" y="2.2" width="11.6" height="11.6" rx="2.2" fill="#fff7ed" />
      <path
        d="M4.1 11.7 5 8.5l4.9-4.9a1.2 1.2 0 0 1 1.7 0l.8.8a1.2 1.2 0 0 1 0 1.7L7.5 11l-3.4.7Z"
        fill="#2563eb"
      />
      <path
        d="m9.6 3.9 2.5 2.5M5 8.5 7.5 11"
        stroke="#f8fafc"
        strokeWidth="1"
        strokeLinecap="round"
      />
      <circle cx="5" cy="4.7" r="1.2" fill="#ec4899" />
      <rect x="9.4" y="10.2" width="2.8" height="2.1" rx=".6" fill="#22c55e" />
    </svg>
  );
}

function isDesignMarkdown(name: string): boolean {
  return name.toLowerCase() === "design.md";
}

function AgentsMarkdownIcon() {
  // Terminal prompt mark (`>_` inside a circle) used for AGENTS.md files,
  // mirroring the dedicated DesignMarkdownIcon treatment for design.md.
  return (
    <svg
      className="agents-file-icon"
      width="16"
      height="16"
      viewBox="0 0 16 16"
      aria-hidden="true"
      focusable="false"
    >
      <circle
        cx="8"
        cy="8"
        r="6.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
      />
      <path
        d="M5.6 5.4 7.7 8l-2.1 2.6"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M8.6 10.6h2.4"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
    </svg>
  );
}

function isAgentsMarkdown(name: string): boolean {
  return name.toLowerCase() === "agents.md";
}

function ClaudeMarkdownIcon() {
  // Anthropic-style terracotta sunburst used for CLAUDE.md files.
  // 12 irregular rays radiating from a small central core.
  return (
    <svg
      className="claude-file-icon"
      width="16"
      height="16"
      viewBox="0 0 16 16"
      aria-hidden="true"
      focusable="false"
    >
      <polygon
        fill="#D97757"
        points="8,1 8.5,6.1 10.8,2.7 9.4,6.6 13.8,4.4 9.9,7.5 14.2,8 9.9,8.5 13.9,11.7 9.4,9.4 10.7,13.1 8.5,9.9 8,14.5 7.5,9.9 5,13.2 6.6,9.4 2.1,11.4 6.1,8.5 1.8,8 6.1,7.5 2.1,4.3 6.6,6.6 5.1,3 7.5,6.1"
      />
    </svg>
  );
}

function isClaudeMarkdown(name: string): boolean {
  return name.toLowerCase() === "claude.md";
}

function TreeContextMenu({
  menu,
  hasClipboard,
  onNewFile,
  onNewFolder,
  onOpen,
  onRename,
  onReveal,
  onCopy,
  onCut,
  onCopyPath,
  onCopyRelativePath,
  onPaste,
  onDelete,
  onRefresh,
}: {
  menu: ContextMenuState;
  hasClipboard: boolean;
  onNewFile: () => void;
  onNewFolder: () => void;
  onOpen: () => void;
  onRename: () => void;
  onReveal: () => void;
  onCopy: () => void;
  onCut: () => void;
  onCopyPath: () => void;
  onCopyRelativePath: () => void;
  onPaste: () => void;
  onDelete: () => void;
  onRefresh: () => void;
}) {
  const entry = menu.entry;

  return (
    <div
      className="tree-menu"
      role="menu"
      style={{ left: menu.x, top: menu.y }}
      onPointerDown={(event) => event.stopPropagation()}
      onContextMenu={(event) => event.preventDefault()}
    >
      {entry && (
        <MenuItem
          icon={entry.kind === "directory" ? "solar:folder-open-linear" : "solar:file-text-linear"}
          label={entry.kind === "directory" ? "Open folder" : "Open"}
          onClick={onOpen}
        />
      )}
      <MenuItem icon="solar:document-add-linear" label="New file" onClick={onNewFile} />
      <MenuItem
        icon="solar:add-folder-linear"
        label="New folder"
        onClick={onNewFolder}
      />
      <MenuSeparator />
      {entry && (
        <>
          <MenuItem icon="solar:copy-linear" label="Copy" onClick={onCopy} />
          <MenuItem icon="solar:scissors-linear" label="Cut" onClick={onCut} />
          <MenuSeparator />
          <MenuItem
            icon="solar:link-linear"
            label="Copy path"
            onClick={onCopyPath}
          />
          <MenuItem
            icon="solar:branching-paths-down-linear"
            label="Copy relative path"
            onClick={onCopyRelativePath}
          />
        </>
      )}
      <MenuItem
        icon="solar:clipboard-list-linear"
        label={hasClipboard ? "Paste" : "Paste from clipboard"}
        onClick={onPaste}
      />
      {entry && (
        <>
          <MenuSeparator />
          <MenuItem icon="solar:pen-linear" label="Rename" onClick={onRename} />
          <MenuItem
            icon="solar:folder-open-linear"
            label="Reveal in Finder"
            onClick={onReveal}
          />
          <MenuItem
            icon="solar:trash-bin-trash-linear"
            label="Delete"
            danger
            onClick={onDelete}
          />
        </>
      )}
      <MenuSeparator />
      <MenuItem icon="solar:refresh-linear" label="Refresh" onClick={onRefresh} />
    </div>
  );
}

function MenuItem({
  icon,
  label,
  danger,
  onClick,
}: {
  icon: string;
  label: string;
  danger?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className="tree-menu__item"
      data-danger={danger ? "true" : "false"}
      role="menuitem"
      onClick={onClick}
    >
      <Icon icon={icon} width={14} height={14} />
      <span>{label}</span>
    </button>
  );
}

function MenuSeparator() {
  return <div className="tree-menu__separator" role="separator" />;
}

function flattenVisibleEntries(
  entries: WorkspaceEntry[],
  expanded: Record<string, NodeState>,
): WorkspaceEntry[] {
  const visible: WorkspaceEntry[] = [];
  const visit = (items: WorkspaceEntry[]) => {
    for (const entry of items) {
      visible.push(entry);
      const state = expanded[entry.relativePath];
      if (entry.kind === "directory" && state?.expanded && state.children) {
        visit(state.children);
      }
    }
  };
  visit(entries);
  return visible;
}

function entriesInVisibleRange(
  entries: WorkspaceEntry[],
  fromRelativePath: string,
  toRelativePath: string,
): WorkspaceEntry[] {
  const from = entries.findIndex(
    (entry) => entry.relativePath === fromRelativePath,
  );
  const to = entries.findIndex((entry) => entry.relativePath === toRelativePath);
  if (from < 0 || to < 0) {
    const entry = entries[to] ?? entries[from];
    return entry ? [entry] : [];
  }

  const start = Math.min(from, to);
  const end = Math.max(from, to);
  return entries.slice(start, end + 1);
}

function selectionMap(
  entries: WorkspaceEntry[],
): Record<string, WorkspaceEntry> {
  return entries.reduce<Record<string, WorkspaceEntry>>((acc, entry) => {
    acc[entry.relativePath] = entry;
    return acc;
  }, {});
}

function compactSelection(entries: WorkspaceEntry[]): WorkspaceEntry[] {
  return entries.filter(
    (entry) =>
      !entries.some(
        (other) =>
          other.relativePath !== entry.relativePath &&
          other.kind === "directory" &&
          entry.relativePath.startsWith(`${other.relativePath}/`),
      ),
  );
}

function hasInternalDragPayload(dataTransfer: DataTransfer): boolean {
  const types = Array.from(dataTransfer.types);
  return (
    types.includes(INTERNAL_DRAG_MIME) ||
    types.includes("application/x-claakecode-file")
  );
}

function parseInternalDragEntries(dataTransfer: DataTransfer): WorkspaceEntry[] {
  const raw = dataTransfer.getData(INTERNAL_DRAG_MIME);
  if (raw) {
    try {
      const payload = JSON.parse(raw) as InternalDragPayload;
      if (Array.isArray(payload.entries)) {
        return payload.entries.filter(isWorkspaceEntry);
      }
    } catch {
      // Ignore malformed drag payloads from older app versions.
    }
  }

  const legacy = dataTransfer.getData("application/x-claakecode-file");
  if (!legacy) return [];
  try {
    const parsed = JSON.parse(legacy) as Partial<WorkspaceEntry>;
    if (
      typeof parsed.relativePath !== "string" ||
      typeof parsed.absolutePath !== "string" ||
      typeof parsed.name !== "string"
    ) {
      return [];
    }
    return [
      {
        name: parsed.name,
        relativePath: parsed.relativePath,
        absolutePath: parsed.absolutePath,
        kind: "file",
        hasChildren: false,
      },
    ];
  } catch {
    return [];
  }
}

function isWorkspaceEntry(value: unknown): value is WorkspaceEntry {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  const entry = value as Partial<WorkspaceEntry>;
  return (
    typeof entry.name === "string" &&
    typeof entry.relativePath === "string" &&
    typeof entry.absolutePath === "string" &&
    (entry.kind === "file" || entry.kind === "directory") &&
    typeof entry.hasChildren === "boolean"
  );
}

function canMoveEntriesToTarget(
  entries: WorkspaceEntry[],
  targetRelativePath: string | null,
): boolean {
  if (!entries.length) return false;
  const target = targetRelativePath || null;
  return compactSelection(entries).every((entry) => {
    if (!target) return true;
    if (target === entry.relativePath) return false;
    if (entry.kind === "directory" && target.startsWith(`${entry.relativePath}/`)) {
      return false;
    }
    return true;
  });
}

function dropTargetForEntry(
  entry: WorkspaceEntry,
  dragged: WorkspaceEntry[],
): string | null | undefined {
  const preferred =
    entry.kind === "directory"
      ? entry.relativePath
      : parentRelativePath(entry.relativePath);
  if (canMoveEntriesToTarget(dragged, preferred)) return preferred;

  const fallback = parentRelativePath(entry.relativePath);
  if (fallback !== preferred && canMoveEntriesToTarget(dragged, fallback)) {
    return fallback;
  }

  return undefined;
}

function deleteConfirmationText(entries: WorkspaceEntry[]): string {
  if (entries.length === 1) return `Delete "${entries[0].name}"?`;
  return `Delete ${entries.length} selected items?`;
}

function targetDirectoryFor(entry: WorkspaceEntry | null): string | null {
  if (!entry) return null;
  if (entry.kind === "directory") return entry.relativePath;
  return parentRelativePath(entry.relativePath);
}

function parentRelativePath(relativePath: string): string | null {
  const idx = relativePath.lastIndexOf("/");
  if (idx <= 0) return null;
  return relativePath.slice(0, idx);
}

async function readExternalClipboardPaths(): Promise<string[]> {
  const nativePaths = await api.readClipboardFilePaths().catch(() => []);
  if (nativePaths.length) return nativePaths;

  const text = await navigator.clipboard?.readText().catch(() => "");
  if (!text) return [];
  return text
    .split(/\r?\n/)
    .map((line) => line.trim().replace(/^file:\/\//, ""))
    .filter((line) => line.startsWith("/") || /^[A-Za-z]:[\\/]/.test(line));
}
