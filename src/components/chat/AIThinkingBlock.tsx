import { useEffect, useRef, useState } from "react";
import { Icon } from "@iconify/react";
import { DotmSquare2 } from "./DotmSquare2";
import { Markdown } from "./Markdown";

type Props = {
  content: string;
  isStreaming?: boolean;
  durationMs?: number;
  onOpenFile?: (path: string) => void;
};

const noopOpenFile = () => {};

function readCompactReasoning(): "disabled" | "compact" | "very-compact" {
  try {
    const master = localStorage.getItem("sinew.power-user-master") || "enabled";
    if (master === "enabled") return "very-compact";
    if (master === "disabled") return "disabled";

    const val = localStorage.getItem("sinew.compact-reasoning");
    if (val === "very-compact") return "very-compact";
    if (val === "compact" || val === "true") return "compact";
    return "disabled";
  } catch {
    return "disabled";
  }
}

export function AIThinkingBlock({
  content,
  isStreaming = false,
  durationMs,
  onOpenFile,
}: Props) {
  const contentRef = useRef<HTMLDivElement>(null);
  const [compactMode, setCompactMode] = useState<"disabled" | "compact" | "very-compact">(() => {
    return readCompactReasoning();
  });
  const [isOpen, setIsOpen] = useState(() => {
    if (compactMode === "compact" || compactMode === "very-compact") return false;
    return true;
  });
  const prevStreamingRef = useRef(isStreaming);
  const prevHasContentRef = useRef(false);
  const hasContent = content.trim().length > 0;
  const contentOpen = hasContent && isOpen;
  const durationLabel =
    durationMs !== undefined
      ? `Thought for ${(Math.floor(Math.max(0, durationMs) / 100) / 10).toFixed(1)}s`
      : "Thought";

  useEffect(() => {
    if (prevStreamingRef.current && !isStreaming) {
      if (compactMode !== "disabled") {
        setIsOpen(false);
      }
    }
    prevStreamingRef.current = isStreaming;
  }, [isStreaming, compactMode]);

  useEffect(() => {
    if (compactMode === "compact" || compactMode === "very-compact") return;
    if (isStreaming && hasContent && !prevHasContentRef.current) {
      setIsOpen(true);
    }
    prevHasContentRef.current = hasContent;
  }, [hasContent, isStreaming, compactMode]);

  useEffect(() => {
    const handler = (event: Event) => {
      const mode = (event as CustomEvent<"disabled" | "compact" | "very-compact">).detail;
      setCompactMode(mode);
      if (mode === "compact" || mode === "very-compact") {
        setIsOpen(false);
      } else {
        setIsOpen(true);
      }
    };
    window.addEventListener("sinew:compact-reasoning-changed", handler as any);
    return () => window.removeEventListener("sinew:compact-reasoning-changed", handler as any);
  }, []);

  useEffect(() => {
    if (!contentOpen || !isStreaming) return;
    const el = contentRef.current;
    if (!el) return;
    const raf = requestAnimationFrame(() => {
      el.scrollTop = el.scrollHeight;
    });
    return () => cancelAnimationFrame(raf);
  }, [content, contentOpen, isStreaming]);

  if (!hasContent) return null;

  if (compactMode === "very-compact") {
    if (!isStreaming) {
      return null;
    }
    return (
      <div className="thinking-block" style={{ paddingLeft: "10px", paddingTop: "4px" }}>
        <DotmSquare2
          speed={1}
          animated
          className="thinking-block__matrix"
        />
      </div>
    );
  }

  return (
    <div className="thinking-block">
      <button
        type="button"
        onClick={() => setIsOpen((o) => !o)}
        className="thinking-block__head"
        data-streaming={isStreaming ? "true" : "false"}
        data-has-content={hasContent ? "true" : "false"}
        aria-expanded={contentOpen}
      >
        <Icon
          icon="solar:alt-arrow-right-linear"
          width={12}
          height={12}
          className="thinking-block__caret"
          data-open={contentOpen ? "true" : "false"}
        />
        {isStreaming && (
          <DotmSquare2
            speed={1}
            animated
            className="thinking-block__matrix"
          />
        )}
        <span
          className="thinking-block__label"
          data-streaming={isStreaming ? "true" : "false"}
        >
          {isStreaming ? "Thinking" : durationLabel}
        </span>
      </button>

      {contentOpen && (
        <div
          className="thinking-block__fade"
          data-streaming={isStreaming ? "true" : "false"}
        >
          <div ref={contentRef} className="thinking-block__content">
            <div className="thinking-block__text">
              <Markdown
                text={content.trimStart()}
                onOpenFile={onOpenFile ?? noopOpenFile}
              />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
