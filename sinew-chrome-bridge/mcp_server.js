const readline = require('readline');
const http = require('http');
const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');

const BRIDGE_ORIGIN = process.env.MCP_BROWSER_CDP_URL || 'http://localhost:29002';
const CHROME_WAIT_MS = 20000;
const BRIDGE_WAIT_MS = 20000;

// Log errors to stderr so they don't corrupt the stdout JSON-RPC stream
function log(msg) {
  console.error(`[MCP Log] ${msg}`);
}

function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

function requestJSON(url, timeoutMs = 3000) {
  return new Promise((resolve, reject) => {
    const req = http.get(url, (res) => {
      let data = '';
      res.on('data', (chunk) => data += chunk);
      res.on('end', () => {
        try {
          resolve(JSON.parse(data || 'null'));
        } catch (e) {
          reject(new Error(`Invalid JSON from ${url}: ${e.message}`));
        }
      });
    });
    req.setTimeout(timeoutMs, () => {
      req.destroy(new Error(`Timeout querying ${url}`));
    });
    req.on('error', reject);
  });
}

function findChromeExecutable() {
  const candidates = [
    process.env.CHROME_PATH,
    'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
    'C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe',
    path.join(process.env.LOCALAPPDATA || '', 'Google', 'Chrome', 'Application', 'chrome.exe'),
  ].filter(Boolean);

  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) return candidate;
  }
  return 'chrome';
}

function normalizeUrl(raw) {
  if (!raw) return null;
  const value = String(raw).trim().replace(/[)\],.;!?]+$/g, '');
  if (/^https?:\/\//i.test(value)) return value;
  if (/^[a-z0-9-]+(\.[a-z0-9-]+)+(\/[^\s]*)?$/i.test(value)) return `https://${value}`;
  return null;
}

function extractUrl(task) {
  if (!task) return null;
  const explicit = String(task).match(/https?:\/\/[^\s)\],.;!?]+/i);
  if (explicit) return normalizeUrl(explicit[0]);

  const domain = String(task).match(/\b[a-z0-9-]+(\.[a-z0-9-]+)+(\/[^\s)\],.;!?]*)?/i);
  return domain ? normalizeUrl(domain[0]) : null;
}

async function requestBridgeLaunch(targetUrl) {
  return requestJSON(`${BRIDGE_ORIGIN}/api/launch_chrome?url=${encodeURIComponent(normalizeUrl(targetUrl) || 'about:blank')}`, 3000).catch(() => null);
}

function launchChrome(targetUrl = 'about:blank') {
  const chromeExe = findChromeExecutable();
  log(`Launching Chrome via ${chromeExe}`);
  const args = [
    '--silent-debugger-extension-api',
    '--no-first-run',
    '--no-default-browser-check',
    normalizeUrl(targetUrl) || 'about:blank',
  ];

  try {
    const child = spawn(chromeExe, args, {
      detached: true,
      stdio: 'ignore',
      windowsHide: true,
    });
    child.unref();
  } catch (err) {
    log(`Failed to launch Chrome: ${err.message}`);
  }
}

async function waitForBridge() {
  const deadline = Date.now() + BRIDGE_WAIT_MS;
  let lastError = null;

  while (Date.now() < deadline) {
    try {
      const status = await requestJSON(`${BRIDGE_ORIGIN}/api/status`, 1500);
      if (status && (status.extensionConnected || status.hasExtensionSocket)) return status;
    } catch (err) {
      lastError = err;
    }
    await sleep(500);
  }

  throw new Error(`Chrome bridge did not become ready${lastError ? `: ${lastError.message}` : ''}`);
}

async function waitForTabs(preferredUrl = null) {
  const deadline = Date.now() + CHROME_WAIT_MS;
  let lastTabs = [];

  while (Date.now() < deadline) {
    let tabs = await requestJSON(`${BRIDGE_ORIGIN}/json`, 3000).catch(() => []);
    if (Array.isArray(tabs) && tabs.length > 0) return tabs;

    if (preferredUrl) {
      const created = await requestJSON(`${BRIDGE_ORIGIN}/api/create_tab?url=${encodeURIComponent(preferredUrl)}`, 7000).catch(() => null);
      if (created && created.success && created.tab) {
        return [created.tab];
      }
    }

    if (Array.isArray(tabs)) lastTabs = tabs;
    await sleep(700);
  }

  return lastTabs;
}

async function ensureChromeReady(preferredUrl = null) {
  const targetUrl = normalizeUrl(preferredUrl) || 'https://www.google.com';
  await requestBridgeLaunch(targetUrl);
  launchChrome(targetUrl);
  await waitForBridge();
  const tabs = await waitForTabs(targetUrl);
  if (!tabs || tabs.length === 0) {
    throw new Error('Chrome bridge is connected, but no debuggable tab could be created.');
  }
  return tabs;
}

function buildActionTasks(task) {
  const text = String(task || '').toLowerCase();
  const actions = [];
  if (text.includes('hamburger') || text.includes('menu')) {
    actions.push('clique le bouton menu hamburger');
    if (text.includes('referme') || text.includes('ferme') || text.includes('close')) {
      actions.push('clique le bouton menu hamburger');
    }
  }
  if (text.includes('trinity')) {
    actions.push('clique la carte Trinity');
  }
  return actions.length > 0 ? actions : [task];
}

async function executeAction(tabId, taskText, timeoutMs = 30000) {
  const taskUrl = `${BRIDGE_ORIGIN}/api/execute_silent_task?tabId=${encodeURIComponent(tabId)}&task=${encodeURIComponent(taskText)}`;
  return await requestJSON(taskUrl, timeoutMs);
}

// Execute the smart browser automation natively and silently (Codex-style local mode)
async function executeBrowserTask(task) {
  log(`Executing native Chrome action: "${task}"`);

  try {
    const preferredUrl = extractUrl(task) || 'https://www.google.com';
    const tabs = await ensureChromeReady(preferredUrl);
    const tab = tabs[0];
    log(`Executing on tab: ${tab.title} (ID: ${tab.id})`);

    const results = [];
    for (const actionTask of buildActionTasks(task)) {
      log(`Executing action step: ${actionTask}`);
      results.push({ task: actionTask, result: await executeAction(tab.id, actionTask) });
      await sleep(900);
    }
    return JSON.stringify({ success: results.every(r => r.result && r.result.success !== false), results });
  } catch (err) {
    return `Error: ${err.message}`;
  }
}

// Line reader for STDIN JSON-RPC communication
const rl = readline.createInterface({
  input: process.stdin,
  output: process.stdout,
  terminal: false
});

rl.on('line', async (line) => {
  if (!line.trim()) return;

  try {
    const request = JSON.parse(line);
    const { method, id, params } = request;

    log(`Request received: ${method} (id: ${id})`);

    if (method === 'initialize') {
      console.log(JSON.stringify({
        jsonrpc: '2.0',
        id,
        result: {
          protocolVersion: '2024-11-05',
          capabilities: { tools: {} },
          serverInfo: { name: 'sinew-chrome-mcp-native', version: '1.0.0' }
        }
      }));
    } else if (method === 'tools/list') {
      console.log(JSON.stringify({
        jsonrpc: '2.0',
        id,
        result: {
          tools: [
            {
              name: 'run_browser_agent',
              description: "Exécute une tâche de navigation ou d'interaction avec Google Chrome localement, sans API cloud, sans browser-use Python et sans ouverture manuelle de Chrome.",
              inputSchema: {
                type: 'object',
                properties: {
                  task: {
                    type: 'string',
                    description: "Description de l'action à faire (ex: 'ouvre julienpiron.fr puis clique sur le menu')"
                  }
                },
                required: ['task']
              }
            }
          ]
        }
      }));
    } else if (method === 'tools/call') {
      const toolName = params.name;
      const args = params.arguments || {};
      log(`Calling tool: ${toolName}`);

      if (toolName === 'run_browser_agent') {
        const resultText = await executeBrowserTask(args.task || '');
        console.log(JSON.stringify({
          jsonrpc: '2.0',
          id,
          result: {
            content: [{ type: 'text', text: resultText }]
          }
        }));
      } else {
        console.log(JSON.stringify({
          jsonrpc: '2.0',
          id,
          error: { code: -32601, message: `Method not found: ${toolName}` }
        }));
      }
    } else if (id !== undefined) {
      console.log(JSON.stringify({ jsonrpc: '2.0', id, result: {} }));
    }
  } catch (err) {
    log(`Error handling line: ${err.message}`);
  }
});
