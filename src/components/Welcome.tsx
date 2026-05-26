import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Icon } from "@iconify/react";
import { loadRecents, removeRecent } from "../lib/recents";
import { api } from "../lib/ipc";
import type {
  ActiveTurnSummary,
  ActiveTurnsChangedPayload,
  RecentWorkspace,
} from "../types";
import { SinewMark } from "./SinewMark";
import { WindowControls, isWindowsPlatform } from "./WindowControls";

type Props = {
  onPick: (path: string) => void;
  error: string | null;
  deriveName: (path: string) => string;
};

const MAX_VISIBLE_RECENTS = 5;
const IS_WINDOWS = isWindowsPlatform();

/// Collapse a list of `ActiveTurnSummary` items down to the set of workspace
/// paths that currently have an in-flight agent turn. The backend reports
/// these globally (across every Sinew window), so a recent workspace can be
/// "live" even when it's owned by a sibling window.
function activeWorkspaceSet(turns: ActiveTurnSummary[]): Set<string> {
  return new Set(turns.map((turn) => turn.workspaceId));
}

export function Welcome({ onPick, error, deriveName }: Props) {
  const [recents, setRecents] = useState<RecentWorkspace[]>([]);
  const [picking, setPicking] = useState(false);
  const [activeWorkspaces, setActiveWorkspaces] = useState<Set<string>>(
    () => new Set(),
  );

  useEffect(() => {
    setRecents(loadRecents());
  }, []);

  // Surface running agent turns on the recents list. We seed from
  // `list_active_turns` (so the loader is correct the moment Welcome paints)
  // then keep in sync with the `active-turns-changed` event the backend
  // fans out whenever a turn starts or finishes anywhere in the app.
  useEffect(() => {
    let cancelled = false;
    let unlisten: UnlistenFn | null = null;

    void api
      .listActiveTurns()
      .then((turns) => {
        if (!cancelled) setActiveWorkspaces(activeWorkspaceSet(turns));
      })
      .catch(() => {
        // Non-fatal: leave the set empty so the regular folder icon shows.
      });

    (async () => {
      const u = await listen<ActiveTurnsChangedPayload>(
        "active-turns-changed",
        (event) => {
          setActiveWorkspaces(activeWorkspaceSet(event.payload.activeTurns));
        },
      );
      if (cancelled) {
        u();
      } else {
        unlisten = u;
      }
    })();

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  const pickFolder = async () => {
    if (picking) return;
    setPicking(true);
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected === "string") {
        onPick(selected);
      }
    } catch {
      // user cancelled or platform error
    } finally {
      setPicking(false);
    }
  };

  const openSandbox = async () => {
    if (picking) return;
    setPicking(true);
    try {
      const sandboxPath = await api.getOrCreateSandboxWorkspace();
      onPick(sandboxPath);
    } catch {
      // ignore
    } finally {
      setPicking(false);
    }
  };

  return (
    <div className="welcome">
      {IS_WINDOWS && (
        /* Drag region + custom window controls for the frameless Windows
           shell. The wrapper itself is the drag handle (it owns
           `data-tauri-drag-region`); buttons inside opt out via
           `data-tauri-drag-region="false"` set by <WindowControls />. */
        <div
          className="welcome__titlebar"
          data-tauri-drag-region
        >
          <WindowControls />
        </div>
      )}
      <main className="welcome__stage">
        <header className="welcome__head">
          <span className="welcome__mark-dot" aria-hidden="true">
            <span className="welcome__mark-inner">
              <SinewMark size={22} className="welcome__mark-glyph" />
            </span>
          </span>
          <h1 className="welcome__title">
            Sinew<span className="welcome__title-dot">.</span>
          </h1>
          <p className="welcome__tag">Your personal Agentic IDE</p>
        </header>

        <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
          <button
            className="welcome__cta"
            onClick={pickFolder}
            disabled={picking}
          >
            <span className="welcome__cta-icon">
              <Icon icon="solar:folder-with-files-bold-duotone" width={22} height={22} />
            </span>
            <span className="welcome__cta-body">
              <span className="welcome__cta-title">Open a folder</span>
              <span className="welcome__cta-sub">
                {picking ? "Opening…" : "Choose any directory to start a session"}
              </span>
            </span>
            <span className="welcome__cta-chev">
              <Icon icon="solar:alt-arrow-right-linear" width={16} height={16} />
            </span>
          </button>

          <button
            className="welcome__cta"
            onClick={openSandbox}
            disabled={picking}
          >
            <span className="welcome__cta-icon" style={{ background: "var(--accent-2-soft)", color: "var(--accent-2)" }}>
              <Icon icon="solar:box-bold-duotone" width={22} height={22} />
            </span>
            <span className="welcome__cta-body">
              <span className="welcome__cta-title">Use without folder (Sandbox)</span>
              <span className="welcome__cta-sub">
                Work on general tasks or system files without opening a project
              </span>
            </span>
            <span className="welcome__cta-chev">
              <Icon icon="solar:alt-arrow-right-linear" width={16} height={16} />
            </span>
          </button>
        </div>

        {error && (
          <div className="welcome__error">{error}</div>
        )}

        {recents.length > 0 ? (
          <section className="welcome__section">
            <h2 className="welcome__section-heading">Recent</h2>
            <div className="welcome__recents">
              {recents.slice(0, MAX_VISIBLE_RECENTS).map((recent) => {
                const isActive = activeWorkspaces.has(recent.path);
                return (
                  <div
                    key={recent.path}
                    className="welcome__recent"
                    data-active={isActive ? "true" : "false"}
                    role="button"
                    tabIndex={0}
                    onClick={() => onPick(recent.path)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        onPick(recent.path);
                      }
                    }}
                  >
                    <span className="welcome__recent-icon">
                      {isActive ? (
                        <span
                          className="welcome__recent-spinner"
                          role="status"
                          aria-label="Agent running"
                        />
                      ) : (
                        <Icon
                          icon="solar:folder-bold-duotone"
                          width={18}
                          height={18}
                        />
                      )}
                    </span>
                    <span className="welcome__recent-body">
                      <span className="welcome__recent-name">
                        {recent.name || deriveName(recent.path)}
                      </span>
                      <span className="welcome__recent-path">{recent.path}</span>
                    </span>
                    <button
                      type="button"
                      className="welcome__recent-remove"
                      title="Remove from recents"
                      onClick={(e) => {
                        e.stopPropagation();
                        const next = removeRecent(recent.path);
                        setRecents(next);
                      }}
                      onKeyDown={(e) => {
                        e.stopPropagation();
                      }}
                    >
                      <Icon icon="solar:close-circle-linear" width={16} height={16} />
                    </button>
                  </div>
                );
              })}
            </div>
          </section>
        ) : (
          <div className="welcome__empty">
            No recent workspaces yet. Pick a folder to get started.
          </div>
        )}
      </main>
    </div>
  );
}
