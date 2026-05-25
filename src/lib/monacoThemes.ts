import type * as MonacoTypes from "monaco-editor";

type Monaco = typeof MonacoTypes;

export const MONACO_DARK_THEME = "claakecode-warm";
export const MONACO_LIGHT_THEME = "claakecode-warm-light";

/**
 * Defines both the dark and light Monaco editor themes that match the
 * Vert Noisette palette. Safe to call multiple times (Monaco overwrites).
 */
export function registerClaakeCodeThemes(monaco: Monaco): void {
  monaco.editor.defineTheme(MONACO_DARK_THEME, {
    base: "vs-dark",
    inherit: true,
    rules: [
      { token: "comment", foreground: "6b6558" },
      { token: "keyword", foreground: "8a9b78" },
      { token: "string", foreground: "b0bca0" },
      { token: "number", foreground: "c8a982" },
      { token: "type", foreground: "d0dac4" },
      { token: "function", foreground: "8a9b78" },
      { token: "variable", foreground: "e8e4d8" },
      { token: "constant", foreground: "c8a982" },
      { token: "regexp", foreground: "b0bca0" },
      { token: "tag", foreground: "e88a8a" },
      { token: "attribute.name", foreground: "8a9b78" },
    ],
    colors: {
      "editor.background": "#151412",
      "editor.foreground": "#e8e4d8",
      "editor.lineHighlightBackground": "#1f1d1a",
      "editorLineNumber.foreground": "#4a463e",
      "editorLineNumber.activeForeground": "#a09a8a",
      "editorCursor.foreground": "#6b7c5c",
      "editor.selectionBackground": "#3d4933",
      "editor.inactiveSelectionBackground": "#252320",
      "editorIndentGuide.background1": "#252320",
      "editorIndentGuide.activeBackground1": "#36332e",
      "editorGutter.background": "#151412",
      "editorWidget.background": "#1f1d1a",
      "editorWidget.border": "#36332e",
      "editorHoverWidget.background": "#1f1d1a",
      "editorHoverWidget.border": "#36332e",
      "editorSuggestWidget.background": "#1f1d1a",
      "editorSuggestWidget.border": "#36332e",
      "editorSuggestWidget.selectedBackground": "#3d4933",
      "editorSuggestWidget.highlightForeground": "#8a9b78",
      "editorBracketMatch.background": "#3d4933",
      "editorBracketMatch.border": "#6b7c5c",
      "scrollbarSlider.background": "#36332ecc",
      "scrollbarSlider.hoverBackground": "#4a463ecc",
      "scrollbarSlider.activeBackground": "#5a5448cc",
    },
  });

  monaco.editor.defineTheme(MONACO_LIGHT_THEME, {
    base: "vs",
    inherit: true,
    rules: [
      { token: "comment", foreground: "a09a8a" },
      { token: "keyword", foreground: "4e5c42" },
      { token: "string", foreground: "6b7c5c" },
      { token: "number", foreground: "8c5a30" },
      { token: "type", foreground: "3d4933" },
      { token: "function", foreground: "5a6b4d" },
      { token: "variable", foreground: "1e1c18" },
      { token: "constant", foreground: "8c5a30" },
      { token: "regexp", foreground: "6b7c5c" },
      { token: "tag", foreground: "8c3a3a" },
      { token: "attribute.name", foreground: "4e5c42" },
    ],
    colors: {
      "editor.background": "#fdfcf8",
      "editor.foreground": "#1e1c18",
      "editor.lineHighlightBackground": "#f3f0e8",
      "editorLineNumber.foreground": "#cfcab8",
      "editorLineNumber.activeForeground": "#6b6558",
      "editorCursor.foreground": "#6b7c5c",
      "editor.selectionBackground": "#d0dac4",
      "editor.inactiveSelectionBackground": "#e8e4d8",
      "editorIndentGuide.background1": "#ede9de",
      "editorIndentGuide.activeBackground1": "#dbd6c6",
      "editorGutter.background": "#fdfcf8",
      "editorWidget.background": "#f3f0e8",
      "editorWidget.border": "#cfcab8",
      "editorHoverWidget.background": "#f3f0e8",
      "editorHoverWidget.border": "#cfcab8",
      "editorSuggestWidget.background": "#f3f0e8",
      "editorSuggestWidget.border": "#cfcab8",
      "editorSuggestWidget.selectedBackground": "#d0dac4",
      "editorSuggestWidget.highlightForeground": "#4e5c42",
      "editorBracketMatch.background": "#d0dac4",
      "editorBracketMatch.border": "#6b7c5c",
      "scrollbarSlider.background": "#cfcab8cc",
      "scrollbarSlider.hoverBackground": "#b3ad9ccc",
      "scrollbarSlider.activeBackground": "#a09a8acc",
    },
  });
}

export function monacoThemeForDocument(): string {
  if (typeof document === "undefined") return MONACO_DARK_THEME;
  return document.documentElement.getAttribute("data-theme") === "light"
    ? MONACO_LIGHT_THEME
    : MONACO_DARK_THEME;
}

let monacoInstance: Monaco | null = null;

/**
 * Register themes once and remember the Monaco instance so later theme
 * toggles can refresh every active editor at once via `setTheme`.
 */
export function attachMonacoTheme(monaco: Monaco): void {
  if (monacoInstance !== monaco) {
    monacoInstance = monaco;
    registerClaakeCodeThemes(monaco);
  }
  monaco.editor.setTheme(monacoThemeForDocument());
}

if (typeof window !== "undefined") {
  window.addEventListener("claakecode:theme-changed", () => {
    if (monacoInstance) {
      monacoInstance.editor.setTheme(monacoThemeForDocument());
    }
  });
}
