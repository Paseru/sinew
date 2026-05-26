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

export function AIThinkingBlock({
  content,
  isStreaming = false,
  durationMs,
  onOpenFile,
}: Props) {
  const contentRef = useRef<HTMLDivElement>(null);
  const [isOpen, setIsOpen] = useState(() => {
    const compact = localStorage.getItem("sinew.compact-reasoning") === "true";
    if (compact) return false;
    return isStreaming && content.trim().length > 0;
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
      setIsOpen(false);
    }
    prevStreamingRef.current = isStreaming;
  }, [isStreaming]);

  useEffect(() => {
    const compact = localStorage.getItem("sinew.compact-reasoning") === "true";
    if (compact) return;
    if (isStreaming && hasContent && !prevHasContentRef.current) {
      setIsOpen(true);
    }
    prevHasContentRef.current = hasContent;
  }, [hasContent, isStreaming]);

  useEffect(() => {
    const handler = (event: Event) => {
      const enabled = (event as CustomEvent<boolean>).detail;
      if (enabled) {
        setIsOpen(false);
      }
    };
    window.addEventListener("sinew:compact-reasoning-changed", handler);
    return () => window.removeEventListener("sinew:compact-reasoning-changed", handler);
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
