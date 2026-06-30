import { useCallback, useEffect, useRef } from "react";
import { Icon } from "@iconify/react";
import { api } from "../lib/ipc";
import { SinewMark } from "./SinewMark";

/// Destination for the primary CTA. Opened through Tauri's external-URL
/// command so it lands in the user's real browser rather than the wry shell;
/// `window.open` is a defensive fallback for non-Tauri / web-preview contexts.
const SINEW_DESIGN_URL = "https://sinewdesign.com/";

type Props = {
  /// Dismiss handler — wired to the close button, the backdrop, and Escape.
  onDismiss: () => void;
};

/// Launch popup introducing Sinew Design, shown once per app session after the
/// boot updater gate resolves (never over the updater lock screen). Purely
/// informational + a single outbound CTA; fully dismissible.
export function SinewDesignDialog({ onDismiss }: Props) {
  const ctaRef = useRef<HTMLButtonElement | null>(null);

  // Open the marketing site in the system browser. Prefer the Tauri command;
  // fall back to window.open so the CTA still works outside the desktop shell.
  const openSite = useCallback(() => {
    void api.openExternalUrl(SINEW_DESIGN_URL).catch(() => {
      try {
        window.open(SINEW_DESIGN_URL, "_blank", "noopener,noreferrer");
      } catch {
        // Nothing else we can do; leave the dialog open so the user can retry.
      }
    });
    onDismiss();
  }, [onDismiss]);

  // Escape closes the dialog. Focus the CTA on mount so keyboard users land
  // on the primary action immediately.
  useEffect(() => {
    ctaRef.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onDismiss();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onDismiss]);

  return (
    <div
      className="sinew-design__backdrop"
      role="presentation"
      onMouseDown={(event) => {
        // Dismiss only on a click that both starts and ends on the backdrop
        // itself — not on a drag that bubbles up from inside the dialog.
        if (event.target === event.currentTarget) onDismiss();
      }}
    >
      <div
        className="sinew-design__dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="sinew-design-title"
        aria-describedby="sinew-design-desc"
      >
        {/* Decorative depth layers — purely visual, never interactive, so the
            backdrop-click and focus model below stay exactly as before. */}
        <div className="sinew-design__aurora" aria-hidden="true" />
        <div className="sinew-design__grid" aria-hidden="true" />
        <div className="sinew-design__sheen" aria-hidden="true" />

        <button
          type="button"
          className="sinew-design__close"
          onClick={onDismiss}
          aria-label="Dismiss"
        >
          <Icon icon="solar:close-circle-linear" width={15} height={15} />
        </button>

        <div className="sinew-design__body">
          <div className="sinew-design__mark" aria-hidden="true">
            <span className="sinew-design__mark-halo" />
            <span className="sinew-design__mark-tile">
              <SinewMark size={30} />
            </span>
          </div>

          <div className="sinew-design__eyebrow">
            <span className="sinew-design__eyebrow-glyph" aria-hidden="true">
              <SinewMark size={11} />
            </span>
            Part of the Sinew ecosystem
          </div>

          <h2 id="sinew-design-title" className="sinew-design__title">
            Meet Sinew Design
          </h2>

          <p id="sinew-design-desc" className="sinew-design__desc">
            A landing-page studio in the Sinew ecosystem. Skip the generic AI
            slop and ship polished, intentional pages that are built to
            convert.
          </p>

          <div className="sinew-design__actions">
            <button
              type="button"
              ref={ctaRef}
              className="sinew-design__cta"
              onClick={openSite}
            >
              <span className="sinew-design__cta-label">
                Explore Sinew Design
                <Icon
                  icon="solar:arrow-right-up-linear"
                  width={15}
                  height={15}
                />
              </span>
            </button>
            <button
              type="button"
              className="sinew-design__dismiss"
              onClick={onDismiss}
            >
              Maybe later
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
