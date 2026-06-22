import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { spawn } from "node:child_process";

const REFERO_URL = "https://styles.refero.design/";
const DEFAULT_LIMIT = 10;
const DEFAULT_OUT_DIR = "refero-heroes";
const VIEWPORT = "1440,1000";
const WAIT_MS = 2500;
const TIMEOUT_MS = 45000;

function argValue(name, fallback) {
  const index = process.argv.indexOf(name);
  if (index === -1 || index === process.argv.length - 1) {
    return fallback;
  }
  return process.argv[index + 1];
}

function slugify(value) {
  return value
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64) || "site";
}

function run(command, args) {
  return new Promise((resolve) => {
    const child = spawn(command, args, {
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, PLAYWRIGHT_BROWSERS_PATH: "0" },
    });

    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("close", (code) => {
      resolve({ code, stdout, stderr });
    });
  });
}

async function fetchReferoStyles(limit) {
  const response = await fetch(REFERO_URL);
  if (!response.ok) {
    throw new Error(`Refero returned ${response.status} ${response.statusText}`);
  }

  const html = await response.text();
  const decoded = html.replace(/\\"/g, '"').replace(/\\\//g, "/");
  const pattern = /"id":"([^"]+)","url":"([^"]+)","siteName":"([^"]+)"/g;
  const styles = [];
  const seen = new Set();

  for (const match of decoded.matchAll(pattern)) {
    const [, id, url, siteName] = match;
    if (seen.has(id)) {
      continue;
    }
    seen.add(id);
    styles.push({
      id,
      siteName,
      url,
      referoUrl: new URL(`/style/${id}`, REFERO_URL).href,
    });
    if (styles.length >= limit) {
      break;
    }
  }

  if (styles.length === 0) {
    throw new Error("No Refero styles found in the page HTML.");
  }

  return styles;
}

async function capture(style, index, screenshotsDir) {
  const filename = `${String(index + 1).padStart(3, "0")}-${slugify(style.siteName)}.png`;
  const outputPath = path.join(screenshotsDir, filename);
  const args = [
    "exec",
    "--yes",
    "--package",
    "playwright",
    "--",
    "playwright",
    "screenshot",
    "--channel",
    "chrome",
    "--viewport-size",
    VIEWPORT,
    "--wait-for-timeout",
    String(WAIT_MS),
    "--timeout",
    String(TIMEOUT_MS),
    style.url,
    outputPath,
  ];

  const result = await run("npm", args);
  if (result.code !== 0) {
    return {
      ...style,
      index: index + 1,
      status: "error",
      screenshot: null,
      error: (result.stderr || result.stdout).trim(),
    };
  }

  return {
    ...style,
    index: index + 1,
    status: "ok",
    screenshot: outputPath,
  };
}

async function main() {
  const limit = Number.parseInt(argValue("--limit", String(DEFAULT_LIMIT)), 10);
  const outDir = path.resolve(argValue("--out", DEFAULT_OUT_DIR));
  const screenshotsDir = path.join(outDir, "screenshots");

  await mkdir(screenshotsDir, { recursive: true });

  console.log(`Fetching first ${limit} Refero styles...`);
  const styles = await fetchReferoStyles(limit);
  const results = [];

  for (const [index, style] of styles.entries()) {
    console.log(`[${index + 1}/${styles.length}] ${style.siteName} -> ${style.url}`);
    const result = await capture(style, index, screenshotsDir);
    results.push(result);

    if (result.status === "error") {
      console.log(`  failed: ${result.error.split("\n").at(-1)}`);
    } else {
      console.log(`  saved: ${result.screenshot}`);
    }
  }

  const metadata = {
    generatedAt: new Date().toISOString(),
    source: REFERO_URL,
    limit,
    viewport: VIEWPORT,
    waitMs: WAIT_MS,
    timeoutMs: TIMEOUT_MS,
    results,
  };
  const errors = results.filter((result) => result.status === "error");

  await writeFile(path.join(outDir, "metadata.json"), `${JSON.stringify(metadata, null, 2)}\n`);
  await writeFile(path.join(outDir, "errors.json"), `${JSON.stringify(errors, null, 2)}\n`);

  console.log(`Done: ${results.length - errors.length}/${results.length} screenshots captured.`);
  if (errors.length > 0) {
    process.exitCode = 1;
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
