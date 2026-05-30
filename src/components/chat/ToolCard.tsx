import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type MouseEvent,
} from "react";
import { Icon } from "@iconify/react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { FileChange, TodoStatus, ToolResultImage } from "../../types";
import { canonicalToolName } from "../../lib/tools";
import { FileChangeBlock } from "./FileChangeBlock";

function extractEditFilePaths(argsPretty?: string): string[] {
  if (!argsPretty) return [];
  try {
    const parsed = JSON.parse(argsPretty);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return [];
    const files = (parsed as Record<string, unknown>).files;
    if (!Array.isArray(files)) return [];
    const seen = new Set<string>();
    const out: string[] = [];
    for (const file of files) {
      if (!file || typeof file !== "object") continue;
      const raw = (file as Record<string, unknown>).path;
      const path = typeof raw === "string" ? raw.trim() : "";
      if (!path || seen.has(path)) continue;
      seen.add(path);
      out.push(path);
    }
    return out;
  } catch {
    return [];
  }
}

export type ToolCardProps = {
  name: string;
  status: "running" | "done" | "error";
  summary?: string;
  argsPretty?: string;
  output?: string;
  isError?: boolean;
  cleaned?: boolean;
  fileChanges?: FileChange[];
  liveFileChange?: FileChange;
  images?: ToolResultImage[];
  meta?: Record<string, unknown> | null;
  onOpenFile: (path: string) => void;
  onOpenSubAgent?: () => void;
  onStopTeam?: (teamName?: string) => void | Promise<void>;
  teamAgents?: ToolCardTeamAgent[];
  teamCompletionByTeam?: Record<string, boolean>;
  activeTeamNames?: ReadonlySet<string>;
  subAgentName?: string;
  compactMode?: "disabled" | "compact" | "very-compact";
  onFixCommand?: (commandText: string) => void;
};

export type ToolCardTeamAgent = {
  name: string;
  status?: string;
  color: string;
  agentId?: string;
};

function TerminalGlyph() {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <rect x="1.5" y="2.5" width="11" height="9" rx="1.6" />
      <path d="M4 6l1.6 1.3L4 8.6" />
      <path d="M7.3 8.8h3" />
    </svg>
  );
}

function AsteriskGlyph() {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.45"
      strokeLinecap="round"
      aria-hidden
    >
      <path d="M7 2.4v9.2" />
      <path d="M3 4.7l8 4.6" />
      <path d="M11 4.7 3 9.3" />
    </svg>
  );
}

function BroomGlyph() {
  return (
    <svg
      width="14"
      height="14"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d="M11.5 2.5 L6.5 7.5" />
      <path d="M4.5 6.5 L7.5 9.5" />
      <path d="M3.8 7.6 L2 12" />
      <path d="M5 8.7 L4.2 12.4" />
      <path d="M6.2 9.8 L6.4 12.5" />
      <path d="M7.3 10.5 L8.6 12.2" />
    </svg>
  );
}

function SwarmGlyph() {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <g>
        <circle cx="5" cy="5" r="2" />
        <circle cx="12" cy="5" r="2" />
        <circle cx="19" cy="5" r="2" />
        <circle cx="12" cy="19" r="2" />
        <path d="M5 7 L5 12 L19 12 L19 7" />
        <path d="M12 7 L12 17" />
      </g>
    </svg>
  );
}

export function AiAgentGlyph() {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 14 14"
      fill="currentColor"
      aria-hidden
    >
      <path d="M7 0.7 C7.45 4.7 8.3 5.55 12.3 6 C8.3 6.45 7.45 7.3 7 11.3 C6.55 7.3 5.7 6.45 1.7 6 C5.7 5.55 6.55 4.7 7 0.7 Z" />
      <circle cx="11.4" cy="11" r="1.05" />
    </svg>
  );
}

function McpGlyph() {
  return (
    <svg
      className="tool-card__mcp-logo"
      width="16"
      height="16"
      viewBox="0 0 180 180"
      fill="none"
      aria-hidden
    >
      <path
        d="M18 84.8528L85.8822 16.9706C95.2548 7.59798 110.451 7.59798 119.823 16.9706C129.196 26.3431 129.196 41.5391 119.823 50.9117L68.5581 102.177"
        stroke="currentColor"
        strokeWidth="12"
        strokeLinecap="round"
      />
      <path
        d="M69.2652 101.47L119.823 50.9117C129.196 41.5391 144.392 41.5391 153.765 50.9117L154.118 51.2652C163.491 60.6378 163.491 75.8338 154.118 85.2063L92.7248 146.6C89.6006 149.724 89.6006 154.789 92.7248 157.913L105.331 170.52"
        stroke="currentColor"
        strokeWidth="12"
        strokeLinecap="round"
      />
      <path
        d="M102.853 33.9411L52.6482 84.1457C43.2756 93.5183 43.2756 108.714 52.6482 118.087C62.0208 127.459 77.2167 127.459 86.5893 118.087L136.794 67.8822"
        stroke="currentColor"
        strokeWidth="12"
        strokeLinecap="round"
      />
    </svg>
  );
}

function SkillGlyph() {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.45"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d="M4 2.5v4.2c0 1.5 1.2 2.7 2.7 2.7H11" />
      <path d="M8.8 7.2 11 9.4l-2.2 2.2" />
    </svg>
  );
}

function readArgs(argsPretty?: string): { path?: string; offset?: number; limit?: number } {
  if (!argsPretty) return {};
  try {
    return JSON.parse(argsPretty) as {
      path?: string;
      offset?: number;
      limit?: number;
    };
  } catch {
    return {};
  }
}

function grepArgs(argsPretty?: string): { path?: string | string[]; include?: string } {
  if (!argsPretty) return {};
  try {
    return JSON.parse(argsPretty) as {
      path?: string | string[];
      include?: string;
    };
  } catch {
    return {};
  }
}

function grepPathScope(path?: string | string[]): string | undefined {
  if (Array.isArray(path)) {
    const paths = path.map((value) => value.trim()).filter(Boolean);
    return paths.length > 0 ? paths.join(" ") : undefined;
  }
  const trimmed = path?.trim();
  return trimmed || undefined;
}

function globArgs(argsPretty?: string): { pattern?: string; path?: string } {
  if (!argsPretty) return {};
  try {
    return JSON.parse(argsPretty) as {
      pattern?: string;
      path?: string;
    };
  } catch {
    return {};
  }
}

type ParsedTodoTask = {
  id: string;
  status: TodoStatus;
  text: string;
};

function parseTodoOutput(output?: string):
  | { state: "active" | "closed"; tasks: ParsedTodoTask[] }
  | null {
  if (!output) return null;
  const state = output.match(/^state:\s*(active|closed)$/m)?.[1] as
    | "active"
    | "closed"
    | undefined;
  if (!state) return null;

  const tasks: ParsedTodoTask[] = [];
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

function parseCleanContextOutput(output?: string): number | null {
  if (!output) return null;
  const cleaned = output.match(/^cleaned:\s*(\d+)$/m)?.[1];
  return cleaned ? Number(cleaned) : null;
}

function todoStatusLabel(status: TodoStatus): string {
  if (status === "in_progress") return "in progress";
  return status;
}

function TodoListCard({
  status,
  summary,
  argsPretty,
  output,
  isError,
  compactMode,
}: Pick<
  ToolCardProps,
  "status" | "summary" | "argsPretty" | "output" | "isError" | "compactMode"
>) {
  const [open, setOpen] = useState(() => {
    if (compactMode === "very-compact") return false;
    return compactMode === "disabled" || !compactMode || isError;
  });

  useEffect(() => {
    if (compactMode === "very-compact") {
      setOpen(false);
    } else if (compactMode === "compact") {
      setOpen(isError);
    } else {
      setOpen(true);
    }
  }, [compactMode, isError]);
  const parsed = useMemo(() => parseTodoOutput(output), [output]);
  const taskCount = parsed?.tasks.length ?? 0;
  const title =
    status === "running"
      ? summary || "Updating ToDoList"
      : isError
        ? "ToDoList failed"
        : parsed?.state === "closed"
          ? "ToDoList closed"
          : `ToDoList · ${taskCount} ${taskCount === 1 ? "task" : "tasks"}`;

  return (
    <div className="tool-card tool-card--todo">
      <div
        className="tool-card__head"
        style={{ cursor: (compactMode === "disabled" || isError) ? "pointer" : "default" }}
        onClick={() => {
          if (compactMode !== "disabled" && !isError) return;
          setOpen((v) => !v);
        }}
      >
        {status === "running" ? (
          <span className="tool-card__spinner" />
        ) : status === "error" ? (
          <span className="tool-card__err-dot" />
        ) : (
          <span className="tool-card__glyph">
            <Icon icon="solar:checklist-linear" width={12} height={12} />
          </span>
        )}
        <span className="tool-card__title">{title}</span>
        {(compactMode === "disabled" || isError) && (
          <span className="tool-card__caret" data-open={open ? "true" : "false"}>
            <Icon
              icon={
                open
                  ? "solar:alt-arrow-down-linear"
                  : "solar:alt-arrow-right-linear"
              }
              width={12}
              height={12}
            />
          </span>
        )}
      </div>

      {!isError && parsed && parsed.tasks.length > 0 && (
        <div className="todo-tool__list">
          {parsed.tasks.map((task) => (
            <div
              key={task.id}
              className="todo-tool__item"
              data-status={task.status}
            >
              <span className="todo-tool__mark">
                {task.status === "done" ? (
                  <Icon icon="solar:check-circle-linear" width={13} height={13} />
                ) : task.status === "in_progress" ? (
                  <span className="todo-tool__pulse" />
                ) : (
                  <span className="todo-tool__dot" />
                )}
              </span>
              <span className="todo-tool__text">{task.text}</span>
              <span className="todo-tool__status">
                {todoStatusLabel(task.status)}
              </span>
            </div>
          ))}
        </div>
      )}

      {open && (
        <div className="tool-card__body">
          <pre className="tool-card__code">{argsPretty || "—"}</pre>
        </div>
      )}
    </div>
  );
}

function CleanContextCard({
  status,
  argsPretty,
  output,
  isError,
  compactMode,
}: Pick<ToolCardProps, "status" | "argsPretty" | "output" | "isError" | "compactMode">) {
  const [open, setOpen] = useState(() => {
    if (compactMode === "very-compact") return false;
    return compactMode === "disabled" || !compactMode || isError;
  });

  useEffect(() => {
    if (compactMode === "very-compact") {
      setOpen(false);
    } else if (compactMode === "compact") {
      setOpen(isError);
    } else {
      setOpen(true);
    }
  }, [compactMode, isError]);
  const cleaned = parseCleanContextOutput(output) ?? 0;
  const noun = cleaned === 1 ? "tool result" : "tool results";
  const title =
    status === "running"
      ? "Cleaning context"
      : isError
        ? "Context clean failed"
        : `Context cleaned (${cleaned} ${noun} cleaned)`;

  return (
    <div className="tool-card tool-card--clean-context">
      <div
        className="tool-card__head"
        style={{ cursor: (compactMode === "disabled" || isError) ? "pointer" : "default" }}
        onClick={() => {
          if (compactMode !== "disabled" && !isError) return;
          setOpen((v) => !v);
        }}
      >
        {status === "running" ? (
          <span className="tool-card__spinner" />
        ) : isError ? (
          <span className="tool-card__err-dot" />
        ) : (
          <span
            className="tool-card__glyph"
            style={{ color: "var(--ok)" }}
          >
            <BroomGlyph />
          </span>
        )}
        <span className="tool-card__title">{title}</span>
        {(compactMode === "disabled" || isError) && (
          <span className="tool-card__caret" data-open={open ? "true" : "false"}>
            <Icon
              icon={
                open
                  ? "solar:alt-arrow-down-linear"
                  : "solar:alt-arrow-right-linear"
              }
              width={12}
              height={12}
            />
          </span>
        )}
      </div>
      {open && (
        <div className="tool-card__body">
          {argsPretty ? <pre className="tool-card__code">{argsPretty}</pre> : null}
          {output !== undefined && (
            <pre
              className="tool-card__code"
              data-kind="output"
              data-error={isError ? "true" : "false"}
            >
              {output.length ? output : "—"}
            </pre>
          )}
        </div>
      )}
    </div>
  );
}

function toolImageSrc(image: ToolResultImage): string {
  if (image.path) return convertFileSrc(image.path);
  return `data:${image.media_type};base64,${image.data}`;
}

function parseReadOutput(output?: string) {
  if (!output) return null;

  const path = output.match(/^path:\s*(.+)$/m)?.[1]?.trim();
  const total = output.match(/^total:\s*(\d+)$/m)?.[1]?.trim();
  let range: string | null = null;
  const matches = [...output.matchAll(/^\s*(\d+)\s\|/gm)];
  const first = matches[0]?.[1];
  const last = matches[matches.length - 1]?.[1];
  if (first && last) range = `${first}-${last}`;
  if (!range && total) {
    range = `${total} lines`;
  }

  return { path, total, range };
}

function grepTitleParts(argsPretty?: string, output?: string, isError?: boolean) {
  const args = grepArgs(argsPretty);
  const outputPath = output?.match(/^path:\s*(.+)$/m)?.[1]?.trim();
  const rawScope = outputPath || grepPathScope(args.path) || args.include || "workspace";
  const scope = rawScope === "." ? args.include || "workspace" : rawScope;
  const matches = output?.match(/^matches:\s*(.+)$/m)?.[1]?.trim();

  if (isError) return { main: `Grep in ${scope}`, meta: "failed" };
  if (matches) {
    const label = matches === "1" ? "match" : "matches";
    return { main: `Grep in ${scope} ·`, meta: `${matches} ${label}` };
  }
  return { main: `Grep in ${scope}`, meta: null };
}

function globTitleParts(argsPretty?: string, output?: string, isError?: boolean) {
  const args = globArgs(argsPretty);
  const pattern = args.pattern?.trim() || "*";
  const rawScope = args.path?.trim() || "workspace";
  const scope = rawScope === "." ? "workspace" : rawScope;
  const matches = output?.match(/^matches:\s*(.+)$/m)?.[1]?.trim();

  if (isError) return { main: `Glob ${pattern} in ${scope}`, meta: "failed" };
  if (matches) {
    const label = matches === "1" ? "match" : "matches";
    return { main: `Glob ${pattern} in ${scope} -`, meta: `${matches} ${label}` };
  }
  return { main: `Glob ${pattern} in ${scope}`, meta: null };
}

function mcpTitleParts(name: string, summary?: string) {
  const summaryParts = summary
    ?.split(" - ")
    .map((part) => part.trim())
    .filter(Boolean);
  if (summaryParts && summaryParts.length >= 2) {
    const meta = summaryParts.slice(1).join(" - ");
    return {
      main: `${mcpServerLabel(summaryParts[0])} -`,
      meta: mcpToolLabel(meta),
    };
  }
  if (summaryParts && summaryParts.length === 1) {
    return {
      main: mcpServerLabel(summaryParts[0]),
      meta: null,
    };
  }

  const [, rawServer, ...rawToolParts] = name.split("__");
  const server = mcpServerLabel(rawServer || "MCP", true);
  const tool = mcpToolLabel(rawToolParts.join("__") || "tool", true);
  return {
    main: `${server} -`,
    meta: tool,
  };
}

function skillTitle(argsPretty?: string, summary?: string) {
  const fromArgs = parseToolArgs(argsPretty).name;
  const fromSummary = summary?.match(/(?:Load skill|Skill)\s*[·:]\s*'?([^']+?)'?$/i)?.[1];
  const name = (fromArgs || fromSummary || "skill").trim();
  return `Skill : ${name}`;
}

function teamMessageTitle(argsPretty?: string, summary?: string) {
  const to = parseToolArgs(argsPretty).to?.trim();
  if (to) return `Message · ${to === "*" ? "swarm" : `@${to.replace(/^@/, "")}`}`;
  return summary?.trim() || "Agent Swarm message";
}

function teamRunTitle(argsPretty?: string): string {
  const args = parseToolArgs(argsPretty);
  const agent = args.agent?.replace(/^@/, "").trim();
  if (agent) return `Agent Swarm restart @${agent}`;
  return "Agent Swarm spawned";
}

function teamCreateTitle(argsPretty?: string): string {
  const team = parseToolArgs(argsPretty).team_name?.trim();
  return team ? `Agent Swarm created : '${team}'` : "Agent Swarm created";
}

function teamRunRestartOutput(agent: string): string {
  const agentLabel = agent.replace(/^@/, "").trim();
  if (agentLabel) return `restarted teammate @${agentLabel}`;
  return "restarted teammate";
}

function teamStopOutput(output?: string): string | undefined {
  if (!output) return output;
  return output.replace(/stopped team `[^`]+`/i, "stopped Agent Swarm");
}

function subAgentNameFromSummary(summary?: string): string | null {
  if (!summary) return null;
  const parts = summary.split(" - ").map((part) => part.trim()).filter(Boolean);
  if (parts.length >= 2 && /^sub-agent$/i.test(parts[0])) {
    return parts.slice(1).join(" - ");
  }
  if (parts.length >= 2 && /^agent$/i.test(parts[0])) {
    return parts[1]?.replace(/^@/, "") ?? null;
  }
  return null;
}

export function subAgentToolTitle(
  summary?: string,
  subAgentName?: string,
): string {
  return `Agent · ${subAgentNameFromSummary(summary) ?? subAgentName ?? "agent"}`;
}

function parseToolArgs(argsPretty?: string): Record<string, string> {
  if (!argsPretty) return {};
  try {
    const parsed = JSON.parse(argsPretty) as Record<string, unknown>;
    return Object.fromEntries(
      Object.entries(parsed).filter(([, value]) => typeof value === "string"),
    ) as Record<string, string>;
  } catch {
    return {};
  }
}

function parseToolInput(argsPretty?: string): Record<string, unknown> {
  if (!argsPretty) return {};
  try {
    const parsed = JSON.parse(argsPretty) as unknown;
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

function displayToolArgs(name: string, argsPretty?: string): string | undefined {
  if (!argsPretty) return argsPretty;
  const input = parseToolInput(argsPretty);
  if (Object.keys(input).length === 0) return argsPretty;
  return prettyToolInput(name, input);
}

function prettyToolInput(name: string, input: Record<string, unknown>): string {
  try {
    return JSON.stringify(displayToolInput(name, input), null, 2);
  } catch {
    return JSON.stringify(input, null, 2);
  }
}

function displayToolInput(
  name: string,
  input: Record<string, unknown>,
): Record<string, unknown> {
  const cleaned = omitInternalTeamFields(input);
  if (canonicalToolName(name) !== "team_run") return cleaned;
  const agent = typeof input.agent === "string" ? input.agent.trim() : "";
  if (!agent) return compactTeamRunInput(cleaned);
  return { agent };
}

function compactTeamRunInput(input: Record<string, unknown>): Record<string, unknown> {
  const cleaned = omitInternalTeamFields(input);
  return Object.fromEntries(
    Object.entries(cleaned).filter(
      ([key]) => key !== "agent_count" && key !== "subagent_type",
    ),
  );
}

function omitInternalTeamFields(input: Record<string, unknown>): Record<string, unknown> {
  return Object.fromEntries(
    Object.entries(input).filter(([key]) => key !== "team_name"),
  );
}

type SwarmAgentMini = {
  name: string;
  status?: string;
  color: string;
  agentId?: string;
};

function teamRunDetails(
  argsPretty?: string,
  meta?: Record<string, unknown> | null,
  liveAgents: ToolCardTeamAgent[] = [],
): { teamName?: string; agents: SwarmAgentMini[] } {
  const input = parseToolInput(argsPretty);
  const args = parseToolArgs(argsPretty);
  const team = meta?.team;
  const teamRecord =
    team && typeof team === "object" && !Array.isArray(team)
      ? (team as Record<string, unknown>)
      : null;
  const teamName =
    args.team_name?.trim() ||
    (typeof teamRecord?.name === "string" ? teamRecord.name.trim() : "") ||
    undefined;

  const agents = new Map<string, SwarmAgentMini>();
  const addAgent = (
    name: unknown,
    status?: unknown,
    color?: string,
    agentId?: string,
  ) => {
    if (typeof name !== "string") return;
    const trimmed = name.replace(/^@/, "").trim();
    if (!trimmed) return;
    const key = trimmed.toLowerCase();
    agents.set(key, {
      name: trimmed,
      status: typeof status === "string" ? status : undefined,
      color: color || fallbackSwarmAgentColor(trimmed),
      agentId,
    });
  };

  const subagents = Array.isArray(meta?.subagents) ? meta?.subagents : [];
  for (const raw of subagents) {
    if (!raw || typeof raw !== "object") continue;
    const record = raw as Record<string, unknown>;
    addAgent(record.name, record.status);
  }

  const teamAgents = Array.isArray(teamRecord?.agents) ? teamRecord.agents : [];
  for (const raw of teamAgents) {
    if (!raw || typeof raw !== "object") continue;
    const record = raw as Record<string, unknown>;
    addAgent(record.name, record.status);
  }

  const inputNames = Array.isArray(input.agent_names) ? input.agent_names : [];
  for (const name of inputNames) addAgent(name);

  for (const agent of liveAgents) {
    const agentTeamName = teamNameFromAgentId(agent.agentId);
    if (teamName && agentTeamName !== teamName) continue;
    addAgent(agent.name, agent.status, agent.color, agent.agentId);
  }

  const sortedAgents = Array.from(agents.values()).sort((left, right) =>
    left.name.localeCompare(right.name),
  );
  return {
    teamName,
    agents: assignUniqueSwarmAgentColors(sortedAgents),
  };
}

function teamNameFromAgentId(agentId?: string): string | undefined {
  const at = agentId?.lastIndexOf("@") ?? -1;
  if (at < 0) return undefined;
  return agentId?.slice(at + 1).trim() || undefined;
}

const TEAM_AGENT_COLORS = [
  "#f72585",
  "#a3e635",
  "#60a5fa",
  "#5eead4",
  "#7bd88f",
  "#ffd166",
  "#b388ff",
  "#ff8a5b",
  "#f472b6",
  "#4cc9f0",
];

function assignUniqueSwarmAgentColors(
  agents: SwarmAgentMini[],
): SwarmAgentMini[] {
  const ordered = [...agents].sort((left, right) =>
    teamColorOrderKey(left).localeCompare(teamColorOrderKey(right)),
  );
  const colors = new Map<string, string>();
  ordered.forEach((agent, index) => {
    colors.set(agent.name.trim().toLowerCase(), teamAgentColorAt(index));
  });
  return agents.map((agent) => ({
    ...agent,
    color: colors.get(agent.name.trim().toLowerCase()) ?? agent.color,
  }));
}

function teamColorOrderKey(
  agent: Pick<SwarmAgentMini, "name" | "agentId">,
): string {
  return `${agent.name.trim().toLowerCase() || "agent"}-${agent.agentId ?? ""}`;
}

function teamAgentColorAt(index: number): string {
  if (index < TEAM_AGENT_COLORS.length) return TEAM_AGENT_COLORS[index];
  const hue = Math.round((index * 137.508) % 360);
  return `hsl(${hue} 82% 68%)`;
}

function fallbackSwarmAgentColor(name: string): string {
  let hash = 0;
  for (const char of name.toLowerCase()) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  }
  return TEAM_AGENT_COLORS[hash % TEAM_AGENT_COLORS.length];
}

function mcpToolLabel(value: string, generated = false): string {
  const trimmed = value.trim();
  if (/^run_browser_agent$/i.test(trimmed)) return "Local Chrome control";
  return generated ? genericMcpLabel(trimmed) : trimmed;
}

function mcpServerLabel(value: string, generated = false): string {
  const trimmed = value.trim();
  const stripped = trimmed
    .replace(/^mcp(?=[A-Z0-9])/i, "")
    .replace(/^mcp[-_.\s]+/i, "")
    .trim();
  const label = stripped || trimmed || "MCP";
  if (/^sinew\s+chrome(?:\s+[a-z])?$/i.test(label)) return "Sinew Chrome";
  return generated ? genericMcpLabel(label) : label;
}

function genericMcpLabel(value: string): string {
  const words = value
    .replace(/_[0-9a-f]{8}$/i, "")
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/[-_.]+/g, " ")
    .trim()
    .split(/\s+/)
    .filter(Boolean)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1).toLowerCase());
  return words.join(" ") || "Tool";
}

function ReadToolInline({
  status,
  argsPretty,
  output,
  isError,
  cleaned,
  onOpenFile,
  compactMode,
}: Pick<
  ToolCardProps,
  "status" | "argsPretty" | "output" | "isError" | "cleaned" | "onOpenFile" | "compactMode"
>) {
  const args = readArgs(argsPretty);
  const parsed = parseReadOutput(output);
  const openPath = parsed?.path ?? args.path ?? null;
  const path = openPath ?? "file";
  const range =
    status === "running"
      ? "reading"
      : isError
        ? "failed"
        : parsed?.range;

  return (
    <div
      className="tool-read-inline"
      data-error={isError ? "true" : "false"}
      data-cleaned={cleaned ? "true" : "false"}
    >
      {status === "running" ? (
        <span className="tool-read-inline__spinner" />
      ) : isError ? (
        <span className="tool-read-inline__err-dot" />
      ) : (
        <span className="tool-read-inline__glyph">
          <Icon icon="solar:document-text-linear" width={13} height={13} />
        </span>
      )}
      <span className="tool-read-inline__label">Read</span>
      {compactMode !== "compact" && openPath ? (
        <span className="tool-read-inline__path" title={path}>
          <button
            type="button"
            className="chat-file-link tool-read-inline__file"
            title="Open file"
            onClick={() => onOpenFile(openPath)}
          >
            {path}
          </button>
        </span>
      ) : compactMode !== "compact" ? (
        <span className="tool-read-inline__path" title={path}>
          <span className="tool-read-inline__file">{path}</span>
        </span>
      ) : null}
      {compactMode !== "compact" && range && <span className="tool-read-inline__range">{range}</span>}
    </div>
  );
}

function SwarmAgentsInline({
  agents,
  loading = false,
}: {
  agents: SwarmAgentMini[];
  loading?: boolean;
}) {
  if (agents.length === 0) {
    return (
      <div className="tool-card__swarm-agents" data-empty="true">
        {loading ? (
          <span className="tool-card__swarm-loading">
            <span className="tool-card__spinner" />
            <span>Preparing agents</span>
          </span>
        ) : (
          <span className="tool-card__swarm-empty">Agents pending</span>
        )}
      </div>
    );
  }
  return (
    <div className="tool-card__swarm-agents">
      {agents.map((agent) => (
        <div
          key={agent.name}
          className="tool-card__swarm-agent"
          style={{ "--swarm-agent-color": agent.color } as CSSProperties}
        >
          <span className="tool-card__swarm-dot" />
          <span className="tool-card__swarm-name">@{agent.name}</span>
          {agent.status && (
            <span className="tool-card__swarm-status">{agent.status}</span>
          )}
        </div>
      ))}
      {loading && (
        <div className="tool-card__swarm-loading">
          <span className="tool-card__spinner" />
          <span>Preparing agents</span>
        </div>
      )}
    </div>
  );
}

export function ToolCard({
  name,
  status,
  summary,
  argsPretty,
  output,
  isError,
  cleaned,
  fileChanges,
  liveFileChange,
  images,
  meta,
  onOpenFile,
  onOpenSubAgent,
  onStopTeam,
  teamAgents,
  activeTeamNames,
  subAgentName,
  compactMode,
  onFixCommand,
}: ToolCardProps) {
  const canonicalName = canonicalToolName(name);
  const isCreateImage = canonicalName === "create_image";
  const isTeamRunTool = canonicalName === "team_run";
  const [open, setOpen] = useState(() => {
    if (compactMode === "very-compact") return false;
    return compactMode === "disabled" || !compactMode || isError;
  });
  const [teamStopState, setTeamStopState] = useState<
    "idle" | "stopping" | "stopped" | "error"
  >("idle");
  const previousTeamRunActiveRef = useRef(false);

  const command = useMemo(() => {
    if ((canonicalName !== "bash" && canonicalName !== "bash_input") || !argsPretty) return null;
    try {
      const parsed = JSON.parse(argsPretty) as {
        command?: string;
        input?: string;
        session_id?: number;
        kill?: boolean;
      };
      if (canonicalName === "bash") return parsed.command ?? null;
      if (parsed.kill && parsed.session_id) return `kill session ${parsed.session_id}`;
      if (typeof parsed.input === "string" && parsed.input.length > 0) {
        return parsed.input;
      }
      return parsed.session_id ? `poll session ${parsed.session_id}` : null;
    } catch {
      return null;
    }
  }, [canonicalName, argsPretty]);
  const teamRunInput = useMemo(
    () => (isTeamRunTool ? parseToolInput(argsPretty) : {}),
    [argsPretty, isTeamRunTool],
  );
  const displayArgsPretty = useMemo(
    () => displayToolArgs(name, argsPretty),
    [argsPretty, name],
  );
  const teamRunAgent =
    typeof teamRunInput.agent === "string" ? teamRunInput.agent.trim() : "";
  const isTeamRunRestart = isTeamRunTool && teamRunAgent.length > 0;
  const isTeamRunSpawn = isTeamRunTool && !isTeamRunRestart;
  const swarmDetails = useMemo(
    () =>
      isTeamRunTool
        ? teamRunDetails(argsPretty, meta, teamAgents ?? [])
        : null,
    [argsPretty, isTeamRunTool, meta, teamAgents],
  );
  const teamRunActive =
    !!swarmDetails?.teamName && activeTeamNames?.has(swarmDetails.teamName);
  const teamRunPreparing = isTeamRunSpawn && status === "running" && !teamRunActive;

  useEffect(() => {
    if (!isTeamRunSpawn) return;
    const wasActive = previousTeamRunActiveRef.current;
    if (teamRunActive) {
      setOpen(false);
    } else if (status === "running") {
      setOpen(true);
    } else if (wasActive) {
      setOpen(true);
    }
    previousTeamRunActiveRef.current = !!teamRunActive;
  }, [isTeamRunSpawn, status, teamRunActive]);

  useEffect(() => {
    if (compactMode === "very-compact") {
      setOpen(false);
    } else if (compactMode === "compact") {
      setOpen(isError);
    } else {
      setOpen(true);
    }
  }, [compactMode, isError]);

  if (canonicalName === "read") {
    return (
      <ReadToolInline
        status={status}
        argsPretty={argsPretty}
        output={output}
        isError={isError}
        cleaned={cleaned}
        onOpenFile={onOpenFile}
        compactMode={compactMode}
      />
    );
  }

  const isBash = canonicalName === "bash" || canonicalName === "bash_input";
  const isGlob = canonicalName === "glob";
  const isGrep = canonicalName === "grep";
  const isEditFile = canonicalName === "edit_file";
  const isWriteFile = canonicalName === "write_file";
  const isCleanContext = canonicalName === "clean_context";
  const isContextCompaction = canonicalName === "context_compaction";
  const isGoalUpdate = canonicalName === "update_goal";
  const isTodo = canonicalName === "todo_list";
  const isWebSearch = canonicalName === "web_search";
  const isWebFetch = canonicalName === "web_fetch";
  const isSkill = canonicalName === "skill";
  const isLoadSkill = isSkill && name !== canonicalName;
  const isMcp = name.startsWith("mcp__");
  const isLoadMcp = canonicalName === "load_mcp_tool";
  const isTeamMessage = canonicalName === "send_message";
  const isTeamRun = isTeamRunTool;
  const isTeamCreate = canonicalName === "team_create";
  const isTeamStatus = canonicalName === "team_status";
  const isTeamStop = canonicalName === "team_stop";
  const isTeam = isTeamRun || isTeamCreate || isTeamStatus || isTeamStop;
  const isSubAgent = name.startsWith("subagent_") || canonicalName === "agent";
  const hasImages = !!images && images.length > 0;
  const editingPaths =
    isEditFile && status === "running" ? extractEditFilePaths(argsPretty) : [];
  const showEditingTitle = editingPaths.length > 0;
  const displayOutput =
    isTeamRunRestart && !isError
      ? teamRunRestartOutput(teamRunAgent)
      : isTeamStop && !isError
        ? teamStopOutput(output)
      : output;

  if (isTodo) {
    return (
      <TodoListCard
        status={status}
        summary={summary}
        argsPretty={argsPretty}
        output={output}
        isError={isError}
        compactMode={compactMode}
      />
    );
  }

  if (isCleanContext) {
    return (
      <CleanContextCard
        status={status}
        argsPretty={argsPretty}
        output={output}
        isError={isError}
        compactMode={compactMode}
      />
    );
  }

  if (isGoalUpdate) {
    const title =
      status === "error"
        ? "Goal update failed"
        : status === "running"
          ? "Finishing goal"
          : "Goal finished";
    return (
      <div className="tool-card tool-card--goal-update">
        <div className="tool-card__head" data-clickable="false">
          {status === "running" ? (
            <span className="tool-card__spinner" />
          ) : status === "error" ? (
            <span className="tool-card__err-dot" />
          ) : (
            <span className="tool-card__glyph">
              <Icon icon="solar:flag-2-linear" width={13} height={13} />
            </span>
          )}
          <span className="tool-card__title">{title}</span>
        </div>
      </div>
    );
  }

  const renderedFileChanges = fileChanges ?? (liveFileChange ? [liveFileChange] : undefined);
  const isLiveFileChange = !fileChanges && !!liveFileChange;

  if (
    (isEditFile || isWriteFile) &&
    !isError &&
    renderedFileChanges &&
    renderedFileChanges.length > 0 &&
    (compactMode === "disabled" || !compactMode)
  ) {
    return (
      <div className="tool-card__changes" data-bare="true">
        {renderedFileChanges.map((change, idx) => (
          <FileChangeBlock key={idx} change={change} live={isLiveFileChange} />
        ))}
      </div>
    );
  }

  const searchTitle = isGrep
    ? grepTitleParts(argsPretty, output, isError)
    : isGlob
      ? globTitleParts(argsPretty, output, isError)
      : null;
  const mcpTitle = isMcp ? mcpTitleParts(name, summary) : null;
  const bashTitle = isBash && command ? command : null;
  const editingTitle = showEditingTitle
    ? `Editing ${editingPaths.length} file${editingPaths.length > 1 ? "s" : ""}`
    : null;
  const title = editingTitle
    ? editingTitle
    : bashTitle
    ? bashTitle
    : isSubAgent
    ? subAgentToolTitle(summary, subAgentName)
    : isTeamMessage
    ? teamMessageTitle(argsPretty, summary)
    : isTeamRun
    ? teamRunTitle(argsPretty)
    : isTeamCreate
    ? teamCreateTitle(argsPretty)
    : isSkill
    ? skillTitle(argsPretty, summary)
    : summary && summary.trim().length > 0
      ? summary
      : canonicalName;
  const hasChanges = !!renderedFileChanges && renderedFileChanges.length > 0;
  const canExpand =
    !(isContextCompaction && status === "running") &&
    !(isEditFile && status === "running");
  const showBody = (compactMode === "disabled" || isError) && canExpand && open && (!isTeamRunSpawn || !teamRunActive);
  const showTeamStop =
    isTeamRunSpawn &&
    !!teamRunActive &&
    !!onStopTeam &&
    teamStopState !== "stopped";
  const canStopTeam =
    showTeamStop &&
    (teamStopState === "idle" || teamStopState === "error");
  const handleStopTeam = async (event: MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    if (
      !onStopTeam ||
      !teamRunActive ||
      teamStopState === "stopping" ||
      teamStopState === "stopped"
    ) {
      return;
    }
    setTeamStopState("stopping");
    try {
      await onStopTeam(swarmDetails?.teamName);
      setTeamStopState("stopped");
    } catch {
      setTeamStopState("error");
    }
  };

  return (
    <div className="tool-card">
      <div
        className="tool-card__head"
        data-cleaned={cleaned ? "true" : "false"}
        data-clickable={(compactMode === "disabled" || isError) && (canExpand || isSubAgent) ? "true" : "false"}
        onClick={() => {
          if (compactMode !== "disabled" && !isError) return;
          if (isSubAgent && onOpenSubAgent) {
            onOpenSubAgent();
            return;
          }
          if (!canExpand) return;
          if (isTeamRunSpawn && teamRunActive) return;
          setOpen((v) => !v);
        }}
      >
        {isSubAgent && status === "running" ? (
          <span className="tool-card__spinner" />
        ) : isSubAgent ? (
          <span className="tool-card__glyph">
            <AiAgentGlyph />
          </span>
        ) : isSkill || isLoadSkill ? (
          <span className="tool-card__glyph">
            <SkillGlyph />
          </span>
        ) : isMcp || isLoadMcp ? (
          <span className="tool-card__glyph">
            <McpGlyph />
          </span>
        ) : isTeam ? (
          <span className="tool-card__glyph tool-card__glyph--swarm">
            <SwarmGlyph />
          </span>
        ) : status === "running" ? (
          <span className="tool-card__spinner" />
        ) : status === "error" ? (
          <span className="tool-card__err-dot" />
        ) : (
          <span className="tool-card__glyph">
            {isBash ? (
              <TerminalGlyph />
            ) : isGlob || isGrep ? (
              <AsteriskGlyph />
            ) : isEditFile || isWriteFile ? (
              <Icon icon="solar:pen-new-square-linear" width={12} height={12} />
            ) : isWebSearch ? (
              <Icon icon="solar:magnifer-linear" width={12} height={12} />
            ) : isWebFetch ? (
              <Icon icon="solar:link-round-linear" width={12} height={12} />
            ) : isCreateImage ? (
              <Icon icon="solar:gallery-wide-linear" width={13} height={13} />
            ) : isTeamMessage ? (
              <Icon icon="solar:chat-round-dots-linear" width={13} height={13} />
            ) : isContextCompaction ? (
              <Icon icon="solar:archive-bold" width={13} height={13} />
            ) : (
              <Icon icon="solar:tuning-linear" width={12} height={12} />
            )}
          </span>
        )}
        {searchTitle ? (
          <span className="tool-card__title tool-card__title--search">
            <span className="tool-card__title-main" title={searchTitle.main}>
              {searchTitle.main}
            </span>
            {searchTitle.meta && (
              <span className="tool-card__title-meta">{searchTitle.meta}</span>
            )}
          </span>
        ) : mcpTitle ? (
          <span className="tool-card__title tool-card__title--mcp">
            <span className="tool-card__title-main" title={mcpTitle.main}>
              {mcpTitle.main}
            </span>
            {mcpTitle.meta && (
              <span className="tool-card__title-meta" title={mcpTitle.meta}>
                {mcpTitle.meta}
              </span>
            )}
          </span>
        ) : (
          <span className="tool-card__title">{title}</span>
        )}
        {isError && isBash && command && onFixCommand && (
          <button
            type="button"
            className="tool-card__fix-action"
            onClick={(e) => {
              e.stopPropagation();
              onFixCommand(command);
            }}
            title="Auto-réparer avec un agent"
          >
            <Icon icon="solar:tuning-linear" width={12} height={12} />
            <span>Auto-réparer</span>
          </button>
        )}
        {showTeamStop && (
          <button
            type="button"
            className="tool-card__stop-team"
            data-state={teamStopState}
            disabled={!canStopTeam}
            onClick={handleStopTeam}
            title="Stop Agent Swarm"
          >
            <Icon
              icon="solar:stop-bold"
              width={13}
              height={13}
            />
            <span>
              {teamStopState === "stopping" ? "Stopping" : "Stop"}
            </span>
          </button>
        )}
        {(compactMode === "disabled" || isError) && (canExpand || isSubAgent) ? (
          <span
            className="tool-card__caret"
            data-open={isSubAgent ? "false" : showBody ? "true" : "false"}
          >
            <Icon
              icon={
                isSubAgent
                  ? "solar:alt-arrow-right-linear"
                  : showBody
                  ? "solar:alt-arrow-down-linear"
                  : "solar:alt-arrow-right-linear"
              }
              width={12}
              height={12}
            />
          </span>
        ) : null}
      </div>
      {showBody && (
        <div className="tool-card__body">
          {isTeamRunSpawn ? (
            <SwarmAgentsInline
              agents={swarmDetails?.agents ?? []}
              loading={teamRunPreparing}
            />
          ) : isBash && command ? (
            <pre className="tool-card__code">{command}</pre>
          ) : displayArgsPretty && !isEditFile && !isWriteFile ? (
            <pre className="tool-card__code">{displayArgsPretty}</pre>
          ) : null}
          {displayOutput !== undefined && (!isTeamRunSpawn || isError) && (!isEditFile && !isWriteFile || isError) && (
            <pre
              className="tool-card__code"
              data-kind="output"
              data-error={isError ? "true" : "false"}
            >
              {displayOutput.length ? displayOutput : "—"}
            </pre>
          )}
          {hasImages && (
            <div className="tool-card__images">
              {images!.map((image, idx) => (
                <img
                  key={`${image.media_type}-${idx}`}
                  className="tool-card__image"
                  src={toolImageSrc(image)}
                  alt={`Generated image ${idx + 1}`}
                />
              ))}
            </div>
          )}
          {hasChanges && (
            <div className="tool-card__changes">
              {renderedFileChanges!.map((change, idx) => (
                <FileChangeBlock key={idx} change={change} live={isLiveFileChange} />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
