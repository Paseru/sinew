import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { Icon } from "@iconify/react";
import type { ChatBlock } from "./stream";
import { Markdown } from "./Markdown";
import { canonicalToolName, isToolName } from "../../lib/tools";
import type { AttachmentInput, TodoStatus } from "../../types";

type ParsedTask = {
  id: string;
  status: TodoStatus;
  text: string;
};

type TeamTaskStatus = "pending" | "in_progress" | "blocked" | "completed";

type ParsedTeamTask = {
  id: string;
  status: TeamTaskStatus;
  text: string;
  owner?: string;
  detail?: string;
  updatedAtMs?: number;
};

type ParsedTodo = {
  state: "active" | "closed";
  tasks: ParsedTask[];
};

type ParsedTeamTasks = {
  teamName?: string;
  tasks: ParsedTeamTask[];
  blockId: string;
};

type ParsedTeamMessage = {
  id?: string;
  from?: string;
  to?: string;
  message: string;
  summary?: string;
  blockId: string;
};

type ParsedTeamMessages = {
  messages: ParsedTeamMessage[];
  blockId: string;
};

export type QueuedPromptStripItem = {
  id: string;
  text: string;
  attachments: AttachmentInput[];
};

type ActivePanel = "queue" | "todo" | "team" | "messages";
const TEAM_TASKS_LABEL = "Agent Swarm tasks";
const TEAM_MESSAGES_LABEL = "Messages";
const QUEUE_LABEL = "Queue";
const TODO_STRIP_MIN_HEIGHT = 92;
const TODO_STRIP_TOP_GAP = 48;

function parseTodoOutput(output?: string): ParsedTodo | null {
  if (!output) return null;
  const state = output.match(/^state:\s*(active|closed)$/m)?.[1] as
    | "active"
    | "closed"
    | undefined;
  if (!state) return null;
  const tasks: ParsedTask[] = [];
  for (const match of output.matchAll(
    /^\s*(\d+)\.\s*\[(pending|in_progress|done)\]\s*(.+)$/gm,
  )) {
    tasks.push({
      id: match[1],
      status: match[2] as TodoStatus,
      text: match[3].trim(),
    });
  }
  return { state, tasks };
}

function parseTeamTasksFromOutput(output?: string): {
  teamName?: string;
  tasks: ParsedTeamTask[];
} | null {
  if (!output) return null;
  const teamName = output.match(/^team:\s*(.+)$/m)?.[1]?.trim();
  const tasks = new Map<string, ParsedTeamTask>();
  for (const match of output.matchAll(
    /^\s*-\s*#(\d+)\s+\[(pending|in_progress|blocked|completed)\]\s*(?:@([^\s]+)\s+)?(.+?)\s*(?:\((.+)\))?\s*$/gm,
  )) {
    tasks.set(match[1], {
      id: match[1],
      status: match[2] as TeamTaskStatus,
      owner: match[3]?.trim(),
      text: match[4].trim(),
      detail: normalizeTeamTaskDetail(match[5]?.trim()),
    });
  }
  for (const match of output.matchAll(
    /^Task\s+#(\d+)\s+created successfully:\s*(.+)$/gm,
  )) {
    if (!tasks.has(match[1])) {
      tasks.set(match[1], {
        id: match[1],
        status: "pending",
        text: match[2].trim(),
      });
    }
  }
  if (tasks.size === 0) return null;
  return {
    teamName,
    tasks: Array.from(tasks.values()).sort(
      (left, right) => Number(left.id) - Number(right.id),
    ),
  };
}

function parseTeamTasksFromMeta(
  meta?: Record<string, unknown> | null,
): {
  teamName?: string;
  tasks: ParsedTeamTask[];
} | null {
  const team = meta?.team;
  if (!team || typeof team !== "object") return null;
  const record = team as Record<string, unknown>;
  const rawTasks = Array.isArray(record.tasks) ? record.tasks : [];
  const tasks = rawTasks
    .map((raw): ParsedTeamTask | null => {
      if (!raw || typeof raw !== "object") return null;
      const task = raw as Record<string, unknown>;
      const id =
        typeof task.id === "number" || typeof task.id === "string"
          ? String(task.id)
          : "";
      const text = typeof task.subject === "string" ? task.subject.trim() : "";
      const status =
        typeof task.status === "string" ? task.status.trim() : "";
      if (!id || !text || !isTeamTaskStatus(status)) return null;
      const blockedBy = Array.isArray(task.blockedBy)
        ? task.blockedBy
            .map((value) =>
              typeof value === "number" || typeof value === "string"
                ? `#${value}`
                : "",
            )
            .filter(Boolean)
        : [];
      const detail =
        blockedBy.length > 0 ? `blocked by ${blockedBy.join(", ")}` : "";
      const updatedAtMs = numberFromUnknown(task.updatedAtMs);
      return {
        id,
        status,
        text,
        owner:
          typeof task.owner === "string" && task.owner.trim()
            ? task.owner.trim()
            : undefined,
        detail: detail || undefined,
        updatedAtMs,
      };
    })
    .filter((task): task is ParsedTeamTask => !!task)
    .sort((left, right) => Number(left.id) - Number(right.id));
  if (tasks.length === 0) return null;
  return {
    teamName: typeof record.name === "string" ? record.name : undefined,
    tasks,
  };
}

function normalizeTeamTaskDetail(value?: string): string | undefined {
  const normalized = value
    ?.replace(/\bblocked:\s*/gi, "")
    .replace(/;\s*/g, " · ")
    .trim();
  if (!normalized) return undefined;
  if (/\b(blocked by|waiting on|blockedby|dependencies?)\b/i.test(normalized)) {
    const ids = Array.from(normalized.matchAll(/#?\b(\d+)\b/g))
      .map((match) => `#${match[1]}`)
      .filter((id, index, all) => all.indexOf(id) === index);
    if (ids.length > 0) return `blocked by ${ids.join(", ")}`;
  }
  return truncateText(normalized, 80);
}

function truncateText(value: string, limit: number): string {
  if (value.length <= limit) return value;
  return `${value.slice(0, Math.max(0, limit - 3)).trimEnd()}...`;
}

function numberFromUnknown(value: unknown): number | undefined {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim()) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : undefined;
  }
  return undefined;
}

function isTeamTaskStatus(value: string): value is TeamTaskStatus {
  return (
    value === "pending" ||
    value === "in_progress" ||
    value === "blocked" ||
    value === "completed"
  );
}

function latestActiveTodo(blocks: ChatBlock[]): {
  parsed: ParsedTodo;
  blockId: string;
} | null {
  for (let i = blocks.length - 1; i >= 0; i--) {
    const b = blocks[i];
    if (b.kind !== "tool") continue;
    if (!isToolName(b.name, "todo_list")) continue;
    if (b.status === "error") continue;
    const parsed = parseTodoOutput(b.output);
    if (!parsed) continue;
    if (parsed.state === "closed") return null;
    if (parsed.tasks.length === 0) return null;
    return { parsed, blockId: b.id };
  }
  return null;
}

function latestTeamTasks(blocks: ChatBlock[]): ParsedTeamTasks | null {
  const tasks = new Map<string, ParsedTeamTask & { sourceOrder: number }>();
  let teamName: string | undefined;
  let blockId = "";
  for (const [sourceOrder, block] of blocks.entries()) {
    if (block.kind !== "tool") continue;
    if (block.status === "error") continue;
    if (!isTeamTaskTool(block.name)) continue;
    const parsedFromMeta = parseTeamTasksFromMeta(block.meta);
    if (parsedFromMeta) {
      teamName = parsedFromMeta.teamName ?? teamName;
      if (mergeTeamTasks(tasks, parsedFromMeta.tasks, sourceOrder)) {
        blockId = block.id;
      }
      continue;
    }
    const parsed = parseTeamTasksFromOutput(block.output);
    if (!parsed) continue;
    teamName = parsed.teamName ?? teamName;
    if (mergeTeamTasks(tasks, parsed.tasks, sourceOrder)) {
      blockId = block.id;
    }
  }
  if (tasks.size === 0) return null;
  return {
    teamName,
    blockId,
    tasks: Array.from(tasks.values()).sort(
      (left, right) => Number(left.id) - Number(right.id),
    ),
  };
}

function mergeTeamTasks(
  tasks: Map<string, ParsedTeamTask & { sourceOrder: number }>,
  incoming: ParsedTeamTask[],
  sourceOrder: number,
): boolean {
  let changed = false;
  for (const task of incoming) {
    const current = tasks.get(task.id);
    if (current && !shouldReplaceTeamTask(current, task, sourceOrder)) {
      continue;
    }
    tasks.set(task.id, { ...task, sourceOrder });
    changed = true;
  }
  return changed;
}

function shouldReplaceTeamTask(
  current: ParsedTeamTask & { sourceOrder: number },
  incoming: ParsedTeamTask,
  sourceOrder: number,
): boolean {
  if (incoming.updatedAtMs !== undefined && current.updatedAtMs !== undefined) {
    return incoming.updatedAtMs >= current.updatedAtMs;
  }
  if (incoming.updatedAtMs !== undefined) return true;
  if (current.updatedAtMs !== undefined) return false;
  return sourceOrder >= current.sourceOrder;
}

function isTeamTaskTool(name: string): boolean {
  return ["team_run", "team_status", "task_create", "task_list", "task_update"].includes(
    canonicalToolName(name),
  );
}

function latestTeamMessages(blocks: ChatBlock[]): ParsedTeamMessages | null {
  const collected: Array<
    ParsedTeamMessage & {
      numericId?: number;
      sourceOrder: number;
      messageOrder: number;
    }
  > = [];
  const seen = new Set<string>();
  for (const [sourceOrder, block] of blocks.entries()) {
    if (block.kind !== "user-text") continue;
    const parsed = teamMessagesFromText(block.text);
    if (!parsed.length) continue;
    for (const [messageOrder, message] of parsed.entries()) {
      collected.push({
        ...message,
        blockId: block.id,
        numericId: numericMessageId(message.id),
        sourceOrder,
        messageOrder,
      });
    }
  }
  if (collected.length === 0) return null;

  const messages: ParsedTeamMessage[] = [];
  for (const message of collected.sort(compareNewestTeamMessageFirst)) {
    const key = teamMessageKey(message);
    if (seen.has(key)) continue;
    seen.add(key);
    messages.push(message);
  }
  return { messages, blockId: messages[0]?.blockId ?? "" };
}

function compareNewestTeamMessageFirst(
  left: { numericId?: number; sourceOrder: number; messageOrder: number },
  right: { numericId?: number; sourceOrder: number; messageOrder: number },
): number {
  if (left.numericId !== undefined && right.numericId !== undefined) {
    return right.numericId - left.numericId;
  }
  if (left.numericId !== undefined) return -1;
  if (right.numericId !== undefined) return 1;
  if (left.sourceOrder !== right.sourceOrder) {
    return right.sourceOrder - left.sourceOrder;
  }
  return right.messageOrder - left.messageOrder;
}

function numericMessageId(id?: string): number | undefined {
  if (!id?.trim()) return undefined;
  const parsed = Number(id.trim());
  return Number.isFinite(parsed) ? parsed : undefined;
}

function teamMessagesFromText(text: string): Omit<ParsedTeamMessage, "blockId">[] {
  if (!text.includes("<team_message") && !text.includes("<teammate-message")) {
    return [];
  }
  const messages: Omit<ParsedTeamMessage, "blockId">[] = [];
  for (const match of text.matchAll(/<team_message\b([^>]*)>([\s\S]*?)<\/team_message>/g)) {
    const attrs = match[1] ?? "";
    const message = decodeXmlEntities(match[2].trim());
    if (!message) continue;
    messages.push({
      id: attrValue(attrs, "id"),
      from: attrValue(attrs, "from"),
      to: attrValue(attrs, "to"),
      message,
    });
  }
  for (const match of text.matchAll(/<teammate-message\b([^>]*)>([\s\S]*?)<\/teammate-message>/g)) {
    const attrs = match[1] ?? "";
    const message = decodeXmlEntities(match[2].trim());
    if (!message) continue;
    messages.push({
      id: attrValue(attrs, "id"),
      from: attrValue(attrs, "teammate_id") ?? attrValue(attrs, "from"),
      to: attrValue(attrs, "to"),
      summary: attrValue(attrs, "summary"),
      message,
    });
  }
  return messages;
}

function teamMessageKey(message: Omit<ParsedTeamMessage, "blockId">): string {
  const fallback = [
    message.from?.trim().toLowerCase() ?? "",
    normalizedRecipient(message.to),
    message.summary?.trim().toLowerCase() ?? "",
    message.message.trim(),
  ].join(":");
  return message.id && normalizedRecipient(message.to) !== "*"
    ? `id:${message.id}`
    : fallback;
}

function filterTeamMessagesForRecipient(
  messages: ParsedTeamMessages | null,
  recipient?: string,
): ParsedTeamMessages | null {
  const recipientKey = recipient?.trim().toLowerCase();
  if (!messages || !recipientKey) return messages;
  const filtered = messages.messages.filter((message) =>
    messageReceivedByAgent(message, recipientKey),
  );
  return filtered.length > 0
    ? { ...messages, messages: filtered }
    : null;
}

function messageReceivedByAgent(
  message: Pick<ParsedTeamMessage, "to">,
  recipientKey: string,
): boolean {
  const to = normalizedRecipient(message.to);
  if (to === "*") return false;
  if (!to) return false;
  return to.toLowerCase().replace(/^@/, "") === recipientKey.replace(/^@/, "");
}

function attrValue(attrs: string, name: string): string | undefined {
  const match = attrs.match(new RegExp(`${name}="([^"]*)"`));
  return match ? decodeXmlEntities(match[1]).trim() : undefined;
}

function decodeXmlEntities(value: string): string {
  return value
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&amp;/g, "&");
}

function normalizedRecipient(value?: string): string {
  const trimmed = value?.trim().toLowerCase();
  return trimmed === "*" ? "*" : trimmed || "";
}

export function TodoStrip({
  blocks,
  teamBlocks,
  queuedPrompts = [],
  showTeamTasks = true,
  teamAgentColors = {},
  teamMessageRecipient,
  onOpenFile,
  onQueuedPromptSend,
  onQueuedPromptEdit,
  onQueuedPromptDelete,
  onQueuedPromptMove,
}: {
  blocks: ChatBlock[];
  teamBlocks?: ChatBlock[];
  queuedPrompts?: QueuedPromptStripItem[];
  showTeamTasks?: boolean;
  teamAgentColors?: Record<string, string>;
  teamMessageRecipient?: string;
  onOpenFile: (path: string) => void;
  onQueuedPromptSend?: (id: string) => void;
  onQueuedPromptEdit?: (id: string) => void;
  onQueuedPromptDelete?: (id: string) => void;
  onQueuedPromptMove?: (draggedId: string, targetId: string) => void;
}) {
  const latest = useMemo(() => latestActiveTodo(blocks), [blocks]);
  const latestTeam = useMemo(
    () => (showTeamTasks ? latestTeamTasks(teamBlocks ?? blocks) : null),
    [blocks, showTeamTasks, teamBlocks],
  );
  const latestMessages = useMemo(
    () => (showTeamTasks ? latestTeamMessages(teamBlocks ?? blocks) : null),
    [blocks, showTeamTasks, teamBlocks],
  );
  const visibleLatestMessages = useMemo(
    () => filterTeamMessagesForRecipient(latestMessages, teamMessageRecipient),
    [latestMessages, teamMessageRecipient],
  );
  const [open, setOpen] = useState(true);
  const [activePanel, setActivePanel] = useState<ActivePanel>("todo");
  const [lastBlockId, setLastBlockId] = useState<string | null>(null);
  const [lastTeamBlockId, setLastTeamBlockId] = useState<string | null>(null);
  const [lastMessagesBlockId, setLastMessagesBlockId] = useState<string | null>(null);
  const [lastQueueSignature, setLastQueueSignature] = useState<string | null>(null);
  const [height, setHeight] = useState<number | null>(null);
  const stripRef = useRef<HTMLDivElement | null>(null);
  const dragRef = useRef<{
    startY: number;
    startHeight: number;
    minHeight: number;
    maxHeight: number;
  } | null>(null);

  useLayoutEffect(() => {
    if (!open || height === null) return;
    const bounds = todoStripResizeBounds(stripRef.current);
    if (!bounds) return;
    setHeight((current) => {
      if (current === null) return current;
      const next = clamp(current, bounds.minHeight, bounds.maxHeight);
      return Math.abs(next - current) < 0.5 ? current : next;
    });
  });

  useEffect(() => {
    if (latest && latest.blockId !== lastBlockId) {
      setLastBlockId(latest.blockId);
      setOpen(true);
    }
  }, [latest, lastBlockId]);

  useEffect(() => {
    if (latestTeam && latestTeam.blockId !== lastTeamBlockId) {
      setLastTeamBlockId(latestTeam.blockId);
      setOpen(true);
      if (!latest) setActivePanel("team");
    }
  }, [latest, latestTeam, lastTeamBlockId]);

  useEffect(() => {
    if (visibleLatestMessages && visibleLatestMessages.blockId !== lastMessagesBlockId) {
      setLastMessagesBlockId(visibleLatestMessages.blockId);
      if (!latest && !latestTeam) setActivePanel("messages");
    }
  }, [latest, visibleLatestMessages, latestTeam, lastMessagesBlockId]);

  const queueSignature = queuedPrompts.map((prompt) => prompt.id).join("|");
  useEffect(() => {
    if (!queueSignature || queueSignature === lastQueueSignature) return;
    setLastQueueSignature(queueSignature);
    setOpen(true);
    setActivePanel("queue");
  }, [lastQueueSignature, queueSignature]);

  const panels = [
    queuedPrompts.length > 0 ? ("queue" as const) : null,
    latest ? ("todo" as const) : null,
    latestTeam ? ("team" as const) : null,
    visibleLatestMessages ? ("messages" as const) : null,
  ].filter((panel): panel is ActivePanel => panel !== null);

  useEffect(() => {
    if (panels.includes(activePanel)) return;
    setActivePanel(panels[0] ?? "todo");
  }, [activePanel, panels]);

  if (panels.length === 0) return null;

  const active = panels.includes(activePanel) ? activePanel : panels[0];
  const todoTasks = latest?.parsed.tasks ?? [];
  const teamTasks = latestTeam?.tasks ?? [];
  const teamMessages = visibleLatestMessages?.messages ?? [];
  const tasks = active === "team" ? teamTasks : active === "todo" ? todoTasks : [];
  const doneCount =
    active === "team"
      ? teamTasks.filter((t) => t.status === "completed").length
      : active === "messages"
        ? teamMessages.length
        : active === "queue"
          ? queuedPrompts.length
          : todoTasks.filter((t) => t.status === "done").length;
  const total =
    active === "messages"
      ? teamMessages.length
      : active === "queue"
        ? queuedPrompts.length
        : tasks.length;
  const visibleTaskEntries = tasks
    .map((task, index) => ({ task, index }))
    .filter(({ task }) => open || task.status === "in_progress");
  const visibleMessages = open ? teamMessages : [];
  const visibleQueuedPrompts = open
    ? queuedPrompts
    : queuedPrompts.slice(0, 1);
  const toggleOpen = () => setOpen((v) => !v);
  const startResize = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (!open) return;
    const strip = stripRef.current;
    if (!strip) return;
    const stripRect = strip.getBoundingClientRect();
    const bounds = todoStripResizeBounds(strip);
    if (!bounds) return;
    dragRef.current = {
      startY: event.clientY,
      startHeight: stripRect.height,
      minHeight: bounds.minHeight,
      maxHeight: bounds.maxHeight,
    };
    event.preventDefault();
    event.stopPropagation();
    document.body.style.cursor = "ns-resize";
    document.body.style.userSelect = "none";

    const onMove = (moveEvent: PointerEvent) => {
      const drag = dragRef.current;
      if (!drag) return;
      const nextHeight = drag.startHeight + drag.startY - moveEvent.clientY;
      setHeight(clamp(nextHeight, drag.minHeight, drag.maxHeight));
    };
    const onUp = () => {
      dragRef.current = null;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp, { once: true });
  };

  return (
    <div
      ref={stripRef}
      className="todo-strip"
      data-open={open ? "true" : "false"}
      data-resized={height === null ? "false" : "true"}
      style={
        height === null
          ? undefined
          : ({ "--todo-strip-height": `${height}px` } as CSSProperties)
      }
    >
      <div className="todo-strip__resize" onPointerDown={startResize} aria-hidden />
      <div className="todo-strip__head" onClick={toggleOpen}>
        {panels.length > 1 ? (
          <span className="todo-strip__switch">
            {panels.map((panel) => (
              <button
                key={panel}
                type="button"
                className="todo-strip__tab"
                data-active={active === panel ? "true" : "false"}
                onClick={(event) => {
                  event.stopPropagation();
                  setActivePanel(panel);
                }}
              >
                <Icon icon={panelIcon(panel)} width={13} height={13} />
                {panelLabel(panel)}
                <span>{panelCount(panel, todoTasks, teamTasks, teamMessages, queuedPrompts)}</span>
              </button>
            ))}
          </span>
        ) : (
          <button
            type="button"
            className="todo-strip__title todo-strip__title-button"
            onClick={(event) => {
              event.stopPropagation();
              toggleOpen();
            }}
            aria-expanded={open ? "true" : "false"}
          >
            <Icon
              icon={panelIcon(active)}
              width={14}
              height={14}
              className="todo-strip__title-icon"
            />
            {panelLabel(active)}
            <span className="todo-strip__count">
              {active === "messages" || active === "queue"
                ? total
                : `${doneCount}/${total}`}
            </span>
          </button>
        )}
        <button
          type="button"
          className="todo-strip__caret"
          data-open={open ? "true" : "false"}
          onClick={(event) => {
            event.stopPropagation();
            toggleOpen();
          }}
          aria-expanded={open ? "true" : "false"}
          aria-label={open ? "Collapse tasks" : "Expand tasks"}
        >
          <Icon icon="solar:alt-arrow-down-linear" width={13} height={13} />
        </button>
      </div>
      {active === "messages" && visibleMessages.length > 0 && (
        <div
          className="todo-strip__messages"
          style={
            {
              "--todo-target-rows": Math.max(
                todoTasks.length,
                teamTasks.length,
                0,
              ),
            } as CSSProperties
          }
        >
          {visibleMessages.map((message, index) => (
            <TeamMessageRow
              key={message.id ?? `${message.from ?? "peer"}-${message.to ?? "to"}-${index}`}
              message={message}
              agentColors={teamAgentColors}
              scopedRecipient={teamMessageRecipient}
              onOpenFile={onOpenFile}
            />
          ))}
        </div>
      )}
      {active === "queue" && visibleQueuedPrompts.length > 0 && (
        <div className="todo-strip__queue">
          {visibleQueuedPrompts.map((prompt, index) => (
            <QueuedPromptRow
              key={prompt.id}
              prompt={prompt}
              index={index}
              canMoveUp={index > 0}
              canMoveDown={index < visibleQueuedPrompts.length - 1}
              onMoveUp={
                onQueuedPromptMove && index > 0
                  ? () =>
                      onQueuedPromptMove(
                        prompt.id,
                        visibleQueuedPrompts[index - 1].id,
                      )
                  : undefined
              }
              onMoveDown={
                onQueuedPromptMove &&
                index < visibleQueuedPrompts.length - 1
                  ? () =>
                      onQueuedPromptMove(
                        prompt.id,
                        visibleQueuedPrompts[index + 1].id,
                      )
                  : undefined
              }
              onSend={onQueuedPromptSend}
              onEdit={onQueuedPromptEdit}
              onDelete={onQueuedPromptDelete}
            />
          ))}
        </div>
      )}
      {active !== "messages" && visibleTaskEntries.length > 0 && (
        <div className="todo-strip__list">
          {visibleTaskEntries.map(({ task, index }) => (
            <div
              key={task.id}
              className="todo-strip__item"
              data-panel={active}
              data-status={task.status}
            >
              <TaskStatusMark status={task.status} />
              <span className="todo-strip__text">
                {task.text}
                {active === "team" && (
                  <TeamTaskMeta
                    task={task as ParsedTeamTask}
                    index={index}
                    agentColors={teamAgentColors}
                  />
                )}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function panelIcon(panel: ActivePanel): string {
  if (panel === "queue") return "solar:playlist-minimalistic-2-linear";
  if (panel === "team") return "solar:checklist-minimalistic-linear";
  if (panel === "messages") return "solar:chat-round-dots-linear";
  return "solar:list-check-linear";
}

function panelLabel(panel: ActivePanel): string {
  if (panel === "queue") return QUEUE_LABEL;
  if (panel === "team") return TEAM_TASKS_LABEL;
  if (panel === "messages") return TEAM_MESSAGES_LABEL;
  return "To-dos";
}

function panelCount(
  panel: ActivePanel,
  todoTasks: ParsedTask[],
  teamTasks: ParsedTeamTask[],
  messages: ParsedTeamMessage[],
  queuedPrompts: QueuedPromptStripItem[] = [],
): string {
  if (panel === "queue") return String(queuedPrompts.length);
  if (panel === "team") {
    return `${teamTasks.filter((task) => task.status === "completed").length}/${teamTasks.length}`;
  }
  if (panel === "messages") return String(messages.length);
  return `${todoTasks.filter((task) => task.status === "done").length}/${todoTasks.length}`;
}

function QueuedPromptRow({
  prompt,
  index,
  canMoveUp,
  canMoveDown,
  onSend,
  onMoveUp,
  onMoveDown,
  onEdit,
  onDelete,
}: {
  prompt: QueuedPromptStripItem;
  index: number;
  canMoveUp: boolean;
  canMoveDown: boolean;
  onSend?: (id: string) => void;
  onMoveUp?: () => void;
  onMoveDown?: () => void;
  onEdit?: (id: string) => void;
  onDelete?: (id: string) => void;
}) {
  const attachmentCount = prompt.attachments.length;
  const hasImage = prompt.attachments.some((attachment) =>
    isImageAttachment(attachment),
  );
  return (
    <div className="todo-strip__queue-item">
      <button
        type="button"
        className="todo-strip__queue-body"
        onClick={() => onEdit?.(prompt.id)}
        title="Edit queued prompt"
      >
        <span className="todo-strip__queue-index">{index + 1}</span>
        <span className="todo-strip__queue-text">{prompt.text}</span>
        {attachmentCount > 0 && (
          <span className="todo-strip__queue-attachments">
            <Icon
              icon={hasImage ? "solar:gallery-linear" : "solar:paperclip-linear"}
              width={12}
              height={12}
            />
            {attachmentCount}
          </span>
        )}
      </button>
      <div className="todo-strip__queue-actions">
        <button
          type="button"
          className="todo-strip__queue-action"
          onClick={(event) => {
            event.stopPropagation();
            onMoveUp?.();
          }}
          disabled={!canMoveUp || !onMoveUp}
          aria-label="Move queued prompt up"
          title="Move up"
        >
          <Icon icon="solar:alt-arrow-up-linear" width={13} height={13} />
        </button>
        <button
          type="button"
          className="todo-strip__queue-action"
          onClick={(event) => {
            event.stopPropagation();
            onMoveDown?.();
          }}
          disabled={!canMoveDown || !onMoveDown}
          aria-label="Move queued prompt down"
          title="Move down"
        >
          <Icon icon="solar:alt-arrow-down-linear" width={13} height={13} />
        </button>
        <button
          type="button"
          className="todo-strip__queue-action"
          onClick={(event) => {
            event.stopPropagation();
            onSend?.(prompt.id);
          }}
          disabled={!onSend}
          aria-label="Send queued prompt now"
          title="Send now"
        >
          <Icon icon="solar:arrow-right-linear" width={14} height={14} />
        </button>
        <button
          type="button"
          className="todo-strip__queue-action todo-strip__queue-action--delete"
          onClick={(event) => {
            event.stopPropagation();
            onDelete?.(prompt.id);
          }}
          aria-label="Remove queued prompt"
          title="Remove"
        >
          <Icon icon="solar:close-circle-linear" width={13} height={13} />
        </button>
      </div>
    </div>
  );
}

function isImageAttachment(attachment: AttachmentInput): boolean {
  return /\.(png|jpe?g|gif|webp|svg|bmp|avif|heic|heif)$/i.test(
    attachment.name ?? attachment.path,
  );
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function todoStripResizeBounds(
  strip: HTMLDivElement | null,
): { minHeight: number; maxHeight: number } | null {
  if (!strip) return null;
  const stripRect = strip.getBoundingClientRect();
  const chatRect = strip.closest(".chat-col")?.getBoundingClientRect();
  const topLimit = (chatRect?.top ?? 0) + TODO_STRIP_TOP_GAP;
  const availableHeight = Math.max(1, stripRect.bottom - topLimit);
  const contentHeight = Math.max(1, measureTodoStripContentHeight(strip));
  const maxHeight = Math.max(1, Math.min(availableHeight, contentHeight));
  const minHeight = Math.min(TODO_STRIP_MIN_HEIGHT, maxHeight);
  return { minHeight, maxHeight };
}

function measureTodoStripContentHeight(strip: HTMLDivElement): number {
  const head = strip.querySelector<HTMLElement>(".todo-strip__head");
  const body = strip.querySelector<HTMLElement>(
    ".todo-strip__list, .todo-strip__messages, .todo-strip__queue",
  );
  return Math.ceil(
    (head?.getBoundingClientRect().height ?? 0) +
      (body ? measureNaturalColumnHeight(body) : 0),
  );
}

function measureNaturalColumnHeight(element: HTMLElement): number {
  const style = window.getComputedStyle(element);
  const children = Array.from(element.children).filter(
    (child): child is HTMLElement => child instanceof HTMLElement,
  );
  const childHeight = children.reduce(
    (total, child) => total + child.getBoundingClientRect().height,
    0,
  );
  const gap = children.length > 1 ? cssPx(style.rowGap || style.gap) : 0;
  return (
    cssPx(style.borderTopWidth) +
    cssPx(style.paddingTop) +
    childHeight +
    gap * Math.max(0, children.length - 1) +
    cssPx(style.paddingBottom) +
    cssPx(style.borderBottomWidth)
  );
}

function cssPx(value: string): number {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

function TaskStatusMark({ status }: { status: TodoStatus | TeamTaskStatus }) {
  return (
    <span className="todo-strip__mark">
      {status === "done" || status === "completed" ? (
        <Icon icon="solar:check-circle-linear" width={14} height={14} />
      ) : status === "in_progress" ? (
        <span className="todo-strip__loader" />
      ) : status === "blocked" ? (
        <Icon icon="solar:danger-triangle-linear" width={14} height={14} />
      ) : (
        <span className="todo-strip__square" />
      )}
    </span>
  );
}

function TeamTaskMeta({
  task,
  index,
  agentColors,
}: {
  task: ParsedTeamTask;
  index: number;
  agentColors: Record<string, string>;
}) {
  const owner = task.owner?.trim();
  const ownerColor = owner ? agentColors[owner.toLowerCase()] : undefined;
  const detail = normalizeTeamTaskDetail(task.detail);
  const bits = [
    task.status === "blocked" && !detail?.startsWith("blocked by")
      ? "Blocked"
      : "",
    detail ?? "",
  ].filter(Boolean);
  if (!owner && bits.length === 0) return null;
  return (
    <span className="todo-strip__meta">
      {owner && (
        <span
          className="todo-strip__owner"
          style={
            ownerColor
              ? ({ "--todo-agent-color": ownerColor } as CSSProperties)
              : undefined
          }
        >
          @{owner}
        </span>
      )}
      {bits.length > 0 && (
        <span className="todo-strip__detail">
          · {bits.join(" · ")}
        </span>
      )}
      <span className="todo-strip__suffix">- {index + 1}</span>
    </span>
  );
}

function TeamMessageRow({
  message,
  agentColors,
  scopedRecipient,
  onOpenFile,
}: {
  message: ParsedTeamMessage;
  agentColors: Record<string, string>;
  scopedRecipient?: string;
  onOpenFile: (path: string) => void;
}) {
  const from = message.from?.trim() || "teammate";
  const to = message.to?.trim();
  const showTarget = !scopedRecipient?.trim();
  const fromColor = colorForAgent(from, agentColors);
  const toColor = to && to !== "*" ? colorForAgent(to, agentColors) : undefined;
  return (
    <div className="todo-strip__message">
      <div className="todo-strip__message-body">
        <div className="todo-strip__message-line">
          {!showTarget && (
            <span className="todo-strip__message-target">from</span>
          )}
          <span
            className="todo-strip__message-agent"
            style={
              fromColor
                ? ({ "--todo-agent-color": fromColor } as CSSProperties)
                : undefined
            }
          >
            @{from}
          </span>
          {showTarget && (
            <>
              <Icon
                icon="solar:arrow-right-linear"
                width={12}
                height={12}
                className="todo-strip__message-arrow"
              />
              <span
                className="todo-strip__message-target"
                data-all={to === "*" ? "true" : "false"}
                style={
                  toColor
                    ? ({ "--todo-agent-color": toColor } as CSSProperties)
                    : undefined
                }
              >
                {recipientLabel(to)}
              </span>
            </>
          )}
          {message.summary && (
            <span className="todo-strip__message-summary">
              {message.summary}
            </span>
          )}
        </div>
        <div className="todo-strip__message-text">
          <Markdown text={message.message} onOpenFile={onOpenFile} />
        </div>
      </div>
    </div>
  );
}

function colorForAgent(
  agentName: string,
  agentColors: Record<string, string>,
): string | undefined {
  return agentColors[agentName.trim().toLowerCase()];
}

function recipientLabel(to?: string): string {
  const trimmed = to?.trim();
  if (!trimmed) return "teammate";
  if (trimmed === "*") return "all agents";
  return `@${trimmed}`;
}
