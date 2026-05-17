import { useCallback, useEffect, useState } from "react";
import { Welcome } from "./components/Welcome";
import { Workspace } from "./components/Workspace";
import { shouldShowOnboarding } from "./lib/onboarding";
import { loadLastWorkspace, recordRecent, deriveName } from "./lib/recents";
import { api } from "./lib/ipc";
import type { WorkspaceBootstrap } from "./types";

type AppState =
  | { kind: "welcome" }
  | { kind: "workspace"; bootstrap: WorkspaceBootstrap };

const startsEmpty =
  new URLSearchParams(window.location.search).get("newWindow") === "1";

export default function App() {
  const [state, setState] = useState<AppState>({ kind: "welcome" });
  const [bootError, setBootError] = useState<string | null>(null);

  const openWorkspace = useCallback(async (path: string) => {
    setBootError(null);
    try {
      const bootstrap = await api.openWorkspace(path);
      recordRecent(bootstrap.workspace.path, bootstrap.workspace.name);
      setState({ kind: "workspace", bootstrap });
    } catch (err) {
      setBootError(String(err));
    }
  }, []);

  // Try to auto-open last workspace on boot. Silent fallback to the
  // welcome screen if the folder no longer exists or fails to open.
  useEffect(() => {
    if (startsEmpty || shouldShowOnboarding()) return;
    const last = loadLastWorkspace();
    if (!last) return;
    (async () => {
      try {
        const bootstrap = await api.openWorkspace(last);
        recordRecent(bootstrap.workspace.path, bootstrap.workspace.name);
        setState({ kind: "workspace", bootstrap });
      } catch {
        // leave on welcome; user can pick again
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const backToWelcome = useCallback(() => {
    void api.resetWindowTitle().catch(() => {
      // best-effort; leaving the previous title is harmless
    });
    setState({ kind: "welcome" });
  }, []);

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
