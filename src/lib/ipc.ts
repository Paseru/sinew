import { invoke } from "@tauri-apps/api/core";
import type {
  AttachmentInput,
  ActiveTurnReplay,
  ActiveTurnSummary,
  AgentMode,
  AnthropicProviderStatus,
  ChatMessage,
  ClipboardImageAttachment,
  ContextEstimate,
  ConversationSummary,
  DatabaseActivityEntry,
  DatabaseConnectionTestResult,
  DatabaseSettings,
  DatabaseSourceConfig,
  FileDocument,
  GitCreateWorktreeOutput,
  GitOperationResult,
  GitPullRequestOutput,
  GitRepositorySnapshot,
  GoogleProviderStatus,
  InstalledSkill,
  KimiProviderStatus,
  MessageVisibility,
  McpServerProbe,
  McpSettings,
  ModeModelSettings,
  ModelRef,
  OpenAiProviderStatus,
  OpenRouterModel,
  OpenRouterModelSearchResult,
  OpenRouterProviderStatus,
  PlanControl,
  QuestionAnswer,
  SavedConversation,
  ServiceTier,
  SkillSettings,
  StartAnthropicLoginOutput,
  StartGoogleLoginOutput,
  StartKimiLoginOutput,
  StartOpenAiLoginOutput,
  SubAgentSettings,
  TerminalCommandResult,
  TerminalPathResolution,
  TerminalSpawnResult,
  ThinkingLevel,
  ToolSettings,
  UpdateInfo,
  WorkspaceBootstrap,
  WorkspaceDeletedEntry,
  WorkspaceEntry,
  WorkspaceSearchResult,
} from "../types";

export const api = {
  openWorkspace(workspacePath: string) {
    return invoke<WorkspaceBootstrap>("open_workspace", {
      input: { workspacePath },
    });
  },
  gitSnapshot(workspacePath: string) {
    return invoke<GitRepositorySnapshot>("git_repository_snapshot_command", {
      input: { workspacePath },
    });
  },
  gitInit(workspacePath: string) {
    return invoke<GitRepositorySnapshot>("git_init_command", {
      input: { workspacePath },
    });
  },
  gitCreateWorktree(
    workspacePath: string,
    branchName: string,
    baseBranch: string | null,
    pushImmediately: boolean,
  ) {
    return invoke<GitCreateWorktreeOutput>("git_create_worktree_command", {
      input: { workspacePath, branchName, baseBranch, pushImmediately },
    });
  },
  gitRemoveWorktree(workspacePath: string, targetPath: string, force: boolean) {
    return invoke<GitOperationResult>("git_remove_worktree_command", {
      input: { workspacePath, targetPath, force },
    });
  },
  gitCreateBranch(
    workspacePath: string,
    branchName: string,
    baseBranch: string | null,
  ) {
    return invoke<GitOperationResult>("git_create_branch_command", {
      input: { workspacePath, branchName, baseBranch },
    });
  },
  gitCommit(workspacePath: string, message: string, paths: string[]) {
    return invoke<GitOperationResult>("git_commit_command", {
      input: { workspacePath, message, paths },
    });
  },
  gitPush(workspacePath: string) {
    return invoke<GitOperationResult>("git_push_command", {
      input: { workspacePath },
    });
  },
  gitPull(workspacePath: string) {
    return invoke<GitOperationResult>("git_pull_command", {
      input: { workspacePath },
    });
  },
  gitCreatePullRequest(
    workspacePath: string,
    title: string,
    body: string,
    targetBranch: string,
  ) {
    return invoke<GitPullRequestOutput>("git_create_pull_request_command", {
      input: { workspacePath, title, body, targetBranch },
    });
  },
  openNewWindow() {
    return invoke<void>("open_new_window");
  },
  resetWindowTitle() {
    return invoke<void>("reset_window_title");
  },
  watchWorkspace(workspacePath: string) {
    return invoke<void>("watch_workspace_command", {
      input: { workspacePath },
    });
  },
  unwatchWorkspace(workspacePath: string) {
    return invoke<boolean>("unwatch_workspace_command", {
      input: { workspacePath },
    });
  },
  listEntries(workspacePath: string, relativePath?: string) {
    return invoke<WorkspaceEntry[]>("list_workspace_entries_command", {
      input: { workspacePath, relativePath },
    });
  },
  listAllFiles(workspacePath: string) {
    return invoke<WorkspaceEntry[]>("list_workspace_files_command", {
      input: { workspacePath },
    });
  },
  searchWorkspace(workspacePath: string, query: string) {
    return invoke<WorkspaceSearchResult>("search_workspace_files_command", {
      input: { workspacePath, query },
    });
  },
  readFile(workspacePath: string, relativePath: string) {
    return invoke<FileDocument>("read_workspace_file_command", {
      input: { workspacePath, relativePath },
    });
  },
  writeFile(workspacePath: string, relativePath: string, content: string) {
    return invoke<FileDocument>("write_workspace_file_command", {
      input: { workspacePath, relativePath, content },
    });
  },
  createFile(workspacePath: string, targetRelativePath: string | null, name: string) {
    return invoke<WorkspaceEntry>("create_workspace_file_command", {
      input: { workspacePath, targetRelativePath, name },
    });
  },
  createDirectory(
    workspacePath: string,
    targetRelativePath: string | null,
    name: string,
  ) {
    return invoke<WorkspaceEntry>("create_workspace_directory_command", {
      input: { workspacePath, targetRelativePath, name },
    });
  },
  renameEntry(workspacePath: string, relativePath: string, newName: string) {
    return invoke<WorkspaceEntry>("rename_workspace_entry_command", {
      input: { workspacePath, relativePath, newName },
    });
  },
  deleteEntry(workspacePath: string, relativePath: string) {
    return invoke<void>("delete_workspace_entry_command", {
      input: { workspacePath, relativePath },
    });
  },
  trashEntry(workspacePath: string, relativePath: string) {
    return invoke<WorkspaceDeletedEntry>("trash_workspace_entry_command", {
      input: { workspacePath, relativePath },
    });
  },
  restoreDeletedEntries(
    workspacePath: string,
    entries: WorkspaceDeletedEntry[],
  ) {
    return invoke<WorkspaceEntry[]>("restore_workspace_deleted_entries_command", {
      input: { workspacePath, entries },
    });
  },
  revealEntry(workspacePath: string, relativePath: string) {
    return invoke<void>("reveal_workspace_entry_command", {
      input: { workspacePath, relativePath },
    });
  },
  revealAbsolutePath(path: string) {
    return invoke<void>("reveal_absolute_path_command", {
      input: { path },
    });
  },
  resolveTerminalPath(workspacePath: string, rawPath: string) {
    return invoke<TerminalPathResolution>("resolve_terminal_path_command", {
      input: { workspacePath, rawPath },
    });
  },
  readExternalFile(path: string) {
    return invoke<FileDocument>("read_external_file_command", {
      input: { path },
    });
  },
  deleteSkill(workspacePath: string, path: string) {
    return invoke<void>("delete_skill_command", {
      input: { workspacePath, path },
    });
  },
  createSkill(
    workspacePath: string,
    name: string,
    content: string,
    scope: "global" | "workspace" = "global",
  ) {
    return invoke<{ absolutePath: string }>("create_skill_command", {
      input: { workspacePath, name, content, scope },
    });
  },
  updateSkill(workspacePath: string, path: string, content: string) {
    return invoke<void>("update_skill_command", {
      input: { workspacePath, path, content },
    });
  },
  openExternalUrl(url: string) {
    return invoke<void>("open_external_url_command", {
      input: { url },
    });
  },
  openPathWithDefaultApp(path: string) {
    return invoke<void>("open_path_with_default_app_command", {
      input: { path },
    });
  },
  copyFileToPath(sourcePath: string, destinationPath: string) {
    return invoke<void>("copy_file_to_path_command", {
      input: { sourcePath, destinationPath },
    });
  },
  copyEntries(
    workspacePath: string,
    sources: string[],
    targetRelativePath: string | null,
    cut: boolean,
  ) {
    return invoke<WorkspaceEntry[]>("copy_workspace_entries_command", {
      input: { workspacePath, sources, targetRelativePath, cut },
    });
  },
  importPaths(
    workspacePath: string,
    sources: string[],
    targetRelativePath?: string,
  ) {
    return invoke<{ sourcePath: string; relativePath: string }[]>(
      "import_workspace_paths_command",
      { input: { workspacePath, sources, targetRelativePath } },
    );
  },
  readClipboardFilePaths() {
    return invoke<string[]>("read_clipboard_file_paths_command");
  },
  saveClipboardImage(
    workspacePath: string,
    name: string | null,
    mediaType: string,
    data: string,
  ) {
    return invoke<ClipboardImageAttachment>(
      "save_clipboard_image_attachment_command",
      {
        input: { workspacePath, name, mediaType, data },
      },
    );
  },
  listConversations(workspacePath: string) {
    return invoke<ConversationSummary[]>("list_conversations", {
      input: { workspacePath },
    });
  },
  createConversation(workspacePath: string) {
    return invoke<WorkspaceBootstrap>("create_conversation", {
      input: { workspacePath },
    });
  },
  loadConversation(workspacePath: string, conversationId: string) {
    return invoke<SavedConversation>("load_conversation", {
      input: { workspacePath, conversationId },
    });
  },
  renameConversation(
    workspacePath: string,
    conversationId: string,
    title: string,
  ) {
    return invoke<ConversationSummary[]>("rename_conversation", {
      input: { workspacePath, conversationId, title },
    });
  },
  deleteConversation(workspacePath: string, conversationId: string) {
    return invoke<WorkspaceBootstrap>("delete_conversation", {
      input: { workspacePath, conversationId },
    });
  },
  setConversationMode(
    workspacePath: string,
    conversationId: string,
    mode: AgentMode,
  ) {
    return invoke<SavedConversation>("set_conversation_mode", {
      input: { workspacePath, conversationId, mode },
    });
  },
  setConversationModelPreference(
    workspacePath: string,
    conversationId: string,
    mode: AgentMode,
    model: ModelRef,
    thinking: ThinkingLevel,
    use1mContext?: boolean,
  ) {
    return invoke<ModeModelSettings>("set_conversation_model_preference", {
      input: {
        workspacePath,
        conversationId,
        mode,
        model,
        thinking,
        use1mContext,
      },
    });
  },
  listMcpSettings() {
    return invoke<McpSettings>("list_mcp_settings");
  },
  saveMcpSettings(settings: McpSettings) {
    return invoke<McpSettings>("save_mcp_settings", {
      input: { settings },
    });
  },
  listToolSettings(workspacePath: string) {
    return invoke<ToolSettings>("list_tool_settings", {
      input: { workspacePath },
    });
  },
  saveToolSettings(workspacePath: string, settings: ToolSettings) {
    return invoke<ToolSettings>("save_tool_settings", {
      input: { workspacePath, settings },
    });
  },
  listDatabaseSettings() {
    return invoke<DatabaseSettings>("list_database_settings");
  },
  saveDatabaseSettings(settings: DatabaseSettings) {
    return invoke<DatabaseSettings>("save_database_settings", {
      input: { settings },
    });
  },
  testDatabaseConnection(source: DatabaseSourceConfig) {
    return invoke<DatabaseConnectionTestResult>("test_database_connection", {
      input: { source },
    });
  },
  listDatabaseSourceActivity(sourceId: string, limit?: number) {
    return invoke<DatabaseActivityEntry[]>("list_database_source_activity", {
      input: { sourceId, limit },
    });
  },
  clearDatabaseSourceActivity(sourceId: string) {
    return invoke<DatabaseActivityEntry[]>("clear_database_source_activity", {
      input: { sourceId },
    });
  },
  listSubAgentSettings() {
    return invoke<SubAgentSettings>("list_sub_agent_settings");
  },
  saveSubAgentSettings(settings: SubAgentSettings) {
    return invoke<SubAgentSettings>("save_sub_agent_settings", {
      input: { settings },
    });
  },
  listConfiguredModelProviders() {
    return invoke<string[]>("list_configured_model_providers");
  },
  getOpenAiProviderStatus() {
    return invoke<OpenAiProviderStatus>("get_openai_provider_status");
  },
  startOpenAiOAuthLogin() {
    return invoke<StartOpenAiLoginOutput>("start_openai_oauth_login");
  },
  cancelOpenAiOAuthLogin() {
    return invoke<OpenAiProviderStatus>("cancel_openai_oauth_login");
  },
  disconnectOpenAiProvider() {
    return invoke<OpenAiProviderStatus>("disconnect_openai_provider");
  },
  getAnthropicProviderStatus() {
    return invoke<AnthropicProviderStatus>("get_anthropic_provider_status");
  },
  startAnthropicOAuthLogin() {
    return invoke<StartAnthropicLoginOutput>("start_anthropic_oauth_login");
  },
  cancelAnthropicOAuthLogin() {
    return invoke<AnthropicProviderStatus>("cancel_anthropic_oauth_login");
  },
  disconnectAnthropicProvider() {
    return invoke<AnthropicProviderStatus>("disconnect_anthropic_provider");
  },
  getGoogleProviderStatus() {
    return invoke<GoogleProviderStatus>("get_google_provider_status");
  },
  startGoogleOAuthLogin() {
    return invoke<StartGoogleLoginOutput>("start_google_oauth_login");
  },
  cancelGoogleOAuthLogin() {
    return invoke<GoogleProviderStatus>("cancel_google_oauth_login");
  },
  disconnectGoogleProvider() {
    return invoke<GoogleProviderStatus>("disconnect_google_provider");
  },
  getKimiProviderStatus() {
    return invoke<KimiProviderStatus>("get_kimi_provider_status");
  },
  startKimiOAuthLogin() {
    return invoke<StartKimiLoginOutput>("start_kimi_oauth_login");
  },
  cancelKimiOAuthLogin() {
    return invoke<KimiProviderStatus>("cancel_kimi_oauth_login");
  },
  disconnectKimiProvider() {
    return invoke<KimiProviderStatus>("disconnect_kimi_provider");
  },
  getOpenRouterProviderStatus() {
    return invoke<OpenRouterProviderStatus>("get_openrouter_provider_status");
  },
  validateOpenRouterApiKey(apiKey: string) {
    return invoke<OpenRouterProviderStatus>("validate_openrouter_api_key", {
      input: { apiKey },
    });
  },
  disconnectOpenRouterProvider() {
    return invoke<OpenRouterProviderStatus>("disconnect_openrouter_provider");
  },
  listOpenRouterModels() {
    return invoke<OpenRouterModel[]>("list_openrouter_models");
  },
  searchOpenRouterModels(query: string) {
    return invoke<OpenRouterModelSearchResult[]>("search_openrouter_models", {
      input: { query },
    });
  },
  addOpenRouterModel(model: OpenRouterModelSearchResult) {
    return invoke<OpenRouterModel[]>("add_openrouter_model", {
      input: { model },
    });
  },
  removeOpenRouterModel(id: string) {
    return invoke<OpenRouterModel[]>("remove_openrouter_model", {
      input: { id },
    });
  },
  probeMcpTools() {
    return invoke<McpServerProbe[]>("probe_mcp_tools");
  },
  listInstalledSkills(workspacePath: string) {
    return invoke<InstalledSkill[]>("list_installed_skills_command", {
      input: { workspacePath },
    });
  },
  saveSkillSettings(workspacePath: string, settings: SkillSettings) {
    return invoke<InstalledSkill[]>("save_skill_settings", {
      input: { workspacePath, settings },
    });
  },
  sendMessage(
    workspacePath: string,
    conversationId: string,
    text: string,
    attachments: AttachmentInput[],
    model: ModelRef,
    thinking: ThinkingLevel,
    mode: AgentMode,
    serviceTier?: ServiceTier | null,
    rewriteFromHistoryIndex?: number,
    planControl?: PlanControl,
    messageVisibility?: MessageVisibility,
    revertWorkspaceChanges?: boolean,
    use1mContext?: boolean,
  ) {
    return invoke<void>("send_message", {
      input: {
        workspacePath,
        conversationId,
        text,
        attachments,
        model,
        thinking,
        use1mContext,
        mode,
        serviceTier,
        rewriteFromHistoryIndex,
        planControl,
        messageVisibility,
        revertWorkspaceChanges,
      },
    });
  },
  compactConversation(
    workspacePath: string,
    conversationId: string,
    model: ModelRef,
    thinking: ThinkingLevel,
    serviceTier?: ServiceTier | null,
    instruction?: string,
    use1mContext?: boolean,
  ) {
    return invoke<void>("compact_conversation", {
      input: {
        workspacePath,
        conversationId,
        model,
        thinking,
        serviceTier,
        use1mContext,
        instruction,
      },
    });
  },
  listActiveTurns() {
    return invoke<ActiveTurnSummary[]>("list_active_turns");
  },
  replayActiveTurnEvents(
    workspacePath: string,
    conversationId: string,
    afterSequence?: number,
  ) {
    return invoke<ActiveTurnReplay>("replay_active_turn_events", {
      input: { workspacePath, conversationId, afterSequence },
    });
  },
  estimateContext(
    workspacePath: string,
    conversationId: string,
    text: string,
    attachments: AttachmentInput[],
    model: ModelRef,
    thinking: ThinkingLevel,
    mode: AgentMode,
    rewriteFromHistoryIndex?: number,
    use1mContext?: boolean,
  ) {
    return invoke<ContextEstimate>("estimate_context", {
      input: {
        workspacePath,
        conversationId,
        text,
        attachments,
        model,
        thinking,
        use1mContext,
        mode,
        rewriteFromHistoryIndex,
      },
    });
  },
  estimateSubAgentContext(
    workspacePath: string,
    agentId: string,
    agentName: string | undefined,
    history: ChatMessage[],
    model: ModelRef,
    mode: AgentMode,
  ) {
    return invoke<ContextEstimate>("estimate_sub_agent_context", {
      input: {
        workspacePath,
        agentId,
        agentName,
        history,
        model,
        mode,
      },
    });
  },
  cancelTurn(workspacePath: string, conversationId: string) {
    return invoke<boolean>("cancel_turn", {
      input: { workspacePath, conversationId },
    });
  },
  answerQuestion(
    workspacePath: string,
    conversationId: string,
    toolCallId: string,
    answers: QuestionAnswer[],
    stopQuestions = false,
  ) {
    return invoke<boolean>("answer_question", {
      input: {
        workspacePath,
        conversationId,
        toolCallId,
        answers,
        stopQuestions,
      },
    });
  },
  rejectQuestion(
    workspacePath: string,
    conversationId: string,
    toolCallId: string,
  ) {
    return invoke<boolean>("reject_question", {
      input: { workspacePath, conversationId, toolCallId },
    });
  },
  stopAgentSwarm(
    workspacePath: string,
    conversationId: string,
    teamName?: string,
  ) {
    return invoke<string>("stop_agent_swarm_command", {
      input: { workspacePath, conversationId, teamName },
    });
  },
  runTerminalCommand(workspacePath: string, command: string) {
    return invoke<TerminalCommandResult>("run_terminal_command", {
      input: { workspacePath, command },
    });
  },
  spawnTerminal(
    workspacePath: string,
    sessionId: string,
    token: string,
    cols: number,
    rows: number,
    pixelWidth?: number,
    pixelHeight?: number,
  ) {
    return invoke<TerminalSpawnResult>("spawn_terminal", {
      input: {
        workspacePath,
        sessionId,
        token,
        cols,
        rows,
        pixelWidth,
        pixelHeight,
      },
    });
  },
  writeTerminal(sessionId: string, token: string, data: string) {
    return invoke<void>("write_terminal", {
      input: { sessionId, token, data },
    });
  },
  resizeTerminal(
    sessionId: string,
    token: string,
    cols: number,
    rows: number,
    pixelWidth?: number,
    pixelHeight?: number,
  ) {
    return invoke<void>("resize_terminal", {
      input: { sessionId, token, cols, rows, pixelWidth, pixelHeight },
    });
  },
  killTerminal(sessionId: string, token: string) {
    return invoke<boolean>("kill_terminal", {
      input: { sessionId, token },
    });
  },
  // ── Auto-updater ──────────────────────────────────────────────────────
  // Round-trips to the `tauri-plugin-updater` integration. Progress events
  // (`updater://progress`, `updater://finished`) are consumed by the
  // <UpdateBadge /> component via `@tauri-apps/api/event::listen`.
  checkForUpdate() {
    return invoke<UpdateInfo>("updater_check");
  },
  installUpdate() {
    return invoke<void>("updater_download_and_install");
  },
  restartForUpdate() {
    return invoke<void>("updater_restart");
  },
  currentAppVersion() {
    return invoke<string>("updater_current_version");
  },
};
