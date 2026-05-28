import { mkdir, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const vendorDir = path.join(__dirname, "vendor");
const protoUrl =
  "https://raw.githubusercontent.com/jaredboynton/cursor-oauth-opencode/main/src/proto/agent_pb.ts";
const outPath = path.join(vendorDir, "agent_pb.ts");

if (existsSync(outPath)) {
  console.log("agent_pb.ts already present");
  process.exit(0);
}

await mkdir(vendorDir, { recursive: true });
const res = await fetch(protoUrl);
if (!res.ok) {
  console.error(`Failed to download agent_pb.ts: ${res.status}`);
  process.exit(1);
}
const text = await res.text();
await writeFile(outPath, text, "utf8");
console.log(`Wrote ${outPath} (${text.length} bytes)`);
