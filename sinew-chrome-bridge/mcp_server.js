const readline = require('readline');
const http = require('http');
const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');
const WebSocket = require('./node_modules/ws');

const BRIDGE_ORIGIN = process.env.MCP_BROWSER_CDP_URL || 'http://localhost:29002';
const CHROME_WAIT_MS = 20000;
const BRIDGE_WAIT_MS = 20000;
const cursorStateByTabId = new Map();

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

function sameOriginOrUrl(tabUrl, targetUrl) {
  try {
    const tab = new URL(tabUrl);
    const target = new URL(targetUrl);
    return tab.hostname.replace(/^www\./, '') === target.hostname.replace(/^www\./, '');
  } catch {
    return false;
  }
}

async function waitForTabs(preferredUrl = null) {
  const deadline = Date.now() + CHROME_WAIT_MS;
  let lastTabs = [];

  while (Date.now() < deadline) {
    let tabs = await requestJSON(`${BRIDGE_ORIGIN}/json`, 3000).catch(() => []);
    if (Array.isArray(tabs) && tabs.length > 0) {
      if (!preferredUrl) return tabs;
      const matching = tabs.find(tab => sameOriginOrUrl(tab.url || '', preferredUrl));
      if (matching) return [matching];
    }

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

function cdpConnect(tabId) {
  const ws = new WebSocket(`ws://localhost:29002/devtools/page/${tabId}`);
  let nextId = 1;
  const pending = new Map();
  ws.on('message', raw => {
    let msg;
    try { msg = JSON.parse(raw.toString()); } catch { return; }
    if (msg.id && pending.has(msg.id)) {
      const { resolve, reject } = pending.get(msg.id);
      pending.delete(msg.id);
      if (msg.error) reject(new Error(msg.error.message || 'CDP error'));
      else resolve(msg.result || {});
    }
  });
  const open = new Promise((resolve, reject) => {
    ws.once('open', resolve);
    ws.once('error', reject);
  });
  return {
    async send(method, params = {}) {
      await open;
      const id = nextId++;
      ws.send(JSON.stringify({ id, method, params }));
      return await new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
        setTimeout(() => {
          if (pending.has(id)) {
            pending.delete(id);
            reject(new Error(`CDP timeout for ${method}`));
          }
        }, 7000);
      });
    },
    close() { try { ws.close(); } catch {} }
  };
}

async function waitForPageInteractive(tabId, timeoutMs = 15000) {
  const cdp = cdpConnect(tabId);
  const deadline = Date.now() + timeoutMs;
  let lastState = null;
  try {
    while (Date.now() < deadline) {
      try {
        const result = await cdp.send('Runtime.evaluate', {
          expression: `(() => ({
            href: location.href,
            readyState: document.readyState,
            hasBody: !!document.body,
            textLength: (document.body?.innerText || '').length,
            actionableCount: document.querySelectorAll('button, a, [role="button"], [onclick], article, section').length
          }))()`,
          returnByValue: true
        });
        const value = result?.result?.value || result?.value;
        lastState = value || lastState;
        if (value?.hasBody && value.readyState !== 'loading' && (value.actionableCount > 0 || value.textLength > 20)) {
          return value;
        }
      } catch {
        // The page can briefly reject Runtime.evaluate while navigating or reloading.
      }
      await sleep(350);
    }
    return lastState || { readyState: 'unknown' };
  } finally {
    cdp.close();
  }
}

function humanPath(start, end, steps = 36) {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const dist = Math.max(1, Math.hypot(dx, dy));
  const nx = -dy / dist;
  const ny = dx / dist;
  const curve = Math.min(120, Math.max(28, dist * 0.18));
  const c1 = { x: start.x + dx * 0.35 + nx * curve, y: start.y + dy * 0.35 + ny * curve };
  const c2 = { x: start.x + dx * 0.72 - nx * curve * 0.55, y: start.y + dy * 0.72 - ny * curve * 0.55 };
  const points = [];
  for (let i = 1; i <= steps; i++) {
    const t = i / steps;
    const ease = t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
    const u = 1 - ease;
    points.push({
      x: u * u * u * start.x + 3 * u * u * ease * c1.x + 3 * u * ease * ease * c2.x + ease * ease * ease * end.x,
      y: u * u * u * start.y + 3 * u * u * ease * c1.y + 3 * u * ease * ease * c2.y + ease * ease * ease * end.y,
    });
  }
  return points;
}

async function ensureCdpCursor(cdp) {
  const expression = `(() => {
    const removeLegacyOverlays = () => {
      const selectors = [
        '#Sinew-agent-overlay-root',
        '#sinew-agent-overlay-root',
        '#codex-agent-overlay-root',
        '#Codex-agent-overlay-root',
        '[id$="-agent-overlay-root"]',
        '[id*="agent-overlay-root"]'
      ];
      for (const selector of selectors) {
        for (const node of document.querySelectorAll(selector)) {
          if (node.id !== 'sinew-cdp-human-cursor') node.remove();
        }
      }
    };

    removeLegacyOverlays();

    let root = window.__sinewCdpCursor;
    if (!root || !root.isConnected || root.id !== 'sinew-cdp-human-cursor') {
      document.querySelectorAll('#sinew-cdp-human-cursor').forEach(node => node.remove());
      root = document.createElement('div');
      root.id = 'sinew-cdp-human-cursor';
      root.style.cssText = 'position:fixed;left:0;top:0;width:28px;height:28px;z-index:2147483647;pointer-events:none;transform:translate3d(-100px,-100px,0);transition:none;filter:drop-shadow(0 0 6px rgba(255,107,0,.9)) drop-shadow(0 0 14px rgba(255,0,128,.55));will-change:transform,opacity;';
      root.innerHTML = '<svg width="28" height="28" viewBox="0 0 28 28" xmlns="http://www.w3.org/2000/svg"><path d="M4 2 L22 14 L14 16 L11 25 Z" fill="#fff" stroke="#111827" stroke-width="2"/><path d="M4 2 L22 14 L14 16 L11 25 Z" fill="none" stroke="#ff6b00" stroke-width="1" opacity=".85"/></svg>';
      document.documentElement.appendChild(root);
      window.__sinewCdpCursor = root;
    }

    window.__sinewCdpCursorMove = (x, y, scale = 1) => {
      window.__sinewCdpCursorPosition = { x: Math.round(x), y: Math.round(y) };
      root.style.opacity = '1';
      root.style.transform = 'translate3d(' + Math.round(x - 5) + 'px,' + Math.round(y - 4) + 'px,0) rotate(-12deg) scale(' + scale + ')';
    };
    window.__sinewCdpCursorPulse = (x, y) => {
      const pulse = document.createElement('div');
      pulse.style.cssText = 'position:fixed;left:' + (x - 7) + 'px;top:' + (y - 7) + 'px;width:14px;height:14px;border-radius:50%;border:2px solid #ff6b00;z-index:2147483646;pointer-events:none;box-shadow:0 0 12px #ff0080;animation:sinewCdpPulse .45s ease-out forwards;';
      if (!document.getElementById('sinew-cdp-pulse-style')) {
        const style = document.createElement('style');
        style.id = 'sinew-cdp-pulse-style';
        style.textContent = '@keyframes sinewCdpPulse{0%{transform:scale(.2);opacity:1}100%{transform:scale(4);opacity:0}}';
        document.documentElement.appendChild(style);
      }
      document.documentElement.appendChild(pulse);
      setTimeout(() => pulse.remove(), 650);
    };
    return true;
  })()`;
  await cdp.send('Runtime.evaluate', { expression, returnByValue: true });
}

async function moveCdpCursor(cdp, x, y, scale = 1) {
  await cdp.send('Runtime.evaluate', { expression: `window.__sinewCdpCursorMove && window.__sinewCdpCursorMove(${Math.round(x)}, ${Math.round(y)}, ${scale})`, returnByValue: true });
}

async function pulseCdpCursor(cdp, x, y) {
  await cdp.send('Runtime.evaluate', { expression: `window.__sinewCdpCursorPulse && window.__sinewCdpCursorPulse(${Math.round(x)}, ${Math.round(y)})`, returnByValue: true });
}

async function detectTargetViaCdp(tabId, taskText) {
  const cdp = cdpConnect(tabId);
  try {
    const taskLiteral = JSON.stringify(String(taskText || ''));
    const expression = `(() => {
      const task = ${taskLiteral};
      const taskText = task.toLowerCase();
      const elements = Array.from(document.querySelectorAll('button, a, input, select, textarea, [role="button"], [onclick], div, span, svg, li, summary, article, section'));
      const cleanTask = taskText.replace(/\\b(cliquez|clique|cliquer|click|ouvrir|ouvre|open|press|selectionne|sélectionne|va sur|aller|dans|sur|le|la|les|un|une|et|du|de|des|site|web|page|url|navigate|navigue|carte|bouton)\\b/g, ' ').trim();
      const queryWordsRaw = cleanTask.split(/\\s+/).filter(w => w.length >= 1);
      const semanticWords = [];
      if (queryWordsRaw.some(w => w === 'hamburger' || w === 'burger' || w === 'menu')) semanticWords.push('menu', 'hamburger', 'burger', 'nav', 'toggle');
      const queryWords = Array.from(new Set([...queryWordsRaw, ...semanticWords]));
      if (taskText.includes('trinity')) {
        const directTrinity = document.querySelector('[id*="trinity" i], [class*="trinity" i], [href*="trinity" i], [aria-label*="trinity" i], [title*="trinity" i]');
        if (directTrinity && typeof directTrinity.scrollIntoView === 'function') {
          directTrinity.scrollIntoView({ block: 'center', inline: 'center', behavior: 'auto' });
        }
      }
      let best = null;
      let bestScore = -1;
      for (const el of elements) {
        const rect = el.getBoundingClientRect();
        if (rect.width === 0 || rect.height === 0) continue;
        if (rect.width * rect.height > window.innerWidth * window.innerHeight * 0.55) continue;
        const style = window.getComputedStyle(el);
        if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') continue;
        const cx = rect.left + rect.width / 2;
        const cy = rect.top + rect.height / 2;
        if (cx < 0 || cy < 0 || cx > window.innerWidth || cy > window.innerHeight) continue;
        const topEl = document.elementFromPoint(cx, cy);
        if (topEl && topEl !== el && !el.contains(topEl) && !topEl.contains(el)) continue;
        const text = (el.innerText || el.textContent || '').toLowerCase().trim();
        const ariaLabel = (el.getAttribute('aria-label') || '').toLowerCase();
        const title = (el.getAttribute('title') || '').toLowerCase();
        const id = (el.id || '').toLowerCase();
        const className = (typeof el.className === 'string' ? el.className : '').toLowerCase();
        const href = (el.getAttribute('href') || '').toLowerCase();
        const role = (el.getAttribute('role') || '').toLowerCase();
        let score = 0;
        for (const word of queryWords) {
          if (text.includes(word)) score += 55;
          if (ariaLabel.includes(word)) score += 80;
          if (title.includes(word)) score += 55;
          if (id.includes(word)) score += 65;
          if (className.includes(word)) score += 45;
          if (href.includes(word)) score += 35;
        }
        if (el.tagName === 'BUTTON' || el.tagName === 'A' || role === 'button' || style.cursor === 'pointer') score += 25;
        if ((taskText.includes('hamburger') || taskText.includes('menu')) && (id.includes('menu') || ariaLabel.includes('menu') || className.includes('menu'))) score += 150;
        if (taskText.includes('trinity')) {
          const hasTrinitySignal = id.includes('trinity') || className.includes('trinity') || text.includes('trinity') || href.includes('trinity') || ariaLabel.includes('trinity') || title.includes('trinity');
          if (!hasTrinitySignal) continue;
          score += 320;
          if (id.includes('trinity')) score += 120;
          if (className.includes('trinity')) score += 100;
          if (el.tagName === 'ARTICLE' || className.includes('project-card')) score += 90;
          if (className.includes('social-icon')) score -= 400;
          if (rect.width * rect.height > 5000) score += 60;
        }
        if (score > bestScore && score > 0) {
          bestScore = score;
          best = { x: Math.round(cx), y: Math.round(cy), rect: { left: rect.left, top: rect.top, width: rect.width, height: rect.height }, element: { tagName: el.tagName, id: el.id || '', className: typeof el.className === 'string' ? el.className : '' }, score };
        }
      }
      return best ? { success: true, action: 'click', target: best } : { success: false, error: 'No target found' };
    })()`;
    const result = await cdp.send('Runtime.evaluate', { expression, returnByValue: true });
    return result?.result?.value || result?.value || { success: false, error: 'No CDP result' };
  } finally {
    cdp.close();
  }
}

async function getCdpCursorStart(cdp, tabId, target) {
  const saved = cursorStateByTabId.get(String(tabId));
  if (saved && Number.isFinite(saved.x) && Number.isFinite(saved.y)) {
    return saved;
  }

  const expression = `(() => {
    const pos = window.__sinewCdpCursorPosition;
    if (pos && Number.isFinite(pos.x) && Number.isFinite(pos.y)) return pos;
    return { x: Math.round(window.innerWidth * 0.5), y: Math.round(window.innerHeight * 0.5) };
  })()`;
  try {
    const result = await cdp.send('Runtime.evaluate', { expression, returnByValue: true });
    const value = result?.result?.value || result?.value;
    if (value && Number.isFinite(value.x) && Number.isFinite(value.y)) return value;
  } catch {}

  return { x: Math.max(24, target.x - 220), y: Math.max(24, target.y + 120) };
}

async function performHumanCdpClick(tabId, target) {
  if (!target || !Number.isFinite(target.x) || !Number.isFinite(target.y)) {
    throw new Error('Missing target coordinates for human CDP click');
  }
  const cdp = cdpConnect(tabId);
  try {
    await cdp.send('Page.bringToFront');
    await ensureCdpCursor(cdp);
    const start = await getCdpCursorStart(cdp, tabId, target);
    const end = { x: Math.round(target.x), y: Math.round(target.y) };
    for (const p of humanPath(start, end, 40)) {
      const x = Math.round(p.x);
      const y = Math.round(p.y);
      await moveCdpCursor(cdp, x, y, 1);
      await cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y, button: 'none' });
      await sleep(10 + Math.random() * 18);
    }
    await sleep(150 + Math.random() * 90);
    await moveCdpCursor(cdp, end.x, end.y, 0.94);
    await pulseCdpCursor(cdp, end.x, end.y);
    await cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: end.x, y: end.y, button: 'left', clickCount: 1 });
    await sleep(55 + Math.random() * 55);
    await cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: end.x, y: end.y, button: 'left', clickCount: 1 });
    cursorStateByTabId.set(String(tabId), end);
    await sleep(250);
    return { success: true, action: 'human_cdp_click', message: `Human CDP click at (${end.x}, ${end.y})` };
  } finally {
    cdp.close();
  }
}

async function executeAction(tabId, taskText, timeoutMs = 30000) {
  const deadline = Date.now() + timeoutMs;
  let lastDetection = null;

  await waitForPageInteractive(tabId, Math.min(10000, timeoutMs)).catch(() => null);

  while (Date.now() < deadline) {
    const detection = await detectTargetViaCdp(tabId, taskText);
    lastDetection = detection;
    if (detection?.target) {
      const performed = await performHumanCdpClick(tabId, detection.target);
      return { detection, performed };
    }
    await sleep(450);
  }

  return lastDetection || { success: false, error: 'No target found before timeout' };
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
          serverInfo: { name: 'Sinew Chrome', version: '1.0.0' }
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
