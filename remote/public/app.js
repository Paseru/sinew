import React, { useCallback, useEffect, useMemo, useRef, useState } from "https://esm.sh/react@18.3.1";
import { createRoot } from "https://esm.sh/react-dom@18.3.1/client";
import { marked } from "https://esm.sh/marked@15.0.6";
import DOMPurify from "https://esm.sh/dompurify@3.2.3";

const h = React.createElement;

const REMOTE_PROTOCOL_VERSION = 1;
const STORAGE_SESSION_KEY = "sinew.remote.session.v1";
const AES_AAD = new TextEncoder().encode("sinew-remote-v1");
const MODES = ["act", "goal", "plan"];
const MODE_LABELS = { act: "Act", goal: "Goal", plan: "Plan" };
const INSTALL_HINT_KEY = "sinew.remote.installHintDismissed.v1";

marked.setOptions({ gfm: true, breaks: true });

/* ── Utilities ─────────────────────────────────────────────────────────── */

function isIosDevice() {
  return /iphone|ipad|ipod/i.test(navigator.userAgent);
}

function isStandaloneDisplay() {
  return window.matchMedia?.("(display-mode: standalone)")?.matches || window.navigator.standalone === true;
}

function wsUrl() {
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${location.host}/ws`;
}

function bytesToBase64(bytes) {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function base64ToBytes(value) {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

function base64UrlToBytes(value) {
  const padded = `${value}${"=".repeat((4 - (value.length % 4)) % 4)}`;
  return base64ToBytes(padded.replace(/-/g, "+").replace(/_/g, "/"));
}

function bytesToBase64Url(bytes) {
  return bytesToBase64(bytes).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

function textBytes(value) {
  return new TextEncoder().encode(value);
}

function concatBytes(...parts) {
  const total = parts.reduce((sum, part) => sum + part.length, 0);
  const out = new Uint8Array(total);
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
}

async function sha256(bytes) {
  return new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
}

async function derivePairingKey(shared, pcPublicKey, phonePublicKey, code) {
  const digest = await sha256(
    concatBytes(
      textBytes("sinew-remote-pairing-v1"),
      new Uint8Array(shared),
      pcPublicKey,
      phonePublicKey,
      textBytes(code),
    ),
  );
  return crypto.subtle.importKey("raw", digest, "AES-GCM", false, ["encrypt", "decrypt"]);
}

async function importAesKey(secretB64) {
  return crypto.subtle.importKey("raw", base64ToBytes(secretB64), "AES-GCM", false, ["encrypt", "decrypt"]);
}

async function encryptJson(key, value) {
  const nonce = crypto.getRandomValues(new Uint8Array(12));
  const plaintext = textBytes(JSON.stringify(value));
  const ciphertext = new Uint8Array(
    await crypto.subtle.encrypt({ name: "AES-GCM", iv: nonce, additionalData: AES_AAD }, key, plaintext),
  );
  return { nonce: bytesToBase64(nonce), ciphertext: bytesToBase64(ciphertext) };
}

async function decryptJson(key, envelope) {
  const plaintext = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv: base64ToBytes(envelope.nonce), additionalData: AES_AAD },
    key,
    base64ToBytes(envelope.ciphertext),
  );
  return JSON.parse(new TextDecoder().decode(plaintext));
}

function requestId() {
  return `req_${bytesToBase64Url(crypto.getRandomValues(new Uint8Array(12)))}`;
}

function loadSession() {
  try {
    const raw = localStorage.getItem(STORAGE_SESSION_KEY);
    return raw ? JSON.parse(raw) : null;
  } catch {
    return null;
  }
}

function saveSession(session) {
  localStorage.setItem(STORAGE_SESSION_KEY, JSON.stringify(session));
}

function clearSession() {
  localStorage.removeItem(STORAGE_SESSION_KEY);
}

function plainTextFromParts(parts = []) {
  return parts
    .filter((part) => part.type === "text" && !part.meta?.attachment_context && !part.meta?.plan_control && !part.meta?.system_reminder)
    .map((part) => part.text || "")
    .join("\n")
    .trim();
}

function modeFromConversation(conversation) {
  if (conversation?.planWorkflow?.status && conversation.planWorkflow.status !== "idle") return "plan";
  if (conversation?.goalWorkflow?.status === "active") return "goal";
  return "act";
}

function thinkingFromModel(model) {
  if (!model) return "medium";
  if (model.effort === "none") return "off";
  if (["low", "medium", "high", "xhigh", "max"].includes(model.effort)) return model.effort;
  if (model.provider === "google" || model.provider === "kimi") return "high";
  return "medium";
}

function renderMarkdown(text) {
  return { __html: DOMPurify.sanitize(marked.parse(text || "")) };
}

function fileToBase64(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error || new Error("Unable to read file"));
    reader.onload = () => {
      const value = String(reader.result || "");
      resolve(value.includes(",") ? value.split(",")[1] : value);
    };
    reader.readAsDataURL(file);
  });
}

function relativeDate(ms) {
  if (!ms) return "";
  const diff = Date.now() - ms;
  const minutes = Math.floor(diff / 60_000);
  if (minutes < 1) return "now";
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d`;
  return new Date(ms).toLocaleDateString();
}

const TOOL_TONES = [
  [/^(read|glob|list)/i, "read"],
  [/^(grep|search|web)/i, "grep"],
  [/^(edit|write|create)/i, "edit"],
  [/^(bash|powershell|terminal|command)/i, "bash"],
];

function toolTone(name = "") {
  for (const [pattern, tone] of TOOL_TONES) {
    if (pattern.test(name)) return tone;
  }
  return "default";
}

function toolTitle(block) {
  if (block.summary) return block.summary;
  const name = block.name || "Tool";
  const input = block.input || {};
  const hint = input.path || input.pattern || input.command || input.description || "";
  if (typeof hint === "string" && hint) {
    const short = hint.length > 48 ? `${hint.slice(0, 48)}…` : hint;
    return `${name} · ${short}`;
  }
  return name;
}

function parseQuestionArgs(raw) {
  if (!raw) return [];
  try {
    const parsed = typeof raw === "string" ? JSON.parse(raw) : raw;
    const records = Array.isArray(parsed.questions) ? parsed.questions : [parsed];
    return records
      .map((record) => {
        if (!record || typeof record !== "object") return null;
        const question = String(record.question || record.prompt || record.title || "").trim();
        if (!question) return null;
        const rawOptions = Array.isArray(record.options) ? record.options : Array.isArray(record.choices) ? record.choices : [];
        const options = rawOptions
          .map((option, index) => {
            if (typeof option === "string" || typeof option === "number") {
              return { id: String(index), label: String(option) };
            }
            if (!option || typeof option !== "object") return null;
            const label = String(option.label || option.text || option.value || option.id || "").trim();
            if (!label) return null;
            return { id: String(option.id || option.value || index), label, description: option.description || "" };
          })
          .filter(Boolean);
        const modeValue = String(record.type || record.mode || record.kind || "").toLowerCase();
        return {
          question,
          options,
          mode: modeValue.includes("multiple") || record.multiple || record.allowMultiple ? "multiple_choice" : "single_choice",
        };
      })
      .filter(Boolean);
  } catch {
    return [];
  }
}

/* ── Blocks: history + live events, no duplication ─────────────────────── */

function blocksFromHistory(history = []) {
  const blocks = [];
  for (const message of history) {
    if (message.role === "user") {
      const text = plainTextFromParts(message.parts);
      if (text) blocks.push({ kind: "user", text });
      continue;
    }
    if (message.role === "assistant") {
      for (const part of message.parts || []) {
        if (part.type === "text" && part.text) blocks.push({ kind: "assistant", text: part.text });
        else if (part.type === "thinking" && part.text) blocks.push({ kind: "thinking", text: part.text, streaming: false });
        else if (part.type === "tool_call") {
          blocks.push({
            kind: "tool",
            id: part.id,
            name: part.name,
            status: "done",
            input: part.input,
            argsPretty: JSON.stringify(part.input || {}),
          });
        }
      }
    }
  }
  return blocks;
}

function blocksFromLiveEvents(liveEvents = []) {
  const blocks = [];
  let currentText = null;
  let currentThinking = null;
  const tools = new Map();
  const subagents = new Map();

  for (const event of liveEvents) {
    switch (event.type) {
      case "text_started":
        currentText = { kind: "assistant", text: "" };
        blocks.push(currentText);
        break;
      case "text_chunk":
        if (!currentText) {
          currentText = { kind: "assistant", text: "" };
          blocks.push(currentText);
        }
        currentText.text += event.delta || "";
        break;
      case "text_finished":
        currentText = null;
        break;
      case "thinking_started":
        currentThinking = { kind: "thinking", text: "", streaming: true };
        blocks.push(currentThinking);
        break;
      case "thinking_chunk":
        if (!currentThinking) {
          currentThinking = { kind: "thinking", text: "", streaming: true };
          blocks.push(currentThinking);
        }
        currentThinking.text += event.delta || "";
        break;
      case "thinking_finished":
        if (currentThinking) currentThinking.streaming = false;
        currentThinking = null;
        break;
      case "tool_started": {
        const block = { kind: "tool", id: event.id, name: event.name, status: "running", output: "", args: "" };
        tools.set(event.id, block);
        blocks.push(block);
        break;
      }
      case "tool_args_delta":
        if (tools.has(event.id)) tools.get(event.id).args += event.delta || "";
        break;
      case "tool_output_delta":
        if (tools.has(event.id)) tools.get(event.id).output += event.delta || "";
        break;
      case "tool_ready":
        if (tools.has(event.id)) {
          Object.assign(tools.get(event.id), { summary: event.summary, argsPretty: event.args_pretty });
        }
        break;
      case "tool_finished":
        if (tools.has(event.id)) {
          Object.assign(tools.get(event.id), {
            status: event.is_error ? "error" : "done",
            output: event.output,
            meta: event.meta,
          });
        }
        break;
      case "sub_agent_event": {
        let block = subagents.get(event.id);
        if (!block) {
          block = { kind: "subagent", id: event.id, name: event.agent_name || "Agent", status: "running" };
          subagents.set(event.id, block);
          blocks.push(block);
        }
        if (event.event?.type === "turn_finished") block.status = "done";
        if (event.event?.type === "error") block.status = "error";
        break;
      }
      case "interrupted":
        blocks.push({ kind: "status", text: "Interrupted" });
        break;
      case "error":
        blocks.push({ kind: "error", text: event.message || "Error" });
        break;
      default:
        break;
    }
  }
  return blocks;
}

/* ── Relay client ──────────────────────────────────────────────────────── */

class RemoteClient {
  constructor({ onStatus, onPayload, onPairingResponse }) {
    this.onStatus = onStatus;
    this.onPayload = onPayload;
    this.onPairingResponse = onPairingResponse;
    this.ws = null;
    this.session = loadSession();
    this.key = null;
    this.pending = new Map();
    this.closed = false;
  }

  async start() {
    if (this.session) this.key = await importAesKey(this.session.deviceSecret);
    this.connect();
  }

  announce() {
    if (!this.session) return false;
    return this.send({ kind: "phone_hello", pcId: this.session.pcId, deviceId: this.session.deviceId, protocolVersion: REMOTE_PROTOCOL_VERSION });
  }

  connect() {
    if (this.closed) return;
    this.onStatus({ relay: "connecting" });
    const ws = new WebSocket(wsUrl());
    this.ws = ws;
    ws.onopen = () => {
      this.onStatus({ relay: "connected" });
      if (this.session) this.announce();
    };
    ws.onmessage = (event) => {
      void this.handleFrame(JSON.parse(event.data)).catch(() => undefined);
    };
    ws.onclose = () => {
      this.onStatus({ relay: "offline", pcReachable: false });
      if (!this.closed) setTimeout(() => this.connect(), 1500);
    };
    ws.onerror = () => this.onStatus({ relay: "error" });
  }

  send(frame) {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return false;
    this.ws.send(JSON.stringify(frame));
    return true;
  }

  async pair(code, deviceName) {
    const keyPair = await crypto.subtle.generateKey({ name: "ECDH", namedCurve: "P-256" }, false, ["deriveBits"]);
    const publicRaw = new Uint8Array(await crypto.subtle.exportKey("raw", keyPair.publicKey));
    this.pairing = { keyPair, publicRaw, code };
    if (!this.send({ kind: "phone_pair_request", code, deviceName, phonePublicKey: bytesToBase64(publicRaw) })) {
      throw new Error("Relay offline");
    }
  }

  async handleFrame(frame) {
    if (frame.kind === "pc_status") {
      this.onStatus({ pcReachable: Boolean(frame.reachable) });
      return;
    }
    if (frame.kind === "device_revoked") {
      clearSession();
      this.session = null;
      this.key = null;
      this.onStatus({ revoked: true, pcReachable: false });
      this.onPayload({ type: "device_revoked" });
      return;
    }
    if (frame.kind === "pairing_response") {
      if (!frame.accepted) {
        this.onPairingResponse({ ok: false, error: frame.error || "Pairing failed" });
        return;
      }
      const pcPublicKey = base64ToBytes(frame.pcPublicKey);
      const publicKey = await crypto.subtle.importKey("raw", pcPublicKey, { name: "ECDH", namedCurve: "P-256" }, false, []);
      const shared = await crypto.subtle.deriveBits({ name: "ECDH", public: publicKey }, this.pairing.keyPair.privateKey, 256);
      const pairingKey = await derivePairingKey(shared, pcPublicKey, this.pairing.publicRaw, this.pairing.code);
      const grant = await decryptJson(pairingKey, frame.encrypted);
      this.session = {
        pcId: grant.pcId,
        relayUrl: grant.relayUrl,
        deviceId: grant.deviceId,
        deviceName: grant.deviceName,
        deviceToken: grant.deviceToken,
        deviceSecret: grant.deviceSecret,
      };
      saveSession(this.session);
      this.key = await importAesKey(this.session.deviceSecret);
      this.onPairingResponse({ ok: true });
      this.announce();
      return;
    }
    if (frame.kind === "pc_cipher") {
      if (!this.key) return;
      const payload = await decryptJson(this.key, frame.envelope);
      if (payload.type === "response") {
        const pending = this.pending.get(payload.request_id);
        if (pending) {
          this.pending.delete(payload.request_id);
          payload.ok ? pending.resolve(payload.data) : pending.reject(new Error(payload.error || "Remote command failed"));
        }
        return;
      }
      this.onPayload(payload);
    }
  }

  async command(command, workspace) {
    if (!this.session || !this.key) throw new Error("Not paired");
    const id = requestId();
    const envelope = await encryptJson(this.key, { requestId: id, token: this.session.deviceToken, workspace: workspace || undefined, command });
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error("The PC did not respond in time."));
      }, 60_000);
      this.pending.set(id, {
        resolve: (value) => {
          clearTimeout(timeout);
          resolve(value);
        },
        reject: (err) => {
          clearTimeout(timeout);
          reject(err);
        },
      });
      const sent = this.send({ kind: "phone_cipher", deviceId: this.session.deviceId, envelope });
      if (!sent) {
        clearTimeout(timeout);
        this.pending.delete(id);
        reject(new Error("Relay offline"));
      }
    });
  }
}

/* ── Small components ──────────────────────────────────────────────────── */

function StatusDot({ on }) {
  return h("span", { className: "status-dot", "data-on": on ? "true" : "false" });
}

function ThinkingBlock({ block }) {
  const [open, setOpen] = useState(false);
  const streaming = Boolean(block.streaming);
  const hasContent = Boolean(block.text);
  return h("div", { className: "thinking" },
    h("button", {
      type: "button",
      className: "thinking__head",
      "data-streaming": streaming ? "true" : "false",
      onClick: () => hasContent && setOpen((value) => !value),
    },
      h("span", { className: "thinking__caret", "data-open": open ? "true" : "false" }, "›"),
      h("span", { className: "thinking__label", "data-streaming": streaming ? "true" : "false" }, streaming ? "Thinking…" : "Thinking"),
    ),
    (open || streaming) && hasContent && h("div", {
      className: "thinking__content md",
      dangerouslySetInnerHTML: renderMarkdown(block.text),
    }),
  );
}

function ToolBlock({ block, children }) {
  const [open, setOpen] = useState(false);
  const hasBody = Boolean(block.output);
  return h("div", { className: "tool-row", "data-status": block.status },
    h("button", {
      type: "button",
      className: "tool-row__head",
      onClick: () => hasBody && setOpen((value) => !value),
    },
      h("span", { className: "tool-row__glyph", "data-tone": toolTone(block.name), "data-status": block.status }),
      h("span", { className: "tool-row__title" }, toolTitle(block)),
      block.status === "running" && h("span", { className: "tool-row__live" }, "running"),
      block.status === "error" && h("span", { className: "tool-row__err" }, "error"),
      hasBody && h("span", { className: "tool-row__caret", "data-open": open ? "true" : "false" }, "›"),
    ),
    open && hasBody && h("pre", { className: "tool-row__body" }, block.output),
    children || null,
  );
}

function QuestionPanel({ block, questions, disabled, pending, allowStopQuestions, onSubmit, onReject }) {
  const [selected, setSelected] = useState(() => questions.map(() => new Set()));
  const [custom, setCustom] = useState(() => questions.map(() => ""));

  useEffect(() => {
    setSelected(questions.map(() => new Set()));
    setCustom(questions.map(() => ""));
  }, [block.id, questions.length]);

  const answers = questions.map((question, index) => {
    const labels = [...(selected[index] || new Set())]
      .map((id) => question.options.find((option) => option.id === id)?.label || "")
      .filter(Boolean);
    const fallback = (custom[index] || "").trim();
    if (fallback) labels.push(fallback);
    return labels;
  });
  const complete = answers.every((answer) => answer.length > 0);

  function toggle(questionIndex, optionId) {
    if (disabled || pending) return;
    setSelected((current) => {
      const next = current.map((set) => new Set(set));
      const question = questions[questionIndex];
      if (question.mode === "multiple_choice") {
        if (next[questionIndex].has(optionId)) next[questionIndex].delete(optionId);
        else next[questionIndex].add(optionId);
      } else {
        next[questionIndex] = new Set([optionId]);
      }
      return next;
    });
  }

  return h("div", { className: "question" },
    h("div", { className: "question__kicker" }, "Sinew needs your answer"),
    questions.map((question, questionIndex) => h("div", { key: `${block.id}-${questionIndex}`, className: "question__group" },
      h("div", { className: "question__title" }, question.question),
      question.options.length > 0
        ? h("div", { className: "question__options" },
          question.options.map((option) => {
            const isSelected = selected[questionIndex]?.has(option.id);
            return h("button", {
              key: option.id,
              type: "button",
              className: "question__option",
              "data-selected": isSelected ? "true" : "false",
              disabled: disabled || pending,
              onClick: () => toggle(questionIndex, option.id),
            },
              h("span", { className: "question__option-label" }, option.label),
              option.description ? h("span", { className: "question__option-desc" }, option.description) : null,
            );
          }),
        )
        : h("textarea", {
          className: "question__input",
          value: custom[questionIndex] || "",
          disabled: disabled || pending,
          rows: 3,
          placeholder: "Type your answer…",
          onChange: (event) => setCustom((current) => current.map((value, index) => (index === questionIndex ? event.target.value : value))),
        }),
      question.mode === "multiple_choice" && h("div", { className: "question__hint" }, "Choose one or more options."),
    )),
    h("div", { className: "question__actions" },
      h("button", { type: "button", className: "btn", disabled: disabled || pending, onClick: () => onReject(block) }, "Dismiss"),
      allowStopQuestions && h("button", {
        type: "button",
        className: "btn",
        disabled: disabled || pending || !complete,
        onClick: () => onSubmit(block, answers, true),
      }, "Answer & stop"),
      h("button", {
        type: "button",
        className: "btn btn--primary",
        disabled: disabled || pending || !complete,
        onClick: () => onSubmit(block, answers, false),
      }, pending ? "Sending…" : "Send answer"),
    ),
  );
}

/* ── App ───────────────────────────────────────────────────────────────── */

function App() {
  const params = new URLSearchParams(location.search);
  const initialCode = params.get("code") || "";
  const deepLinkConversation = params.get("conversation") || null;

  const [session, setSession] = useState(loadSession());
  const [status, setStatus] = useState({ relay: "connecting", pcReachable: false, revoked: false });
  const [pairCode, setPairCode] = useState(initialCode);
  const [pairError, setPairError] = useState(null);
  const [pairing, setPairing] = useState(false);

  const [workspace, setWorkspace] = useState(null);
  const [workspaces, setWorkspaces] = useState([]);
  const [workspacePath, setWorkspacePath] = useState(null);
  const [wsMenuOpen, setWsMenuOpen] = useState(false);
  const [conversations, setConversations] = useState([]);
  const [conv, setConv] = useState(null);
  const [view, setView] = useState("list");
  const [liveEvents, setLiveEvents] = useState(new Map());
  const [activeTurns, setActiveTurns] = useState([]);
  const [synced, setSynced] = useState(false);

  const [prompt, setPrompt] = useState("");
  const [mode, setMode] = useState("act");
  const [attachments, setAttachments] = useState([]);
  const [sendPending, setSendPending] = useState(false);
  const [questionPendingId, setQuestionPendingId] = useState(null);
  const [error, setError] = useState(null);
  const [pushState, setPushState] = useState("idle");
  const [deleteArmed, setDeleteArmed] = useState(null);
  const [installHintDismissed, setInstallHintDismissed] = useState(() => localStorage.getItem(INSTALL_HINT_KEY) === "true");

  const clientRef = useRef(null);
  const convRef = useRef(conv);
  const statusRef = useRef(status);
  const activeTurnsRef = useRef(activeTurns);
  const workspacePathRef = useRef(null);
  const syncedRef = useRef(false);
  const bodyRef = useRef(null);
  const inputRef = useRef(null);
  const stickRef = useRef(true);

  useEffect(() => { convRef.current = conv; }, [conv]);
  useEffect(() => { statusRef.current = status; }, [status]);
  useEffect(() => { activeTurnsRef.current = activeTurns; }, [activeTurns]);
  useEffect(() => { workspacePathRef.current = workspacePath; }, [workspacePath]);

  const cmd = useCallback((command, workspaceOverride) => {
    const client = clientRef.current;
    if (!client) return Promise.reject(new Error("Not connected"));
    return client.command(command, workspaceOverride || workspacePathRef.current || undefined);
  }, []);

  const canReachPc = Boolean(status.pcReachable);

  /* — sync — */

  const syncList = useCallback(async () => {
    const client = clientRef.current;
    if (!client?.session) return;
    const list = await cmd({ type: "list_conversations" });
    setConversations(Array.isArray(list) ? list : []);
  }, [cmd]);

  const handleTurnFinished = useCallback(async (conversationId, workspaceId) => {
    const client = clientRef.current;
    if (!client?.session) return;
    try {
      const fresh = await cmd({ type: "load_conversation", conversation_id: conversationId }, workspaceId);
      setConv((current) => (current && current.id === conversationId ? fresh : current));
      setLiveEvents((current) => {
        const next = new Map(current);
        next.delete(conversationId);
        return next;
      });
    } catch {
      // keep live events visible if reload fails
    }
    if (!workspaceId || workspaceId === workspacePathRef.current) {
      syncList().catch(() => undefined);
    }
  }, [cmd, syncList]);

  const initialSync = useCallback(async () => {
    const client = clientRef.current;
    if (!client?.session || !statusRef.current.pcReachable || syncedRef.current) return;
    try {
      const data = await client.command({ type: "bootstrap" });
      const bootstrap = data.bootstrap || data;
      const resolvedPath = data.workspacePath || bootstrap.workspace?.path || null;
      workspacePathRef.current = resolvedPath;
      setWorkspacePath(resolvedPath);
      setWorkspaces(Array.isArray(data.workspaces) ? data.workspaces : []);
      setWorkspace(bootstrap.workspace || null);
      setConversations(bootstrap.conversations || []);
      if (Array.isArray(data.activeTurns)) setActiveTurns(data.activeTurns);
      syncedRef.current = true;
      setSynced(true);
      if (deepLinkConversation && (bootstrap.conversations || []).some((c) => c.id === deepLinkConversation)) {
        await openConversationById(deepLinkConversation);
      }
    } catch (err) {
      setError(String(err.message || err));
    }
  }, [deepLinkConversation]); // eslint-disable-line react-hooks/exhaustive-deps

  /* — client lifecycle — */

  useEffect(() => {
    const client = new RemoteClient({
      onStatus: (patch) => setStatus((current) => ({ ...current, ...patch })),
      onPairingResponse: (result) => {
        setPairing(false);
        if (!result.ok) {
          setPairError(result.error);
          return;
        }
        setPairError(null);
        setSession(loadSession());
      },
      onPayload: (payload) => {
        if (payload.type === "agent_event") {
          setLiveEvents((current) => {
            const next = new Map(current);
            const events = next.get(payload.conversation_id) || [];
            next.set(payload.conversation_id, [...events, payload.event]);
            return next;
          });
          if (payload.event?.type === "turn_finished") void handleTurnFinished(payload.conversation_id, payload.workspace_id);
        }
        if (payload.type === "active_turns_changed") setActiveTurns(payload.active_turns || []);
        if (payload.type === "device_revoked") {
          setSession(null);
          setWorkspace(null);
          setWorkspaces([]);
          setWorkspacePath(null);
          setConversations([]);
          setConv(null);
          setLiveEvents(new Map());
          setActiveTurns([]);
          syncedRef.current = false;
          setSynced(false);
        }
      },
    });
    clientRef.current = client;
    void client.start();
    return () => {
      client.closed = true;
      client.ws?.close();
    };
  }, [handleTurnFinished]);

  useEffect(() => {
    if (session && status.pcReachable && !synced) void initialSync();
  }, [session, status.pcReachable, synced, initialSync]);

  useEffect(() => {
    if (!("serviceWorker" in navigator) || !("PushManager" in window)) return;
    navigator.serviceWorker.ready
      .then((registration) => registration.pushManager.getSubscription())
      .then((subscription) => {
        if (subscription) setPushState("enabled");
      })
      .catch(() => undefined);
  }, []);

  /* — derived — */

  const events = conv ? liveEvents.get(conv.id) || [] : [];
  const blocks = useMemo(() => {
    if (!conv) return [];
    return [...blocksFromHistory(conv.history || []), ...blocksFromLiveEvents(events)];
  }, [conv, events]);
  const isStreaming = Boolean(conv && activeTurns.some((turn) => turn.conversationId === conv.id));
  const showInstallHint = Boolean(session && !installHintDismissed && !isStandaloneDisplay());

  useEffect(() => {
    const body = bodyRef.current;
    if (body && stickRef.current) body.scrollTop = body.scrollHeight;
  }, [blocks, view]);

  function onBodyScroll() {
    const body = bodyRef.current;
    if (!body) return;
    stickRef.current = body.scrollHeight - body.scrollTop - body.clientHeight < 80;
  }

  /* — actions — */

  async function doPair(event) {
    event.preventDefault();
    setPairing(true);
    setPairError(null);
    try {
      await clientRef.current.pair(pairCode.replace(/\D/g, ""), isIosDevice() ? "iPhone" : "Phone");
    } catch (err) {
      setPairing(false);
      setPairError(String(err.message || err));
    }
  }

  async function switchWorkspace(path) {
    setWsMenuOpen(false);
    if (!path || path === workspacePathRef.current || !statusRef.current.pcReachable) return;
    try {
      const data = await clientRef.current.command({ type: "bootstrap" }, path);
      const bootstrap = data.bootstrap || data;
      const resolvedPath = data.workspacePath || path;
      workspacePathRef.current = resolvedPath;
      setWorkspacePath(resolvedPath);
      setWorkspaces(Array.isArray(data.workspaces) ? data.workspaces : []);
      setWorkspace(bootstrap.workspace || null);
      setConversations(bootstrap.conversations || []);
      if (Array.isArray(data.activeTurns)) setActiveTurns(data.activeTurns);
      setConv(null);
      setView("list");
    } catch (err) {
      setError(String(err.message || err));
    }
  }

  async function openConversationById(id) {
    if (!statusRef.current.pcReachable) return;
    try {
      const conversation = await cmd({ type: "load_conversation", conversation_id: id });
      setConv(conversation);
      setMode(modeFromConversation(conversation));
      setView("chat");
      stickRef.current = true;
      const turnActive = activeTurnsRef.current.some((item) => item.conversationId === id);
      if (turnActive) {
        try {
          const replay = await cmd({ type: "replay_active_turn_events", conversation_id: id, after_sequence: 0 });
          setLiveEvents((current) => {
            const next = new Map(current);
            next.set(id, (replay.events || []).map((entry) => entry.event));
            return next;
          });
        } catch {
          // replay is best-effort
        }
      } else {
        setLiveEvents((current) => {
          if (!current.has(id)) return current;
          const next = new Map(current);
          next.delete(id);
          return next;
        });
      }
    } catch (err) {
      setError(`Open conversation failed: ${String(err.message || err)}`);
    }
  }

  async function createConversation() {
    if (!canReachPc) return;
    try {
      const bootstrap = await cmd({ type: "create_conversation" });
      setConversations(bootstrap.conversations || []);
      const active = bootstrap.activeConversation;
      if (active) {
        setConv(active);
        setMode(modeFromConversation(active));
        setView("chat");
      }
    } catch (err) {
      setError(String(err.message || err));
    }
  }

  async function deleteConversation(id) {
    if (!canReachPc) return;
    if (deleteArmed !== id) {
      setDeleteArmed(id);
      setTimeout(() => setDeleteArmed((current) => (current === id ? null : current)), 2600);
      return;
    }
    setDeleteArmed(null);
    try {
      await cmd({ type: "delete_conversation", conversation_id: id });
      await syncList();
      setConv((current) => (current && current.id === id ? null : current));
      if (convRef.current?.id === id) setView("list");
    } catch (err) {
      setError(String(err.message || err));
    }
  }

  async function sendPrompt(event) {
    event.preventDefault();
    if (!prompt.trim() || !conv || sendPending || !canReachPc) return;
    const text = prompt;
    const files = attachments;
    setSendPending(true);
    try {
      const encoded = [];
      for (const file of files) {
        encoded.push({ name: file.name, mediaType: file.type || "application/octet-stream", data: await fileToBase64(file) });
      }
      const model = conv.modeModelSettings?.[mode] || conv.model;
      await cmd({
        type: "send_message",
        conversation_id: conv.id,
        text,
        attachments: encoded,
        mode,
        model,
        thinking: thinkingFromModel(model),
      });
      setPrompt("");
      setAttachments([]);
      if (inputRef.current) inputRef.current.style.height = "auto";
      stickRef.current = true;
      setConv((current) => (current && current.id === conv.id
        ? { ...current, history: [...(current.history || []), { role: "user", parts: [{ type: "text", text }] }] }
        : current));
    } catch (err) {
      setError(String(err.message || err));
    } finally {
      setSendPending(false);
    }
  }

  async function setConversationMode(nextMode) {
    setMode(nextMode);
    if (!conv || !canReachPc || isStreaming) return;
    try {
      const updated = await cmd({ type: "set_conversation_mode", conversation_id: conv.id, mode: nextMode });
      setConv((current) => (current && current.id === updated.id ? updated : current));
    } catch (err) {
      setError(String(err.message || err));
    }
  }

  async function compact() {
    if (!conv || !canReachPc || isStreaming) return;
    try {
      await cmd({
        type: "compact_conversation",
        conversation_id: conv.id,
        model: conv.model,
        thinking: thinkingFromModel(conv.model),
      });
      await handleTurnFinished(conv.id);
    } catch (err) {
      setError(String(err.message || err));
    }
  }

  async function answerQuestion(block, answers, stopQuestions = false) {
    if (!conv || !canReachPc) return;
    setQuestionPendingId(block.id);
    try {
      await cmd({
        type: "answer_question",
        conversation_id: conv.id,
        tool_call_id: block.id,
        answers,
        stop_questions: stopQuestions,
      });
    } catch (err) {
      setError(String(err.message || err));
    } finally {
      setQuestionPendingId(null);
    }
  }

  async function rejectQuestion(block) {
    if (!conv || !canReachPc) return;
    setQuestionPendingId(block.id);
    try {
      await cmd({ type: "reject_question", conversation_id: conv.id, tool_call_id: block.id });
    } catch (err) {
      setError(String(err.message || err));
    } finally {
      setQuestionPendingId(null);
    }
  }

  async function togglePush() {
    if (!clientRef.current?.session || !canReachPc || !("serviceWorker" in navigator) || !("PushManager" in window)) return;
    if (pushState === "enabled") return;
    try {
      setPushState("requesting");
      const registration = await navigator.serviceWorker.ready;
      const vapid = await fetch("/vapid-public-key", { cache: "no-store" }).then((r) => r.json());
      if (!vapid.publicKey) throw new Error("Push is not configured on the relay.");
      const subscription = await registration.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: base64UrlToBytes(vapid.publicKey) });
      await cmd({ type: "subscribe_push", subscription: subscription.toJSON() });
      setPushState("enabled");
    } catch (err) {
      setPushState("idle");
      setError(String(err.message || err));
    }
  }

  function retryConnection() {
    setError(null);
    const client = clientRef.current;
    if (!client) return;
    const announced = client.announce();
    if (!announced && client.ws?.readyState !== WebSocket.CONNECTING) client.connect();
  }

  function logout() {
    clearSession();
    setSession(null);
    setWorkspace(null);
    setWorkspaces([]);
    setWorkspacePath(null);
    setConversations([]);
    setConv(null);
    setLiveEvents(new Map());
    setActiveTurns([]);
    syncedRef.current = false;
    setSynced(false);
  }

  function dismissInstallHint() {
    localStorage.setItem(INSTALL_HINT_KEY, "true");
    setInstallHintDismissed(true);
  }

  /* — render: pairing — */

  if (!session) {
    return h("main", { className: "pairing" },
      h("div", { className: "pairing__inner" },
        h("div", { className: "pairing__brand" },
          h("img", { src: "/icons/icon.svg", alt: "" }),
          h("span", null, "Sinew Remote"),
        ),
        h("h1", { className: "pairing__title" }, "Pair this phone"),
        h("p", { className: "pairing__sub" }, "Open Remote on your PC, then scan the QR code or enter the 6-digit code."),
        h("form", { className: "pairing__form", onSubmit: doPair },
          h("input", {
            className: "pairing__code",
            inputMode: "numeric",
            pattern: "[0-9]*",
            maxLength: 6,
            value: pairCode,
            onChange: (e) => setPairCode(e.target.value.replace(/\D/g, "").slice(0, 6)),
            placeholder: "000000",
            autoFocus: !initialCode,
          }),
          h("button", { className: "btn btn--primary btn--wide", disabled: pairCode.length !== 6 || pairing }, pairing ? "Pairing…" : "Pair"),
        ),
        pairError && h("p", { className: "pairing__error" }, pairError),
        h("p", { className: "pairing__note" }, "End-to-end encrypted between this phone and your PC. The relay never sees your data."),
      ),
    );
  }

  /* — render: shell — */

  const listView = h("section", { className: "pane pane--list", "data-active": view === "list" ? "true" : "false" },
    h("header", { className: "pane__head" },
      h("div", { className: "pane__head-main" },
        h("button", {
          className: "ws-switch",
          onClick: () => setWsMenuOpen((value) => !value),
          disabled: workspaces.length === 0,
          "aria-haspopup": "menu",
          "aria-expanded": wsMenuOpen ? "true" : "false",
        },
          h(StatusDot, { on: canReachPc }),
          h("span", { className: "ws-switch__name" }, workspace?.name || "Sinew"),
          workspaces.length > 0 && h("span", { className: "ws-switch__caret", "data-open": wsMenuOpen ? "true" : "false" }, "›"),
        ),
        h("div", { className: "pane__sub" }, canReachPc ? "PC connected" : status.relay === "connected" ? "PC unreachable" : `Relay ${status.relay}`),
      ),
      h("div", { className: "pane__head-actions" },
        h("button", {
          className: "icon-btn",
          "data-on": pushState === "enabled" ? "true" : "false",
          disabled: !canReachPc || pushState === "requesting",
          title: "Notifications",
          onClick: togglePush,
        }, pushState === "enabled" ? "Push on" : pushState === "requesting" ? "…" : "Push"),
        h("button", { className: "icon-btn", onClick: logout, title: "Forget this PC" }, "Forget"),
      ),
    ),
    wsMenuOpen && h(React.Fragment, null,
      h("div", { className: "ws-overlay", onClick: () => setWsMenuOpen(false) }),
      h("div", { className: "ws-menu", role: "menu" },
        h("div", { className: "ws-menu__kicker" }, "Open workspaces"),
        workspaces.map((item) => h("button", {
          key: item.path,
          className: "ws-menu__item",
          "data-active": item.path === workspacePath ? "true" : "false",
          disabled: !canReachPc,
          onClick: () => void switchWorkspace(item.path),
        },
          h("span", { className: "ws-menu__name" }, item.name || item.path),
          h("span", { className: "ws-menu__path" }, item.path),
        )),
      ),
    ),
    showInstallHint && h("div", { className: "hint" },
      h("div", { className: "hint__text" },
        h("strong", null, "Install Sinew Remote. "),
        isIosDevice()
          ? "Add to Home Screen (Share → Add to Home Screen) to enable notifications on iPhone."
          : "Install the PWA for full-screen chat and notifications.",
      ),
      h("button", { className: "icon-btn", onClick: dismissInstallHint }, "Got it"),
    ),
    !canReachPc && h("div", { className: "offline" },
      h("span", null, "Keep Sinew open on your PC with Remote enabled."),
      h("button", { className: "icon-btn", onClick: retryConnection }, "Retry"),
    ),
    h("div", { className: "convs" },
      h("button", { className: "conv-new", disabled: !canReachPc, onClick: createConversation }, "+ New conversation"),
      conversations.length === 0 && h("div", { className: "convs__empty" }, canReachPc ? "No conversations yet." : "Waiting for the PC…"),
      conversations.map((item) => {
        const streaming = activeTurns.some((turn) => turn.conversationId === item.id);
        return h("div", {
          key: item.id,
          className: "conv-row",
          "data-active": conv?.id === item.id ? "true" : "false",
          role: "button",
          tabIndex: 0,
          onClick: () => void openConversationById(item.id),
          onKeyDown: (e) => { if (e.key === "Enter") void openConversationById(item.id); },
        },
          streaming && h("span", { className: "conv-row__pulse" }),
          h("span", { className: "conv-row__title" }, item.title || "New conversation"),
          h("span", { className: "conv-row__meta" }, streaming ? "streaming" : relativeDate(item.updatedAtMs)),
          h("button", {
            className: "conv-row__delete",
            "data-armed": deleteArmed === item.id ? "true" : "false",
            disabled: !canReachPc,
            onClick: (e) => { e.stopPropagation(); void deleteConversation(item.id); },
            "aria-label": "Delete conversation",
          }, deleteArmed === item.id ? "Sure?" : "×"),
        );
      }),
    ),
  );

  const chatView = h("section", { className: "pane pane--chat", "data-active": view === "chat" ? "true" : "false" },
    conv ? h(React.Fragment, null,
      h("header", { className: "pane__head pane__head--chat" },
        h("button", { className: "chat-back", onClick: () => setView("list"), "aria-label": "Back" }, "‹"),
        h("div", { className: "pane__head-main" },
          h("div", { className: "pane__title pane__title--chat" }, conv.title || "Conversation"),
          h("div", { className: "pane__sub" },
            isStreaming
              ? h("span", { className: "shimmer" }, "Streaming…")
              : sendPending
                ? "Waiting for PC…"
                : canReachPc ? "Encrypted" : "PC unreachable",
          ),
        ),
        h("button", { className: "icon-btn", disabled: !canReachPc || isStreaming, onClick: compact }, "Compact"),
      ),
      h("div", { className: "chat-body", ref: bodyRef, onScroll: onBodyScroll },
        blocks.length === 0 && !isStreaming && h("div", { className: "chat-empty" }, "Send a message to get started."),
        blocks.map((block, index) => {
          if (block.kind === "user") {
            return h("div", { key: index, className: "msg" }, h("div", { className: "user-text" }, block.text));
          }
          if (block.kind === "assistant") {
            return h("div", { key: index, className: "msg" },
              h("div", { className: "md", dangerouslySetInnerHTML: renderMarkdown(block.text) }),
            );
          }
          if (block.kind === "thinking") {
            return h(ThinkingBlock, { key: index, block });
          }
          if (block.kind === "tool") {
            const questions = block.name === "question" ? parseQuestionArgs(block.argsPretty || block.args) : [];
            const canAnswer = questions.length > 0 && block.status === "running";
            return h(ToolBlock, { key: block.id || index, block },
              canAnswer ? h(QuestionPanel, {
                block,
                questions,
                disabled: !canReachPc,
                pending: questionPendingId === block.id,
                allowStopQuestions: mode === "plan",
                onSubmit: answerQuestion,
                onReject: rejectQuestion,
              }) : null,
            );
          }
          if (block.kind === "subagent") {
            return h(ToolBlock, { key: block.id || index, block: { ...block, name: "subagent" } });
          }
          if (block.kind === "error") {
            return h("div", { key: index, className: "err-block" }, block.text);
          }
          return h("div", { key: index, className: "status-line" }, block.text);
        }),
        isStreaming && events.length === 0 && h("div", { className: "status-line shimmer" }, "Working…"),
      ),
      h("form", { className: "composer", onSubmit: sendPrompt },
        attachments.length > 0 && h("div", { className: "composer__chips" },
          attachments.map((file, index) => h("button", {
            key: `${file.name}-${index}`,
            type: "button",
            className: "chip",
            onClick: () => setAttachments((current) => current.filter((_, i) => i !== index)),
          }, `${file.name} ×`)),
        ),
        h("div", { className: "composer__box" },
          h("textarea", {
            className: "composer__input",
            ref: inputRef,
            rows: 1,
            value: prompt,
            disabled: !canReachPc || sendPending,
            placeholder: canReachPc ? "Message Sinew…" : "PC unreachable",
            onChange: (e) => {
              setPrompt(e.target.value);
              e.target.style.height = "auto";
              e.target.style.height = `${Math.min(e.target.scrollHeight, 160)}px`;
            },
          }),
          h("div", { className: "composer__row" },
            h("label", { className: "composer__attach", "data-disabled": !canReachPc || sendPending ? "true" : "false" },
              "+",
              h("input", {
                type: "file",
                multiple: true,
                disabled: !canReachPc || sendPending,
                onChange: (e) => { setAttachments((current) => [...current, ...Array.from(e.target.files || [])]); e.target.value = ""; },
              }),
            ),
            h("div", { className: "composer__modes" },
              MODES.map((value) => h("button", {
                key: value,
                type: "button",
                className: "composer__mode",
                "data-selected": mode === value ? "true" : "false",
                disabled: !canReachPc,
                onClick: () => void setConversationMode(value),
              }, MODE_LABELS[value])),
            ),
            h("button", {
              className: "composer__send",
              disabled: !prompt.trim() || !canReachPc || sendPending,
            }, sendPending ? "Sending…" : "Send"),
          ),
        ),
      ),
    ) : h("div", { className: "chat-none" },
      h("span", null, "Select a conversation"),
    ),
  );

  return h("main", { className: "app" },
    error && h("button", { className: "toast", onClick: () => setError(null) }, error),
    listView,
    chatView,
  );
}

if ("serviceWorker" in navigator) {
  navigator.serviceWorker.register("/sw.js").catch(() => undefined);
}

createRoot(document.getElementById("app")).render(h(App));
