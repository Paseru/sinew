#!/usr/bin/env node
/**
 * Export agent.v1 FileDescriptorSet from vendor/agent_pb.ts for prost-build.
 * Requires: cd scripts/agent-bridge && npm ci
 * Run: node scripts/export-agent-descriptor.mjs
 */
import { writeFile, mkdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const bridgeDir = path.join(__dirname, "agent-bridge");
const protobufRoot = path.join(bridgeDir, "node_modules", "@bufbuild", "protobuf");
const { fromBinary, FileDescriptorSetSchema } = await import(
  pathToFileURL(path.join(protobufRoot, "dist", "esm", "index.js")).href
);
const { file_agent } = await import(
  pathToFileURL(path.join(bridgeDir, "vendor", "agent_pb.ts")).href
);

const outDir = path.join(__dirname, "..", "crates", "sinew-cursor", "proto");
const outPath = path.join(outDir, "agent.fds");

// fileDesc() embeds a serialized FileDescriptorSet (base64 in generated TS).
const raw = file_agent.proto;
if (!raw) {
  console.error("file_agent.proto missing — run: cd scripts/agent-bridge && npm install");
  process.exit(1);
}

const bytes = Buffer.from(raw, "base64");
const fds = fromBinary(FileDescriptorSetSchema, bytes);
await mkdir(outDir, { recursive: true });
await writeFile(outPath, Buffer.from(bytes));
console.log(`Wrote ${outPath} (${bytes.length} bytes, ${fds.file.length} files)`);
