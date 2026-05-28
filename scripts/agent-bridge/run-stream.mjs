#!/usr/bin/env node
/**
 * Minimal agent.v1 Run bridge for Sinew.
 * stdin: one JSON line { accessToken, modelId, systemPrompt, userText, workspaceRoot? }
 * stdout: NDJSON { type: "text"|"thinking"|"error", delta?, error? }
 */
import { spawn } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import path from "node:path";
import { fileURLToPath } from "node:url";
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
  McpRejectedSchema,
  McpResultSchema,
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

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const BRIDGE_PATH = path.join(__dirname, "h2-bridge.mjs");
const CONNECT_END = 0b00000010;

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

function lpEncode(buf) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(buf.length, 0);
  return Buffer.concat([len, buf]);
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

function buildRequest(modelId, systemPrompt, userText) {
  const blobStore = new Map();

  const systemJson = JSON.stringify({ role: "system", content: systemPrompt || "" });
  const systemBlobId = storeBlob(blobStore, new TextEncoder().encode(systemJson));

  const conversationState = create(ConversationStateStructureSchema, {
    rootPromptMessagesJson: [systemBlobId],
    turns: [],
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
    conversationId: randomUUID(),
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

function buildRequestContext(workspaceRoot) {
  const root = workspaceRoot?.trim() || process.cwd();
  const projectFolder = path.join(
    process.env.USERPROFILE || process.env.HOME || root,
    ".cursor",
    "projects",
    "sinew-bridge",
  );
  const layout = create(LsDirectoryTreeNodeSchema, {
    absPath: root,
    childrenDirs: [],
    childrenFiles: [],
    childrenWereProcessed: true,
    fullSubtreeExtensionCounts: {},
    numFiles: 0,
  });
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
    tools: [],
    gitRepos: [],
    projectLayouts: [layout],
    mcpInstructions: [],
    fileContents: {},
    customSubagents: [],
  });
}

const REJECT = "Tool not available in Sinew agent bridge spike.";

function handleExecMessage(execMsg, sendFrame) {
  const execCase = execMsg.message?.case;
  debug(`exec ${execCase ?? "?"}`);
  if (execCase === "requestContextArgs") {
    const requestContext = buildRequestContext(config.workspaceRoot);
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
    const args = execMsg.message.value;
    sendExecResult(
      execMsg,
      "readResult",
      create(ReadResultSchema, {
        result: {
          case: "rejected",
          value: create(ReadRejectedSchema, { path: args.path, reason: REJECT }),
        },
      }),
      sendFrame,
    );
    return;
  }
  if (execCase === "lsArgs") {
    const args = execMsg.message.value;
    sendExecResult(
      execMsg,
      "lsResult",
      create(LsResultSchema, {
        result: {
          case: "rejected",
          value: create(LsRejectedSchema, { path: args.path, reason: REJECT }),
        },
      }),
      sendFrame,
    );
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
    const args = execMsg.message.value;
    sendExecResult(
      execMsg,
      "writeResult",
      create(WriteResultSchema, {
        result: {
          case: "rejected",
          value: create(WriteRejectedSchema, { path: args.path, reason: REJECT }),
        },
      }),
      sendFrame,
    );
    return;
  }
  if (execCase === "deleteArgs") {
    const args = execMsg.message.value;
    sendExecResult(
      execMsg,
      "deleteResult",
      create(DeleteResultSchema, {
        result: {
          case: "rejected",
          value: create(DeleteRejectedSchema, { path: args.path, reason: REJECT }),
        },
      }),
      sendFrame,
    );
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
    sendExecResult(
      execMsg,
      "mcpResult",
      create(McpResultSchema, {
        result: {
          case: "rejected",
          value: create(McpRejectedSchema, {
            reason: REJECT,
            isReadonly: false,
          }),
        },
      }),
      sendFrame,
    );
    return;
  }
  debug(`unhandled exec: ${execCase ?? "?"}`);
}

function handleServerMessage(msg, blobStore, sendFrame, emit) {
  const msgCase = msg.message?.case;
  if (msgCase === "execServerMessage") {
    handleExecMessage(msg.message.value, sendFrame);
  } else if (msgCase === "interactionUpdate") {
    const u = msg.message.value;
    const c = u.message?.case;
    debug(`interaction ${c ?? "?"}`);
    if (c === "textDelta") {
      const d = u.message.value.text || "";
      if (d) {
        sawText = true;
        emit({ type: "text", delta: d });
        bumpIdleFinish();
      }
    } else if (c === "thinkingDelta") {
      const d = u.message.value.text || "";
      if (d) emit({ type: "thinking", delta: d });
    } else if (c === "thinkingCompleted") {
      debug("interaction thinkingCompleted");
      if (sawText) bumpIdleFinish();
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

async function readConfig() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk);
  }
  const raw = Buffer.concat(chunks).toString("utf8").trim();
  if (!raw) throw new Error("no config on stdin");
  return JSON.parse(raw);
}

let sawText = false;
let finished = false;
let idleTimer = null;
let heartbeatTimer = null;
let bridgeProc = null;

function gracefulFinish(code = 0) {
  if (finished) return;
  finished = true;
  if (heartbeatTimer) clearInterval(heartbeatTimer);
  if (idleTimer) clearTimeout(idleTimer);
  try {
    bridgeProc?.stdin?.end();
  } catch {
    /* ignore */
  }
  setTimeout(() => {
    if (bridgeProc && !bridgeProc.killed) bridgeProc.kill();
    process.exit(code);
  }, 500);
}

function bumpIdleFinish() {
  if (finished) return;
  if (idleTimer) clearTimeout(idleTimer);
  idleTimer = setTimeout(() => gracefulFinish(0), 2500);
}

const config = await readConfig();
const { accessToken, modelId, systemPrompt, userText, workspaceRoot } = config;
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

const proc = spawn("node", [BRIDGE_PATH], { stdio: ["pipe", "pipe", "pipe"] });
bridgeProc = proc;

const bridgeConfig = JSON.stringify({
  accessToken,
  url: "https://api2.cursor.sh",
  path: "/agent.v1.AgentService/Run",
  unary: false,
  headers:
    config.apiHeaders && typeof config.apiHeaders === "object"
      ? config.apiHeaders
      : {
          "x-cursor-client-type": "cli",
          "x-ghost-mode": "true",
          "x-client-key": sha256Hex(accessToken),
          "x-cursor-checksum": cursorChecksum(accessToken),
          "x-cursor-client-version": "cli-2026.01.09-231024f",
        },
});

proc.stdin.write(lpEncode(Buffer.from(bridgeConfig, "utf8")));

const { requestBytes, blobStore } = buildRequest(
  modelId,
  systemPrompt || "You are Composer in Cursor IDE.",
  userText,
);

proc.stdin.write(lpEncode(frameConnect(requestBytes)));

const heartbeat = create(AgentClientMessageSchema, {
  message: { case: "clientHeartbeat", value: create(ClientHeartbeatSchema, {}) },
});
heartbeatTimer = setInterval(() => {
  if (!proc.killed) {
    proc.stdin.write(lpEncode(frameConnect(toBinary(AgentClientMessageSchema, heartbeat))));
  }
}, 15_000);

let stdoutBuf = Buffer.alloc(0);
proc.stdout.on("data", (chunk) => {
  stdoutBuf = Buffer.concat([stdoutBuf, chunk]);
  while (stdoutBuf.length >= 4) {
    const len = stdoutBuf.readUInt32BE(0);
    if (stdoutBuf.length < 4 + len) break;
    const payload = stdoutBuf.subarray(4, 4 + len);
    stdoutBuf = stdoutBuf.subarray(4 + len);

    let offset = 0;
    while (offset + 5 <= payload.length) {
      const flags = payload[offset];
      const flen = payload.readUInt32BE(offset + 1);
      if (offset + 5 + flen > payload.length) break;
      const frame = payload.subarray(offset + 5, offset + 5 + flen);
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
        handleServerMessage(msg, blobStore, (f) => {
          proc.stdin.write(lpEncode(f));
        }, emit);
      } catch (err) {
        debug(`parse err: ${err}`);
      }
    }
  }
});

proc.on("close", (code) => {
  if (finished) return;
  clearInterval(heartbeatTimer);
  if (code !== 0) {
    emit({ error: `bridge exited ${code}` });
  } else if (!sawText) {
    emit({ error: "stream ended without text deltas" });
    process.exit(1);
    return;
  }
  process.exit(code === 0 ? 0 : 1);
});

proc.stderr.on("data", (chunk) => {
  process.stderr.write(chunk);
});
