import { getCurrentWebview } from "@tauri-apps/api/webview";

// Persisted appearance settings (window zoom, Monaco / xterm / chat font,
// terminal cursor). Each setter dispatches an event so live components
// pick up the change without prop drilling.

const ZOOM_KEY = "sinew.zoomLevel";
const EDITOR_FONT_KEY = "sinew.editorFontSize";
const TERMINAL_FONT_KEY = "sinew.terminalFontSize";
const CHAT_FONT_KEY = "sinew.chatFontSize";
const TERMINAL_CURSOR_STYLE_KEY = "sinew.terminalCursorStyle";
const TERMINAL_CURSOR_BLINK_KEY = "sinew.terminalCursorBlink";
const TERMINAL_SHELL_KEY = "sinew.terminalShell";
const TERMINAL_COPY_ON_SELECT_KEY = "sinew.terminalCopyOnSelect";
const THEME_KEY = "sinew.theme";
const ACCENT_KEY = "sinew.accent";
const EDITOR_AUTOSAVE_MODE_KEY = "sinew.editorAutosaveMode";
const EDITOR_AUTOSAVE_DELAY_KEY = "sinew.editorAutosaveDelayMs";
const EDITOR_LINE_NUMBERS_KEY = "sinew.editorLineNumbers";
const EDITOR_RENDER_WHITESPACE_KEY = "sinew.editorRenderWhitespace";
const CHAT_SHOW_TIMESTAMPS_KEY = "sinew.chatShowTimestamps";
const FILETREE_SHOW_HIDDEN_KEY = "sinew.fileTreeShowHidden";
const WORKSPACE_CONFIRM_CLOSE_KEY = "sinew.confirmCloseUnsaved";
const LEGACY_UI_SCALE_KEY = "sinew.uiScale";

export const ZOOM_CHANGED_EVENT = "sinew:zoom-changed";
export const EDITOR_FONT_CHANGED_EVENT = "sinew:editor-font-changed";
export const TERMINAL_FONT_CHANGED_EVENT = "sinew:terminal-font-changed";
export const CHAT_FONT_CHANGED_EVENT = "sinew:chat-font-changed";
export const TERMINAL_CURSOR_CHANGED_EVENT = "sinew:terminal-cursor-changed";
export const TERMINAL_SHELL_CHANGED_EVENT = "sinew:terminal-shell-changed";
export const TERMINAL_COPY_ON_SELECT_CHANGED_EVENT =
  "sinew:terminal-copy-on-select-changed";
export const THEME_CHANGED_EVENT = "sinew:theme-changed";
export const ACCENT_CHANGED_EVENT = "sinew:accent-changed";
export const EDITOR_AUTOSAVE_CHANGED_EVENT = "sinew:editor-autosave-changed";
export const EDITOR_LINE_NUMBERS_CHANGED_EVENT =
  "sinew:editor-line-numbers-changed";
export const EDITOR_RENDER_WHITESPACE_CHANGED_EVENT =
  "sinew:editor-render-whitespace-changed";
export const CHAT_SHOW_TIMESTAMPS_CHANGED_EVENT =
  "sinew:chat-show-timestamps-changed";
export const FILETREE_SHOW_HIDDEN_CHANGED_EVENT =
  "sinew:filetree-show-hidden-changed";
export const WORKSPACE_CONFIRM_CLOSE_CHANGED_EVENT =
  "sinew:workspace-confirm-close-changed";

// Matches Chromium / VSCode: each zoom level steps by 1.2x.
const ZOOM_FACTOR_BASE = 1.2;
export const ZOOM_LEVEL_MIN = -5;
export const ZOOM_LEVEL_MAX = 8;
export const ZOOM_LEVEL_DEFAULT = 0;

export const EDITOR_FONT_MIN = 6;
export const EDITOR_FONT_MAX = 32;
export const EDITOR_FONT_DEFAULT = 12;

export const TERMINAL_FONT_MIN = 6;
export const TERMINAL_FONT_MAX = 32;
export const TERMINAL_FONT_DEFAULT = 12;

export const CHAT_FONT_MIN = 10;
export const CHAT_FONT_MAX = 22;
export const CHAT_FONT_DEFAULT = 13;

export type TerminalCursorStyle = "block" | "underline" | "bar";
export type TerminalCursorState = {
  style: TerminalCursorStyle;
  blink: boolean;
};
export const TERMINAL_CURSOR_STYLE_DEFAULT: TerminalCursorStyle = "block";
export const TERMINAL_CURSOR_BLINK_DEFAULT = true;

export type Theme = "dark" | "light" | "system";
export type EffectiveTheme = "dark" | "light";
export const THEME_DEFAULT: Theme = "system";

export type EditorAutosaveMode = "off" | "afterDelay" | "onBlur";
export const EDITOR_AUTOSAVE_MODE_DEFAULT: EditorAutosaveMode = "off";
export const EDITOR_AUTOSAVE_DELAY_DEFAULT = 1000;
export const EDITOR_AUTOSAVE_DELAY_MIN = 200;
export const EDITOR_AUTOSAVE_DELAY_MAX = 30_000;

export type EditorLineNumbers = "on" | "off" | "relative";
export const EDITOR_LINE_NUMBERS_DEFAULT: EditorLineNumbers = "on";

export type EditorRenderWhitespace = "none" | "boundary" | "all";
export const EDITOR_RENDER_WHITESPACE_DEFAULT: EditorRenderWhitespace = "none";

export const CHAT_SHOW_TIMESTAMPS_DEFAULT = false;
export const TERMINAL_COPY_ON_SELECT_DEFAULT = false;
export const FILETREE_SHOW_HIDDEN_DEFAULT = false;
export const WORKSPACE_CONFIRM_CLOSE_DEFAULT = true;

export type Accent = "blue" | "lavender" | "green" | "orange" | "pink";
export const ACCENT_DEFAULT: Accent = "blue";
export const ACCENT_SWATCHES: { value: Accent; label: string; color: string }[] = [
  { value: "blue", label: "Blue", color: "#3b82f6" },
  { value: "lavender", label: "Lavender", color: "#8b5cf6" },
  { value: "green", label: "Green", color: "#22c55e" },
  { value: "orange", label: "Orange", color: "#fb923c" },
  { value: "pink", label: "Pink", color: "#f43f5e" },
];

// Mirrors WindowControls.tsx's platform sniff.
export const IS_MAC: boolean = (() => {
  if (typeof navigator === "undefined") return false;
  const uaData = (navigator as Navigator & {
    userAgentData?: { platform?: string };
  }).userAgentData;
  const raw = uaData?.platform ?? navigator.platform ?? navigator.userAgent ?? "";
  return raw.toLowerCase().includes("mac");
})();

export const PRIMARY_MOD_LABEL: string = IS_MAC ? "⌘" : "Ctrl";

function clamp(n: number, min: number, max: number): number {
  if (!Number.isFinite(n)) return min;
  return Math.min(max, Math.max(min, n));
}

function readNumber(key: string, fallback: number, min: number, max: number): number {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return fallback;
    const n = parseFloat(raw);
    return Number.isFinite(n) ? clamp(n, min, max) : fallback;
  } catch {
    return fallback;
  }
}

function writeNumber(key: string, value: number): void {
  try {
    localStorage.setItem(key, String(value));
  } catch {
    // ignore quota errors
  }
}

function readBoolean(key: string, fallback: boolean): boolean {
  try {
    const raw = localStorage.getItem(key);
    if (raw === null) return fallback;
    return raw === "true";
  } catch {
    return fallback;
  }
}

function writeBoolean(key: string, value: boolean): void {
  try {
    localStorage.setItem(key, value ? "true" : "false");
  } catch {
    // ignore quota errors
  }
}

function writeString(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // ignore quota errors
  }
}

export function zoomLevelToFactor(level: number): number {
  return Math.pow(ZOOM_FACTOR_BASE, level);
}

export function zoomLevelToPercent(level: number): number {
  return Math.round(zoomLevelToFactor(level) * 100);
}

function applyZoom(level: number): void {
  void getCurrentWebview()
    .setZoom(zoomLevelToFactor(level))
    .catch((err) => console.error("[appearance] setZoom failed", err));
}

export function getZoomLevel(): number {
  return Math.round(readNumber(ZOOM_KEY, ZOOM_LEVEL_DEFAULT, ZOOM_LEVEL_MIN, ZOOM_LEVEL_MAX));
}

export function setZoomLevel(level: number): number {
  const next = Math.round(clamp(level, ZOOM_LEVEL_MIN, ZOOM_LEVEL_MAX));
  writeNumber(ZOOM_KEY, next);
  applyZoom(next);
  window.dispatchEvent(new CustomEvent<number>(ZOOM_CHANGED_EVENT, { detail: next }));
  return next;
}

export function bumpZoomLevel(delta: number): number {
  return setZoomLevel(getZoomLevel() + delta);
}

export function resetZoomLevel(): number {
  return setZoomLevel(ZOOM_LEVEL_DEFAULT);
}

export function getEditorFontSize(): number {
  return readNumber(EDITOR_FONT_KEY, EDITOR_FONT_DEFAULT, EDITOR_FONT_MIN, EDITOR_FONT_MAX);
}

export function setEditorFontSize(size: number): number {
  const next = clamp(size, EDITOR_FONT_MIN, EDITOR_FONT_MAX);
  writeNumber(EDITOR_FONT_KEY, next);
  window.dispatchEvent(
    new CustomEvent<number>(EDITOR_FONT_CHANGED_EVENT, { detail: next }),
  );
  return next;
}

export function resetEditorFontSize(): number {
  return setEditorFontSize(EDITOR_FONT_DEFAULT);
}

export function getTerminalFontSize(): number {
  return readNumber(TERMINAL_FONT_KEY, TERMINAL_FONT_DEFAULT, TERMINAL_FONT_MIN, TERMINAL_FONT_MAX);
}

export function setTerminalFontSize(size: number): number {
  const next = clamp(size, TERMINAL_FONT_MIN, TERMINAL_FONT_MAX);
  writeNumber(TERMINAL_FONT_KEY, next);
  window.dispatchEvent(
    new CustomEvent<number>(TERMINAL_FONT_CHANGED_EVENT, { detail: next }),
  );
  return next;
}

export function resetTerminalFontSize(): number {
  return setTerminalFontSize(TERMINAL_FONT_DEFAULT);
}

export function getChatFontSize(): number {
  return readNumber(CHAT_FONT_KEY, CHAT_FONT_DEFAULT, CHAT_FONT_MIN, CHAT_FONT_MAX);
}

export function setChatFontSize(size: number): number {
  const next = clamp(size, CHAT_FONT_MIN, CHAT_FONT_MAX);
  writeNumber(CHAT_FONT_KEY, next);
  window.dispatchEvent(
    new CustomEvent<number>(CHAT_FONT_CHANGED_EVENT, { detail: next }),
  );
  return next;
}

export function resetChatFontSize(): number {
  return setChatFontSize(CHAT_FONT_DEFAULT);
}

function parseCursorStyle(raw: string | null): TerminalCursorStyle {
  if (raw === "underline" || raw === "bar") return raw;
  return "block";
}

export function getTerminalCursorStyle(): TerminalCursorStyle {
  try {
    return parseCursorStyle(localStorage.getItem(TERMINAL_CURSOR_STYLE_KEY));
  } catch {
    return TERMINAL_CURSOR_STYLE_DEFAULT;
  }
}

export function setTerminalCursorStyle(style: TerminalCursorStyle): TerminalCursorStyle {
  writeString(TERMINAL_CURSOR_STYLE_KEY, style);
  window.dispatchEvent(
    new CustomEvent(TERMINAL_CURSOR_CHANGED_EVENT, {
      detail: { style, blink: getTerminalCursorBlink() },
    }),
  );
  return style;
}

export function getTerminalCursorBlink(): boolean {
  return readBoolean(TERMINAL_CURSOR_BLINK_KEY, TERMINAL_CURSOR_BLINK_DEFAULT);
}

export function setTerminalCursorBlink(blink: boolean): boolean {
  writeBoolean(TERMINAL_CURSOR_BLINK_KEY, blink);
  window.dispatchEvent(
    new CustomEvent(TERMINAL_CURSOR_CHANGED_EVENT, {
      detail: { style: getTerminalCursorStyle(), blink },
    }),
  );
  return blink;
}

function parseAccent(raw: string | null): Accent {
  switch (raw) {
    case "blue":
    case "lavender":
    case "green":
    case "orange":
    case "pink":
      return raw;
    default:
      return ACCENT_DEFAULT;
  }
}

export function getAccent(): Accent {
  try {
    return parseAccent(localStorage.getItem(ACCENT_KEY));
  } catch {
    return ACCENT_DEFAULT;
  }
}

function applyAccent(accent: Accent): void {
  document.documentElement.dataset.accent = accent;
}

export function setAccent(accent: Accent): Accent {
  writeString(ACCENT_KEY, accent);
  applyAccent(accent);
  window.dispatchEvent(
    new CustomEvent<Accent>(ACCENT_CHANGED_EVENT, { detail: accent }),
  );
  return accent;
}

export function getTerminalShell(): string {
  try {
    return localStorage.getItem(TERMINAL_SHELL_KEY) ?? "";
  } catch {
    return "";
  }
}

export function setTerminalShell(path: string): string {
  const next = path.trim();
  if (next) writeString(TERMINAL_SHELL_KEY, next);
  else
    try {
      localStorage.removeItem(TERMINAL_SHELL_KEY);
    } catch {
      // ignore
    }
  window.dispatchEvent(
    new CustomEvent<string>(TERMINAL_SHELL_CHANGED_EVENT, { detail: next }),
  );
  return next;
}

function parseTheme(raw: string | null): Theme {
  if (raw === "light" || raw === "dark" || raw === "system") return raw;
  return THEME_DEFAULT;
}

export function getTheme(): Theme {
  try {
    return parseTheme(localStorage.getItem(THEME_KEY));
  } catch {
    return THEME_DEFAULT;
  }
}

export function getEffectiveTheme(): EffectiveTheme {
  const theme = getTheme();
  if (theme === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
  }
  return theme;
}

function applyTheme(theme: Theme): EffectiveTheme {
  const effective: EffectiveTheme =
    theme === "system"
      ? window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light"
      : theme;
  document.documentElement.dataset.theme = effective;
  return effective;
}

export function setTheme(theme: Theme): Theme {
  writeString(THEME_KEY, theme);
  const effective = applyTheme(theme);
  window.dispatchEvent(
    new CustomEvent<EffectiveTheme>(THEME_CHANGED_EVENT, { detail: effective }),
  );
  return theme;
}

export function resetTheme(): Theme {
  return setTheme(THEME_DEFAULT);
}

// Reapply the effective theme whenever the OS preference changes — only
// matters while the user has the "system" option selected.
let systemThemeBound = false;
function bindSystemThemeListener(): void {
  if (systemThemeBound || typeof window.matchMedia !== "function") return;
  systemThemeBound = true;
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener(
    "change",
    () => {
      if (getTheme() !== "system") return;
      const effective = applyTheme("system");
      window.dispatchEvent(
        new CustomEvent<EffectiveTheme>(THEME_CHANGED_EVENT, {
          detail: effective,
        }),
      );
    },
  );
}

function parseAutosaveMode(raw: string | null): EditorAutosaveMode {
  return raw === "afterDelay" || raw === "onBlur" ? raw : "off";
}

export function getEditorAutosaveMode(): EditorAutosaveMode {
  try {
    return parseAutosaveMode(localStorage.getItem(EDITOR_AUTOSAVE_MODE_KEY));
  } catch {
    return EDITOR_AUTOSAVE_MODE_DEFAULT;
  }
}

export function getEditorAutosaveDelay(): number {
  return readNumber(
    EDITOR_AUTOSAVE_DELAY_KEY,
    EDITOR_AUTOSAVE_DELAY_DEFAULT,
    EDITOR_AUTOSAVE_DELAY_MIN,
    EDITOR_AUTOSAVE_DELAY_MAX,
  );
}

export type EditorAutosaveState = {
  mode: EditorAutosaveMode;
  delayMs: number;
};

function emitAutosave(): void {
  window.dispatchEvent(
    new CustomEvent<EditorAutosaveState>(EDITOR_AUTOSAVE_CHANGED_EVENT, {
      detail: { mode: getEditorAutosaveMode(), delayMs: getEditorAutosaveDelay() },
    }),
  );
}

export function setEditorAutosaveMode(mode: EditorAutosaveMode): EditorAutosaveMode {
  writeString(EDITOR_AUTOSAVE_MODE_KEY, mode);
  emitAutosave();
  return mode;
}

export function setEditorAutosaveDelay(delayMs: number): number {
  const next = Math.round(
    clamp(delayMs, EDITOR_AUTOSAVE_DELAY_MIN, EDITOR_AUTOSAVE_DELAY_MAX),
  );
  writeNumber(EDITOR_AUTOSAVE_DELAY_KEY, next);
  emitAutosave();
  return next;
}

function parseLineNumbers(raw: string | null): EditorLineNumbers {
  return raw === "off" || raw === "relative" ? raw : "on";
}

export function getEditorLineNumbers(): EditorLineNumbers {
  try {
    return parseLineNumbers(localStorage.getItem(EDITOR_LINE_NUMBERS_KEY));
  } catch {
    return EDITOR_LINE_NUMBERS_DEFAULT;
  }
}

export function setEditorLineNumbers(mode: EditorLineNumbers): EditorLineNumbers {
  writeString(EDITOR_LINE_NUMBERS_KEY, mode);
  window.dispatchEvent(
    new CustomEvent<EditorLineNumbers>(EDITOR_LINE_NUMBERS_CHANGED_EVENT, {
      detail: mode,
    }),
  );
  return mode;
}

function parseRenderWhitespace(raw: string | null): EditorRenderWhitespace {
  return raw === "boundary" || raw === "all" ? raw : "none";
}

export function getEditorRenderWhitespace(): EditorRenderWhitespace {
  try {
    return parseRenderWhitespace(localStorage.getItem(EDITOR_RENDER_WHITESPACE_KEY));
  } catch {
    return EDITOR_RENDER_WHITESPACE_DEFAULT;
  }
}

export function setEditorRenderWhitespace(
  mode: EditorRenderWhitespace,
): EditorRenderWhitespace {
  writeString(EDITOR_RENDER_WHITESPACE_KEY, mode);
  window.dispatchEvent(
    new CustomEvent<EditorRenderWhitespace>(EDITOR_RENDER_WHITESPACE_CHANGED_EVENT, {
      detail: mode,
    }),
  );
  return mode;
}

export function getChatShowTimestamps(): boolean {
  return readBoolean(CHAT_SHOW_TIMESTAMPS_KEY, CHAT_SHOW_TIMESTAMPS_DEFAULT);
}

export function setChatShowTimestamps(value: boolean): boolean {
  writeBoolean(CHAT_SHOW_TIMESTAMPS_KEY, value);
  window.dispatchEvent(
    new CustomEvent<boolean>(CHAT_SHOW_TIMESTAMPS_CHANGED_EVENT, { detail: value }),
  );
  return value;
}

export function getTerminalCopyOnSelect(): boolean {
  return readBoolean(TERMINAL_COPY_ON_SELECT_KEY, TERMINAL_COPY_ON_SELECT_DEFAULT);
}

export function setTerminalCopyOnSelect(value: boolean): boolean {
  writeBoolean(TERMINAL_COPY_ON_SELECT_KEY, value);
  window.dispatchEvent(
    new CustomEvent<boolean>(TERMINAL_COPY_ON_SELECT_CHANGED_EVENT, { detail: value }),
  );
  return value;
}

export function getFileTreeShowHidden(): boolean {
  return readBoolean(FILETREE_SHOW_HIDDEN_KEY, FILETREE_SHOW_HIDDEN_DEFAULT);
}

export function setFileTreeShowHidden(value: boolean): boolean {
  writeBoolean(FILETREE_SHOW_HIDDEN_KEY, value);
  window.dispatchEvent(
    new CustomEvent<boolean>(FILETREE_SHOW_HIDDEN_CHANGED_EVENT, { detail: value }),
  );
  return value;
}

export function getWorkspaceConfirmClose(): boolean {
  return readBoolean(WORKSPACE_CONFIRM_CLOSE_KEY, WORKSPACE_CONFIRM_CLOSE_DEFAULT);
}

export function setWorkspaceConfirmClose(value: boolean): boolean {
  writeBoolean(WORKSPACE_CONFIRM_CLOSE_KEY, value);
  window.dispatchEvent(
    new CustomEvent<boolean>(WORKSPACE_CONFIRM_CLOSE_CHANGED_EVENT, {
      detail: value,
    }),
  );
  return value;
}

// Collect every persisted appearance / preference key under the
// `sinew.*` namespace into a JSON blob — used by Settings → export.
export function exportSettings(): string {
  const out: Record<string, string> = {};
  try {
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (key && key.startsWith("sinew.")) {
        const value = localStorage.getItem(key);
        if (value !== null) out[key] = value;
      }
    }
  } catch {
    // ignore
  }
  return JSON.stringify(out, null, 2);
}

export function importSettings(json: string): number {
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch {
    return 0;
  }
  if (!parsed || typeof parsed !== "object") return 0;
  let applied = 0;
  for (const [key, value] of Object.entries(parsed as Record<string, unknown>)) {
    if (!key.startsWith("sinew.")) continue;
    if (typeof value !== "string") continue;
    try {
      localStorage.setItem(key, value);
      applied += 1;
    } catch {
      // ignore quota / storage errors
    }
  }
  return applied;
}

// Carry users forward from the earlier factor-based slider.
function migrateLegacy(): void {
  try {
    const legacy = localStorage.getItem(LEGACY_UI_SCALE_KEY);
    if (!legacy) return;
    if (localStorage.getItem(ZOOM_KEY) === null) {
      const factor = parseFloat(legacy);
      if (Number.isFinite(factor) && factor > 0) {
        const level = Math.round(Math.log(factor) / Math.log(ZOOM_FACTOR_BASE));
        writeNumber(ZOOM_KEY, clamp(level, ZOOM_LEVEL_MIN, ZOOM_LEVEL_MAX));
      }
    }
    localStorage.removeItem(LEGACY_UI_SCALE_KEY);
  } catch {
    // ignore
  }
}

export function applyStoredAppearance(): void {
  migrateLegacy();
  applyTheme(getTheme());
  applyAccent(getAccent());
  bindSystemThemeListener();
  applyZoom(getZoomLevel());
}
