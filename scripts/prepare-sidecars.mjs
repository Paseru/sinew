import { execFile as execFileCallback } from "node:child_process";
import { promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const execFile = promisify(execFileCallback);

const RG_VERSION = "14.1.1";
const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, "..");
const binariesDir = path.join(projectRoot, "src-tauri", "binaries");

await main();

async function main() {
  await fs.mkdir(binariesDir, { recursive: true });
  const targets = sidecarTargets();
  for (const target of targets.filter((target) => !target.universal)) {
    await ensureRipgrepTarget(target);
  }
  for (const target of targets.filter((target) => target.universal)) {
    await ensureUniversalTarget(target);
  }
}

function sidecarTargets() {
  if (process.platform === "win32") {
    return [
      {
        archiveTriple: "x86_64-pc-windows-msvc",
        outputTriple: "x86_64-pc-windows-msvc",
        archiveExt: "zip",
        executable: "rg.exe",
        outputExt: ".exe",
      },
    ];
  }

  if (process.platform === "darwin") {
    const x64 = {
      archiveTriple: "x86_64-apple-darwin",
      outputTriple: "x86_64-apple-darwin",
      archiveExt: "tar.gz",
      executable: "rg",
      outputExt: "",
    };
    const arm64 = {
      archiveTriple: "aarch64-apple-darwin",
      outputTriple: "aarch64-apple-darwin",
      archiveExt: "tar.gz",
      executable: "rg",
      outputExt: "",
    };
    return [
      x64,
      arm64,
      {
        universal: true,
        outputTriple: "universal-apple-darwin",
        outputExt: "",
        inputs: [x64, arm64],
      },
    ];
  }

  if (process.platform === "linux") {
    if (process.arch === "x64") {
      return [
        {
          archiveTriple: "x86_64-unknown-linux-musl",
          outputTriple: "x86_64-unknown-linux-gnu",
          archiveExt: "tar.gz",
          executable: "rg",
          outputExt: "",
        },
      ];
    }
    if (process.arch === "arm64") {
      return [
        {
          archiveTriple: "aarch64-unknown-linux-gnu",
          outputTriple: "aarch64-unknown-linux-gnu",
          archiveExt: "tar.gz",
          executable: "rg",
          outputExt: "",
        },
      ];
    }
  }

  throw new Error(`No ripgrep sidecar target configured for ${process.platform}/${process.arch}`);
}

async function ensureRipgrepTarget(target) {
  const outputPath = sidecarOutputPath(target);
  if (await fileExists(outputPath)) {
    console.log(`ripgrep sidecar already present: ${path.relative(projectRoot, outputPath)}`);
    return outputPath;
  }

  const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "claakecode-rg-"));
  const archiveName = `ripgrep-${RG_VERSION}-${target.archiveTriple}.${target.archiveExt}`;
  const archivePath = path.join(tempRoot, archiveName);
  const extractDir = path.join(tempRoot, "extract");
  const url = `https://github.com/BurntSushi/ripgrep/releases/download/${RG_VERSION}/${archiveName}`;

  console.log(`downloading ${archiveName}`);
  await download(url, archivePath);
  await fs.mkdir(extractDir, { recursive: true });
  await extractArchive(archivePath, extractDir, target.archiveExt);

  const extractedPath = path.join(
    extractDir,
    `ripgrep-${RG_VERSION}-${target.archiveTriple}`,
    target.executable,
  );
  await fs.copyFile(extractedPath, outputPath);
  if (process.platform !== "win32") {
    await fs.chmod(outputPath, 0o755);
  }
  await fs.rm(tempRoot, { recursive: true, force: true });
  console.log(`prepared ${path.relative(projectRoot, outputPath)}`);
  return outputPath;
}

async function ensureUniversalTarget(target) {
  const outputPath = sidecarOutputPath(target);
  if (await fileExists(outputPath)) {
    console.log(`ripgrep universal sidecar already present: ${path.relative(projectRoot, outputPath)}`);
    return outputPath;
  }

  const inputPaths = target.inputs.map(sidecarOutputPath);
  await execFile("lipo", ["-create", "-output", outputPath, ...inputPaths]);
  await fs.chmod(outputPath, 0o755);
  console.log(`prepared ${path.relative(projectRoot, outputPath)}`);
  return outputPath;
}

function sidecarOutputPath(target) {
  return path.join(binariesDir, `rg-${target.outputTriple}${target.outputExt}`);
}

async function download(url, destination) {
  const response = await fetch(url, { redirect: "follow" });
  if (!response.ok) {
    throw new Error(`failed to download ${url}: ${response.status} ${response.statusText}`);
  }
  const bytes = Buffer.from(await response.arrayBuffer());
  await fs.writeFile(destination, bytes);
}

async function extractArchive(archivePath, extractDir, archiveExt) {
  const args = archiveExt === "zip"
    ? ["-xf", archivePath, "-C", extractDir]
    : ["-xzf", archivePath, "-C", extractDir];
  await execFile("tar", args);
}

async function fileExists(filePath) {
  try {
    const stat = await fs.stat(filePath);
    return stat.isFile();
  } catch {
    return false;
  }
}
