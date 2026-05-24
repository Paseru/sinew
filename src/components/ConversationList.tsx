import { useRef, useState } from "react";
import { Icon } from "@iconify/react";
import type { ConversationSummary } from "../types";

type Props = {
  conversations: ConversationSummary[];
  activeId: string | null;
  streamingIds: ReadonlySet<string>;
  onSelect: (id: string) => void;
  onRename: (id: string, title: string) => void;
  onDelete: (id: string) => void;
};

// Renders only the body of the conversations list. The parent owns the
// `sidebar__section` shell and the head (now shared with the Git tab),
// so this component focuses on the actual list of rows.
export function ConversationList({
  conversations,
  activeId,
  streamingIds,
  onSelect,
  onRename,
  onDelete,
}: Props) {
  const [editingId, setEditingId] = useState<string | null>(null);
  const editRef = useRef<HTMLSpanElement | null>(null);

  const commitRename = (id: string) => {
    const value = editRef.current?.textContent?.trim() ?? "";
    setEditingId(null);
    if (value) {
      onRename(id, value);
    }
  };

  return (
    <div className="sidebar__body">
      <div className="conv-list">
        {conversations.length === 0 && (
          <div className="conv-empty">No conversations yet.</div>
        )}
        {conversations.map((conv) => {
          const isEditing = editingId === conv.id;
          const isActive = activeId === conv.id;
          const isStreaming = streamingIds.has(conv.id);
          return (
            <div
              key={conv.id}
              className="conv-row"
              data-active={isActive ? "true" : "false"}
              data-streaming={isStreaming ? "true" : "false"}
              onClick={() => !isEditing && onSelect(conv.id)}
            >
              <span className="conv-row__icon">
                {isStreaming ? (
                  <span className="conv-row__spinner" aria-label="Streaming" />
                ) : (
                  <Icon
                    icon={
                      isActive
                        ? "solar:chat-round-dots-bold"
                        : "solar:chat-round-dots-linear"
                    }
                    width={15}
                    height={15}
                  />
                )}
              </span>
              <span
                ref={isEditing ? editRef : undefined}
                className="conv-row__title"
                contentEditable={isEditing}
                suppressContentEditableWarning
                onKeyDown={(event) => {
                  if (!isEditing) return;
                  if (event.key === "Enter") {
                    event.preventDefault();
                    commitRename(conv.id);
                  } else if (event.key === "Escape") {
                    setEditingId(null);
                  }
                }}
                onBlur={() => {
                  if (isEditing) commitRename(conv.id);
                }}
              >
                {conv.title || "Untitled"}
              </span>
              <span className="conv-row__actions">
                <button
                  className="conv-row__btn"
                  title="Rename"
                  onClick={(event) => {
                    event.stopPropagation();
                    setEditingId(conv.id);
                    queueMicrotask(() => {
                      const node = editRef.current;
                      if (node) {
                        node.focus();
                        const sel = window.getSelection();
                        const range = document.createRange();
                        range.selectNodeContents(node);
                        sel?.removeAllRanges();
                        sel?.addRange(range);
                      }
                    });
                  }}
                >
                  <Icon icon="solar:pen-linear" width={13} height={13} />
                </button>
                <button
                  className="conv-row__btn conv-row__btn--danger"
                  title="Delete"
                  onClick={(event) => {
                    event.stopPropagation();
                    if (confirm("Delete this conversation?")) {
                      onDelete(conv.id);
                    }
                  }}
                >
                  <Icon icon="solar:trash-bin-minimalistic-linear" width={13} height={13} />
                </button>
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
