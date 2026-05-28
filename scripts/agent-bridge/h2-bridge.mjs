#!/usr/bin/env node
/**
 * HTTP/2 bidirectional pipe for Cursor agent.v1 (from cursor-oauth-opencode).
 */
import http2 from "node:http2";
import crypto from "node:crypto";

function writeMessage(data) {
  const lenBuf = Buffer.alloc(4);
  lenBuf.writeUInt32BE(data.length, 0);
  process.stdout.write(lenBuf);
  process.stdout.write(data);
}

let stdinBuf = Buffer.alloc(0);
let stdinResolve = null;
let stdinEnded = false;

process.stdin.on("data", (chunk) => {
  stdinBuf = Buffer.concat([stdinBuf, chunk]);
  if (stdinResolve) {
    const r = stdinResolve;
    stdinResolve = null;
    r();
  }
});

process.stdin.on("end", () => {
  stdinEnded = true;
  if (stdinResolve) {
    const r = stdinResolve;
    stdinResolve = null;
    r();
  }
});

function waitForData() {
  return new Promise((resolve) => {
    stdinResolve = resolve;
  });
}

async function readExact(n) {
  while (stdinBuf.length < n) {
    if (stdinEnded) return null;
    await waitForData();
  }
  const result = stdinBuf.subarray(0, n);
  stdinBuf = stdinBuf.subarray(n);
  return Buffer.from(result);
}

async function readMessage() {
  const lenBuf = await readExact(4);
  if (!lenBuf) return null;
  const len = lenBuf.readUInt32BE(0);
  if (len === 0) return Buffer.alloc(0);
  return readExact(len);
}

const configBuf = await readMessage();
if (!configBuf) process.exit(1);

const config = JSON.parse(configBuf.toString("utf8"));
const { accessToken, url, path: rpcPath, unary } = config;
const extraHeaders =
  config.headers && typeof config.headers === "object" ? config.headers : {};

const client = http2.connect(url || "https://api2.cursor.sh");

let timeout = setTimeout(killBridge, 30_000);

function resetTimeout() {
  clearTimeout(timeout);
  timeout = setTimeout(killBridge, 120_000);
}

function killBridge() {
  clearTimeout(timeout);
  client.destroy();
  process.exit(1);
}

client.on("error", () => {
  clearTimeout(timeout);
  process.exit(1);
});

const headers = {
  ":method": "POST",
  ":path": rpcPath || "/agent.v1.AgentService/Run",
  "content-type": unary ? "application/proto" : "application/connect+proto",
  te: "trailers",
  authorization: `Bearer ${accessToken}`,
  "x-ghost-mode": "true",
  "x-cursor-client-version":
    extraHeaders["x-cursor-client-version"] || "cli-2026.01.09-231024f",
  "x-cursor-client-type": extraHeaders["x-cursor-client-type"] || "cli",
  "x-request-id": crypto.randomUUID(),
};
for (const [key, value] of Object.entries(extraHeaders)) {
  if (!key.startsWith(":") && typeof value === "string" && value.length > 0) {
    headers[key] = value;
  }
}
if (!unary) {
  headers["connect-protocol-version"] = "1";
}

const h2Stream = client.request(headers);

h2Stream.on("data", (chunk) => {
  resetTimeout();
  writeMessage(chunk);
});

h2Stream.on("end", () => {
  clearTimeout(timeout);
  client.close();
  setTimeout(() => process.exit(0), 100);
});

h2Stream.on("error", () => {
  clearTimeout(timeout);
  client.close();
  process.exit(1);
});

if (unary) {
  const body = await readMessage();
  if (body && body.length > 0 && !h2Stream.closed && !h2Stream.destroyed) {
    h2Stream.end(body);
  } else {
    h2Stream.end();
  }
} else {
  (async () => {
    while (true) {
      const msg = await readMessage();
      if (!msg || msg.length === 0) break;
      if (!h2Stream.closed && !h2Stream.destroyed) {
        resetTimeout();
        h2Stream.write(msg);
      }
    }
    if (!h2Stream.closed && !h2Stream.destroyed) {
      h2Stream.end();
    }
  })();
}
