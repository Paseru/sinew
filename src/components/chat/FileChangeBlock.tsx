import { useEffect, useRef, useState } from "react";
import { Icon } from "@iconify/react";
import { fileIcon } from "../../lib/fileIcon";
import type { FileChange } from "../../types";

function basename(path: string): string {
  const i = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return i >= 0 ? path.slice(i + 1) : path;
}

type RenderedDiffLineKind = "context" | "added" | "removed";

function normalizeDiffLineKind(kind: string): RenderedDiffLineKind {
  const raw = kind.toLowerCase();
  if (raw === "added" || raw === "insert") return "added";
  if (raw === "removed" || raw === "delete" || raw === "deleted") {
    return "removed";
  }
  return "context";
}

export function FileChangeBlock({
  change,
  live = false,
}: {
  change: FileChange;
  live?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const name = basename(change.relativePath);
  const visibleLineKinds = change.lines.map((line) =>
    normalizeDiffLineKind(line.kind),
  );
  const firstChangedLineIndex = visibleLineKinds.findIndex(
    (kind) => kind !== "context",
  );
  const previewAdded = change.lines.reduce(
    (n, _line, idx) => (visibleLineKinds[idx] === "added" ? n + 1 : n),
    0,
  );
  const previewRemoved = change.lines.reduce(
    (n, _line, idx) => (visibleLineKinds[idx] === "removed" ? n + 1 : n),
    0,
  );
  const added = change.addedLines ?? previewAdded;
  const removed = change.removedLines ?? previewRemoved;
  const hasTextStats = added > 0 || removed > 0;
  const hasVisibleDiff = visibleLineKinds.some((kind) => kind !== "context");
  const hasOnlyContextPreview =
    hasTextStats && change.lines.length > 0 && !hasVisibleDiff;
  const quietLabel = !hasTextStats
    ? change.binary
      ? "binary"
      : "no text diff"
    : null;

  useEffect(() => {
    if (!open || firstChangedLineIndex < 0 || live) return undefined;
    const body = bodyRef.current;
    if (!body) return undefined;
    const frame = window.requestAnimationFrame(() => {
      const firstChange = body.querySelector<HTMLElement>(
        '[data-first-change="true"]',
      );
      if (!firstChange) return;
      body.scrollTop = Math.max(0, firstChange.offsetTop - body.offsetTop);
    });
    return () => window.cancelAnimationFrame(frame);
  }, [firstChangedLineIndex, open, live]);

  useEffect(() => {
    if (!live || !open) return undefined;
    const body = bodyRef.current;
    if (!body) return undefined;
    const frame = window.requestAnimationFrame(() => {
      body.scrollTop = body.scrollHeight;
    });
    return () => window.cancelAnimationFrame(frame);
  }, [change.lines.length, open, live]);

  return (
    <div className="filechange">
      <div className="filechange__head" onClick={() => setOpen((v) => !v)}>
        <span
          className="tab__icon"
          style={{ color: "var(--text-2)", flex: "0 0 14px" }}
        >
          <Icon icon={fileIcon(name)} width={14} height={14} />
        </span>
        <span className="filechange__path" title={change.relativePath}>
          {name}
        </span>
        {hasTextStats && (
          <span className="filechange__stats">
            {added > 0 && (
              <span className="filechange__stat" data-kind="added">
                +{added}
              </span>
            )}
            {removed > 0 && (
              <span className="filechange__stat" data-kind="removed">
                -{removed}
              </span>
            )}
          </span>
        )}
        {quietLabel && <span className="filechange__badge">{quietLabel}</span>}
        <span className="filechange__caret">
          <Icon
            icon={
              open
                ? "solar:alt-arrow-down-bold"
                : "solar:alt-arrow-right-bold"
            }
            width={14}
            height={14}
          />
        </span>
      </div>
      {open && (
        <div className="filechange__body" ref={bodyRef}>
          {change.binary ? (
            <div style={{ padding: "10px 12px", color: "var(--editor-fg-dim)" }}>
              Binary file, diff omitted.
            </div>
          ) : change.lines.length === 0 ? (
            <div style={{ padding: "10px 12px", color: "var(--editor-fg-dim)" }}>
              No text diff available. {change.summary}
            </div>
          ) : hasOnlyContextPreview ? (
            <div style={{ padding: "10px 12px", color: "var(--editor-fg-dim)" }}>
              Text diff preview unavailable. {change.summary}
            </div>
          ) : (
            <>
              {change.lines.map((line, idx) => {
                const kind = visibleLineKinds[idx];
                return (
                  <span
                    key={idx}
                    className="diff-line"
                    data-kind={kind}
                    data-first-change={
                      idx === firstChangedLineIndex ? "true" : undefined
                    }
                  >
                    {kind === "added" ? "+ " : kind === "removed" ? "- " : "  "}
                    {line.text.replace(/\n$/, "")}
                  </span>
                );
              })}
              {change.truncated && (
                <span
                  className="diff-line"
                  style={{ color: "var(--editor-fg-dim)" }}
                >
                  … diff truncated
                </span>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}
