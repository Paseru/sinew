#!/usr/bin/env node
/**
 * Export agent.v1 FileDescriptorSet embedded in vendor/agent_pb.ts for prost-build.
 * Requires: cd scripts/agent-bridge && npm ci
 * Run: node scripts/export-agent-descriptor.mjs
 */
import { readFile, writeFile, mkdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const bridgeDir = path.join(__dirname, "agent-bridge");
const vendorPath = path.join(bridgeDir, "vendor", "agent_pb.ts");
const outDir = path.join(__dirname, "..", "crates", "sinew-cursor", "proto");
const outPath = path.join(outDir, "agent.fds");

const source = await readFile(vendorPath, "utf8");
const match = source.match(/fileDesc\(\s*\n?\s*"([A-Za-z0-9+/=]+)"/);
if (!match) {
  console.error(`Could not find fileDesc(...) payload in ${vendorPath}`);
  process.exit(1);
}

const bytes = Buffer.from(match[1], "base64");
await mkdir(outDir, { recursive: true });
await writeFile(outPath, bytes);
console.log(`Wrote ${outPath} (${bytes.length} bytes)`);
