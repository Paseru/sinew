import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri dev server config
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**", "**/target/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    target: "es2022",
    minify: "esbuild",
    sourcemap: false,
    // Monaco editor sits around 3.8 MB by itself; raise the limit so the
    // bundler warning only fires for genuinely unexpected growth.
    chunkSizeWarningLimit: 4000,
    rollupOptions: {
      output: {
        manualChunks(id: string) {
          if (!id.includes("node_modules")) return;
          if (id.includes("monaco-editor") || id.includes("@monaco-editor")) {
            return "monaco";
          }
          if (id.includes("@xterm/")) return "xterm";
          if (id.includes("mermaid") || id.includes("@mermaid-js")) {
            return "mermaid";
          }
          if (
            id.includes("react-markdown") ||
            id.includes("remark-") ||
            id.includes("rehype-") ||
            id.includes("highlight.js")
          ) {
            return "markdown";
          }
          if (id.includes("@iconify") || id.includes("lucide-react")) {
            return "icons";
          }
        },
      },
    },
  },
}));
