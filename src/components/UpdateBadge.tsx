import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Icon } from "@iconify/react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { api } from "../lib/ipc";
import type { UpdateInfo, UpdateProgress } from "../types";

type Status =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "available"; info: UpdateInfo }
  | { kind: "downloading"; info: UpdateInfo; downloaded: number; total: number | null }
  | { kind: "ready"; info: UpdateInfo }
  | { kind: "error"; message: string };

/// Background check cadence once the app has booted.
const CHECK_INTERVAL_MS = 30 * 60 * 1000; // 30 min

/// Tauri events emitted by `src-tauri/src/updater.rs`.
const PROGRESS_EVENT = "updater://progress";
const FINISHED_EVENT = "updater://finished";

/// Titlebar affordance that surfaces auto-updates. Stays hidden when the app
/// is on the latest version; expands into a pill + popover when an update is
/// available, downloading, or ready to install.
export function UpdateBadge() {
  const [status, setStatus] = useState<Status>({ kind: "idle" });
  const [open, setOpen] = useState(false);

  // We keep the interval id around so manual checks reset the cadence.
  const intervalRef = useRef<number | null>(null);
  // Latest info snapshot — used to keep the popover content after a download
  // mutates the status into `ready`.
  const lastInfoRef = useRef<UpdateInfo | null>(null);

  const runCheck = useCallback(
    async ({ silent = false }: { silent?: boolean } = {}) => {
      // Don't clobber an in-flight download with a check result.
      setStatus((current) =>
        current.kind === "downloading" || current.kind === "ready"
          ? current
          : silent
            ? current
            : { kind: "checking" },
      );
      try {
        const info = await api.checkForUpdate();
        lastInfoRef.current = info;
        setStatus((current) => {
          if (current.kind === "downloading" || current.kind === "ready") {
            return current;
          }
          if (info.available && info.version) {
            return { kind: "available", info };
          }
          return { kind: "idle" };
        });
      } catch (err) {
        // Silent failures (background poll) shouldn't surface; only manual
        // checks should turn into a visible error pill.
        if (!silent) {
          setStatus({ kind: "error", message: String(err) });
        }
      }
    },
    [],
  );

  // Boot: first check + 30 min interval. We re-check on focus too so a long-
  // running session doesn't miss a release.
  useEffect(() => {
    let shouldCheck = true;
    try {
      const saved = localStorage.getItem("sinew.auto-update-check");
      if (saved === "false") {
        shouldCheck = false;
      }
    } catch {}

    if (!shouldCheck) return;

    void runCheck({ silent: true });
    intervalRef.current = window.setInterval(() => {
      void runCheck({ silent: true });
    }, CHECK_INTERVAL_MS);

    const onFocus = () => {
      void runCheck({ silent: true });
    };
    window.addEventListener("focus", onFocus);

    return () => {
      if (intervalRef.current !== null) {
        window.clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
      window.removeEventListener("focus", onFocus);
    };
  }, [runCheck]);

  // Subscribe to backend progress events while mounted.
  useEffect(() => {
    let progressUnlisten: UnlistenFn | null = null;
    let finishedUnlisten: UnlistenFn | null = null;

    (async () => {
      progressUnlisten = await listen<UpdateProgress>(PROGRESS_EVENT, (e) => {
        setStatus((current) => {
          if (current.kind !== "downloading") return current;
          return {
            kind: "downloading",
            info: current.info,
            downloaded: e.payload.downloaded,
            total: e.payload.total,
          };
        });
      });
      finishedUnlisten = await listen(FINISHED_EVENT, () => {
        setStatus((current) => {
          const info = current.kind === "downloading" ? current.info : lastInfoRef.current;
          if (!info) return { kind: "idle" };
          return { kind: "ready", info };
        });
      });
    })();

    return () => {
      progressUnlisten?.();
      finishedUnlisten?.();
    };
  }, []);

  // Close the popover when clicking outside.
  const wrapRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    if (!open) return;
    const onDocClick = (ev: MouseEvent) => {
      if (!wrapRef.current) return;
      if (!wrapRef.current.contains(ev.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", onDocClick);
    return () => document.removeEventListener("mousedown", onDocClick);
  }, [open]);

  const onInstall = useCallback(async () => {
    const info = lastInfoRef.current;
    if (!info) return;
    // Hand off to the global <UpdaterLockScreen />: <App /> listens for
    // this event, swaps the whole window to the lock screen, and the
    // lock screen auto-starts the install. We don't kick the local
    // `downloading` state — by the next frame this <UpdateBadge /> is
    // unmounted along with the Workspace.
    window.dispatchEvent(
      new CustomEvent<{ info: UpdateInfo }>("sinew:install-update", {
        detail: { info },
      }),
    );
    setOpen(false);
  }, []);

  const onRestart = useCallback(async () => {
    try {
      await api.restartForUpdate();
    } catch (err) {
      setStatus({ kind: "error", message: String(err) });
    }
  }, []);

  const onPillClick = useCallback(() => {
    if (status.kind === "error") {
      // Retry path: re-run a check.
      void runCheck();
      return;
    }
    if (status.kind === "ready") {
      void onRestart();
      return;
    }
    if (status.kind === "available") {
      setOpen((value) => !value);
      return;
    }
    if (status.kind === "downloading") {
      setOpen((value) => !value);
      return;
    }
  }, [status, runCheck, onRestart]);

  // Render guard — keep the titlebar visually empty when there's nothing to say.
  if (status.kind === "idle" || status.kind === "checking") {
    return null;
  }

  const percent =
    status.kind === "downloading" && status.total && status.total > 0
      ? Math.min(100, Math.round((status.downloaded / status.total) * 100))
      : null;

  const dataState = statusKey(status);

  return (
    <div className="titlebar__update" ref={wrapRef} data-state={dataState}>
      <button
        type="button"
        className="titlebar__update-pill"
        onClick={onPillClick}
        title={pillTitle(status)}
      >
        <Icon icon={pillIcon(status)} width={12} height={12} />
        <span className="titlebar__update-label">{pillLabel(status, percent)}</span>
        {status.kind === "downloading" && (
          <span className="titlebar__update-bar" aria-hidden="true">
            <span
              className="titlebar__update-bar-fill"
              style={{
                width: percent !== null ? `${percent}%` : undefined,
                // When total is unknown, fall back to an animated indeterminate fill.
                animation:
                  percent === null
                    ? "updateBarIndeterminate 1.4s ease-in-out infinite"
                    : undefined,
              }}
            />
          </span>
        )}
      </button>

      {open && (status.kind === "available" || status.kind === "downloading") && (
        <UpdatePopover
          status={status}
          onInstall={onInstall}
          onDismiss={() => setOpen(false)}
        />
      )}
    </div>
  );
}

function UpdatePopover({
  status,
  onInstall,
  onDismiss,
}: {
  status: Extract<Status, { kind: "available" } | { kind: "downloading" }>;
  onInstall: () => void;
  onDismiss: () => void;
}) {
  const info = status.info;
  const isDownloading = status.kind === "downloading";

  const notes = useMemo(() => {
    const body = info.notes?.trim();
    if (!body) return null;
    // Plain-text safe rendering — we don't trust the manifest enough to dump
    // HTML/MD into the DOM. Splitting on newlines keeps formatting readable.
    return body.split(/\r?\n/);
  }, [info.notes]);

  return (
    <div className="titlebar__update-popover" role="dialog" aria-label="App update">
      <div className="titlebar__update-popover-head">
        <span className="titlebar__update-popover-title">
          New version {info.version}
        </span>
        <span className="titlebar__update-popover-sub">
          You are on {info.currentVersion}
        </span>
      </div>
      {notes && (
        <div className="titlebar__update-popover-notes">
          {notes.map((line, i) => (
            <p key={i}>{line || "\u00A0"}</p>
          ))}
        </div>
      )}
      <div className="titlebar__update-popover-actions">
        <button
          type="button"
          className="titlebar__update-popover-btn titlebar__update-popover-btn--ghost"
          onClick={onDismiss}
          disabled={isDownloading}
        >
          Later
        </button>
        <button
          type="button"
          className="titlebar__update-popover-btn titlebar__update-popover-btn--primary"
          onClick={onInstall}
          disabled={isDownloading}
        >
          {isDownloading ? "Downloading…" : "Install & restart"}
        </button>
      </div>
    </div>
  );
}

// ── helpers ──────────────────────────────────────────────────────────────

function statusKey(status: Status): string {
  return status.kind;
}

function pillIcon(status: Status): string {
  switch (status.kind) {
    case "available":
      return "solar:arrow-up-linear";
    case "downloading":
      return "solar:download-minimalistic-linear";
    case "ready":
      return "solar:restart-linear";
    case "error":
      return "solar:danger-triangle-linear";
    default:
      return "solar:refresh-linear";
  }
}

function pillLabel(status: Status, percent: number | null): string {
  switch (status.kind) {
    case "available":
      return `New version ${status.info.version ?? ""}`.trim();
    case "downloading":
      return percent !== null ? `Downloading ${percent}%` : "Downloading…";
    case "ready":
      return "Restart to update";
    case "error":
      return "Update failed";
    default:
      return "";
  }
}

function pillTitle(status: Status): string {
  switch (status.kind) {
    case "available":
      return `Update ${status.info.version} available — click to view and install`;
    case "downloading":
      return "Downloading the update…";
    case "ready":
      return "Update downloaded — click to restart and apply";
    case "error":
      return `Update failed: ${status.message}. Click to retry.`;
    default:
      return "";
  }
}
