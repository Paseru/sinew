import { useCallback, useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Icon } from "@iconify/react";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { WebglAddon } from "@xterm/addon-webgl";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { ClipboardAddon } from "@xterm/addon-clipboard";
import {
  Terminal,
  type IBufferRange,
  type ILink,
  type ILinkProvider,
} from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { api } from "../lib/ipc";
import {
  ACCENT_CHANGED_EVENT,
  getTerminalCopyOnSelect,
  getTerminalCursorBlink,
  getTerminalCursorStyle,
  getTerminalFontSize,
  getTerminalShell,
  TERMINAL_CURSOR_CHANGED_EVENT,
  TERMINAL_FONT_CHANGED_EVENT,
  THEME_CHANGED_EVENT,
  type TerminalCursorState,
} from "../lib/appearance";
import type { TerminalDataPayload, TerminalExitPayload } from "../types";

type TerminalStatus = "idle" | "starting" | "running" | "exited" | "error";

type TerminalSession = {
  id: string;
  index: number;
  title: string;
  status: TerminalStatus;
};

type Props = {
  active: boolean;
  fullHeight: boolean;
  workspacePath: string;
  onClose: () => void;
  onCloseLastSession: () => void;
  onToggleFullHeight: () => void;
  /**
   * Invoked when the user cmd/ctrl+clicks a file path detected in the
   * terminal buffer. The parent decides how to open it (workspace tab,
   * external read-only tab, file-tree reveal, ...).
   */
  onOpenTerminalPath?: (rawPath: string) => void;
};

type Disposable = {
  dispose: () => void;
};

export function TerminalPanel({
  active,
  fullHeight,
  workspacePath,
  onClose,
  onCloseLastSession,
  onToggleFullHeight,
  onOpenTerminalPath,
}: Props) {
  const [sessions, setSessions] = useState<TerminalSession[]>(() => [
    createSession(1),
  ]);
  const [activeId, setActiveId] = useState("terminal-1");
  const nextSessionIndex = useRef(2);

  const activeSession =
    sessions.find((session) => session.id === activeId) ?? sessions[0];

  const patchSession = useCallback(
    (id: string, patch: Partial<TerminalSession>) => {
      setSessions((prev) =>
        prev.map((session) =>
          session.id === id ? { ...session, ...patch } : session,
        ),
      );
    },
    [],
  );

  const addSession = useCallback(() => {
    const session = createSession(nextSessionIndex.current++);
    setSessions((prev) => [...prev, session]);
    setActiveId(session.id);
  }, []);

  const closeSession = useCallback(
    (id: string) => {
      if (sessions.length <= 1) {
        onCloseLastSession();
        return;
      }
      const index = sessions.findIndex((session) => session.id === id);
      const next = sessions.filter((session) => session.id !== id);
      setSessions(next);
      setActiveId((current) => {
        if (current !== id) return current;
        return next[Math.max(0, Math.min(index, next.length - 1))].id;
      });
    },
    [onCloseLastSession, sessions],
  );

  const closeAllSessions = useCallback(() => {
    onCloseLastSession();
  }, [onCloseLastSession]);

  if (!activeSession) return null;

  return (
    <section className="terminal-panel">
      <div className="terminal-tabs">
        <div className="terminal-tabs__list">
          {sessions.map((session) => (
            <button
              key={session.id}
              type="button"
              className="terminal-tab"
              data-active={session.id === activeId ? "true" : "false"}
              onClick={() => setActiveId(session.id)}
              title={session.title}
            >
              <Icon icon="solar:command-linear" width={13} height={13} />
              <span>{session.title}</span>
              <span
                className="terminal-tab__status"
                data-status={session.status}
              />
              <span
                className="terminal-tab__close"
                role="button"
                tabIndex={-1}
                title="Close terminal"
                onClick={(event) => {
                  event.stopPropagation();
                  closeSession(session.id);
                }}
              >
                <Icon icon="solar:close-circle-linear" width={13} height={13} />
              </span>
            </button>
          ))}
        </div>
        <div className="terminal-tabs__actions">
          <button
            type="button"
            className="terminal-action"
            onClick={addSession}
            title="New terminal"
          >
            <Icon icon="solar:add-circle-linear" width={14} height={14} />
          </button>
          <button
            type="button"
            className="terminal-action"
            onClick={closeAllSessions}
            title="Close all terminals"
            aria-label="Close all terminals"
          >
            <Icon icon="solar:trash-bin-trash-linear" width={14} height={14} />
          </button>
          <button
            type="button"
            className="terminal-action"
            data-active={fullHeight ? "true" : "false"}
            onClick={onToggleFullHeight}
            title={fullHeight ? "Restore terminal height" : "Full height"}
          >
            <Icon
              icon={
                fullHeight
                  ? "solar:quit-full-screen-square-linear"
                  : "solar:full-screen-square-linear"
              }
              width={14}
              height={14}
            />
          </button>
          <button
            type="button"
            className="terminal-action"
            onClick={onClose}
            title="Hide terminal"
          >
            <Icon
              icon="solar:square-alt-arrow-down-linear"
              width={14}
              height={14}
            />
          </button>
        </div>
      </div>

      <div className="terminal-views">
        {sessions.map((session) => (
          <TerminalSurface
            key={session.id}
            session={session}
            active={active && session.id === activeId}
            workspacePath={workspacePath}
            onStatus={(status) => patchSession(session.id, { status })}
            onTitle={(title) => patchSession(session.id, { title })}
            onOpenTerminalPath={onOpenTerminalPath}
          />
        ))}
      </div>
    </section>
  );
}

function TerminalSurface({
  session,
  active,
  workspacePath,
  onStatus,
  onTitle,
  onOpenTerminalPath,
}: {
  session: TerminalSession;
  active: boolean;
  workspacePath: string;
  onStatus: (status: TerminalStatus) => void;
  onTitle: (title: string) => void;
  onOpenTerminalPath?: (rawPath: string) => void;
}) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const webglRef = useRef<WebglAddon | null>(null);
  const tokenRef = useRef<string | null>(null);
  const unlistenersRef = useRef<UnlistenFn[]>([]);
  const disposablesRef = useRef<Disposable[]>([]);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const disposedRef = useRef(false);
  // Latest path-click handler kept in a ref so the link provider sees
  // updates without having to re-register every time the parent re-renders.
  const onOpenTerminalPathRef = useRef(onOpenTerminalPath);
  onOpenTerminalPathRef.current = onOpenTerminalPath;

  const fitTerminal = useCallback(() => {
    const terminal = terminalRef.current;
    const fit = fitRef.current;
    const container = containerRef.current;
    const token = tokenRef.current;
    if (!terminal || !fit || !container) return;

    const rect = container.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return;

    try {
      fit.fit();
    } catch {
      // xterm cannot measure while the panel is hidden.
      return;
    }

    if (token) {
      void api.resizeTerminal(
        session.id,
        token,
        terminal.cols,
        terminal.rows,
        Math.max(0, Math.round(rect.width)),
        Math.max(0, Math.round(rect.height)),
      );
    }
  }, [session.id]);

  const startTerminal = useCallback(() => {
    const container = containerRef.current;
    if (!container || terminalRef.current) return;

    disposedRef.current = false;
    const token = createTerminalToken(session.id);
    tokenRef.current = token;
    onStatus("starting");

    const styles = getComputedStyle(document.documentElement);
    const cssVar = (name: string, fallback: string) =>
      styles.getPropertyValue(name).trim() || fallback;
    const fontFamily = cssVar(
      "--font-mono",
      '"Geist Mono", ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace',
    );
    const fontSize = getTerminalFontSize();

    const terminal = new Terminal({
      // Required for `unicode11` and a few other addons we register below.
      allowProposedApi: true,
      cursorBlink: getTerminalCursorBlink(),
      cursorStyle: getTerminalCursorStyle(),
      fontFamily,
      fontSize,
      letterSpacing: 0,
      // VS Code-style line height: keeps cells flush so WebGL redraws
      // don't leave half-pixel artefacts when TUIs repaint full-screen.
      lineHeight: 1.0,
      macOptionIsMeta: true,
      scrollback: 50_000,
      // Smooth pasted text in apps that don't enable bracketed paste.
      convertEol: false,
      theme: terminalTheme(),
    });
    const fit = new FitAddon();
    const webLinks = new WebLinksAddon((event, uri) => {
      if (!event.metaKey && !event.ctrlKey) return;
      event.preventDefault();
      event.stopPropagation();
      void api.openExternalUrl(uri);
    });
    const unicode11 = new Unicode11Addon();
    const clipboard = new ClipboardAddon();

    terminal.loadAddon(fit);
    terminal.loadAddon(webLinks);
    terminal.loadAddon(unicode11);
    terminal.loadAddon(clipboard);
    try {
      terminal.unicode.activeVersion = "11";
    } catch {
      // Older xterm versions or proposed API disabled: silently ignore.
    }
    terminal.open(container);

    // GPU renderer with graceful fallback to the built-in DOM renderer.
    // We try WebGL first (closest to VS Code), and if the context is
    // ever lost or the addon refuses to load we just dispose it -- xterm
    // automatically falls back to the DOM renderer.
    const tryEnableWebgl = () => {
      if (webglRef.current) return;
      try {
        const addon = new WebglAddon();
        addon.onContextLoss(() => {
          // Lost the GPU context (suspend / driver crash / tab background):
          // dispose the addon so xterm falls back to the DOM renderer.
          addon.dispose();
          webglRef.current = null;
        });
        terminal.loadAddon(addon);
        webglRef.current = addon;
      } catch (err) {
        // WebGL is unavailable or initialisation failed: stay on the
        // DOM renderer, which is slower but always works.
        console.warn("WebGL renderer unavailable, falling back to DOM", err);
      }
    };
    tryEnableWebgl();

    // Register the path link provider *after* the addons are loaded.
    // The provider opens files in the editor on cmd/ctrl+click; plain
    // hover decoration is handled by xterm.js for free.
    const linkProvider = createPathLinkProvider(terminal, () =>
      onOpenTerminalPathRef.current,
    );
    const linkProviderDisposable = terminal.registerLinkProvider(linkProvider);

    terminalRef.current = terminal;
    fitRef.current = fit;

    disposablesRef.current = [
      linkProviderDisposable,
      terminal.onSelectionChange(() => {
        if (!getTerminalCopyOnSelect()) return;
        const text = terminal.getSelection();
        if (!text) return;
        void navigator.clipboard.writeText(text).catch((err) =>
          console.error("[terminal] copy-on-select failed", err),
        );
      }),
      terminal.onData((data) => {
        const currentToken = tokenRef.current;
        if (!currentToken) return;
        void api.writeTerminal(session.id, currentToken, data);
      }),
      terminal.onResize(({ cols, rows }) => {
        const currentToken = tokenRef.current;
        if (!currentToken) return;
        const rect = containerRef.current?.getBoundingClientRect();
        void api.resizeTerminal(
          session.id,
          currentToken,
          cols,
          rows,
          Math.max(0, Math.round(rect?.width ?? 0)),
          Math.max(0, Math.round(rect?.height ?? 0)),
        );
      }),
      terminal.onTitleChange((title) => {
        const trimmed = title.trim();
        if (trimmed) onTitle(trimmed);
      }),
    ];

    resizeObserverRef.current = new ResizeObserver(() => {
      if (active) fitTerminal();
    });
    resizeObserverRef.current.observe(container);

    const dataListener = listen<TerminalDataPayload>(
      "terminal-data",
      (event) => {
        const payload = event.payload;
        if (payload.sessionId !== session.id || payload.token !== token) return;
        terminal.write(payload.data);
      },
    );

    const exitListener = listen<TerminalExitPayload>(
      "terminal-exit",
      (event) => {
        const payload = event.payload;
        if (payload.sessionId !== session.id || payload.token !== token) return;
        tokenRef.current = null;
        onStatus(payload.exitCode === 0 ? "exited" : "error");
        terminal.write("\r\n\x1b[2m[process exited]\x1b[0m\r\n");
      },
    );

    void Promise.all([dataListener, exitListener]).then((unlisteners) => {
      for (const unlisten of unlisteners) {
        if (disposedRef.current) {
          unlisten();
        } else {
          unlistenersRef.current.push(unlisten);
        }
      }

      requestAnimationFrame(() => {
        if (disposedRef.current) return;
        try {
          fit.fit();
        } catch {
          // panel may still be measuring; spawn with current cols/rows anyway.
        }
        const rect = container.getBoundingClientRect();
        void api
          .spawnTerminal(
            workspacePath,
            session.id,
            token,
            Math.max(terminal.cols, 20),
            Math.max(terminal.rows, 4),
            Math.max(0, Math.round(rect.width)),
            Math.max(0, Math.round(rect.height)),
            getTerminalShell() || undefined,
          )
          .then(() => {
            if (disposedRef.current || tokenRef.current !== token) return;
            onStatus("running");
            if (active) terminal.focus();
          })
          .catch((err) => {
            if (disposedRef.current) return;
            tokenRef.current = null;
            onStatus("error");
            terminal.write(`\r\n\x1b[31m${stripAnsi(String(err))}\x1b[0m\r\n`);
          });
      });
    }).catch((err) => {
      if (disposedRef.current) return;
      tokenRef.current = null;
      onStatus("error");
      terminal.write(`\r\n\x1b[31m${stripAnsi(String(err))}\x1b[0m\r\n`);
    });
  }, [active, fitTerminal, onStatus, onTitle, session.id, workspacePath]);

  useEffect(() => {
    if (active) startTerminal();
  }, [active, startTerminal]);

  useEffect(() => {
    if (!active) return;
    const id = requestAnimationFrame(() => {
      fitTerminal();
      terminalRef.current?.focus();
    });
    return () => cancelAnimationFrame(id);
  }, [active, fitTerminal]);

  useEffect(() => {
    const onFontSize = (event: Event) => {
      const terminal = terminalRef.current;
      if (!terminal) return;
      const detail = (event as CustomEvent<number>).detail;
      if (typeof detail === "number") {
        terminal.options.fontSize = detail;
      }
      requestAnimationFrame(() => fitTerminal());
    };
    const onCursor = (event: Event) => {
      const terminal = terminalRef.current;
      if (!terminal) return;
      const detail = (event as CustomEvent<TerminalCursorState>).detail;
      if (!detail) return;
      terminal.options.cursorStyle = detail.style;
      terminal.options.cursorBlink = detail.blink;
    };
    const onTheme = () => {
      const terminal = terminalRef.current;
      if (!terminal) return;
      // terminalTheme() reads the new CSS-var values on the next frame.
      requestAnimationFrame(() => {
        terminal.options.theme = terminalTheme();
      });
    };
    window.addEventListener(TERMINAL_FONT_CHANGED_EVENT, onFontSize);
    window.addEventListener(TERMINAL_CURSOR_CHANGED_EVENT, onCursor);
    window.addEventListener(THEME_CHANGED_EVENT, onTheme);
    window.addEventListener(ACCENT_CHANGED_EVENT, onTheme);
    return () => {
      window.removeEventListener(TERMINAL_FONT_CHANGED_EVENT, onFontSize);
      window.removeEventListener(TERMINAL_CURSOR_CHANGED_EVENT, onCursor);
      window.removeEventListener(THEME_CHANGED_EVENT, onTheme);
      window.removeEventListener(ACCENT_CHANGED_EVENT, onTheme);
    };
  }, [fitTerminal]);

  useEffect(() => {
    return () => {
      disposedRef.current = true;
      const token = tokenRef.current;
      tokenRef.current = null;
      if (token) {
        void api.killTerminal(session.id, token);
      }
      resizeObserverRef.current?.disconnect();
      resizeObserverRef.current = null;
      for (const dispose of disposablesRef.current) dispose.dispose();
      disposablesRef.current = [];
      for (const unlisten of unlistenersRef.current) unlisten();
      unlistenersRef.current = [];
      // WebGL must be disposed before the parent terminal so its GPU
      // resources are released cleanly.
      try {
        webglRef.current?.dispose();
      } catch {
        // ignore
      }
      webglRef.current = null;
      terminalRef.current?.dispose();
      terminalRef.current = null;
      fitRef.current = null;
    };
  }, [session.id]);

  return (
    <div
      ref={containerRef}
      className="terminal-viewport"
      data-active={active ? "true" : "false"}
      // No global onMouseDown focus -- it would steal the mouse
      // selection. Click on the empty area still focuses thanks to
      // xterm's own handlers.
    />
  );
}

function createSession(index: number): TerminalSession {
  return {
    id: `terminal-${index}`,
    index,
    title: `Terminal ${index}`,
    status: "idle",
  };
}

function createTerminalToken(sessionId: string): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return `${sessionId}-${crypto.randomUUID()}`;
  }
  return `${sessionId}-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function terminalTheme() {
  const root = getComputedStyle(document.documentElement);
  const css = (name: string, fallback: string) =>
    root.getPropertyValue(name).trim() || fallback;
  const isLight = document.documentElement.dataset.theme === "light";

  if (isLight) {
    return {
      background: css("--bg-0", "#faf8f3"),
      foreground: css("--editor-fg", "#2a2620"),
      cursor: css("--accent", "#2563eb"),
      selectionBackground: "rgba(48, 40, 30, 0.18)",
      black: "#3c3830",
      red: css("--danger", "#c43232"),
      green: css("--ok", "#2f8a4a"),
      yellow: "#a85a1a",
      blue: css("--accent", "#2563eb"),
      magenta: css("--accent-2", "#7c3aed"),
      cyan: "#0e7490",
      white: "#5c574d",
      brightBlack: "#8a8478",
      brightRed: "#dc2626",
      brightGreen: "#16a34a",
      brightYellow: "#d97706",
      brightBlue: css("--accent-hi", "#1d4ed8"),
      brightMagenta: "#8b5cf6",
      brightCyan: "#0891b2",
      brightWhite: "#2a2620",
    };
  }

  return {
    background: css("--bg-0", "#0b0b0d"),
    foreground: css("--editor-fg", "#e8e9ec"),
    cursor: css("--accent", "#3b82f6"),
    selectionBackground: "rgba(232, 233, 236, 0.18)",
    black: "#111318",
    red: css("--danger", "#f5737f"),
    green: css("--ok", "#22c55e"),
    yellow: "#f5d36b",
    blue: css("--accent", "#3b82f6"),
    magenta: css("--accent-2", "#c4b5fd"),
    cyan: "#67e8f9",
    white: css("--text-1", "#d2d4d9"),
    brightBlack: css("--text-3", "#6b6f78"),
    brightRed: "#ff8a94",
    brightGreen: "#86efac",
    brightYellow: "#fde68a",
    brightBlue: css("--accent-hi", "#5b8cff"),
    brightMagenta: "#ddd6fe",
    brightCyan: "#a5f3fc",
    brightWhite: css("--text-0", "#e8e9ec"),
  };
}

function stripAnsi(value: string): string {
  return value.replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, "");
}

// ---------------------------------------------------------------------------
// Path link provider
// ---------------------------------------------------------------------------
//
// VS Code-style cmd/ctrl+click on file paths printed in the terminal.
// Detects the most common path shapes emitted by CLIs (compilers, AI
// coding agents, grep/rg, ...):
//
//   foo.tsx                 (bare filename with known-ish extension)
//   src/foo/bar.ts          (relative path with at least one slash)
//   ./foo.ts, ../foo.ts     (explicitly relative)
//   /Users/.../foo.ts       (absolute POSIX)
//   ~/.config/foo.ts        (home-relative)
//   foo.tsx:42, foo.tsx:42:5
//
// URLs (http/https/...) are deliberately ignored -- the WebLinksAddon
// handles those.

const PATH_LINK_REGEX =
  // 1) (~ or ./ or ../)?/... or 2) bare path with slash or 3) bare filename
  //    .ext, all with an optional :line[:col] suffix.
  /(?:(?:~|\.{1,2})?\/[A-Za-z0-9_.+\-@~/]+|[A-Za-z0-9_.+\-@]+(?:\/[A-Za-z0-9_.+\-@]+)+|[A-Za-z0-9_.+\-@]+\.[A-Za-z][A-Za-z0-9]{0,6})(?::\d+(?::\d+)?)?/g;

const URL_LIKE_PREFIX = /^(?:https?:|ftp:|ssh:|file:|mailto:|git\+|tel:)/i;

function createPathLinkProvider(
  terminal: Terminal,
  getHandler: () => ((rawPath: string) => void) | undefined,
): ILinkProvider {
  return {
    provideLinks(bufferLineNumber: number, callback: (links: ILink[] | undefined) => void) {
      const text = readLogicalLine(terminal, bufferLineNumber);
      if (!text) {
        callback(undefined);
        return;
      }

      const links: ILink[] = [];
      // `lastIndex` cursor handles overlapping potential matches.
      PATH_LINK_REGEX.lastIndex = 0;
      let match: RegExpExecArray | null;
      while ((match = PATH_LINK_REGEX.exec(text)) !== null) {
        const raw = match[0];
        if (!raw || URL_LIKE_PREFIX.test(raw)) continue;
        // Strip trailing punctuation that the regex may have eaten
        // ("see foo.ts." or "(foo.ts)").
        const trimmed = raw.replace(/[),.;:\]]+$/g, (tail) => {
          // Keep `:N` and `:N:M` line/col suffixes.
          if (/^:\d+(?::\d+)?$/.test(tail)) return tail;
          // Drop other trailing punctuation only if it isn't part of a
          // line/col suffix.
          return tail.replace(/[),.;\]]+$/g, "");
        });
        if (!trimmed) continue;

        const startIndex = match.index;
        const endIndex = startIndex + trimmed.length;
        const range = logicalRangeToBufferRange(
          terminal,
          bufferLineNumber,
          startIndex,
          endIndex,
        );
        if (!range) continue;

        links.push({
          range,
          text: trimmed,
          activate(event) {
            // VS Code semantics: cmd/ctrl+click only.
            if (!event.metaKey && !event.ctrlKey) return;
            event.preventDefault();
            event.stopPropagation();
            getHandler()?.(trimmed);
          },
          hover() {
            // xterm.js applies the default link styling automatically.
          },
          leave() {
            // no-op
          },
        });
      }

      callback(links.length > 0 ? links : undefined);
    },
  };
}

/**
 * xterm represents wrapped lines as multiple buffer rows. For accurate
 * path matching we collapse the wrapped slice back into a single logical
 * string starting at `bufferLineNumber`.
 */
function readLogicalLine(terminal: Terminal, bufferLineNumber: number): string {
  const buffer = terminal.buffer.active;
  const line = buffer.getLine(bufferLineNumber);
  if (!line) return "";
  // Walk back to the start of a wrapped logical line so the regex sees
  // the full text. We then stitch all wrapped rows forward.
  let start = bufferLineNumber;
  while (start > 0) {
    const prev = buffer.getLine(start - 1);
    if (!prev || !buffer.getLine(start)?.isWrapped) break;
    start -= 1;
  }
  const parts: string[] = [];
  let cursor = start;
  while (true) {
    const current = buffer.getLine(cursor);
    if (!current) break;
    parts.push(current.translateToString(true));
    const next = buffer.getLine(cursor + 1);
    if (!next || !next.isWrapped) break;
    cursor += 1;
  }
  return parts.join("");
}

/**
 * Convert a [start,end] offset inside the *logical* (unwrapped) line
 * starting at `bufferLineNumber` to xterm's wrapped-buffer coordinates.
 */
function logicalRangeToBufferRange(
  terminal: Terminal,
  bufferLineNumber: number,
  startIndex: number,
  endIndex: number,
): IBufferRange | null {
  const cols = terminal.cols;
  if (cols <= 0) return null;
  // Find the first row of the wrapped logical line.
  const buffer = terminal.buffer.active;
  let firstRow = bufferLineNumber;
  while (firstRow > 0) {
    if (!buffer.getLine(firstRow)?.isWrapped) break;
    firstRow -= 1;
  }
  const startRowOffset = Math.floor(startIndex / cols);
  const startCol = startIndex - startRowOffset * cols;
  const endRowOffset = Math.floor(Math.max(0, endIndex - 1) / cols);
  const endCol = (endIndex - 1) - endRowOffset * cols;
  return {
    // xterm.js link ranges are 1-based.
    start: { x: startCol + 1, y: firstRow + startRowOffset + 1 },
    end: { x: endCol + 1, y: firstRow + endRowOffset + 1 },
  };
}
