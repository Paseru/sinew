import { execFile as execFileCallback } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const execFile = promisify(execFileCallback);

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const bridgeDir = path.join(scriptDir, "agent-bridge");
const tsxBin = path.join(
  bridgeDir,
  "node_modules",
  ".bin",
  process.platform === "win32" ? "tsx.cmd" : "tsx",
);
const vendorProto = path.join(bridgeDir, "vendor", "agent_pb.ts");

await main();

async function main() {
  if (!existsSync(path.join(bridgeDir, "package.json"))) {
    throw new Error(`agent-bridge package.json missing: ${bridgeDir}`);
  }
  if (existsSync(tsxBin) && existsSync(vendorProto)) {
    console.log(`agent-bridge deps already present: ${path.relative(scriptDir, bridgeDir)}`);
    return;
  }
  const npm = process.platform === "win32" ? "npm.cmd" : "npm";
  console.log(`Installing agent-bridge dependencies in ${bridgeDir}...`);
  await execFile(npm, ["ci", "--omit=dev"], {
    cwd: bridgeDir,
    env: process.env,
    windowsHide: true,
  });
  if (!existsSync(tsxBin)) {
    throw new Error(`tsx missing after npm ci: ${tsxBin}`);
  }
  console.log("agent-bridge ready for bundle/runtime.");
}
