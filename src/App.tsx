import { useCallback, useEffect, useState } from "react";
import { Welcome } from "./components/Welcome";
import { Workspace } from "./components/Workspace";
import { UpdaterLockScreen } from "./components/UpdaterLockScreen";
import { SinewMark } from "./components/SinewMark";
import { loadLastWorkspace, recordRecent, deriveName, clearLastWorkspace } from "./lib/recents";
import { api } from "./lib/ipc";
import type { UpdateInfo, WorkspaceBootstrap } from "./types";

type AppState =
  | { kind: "boot" }
  | { kind: "update_required"; info: UpdateInfo; autoInstall: boolean }
  | { kind: "welcome" }
  | { kind: "workspace"; bootstrap: WorkspaceBootstrap };

const startsEmpty =
  new URLSearchParams(window.location.search).get("newWindow") === "1";

/// Maximum time we wait on the boot updater check before falling through to
/// the normal flow. Keeps the app responsive on flaky networks — if the
/// update endpoint is unreachable we don't trap the user on a black canvas.
const BOOT_CHECK_TIMEOUT_MS = 4000;

export default function App() {
  const [state, setState] = useState<AppState>({ kind: "boot" });
  const [bootError, setBootError] = useState<string | null>(null);

  useEffect(() => {
    try {
      const savedChat = localStorage.getItem("sinew.chat-font-size");
      const sizeChat = savedChat ? parseInt(savedChat, 10) : 13;
      document.documentElement.style.setProperty("--chat-font-size", `${sizeChat}px`);
    } catch {}
    try {
      const savedEditor = localStorage.getItem("sinew.editor-font-size");
      const sizeEditor = savedEditor ? parseInt(savedEditor, 10) : 12;
      document.documentElement.style.setProperty("--editor-font-size", `${sizeEditor}px`);
    } catch {}
    try {
      const savedTheme = localStorage.getItem("sinew.theme") || "dark";
      document.documentElement.setAttribute("data-theme", savedTheme);
    } catch {}

    // SOTA 1: Cursor-Tracking Glow event listener for AI Glass Theme
    const handleMouseMove = (e: MouseEvent) => {
      const isAI = document.documentElement.getAttribute("data-theme") === "ai";
      if (!isAI) return;
      
      const target = (e.target as HTMLElement).closest(
        ".tool-card, .composer__box, .settings-pane__provider-card, .settings-pane__provider-card--compact"
      );
      if (target) {
        const rect = target.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        (target as HTMLElement).style.setProperty("--mouse-x", `${x}px`);
        (target as HTMLElement).style.setProperty("--mouse-y", `${y}px`);
      }
    };
    window.addEventListener("mousemove", handleMouseMove);
    return () => window.removeEventListener("mousemove", handleMouseMove);
  }, []);

  const openWorkspace = useCallback(async (path: string) => {
    setBootError(null);
    try {
      const bootstrap = await api.openWorkspace(path);
      const displayName = bootstrap.workspace.name === ".sinew-sandbox" ? "Sans dossier" : bootstrap.workspace.name;
      recordRecent(bootstrap.workspace.path, displayName);
      setState({ kind: "workspace", bootstrap });
    } catch (err) {
      setBootError(String(err));
    }
  }, []);

  // Boot sequence, in order:
  //   1. Updater gate — race the check against a short timeout. If an
  //      update is available we render <UpdaterLockScreen /> and stop;
  //      the user can only install or quit (no "Later", no "Skip").
  //   2. Auto-open last workspace (existing behaviour) when no update is
  //      pending. Silent fallback to Welcome on any failure.
  // The whole thing runs once at mount; the in-session <UpdateBadge />
  // still handles mid-session checks via its own 30 min interval.
  useEffect(() => {
    let cancelled = false;
    const startTime = Date.now();
    const MIN_BOOT_TIME_MS = 1500;

    const transitionTo = async (nextState: AppState) => {
      const elapsed = Date.now() - startTime;
      const remaining = MIN_BOOT_TIME_MS - elapsed;
      if (remaining > 0) {
        await new Promise((resolve) => setTimeout(resolve, remaining));
      }
      if (!cancelled) {
        setState(nextState);
      }
    };

    (async () => {
      // 1. Updater gate.
      let updateInfo: UpdateInfo | null = null;
      try {
        const info = await Promise.race<UpdateInfo | null>([
          api.checkForUpdate(),
          new Promise<null>((resolve) =>
            window.setTimeout(() => resolve(null), BOOT_CHECK_TIMEOUT_MS),
          ),
        ]);
        if (info && info.available && info.version) {
          updateInfo = info;
        }
      } catch {
        // Silent: a failed check (offline, server down, manifest 5xx)
        // shouldn't prevent the app from booting. The mid-session badge
        // will retry later, and the next launch will re-gate cleanly.
      }

      if (cancelled) return;

      if (updateInfo) {
        await transitionTo({ kind: "update_required", info: updateInfo, autoInstall: false });
        return;
      }

      // 2. Auto-open last workspace, falling back to Welcome.
      if (startsEmpty) {
        await transitionTo({ kind: "welcome" });
        return;
      }
      const last = loadLastWorkspace();
      if (!last) {
        await transitionTo({ kind: "welcome" });
        return;
      }
      try {
        const bootstrap = await api.openWorkspace(last);
        const displayName = bootstrap.workspace.name === ".sinew-sandbox" ? "Sans dossier" : bootstrap.workspace.name;
        recordRecent(bootstrap.workspace.path, displayName);
        await transitionTo({ kind: "workspace", bootstrap });
      } catch {
        await transitionTo({ kind: "welcome" });
      }
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  // Mid-session escalation: when the <UpdateBadge /> in Workspace fires
  // "sinew:install-update" (user clicked "Install & restart" in the
  // popover), we swap the whole window to the lock screen with
  // `autoInstall` enabled. From there the screen runs the same download
  // → install → auto-restart flow as the boot gate. This means the
  // policy is identical regardless of entry point: once the user
  // commits to installing, Sinew becomes uninteractive until the update
  // is applied or they quit.
  useEffect(() => {
    const handler = (event: WindowEventMap["sinew:install-update"]) => {
      const info = event.detail?.info;
      if (!info || !info.available || !info.version) return;
      setState({ kind: "update_required", info, autoInstall: true });
    };
    window.addEventListener("sinew:install-update", handler);
    return () => window.removeEventListener("sinew:install-update", handler);
  }, []);

  const backToWelcome = useCallback(() => {
    void api.resetWindowTitle().catch(() => {
      // best-effort; leaving the previous title is harmless
    });
    setState({ kind: "welcome" });
  }, []);

  if (state.kind === "boot") {
    // Splash screen while the updater check resolves. Render the animated logo and text.
    return (
      <div className="app-boot" aria-hidden="true">
        <div className="boot-logo-container">
          <SinewMark size={140} className="boot-logo-svg" />
          <h1 className="boot-logo-text">
            Sinew<span className="boot-logo-text-dot">.</span>
          </h1>
        </div>
      </div>
    );
  }

  if (state.kind === "update_required") {
    return (
      <UpdaterLockScreen info={state.info} autoInstall={state.autoInstall} />
    );
  }

  if (state.kind === "welcome") {
    return (
      <Welcome
        onPick={openWorkspace}
        error={bootError}
        deriveName={deriveName}
      />
    );
  }

  return (
    <Workspace
      bootstrap={state.bootstrap}
      onSwitchWorkspace={backToWelcome}
      onBootstrapReplace={(b) =>
        setState({ kind: "workspace", bootstrap: b })
      }
    />
  );
}
