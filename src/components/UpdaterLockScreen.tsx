import { useCallback, useEffect, useMemo, useState } from "react";
import { Icon } from "@iconify/react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api } from "../lib/ipc";
import type { UpdateInfo, UpdateProgress } from "../types";
import { ClaakeCodeMark } from "./ClaakeCodeMark";
import { WindowControls, isWindowsPlatform } from "./WindowControls";

/// Tauri events emitted by `src-tauri/src/updater.rs::updater_download_and_install`.
const PROGRESS_EVENT = "updater://progress";
const FINISHED_EVENT = "updater://finished";

/// How long we wait on the "Update ready" screen before triggering the
/// restart ourselves. The user can short-circuit with the "Restart now"
/// button — both code paths call `updater_restart`.
const AUTO_RESTART_SECS = 3;

const IS_WINDOWS = isWindowsPlatform();

type Phase =
  | { kind: "prompt" }
  | { kind: "downloading"; downloaded: number; total: number | null }
  | { kind: "installing" }
  | { kind: "ready" }
  | { kind: "error"; message: string };

type Props = {
  /// Update descriptor pre-loaded by the boot check in `App.tsx`. Holds the
  /// version string, the current installed version, release notes, and the
  /// optional publish date. The screen reuses these to render headers and
  /// the install CTA label.
  info: UpdateInfo;
  /// When `true`, the screen skips the "prompt" phase entirely and kicks
  /// off the install on mount. Used when the gate is opened from the
  /// in-session <UpdateBadge /> (the user has already confirmed in the
  /// popover — surfacing the prompt again would feel like a re-ask).
  /// Defaults to `false` (boot flow).
  autoInstall?: boolean;
};

/// Full-screen, unbypassable update gate. Mounted by `App.tsx` when the boot
/// updater check reports an available release. The user can only:
///   * **Install** the update (download → install → auto-restart).
///   * **Quit Claake Code** (the OS-level close button on Windows; on macOS/Linux,
///     the system traffic lights, plus an explicit button on the error
///     path).
///
/// There is no "Later", no "Skip" — the policy is "update or quit". While
/// downloading/installing, the only escape is closing the window through
/// the OS chrome. The component owns the entire lifecycle: it kicks off
/// the install, listens to `updater://progress` / `updater://finished`,
/// runs the auto-restart countdown, and surfaces errors with a Retry
/// path.
export function UpdaterLockScreen({ info, autoInstall = false }: Props) {
  const [phase, setPhase] = useState<Phase>({ kind: "prompt" });
  const [countdown, setCountdown] = useState<number>(AUTO_RESTART_SECS);

  // Subscribe to backend update events for the lifetime of the screen.
  // We never tear this down between phases — it costs nothing and keeps
  // the wiring simple. The reducer-ish state transitions inside the
  // handlers guarantee we won't downgrade `ready`/`error` back to
  // `downloading` if a late progress tick arrives.
  useEffect(() => {
    let progressUnlisten: UnlistenFn | null = null;
    let finishedUnlisten: UnlistenFn | null = null;
    let mounted = true;

    (async () => {
      const pUn = await listen<UpdateProgress>(PROGRESS_EVENT, (e) => {
        const { downloaded, total } = e.payload;
        setPhase((cur) => {
          if (cur.kind === "ready" || cur.kind === "error") return cur;
          // Once the entire bundle is downloaded, flip to the
          // indeterminate "installing" state so the bar doesn't sit at
          // 100% for the (sometimes long) tail of the install step.
          if (total !== null && total > 0 && downloaded >= total) {
            return { kind: "installing" };
          }
          return { kind: "downloading", downloaded, total };
        });
      });
      const fUn = await listen(FINISHED_EVENT, () => {
        setPhase((cur) => (cur.kind === "error" ? cur : { kind: "ready" }));
      });
      if (!mounted) {
        pUn();
        fUn();
        return;
      }
      progressUnlisten = pUn;
      finishedUnlisten = fUn;
    })();

    return () => {
      mounted = false;
      progressUnlisten?.();
      finishedUnlisten?.();
    };
  }, []);

  // Auto-restart countdown. Wires up when entering the `ready` phase and
  // tears down on every other phase. We rely on the functional setState
  // form so the interval can read the latest value without restarting.
  useEffect(() => {
    if (phase.kind !== "ready") return;
    setCountdown(AUTO_RESTART_SECS);
    const tick = window.setInterval(() => {
      setCountdown((c) => {
        if (c <= 1) {
          window.clearInterval(tick);
          void api.restartForUpdate();
          return 0;
        }
        return c - 1;
      });
    }, 1000);
    return () => window.clearInterval(tick);
  }, [phase.kind]);

  const startInstall = useCallback(async () => {
    setPhase({ kind: "downloading", downloaded: 0, total: null });
    try {
      await api.installUpdate();
      // Safety net — if the FINISHED event was missed (slow webview,
      // permission glitch), flip to `ready` so the user isn't stuck on
      // an "Installing…" screen forever.
      setPhase((cur) =>
        cur.kind === "downloading" || cur.kind === "installing"
          ? { kind: "ready" }
          : cur,
      );
    } catch (err) {
      setPhase({ kind: "error", message: String(err) });
    }
  }, []);

  const onRestart = useCallback(() => {
    void api.restartForUpdate();
  }, []);

  // Auto-kick the install when the screen was opened mid-session from
  // the badge (the user already confirmed in the popover). We gate on
  // `phase.kind === "prompt"` so a re-render of the effect (e.g. on
  // hot-reload) doesn't loop us back into a fresh install once we've
  // moved past the initial phase. `startInstall` is stable (empty deps
  // in its `useCallback`), so in practice this effect fires once on
  // mount.
  useEffect(() => {
    if (!autoInstall) return;
    if (phase.kind !== "prompt") return;
    void startInstall();
  }, [autoInstall, phase.kind, startInstall]);

  const onQuit = useCallback(() => {
    void getCurrentWindow()
      .close()
      .catch(() => {
        // close() fails silently on weird WebView states; the user can
        // still use the OS chrome to kill the window.
      });
  }, []);

  // Derived UI bits — easier to read than inlined ternaries down below.
  const percent = useMemo(() => {
    if (phase.kind !== "downloading") return null;
    if (phase.total === null || phase.total <= 0) return null;
    return Math.min(100, Math.round((phase.downloaded / phase.total) * 100));
  }, [phase]);

  const isBusy = phase.kind === "downloading" || phase.kind === "installing";

  return (
    <div className="updater-lock" role="dialog" aria-label="Update required">
      {IS_WINDOWS && (
        /* Frameless Windows shell needs a drag region + custom controls,
           same pattern as Welcome. We expose the controls so the user can
           still minimize/close — the lock screen is a UX gate, not a
           prison. */
        <div className="updater-lock__titlebar" data-tauri-drag-region>
          <WindowControls />
        </div>
      )}

      <main className="updater-lock__stage">
        <header className="updater-lock__head">
          <span className="updater-lock__mark-dot" aria-hidden="true">
            <span
              className={
                "updater-lock__mark-inner" +
                (isBusy ? " updater-lock__mark-inner--pulse" : "") +
                (phase.kind === "ready"
                  ? " updater-lock__mark-inner--ok"
                  : "") +
                (phase.kind === "error"
                  ? " updater-lock__mark-inner--err"
                  : "")
              }
            >
              {phase.kind === "ready" ? (
                <Icon icon="solar:check-circle-bold" width={22} height={22} />
              ) : phase.kind === "error" ? (
                <Icon
                  icon="solar:danger-triangle-bold"
                  width={22}
                  height={22}
                />
              ) : (
                <ClaakeCodeMark size={22} className="updater-lock__mark-glyph" />
              )}
            </span>
          </span>
          <h1 className="updater-lock__title">{titleFor(phase)}</h1>
          <p className="updater-lock__sub">{subFor(phase, info, countdown)}</p>
        </header>

        {phase.kind === "prompt" && info.notes && (
          <ReleaseNotes notes={info.notes} />
        )}

        {(phase.kind === "downloading" || phase.kind === "installing") && (
          <div className="updater-lock__card">
            <div
              className={
                "updater-lock__bar" +
                (phase.kind === "installing" || percent === null
                  ? " updater-lock__bar--indeterminate"
                  : "")
              }
              aria-hidden="true"
            >
              <span
                className="updater-lock__bar-fill"
                style={{
                  width:
                    percent !== null && phase.kind === "downloading"
                      ? `${percent}%`
                      : undefined,
                }}
              />
            </div>
            <div className="updater-lock__bar-meta">
              <span className="updater-lock__bar-state">
                {phase.kind === "installing"
                  ? "Finalizing installation…"
                  : percent !== null
                    ? `Downloading · ${percent}%`
                    : "Downloading…"}
              </span>
              {phase.kind === "downloading" && phase.total !== null && (
                <span className="updater-lock__bar-bytes">
                  {formatBytes(phase.downloaded)} / {formatBytes(phase.total)}
                </span>
              )}
            </div>
          </div>
        )}

        {phase.kind === "error" && (
          <div className="updater-lock__card updater-lock__card--error">
            <p className="updater-lock__error">{phase.message}</p>
          </div>
        )}

        <footer className="updater-lock__actions">
          {phase.kind === "prompt" && (
            <button
              type="button"
              className="updater-lock__cta"
              onClick={startInstall}
              autoFocus
            >
              <span className="updater-lock__cta-label">
                Install update {info.version}
              </span>
              <span className="updater-lock__cta-chev">
                <Icon
                  icon="solar:alt-arrow-right-linear"
                  width={16}
                  height={16}
                />
              </span>
            </button>
          )}
          {phase.kind === "ready" && (
            <button
              type="button"
              className="updater-lock__cta"
              onClick={onRestart}
              autoFocus
            >
              <span className="updater-lock__cta-label">Restart now</span>
              <span className="updater-lock__cta-chev">
                <Icon icon="solar:restart-linear" width={16} height={16} />
              </span>
            </button>
          )}
          {phase.kind === "error" && (
            <>
              <button
                type="button"
                className="updater-lock__btn updater-lock__btn--ghost"
                onClick={onQuit}
              >
                Quit Claake Code
              </button>
              <button
                type="button"
                className="updater-lock__btn updater-lock__btn--primary"
                onClick={startInstall}
                autoFocus
              >
                Retry
              </button>
            </>
          )}
        </footer>
      </main>
    </div>
  );
}

/// Plain-text release notes block. We don't trust the manifest enough to dump
/// HTML/MD into the DOM (same policy as <UpdateBadge />'s popover), so we
/// split on newlines and render each line as its own paragraph.
function ReleaseNotes({ notes }: { notes: string }) {
  const lines = useMemo(() => notes.trim().split(/\r?\n/), [notes]);
  return (
    <div
      className="updater-lock__notes"
      role="region"
      aria-label="Release notes"
    >
      {lines.map((line, i) => (
        <p key={i}>{line || "\u00A0"}</p>
      ))}
    </div>
  );
}

// ── helpers ──────────────────────────────────────────────────────────────

function titleFor(phase: Phase): string {
  switch (phase.kind) {
    case "prompt":
      return "Update required";
    case "downloading":
      return "Downloading update";
    case "installing":
      return "Finalizing installation";
    case "ready":
      return "Update ready";
    case "error":
      return "Update failed";
  }
}

function subFor(phase: Phase, info: UpdateInfo, countdown: number): string {
  switch (phase.kind) {
    case "prompt":
      return `Claake Code ${info.version ?? ""} · you're on ${info.currentVersion}`;
    case "downloading":
      return `${info.currentVersion} → ${info.version ?? ""}`;
    case "installing":
      return "Keep Claake Code open — almost there";
    case "ready":
      return countdown > 0
        ? `Restarting in ${countdown}s…`
        : "Restarting…";
    case "error":
      return "We couldn't install the update. Claake Code can't start until it's done.";
  }
}

/// Compact byte formatter shared with the progress meta line. Uses binary
/// units (KB = 1024) to match what most installers and OS file dialogs
/// display, so the numbers line up with what users see elsewhere.
function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}
