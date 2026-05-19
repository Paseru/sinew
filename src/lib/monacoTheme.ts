import type * as Monaco from "monaco-editor";

import { ACCENT_SWATCHES, type Accent, type EffectiveTheme } from "./appearance";

export const SINEW_DARK_THEME = "sinew-cool";
export const SINEW_LIGHT_THEME = "sinew-light";

export function monacoThemeName(theme: EffectiveTheme): string {
  return theme === "light" ? SINEW_LIGHT_THEME : SINEW_DARK_THEME;
}

function accentHex(accent: Accent): string {
  return (
    ACCENT_SWATCHES.find((swatch) => swatch.value === accent)?.color ?? "#3b82f6"
  );
}

// Both Monaco theme variants share the same syntax token rules; only the
// cursor / bracket-match / suggest highlight pick up the user-selected
// accent so live changes show up in the editor.
export function defineMonacoThemes(
  monaco: typeof Monaco,
  accent: Accent,
): void {
  const cursor = accentHex(accent);
  monaco.editor.defineTheme(SINEW_DARK_THEME, {
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
      "editorCursor.foreground": cursor,
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
      "editorSuggestWidget.highlightForeground": cursor,
      "editorBracketMatch.background": "#1e2b4a",
      "editorBracketMatch.border": cursor,
      "scrollbarSlider.background": "#23252bcc",
      "scrollbarSlider.hoverBackground": "#2b2e35cc",
      "scrollbarSlider.activeBackground": "#3a3d44cc",
    },
  });
  monaco.editor.defineTheme(SINEW_LIGHT_THEME, {
    base: "vs",
    inherit: true,
    rules: [
      { token: "comment", foreground: "8a8478" },
      { token: "keyword", foreground: "7c3aed" },
      { token: "string", foreground: "1f6b39" },
      { token: "number", foreground: "a85a1a" },
      { token: "type", foreground: "a85a1a" },
      { token: "function", foreground: "1d4ed8" },
      { token: "variable", foreground: "2a2620" },
      { token: "constant", foreground: "a85a1a" },
      { token: "regexp", foreground: "1f6b39" },
      { token: "tag", foreground: "a32424" },
      { token: "attribute.name", foreground: "7c3aed" },
    ],
    colors: {
      "editor.background": "#fbf9f4",
      "editor.foreground": "#2a2620",
      "editor.lineHighlightBackground": "#f3efe7",
      "editorLineNumber.foreground": "#c2bcaf",
      "editorLineNumber.activeForeground": "#5c574d",
      "editorCursor.foreground": cursor,
      "editor.selectionBackground": "#dde7ff",
      "editor.inactiveSelectionBackground": "#ece7dc",
      "editorIndentGuide.background1": "#ece7dc",
      "editorIndentGuide.activeBackground1": "#d8d1c0",
      "editorGutter.background": "#fbf9f4",
      "editorWidget.background": "#f6f2ea",
      "editorWidget.border": "#d8d1c0",
      "editorHoverWidget.background": "#f6f2ea",
      "editorHoverWidget.border": "#d8d1c0",
      "editorSuggestWidget.background": "#f6f2ea",
      "editorSuggestWidget.border": "#d8d1c0",
      "editorSuggestWidget.selectedBackground": "#dde7ff",
      "editorSuggestWidget.highlightForeground": cursor,
      "editorBracketMatch.background": "#dde7ff",
      "editorBracketMatch.border": cursor,
      "scrollbarSlider.background": "#cbc3afcc",
      "scrollbarSlider.hoverBackground": "#b8af99cc",
      "scrollbarSlider.activeBackground": "#a39880cc",
    },
  });
}
