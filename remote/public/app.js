import React, { useCallback, useEffect, useMemo, useRef, useState } from "https://esm.sh/react@18.3.1";
import { createRoot } from "https://esm.sh/react-dom@18.3.1/client";
import { marked } from "https://esm.sh/marked@15.0.6";
import DOMPurify from "https://esm.sh/dompurify@3.2.3";

const REMOTE_PROTOCOL_VERSION = 1;
const STORAGE_SESSION_KEY = "sinew.remote.session.v1";
const AES_AAD = new TextEncoder().encode("sinew-remote-v1");
const MODES = ["act", "goal", "plan"];
const modeLabels = { act: "Act", goal: "Goal", plan: "Plan" };
const INSTALL_HINT_KEY = "sinew.remote.installHintDismissed.v1";

function isIosDevice() {
  return /iphone|ipad|ipod/i.test(navigator.userAgent);
}

function isStandaloneDisplay() {
  return window.matchMedia?.("(display-mode: standalone)")?.matches || window.navigator.standalone === true;
}

function nowMs() {
  return Date.now();
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

function htmlFromMarkdown(text) {
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

function parseQuestionArgs(argsPretty) {
  if (!argsPretty) return [];
  try {
    const parsed = JSON.parse(argsPretty);
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

function buildBlocks(history = [], liveEvents = []) {
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
        if (part.type === "thinking" && part.text) blocks.push({ kind: "thinking", text: part.text });
        if (part.type === "tool_call") blocks.push({ kind: "tool", id: part.id, name: part.name, status: "done", input: part.input, argsPretty: JSON.stringify(part.input || {}) });
      }
    }
  }

  let currentText = null;
  let currentThinking = null;
  const tools = new Map();
  for (const event of liveEvents) {
    switch (event.type) {
      case "turn_started":
        blocks.push({ kind: "status", text: "Agent started" });
        break;
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
        currentThinking = { kind: "thinking", text: "" };
        blocks.push(currentThinking);
        break;
      case "thinking_chunk":
        if (!currentThinking) {
          currentThinking = { kind: "thinking", text: "" };
          blocks.push(currentThinking);
        }
        currentThinking.text += event.delta || "";
        break;
      case "thinking_finished":
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
          Object.assign(tools.get(event.id), { status: event.is_error ? "error" : "done", output: event.output, meta: event.meta });
        }
        break;
      case "error":
        blocks.push({ kind: "error", text: event.message || "Error" });
        break;
      case "turn_finished":
        blocks.push({ kind: "status", text: "Response finished" });
        break;
    }
  }
  return blocks;
}

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
    ws.onmessage = (event) => void this.handleFrame(JSON.parse(event.data));
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
      } else {
        this.onPayload(payload);
      }
    }
  }

  async command(command) {
    if (!this.session || !this.key) throw new Error("Not paired");
    const id = requestId();
    const envelope = await encryptJson(this.key, { requestId: id, token: this.session.deviceToken, command });
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error("PC did not confirm this request."));
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

function QuestionToolPanel({ block, questions, disabled, pending, allowStopQuestions, onSubmit, onReject }) {
  const [selected, setSelected] = useState(() => questions.map(() => new Set()));
  const [custom, setCustom] = useState(() => questions.map(() => ""));

  useEffect(() => {
    setSelected(questions.map(() => new Set()));
    setCustom(questions.map(() => ""));
  }, [block.id, block.argsPretty, questions.length]);

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

  return React.createElement("div", { className: "question-card" },
    React.createElement("div", { className: "question-card__head" },
      React.createElement("strong", null, "Sinew needs your answer"),
      React.createElement("span", null, questions.length > 1 ? `${questions.length} questions` : "Question")
    ),
    questions.map((question, questionIndex) => React.createElement("fieldset", { key: `${block.id}-${questionIndex}`, className: "question-card__fieldset" },
      React.createElement("legend", null, question.question),
      question.options.length > 0 ? React.createElement("div", { className: "question-options" },
        question.options.map((option) => {
          const isSelected = selected[questionIndex]?.has(option.id);
          return React.createElement("button", {
            key: option.id,
            type: "button",
            className: "question-option",
            "data-selected": isSelected ? "true" : "false",
            disabled: disabled || pending,
            onClick: () => toggle(questionIndex, option.id),
          },
            React.createElement("span", null, option.label),
            option.description && React.createElement("small", null, option.description)
          );
        })
      ) : React.createElement("textarea", {
        value: custom[questionIndex] || "",
        disabled: disabled || pending,
        placeholder: "Type your answer…",
        onChange: (event) => setCustom((current) => current.map((value, index) => index === questionIndex ? event.target.value : value)),
      }),
      question.mode === "multiple_choice" && React.createElement("p", { className: "question-card__hint" }, "Choose one or more options.")
    )),
    React.createElement("div", { className: "question-card__actions" },
      React.createElement("button", { type: "button", className: "ghost", disabled: disabled || pending, onClick: () => onReject(block) }, "Dismiss"),
      allowStopQuestions && React.createElement("button", { type: "button", className: "ghost", disabled: disabled || pending || !complete, onClick: () => onSubmit(block, answers, true) }, "Answer & stop questions"),
      React.createElement("button", { type: "button", disabled: disabled || pending || !complete, onClick: () => onSubmit(block, answers, false) }, pending ? "Sending…" : "Send answer")
    )
  );
}

function App() {
  const initialCode = new URLSearchParams(location.search).get("code") || "";
  const openConversation = new URLSearchParams(location.search).get("conversation") || null;
  const [session, setSession] = useState(loadSession());
  const [status, setStatus] = useState({ relay: "connecting", pcReachable: false, revoked: false });
  const [pairCode, setPairCode] = useState(initialCode);
  const [pairError, setPairError] = useState(null);
  const [pairing, setPairing] = useState(false);
  const [bootstrap, setBootstrap] = useState(null);
  const [conversations, setConversations] = useState([]);
  const [activeConv, setActiveConv] = useState(null);
  const [liveEventsByConversation, setLiveEventsByConversation] = useState(new Map());
  const [activeTurns, setActiveTurns] = useState([]);
  const [prompt, setPrompt] = useState("");
  const [mode, setMode] = useState("act");
  const [attachments, setAttachments] = useState([]);
  const [error, setError] = useState(null);
  const [pushState, setPushState] = useState("idle");
  const [sendPending, setSendPending] = useState(false);
  const [questionPendingId, setQuestionPendingId] = useState(null);
  const [installHintDismissed, setInstallHintDismissed] = useState(() => localStorage.getItem(INSTALL_HINT_KEY) === "true");
  const clientRef = useRef(null);
  const statusRef = useRef(status);

  useEffect(() => {
    statusRef.current = status;
  }, [status]);

  const activeConvRef = useRef(activeConv);
  useEffect(() => {
    activeConvRef.current = activeConv;
  }, [activeConv]);

  const mergeBootstrap = useCallback((data) => {
    setBootstrap(data.bootstrap || data);
    const nextBootstrap = data.bootstrap || data;
    setConversations(nextBootstrap.conversations || []);
    const preferred = openConversation && (nextBootstrap.conversations || []).some((c) => c.id === openConversation);
    const active = preferred ? { ...nextBootstrap.activeConversation, id: openConversation } : nextBootstrap.activeConversation;
    setActiveConv(active);
    setMode(modeFromConversation(active));
    if (Array.isArray(data.activeTurns)) setActiveTurns(data.activeTurns);
  }, [openConversation]);

  const refresh = useCallback(async () => {
    if (!clientRef.current?.session || !statusRef.current.pcReachable) return;
    try {
      const data = await clientRef.current.command({ type: "bootstrap" });
      mergeBootstrap(data);
    } catch (err) {
      setError(String(err.message || err));
    }
  }, [mergeBootstrap]);

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
        void refresh();
      },
      onPayload: (payload) => {
        if (payload.type === "agent_event") {
          setLiveEventsByConversation((current) => {
            const next = new Map(current);
            const events = next.get(payload.conversation_id) || [];
            next.set(payload.conversation_id, [...events, payload.event]);
            return next;
          });
          if (payload.event.type === "turn_finished") void refresh();
        }
        if (payload.type === "active_turns_changed") setActiveTurns(payload.active_turns || []);
        if (payload.type === "device_revoked") {
          clearSession();
          setSession(null);
          clearLocalConversationState();
        }
      },
    });
    clientRef.current = client;
    void client.start().then(refresh);
    return () => {
      client.closed = true;
      client.ws?.close();
    };
  }, [refresh]);

  useEffect(() => {
    if (session && status.pcReachable && !bootstrap) void refresh();
  }, [session, status.pcReachable, bootstrap, refresh]);

  const isStreaming = activeTurns.some((turn) => turn.conversationId === activeConv?.id);
  const liveEvents = activeConv ? liveEventsByConversation.get(activeConv.id) || [] : [];
  const blocks = useMemo(() => buildBlocks(activeConv?.history || [], liveEvents), [activeConv, liveEvents]);
  const canReachPc = Boolean(status.pcReachable);
  const showInstallHint = Boolean(session && !installHintDismissed && !isStandaloneDisplay());

  function clearLocalConversationState() {
    setBootstrap(null);
    setConversations([]);
    setActiveConv(null);
    setLiveEventsByConversation(new Map());
    setActiveTurns([]);
  }

  async function doPair(event) {
    event.preventDefault();
    setPairing(true);
    setPairError(null);
    try {
      await clientRef.current.pair(pairCode.replace(/\D/g, ""), navigator.userAgent.includes("iPhone") ? "iPhone" : "Phone");
    } catch (err) {
      setPairing(false);
      setPairError(String(err.message || err));
    }
  }

  async function openConversationById(id) {
    if (!canReachPc) return;
    try {
      const conversation = await clientRef.current.command({ type: "load_conversation", conversation_id: id });
      setActiveConv(conversation);
      setMode(modeFromConversation(conversation));
      setLiveEventsByConversation((current) => {
        const next = new Map(current);
        next.set(id, []);
        return next;
      });
      const turn = activeTurns.find((item) => item.conversationId === id);
      if (turn) {
        const replay = await clientRef.current.command({ type: "replay_active_turn_events", conversation_id: id, after_sequence: 0 });
        setLiveEventsByConversation((current) => {
          const next = new Map(current);
          next.set(id, (replay.events || []).map((entry) => entry.event));
          return next;
        });
      }
    } catch (err) {
      setError(String(err.message || err));
    }
  }

  async function createConversation() {
    if (!canReachPc) return;
    try {
      const next = await clientRef.current.command({ type: "create_conversation" });
      setConversations(next.conversations || []);
      setActiveConv(next.activeConversation);
      setMode("act");
    } catch (err) {
      setError(String(err.message || err));
    }
  }

  async function deleteConversation(id) {
    if (!canReachPc) return;
    if (!confirm("Delete this conversation?")) return;
    try {
      const next = await clientRef.current.command({ type: "delete_conversation", conversation_id: id });
      setConversations(next.conversations || []);
      setActiveConv(next.activeConversation);
      setMode(modeFromConversation(next.activeConversation));
    } catch (err) {
      setError(String(err.message || err));
    }
  }

  async function sendPrompt(event) {
    event.preventDefault();
    if (!prompt.trim() || !activeConv || sendPending || !canReachPc) return;
    const files = attachments;
    const encoded = [];
    const text = prompt;
    setSendPending(true);
    try {
      for (const file of files) {
        encoded.push({ name: file.name, mediaType: file.type || "application/octet-stream", data: await fileToBase64(file) });
      }
      await clientRef.current.command({
        type: "send_message",
        conversation_id: activeConv.id,
        text,
        attachments: encoded,
        mode,
        model: activeConv.modeModelSettings?.[mode] || activeConv.model,
        thinking: thinkingFromModel(activeConv.modeModelSettings?.[mode] || activeConv.model),
      });
      setPrompt("");
      setAttachments([]);
      setLiveEventsByConversation((current) => {
        const next = new Map(current);
        if (!next.has(activeConv.id)) next.set(activeConv.id, [{ type: "turn_started" }]);
        return next;
      });
      setActiveConv((current) => current ? { ...current, history: [...current.history, { role: "user", parts: [{ type: "text", text }] }] } : current);
      await refresh();
    } catch (err) {
      setError(String(err.message || err));
    } finally {
      setSendPending(false);
    }
  }

  async function setConversationMode(nextMode) {
    setMode(nextMode);
    if (!activeConv || !canReachPc) return;
    try {
      const updated = await clientRef.current.command({ type: "set_conversation_mode", conversation_id: activeConv.id, mode: nextMode });
      setActiveConv(updated);
    } catch (err) {
      setError(String(err.message || err));
    }
  }

  async function compact() {
    if (!activeConv || !canReachPc) return;
    try {
      await clientRef.current.command({
        type: "compact_conversation",
        conversation_id: activeConv.id,
        model: activeConv.model,
        thinking: thinkingFromModel(activeConv.model),
      });
      await refresh();
    } catch (err) {
      setError(String(err.message || err));
    }
  }

  async function answerQuestion(block, answers, stopQuestions = false) {
    if (!activeConv || !canReachPc) return;
    setQuestionPendingId(block.id);
    try {
      await clientRef.current.command({ type: "answer_question", conversation_id: activeConv.id, tool_call_id: block.id, answers, stop_questions: stopQuestions });
      await refresh();
    } catch (err) {
      setError(String(err.message || err));
    } finally {
      setQuestionPendingId(null);
    }
  }

  async function rejectQuestion(block) {
    if (!activeConv || !canReachPc) return;
    setQuestionPendingId(block.id);
    try {
      await clientRef.current.command({ type: "reject_question", conversation_id: activeConv.id, tool_call_id: block.id });
      await refresh();
    } catch (err) {
      setError(String(err.message || err));
    } finally {
      setQuestionPendingId(null);
    }
  }

  async function enablePush() {
    if (!clientRef.current?.session || !canReachPc || !("serviceWorker" in navigator) || !("PushManager" in window)) return;
    try {
      setPushState("requesting");
      const registration = await navigator.serviceWorker.ready;
      const vapid = await fetch("/vapid-public-key", { cache: "no-store" }).then((r) => r.json());
      if (!vapid.publicKey) throw new Error("Push is not configured on the relay.");
      const subscription = await registration.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: base64UrlToBytes(vapid.publicKey) });
      const json = subscription.toJSON();
      await clientRef.current.command({ type: "subscribe_push", subscription: json });
      setPushState("enabled");
    } catch (err) {
      setPushState("error");
      setError(String(err.message || err));
    }
  }

  function dismissInstallHint() {
    localStorage.setItem(INSTALL_HINT_KEY, "true");
    setInstallHintDismissed(true);
  }

  function retryConnection() {
    setError(null);
    const client = clientRef.current;
    if (!client) return;
    const announced = client.announce();
    if (!announced && client.ws?.readyState !== WebSocket.CONNECTING) {
      client.connect();
    }
    if (statusRef.current.pcReachable) void refresh();
  }

  function logout() {
    clearSession();
    setSession(null);
    clearLocalConversationState();
  }

  if (!session) {
    return (
      React.createElement("main", { className: "pairing-screen" },
        React.createElement("section", { className: "pairing-card" },
          React.createElement("div", { className: "brand" }, React.createElement("img", { src: "/icons/icon.svg", alt: "" }), React.createElement("span", null, "Sinew Remote")),
          React.createElement("h1", null, "Pair your phone"),
          React.createElement("p", null, "Open Remote on your PC, scan the QR or enter the 6-digit code."),
          React.createElement("form", { onSubmit: doPair },
            React.createElement("input", { inputMode: "numeric", pattern: "[0-9]*", maxLength: 6, value: pairCode, onChange: (e) => setPairCode(e.target.value.replace(/\D/g, "").slice(0, 6)), placeholder: "000000", autoFocus: true }),
            React.createElement("button", { disabled: pairCode.length !== 6 || pairing }, pairing ? "Pairing…" : "Pair")
          ),
          pairError && React.createElement("p", { className: "error" }, pairError),
          React.createElement("p", { className: "small" }, "Traffic is end-to-end encrypted between this phone and your PC.")
        )
      )
    );
  }

  return (
    React.createElement("main", { className: "remote-app" },
      React.createElement("header", { className: "topbar" },
        React.createElement("div", { className: "topbar__title" },
          React.createElement("strong", null, bootstrap?.workspace?.name || "Sinew"),
          React.createElement("span", { className: canReachPc ? "online" : "offline" }, canReachPc ? "PC reachable" : `PC unreachable · relay ${status.relay}`)
        ),
        React.createElement("div", { className: "topbar__actions" },
          React.createElement("button", { onClick: enablePush, className: pushState === "enabled" ? "pill ok" : "pill", disabled: !canReachPc || pushState === "requesting" }, pushState === "enabled" ? "Push on" : pushState === "requesting" ? "Enabling…" : "Enable push"),
          React.createElement("button", { onClick: logout, className: "ghost" }, "Forget")
        )
      ),
      error && React.createElement("button", { className: "toast", onClick: () => setError(null) }, error),
      showInstallHint && React.createElement("section", { className: "install-card" },
        React.createElement("div", null,
          React.createElement("h2", null, "Install Sinew Remote"),
          React.createElement("p", null, isIosDevice()
            ? "For notifications on iPhone, add this app to your Home Screen first, then enable push from here."
            : "Install the PWA for a full-screen chat and reliable response-ready notifications."),
          React.createElement("small", null, "Notification text stays generic; conversation details are decrypted only when you open the app.")
        ),
        React.createElement("button", { type: "button", className: "ghost", onClick: dismissInstallHint }, "Got it")
      ),
      !canReachPc && React.createElement("section", { className: "offline-panel" },
        React.createElement("h2", null, "PC unreachable"),
        React.createElement("p", null, "Keep Sinew open on your PC with Remote enabled. Messages stay on this phone until the PC confirms the request."),
        React.createElement("button", { onClick: retryConnection }, "Retry connection")
      ),
      React.createElement("div", { className: "mobile-layout" },
        React.createElement("aside", { className: "conversation-rail" },
          React.createElement("div", { className: "rail-actions" },
            React.createElement("button", { onClick: createConversation, disabled: !canReachPc }, "+ New"),
            React.createElement("button", { onClick: refresh, disabled: !canReachPc }, "Refresh")
          ),
          conversations.length === 0 && React.createElement("p", { className: "rail-empty" }, canReachPc ? "No conversations yet." : "Conversations will load when the PC is reachable."),
          conversations.map((conversation) => React.createElement("button", { key: conversation.id, disabled: !canReachPc, className: conversation.id === activeConv?.id ? "conv active" : "conv", onClick: () => openConversationById(conversation.id) },
            React.createElement("span", null, conversation.title || "New conversation"),
            React.createElement("small", null, activeTurns.some((turn) => turn.conversationId === conversation.id) ? "Streaming" : new Date(conversation.updatedAtMs || Date.now()).toLocaleDateString()),
            React.createElement("i", { role: "button", "aria-label": "Delete conversation", onClick: (event) => { event.stopPropagation(); void deleteConversation(conversation.id); } }, "×")
          ))
        ),
        React.createElement("section", { className: "chat-shell" },
          activeConv ? React.createElement(React.Fragment, null,
            React.createElement("div", { className: "chat-head" },
              React.createElement("div", null,
                React.createElement("h2", null, activeConv.title || "Conversation"),
                React.createElement("span", { className: isStreaming ? "streaming" : "chat-head__meta" }, isStreaming ? "Streaming live" : sendPending ? "Waiting for PC confirmation" : "Encrypted remote chat")
              ),
              React.createElement("button", { className: "ghost", disabled: !canReachPc || sendPending, onClick: compact }, "Compact"),
              React.createElement("div", { className: "modes" }, MODES.map((m) => React.createElement("button", { key: m, disabled: !canReachPc, className: mode === m ? "selected" : "", onClick: () => setConversationMode(m) }, modeLabels[m])))
            ),
            React.createElement("div", { className: "messages" },
              blocks.length === 0 && React.createElement("div", { className: "empty-message" }, "Start the conversation from your phone."),
              blocks.map((block, index) => {
                if (block.kind === "user") return React.createElement("article", { key: index, className: "bubble user" }, block.text);
                if (block.kind === "assistant") return React.createElement("article", { key: index, className: "bubble assistant", dangerouslySetInnerHTML: htmlFromMarkdown(block.text) });
                if (block.kind === "thinking") return React.createElement("details", { key: index, className: "thinking" }, React.createElement("summary", null, "Thinking"), React.createElement("pre", null, block.text));
                if (block.kind === "tool") {
                  const questions = block.name === "question" ? parseQuestionArgs(block.argsPretty || block.args) : [];
                  const canAnswer = questions.length > 0 && block.status !== "done" && block.status !== "error";
                  return React.createElement("article", { key: index, className: `tool ${block.status}` },
                    React.createElement("strong", null, block.summary || block.name || "Tool"),
                    block.output && React.createElement("pre", null, block.output),
                    canAnswer && React.createElement(QuestionToolPanel, {
                      block,
                      questions,
                      disabled: !canReachPc,
                      pending: questionPendingId === block.id,
                      allowStopQuestions: mode === "plan",
                      onSubmit: answerQuestion,
                      onReject: rejectQuestion,
                    })
                  );
                }
                if (block.kind === "error") return React.createElement("article", { key: index, className: "bubble error" }, block.text);
                return React.createElement("div", { key: index, className: "status-line" }, block.text);
              })
            ),
            React.createElement("form", { className: "composer", onSubmit: sendPrompt },
              attachments.length > 0 && React.createElement("div", { className: "attachments" }, attachments.map((file, index) => React.createElement("span", { key: `${file.name}-${index}` }, file.name))),
              sendPending && React.createElement("div", { className: "composer__pending" }, "Waiting for the PC to accept this message…"),
              React.createElement("textarea", { rows: 3, value: prompt, disabled: !canReachPc || sendPending, onChange: (e) => setPrompt(e.target.value), placeholder: canReachPc ? "Message Sinew…" : "PC unreachable" }),
              React.createElement("label", { className: "attach", "data-disabled": !canReachPc || sendPending ? "true" : "false" }, "Attach", React.createElement("input", { type: "file", multiple: true, disabled: !canReachPc || sendPending, onChange: (e) => setAttachments(Array.from(e.target.files || [])) })),
              React.createElement("button", { disabled: !prompt.trim() || !canReachPc || sendPending }, sendPending ? "Sending…" : "Send")
            )
          ) : React.createElement("div", { className: "empty" },
            React.createElement("button", { onClick: refresh, disabled: !canReachPc }, canReachPc ? "Load conversations" : "Waiting for PC")
          )
        )
      )
    )
  );
}

if ("serviceWorker" in navigator) {
  navigator.serviceWorker.register("/sw.js").catch(() => undefined);
}

createRoot(document.getElementById("app")).render(React.createElement(App));
