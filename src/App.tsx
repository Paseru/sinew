import { useCallback, useEffect, useState } from "react";
import { Welcome } from "./components/Welcome";
import { Workspace } from "./components/Workspace";
import { UpdaterLockScreen } from "./components/UpdaterLockScreen";
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

  const openWorkspace = useCallback(async (path: string) => {
    setBootError(null);
    try {
      const bootstrap = await api.openWorkspace(path);
      const displayName = bootstrap.workspace.name === ".sinew-sandbox" ? "Espace Fourtout (Sandbox)" : bootstrap.workspace.name;
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

    (async () => {
      // 1. Updater gate.
      try {
        const info = await Promise.race<UpdateInfo | null>([
          api.checkForUpdate(),
          new Promise<null>((resolve) =>
            window.setTimeout(() => resolve(null), BOOT_CHECK_TIMEOUT_MS),
          ),
        ]);
        if (cancelled) return;
        if (info && info.available && info.version) {
          setState({ kind: "update_required", info, autoInstall: false });
          return;
        }
      } catch {
        // Silent: a failed check (offline, server down, manifest 5xx)
        // shouldn't prevent the app from booting. The mid-session badge
        // will retry later, and the next launch will re-gate cleanly.
      }

      // 2. Auto-open last workspace, falling back to Welcome.
      if (cancelled) return;
      if (startsEmpty) {
        setState({ kind: "welcome" });
        return;
      }
      const last = loadLastWorkspace();
      if (!last) {
        setState({ kind: "welcome" });
        return;
      }
      try {
        const bootstrap = await api.openWorkspace(last);
        if (cancelled) return;
        const displayName = bootstrap.workspace.name === ".sinew-sandbox" ? "Espace Fourtout (Sandbox)" : bootstrap.workspace.name;
        recordRecent(bootstrap.workspace.path, displayName);
        setState({ kind: "workspace", bootstrap });
      } catch {
        if (!cancelled) setState({ kind: "welcome" });
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
    // Minimal splash while the updater check resolves. Pure canvas — the
    // real updater UI (or Welcome) takes over within a few hundred ms on
    // a healthy network, ~4s worst case before the timeout fires.
    return <div className="app-boot" aria-hidden="true" />;
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
