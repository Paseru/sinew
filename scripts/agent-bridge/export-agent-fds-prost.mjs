#!/usr/bin/env node
/**
 * Export agent.v1 as standard google.protobuf.FileDescriptorSet (prost-reflect).
 */
import { readFile, writeFile, mkdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { create, fromBinary, toBinary } from "@bufbuild/protobuf";
import {
  FileDescriptorProtoSchema,
  FileDescriptorSetSchema,
} from "@bufbuild/protobuf/wkt";
import { base64Decode } from "@bufbuild/protobuf/wire";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const vendorPath = path.join(__dirname, "vendor", "agent_pb.ts");
const outPath = path.join(
  __dirname,
  "..",
  "..",
  "crates",
  "sinew-cursor",
  "proto",
  "agent.pb",
);

const source = await readFile(vendorPath, "utf8");
const match = source.match(/fileDesc\(\s*\n?\s*"([A-Za-z0-9+/=]+)"/);
if (!match) {
  console.error(`Could not find fileDesc(...) in ${vendorPath}`);
  process.exit(1);
}

const root = fromBinary(FileDescriptorProtoSchema, base64Decode(match[1]));
const fds = create(FileDescriptorSetSchema, { file: [root] });
const bytes = toBinary(FileDescriptorSetSchema, fds);
await mkdir(path.dirname(outPath), { recursive: true });
await writeFile(outPath, bytes);
console.log(`Wrote ${outPath} (${bytes.length} bytes)`);
