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
  await ensureAgentBridge();
  await ensureChromeBridgeDependencies();
}

async function ensureAgentBridge() {
  if (!existsSync(path.join(bridgeDir, "package.json"))) {
    throw new Error(`agent-bridge package.json missing: ${bridgeDir}`);
  }
  if (existsSync(tsxBin) && existsSync(vendorProto)) {
    console.log(`agent-bridge deps already present: ${path.relative(scriptDir, bridgeDir)}`);
  } else {
    const npm = process.platform === "win32" ? "npm.cmd" : "npm";
    console.log(`Installing agent-bridge dependencies in ${bridgeDir}...`);
    await execFile(npm, ["ci", "--omit=dev"], {
      cwd: bridgeDir,
      env: process.env,
      shell: true,
      windowsHide: true,
    });
    if (!existsSync(tsxBin)) {
      throw new Error(`tsx missing after npm ci: ${tsxBin}`);
    }
  }

  const exportDescriptor = path.join(scriptDir, "export-agent-descriptor.mjs");
  const fdsPath = path.join(scriptDir, "..", "crates", "sinew-cursor", "proto", "agent.fds");
  if (existsSync(vendorProto) && (!existsSync(fdsPath) || process.env.FORCE_AGENT_FDS === "1")) {
    const node = process.execPath;
    await execFile(node, [exportDescriptor], { cwd: scriptDir, windowsHide: true });
  }
  console.log("agent-bridge ready for bundle/runtime.");
}

async function ensureChromeBridgeDependencies() {
  const chromeBridgeDir = path.join(scriptDir, "..", "sinew-chrome-bridge");
  if (!existsSync(path.join(chromeBridgeDir, "package.json"))) {
    console.log("Chrome bridge directory or package.json missing, skipping.");
    return;
  }
  const wsDir = path.join(chromeBridgeDir, "node_modules", "ws");
  if (existsSync(wsDir)) {
    console.log("Chrome bridge dependencies already present.");
    return;
  }
  const npm = process.platform === "win32" ? "npm.cmd" : "npm";
  console.log(`Installing Chrome bridge dependencies in ${chromeBridgeDir}...`);
  await execFile(npm, ["install", "--omit=dev"], {
    cwd: chromeBridgeDir,
    env: process.env,
    shell: true,
    windowsHide: true,
  });
  console.log("Chrome bridge ready.");
}
