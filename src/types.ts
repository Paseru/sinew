// Domain types shared with the Rust backend. Kept in sync manually with
// src-tauri/src/lib.rs and crates/claakecode-app.

export type Role = "user" | "assistant";

/// Snapshot of an updater check, returned by `updater_check`.
/// Mirrors `src-tauri/src/updater.rs::UpdateInfo`.
export type UpdateInfo = {
  available: boolean;
  currentVersion: string;
  version: string | null;
  notes: string | null;
  date: string | null;
};

/// Payload of the `updater://progress` event.
export type UpdateProgress = {
  downloaded: number;
  total: number | null;
};

/// Detail payload of the `sinew:install-update` DOM event. Dispatched on the
/// `window` by <UpdateBadge /> when the user clicks "Install & restart" in
/// the in-session popover. <App /> listens for it and swaps the whole UI
/// for <UpdaterLockScreen autoInstall />, which then drives the download +
/// install + auto-restart flow.
export type InstallUpdateEventDetail = {
  info: UpdateInfo;
};

declare global {
  interface WindowEventMap {
    "sinew:install-update": CustomEvent<InstallUpdateEventDetail>;
  }
}

export type TextPart = {
  type: "text";
  text: string;
  meta?: Record<string, unknown> | null;
};

export type ThinkingPart = {
  type: "thinking";
  text: string;
  meta?: Record<string, unknown> | null;
};

export type ImagePart = {
  type: "image";
  media_type: string;
  data: string;
  meta?: Record<string, unknown> | null;
};

export type ToolCallPart = {
  type: "tool_call";
  id: string;
  name: string;
  input: unknown;
  meta?: Record<string, unknown> | null;
};

export type ToolResultPart = {
  type: "tool_result";
  tool_call_id: string;
  content: string;
  images?: ToolResultImage[];
  is_error?: boolean;
  meta?: {
    file_changes?: FileChange[];
    todo_list?: TodoListState;
    [k: string]: unknown;
  } | null;
};

export type ToolResultImage = {
  media_type: string;
  data: string;
  path?: string | null;
};

export type Part =
  | TextPart
  | ImagePart
  | ThinkingPart
  | ToolCallPart
  | ToolResultPart;

export type QuestionAnswer = string[];

export type ChatMessage = {
  role: Role;
  parts: Part[];
};

export type WorkspaceInfo = {
  path: string;
  name: string;
};

export type ConversationSummary = {
  id: string;
  title: string;
  updatedAtMs: number;
};

export type SavedConversation = {
  id: string;
  workspaceId: string;
  title: string;
  model: ModelRef;
  modeModelSettings: ModeModelSettings;
  systemPrompt: string;
  todoList?: TodoListState;
  planWorkflow: PlanWorkflowState;
  goalWorkflow: GoalWorkflowState;
  history: ChatMessage[];
};

export type TodoStatus = "pending" | "in_progress" | "done";

export type TodoTask = {
  id: string;
  text: string;
  status: TodoStatus;
};

export type TodoListState = {
  active: boolean;
  tasks: TodoTask[];
  nextId: number;
};

export type ModelRef = {
  provider: string;
  name: string;
  effort?: "none" | "low" | "medium" | "high" | "xhigh" | "max" | null;
  use1mContext?: boolean | null;
};

export type AgentMode = "act" | "plan" | "goal";

export type ServiceTier = "fast";

export type ModeModelSettings = Record<AgentMode, ModelRef>;

export type SubAgentConfig = {
  id: string;
  name: string;
  description: string;
  prompt: string;
  model: ModelRef;
  enabled: boolean;
};

export type SubAgentSettings = {
  agents: SubAgentConfig[];
};

export type ToolConfig = {
  name: string;
  displayName?: string;
  description: string;
  defaultDescription: string;
  enabled: boolean;
};

export type ImageProvider = "gptImage2" | "nanoBanana2";
export type WebSearchProvider = "linkup" | "classic";

export type ToolSettings = {
  tools: ToolConfig[];
  planModePrompt: string;
  defaultPlanModePrompt: string;
  imageProvider: ImageProvider;
  openaiImageUseSubscription: boolean;
  openaiImageApiKey: string;
  nanoBananaApiKey: string;
  webSearchProvider: WebSearchProvider;
  linkupApiKey: string;
};

export type DatabaseEngine =
  | "postgres"
  | "mysql"
  | "sqlite"
  | "mssql"
  | "supabaseRest";

export type DatabaseCredentialMode = "connectionString" | "fields";

export type DatabaseSslMode = "disabled" | "required" | "strict";

export type DatabaseConnectionStatusState = "ok" | "error" | "untested";

export type DatabaseConnectionStatus = {
  status: DatabaseConnectionStatusState;
  message?: string | null;
  timestampMs?: number | null;
};

export type DatabaseConnectionFields = {
  host: string;
  port: number | null;
  user: string;
  password: string;
  database: string;
  sslMode: DatabaseSslMode;
  sslCertificate?: string | null;
};

export type DatabaseSqliteConfig = {
  filePath: string;
  createIfMissing: boolean;
};

export type DatabaseSupabaseRestConfig = {
  projectUrl: string;
  anonKey: string;
  serviceRoleKey: string;
  useServiceRole: boolean;
  allowRpc: boolean;
};

export type DatabaseSourceConfig = {
  id: string;
  name: string;
  engine: DatabaseEngine;
  enabled: boolean;
  note: string;
  defaultSchema?: string | null;
  defaultRowLimit: number;
  defaultTimeoutMs: number;
  readOnly: boolean;
  requireConfirmationForDestructive: boolean;
  lastConnectionStatus: DatabaseConnectionStatus;
  credentialMode?: DatabaseCredentialMode;
  connectionString?: string;
  fields?: DatabaseConnectionFields;
  sqlite?: DatabaseSqliteConfig;
  supabaseRest?: DatabaseSupabaseRestConfig;
};

export type DatabaseSettings = {
  sources: DatabaseSourceConfig[];
};

export type DatabaseConnectionTestResult = {
  ok: boolean;
  message: string;
  status?: DatabaseConnectionStatusState | null;
  timestampMs?: number | null;
  durationMs?: number | null;
};

export type DatabaseActivityStatus = "success" | "error" | "cancelled" | "pending";

export type DatabaseActivityEntry = {
  id: string;
  sourceId: string;
  timestampMs: number;
  status: DatabaseActivityStatus;
  operation?: string | null;
  queryPreview: string;
  durationMs?: number | null;
  rowsAffected?: number | null;
  rowsReturned?: number | null;
  error?: string | null;
};

export type ProviderConnectionState =
  | "connected"
  | "connecting"
  | "disconnected"
  | "error";

export type OpenAiProviderConnectionState = ProviderConnectionState;

export type OpenAiProviderStatus = {
  connected: boolean;
  connectionState: OpenAiProviderConnectionState;
  email?: string | null;
  accountId?: string | null;
  planType?: string | null;
  expiresAtMs?: number | null;
  lastRefreshMs?: number | null;
  loginId?: string | null;
  error?: string | null;
};

export type AnthropicProviderStatus = {
  connected: boolean;
  connectionState: ProviderConnectionState;
  expiresAtMs?: number | null;
  lastRefreshMs?: number | null;
  loginId?: string | null;
  error?: string | null;
};

export type StartOpenAiLoginOutput = {
  loginId: string;
  authUrl: string;
};

export type StartAnthropicLoginOutput = {
  loginId: string;
  authUrl: string;
};

export type GoogleProviderStatus = {
  connected: boolean;
  connectionState: ProviderConnectionState;
  email?: string | null;
  projectId?: string | null;
  userTier?: string | null;
  expiresAtMs?: number | null;
  lastRefreshMs?: number | null;
  loginId?: string | null;
  error?: string | null;
};

export type StartGoogleLoginOutput = {
  loginId: string;
  authUrl: string;
};

export type KimiProviderStatus = {
  connected: boolean;
  connectionState: ProviderConnectionState;
  expiresAtMs?: number | null;
  lastRefreshMs?: number | null;
  loginId?: string | null;
  error?: string | null;
};

export type StartKimiLoginOutput = {
  loginId: string;
  authUrl: string;
  userCode: string;
};

export type OpenRouterProviderStatus = {
  connected: boolean;
  connectionState: ProviderConnectionState;
  keyPreview?: string | null;
  lastValidatedMs?: number | null;
  modelCount: number;
  error?: string | null;
};

export type OpenRouterModel = {
  id: string;
  name: string;
  contextWindow: number;
  maxOutputTokens: number;
  supportsImages: boolean;
  supportsThinking: boolean;
  supportsTools: boolean;
  addedAtMs?: number | null;
};

export type OpenRouterModelSearchResult = Omit<OpenRouterModel, "addedAtMs">;

export type McpEnvVar = {
  key: string;
  value: string;
};

export type McpServerConfig = {
  id: string;
  name: string;
  command: string;
  args: string[];
  env: McpEnvVar[];
  cwd?: string | null;
  enabled: boolean;
};

export type McpSettings = {
  servers: McpServerConfig[];
};

export type McpToolInfo = {
  serverId: string;
  serverName: string;
  name: string;
  toolName: string;
  title?: string | null;
  description?: string | null;
};

export type McpServerProbe = {
  serverId: string;
  serverName: string;
  enabled: boolean;
  ok: boolean;
  tools: McpToolInfo[];
  error?: string | null;
};

export type SkillSource = "workspace" | "global";

export type InstalledSkill = {
  name: string;
  description?: string | null;
  source: SkillSource;
  rootLabel: string;
  absolutePath: string;
  content: string;
  enabled: boolean;
};

export type SkillConfig = {
  name: string;
  enabled: boolean;
};

export type SkillSettings = {
  skills: SkillConfig[];
};

export type PlanControl = "stopQuestions" | "updatePlan" | "implementPlan";

export type MessageVisibility = "normal" | "systemReminder";

export type PlanArtifact = {
  path: string;
  absolutePath?: string;
  title?: string;
  updatedAtMs?: number;
};

export type PlanWorkflowState =
  | { status: "idle" }
  | { status: "planningQuestions" }
  | { status: "planReady"; artifact: PlanArtifact };

export type GoalWorkflowState =
  | { status: "idle" }
  | {
      status: "active";
      objective: string;
      startedAtMs: number;
      updatedAtMs: number;
    }
  | {
      status: "paused";
      objective: string;
      startedAtMs: number;
      updatedAtMs: number;
    }
  | {
      status: "complete";
      objective: string;
      startedAtMs: number;
      completedAtMs: number;
    };

export type GitStatusKind =
  | "modified"
  | "added"
  | "deleted"
  | "untracked"
  | "renamed"
  | "conflicted";

export type GitStatusFile = {
  path: string;
  oldPath?: string | null;
  indexStatus: string;
  worktreeStatus: string;
  kind: GitStatusKind | string;
  staged: boolean;
};

export type GitWorktree = {
  name: string;
  path: string;
  branch?: string | null;
  head?: string | null;
  isCurrent: boolean;
  dirty: boolean;
  dirtyCount: number;
};

export type GitBranchKind = "local" | "remote";

export type GitBranch = {
  name: string;
  kind: GitBranchKind | string;
  current: boolean;
  upstream?: string | null;
};

export type GitRepositorySnapshot = {
  gitAvailable: boolean;
  ghAvailable: boolean;
  isRepository: boolean;
  workspacePath: string;
  repoRoot?: string | null;
  currentBranch?: string | null;
  mainBranch?: string | null;
  dirtyCount: number;
  status: GitStatusFile[];
  worktrees: GitWorktree[];
  branches: GitBranch[];
  error?: string | null;
};

export type GitOperationResult = {
  message: string;
  stdout?: string | null;
  stderr?: string | null;
};

export type GitCreateWorktreeOutput = {
  worktreePath: string;
  branch: string;
  pushed: boolean;
  message: string;
  warning?: string | null;
};

export type GitPullRequestOutput = {
  url: string;
  message: string;
};

export type WorkspaceBootstrap = {
  workspace: WorkspaceInfo;
  conversations: ConversationSummary[];
  activeConversation: SavedConversation;
  modeModelSettings: ModeModelSettings;
};

export type WorkspaceEntry = {
  name: string;
  relativePath: string;
  absolutePath: string;
  kind: "file" | "directory";
  hasChildren: boolean;
};

export type WorkspaceDeletedEntry = {
  name: string;
  relativePath: string;
  originalAbsolutePath: string;
  trashPath: string;
  kind: WorkspaceEntry["kind"];
};

export type FileDocument = {
  name: string;
  relativePath: string;
  absolutePath: string;
  editable: boolean;
  content: string | null;
  reason: string | null;
  size: number;
  lastModifiedMs: number | null;
  imageMediaType: string | null;
  imageData: string | null;
};

export type TerminalPathKind = "file" | "directory" | "missing";

export type TerminalPathResolution = {
  kind: TerminalPathKind;
  absolutePath: string;
  relativePath: string | null;
  isOutsideWorkspace: boolean;
  line: number | null;
  column: number | null;
};

export type WorkspaceSearchMatch = {
  lineNumber: number;
  columnStart: number;
  columnEnd: number;
  lineText: string;
  matchStart: number;
  matchEnd: number;
};

export type WorkspaceSearchFile = {
  name: string;
  relativePath: string;
  absolutePath: string;
  pathMatch: boolean;
  matchCount: number;
  matches: WorkspaceSearchMatch[];
};

export type WorkspaceSearchResult = {
  query: string;
  filesScanned: number;
  totalMatches: number;
  files: WorkspaceSearchFile[];
};

export type EditorRevealTarget = {
  id: number;
  relativePath: string;
  lineNumber: number;
  columnStart: number;
  columnEnd: number;
  query: string;
};

export type DiffLineKind = "context" | "added" | "removed";

export type DiffLine = {
  kind: DiffLineKind;
  text: string;
};

export type FileChangeKind = "added" | "modified" | "deleted";

export type FileChange = {
  relativePath: string;
  kind: FileChangeKind;
  summary: string;
  binary: boolean;
  addedLines?: number;
  removedLines?: number;
  truncated: boolean;
  lines: DiffLine[];
};

// Agent event stream - tagged union on `type`.
export type AgentEvent =
  | { type: "turn_started" }
  | { type: "text_started" }
  | { type: "text_chunk"; delta: string }
  | { type: "text_finished" }
  | { type: "thinking_started" }
  | { type: "thinking_chunk"; delta: string }
  | { type: "thinking_finished" }
  | { type: "tool_started"; id: string; name: string }
  | { type: "tool_args_delta"; id: string; delta: string }
  | { type: "tool_output_delta"; id: string; delta: string }
  | { type: "tool_ready"; id: string; summary: string; args_pretty: string }
  | {
      type: "tool_finished";
      id: string;
      output: string;
      is_error: boolean;
      file_changes: FileChange[];
      images: ToolResultImage[];
      meta?: Record<string, unknown> | null;
    }
  | {
      type: "token_usage";
      provider: string;
      model: string;
      context_window: number;
      preferred_window: number;
      max_output_tokens: number;
      usage: StreamTokenUsage;
    }
  | { type: "interrupted" }
  | { type: "error"; message: string }
  | {
      type: "peer_message_received";
      id: string;
      from: string;
      to: string;
      message: string;
    }
  | {
      type: "sub_agent_event";
      id: string;
      agent_id: string;
      agent_name: string;
      team_name?: string;
      model?: ModelRef;
      initial_message?: string;
      event: AgentEvent;
    }
  | { type: "agent_slept" }
  | { type: "turn_finished" };

export type StreamTokenUsage = {
  input_tokens: number;
  output_tokens: number;
  total_tokens: number;
  reasoning_tokens: number;
  cache_read_tokens: number;
  cache_creation_tokens: number;
};

export type ConversationEventPayload = {
  conversationId: string;
  workspaceId?: string;
  sequence?: number;
  event: AgentEvent;
};

export type SequencedAgentEvent = {
  sequence: number;
  event: AgentEvent;
};

export type ActiveTurnSummary = {
  workspaceId: string;
  conversationId: string;
  startedAtMs: number;
  latestSequence: number;
};

export type ActiveTurnsChangedPayload = {
  activeTurns: ActiveTurnSummary[];
};

export type ActiveTurnReplay = {
  active: boolean;
  workspaceId: string;
  conversationId: string;
  startedAtMs?: number | null;
  latestSequence: number;
  events: SequencedAgentEvent[];
};

export type WorkspaceFileChangedPayload = {
  workspacePath: string;
  relativePath: string;
};

// Attachment payload for send_message.
export type AttachmentInput = {
  path: string;
  name?: string;
};

export type ClipboardImageAttachment = {
  path: string;
  name: string;
};

export type ThinkingLevel =
  | "off"
  | "minimal"
  | "low"
  | "medium"
  | "high"
  | "max"
  | "xhigh";

export type TerminalCommandResult = {
  content: string;
  isError: boolean;
};

export type TerminalSpawnResult = {
  sessionId: string;
};

export type TerminalDataPayload = {
  sessionId: string;
  token: string;
  data: string;
};

export type TerminalExitPayload = {
  sessionId: string;
  token: string;
  exitCode?: number | null;
  signal?: string | null;
};

export type ContextEstimate = {
  usedTokens: number;
  contextWindow: number;
  preferredWindow: number;
  maxOutputTokens: number;
  inputTokens: number;
  outputTokens: number;
  reasoningTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  exact: boolean;
  error?: string | null;
  breakdown: ContextBreakdownItem[];
};

export type ContextBreakdownItem = {
  key: string;
  label: string;
  tokens: number;
};

// A loaded tab in the editor.
export type EditorTab = {
  relativePath: string;
  doc: FileDocument;
  // Current editor buffer; diverges from `doc.content` when dirty.
  buffer: string;
  dirty: boolean;
  // When set, the tab represents a file that lives *outside* the active
  // workspace (typically opened from the terminal). External tabs are
  // always rendered read-only and ignore save / rename / file-tree
  // events.
  external?: boolean;
};

// Recent workspace entry persisted in localStorage.
export type RecentWorkspace = {
  path: string;
  name: string;
  lastOpenedMs: number;
};
