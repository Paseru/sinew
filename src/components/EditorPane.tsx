import Editor, { loader, type OnMount } from "@monaco-editor/react";
import { convertFileSrc } from "@tauri-apps/api/core";
import * as monacoNs from "monaco-editor";
import type * as Monaco from "monaco-editor";
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import jsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";
import cssWorker from "monaco-editor/esm/vs/language/css/css.worker?worker";
import htmlWorker from "monaco-editor/esm/vs/language/html/html.worker?worker";
import tsWorker from "monaco-editor/esm/vs/language/typescript/ts.worker?worker";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { Icon } from "@iconify/react";
import { languageForPath } from "../lib/language";
import { fileIcon } from "../lib/fileIcon";
import { api } from "../lib/ipc";
import { Markdown } from "./chat/Markdown";
import type { EditorDiagnosticInput, EditorRevealTarget, EditorTab } from "../types";
import { ImageContextMenu } from "./ImageContextMenu";
import { getAppLocale } from "../lib/locale";

if (!(globalThis as typeof globalThis & { MonacoEnvironment?: unknown }).MonacoEnvironment) {
  (
    globalThis as typeof globalThis & {
      MonacoEnvironment?: {
        getWorker: (_moduleId: string, label: string) => Worker;
      };
    }
  ).MonacoEnvironment = {
    getWorker(_moduleId, label) {
      if (label === "json") return new jsonWorker();
      if (label === "css" || label === "scss" || label === "less") {
        return new cssWorker();
      }
      if (label === "html" || label === "handlebars" || label === "razor") {
        return new htmlWorker();
      }
      if (label === "typescript" || label === "javascript") {
        return new tsWorker();
      }
      return new editorWorker();
    },
  };
}

loader.config({ monaco: monacoNs });

type Props = {
  tabs: EditorTab[];
  activeIndex: number;
  onActivate: (index: number) => void;
  onClose: (index: number) => void;
  onCloseOthers: (index: number) => void;
  onCloseToRight: (index: number) => void;
  onCloseAll: () => void;
  onChange: (index: number, value: string) => void;
  onSave: (index: number) => void;
  onRevealTab: (index: number) => void;
  onOpenFile?: (path: string) => void;
  settingsOpen?: boolean;
  settingsActive?: boolean;
  settingsView?: ReactNode;
  revealTarget?: EditorRevealTarget | null;
  onSettingsActivate?: () => void;
  onSettingsClose?: () => void;
};

export function EditorPane({
  tabs,
  activeIndex,
  onActivate,
  onClose,
  onCloseOthers,
  onCloseToRight,
  onCloseAll,
  onChange,
  onSave,
  onRevealTab,
  onOpenFile,
  settingsOpen = false,
  settingsActive = false,
  settingsView,
  revealTarget,
  onSettingsActivate,
  onSettingsClose,
}: Props) {
  const editorRef = useRef<Monaco.editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<any>(null);
  const searchDecorationsRef =
    useRef<Monaco.editor.IEditorDecorationsCollection | null>(null);
  const onSaveRef = useRef(onSave);
  const [editorReadySeq, setEditorReadySeq] = useState(0);
  // Per-tab toggle: `true` shows the rendered markdown preview instead of
  // the Monaco editor. Keyed by `relativePath` so switching tabs preserves
  // each file's view mode.
  const [markdownPreview, setMarkdownPreview] = useState<
    Record<string, boolean>
  >({});
  // Position of the custom right-click menu on the image viewer. `null`
  // means the menu is closed. We store viewport coordinates because the
  // menu is rendered with `position: fixed`.
  const [imageMenu, setImageMenu] = useState<{ x: number; y: number } | null>(
    null,
  );
  const [tabMenu, setTabMenu] = useState<{
    x: number;
    y: number;
    index: number;
  } | null>(null);
  const activeTab: EditorTab | undefined = settingsActive ? undefined : tabs[activeIndex];

  const [editorFontSize, setEditorFontSize] = useState<number>(() => {
    try {
      const saved = localStorage.getItem("sinew.editor-font-size");
      return saved ? parseInt(saved, 10) : 12;
    } catch {
      return 12;
    }
  });

  const [autosaveEnabled, setAutosaveEnabled] = useState<boolean>(() => {
    try {
      return localStorage.getItem("sinew.autosave") === "true";
    } catch {
      return false;
    }
  });

  const [appTheme, setAppTheme] = useState<string>(() => {
    try {
      return localStorage.getItem("sinew.theme") || "dark";
    } catch {
      return "dark";
    }
  });

  const [systemPrefersLight, setSystemPrefersLight] = useState<boolean>(() => {
    if (typeof window === "undefined") return false;
    try {
      return window.matchMedia("(prefers-color-scheme: light)").matches;
    } catch {
      return false;
    }
  });

  const getMonacoTheme = (themeName: string) => {
    if (themeName === "light") return "sinew-light";
    if (themeName === "ai") return "sinew-ai";
    if (themeName === "system") {
      return systemPrefersLight ? "sinew-light" : "sinew-cool";
    }
    return "sinew-cool";
  };

  useEffect(() => {
    if (typeof window === "undefined") return;
    try {
      const mediaQuery = window.matchMedia("(prefers-color-scheme: light)");
      const handler = (e: MediaQueryListEvent) => {
        setSystemPrefersLight(e.matches);
      };
      mediaQuery.addEventListener("change", handler);
      return () => mediaQuery.removeEventListener("change", handler);
    } catch (e) {
      console.error("Failed to setup prefers-color-scheme listener:", e);
    }
  }, []);

  useEffect(() => {
    if (monacoRef.current) {
      try {
        monacoRef.current.editor.setTheme(getMonacoTheme(appTheme));
      } catch (e) {
        console.error("Failed to update editor theme:", e);
      }
    }
  }, [appTheme, systemPrefersLight]);

  useEffect(() => {
    const handleFont = (event: Event) => {
      const size = (event as CustomEvent<number>).detail;
      setEditorFontSize(size);
    };
    const handleAutosave = (event: Event) => {
      const enabled = (event as CustomEvent<boolean>).detail;
      setAutosaveEnabled(enabled);
    };
    const handleTheme = (event: Event) => {
      const t = (event as CustomEvent<string>).detail;
      setAppTheme(t);
    };
    window.addEventListener("sinew:editor-font-size-changed", handleFont);
    window.addEventListener("sinew:autosave-changed", handleAutosave);
    window.addEventListener("sinew:theme-changed", handleTheme);
    return () => {
      window.removeEventListener("sinew:editor-font-size-changed", handleFont);
      window.removeEventListener("sinew:autosave-changed", handleAutosave);
      window.removeEventListener("sinew:theme-changed", handleTheme);
    };
  }, []);

  useEffect(() => {
    if (editorRef.current) {
      editorRef.current.updateOptions({
        fontSize: editorFontSize,
        lineHeight: editorFontSize + 6,
      });
    }
  }, [editorFontSize]);

  useEffect(() => {
    onSaveRef.current = onSave;
  }, [onSave]);

  // SOTA Auto-save debounced effect
  useEffect(() => {
    if (!autosaveEnabled || !activeTab || !activeTab.dirty || isPlanMarkdownPath(activeTab.relativePath)) return;

    const timer = setTimeout(() => {
      onSaveRef.current(activeIndex);
    }, 1500);

    return () => clearTimeout(timer);
  }, [activeTab?.buffer, activeTab?.dirty, autosaveEnabled, activeIndex]);

  // Close the image context menu whenever the user switches tabs or
  // toggles into the settings view, so it never lingers on the wrong file.
  useEffect(() => {
    setImageMenu(null);
    setTabMenu(null);
  }, [activeIndex, settingsActive]);

  const activeIsMarkdown = activeTab
    ? isMarkdownPath(activeTab.relativePath)
    : false;
  const activePreview = activeTab
    ? markdownPreview[activeTab.relativePath] ?? isPlanMarkdownPath(activeTab.relativePath)
    : false;
  // External files are tabs whose path lives outside of the active
  // workspace (typically opened from the terminal). We still render them
  // in Monaco but force the editor into read-only mode and skip every
  // save / dirty / rename plumbing in the parent.
  const isExternalTab = Boolean(activeTab?.external);
  const readOnlyEditor = isExternalTab || (activeTab ? !activeTab.doc.editable : false);
  const showTextEditor = Boolean(
    activeTab &&
      (activeTab.doc.editable || isExternalTab) &&
      activeTab.doc.content !== null &&
      !isPreviewableImagePath(activeTab.relativePath),
  );

  const toggleMarkdownPreview = useCallback(() => {
    if (!activeTab) return;
    const path = activeTab.relativePath;
    setMarkdownPreview((prev) => ({
      ...prev,
      [path]: !(prev[path] ?? isPlanMarkdownPath(path)),
    }));
  }, [activeTab]);

  const handleOpenFileLink = useCallback(
    (path: string) => {
      if (onOpenFile) onOpenFile(path);
    },
    [onOpenFile],
  );

  const handleMount: OnMount = (editor, monaco) => {
    editorRef.current = editor;
    monacoRef.current = monaco;
    setEditorReadySeq((value) => value + 1);
    
    try {
      editor.updateOptions({
        fontSize: editorFontSize,
        lineHeight: editorFontSize + 6,
      });
    } catch (e) {
      console.error("Failed to set font size on editor mount:", e);
    }
    
    // 1. sinew-cool (Default Dark)
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
        "editorCursor.foreground": "#9ca3af",
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
      }
    });

    // 2. sinew-light (Day Theme)
    monaco.editor.defineTheme("sinew-light", {
      base: "vs",
      inherit: true,
      rules: [
        { token: "comment", foreground: "9ca3af" },
        { token: "keyword", foreground: "2563eb" },
        { token: "string", foreground: "16a34a" },
        { token: "number", foreground: "ea580c" },
        { token: "type", foreground: "b45309" },
        { token: "function", foreground: "7c3aed" },
        { token: "variable", foreground: "111827" },
        { token: "constant", foreground: "ea580c" },
        { token: "regexp", foreground: "16a34a" },
        { token: "tag", foreground: "dc2626" },
        { token: "attribute.name", foreground: "7c3aed" },
      ],
      colors: {
        "editor.background": "#ffffff",
        "editor.foreground": "#111827",
        "editor.lineHighlightBackground": "#f3f4f6",
        "editorLineNumber.foreground": "#9ca3af",
        "editorLineNumber.activeForeground": "#1b2a47",
        "editorCursor.foreground": "#4b5563",
        "editor.selectionBackground": "#bfdbfe",
        "editor.inactiveSelectionBackground": "#e5e7eb",
        "editorIndentGuide.background1": "#f3f4f6",
        "editorIndentGuide.activeBackground1": "#e5e7eb",
        "editorGutter.background": "#ffffff",
        "editorWidget.background": "#f9fafb",
        "editorWidget.border": "#e5e7eb",
        "editorHoverWidget.background": "#f9fafb",
        "editorHoverWidget.border": "#e5e7eb",
        "editorSuggestWidget.background": "#f9fafb",
        "editorSuggestWidget.border": "#e5e7eb",
      }
    });

    // 3. sinew-ai (✨ AI Modern Glass Theme)
    monaco.editor.defineTheme("sinew-ai", {
      base: "vs-dark",
      inherit: true,
      rules: [
        { token: "comment", foreground: "636f88" },
        { token: "keyword", foreground: "ab7cf6" }, // Elegant Gemini purple
        { token: "string", foreground: "86efac" }, // Calm green
        { token: "number", foreground: "9fc2ff" }, // Soft cool gray
        { token: "type", foreground: "c4b5fd" },
        { token: "function", foreground: "ab7cf6" },
        { token: "variable", foreground: "e8e9ec" },
        { token: "constant", foreground: "9fc2ff" },
        { token: "regexp", foreground: "86efac" },
        { token: "tag", foreground: "ab7cf6" },
        { token: "attribute.name", foreground: "9fc2ff" },
      ],
      colors: {
        "editor.background": "#0b0b0d", // Dark slate background matching Night base
        "editor.foreground": "#e8e9ec",
        "editor.lineHighlightBackground": "#141518",
        "editorLineNumber.foreground": "#52555c",
        "editorLineNumber.activeForeground": "#9b51e0",
        "editorCursor.foreground": "#9b51e0",
        "editor.selectionBackground": "#9b51e033",
        "editor.inactiveSelectionBackground": "#9b51e015",
        "editorIndentGuide.background1": "#181a1f",
        "editorIndentGuide.activeBackground1": "#23252b",
        "editorGutter.background": "#0b0b0d",
        "editorWidget.background": "#0f1013",
        "editorWidget.border": "#9b51e055",
        "editorHoverWidget.background": "#0f1013",
        "editorHoverWidget.border": "#9b51e055",
        "editorSuggestWidget.background": "#0f1013",
        "editorSuggestWidget.border": "#9b51e055",
      }
    });
    try {
      monaco.editor.setTheme(getMonacoTheme(appTheme));
    } catch (e) {
      console.error("Failed to apply initial theme in handleMount:", e);
    }

    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
      const path = currentPathRef.current;
      if (!path) return;
      const idx = tabsRef.current.findIndex((t) => t.relativePath === path);
      if (idx >= 0) onSaveRef.current(idx);
    });

    window.requestAnimationFrame(() => {
      editor.layout();
    });
  };

  const currentPathRef = useRef<string | null>(null);
  const tabsRef = useRef<EditorTab[]>(tabs);
  tabsRef.current = tabs;
  onSaveRef.current = onSave;
  currentPathRef.current = activeTab?.relativePath ?? null;

  useEffect(() => {
    if (!showTextEditor || activePreview) return;
    const editor = editorRef.current;
    if (!editor?.getDomNode()?.isConnected) return;
    const frame = window.requestAnimationFrame(() => {
      editor.layout();
    });
    return () => window.cancelAnimationFrame(frame);
  }, [activePreview, activeTab?.relativePath, showTextEditor]);

  useEffect(() => {
    if (!revealTarget || activeTab?.relativePath !== revealTarget.relativePath) {
      return;
    }
    if (!activeIsMarkdown || !activePreview) return;
    setMarkdownPreview((prev) => ({
      ...prev,
      [revealTarget.relativePath]: false,
    }));
  }, [
    activeIsMarkdown,
    activePreview,
    activeTab?.relativePath,
    revealTarget?.id,
    revealTarget?.relativePath,
  ]);

  useEffect(() => {
    const editor = editorRef.current;
    if (!editor || !activeTab || !revealTarget) {
      searchDecorationsRef.current?.clear();
      return;
    }
    if (activeTab.relativePath !== revealTarget.relativePath) {
      searchDecorationsRef.current?.clear();
      return;
    }
    if (!showTextEditor || activePreview) return;

    const model = editor.getModel();
    if (!model) return;

    const lineNumber = Math.max(
      1,
      Math.min(revealTarget.lineNumber, model.getLineCount()),
    );
    const lineLength = model.getLineLength(lineNumber);
    const startColumn = Math.max(
      1,
      Math.min(revealTarget.columnStart, lineLength + 1),
    );
    const endColumn = Math.max(
      startColumn + 1,
      Math.min(revealTarget.columnEnd, lineLength + 1),
    );
    const range = new monacoNs.Range(
      lineNumber,
      startColumn,
      lineNumber,
      endColumn,
    );

    searchDecorationsRef.current?.clear();
    searchDecorationsRef.current = editor.createDecorationsCollection([
      {
        range,
        options: {
          inlineClassName: "editor-search-hit",
          className: "editor-search-line-hit",
          overviewRuler: {
            color: "rgba(196, 181, 253, 0.8)",
            position: monacoNs.editor.OverviewRulerLane.Center,
          },
        },
      },
    ]);

    const reveal = () => {
      if (editorRef.current !== editor) return;
      if (currentPathRef.current !== revealTarget.relativePath) return;
      editor.layout();
      editor.setSelection(range);
      editor.revealRangeInCenter(range, monacoNs.editor.ScrollType.Smooth);
      editor.focus();
    };

    let nextFrame = 0;
    const frame = window.requestAnimationFrame(() => {
      reveal();
      nextFrame = window.requestAnimationFrame(reveal);
    });

    return () => {
      window.cancelAnimationFrame(frame);
      if (nextFrame) window.cancelAnimationFrame(nextFrame);
    };
  }, [
    activePreview,
    activeTab?.relativePath,
    editorReadySeq,
    revealTarget,
    revealTarget?.id,
    showTextEditor,
  ]);

  useEffect(() => {
    if (!showTextEditor) return;
    let disposed = false;
    let timer: number | undefined;

    const pushDiagnostics = () => {
      if (disposed) return;
      const diagnostics = collectMonacoDiagnostics(monacoNs);
      void api.pushEditorDiagnostics(diagnostics).catch(() => {});
    };

    const schedulePush = () => {
      if (timer) window.clearTimeout(timer);
      timer = window.setTimeout(pushDiagnostics, 400);
    };

    schedulePush();
    const subscription = monacoNs.editor.onDidChangeMarkers(() => {
      schedulePush();
    });

    return () => {
      disposed = true;
      if (timer) window.clearTimeout(timer);
      subscription.dispose();
    };
  }, [showTextEditor, tabs, activeTab?.relativePath, editorReadySeq, activeTab?.buffer]);

  const onEditorChange = useCallback(
    (value: string | undefined) => {
      if (!activeTab) return;
      onChange(activeIndex, value ?? "");
    },
    [activeTab, activeIndex, onChange],
  );

  return (
    <div className="editor-col">
      <div className="tabs">
        {tabs.map((tab, index) => (
          <div
            key={tab.relativePath}
            className="tab"
            data-active={!settingsActive && index === activeIndex ? "true" : "false"}
            role="tab"
            aria-selected={!settingsActive && index === activeIndex}
            tabIndex={!settingsActive && index === activeIndex ? 0 : -1}
            onClick={() => onActivate(index)}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onActivate(index);
                return;
              }
              if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
                event.preventDefault();
                const rect = event.currentTarget.getBoundingClientRect();
                onActivate(index);
                setTabMenu({ x: rect.left + 16, y: rect.bottom - 2, index });
              }
            }}
            onContextMenu={(event) => {
              event.preventDefault();
              onActivate(index);
              setTabMenu({ x: event.clientX, y: event.clientY, index });
            }}
            title={tab.relativePath}
          >
            <span className="tab__icon">
              <EditorFileIcon name={tab.doc.name} />
            </span>
            <span className="tab__name">{tab.doc.name}</span>
            {tab.dirty && <span className="tab__dirty" />}
            <button
              className="tab__close"
              onClick={(event) => {
                event.stopPropagation();
                onClose(index);
              }}
              title="Close tab"
            >
              <Icon icon="solar:close-square-linear" width={13} height={13} />
            </button>
          </div>
        ))}
        {settingsOpen && (
          <div
            className="tab"
            data-active={settingsActive ? "true" : "false"}
            role="tab"
            aria-selected={settingsActive}
            tabIndex={settingsActive ? 0 : -1}
            onClick={onSettingsActivate}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onSettingsActivate?.();
              }
            }}
            title="Settings"
          >
            <span className="tab__icon">
              <Icon icon="solar:settings-linear" width={14} height={14} />
            </span>
            <span className="tab__name">Settings</span>
            <button
              className="tab__close"
              onClick={(event) => {
                event.stopPropagation();
                onSettingsClose?.();
              }}
              title="Close tab"
            >
              <Icon icon="solar:close-square-linear" width={13} height={13} />
            </button>
          </div>
        )}
        <div className="tabs__spacer" />
        {activeIsMarkdown && (
          <button
            type="button"
            className="tab-action"
            data-active={activePreview ? "true" : "false"}
            onClick={toggleMarkdownPreview}
            title={
              activePreview
                ? "Show raw markdown source"
                : "Show rendered markdown preview"
            }
          >
            <Icon
              icon={
                activePreview
                  ? "solar:code-square-linear"
                  : "solar:eye-linear"
              }
              width={13}
              height={13}
            />
            <span>{activePreview ? "Source" : "Preview"}</span>
          </button>
        )}
      </div>

      {tabMenu && tabs[tabMenu.index] && (
        <EditorTabContextMenu
          x={tabMenu.x}
          y={tabMenu.y}
          tab={tabs[tabMenu.index]}
          tabCount={tabs.length}
          index={tabMenu.index}
          settingsOpen={settingsOpen}
          onClose={() => setTabMenu(null)}
          onCloseTab={() => onClose(tabMenu.index)}
          onCloseOthers={() => onCloseOthers(tabMenu.index)}
          onCloseToRight={() => onCloseToRight(tabMenu.index)}
          onCloseAll={onCloseAll}
          onReveal={() => onRevealTab(tabMenu.index)}
        />
      )}

      <div className="editor-host">
        {settingsOpen && (
          <div
            className="editor-settings-layer"
            data-active={settingsActive ? "true" : "false"}
          >
            {settingsView}
          </div>
        )}
        {!settingsActive &&
          (!activeTab ? (
            <div className="editor-empty">
              <span className="editor-empty__mark">
                <Icon icon="solar:document-text-linear" width={18} height={18} />
              </span>
              <span className="editor-empty__title">Nothing open</span>
              <span className="editor-empty__sub">
                Click a file in the sidebar to get started
              </span>
            </div>
          ) : isPreviewableImagePath(activeTab.relativePath) ? (
            <div
              className="editor-image-preview"
              onContextMenu={(event) => {
                event.preventDefault();
                setImageMenu({ x: event.clientX, y: event.clientY });
              }}
            >
              <img
                src={imagePreviewSrc(activeTab.doc)}
                alt={activeTab.doc.name}
                draggable={false}
              />
              <div className="editor-image-preview__meta">
                <span>{activeTab.doc.name}</span>
                <span>{formatBytes(activeTab.doc.size)}</span>
              </div>
              {imageMenu && (
                <ImageContextMenu
                  x={imageMenu.x}
                  y={imageMenu.y}
                  imageSrc={imagePreviewSrc(activeTab.doc)}
                  absolutePath={activeTab.doc.absolutePath}
                  fileName={activeTab.doc.name}
                  onClose={() => setImageMenu(null)}
                />
              )}
            </div>
          ) : showTextEditor ? (
            <div
              className="editor-text-stack"
              data-preview={activeIsMarkdown && activePreview ? "true" : "false"}
            >
              <div className="editor-monaco-layer">
                <Editor
                  key={activeTab.relativePath}
                  height="100%"
                  theme={getMonacoTheme(appTheme)}
                  path={activeTab.relativePath}
                  value={activeTab.buffer}
                  language={languageForPath(activeTab.relativePath)}
                  onMount={handleMount}
                  onChange={onEditorChange}
                  options={{
                    fontFamily:
                      '"Geist Mono", ui-monospace, "SF Mono", Menlo, monospace',
                    fontSize: editorFontSize,
                    lineHeight: editorFontSize + 6,
                    minimap: { enabled: false },
                    scrollBeyondLastLine: false,
                    smoothScrolling: true,
                    renderLineHighlight: "line",
                    padding: { top: 14, bottom: 14 },
                    tabSize: 2,
                    wordWrap: "off",
                    scrollbar: {
                      verticalScrollbarSize: 10,
                      horizontalScrollbarSize: 10,
                    },
                    automaticLayout: true,
                    readOnly: readOnlyEditor,
                  }}
                />
              </div>
              {activeIsMarkdown && activePreview && (
                <div className="editor-md-preview">
                  <Markdown text={activeTab.buffer} onOpenFile={handleOpenFileLink} />
                </div>
              )}
            </div>
          ) : !activeTab.doc.editable ? (
            <div className="editor-noneditable">
              <div>This file can&rsquo;t be edited here.</div>
              <code>{activeTab.doc.reason ?? "binary or too large"}</code>
              <small style={{ color: "var(--text-3)" }}>
                {activeTab.doc.relativePath} &middot;{" "}
                {formatBytes(activeTab.doc.size)}
              </small>
            </div>
          ) : null)}
      </div>
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function isPreviewableImagePath(relativePath: string): boolean {
  return /\.(png|jpe?g|gif|webp|svg|bmp|avif|heic|heif)$/i.test(relativePath);
}

function imagePreviewSrc(doc: EditorTab["doc"]): string {
  if (doc.imageMediaType && doc.imageData) {
    return `data:${doc.imageMediaType};base64,${doc.imageData}`;
  }
  return convertFileSrc(doc.absolutePath);
}

function EditorFileIcon({ name }: { name: string }) {
  if (isDesignMarkdown(name)) {
    return <DesignMarkdownIcon />;
  }
  if (isAgentsMarkdown(name)) {
    return <AgentsMarkdownIcon />;
  }
  if (isClaudeMarkdown(name)) {
    return <ClaudeMarkdownIcon />;
  }
  return <Icon icon={fileIcon(name)} width={14} height={14} />;
}

function DesignMarkdownIcon() {
  return (
    <svg
      className="design-file-icon"
      width="14"
      height="14"
      viewBox="0 0 16 16"
      aria-hidden="true"
      focusable="false"
    >
      <rect x="1" y="1" width="14" height="14" rx="3" fill="#18181b" />
      <rect x="2.2" y="2.2" width="11.6" height="11.6" rx="2.2" fill="#fff7ed" />
      <path
        d="M4.1 11.7 5 8.5l4.9-4.9a1.2 1.2 0 0 1 1.7 0l.8.8a1.2 1.2 0 0 1 0 1.7L7.5 11l-3.4.7Z"
        fill="#4b5563"
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
  // matching the dedicated icon shown in the left sidebar tree.
  return (
    <svg
      className="agents-file-icon"
      width="14"
      height="14"
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
  // Anthropic-style terracotta sunburst used for CLAUDE.md files,
  // mirroring the dedicated icon shown in the left sidebar tree.
  return (
    <svg
      className="claude-file-icon"
      width="14"
      height="14"
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

function isMarkdownPath(relativePath: string): boolean {
  return /\.(md|markdown|mdx)$/i.test(relativePath);
}

function isPlanMarkdownPath(relativePath: string): boolean {
  return /^\.sinew\/plans\/.+\.md$/i.test(relativePath);
}

function collectMonacoDiagnostics(monaco: typeof monacoNs): EditorDiagnosticInput[] {
  const diagnostics: EditorDiagnosticInput[] = [];
  for (const model of monaco.editor.getModels()) {
    const path = relativePathFromModelUri(model.uri);
    if (!path) continue;
    for (const marker of monaco.editor.getModelMarkers({ resource: model.uri })) {
      diagnostics.push({
        path,
        line: marker.startLineNumber,
        column: marker.startColumn,
        endLine: marker.endLineNumber,
        endColumn: marker.endColumn,
        severity: markerSeverityName(marker.severity),
        message: marker.message,
        source: marker.source ?? languageForPath(path),
      });
    }
  }
  return diagnostics;
}

function relativePathFromModelUri(uri: Monaco.Uri): string | null {
  const path = uri.path.replace(/^\/+/, "");
  if (!path || path.startsWith("inmemory:")) return null;
  return path.replace(/\\/g, "/");
}

function markerSeverityName(severity: Monaco.MarkerSeverity): string {
  if (severity === monacoNs.MarkerSeverity.Error) return "error";
  if (severity === monacoNs.MarkerSeverity.Warning) return "warning";
  if (severity === monacoNs.MarkerSeverity.Info) return "info";
  return "hint";
}

const TAB_MENU_WIDTH = 236;
const TAB_MENU_HEIGHT = 250;

function EditorTabContextMenu({
  x,
  y,
  tab,
  tabCount,
  index,
  settingsOpen,
  onClose,
  onCloseTab,
  onCloseOthers,
  onCloseToRight,
  onCloseAll,
  onReveal,
}: {
  x: number;
  y: number;
  tab: EditorTab;
  tabCount: number;
  index: number;
  settingsOpen: boolean;
  onClose: () => void;
  onCloseTab: () => void;
  onCloseOthers: () => void;
  onCloseToRight: () => void;
  onCloseAll: () => void;
  onReveal: () => void;
}) {
  useEffect(() => {
    const close = () => onClose();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("pointerdown", close);
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("resize", close);
    document.addEventListener("scroll", close, true);
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("resize", close);
      document.removeEventListener("scroll", close, true);
    };
  }, [onClose]);

  const clampedX =
    typeof window === "undefined"
      ? x
      : Math.max(8, Math.min(x, window.innerWidth - TAB_MENU_WIDTH - 8));
  const clampedY =
    typeof window === "undefined"
      ? y
      : Math.max(8, Math.min(y, window.innerHeight - TAB_MENU_HEIGHT - 8));

  const runAction = (action: () => void | Promise<void>) => () => {
    onClose();
    void Promise.resolve(action()).catch((err) =>
      console.error("[tab-menu] action failed", err),
    );
  };

  const copyFullPath = runAction(() => copyText(tab.doc.absolutePath));
  const copyRelativePath = runAction(() => copyText(tab.relativePath));

  const isFr = getAppLocale() === "fr";

  return (
    <div
      className="tree-menu tab-menu"
      role="menu"
      style={{ left: clampedX, top: clampedY }}
      onPointerDown={(event) => event.stopPropagation()}
      onContextMenu={(event) => event.preventDefault()}
    >
      <TabMenuItem
        icon="solar:close-square-linear"
        label={isFr ? "Fermer l'onglet" : "Close Tab"}
        shortcut="Ctrl+F4"
        onClick={runAction(onCloseTab)}
      />
      <TabMenuItem
        icon="solar:close-square-linear"
        label={isFr ? "Fermer les autres" : "Close Others"}
        disabled={tabCount <= 1}
        onClick={runAction(onCloseOthers)}
      />
      <TabMenuItem
        icon="solar:alt-arrow-right-bold"
        label={isFr ? "Fermer les onglets à droite" : "Close to the Right"}
        disabled={index >= tabCount - 1 && !settingsOpen}
        onClick={runAction(onCloseToRight)}
      />
      <TabMenuItem
        icon="solar:layers-minimalistic-linear"
        label={isFr ? "Fermer tous les onglets" : "Close All"}
        onClick={runAction(onCloseAll)}
      />
      <div className="tree-menu__separator" role="separator" />
      <TabMenuItem
        icon="solar:copy-linear"
        label={isFr ? "Copier le chemin absolu" : "Copy Full Path"}
        onClick={copyFullPath}
      />
      <TabMenuItem
        icon="solar:copy-linear"
        label={isFr ? "Copier le chemin relatif" : "Copy Relative Path"}
        disabled={Boolean(tab.external)}
        onClick={copyRelativePath}
      />
      <div className="tree-menu__separator" role="separator" />
      <TabMenuItem
        icon="solar:folder-open-linear"
        label={revealLabel(isFr)}
        onClick={runAction(onReveal)}
      />
    </div>
  );
}

function TabMenuItem({
  icon,
  label,
  shortcut,
  disabled = false,
  onClick,
}: {
  icon: string;
  label: string;
  shortcut?: string;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className="tree-menu__item tab-menu__item"
      data-danger="false"
      role="menuitem"
      disabled={disabled}
      onClick={onClick}
    >
      <Icon icon={icon} width={14} height={14} />
      <span>{label}</span>
      {shortcut && <kbd className="tab-menu__shortcut">{shortcut}</kbd>}
    </button>
  );
}

async function copyText(text: string): Promise<void> {
  await navigator.clipboard.writeText(text);
}

function revealLabel(isFr: boolean): string {
  const platform =
    typeof navigator !== "undefined" ? navigator.platform.toLowerCase() : "";
  if (platform.includes("mac")) return isFr ? "Afficher dans le Finder" : "Reveal in Finder";
  if (platform.includes("win")) return isFr ? "Afficher dans l'Explorateur" : "Reveal in File Explorer";
  return isFr ? "Afficher dans le gestionnaire de fichiers" : "Reveal in File Manager";
}
