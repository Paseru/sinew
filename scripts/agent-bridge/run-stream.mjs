#!/usr/bin/env node
/**
 * Minimal agent.v1 Run bridge for Sinew.
 * stdin: line 1 = config JSON; further lines = tool_response from Sinew
 * stdout: NDJSON text/thinking/error/tool_request
 */
import http2 from "node:http2";
import { createHash, randomUUID } from "node:crypto";
import path from "node:path";
import readline from "node:readline";
import {
  buildProjectLayout,
  handleDeleteArgs,
  handleLsArgs,
  handleReadArgs,
  handleWriteArgs,
} from "./exec-handlers.mjs";
import { create, toBinary, fromBinary } from "@bufbuild/protobuf";
import {
  AgentClientMessageSchema,
  AgentRunRequestSchema,
  AgentServerMessageSchema,
  AssistantMessageSchema,
  ClientHeartbeatSchema,
  ConversationActionSchema,
  ConversationStateStructureSchema,
  ConversationStepSchema,
  AgentConversationTurnStructureSchema,
  ConversationTurnStructureSchema,
  BackgroundShellSpawnResultSchema,
  DeleteRejectedSchema,
  DeleteResultSchema,
  DiagnosticsResultSchema,
  ExecClientMessageSchema,
  FetchErrorSchema,
  FetchResultSchema,
  GetBlobResultSchema,
  GrepErrorSchema,
  GrepResultSchema,
  LsRejectedSchema,
  LsResultSchema,
  ReadRejectedSchema,
  ReadResultSchema,
  ShellRejectedSchema,
  ShellResultSchema,
  WriteRejectedSchema,
  WriteResultSchema,
  KvClientMessageSchema,
  LsDirectoryTreeNodeSchema,
  McpErrorSchema,
  McpInstructionsSchema,
  McpRejectedSchema,
  McpResultSchema,
  McpTextContentSchema,
  McpToolDefinitionSchema,
  McpToolResultContentItemSchema,
  ModelDetailsSchema,
  RequestContextEnvSchema,
  RequestContextResultSchema,
  RequestContextSchema,
  RequestContextSuccessSchema,
  SetBlobResultSchema,
  WriteShellStdinErrorSchema,
  WriteShellStdinResultSchema,
  UserMessageActionSchema,
  UserMessageSchema,
} from "./vendor/agent_pb.ts";

const CONNECT_END = 0b00000010;
let outputTokenCount = 0;
let totalTokenCount = 0;

function sha256Hex(input) {
  return createHash("sha256").update(input, "utf8").digest("hex");
}

function cursorChecksum(accessToken) {
  const machineId = sha256Hex(`${accessToken}machineId`);
  const millis = Date.now();
  const bucket = Math.floor(millis / 1_000_000);
  const bytes = [
    (bucket >> 40) & 0xff,
    (bucket >> 32) & 0xff,
    (bucket >> 24) & 0xff,
    (bucket >> 16) & 0xff,
    (bucket >> 8) & 0xff,
    bucket & 0xff,
  ];
  let state = 165;
  for (let index = 0; index < bytes.length; index++) {
    let x = (bytes[index] ^ state) + (index % 256);
    x &= 0xff;
    bytes[index] = x;
    state = x;
  }
  const encoded = Buffer.from(bytes).toString("base64url");
  return `${encoded}${machineId}`;
}

function frameConnect(data, flags = 0) {
  const frame = Buffer.alloc(5 + data.length);
  frame[0] = flags;
  frame.writeUInt32BE(data.length, 1);
  frame.set(data, 5);
  return frame;
}

function parseConnectEnd(data) {
  try {
    const payload = JSON.parse(Buffer.from(data).toString("utf8"));
    if (payload?.error) {
      return `${payload.error.code}: ${payload.error.message}`;
    }
  } catch {
    /* ignore */
  }
  return null;
}

function storeBlob(blobStore, data) {
  const blobId = new Uint8Array(createHash("sha256").update(data).digest());
  blobStore.set(Buffer.from(blobId).toString("hex"), data);
  return blobId;
}

function buildMcpToolDefinitions(tools) {
  if (!Array.isArray(tools)) return [];
  return tools.map((tool) => {
    const schema = tool.parameters || { type: "object", properties: {}, required: [] };
    const inputSchema = new TextEncoder().encode(JSON.stringify(schema));
    return create(McpToolDefinitionSchema, {
      name: tool.name,
      toolName: tool.name,
      description: tool.description || "",
      providerIdentifier: "sinew",
      inputSchema,
    });
  });
}

function decodeMcpArgs(argsMap) {
  const decoded = {};
  for (const [key, value] of Object.entries(argsMap || {})) {
    if (value instanceof Uint8Array) {
      try {
        decoded[key] = JSON.parse(new TextDecoder().decode(value));
      } catch {
        decoded[key] = new TextDecoder().decode(value);
      }
    } else {
      decoded[key] = value;
    }
  }
  return decoded;
}

function restoreBlobStore(blobs) {
  const blobStore = new Map();
  if (blobs && typeof blobs === "object") {
    for (const [hex, b64] of Object.entries(blobs)) {
      if (typeof b64 === "string") {
        blobStore.set(hex, Buffer.from(b64, "base64"));
      }
    }
  }
  return blobStore;
}

function buildRootPromptMessagesJson(systemPrompt, turns, blobStore) {
  const ids = [];
  ids.push(
    storeBlob(
      blobStore,
      new TextEncoder().encode(
        JSON.stringify({ role: "system", content: systemPrompt || "" }),
      ),
    ),
  );
  for (const turn of turns || []) {
    const userText = turn.user_text || turn.userText || "";
    const assistantText = turn.assistant_text || turn.assistantText || "";
    if (userText.trim()) {
      ids.push(
        storeBlob(
          blobStore,
          new TextEncoder().encode(
            JSON.stringify({
              role: "user",
              content: [{ type: "text", text: userText }],
            }),
          ),
        ),
      );
    }
    if (assistantText.trim()) {
      ids.push(
        storeBlob(
          blobStore,
          new TextEncoder().encode(
            JSON.stringify({
              role: "assistant",
              content: [{ type: "text", text: assistantText }],
            }),
          ),
        ),
      );
    }
  }
  return ids;
}

function buildTurnBlobIds(turns, blobStore) {
  const turnIds = [];
  for (const turn of turns || []) {
    const userText = turn.user_text || turn.userText || "";
    if (!userText.trim()) continue;
    const userMsg = create(UserMessageSchema, {
      text: userText,
      messageId: randomUUID(),
    });
    const userBlobId = storeBlob(blobStore, toBinary(UserMessageSchema, userMsg));
    const stepBlobIds = [];
    const assistantText = turn.assistant_text || turn.assistantText || "";
    if (assistantText.trim()) {
      const step = create(ConversationStepSchema, {
        message: {
          case: "assistantMessage",
          value: create(AssistantMessageSchema, { text: assistantText }),
        },
      });
      stepBlobIds.push(storeBlob(blobStore, toBinary(ConversationStepSchema, step)));
    }
    const agentTurn = create(AgentConversationTurnStructureSchema, {
      userMessage: userBlobId,
      steps: stepBlobIds,
    });
    const turnStructure = create(ConversationTurnStructureSchema, {
      turn: { case: "agentConversationTurn", value: agentTurn },
    });
    turnIds.push(storeBlob(blobStore, toBinary(ConversationTurnStructureSchema, turnStructure)));
  }
  return turnIds;
}

function loadCheckpointState(config, blobStore) {
  if (!config.checkpointB64) return null;
  try {
    const bytes = Buffer.from(config.checkpointB64, "base64");
    return fromBinary(ConversationStateStructureSchema, bytes);
  } catch (err) {
    debug(`checkpoint load failed: ${err}`);
    return null;
  }
}

function buildRequest(modelId, systemPrompt, userText, conversationId, config) {
  const blobStore = restoreBlobStore(config.blobs);
  const historyTurns = config.turns || [];
  const loaded = loadCheckpointState(config, blobStore);

  const conversationState =
    loaded ??
    create(ConversationStateStructureSchema, {
      rootPromptMessagesJson: buildRootPromptMessagesJson(
        systemPrompt,
        historyTurns,
        blobStore,
      ),
      turns: buildTurnBlobIds(historyTurns, blobStore),
      todos: [],
      pendingToolCalls: [],
      previousWorkspaceUris: [],
      fileStates: {},
      fileStatesV2: {},
      summaryArchives: [],
      turnTimings: [],
      subagentStates: {},
      selfSummaryCount: 0,
      readPaths: [],
    });

  const action = create(ConversationActionSchema, {
    action: {
      case: "userMessageAction",
      value: create(UserMessageActionSchema, {
        userMessage: create(UserMessageSchema, {
          text: userText,
          messageId: randomUUID(),
        }),
      }),
    },
  });
  // Current user turn lives in action only; history turn blobs use blob-id refs (see opencode buildCursorRequest).

  const modelDetails = create(ModelDetailsSchema, {
    modelId,
    displayModelId: modelId,
    displayName: modelId,
  });

  const runRequest = create(AgentRunRequestSchema, {
    conversationState,
    action,
    modelDetails,
    conversationId: conversationId || randomUUID(),
  });

  const clientMessage = create(AgentClientMessageSchema, {
    message: { case: "runRequest", value: runRequest },
  });

  return { requestBytes: toBinary(AgentClientMessageSchema, clientMessage), blobStore };
}

function sendExecResult(execMsg, resultCase, result, sendFrame) {
  const execClient = create(ExecClientMessageSchema, {
    execId: execMsg.execId,
    id: execMsg.id,
    message: { case: resultCase, value: result },
  });
  const clientMsg = create(AgentClientMessageSchema, {
    message: { case: "execClientMessage", value: execClient },
  });
  sendFrame(frameConnect(toBinary(AgentClientMessageSchema, clientMsg)));
}

function buildRequestContext(workspaceRoot, snapshot, tools) {
  const root = workspaceRoot?.trim() || process.cwd();
  const projectFolder = path.join(
    process.env.USERPROFILE || process.env.HOME || root,
    ".cursor",
    "projects",
    "sinew-bridge",
  );
  const layout = buildProjectLayout(root, snapshot);
  const mcpTools = buildMcpToolDefinitions(tools);
  return create(RequestContextSchema, {
    rules: [],
    env: create(RequestContextEnvSchema, {
      osVersion: `${process.platform} ${process.version}`,
      workspacePaths: [root],
      shell: process.env.ComSpec || process.env.SHELL || "",
      sandboxEnabled: false,
      terminalsFolder: path.join(projectFolder, "terminals"),
      agentSharedNotesFolder: path.join(projectFolder, "shared-notes"),
      agentConversationNotesFolder: path.join(projectFolder, "conversation-notes"),
      timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC",
      projectFolder,
      agentTranscriptsFolder: path.join(projectFolder, "transcripts"),
    }),
    repositoryInfo: [],
    tools: mcpTools,
    gitRepos: [],
    projectLayouts: layout ? [layout] : [],
    mcpInstructions: [
      create(McpInstructionsSchema, {
        serverName: "sinew",
        instructions: [
          `Workspace root: ${root}.`,
          "Use the MCP tools listed in this request context (Read, Grep, Bash, etc.).",
        ].join("\n"),
      }),
    ],
    fileContents: {},
    customSubagents: [],
  });
}

const REJECT = "Tool not available in Sinew agent bridge spike.";

async function handleExecMessage(execMsg, sendFrame, emit, waitToolResponse) {
  const execCase = execMsg.message?.case;
  debug(`exec ${execCase ?? "?"}`);
  const send = (msg, resultCase, result) => sendExecResult(msg, resultCase, result, sendFrame);
  if (execCase === "requestContextArgs") {
    const requestContext = buildRequestContext(
      config.workspaceRoot,
      config.workspaceSnapshot,
      config.tools,
    );
    const result = create(RequestContextResultSchema, {
      result: {
        case: "success",
        value: create(RequestContextSuccessSchema, { requestContext }),
      },
    });
    sendExecResult(execMsg, "requestContextResult", result, sendFrame);
    return;
  }
  if (execCase === "readArgs") {
    handleReadArgs(execMsg, execMsg.message.value, config.workspaceRoot, send);
    return;
  }
  if (execCase === "lsArgs") {
    handleLsArgs(execMsg, execMsg.message.value, config.workspaceRoot, send);
    return;
  }
  if (execCase === "grepArgs") {
    sendExecResult(
      execMsg,
      "grepResult",
      create(GrepResultSchema, {
        result: { case: "error", value: create(GrepErrorSchema, { error: REJECT }) },
      }),
      sendFrame,
    );
    return;
  }
  if (execCase === "writeArgs" || execCase === "editArgs") {
    handleWriteArgs(execMsg, execMsg.message.value, config.workspaceRoot, send);
    return;
  }
  if (execCase === "deleteArgs") {
    handleDeleteArgs(execMsg, execMsg.message.value, config.workspaceRoot, send);
    return;
  }
  if (execCase === "shellArgs" || execCase === "shellStreamArgs") {
    const args = execMsg.message.value;
    sendExecResult(
      execMsg,
      "shellResult",
      create(ShellResultSchema, {
        result: {
          case: "rejected",
          value: create(ShellRejectedSchema, {
            command: args.command ?? "",
            workingDirectory: args.workingDirectory ?? "",
            reason: REJECT,
            isReadonly: false,
          }),
        },
      }),
      sendFrame,
    );
    return;
  }
  if (execCase === "backgroundShellSpawnArgs") {
    const args = execMsg.message.value;
    sendExecResult(
      execMsg,
      "backgroundShellSpawnResult",
      create(BackgroundShellSpawnResultSchema, {
        result: {
          case: "rejected",
          value: create(ShellRejectedSchema, {
            command: args.command ?? "",
            workingDirectory: args.workingDirectory ?? "",
            reason: REJECT,
            isReadonly: false,
          }),
        },
      }),
      sendFrame,
    );
    return;
  }
  if (execCase === "writeShellStdinArgs") {
    sendExecResult(
      execMsg,
      "writeShellStdinResult",
      create(WriteShellStdinResultSchema, {
        result: { case: "error", value: create(WriteShellStdinErrorSchema, { error: REJECT }) },
      }),
      sendFrame,
    );
    return;
  }
  if (execCase === "fetchArgs") {
    const args = execMsg.message.value;
    sendExecResult(
      execMsg,
      "fetchResult",
      create(FetchResultSchema, {
        result: {
          case: "error",
          value: create(FetchErrorSchema, { url: args.url ?? "", error: REJECT }),
        },
      }),
      sendFrame,
    );
    return;
  }
  if (execCase === "diagnosticsArgs") {
    sendExecResult(execMsg, "diagnosticsResult", create(DiagnosticsResultSchema, {}), sendFrame);
    return;
  }
  const miscCaseMap = {
    listMcpResourcesExecArgs: "listMcpResourcesExecResult",
    readMcpResourceExecArgs: "readMcpResourceExecResult",
    recordScreenArgs: "recordScreenResult",
    computerUseArgs: "computerUseResult",
    setupVmEnvironmentArgs: "setupVmEnvironmentResult",
  };
  const resultCase = miscCaseMap[execCase];
  if (resultCase) {
    sendExecResult(execMsg, resultCase, create(McpResultSchema, {}), sendFrame);
    return;
  }
  if (execCase === "mcpArgs") {
    const mcpArgs = execMsg.message.value;
    const toolName = mcpArgs.toolName || mcpArgs.name || "";
    const args = decodeMcpArgs(mcpArgs.args);
    emit({
      type: "tool_request",
      execId: execMsg.execId,
      execMsgId: execMsg.id,
      toolCallId: mcpArgs.toolCallId || randomUUID(),
      toolName,
      args,
    });
    const resp = await waitToolResponse();
    const content = resp?.content || "Error: empty tool response";
    const isError = Boolean(resp?.isError) || content.startsWith("Error:");
    const mcpResult = isError
      ? create(McpResultSchema, {
          result: { case: "error", value: create(McpErrorSchema, { error: content }) },
        })
      : create(McpResultSchema, {
          result: {
            case: "success",
            value: create(McpSuccessSchema, {
              isError: false,
              content: [
                create(McpToolResultContentItemSchema, {
                  content: {
                    case: "text",
                    value: create(McpTextContentSchema, { text: content }),
                  },
                }),
              ],
            }),
          },
        });
    sendExecResult(execMsg, "mcpResult", mcpResult, sendFrame);
    return;
  }
  debug(`unhandled exec: ${execCase ?? "?"}`);
}

function emitCheckpoint(state, blobStore, emit) {
  try {
    const bytes = toBinary(ConversationStateStructureSchema, state);
    const blobs = {};
    for (const [hex, data] of blobStore.entries()) {
      blobs[hex] = Buffer.from(data).toString("base64");
    }
    emit({
      type: "checkpoint",
      checkpointB64: Buffer.from(bytes).toString("base64"),
      blobs,
    });
  } catch (err) {
    debug(`checkpoint emit failed: ${err}`);
  }
}

function handleServerMessage(msg, blobStore, sendFrame, emit, waitToolResponse) {
  const msgCase = msg.message?.case;
  if (msgCase === "execServerMessage") {
    void handleExecMessage(msg.message.value, sendFrame, emit, waitToolResponse);
  } else if (msgCase === "conversationCheckpointUpdate") {
    emitCheckpoint(msg.message.value, blobStore, emit);
  } else if (msgCase === "interactionUpdate") {
    const u = msg.message.value;
    const c = u.message?.case;
    debug(`interaction ${c ?? "?"}`);
    if (c === "textDelta") {
      const d = u.message.value.text || "";
      if (d) {
        if (!sawText) {
          sawText = true;
          armMaxTurnTimer();
        }
        lastTextAt = Date.now();
        emit({ type: "text", delta: d });
        bumpIdleFinish();
      }
    } else if (c === "thinkingDelta") {
      const d = u.message.value.text || "";
      if (d) emit({ type: "thinking", delta: d });
    } else if (c === "tokenDelta") {
      const delta = u.message.value.tokens ?? 0;
      outputTokenCount += delta;
      totalTokenCount = Math.max(totalTokenCount, outputTokenCount);
      emit({
        type: "usage",
        outputTokens: outputTokenCount,
        totalTokens: totalTokenCount,
      });
    } else if (c === "thinkingCompleted") {
      debug("interaction thinkingCompleted");
      if (sawText) bumpIdleFinish();
    } else if (c === "heartbeat") {
      if (sawText && lastTextAt > 0 && Date.now() - lastTextAt >= IDLE_AFTER_TEXT_MS) {
        debug("finish after text + server heartbeat idle");
        gracefulFinish(0);
      }
    } else if (c === "stepCompleted") {
      debug("interaction stepCompleted");
      gracefulFinish(0);
    } else if (c === "turnEnded") {
      debug("interaction turnEnded");
      gracefulFinish(0);
    }
  } else if (msgCase === "kvServerMessage") {
    const kv = msg.message.value;
    if (kv.message?.case === "getBlobArgs") {
      const blobId = kv.message.value.blobId;
      const key = Buffer.from(blobId).toString("hex");
      const blobData = blobStore.get(key);
      const response = create(KvClientMessageSchema, {
        id: kv.id,
        message: {
          case: "getBlobResult",
          value: create(GetBlobResultSchema, blobData ? { blobData } : {}),
        },
      });
      const clientMsg = create(AgentClientMessageSchema, {
        message: { case: "kvClientMessage", value: response },
      });
      sendFrame(frameConnect(toBinary(AgentClientMessageSchema, clientMsg)));
    } else if (kv.message?.case === "setBlobArgs") {
      const { blobId, blobData } = kv.message.value;
      blobStore.set(Buffer.from(blobId).toString("hex"), blobData);
      const response = create(KvClientMessageSchema, {
        id: kv.id,
        message: { case: "setBlobResult", value: create(SetBlobResultSchema, {}) },
      });
      const clientMsg = create(AgentClientMessageSchema, {
        message: { case: "kvClientMessage", value: response },
      });
      sendFrame(frameConnect(toBinary(AgentClientMessageSchema, clientMsg)));
    }
  }
}

async function readConfigLine(rl) {
  const line = await new Promise((resolve, reject) => {
    rl.once("line", resolve);
    rl.once("close", () => reject(new Error("stdin closed before config")));
  });
  if (!line?.trim()) throw new Error("no config on stdin");
  return JSON.parse(line);
}

function waitToolResponse(rl) {
  return new Promise((resolve, reject) => {
    rl.once("line", (line) => {
      try {
        resolve(JSON.parse(line));
      } catch (err) {
        reject(err);
      }
    });
    rl.once("close", () => reject(new Error("stdin closed waiting for tool response")));
  });
}

let sawText = false;
let lastTextAt = 0;
let finished = false;
let idleTimer = null;
let maxTurnTimer = null;
let heartbeatTimer = null;
let h2Client = null;
let h2Stream = null;

const IDLE_AFTER_TEXT_MS = 2500;
const MAX_TURN_MS = 120_000;

function gracefulFinish(code = 0) {
  if (finished) return;
  finished = true;
  if (heartbeatTimer) clearInterval(heartbeatTimer);
  if (idleTimer) clearTimeout(idleTimer);
  if (maxTurnTimer) clearTimeout(maxTurnTimer);
  try {
    h2Stream?.end();
  } catch {
    /* ignore */
  }
  setTimeout(() => {
    try {
      h2Client?.close();
    } catch {
      /* ignore */
    }
    process.exit(code);
  }, 300);
}

function armMaxTurnTimer() {
  if (finished || maxTurnTimer) return;
  maxTurnTimer = setTimeout(() => {
    debug(`max turn ${MAX_TURN_MS}ms`);
    gracefulFinish(0);
  }, MAX_TURN_MS);
  maxTurnTimer.unref?.();
}

function bumpIdleFinish() {
  if (finished) return;
  if (idleTimer) clearTimeout(idleTimer);
  idleTimer = setTimeout(() => gracefulFinish(0), IDLE_AFTER_TEXT_MS);
  idleTimer.unref?.();
}

const stdinRl = readline.createInterface({ input: process.stdin, terminal: false });
const config = await readConfigLine(stdinRl);
const {
  accessToken,
  modelId,
  systemPrompt,
  userText,
  workspaceRoot,
  conversationId,
  tools,
  workspaceSnapshot,
} = config;
const waitForTool = () => waitToolResponse(stdinRl);
if (!accessToken || !modelId || !userText) {
  console.log(JSON.stringify({ error: "missing accessToken, modelId, or userText" }));
  process.exit(1);
}

const emit = (obj) => {
  process.stdout.write(`${JSON.stringify(obj)}\n`);
};
const debug = (msg) => {
  process.stderr.write(`[agent-bridge] ${msg}\n`);
};

const apiHeaders =
  config.apiHeaders && typeof config.apiHeaders === "object"
    ? config.apiHeaders
    : {
        "x-cursor-client-type": "cli",
        "x-ghost-mode": "true",
        "x-client-key": sha256Hex(accessToken),
        "x-cursor-checksum": cursorChecksum(accessToken),
        "x-cursor-client-version": "cli-2026.01.09-231024f",
      };

const { requestBytes, blobStore } = buildRequest(
  modelId,
  systemPrompt || "You are Composer in Cursor IDE.",
  userText,
  conversationId,
  config,
);

const sendFrame = (frame) => {
  if (finished || !h2Stream || h2Stream.closed || h2Stream.destroyed) return;
  h2Stream.write(frame);
};

h2Client = http2.connect("https://api2.cursor.sh");
h2Client.on("error", (err) => {
  emit({ error: `h2 client: ${err}` });
  gracefulFinish(1);
});

const h2Headers = {
  ...apiHeaders,
  ":method": "POST",
  ":path": "/agent.v1.AgentService/Run",
  "content-type": "application/connect+proto",
  te: "trailers",
  authorization: `Bearer ${accessToken}`,
  "connect-protocol-version": "1",
};
h2Stream = h2Client.request(h2Headers);
sendFrame(frameConnect(requestBytes));

const heartbeat = create(AgentClientMessageSchema, {
  message: { case: "clientHeartbeat", value: create(ClientHeartbeatSchema, {}) },
});
const heartbeatBytes = () =>
  frameConnect(toBinary(AgentClientMessageSchema, heartbeat));
heartbeatTimer = setInterval(() => {
  sendFrame(heartbeatBytes());
}, 15_000);

let pending = Buffer.alloc(0);
function ingestConnectBytes(chunk) {
  pending = Buffer.concat([pending, chunk]);
  let offset = 0;
  while (pending.length >= offset + 5) {
    const flags = pending[offset];
    const flen = pending.readUInt32BE(offset + 1);
    if (pending.length < offset + 5 + flen) break;
    const frame = pending.subarray(offset + 5, offset + 5 + flen);
    offset += 5 + flen;

    if (flags & CONNECT_END) {
      const err = parseConnectEnd(frame);
      if (err) emit({ error: err });
      continue;
    }
    if (!frame.length) continue;

    try {
      const msg = fromBinary(AgentServerMessageSchema, frame);
      debug(`server case=${msg.message?.case ?? "?"}`);
      handleServerMessage(msg, blobStore, sendFrame, emit, waitForTool);
    } catch (err) {
      debug(`parse err: ${err}`);
    }
  }
  pending = pending.subarray(offset);
}

h2Stream.on("data", ingestConnectBytes);
h2Stream.on("end", () => {
  if (finished) return;
  if (!sawText) {
    emit({ error: "stream ended without text deltas" });
    gracefulFinish(1);
    return;
  }
  gracefulFinish(0);
});
h2Stream.on("error", (err) => {
  emit({ error: `h2 stream: ${err}` });
  gracefulFinish(1);
});
