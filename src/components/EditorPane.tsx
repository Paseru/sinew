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
import { attachMonacoTheme, monacoThemeForDocument } from "../lib/monacoThemes";
import { Markdown } from "./chat/Markdown";
import type { EditorRevealTarget, EditorTab } from "../types";
import { ImageContextMenu } from "./ImageContextMenu";

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
  onChange: (index: number, value: string) => void;
  onSave: (index: number) => void;
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
  onChange,
  onSave,
  onOpenFile,
  settingsOpen = false,
  settingsActive = false,
  settingsView,
  revealTarget,
  onSettingsActivate,
  onSettingsClose,
}: Props) {
  const editorRef = useRef<Monaco.editor.IStandaloneCodeEditor | null>(null);
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
  const activeTab: EditorTab | undefined = settingsActive ? undefined : tabs[activeIndex];
  // Close the image context menu whenever the user switches tabs or
  // toggles into the settings view, so it never lingers on the wrong file.
  useEffect(() => {
    setImageMenu(null);
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
    setEditorReadySeq((value) => value + 1);
    attachMonacoTheme(monaco);

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
            onClick={() => onActivate(index)}
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
              <Icon icon="solar:close-circle-linear" width={13} height={13} />
            </button>
          </div>
        ))}
        {settingsOpen && (
          <div
            className="tab"
            data-active={settingsActive ? "true" : "false"}
            onClick={onSettingsActivate}
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
              <Icon icon="solar:close-circle-linear" width={13} height={13} />
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
                  path={activeTab.relativePath}
                  value={activeTab.buffer}
                  language={languageForPath(activeTab.relativePath)}
                  onMount={handleMount}
                  onChange={onEditorChange}
                  options={{
                    fontFamily:
                      '"Geist Mono", ui-monospace, "SF Mono", Menlo, monospace',
                    fontSize: 12,
                    lineHeight: 18,
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
  return /^\.claakecode\/plans\/.+\.md$/i.test(relativePath);
}
