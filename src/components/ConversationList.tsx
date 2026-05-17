import { useRef, useState } from "react";
import { Icon } from "@iconify/react";
import { useLanguage } from "../lib/i18n";
import type { ConversationSummary } from "../types";

type Props = {
  conversations: ConversationSummary[];
  activeId: string | null;
  streamingIds: ReadonlySet<string>;
  onSelect: (id: string) => void;
  onCreate: () => void;
  onRename: (id: string, title: string) => void;
  onDelete: (id: string) => void;
};

export function ConversationList({
  conversations,
  activeId,
  streamingIds,
  onSelect,
  onCreate,
  onRename,
  onDelete,
}: Props) {
  const language = useLanguage();
  const copy = conversationCopy[language];
  const [editingId, setEditingId] = useState<string | null>(null);
  const editRef = useRef<HTMLSpanElement | null>(null);

  const displayTitle = (title: string) => {
    const trimmed = title.trim();
    if (!trimmed) return copy.untitled;
    return trimmed === "New conversation" ? copy.newConversation : trimmed;
  };

  const commitRename = (id: string) => {
    const value = editRef.current?.textContent?.trim() ?? "";
    setEditingId(null);
    if (value) {
      onRename(id, value);
    }
  };

  return (
    <div className="sidebar__section" style={{ flex: "1 1 0" }}>
      <div className="sidebar__head">
        <span className="sidebar__head-title">
          <Icon icon="solar:chat-round-dots-bold-duotone" width={16} height={16} />
          <span>{copy.title}</span>
        </span>
        <button
          className="sidebar__head-btn"
          onClick={onCreate}
          title={copy.newConversation}
        >
          <Icon icon="solar:add-square-linear" width={15} height={15} />
        </button>
      </div>
      <div className="sidebar__body">
        <div className="conv-list">
          {conversations.length === 0 && (
            <div className="conv-empty">{copy.empty}</div>
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
                    <span className="conv-row__spinner" aria-label={copy.streaming} />
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
                  {displayTitle(conv.title)}
                </span>
                <span className="conv-row__actions">
                  <button
                    className="conv-row__btn"
                    title={copy.rename}
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
                    title={copy.delete}
                    onClick={(event) => {
                      event.stopPropagation();
                      if (confirm(copy.deleteConfirm)) {
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
    </div>
  );
}
const conversationCopy = {
  en: {
    title: "Conversations",
    newConversation: "New conversation",
    empty: "No conversations yet.",
    streaming: "Streaming",
    untitled: "Untitled",
    rename: "Rename",
    delete: "Delete",
    deleteConfirm: "Delete this conversation?",
  },
  fr: {
    title: "Conversations",
    newConversation: "Nouvelle conversation",
    empty: "Aucune conversation pour le moment.",
    streaming: "Génération en cours",
    untitled: "Sans titre",
    rename: "Renommer",
    delete: "Supprimer",
    deleteConfirm: "Supprimer cette conversation ?",
  },
} as const;
