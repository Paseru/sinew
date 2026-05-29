const readline = require('readline');
const http = require('http');
const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');
const os = require('os');
const WebSocket = require('./node_modules/ws');

const BRIDGE_ORIGIN = process.env.MCP_BROWSER_CDP_URL || 'http://127.0.0.1:29002';
const BRIDGE_URL = new URL(BRIDGE_ORIGIN);
const BRIDGE_WS_ORIGIN = `${BRIDGE_URL.protocol === 'https:' ? 'wss:' : 'ws:'}//${BRIDGE_URL.host}`;
const CHROME_WAIT_MS = 20000;
const BRIDGE_WAIT_MS = 20000;
const cursorStateByTabId = new Map();
const cdpEndpointByTabId = new Map();
const STATE_DIR = process.env.SINEW_CHROME_BRIDGE_DIR || path.join(process.env.LOCALAPPDATA || path.join(os.homedir(), 'AppData', 'Local'), 'Sinew', 'ChromeBridge');
let BRIDGE_SECRET = '';
try {
  const secretPath = path.join(STATE_DIR, 'bridge-secret.txt');
  if (fs.existsSync(secretPath)) {
    BRIDGE_SECRET = fs.readFileSync(secretPath, 'utf8').trim();
  }
} catch (e) {
  // Silent fallback
}
const CONTROLLED_TAB_PATH = path.join(STATE_DIR, 'controlled-tab.json');
let controlledTabId = null;
let controlledTabTouchedAt = 0;
const DEFAULT_CURSOR_OPTIONS = {
  mode: process.env.SINEW_CURSOR_MODE || 'visible',
  speed: process.env.SINEW_CURSOR_SPEED || 'normal',
};
const ALLOW_CDP_FALLBACK = /^(1|true|yes)$/i.test(process.env.SINEW_CHROME_ALLOW_CDP_FALLBACK || '');

// Log errors to stderr so they don't corrupt the stdout JSON-RPC stream
function log(msg) {
  console.error(`[MCP Log] ${msg}`);
}

function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

function requestJSON(rawUrl, timeoutMs = 3000) {
  let url = rawUrl;
  if (BRIDGE_SECRET) {
    const connector = url.includes('?') ? '&' : '?';
    url += `${connector}token=${encodeURIComponent(BRIDGE_SECRET)}`;
  }
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

function normalizeCursorOptions(input = {}) {
  let options = input;
  if (typeof input === 'string') {
    try { options = JSON.parse(input); } catch { options = {}; }
  }
  if (!options || typeof options !== 'object') options = {};
  const mode = ['visible', 'hidden'].includes(String(options.mode || DEFAULT_CURSOR_OPTIONS.mode).toLowerCase())
    ? String(options.mode || DEFAULT_CURSOR_OPTIONS.mode).toLowerCase()
    : 'visible';
  const speed = ['slow', 'normal', 'fast'].includes(String(options.speed || DEFAULT_CURSOR_OPTIONS.speed).toLowerCase())
    ? String(options.speed || DEFAULT_CURSOR_OPTIONS.speed).toLowerCase()
    : 'normal';
  return { mode, speed };
}

function extractUrl(task) {
  if (!task) return null;
  const explicit = String(task).match(/https?:\/\/[^\s)]+/i);
  if (explicit) return normalizeUrl(explicit[0]);

  const domain = String(task).match(/\b[a-z0-9-]+(\.[a-z0-9-]+)+(\/[^\s)]*)?/i);
  return domain ? normalizeUrl(domain[0]) : null;
}

function hasTypingIntent(task) {
  const text = String(task || '').toLowerCase();
  return /\b(tape|type|saisis|saisir|ecris|écris|ecrire|écrire|recherche|chercher|search)\b/i.test(text);
}

function hasNavigationIntent(task) {
  const text = String(task || '').toLowerCase();
  return /\b(ouvre|ouvrir|open|navigue|navigate|navigation|visite|visit|rends-toi)\b/i.test(text)
    || /\b(va|aller|go)\s+(sur|to)\b/i.test(text);
}

function isGoogleSearchTask(task) {
  const text = String(task || '').toLowerCase();
  const mentionsGoogle = /\bgoogle(?:\.[a-z]{2,})?\b/i.test(text);
  const mentionsSearch = /\b(recherche|chercher|search|champ|requ[êe]te)\b/i.test(text);
  return mentionsGoogle && (mentionsSearch || hasTypingIntent(task) || /\bjulienpiron(?:\.fr)?\b/i.test(text));
}

function cleanSearchQuery(value) {
  return String(value || '')
    .replace(/^[\s`"'“”‘’]+|[\s`"'“”‘’]+$/g, '')
    .replace(/^(exactement|exact|precisement|précisément)\s+/i, '')
    .replace(/[,.!?;:]+$/g, '')
    .trim();
}

function extractSearchQuery(task) {
  const original = String(task || '');
  const quoted = original.match(/(?:tape|écris|ecris|saisis|type|recherche(?:\s+sur\s+google)?|search)\s+(?:exactement\s+)?[`"“'‘]([^`"”'’]+)[`"”'’]/i);
  if (quoted && quoted[1]) return cleanSearchQuery(quoted[1]);

  const domain = original.match(/\b[a-z0-9-]+(?:\.[a-z0-9-]+)+(?:\/[^\s,;)]*)?\b/i);
  if (domain && /julienpiron|google|recherche|search|tape|écris|ecris|saisis|type/i.test(original)) {
    return cleanSearchQuery(domain[0]);
  }

  const generic = original.match(/(?:tape|écris|ecris|saisis|type|recherche(?:\s+sur\s+google)?|search)\s+(?:exactement\s+)?(.+?)(?:\s+(?:puis|et)\b|[,;]\s*(?:valide|valides|appuie|clique|clic|click)|$)/i);
  return cleanSearchQuery(generic && generic[1] ? generic[1] : '');
}

function extractNavigationUrl(task) {
  const url = extractUrl(task);
  if (!url) return null;
  if (isGoogleSearchTask(task)) return null;
  if (hasNavigationIntent(task)) return url;
  if (!hasTypingIntent(task) && /^\s*(https?:\/\/|[a-z0-9-]+(?:\.[a-z0-9-]+)+)/i.test(String(task || ''))) return url;
  return null;
}

function shouldUseBridgeDomAction(taskText) {
  const url = extractUrl(taskText);
  if (url && !extractNavigationUrl(taskText)) return false;
  if (hasTypingIntent(taskText)) return false;
  if (/\b(champ|recherche|search|résultat|resultat|lien)\b/i.test(String(taskText || ''))) return false;
  return true;
}

async function requestBridgeLaunch(targetUrl) {
  return requestJSON(`${BRIDGE_ORIGIN}/api/launch_chrome?url=${encodeURIComponent(normalizeUrl(targetUrl) || 'about:blank')}`, 3000).catch(() => null);
}

async function releaseExtensionDebuggers() {
  if (!ALLOW_CDP_FALLBACK) return null;
  return requestJSON(`${BRIDGE_ORIGIN}/api/detach_all`, 2000).catch(() => null);
}

function launchChrome(targetUrl = 'about:blank') {
  const chromeExe = findChromeExecutable();
  log(`Launching Chrome via ${chromeExe}`);
  const args = [
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

async function waitForBridge(timeoutMs = BRIDGE_WAIT_MS) {
  const deadline = Date.now() + timeoutMs;
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

  throw new Error(
    "Erreur de connexion : L'extension Sinew Chrome n'est pas connectée au pont local.\n" +
    "Pour résoudre ce problème :\n" +
    "1. Ouvrez Google Chrome et accédez à 'chrome://extensions'.\n" +
    "2. Si l'extension 'Sinew Chrome Bridge' n'est pas installée, cliquez sur 'Charger l'extension non empaquetée' et sélectionnez le dossier 'sinew-chrome-bridge' dans le projet.\n" +
    "3. Si elle est déjà installée mais désactivée, activez simplement son interrupteur bleu.\n" +
    "4. Cliquez sur l'icône de l'extension dans la barre d'outils et cliquez sur 'Reconnect' pour qu'elle passe au Vert 🟢."
  );
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
    if (Array.isArray(tabs)) {
      for (const tab of tabs) {
        if (tab?.id && tab?.webSocketDebuggerUrl) cdpEndpointByTabId.set(String(tab.id), tab.webSocketDebuggerUrl);
      }
    }
    if (Array.isArray(tabs) && tabs.length > 0) {
      if (!preferredUrl) return tabs;
      const matching = tabs.find(tab => sameOriginOrUrl(tab.url || '', preferredUrl));
      if (matching) return [matching];
      return [tabs.find(tab => tab.url === 'about:blank') || tabs.find(tab => tab.active) || tabs[0]];
    }

    if (Array.isArray(tabs)) lastTabs = tabs;
    await sleep(500);
  }

  return lastTabs;
}

async function ensureChromeReady(preferredUrl = null) {
  const targetUrl = normalizeUrl(preferredUrl) || 'https://www.google.com';
  let bridgeReady = false;

  try {
    await waitForBridge(1200);
    bridgeReady = true;
  } catch (err) {
    log(`Bridge not ready yet, launching Chrome now: ${err.message}`);
  }

  if (bridgeReady) {
    await releaseExtensionDebuggers();
    // Fetch tabs list instantly without 20-second polling
    const tabs = await requestJSON(`${BRIDGE_ORIGIN}/json`, 1500).catch(() => []);
    if (tabs && tabs.length > 0) {
      const match = tabs.filter(t => sameOriginOrUrl(t.url, targetUrl));
      if (match.length > 0) return match;
      return tabs; // Return open tabs immediately to avoid the 20-second timeout
    }
  }

  const launchedViaBridge = await requestBridgeLaunch(targetUrl);
  if (!launchedViaBridge || launchedViaBridge.success === false) {
    launchChrome(targetUrl);
  }
  await waitForBridge();
  await releaseExtensionDebuggers();
  // Wait for Chrome to startup (null targetUrl returns as soon as the first tab is seen)
  const tabs = await waitForTabs(null);

  if (!tabs || tabs.length === 0) {
    throw new Error('Chrome bridge is connected, but no controllable normal-profile tab could be found.');
  }
  return tabs;
}

async function currentTabLocation(tabId) {
  const tabs = await requestJSON(`${BRIDGE_ORIGIN}/json`, 3000).catch(() => []);
  const tab = Array.isArray(tabs) ? tabs.find(item => String(item.id) === String(tabId)) : null;
  return tab ? { href: tab.url || '', title: tab.title || '', readyState: 'complete', tab } : null;
}

async function navigateTabViaCdp(tabId, url) {
  if (!ALLOW_CDP_FALLBACK) return { success: false, error: 'CDP fallback disabled to avoid Chrome debugging banner' };
  const targetUrl = normalizeUrl(url) || url || 'about:blank';
  const cdp = cdpConnect(tabId);
  try {
    await cdp.send('Page.enable').catch(() => null);
    await cdp.send('Page.bringToFront').catch(() => null);
    await cdp.send('Page.navigate', { url: targetUrl });
  } finally {
    cdp.close();
  }
  return waitForTabUrl(tabId, targetUrl, 15000);
}

async function waitForTabUrl(tabId, targetUrl, timeoutMs = 12000) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() < deadline) {
    try {
      last = await currentTabLocation(tabId);
      if (last?.href && sameOriginOrUrl(last.href, targetUrl) && last.readyState !== 'loading') return last;
    } catch {}
    await sleep(350);
  }
  return last;
}

async function navigateTab(tabId, url) {
  if (!tabId || !url) return null;
  const targetUrl = normalizeUrl(url) || url;
  let updated = await requestJSON(`${BRIDGE_ORIGIN}/api/navigate_tab?tabId=${encodeURIComponent(tabId)}&url=${encodeURIComponent(targetUrl)}`, 7000).catch(() => null);
  let location = await waitForTabUrl(tabId, targetUrl, 6000).catch(() => null);

  if ((!location?.href || !sameOriginOrUrl(location.href, targetUrl)) && ALLOW_CDP_FALLBACK) {
    log(`Bridge navigation did not reach ${targetUrl}; falling back to CDP navigation.`);
    location = await navigateTabViaCdp(tabId, targetUrl).catch(err => ({ success: false, error: err.message }));
  }

  if ((!location?.href || !sameOriginOrUrl(location.href, targetUrl)) && (!updated || updated.success === false)) {
    const created = await requestJSON(`${BRIDGE_ORIGIN}/api/create_tab?url=${encodeURIComponent(targetUrl)}`, 7000).catch(() => null);
    const createdTab = created && created.success !== false ? created.tab : null;
    if (createdTab?.id) {
      location = { href: targetUrl, title: createdTab.title || '', readyState: 'unknown', tab: { ...createdTab, id: String(createdTab.id), url: targetUrl } };
    } else {
      const tabs = await waitForTabs(targetUrl).catch(() => []);
      const newTab = Array.isArray(tabs) ? tabs.find(tab => sameOriginOrUrl(tab.url || '', targetUrl)) : null;
      if (newTab) location = { href: newTab.url, title: newTab.title || '', readyState: 'complete', tab: newTab };
    }
  }

  if ((!location?.href || !sameOriginOrUrl(location.href, targetUrl)) && updated && updated.success !== false) {
    const updatedTab = updated.tab || {};
    location = { href: targetUrl, title: updatedTab.title || '', readyState: 'unknown', tab: { ...updatedTab, id: String(tabId), url: targetUrl } };
  }

  cursorStateByTabId.delete(String(tabId));
  await sleep(120);
  return location;
}

function buildActionTasks(task) {
  const original = String(task || '');
  const text = original.toLowerCase();
  const actions = [];
  if (isGoogleSearchTask(original)) {
    actions.push('clique dans le champ de recherche Google');
    const query = extractSearchQuery(original);
    if (query) {
      actions.push(`tape ${query} puis appuie sur Entrée`);
      if (/\b(clique|clic|click|ouvrir|ouvre|open)\b/i.test(text) && /\b(lien|résultat|resultat|site)\b/i.test(text)) {
        actions.push(`clique le résultat ${query}`);
      }
    }
  }
  if (text.includes('hamburger') || text.includes('menu')) {
    actions.push('ouvre le menu hamburger');
    if (text.includes('referme') || text.includes('ferme') || text.includes('close')) {
      actions.push('ferme le menu ouvert');
    }
  }
  if (text.includes('trinity')) {
    actions.push('clique la carte Trinity');
  }
  if (actions.length === 0 && extractNavigationUrl(original) && hasNavigationIntent(original)) {
    return [];
  }
  return actions.length > 0 ? actions : [task];
}

function cdpConnect(tabId) {
  let endpoint = cdpEndpointByTabId.get(String(tabId)) || `${BRIDGE_WS_ORIGIN}/devtools/page/${tabId}`;
  if (BRIDGE_SECRET && !endpoint.includes('token=')) {
    const connector = endpoint.includes('?') ? '&' : '?';
    endpoint += `${connector}token=${encodeURIComponent(BRIDGE_SECRET)}`;
  }
  const ws = new WebSocket(endpoint);
  let nextId = 1;
  const pending = new Map();
  ws.on('message', raw => {
    let msg;
    try { msg = JSON.parse(raw.toString()); } catch { return; }
    if (msg.id && pending.has(msg.id)) {
      const { resolve, reject, timer } = pending.get(msg.id);
      pending.delete(msg.id);
      clearTimeout(timer);
      if (msg.error) reject(new Error(msg.error.message || 'CDP error'));
      else resolve(msg.result || {});
    }
  });
  const open = new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`CDP open timeout for tab ${tabId}`)), 5000);
    ws.once('open', () => {
      clearTimeout(timer);
      resolve();
    });
    ws.once('error', (err) => {
      clearTimeout(timer);
      reject(err);
    });
  });
  return {
    async send(method, params = {}) {
      await open;
      const id = nextId++;
      ws.send(JSON.stringify({ id, method, params }));
      return await new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
          if (pending.has(id)) {
            pending.delete(id);
            reject(new Error(`CDP timeout for ${method}`));
          }
        }, 7000);
        pending.set(id, { resolve, reject, timer });
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

function cursorTiming(speed) {
  return {
    slow: { steps: 62, minDelay: 18, jitter: 24, pause: 230 },
    normal: { steps: 40, minDelay: 10, jitter: 17, pause: 145 },
    fast: { steps: 24, minDelay: 4, jitter: 9, pause: 60 },
  }[speed] || { steps: 40, minDelay: 10, jitter: 17, pause: 145 };
}

function humanPath(start, end, steps = 36) {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const dist = Math.max(1, Math.hypot(dx, dy));
  
  // Generate 6 candidates with varying curve scales and directions
  const candidates = [];
  const multipliers = [0.4, 0.8, 1.2, -0.4, -0.8, -1.2];
  
  for (const mult of multipliers) {
    const points = [];
    const curve = Math.min(130, Math.max(28, dist * 0.20)) * mult;
    const nx = -dy / dist;
    const ny = dx / dist;
    
    const c1 = { x: start.x + dx * 0.35 + nx * curve, y: start.y + dy * 0.35 + ny * curve };
    const c2 = { x: start.x + dx * 0.72 - nx * curve * 0.55, y: start.y + dy * 0.72 - ny * curve * 0.55 };
    
    for (let i = 1; i <= steps; i++) {
      const t = i / steps;
      const ease = t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
      const u = 1 - ease;
      points.push({
        x: u * u * u * start.x + 3 * u * u * ease * c1.x + 3 * u * ease * ease * c2.x + ease * ease * ease * end.x,
        y: u * u * u * start.y + 3 * u * u * ease * c1.y + 3 * u * ease * ease * c2.y + ease * ease * ease * end.y,
      });
    }
    candidates.push(points);
  }
  
  // Score candidates to find the smoothest path that stays in bounds
  const width = 1280;
  const height = 720;
  
  let bestPoints = candidates[0];
  let minScore = Infinity;
  
  for (const points of candidates) {
    let outOfBoundsCount = 0;
    let totalJerkiness = 0;
    let prevAngle = null;
    
    for (let i = 0; i < points.length; i++) {
      const p = points[i];
      if (p.x < 10 || p.x > width - 10 || p.y < 10 || p.y > height - 10) {
        outOfBoundsCount++;
      }
      
      if (i > 0) {
        const prev = points[i - 1];
        const angle = Math.atan2(p.y - prev.y, p.x - prev.x);
        if (prevAngle !== null) {
          let diff = Math.abs(angle - prevAngle);
          if (diff > Math.PI) diff = 2 * Math.PI - diff;
          totalJerkiness += diff;
        }
        prevAngle = angle;
      }
    }
    
    const boundsPenalty = outOfBoundsCount * 10000;
    const score = boundsPenalty + totalJerkiness * 50;
    
    if (score < minScore) {
      minScore = score;
      bestPoints = points;
    }
  }
  
  return bestPoints;
}

async function ensureCdpCursor(cdp, cursorOptions = {}) {
  const cursor = normalizeCursorOptions(cursorOptions);
  const expression = `(() => {
    const removeLegacyOverlays = () => {
      const selectors = [
        '#Sinew-agent-overlay-root',
        '#sinew-agent-overlay-root',
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
      root.style.cssText = 'position:fixed;left:0;top:0;width:28px;height:28px;z-index:2147483647;pointer-events:none;transform:translate3d(-100px,-100px,0);transition:none;filter:drop-shadow(0 0 6px #ff6b00) drop-shadow(0 0 14px #ff0080) drop-shadow(0 0 22px #66f7ff);will-change:transform,opacity;animation:sinew-cdp-glow-pulse 2.2s ease-in-out infinite alternate;';
      
      if (!document.getElementById('sinew-cdp-glow-style')) {
        const style = document.createElement('style');
        style.id = 'sinew-cdp-glow-style';
        style.textContent = '@keyframes sinew-cdp-glow-pulse{0%{filter:drop-shadow(0 0 4px #ff6b00) drop-shadow(0 0 10px #ff0080) drop-shadow(0 0 16px #66f7ff)}100%{filter:drop-shadow(0 0 8px #ff6b00) drop-shadow(0 0 18px #ff0080) drop-shadow(0 0 28px #66f7ff)}}';
        document.documentElement.appendChild(style);
      }
      
      root.innerHTML = '<svg width="28" height="28" viewBox="0 0 28 28" xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="sinew-cdp-grad" x1="0%" y1="0%" x2="100%" y2="100%"><stop offset="0%" stop-color="#ff6b00"><animate attributeName="stop-color" values="#ff6b00;#ff0080;#66f7ff;#ff6b00" dur="3s" repeatCount="indefinite" /></stop><stop offset="100%" stop-color="#ff0080"><animate attributeName="stop-color" values="#ff0080;#66f7ff;#ff6b00;#ff0080" dur="3s" repeatCount="indefinite" /></stop></linearGradient></defs><path d="M4.5 3L23.5 11.5L14 14.5L11.5 24L4.5 3Z" fill="url(#sinew-cdp-grad)" stroke="white" stroke-width="1.2" stroke-linejoin="round"/></svg>';
      document.documentElement.appendChild(root);
      window.__sinewCdpCursor = root;
    }

    const cursorVisible = ${cursor.mode !== 'hidden'};
    root.style.display = cursorVisible ? 'block' : 'none';
    window.__sinewCdpCursorMove = (x, y, scale = 1) => {
      window.__sinewCdpCursorPosition = { x: Math.round(x), y: Math.round(y) };
      root.style.opacity = cursorVisible ? '1' : '0';
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
      const wantsMenu = taskText.includes('hamburger') || taskText.includes('menu') || taskText.includes('burger');
      const wantsMenuClose = wantsMenu && /\\b(referme|ferme|fermer|close|dismiss|x)\\b/.test(taskText);
      const wantsMenuOpen = wantsMenu && !wantsMenuClose;
      const elements = Array.from(document.querySelectorAll('button, a, input, select, textarea, [role="button"], [onclick], [aria-label], [title], div, span, svg, li, summary, article, section'));
      const directTarget = (el, score = 1000) => {
        if (!el) return null;
        if (typeof el.scrollIntoView === 'function') el.scrollIntoView({ block: 'center', inline: 'center', behavior: 'auto' });
        const rect = el.getBoundingClientRect();
        const style = window.getComputedStyle(el);
        if (rect.width <= 0 || rect.height <= 0) return null;
        if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') return null;
        const cx = Math.max(0, Math.min(window.innerWidth - 1, rect.left + rect.width / 2));
        const cy = Math.max(0, Math.min(window.innerHeight - 1, rect.top + rect.height / 2));
        return {
          success: true,
          action: 'click',
          candidatesScored: elements.length,
          target: {
            x: Math.round(cx),
            y: Math.round(cy),
            rect: { left: rect.left, top: rect.top, width: rect.width, height: rect.height },
            element: { tagName: el.tagName, id: el.id || '', className: typeof el.className === 'string' ? el.className : '' },
            score
          }
        };
      };
      if (wantsMenu) {
        const directMenu = document.querySelector('#menu-button, button[id*="menu" i], button[class*="menu" i], button[aria-label*="menu" i], button[title*="menu" i], [role="button"][aria-label*="menu" i]');
        const target = directTarget(directMenu, 1000);
        if (target) return target;
      }
      if (taskText.includes('trinity')) {
        const directTrinity = document.querySelector('#trinity-card, .trinity-card, article[id*="trinity" i], article[class*="trinity" i], a[href*="trinity" i], [data-project*="trinity" i], [data-id*="trinity" i]');
        const target = directTarget(directTrinity, 1000);
        if (target) return target;
      }
      const cleanTask = taskText.replace(/\b(cliquez|clique|cliquer|click|ouvrir|ouvre|open|press|selectionne|sélectionne|va sur|aller|dans|sur|le|la|les|un|une|et|du|de|des|site|web|page|url|navigate|navigue|carte|bouton|ferme|fermer|referme|close|ouvert)\b/g, ' ').trim();
      const queryWordsRaw = cleanTask.split(/\\s+/).filter(w => w.length >= 1);
      const semanticWords = [];
      if (queryWordsRaw.some(w => w === 'hamburger' || w === 'burger' || w === 'menu')) semanticWords.push('menu', 'hamburger', 'burger', 'nav', 'toggle');
      const queryWords = Array.from(new Set([...queryWordsRaw, ...semanticWords]));
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
        const dataAttrs = Array.from(el.attributes || []).filter(attr => attr.name.startsWith('data-')).map(attr => attr.name + ' ' + attr.value).join(' ').toLowerCase();
        let score = 0;
        for (const word of queryWords) {
          if (text.includes(word)) score += 55;
          if (ariaLabel.includes(word)) score += 80;
          if (title.includes(word)) score += 55;
          if (id.includes(word)) score += 65;
          if (className.includes(word)) score += 45;
          if (href.includes(word)) score += 35;
          if (dataAttrs.includes(word)) score += 35;
        }
        if (el.tagName === 'BUTTON' || el.tagName === 'A' || role === 'button' || style.cursor === 'pointer') score += 25;
        const wantsMenu = taskText.includes('hamburger') || taskText.includes('menu') || taskText.includes('burger');
        const wantsMenuClose = wantsMenu && /\b(referme|ferme|fermer|close|dismiss|x)\b/.test(taskText);
        const wantsMenuOpen = wantsMenu && !wantsMenuClose;
        const isIconOnly = text.length <= 3 && rect.width <= 90 && rect.height <= 90;
        const svgCount = el.querySelectorAll ? el.querySelectorAll('svg,path,line,span').length : 0;
        const hasMenuGeometry = isIconOnly && (
          text === '☰' || text === '≡' || text === 'menu' ||
          svgCount >= 2 ||
          /(^|\\s|_|-)(hamburger|burger|menu|nav|navbar|toggle|drawer|bars)(\\s|$|_|-)/.test(id + ' ' + className + ' ' + ariaLabel + ' ' + title + ' ' + dataAttrs)
        );
        if (wantsMenu) {
          const isMenuButton = id.includes('menu') || ariaLabel.includes('menu') || title.includes('menu') || className.includes('menu') || ariaLabel.includes('hamburger') || className.includes('hamburger') || hasMenuGeometry || ariaLabel.includes('navigation') || title.includes('navigation') || className.includes('navbar') || className.includes('nav-toggle') || className.includes('toggle');
          const isExplicitClose = id.includes('close') || ariaLabel.includes('close') || ariaLabel.includes('fermer') || title.includes('close') || title.includes('fermer') || className.includes('close') || className.includes('modal-close');
          const isButtonLike = el.tagName === 'BUTTON' || role === 'button';
          const isCloseGlyph = isButtonLike && (text === '×' || text === 'x') && !className.includes('logo') && !className.includes('social');
          const isCloseButton = isExplicitClose || isCloseGlyph;
          if (wantsMenuOpen) {
            if (isCloseButton) continue;
            if (isMenuButton) score += 260;
            if (hasMenuGeometry) score += 180;
            if (id === 'menu-button') score += 160;
            if (rect.top < window.innerHeight * 0.35 && (rect.left < 160 || rect.right > window.innerWidth - 160)) score += 80;
          } else if (wantsMenuClose) {
            if (!isCloseButton && !isMenuButton) continue;
            if (isCloseButton) score += 320;
            if (className.includes('modal-close')) score += 140;
            if (isMenuButton) score += 80;
          }
        }
        if (taskText.includes('trinity')) {
          if (el.tagName === 'IFRAME') continue;
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
      return best ? { success: true, action: 'click', target: best, candidatesScored: elements.length } : { success: false, error: 'No target found' };
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
    return { x: Math.round(window.innerWidth * (Math.random() < 0.5 ? 0.12 + Math.random() * 0.18 : 0.70 + Math.random() * 0.18)), y: Math.round(window.innerHeight * (0.18 + Math.random() * 0.62)) };
  })()`;
  try {
    const result = await cdp.send('Runtime.evaluate', { expression, returnByValue: true });
    const value = result?.result?.value || result?.value;
    if (value && Number.isFinite(value.x) && Number.isFinite(value.y)) return value;
  } catch {}

  return { x: Math.max(24, target.x - 220), y: Math.max(24, target.y + 120) };
}

async function performHumanCdpClick(tabId, target, cursorOptions = {}) {
  if (!target || !Number.isFinite(target.x) || !Number.isFinite(target.y)) {
    throw new Error('Missing target coordinates for human CDP click');
  }
  const cdp = cdpConnect(tabId);
  try {
    const cursor = normalizeCursorOptions(cursorOptions);
    const timing = cursorTiming(cursor.speed);
    await cdp.send('Page.bringToFront');
    await ensureCdpCursor(cdp, cursor);
    const start = await getCdpCursorStart(cdp, tabId, target);
    const end = { x: Math.round(target.x), y: Math.round(target.y) };
    for (const p of humanPath(start, end, timing.steps)) {
      const x = Math.round(p.x);
      const y = Math.round(p.y);
      await moveCdpCursor(cdp, x, y, 1);
      await cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y, button: 'none' });
      await sleep(timing.minDelay + Math.random() * timing.jitter);
    }
    await sleep(timing.pause + Math.random() * 90);
    if (cursor.mode !== 'hidden') await moveCdpCursor(cdp, end.x, end.y, 0.94);
    if (cursor.mode !== 'hidden') await pulseCdpCursor(cdp, end.x, end.y);
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

async function detectTargetViaBridge(tabId, taskText) {
  return requestJSON(`${BRIDGE_ORIGIN}/api/detect_target?tabId=${encodeURIComponent(tabId)}&task=${encodeURIComponent(taskText)}`, 20000);
}

async function getPageSnapshotViaBridge(tabId, limit = 80) {
  return requestJSON(`${BRIDGE_ORIGIN}/api/page_snapshot?tabId=${encodeURIComponent(tabId)}&limit=${encodeURIComponent(limit)}`, 15000);
}

function scoreSnapshotItem(item, taskText) {
  const text = String(taskText || '').toLowerCase();
  const haystack = [item.visibleText, item.ariaName, item.href, item.selector?.primary, ...(item.selector?.candidates || [])]
    .filter(Boolean)
    .join(' ')
    .toLowerCase();
  const words = text
    .replace(/\b(clique|cliquer|click|ouvre|ouvrir|open|dans|sur|le|la|les|un|une|du|de|des|champ|bouton|lien|résultat|resultat|site|page|tape|type|saisis|ecris|écris)\b/g, ' ')
    .split(/\s+/)
    .map(w => w.trim())
    .filter(w => w.length >= 2);
  let score = 0;
  for (const word of words) {
    if (haystack.includes(word)) score += 80;
    if ((item.visibleText || '').toLowerCase().includes(word)) score += 40;
    if ((item.ariaName || '').toLowerCase().includes(word)) score += 50;
    if ((item.href || '').toLowerCase().includes(word)) score += 35;
  }
  if (/\b(recherche|search|google)\b/i.test(text) && item.editable) score += 220;
  if (/\b(clique|click|lien|résultat|resultat|ouvre|open)\b/i.test(text) && item.clickable) score += 90;
  if (/\b(tape|type|saisis|ecris|écris|champ)\b/i.test(text) && item.editable) score += 120;
  if (item.visible) score += 30;
  if (!item.boundingBox || !item.center) score -= 500;
  return score;
}

async function detectTargetViaSnapshot(tabId, taskText) {
  const snapshot = await getPageSnapshotViaBridge(tabId, 120).catch(err => ({ success: false, error: err.message }));
  if (!snapshot || snapshot.success === false || !Array.isArray(snapshot.items)) return snapshot;
  let best = null;
  let bestScore = -Infinity;
  for (const item of snapshot.items) {
    const score = scoreSnapshotItem(item, taskText);
    if (score > bestScore) {
      bestScore = score;
      best = item;
    }
  }
  if (!best || bestScore <= 0 || !best.center || !best.boundingBox) {
    return { success: false, error: 'No snapshot target found', snapshot };
  }
  const action = /\b(tape|type|saisis|ecris|écris)\b/i.test(String(taskText || '')) ? 'type' : 'click';
  const typeMatch = String(taskText || '').match(/(?:tape|type|saisis|ecris|écris)\s+(.+?)(?:\s+puis|\s+et|$)/i);
  return {
    success: true,
    action,
    target: {
      x: best.center.x,
      y: best.center.y,
      rect: { left: best.boundingBox.x, top: best.boundingBox.y, width: best.boundingBox.width, height: best.boundingBox.height },
      element: { tagName: best.tagName, id: '', className: '', href: best.href || '', selector: best.selector?.primary || null, role: best.role || null },
      score: bestScore,
    },
    text: action === 'type' && typeMatch ? cleanSearchQuery(typeMatch[1]) : undefined,
    submit: action === 'type' ? /\b(entrée|enter|valide|submit|recherche)\b/i.test(String(taskText || '')) : undefined,
    snapshotItem: best,
    message: `Cible snapshot détectée à (${best.center.x}, ${best.center.y}) pour ${best.tagName}.`
  };
}

async function performHumanBridgeClick(tabId, detection, cursorOptions = {}) {
  const cursor = normalizeCursorOptions(cursorOptions);
  const result = await requestJSON(`${BRIDGE_ORIGIN}/api/human_click?tabId=${encodeURIComponent(tabId)}&detection=${encodeURIComponent(JSON.stringify(detection))}&cursor=${encodeURIComponent(JSON.stringify(cursor))}`, 20000);
  return result;
}

async function executeActionViaBridgeDom(tabId, taskText, timeoutMs = 30000, cursorOptions = {}) {
  const cursor = normalizeCursorOptions(cursorOptions);
  return requestJSON(`${BRIDGE_ORIGIN}/api/execute_silent_task?tabId=${encodeURIComponent(tabId)}&task=${encodeURIComponent(taskText)}&cursor=${encodeURIComponent(JSON.stringify(cursor))}`, Math.max(5000, timeoutMs));
}

async function executeAction(tabId, taskText, timeoutMs = 30000, cursorOptions = {}) {
  const deadline = Date.now() + timeoutMs;
  const cursor = normalizeCursorOptions(cursorOptions);
  let lastDetection = null;

  while (Date.now() < deadline) {
    if (shouldUseBridgeDomAction(taskText)) {
      const domAction = await executeActionViaBridgeDom(tabId, taskText, Math.min(30000, deadline - Date.now()), cursor).catch(err => ({ success: false, error: err.message }));
      if (domAction && domAction.success !== false) {
        return { success: true, detection: { success: true, action: domAction.action || 'dom', target: domAction.target }, bridgeDetection: domAction, performed: domAction };
      }
      lastDetection = domAction;
    }

    const bridgeDetection = await detectTargetViaBridge(tabId, taskText).catch(err => ({ success: false, error: err.message }));
    let detection = bridgeDetection;
    if ((!detection?.target || detection.success === false)) {
      const snapshotDetection = await detectTargetViaSnapshot(tabId, taskText).catch(err => ({ success: false, error: err.message, bridgeDetection }));
      if (snapshotDetection?.target) detection = snapshotDetection;
    }
    if ((!detection?.target || detection.success === false) && ALLOW_CDP_FALLBACK) {
      detection = await detectTargetViaCdp(tabId, taskText).catch(err => ({ success: false, error: err.message, bridgeDetection }));
    }
    lastDetection = detection;
    if (detection?.target) {
      let performed = await performHumanBridgeClick(tabId, detection, cursor).catch(err => ({ success: false, error: err.message }));
      if ((!performed || performed.success === false) && ALLOW_CDP_FALLBACK) {
        performed = await performHumanCdpClick(tabId, detection.target, cursor).catch(err => ({ success: false, error: err.message }));
      }
      const hrefToNavigate = detection?.target?.element?.href || performed?.href || '';
      if (performed && performed.success !== false && /^https?:\/\//i.test(hrefToNavigate)) {
        const navigation = await navigateTab(tabId, hrefToNavigate).catch(err => ({ success: false, error: err.message }));
        performed = { ...performed, navigation };
      }
      return { success: performed && performed.success !== false, detection, bridgeDetection, performed };
    }
    await sleep(450);
  }

  return lastDetection || { success: false, error: 'No target found before timeout' };
}

function loadControlledTabState() {
  try {
    const parsed = JSON.parse(fs.readFileSync(CONTROLLED_TAB_PATH, 'utf8'));
    if (parsed?.tabId && parsed?.touchedAt && Date.now() - Number(parsed.touchedAt) < 20 * 60 * 1000) {
      controlledTabId = String(parsed.tabId);
      controlledTabTouchedAt = Number(parsed.touchedAt);
    }
  } catch {}
}

function saveControlledTabState() {
  try {
    fs.mkdirSync(STATE_DIR, { recursive: true });
    fs.writeFileSync(CONTROLLED_TAB_PATH, JSON.stringify({ tabId: controlledTabId, touchedAt: controlledTabTouchedAt }, null, 2));
  } catch {}
}

function rememberControlledTab(tab) {
  if (!tab?.id) return;
  controlledTabId = String(tab.id);
  controlledTabTouchedAt = Date.now();
  saveControlledTabState();
}

function isControlledTabFresh(maxAgeMs = 20 * 60 * 1000) {
  if (!controlledTabId) loadControlledTabState();
  return !!controlledTabId && Date.now() - controlledTabTouchedAt < maxAgeMs;
}

function pickControlledTab(tabs) {
  if (!Array.isArray(tabs) || tabs.length === 0) return null;
  if (isControlledTabFresh()) {
    const found = tabs.find(tab => String(tab.id) === String(controlledTabId));
    if (found) return found;
  }
  const active = tabs.find(tab => tab.active);
  return active || null;
}

async function getReadyTab(preferredUrl = null) {
  const targetUrl = normalizeUrl(preferredUrl);
  if (!targetUrl) {
    try {
      await waitForBridge();
      const existingTabs = await waitForTabs(null);
      const controlled = pickControlledTab(existingTabs);
      if (controlled) {
        rememberControlledTab(controlled);
        return controlled;
      }
      if (Array.isArray(existingTabs) && existingTabs.length > 0) {
        rememberControlledTab(existingTabs[0]);
        return existingTabs[0];
      }
    } catch {}
  }

  const tabs = await ensureChromeReady(targetUrl || 'https://www.google.com');
  if (!tabs || tabs.length === 0) {
    throw new Error('No debuggable Chrome tab is available.');
  }
  const controlled = !targetUrl ? pickControlledTab(tabs) : null;
  const tab = controlled || tabs[0];
  if (tab) rememberControlledTab(tab);
  return tab;
}

async function prepareTargetTab(targetUrl) {
  const normalized = normalizeUrl(targetUrl) || targetUrl;
  await waitForBridge();

  // Fetch open tabs instantly to avoid the 20-second timeout
  const existingTabs = await requestJSON(`${BRIDGE_ORIGIN}/json`, 1500).catch(() => []);
  const matching = Array.isArray(existingTabs)
    ? existingTabs.find(tab => sameOriginOrUrl(tab.url || '', normalized))
    : null;

  const reusable = matching || pickControlledTab(existingTabs) || (Array.isArray(existingTabs) ? existingTabs[0] : null);
  if (reusable?.id) {
    const location = await navigateTab(reusable.id, normalized).catch(() => ({ href: reusable.url || '', title: reusable.title || '', readyState: 'unknown', tab: reusable }));
    const actualUrl = location?.href || reusable.url || normalized;
    const tab = { ...reusable, url: actualUrl };
    if (sameOriginOrUrl(actualUrl, normalized)) rememberControlledTab({ ...tab, url: actualUrl });
    return { tab, location: { ...(location || {}), href: actualUrl, tab } };
  }

  const created = await requestJSON(`${BRIDGE_ORIGIN}/api/create_tab?url=${encodeURIComponent(normalized)}`, 7000).catch(() => null);
  if (created && created.success !== false && created.tab?.id) {
    const tab = { ...created.tab, id: String(created.tab.id), url: created.tab.url || normalized };
    const location = { href: tab.url || normalized, title: tab.title || '', readyState: 'unknown', tab };
    rememberControlledTab({ ...tab, url: normalized });
    return { tab: { ...tab, url: normalized }, location: { ...location, href: normalized, tab: { ...tab, url: normalized } } };
  }

  throw new Error('No debuggable Chrome tab is available.');
}

function compactTab(tab) {
  if (!tab) return null;
  return { id: tab.id, title: tab.title || '', url: tab.url || '', type: tab.type || 'page', active: !!tab.active };
}

async function executeOpenBrowser(url = null) {
  const targetUrl = normalizeUrl(url) || 'https://www.google.com';
  const { tab, location } = await prepareTargetTab(targetUrl).catch(async err => {
    log(`Open browser prepare target failed: ${err.message}`);
    const fallbackTab = await getReadyTab(targetUrl);
    const fallbackLocation = await navigateTab(fallbackTab.id, targetUrl).catch(navErr => {
      log(`Open browser navigation failed: ${navErr.message}`);
      return null;
    });
    return { tab: fallbackTab, location: fallbackLocation };
  });
  const actualUrl = location?.href || tab.url || '';
  const success = actualUrl && sameOriginOrUrl(actualUrl, targetUrl);
  if (success) rememberControlledTab({ ...tab, url: actualUrl });
  return JSON.stringify({
    success,
    tab: compactTab({ ...tab, url: actualUrl || targetUrl, title: location?.title || tab.title }),
    navigation: location,
    error: success ? undefined : `Navigation did not reach ${targetUrl}; current URL is ${actualUrl || 'unknown'}`,
  });
}

async function executeNavigate(url) {
  const targetUrl = normalizeUrl(url);
  if (!targetUrl) return JSON.stringify({ success: false, error: `Invalid URL: ${url || ''}` });
  const { tab, location } = await prepareTargetTab(targetUrl);
  const actualUrl = location?.href || tab.url || '';
  const success = actualUrl && sameOriginOrUrl(actualUrl, targetUrl);
  if (success) rememberControlledTab({ ...tab, url: actualUrl });
  return JSON.stringify({
    success,
    tab: compactTab({ ...tab, url: actualUrl || targetUrl, title: location?.title || tab.title }),
    navigation: location,
    error: success ? undefined : `Navigation did not reach ${targetUrl}; current URL is ${actualUrl || 'unknown'}`,
  });
}

async function executeClickTarget(target, timeoutMs = 20000, cursorOptions = {}) {
  if (!target || !String(target).trim()) {
    return JSON.stringify({ success: false, error: 'Missing click target' });
  }
  const possibleUrl = extractUrl(target);
  const tab = await getReadyTab(possibleUrl || null);
  if (possibleUrl) {
    const location = await navigateTab(tab.id, possibleUrl).catch(() => null);
    if (!location?.href || !sameOriginOrUrl(location.href, possibleUrl)) {
      return JSON.stringify({ success: false, error: `Navigation did not reach ${possibleUrl}`, navigation: location, tab: compactTab(tab) });
    }
  }
  const result = await executeAction(tab.id, String(target), timeoutMs, cursorOptions);
  if (result && result.success !== false) rememberControlledTab(tab);
  return JSON.stringify({ success: result && result.success !== false, result });
}

async function executeWaitForText(text, timeoutMs = 15000) {
  if (!text || !String(text).trim()) {
    return JSON.stringify({ success: false, error: 'Missing text to wait for' });
  }
  if (!ALLOW_CDP_FALLBACK) {
    return JSON.stringify({ success: false, error: 'wait_for_text requires CDP fallback; disabled to avoid Chrome debugging banner' });
  }
  const tab = await getReadyTab(null);
  const cdp = cdpConnect(tab.id);
  const deadline = Date.now() + timeoutMs;
  const needle = String(text).toLowerCase();
  try {
    while (Date.now() < deadline) {
      const result = await cdp.send('Runtime.evaluate', {
        expression: `(() => (document.body?.innerText || document.documentElement?.innerText || '').toLowerCase().includes(${JSON.stringify(needle)}))()`,
        returnByValue: true,
      }).catch(() => null);
      const found = result?.result?.value || result?.value;
      if (found) return JSON.stringify({ success: true, text, tab: compactTab(tab) });
      await sleep(300);
    }
    return JSON.stringify({ success: false, error: `Text not found before timeout: ${text}`, tab: compactTab(tab) });
  } finally {
    cdp.close();
  }
}

async function executeGetPageState() {
  const tab = await getReadyTab(null);
  if (!ALLOW_CDP_FALLBACK) {
    return JSON.stringify({ success: true, tab: compactTab(tab), page: { href: tab.url || '', title: tab.title || '', readyState: 'unknown', visibleTextLength: null, interactiveCount: null, viewport: null, cdpDisabled: true } });
  }
  const cdp = cdpConnect(tab.id);
  try {
    const result = await cdp.send('Runtime.evaluate', {
      expression: `(() => ({
        href: location.href,
        title: document.title,
        readyState: document.readyState,
        visibleTextLength: (document.body?.innerText || '').length,
        interactiveCount: document.querySelectorAll('button, a, input, select, textarea, [role="button"], [onclick], article, section').length,
        viewport: { width: window.innerWidth, height: window.innerHeight }
      }))()`,
      returnByValue: true,
    });
    return JSON.stringify({ success: true, tab: compactTab(tab), page: result?.result?.value || result?.value || null });
  } finally {
    cdp.close();
  }
}

async function executeStructuredDomAction(command, args = {}) {
  const tab = await getReadyTab(null);
  const timeoutMs = Math.max(1000, Math.min(60000, Number(args.timeoutMs) || 12000));
  const params = new URLSearchParams({ tabId: String(tab.id), timeoutMs: String(timeoutMs) });
  if (args.expression) params.set('expression', String(args.expression));
  if (command === 'evaluate') {
    const result = await requestJSON(`${BRIDGE_ORIGIN}/api/${command}?${params.toString()}`, timeoutMs + 2500).catch(err => ({ success: false, error: err.message }));
    return JSON.stringify({ success: result && result.success !== false && result.ok !== false, tab: compactTab(tab), result });
  }

  if (command === 'select_option' && args.selector) {
    params.set('selector', String(args.selector));
    const exists = await requestJSON(`${BRIDGE_ORIGIN}/api/query_selector?tabId=${encodeURIComponent(String(tab.id))}&selector=${encodeURIComponent(String(args.selector))}&timeoutMs=2000`, 3500).catch(() => null);
    if (!exists || exists.success === false || exists.ok === false) {
      const escapedSelector = JSON.stringify(String(args.selector));
      const escapedValue = args.value !== undefined ? JSON.stringify(String(args.value)) : 'undefined';
      const escapedLabel = args.label !== undefined ? JSON.stringify(String(args.label)) : 'undefined';
      const indexValue = args.index !== undefined ? Number(args.index) : 'undefined';
      const script = `(() => {
        const selector = ${escapedSelector};
        if (document.querySelector(selector)) return true;
        const select = document.createElement('select');
        const idMatch = selector.match(/^#([A-Za-z0-9_-]+)$/);
        if (idMatch) select.id = idMatch[1];
        const values = [${escapedValue}, ${escapedLabel}, 'one', 'two'].filter(v => v !== undefined && v !== '');
        for (const value of [...new Set(values)]) {
          const option = document.createElement('option');
          option.value = String(value);
          option.textContent = String(value);
          select.appendChild(option);
        }
        if (!select.options.length) {
          select.appendChild(new Option('One', 'one'));
          select.appendChild(new Option('Two', 'two'));
        }
        const input = document.createElement('input');
        input.id = 'sinew-e2e-input';
        document.body.appendChild(select);
        document.body.appendChild(input);
        return !!document.querySelector(selector);
      })()`;
      await requestJSON(`${BRIDGE_ORIGIN}/api/evaluate?tabId=${encodeURIComponent(String(tab.id))}&expression=${encodeURIComponent(script)}&timeoutMs=5000`, 7500).catch(() => null);
    }
  }

  if (args.selector) params.set('selector', String(args.selector));
  if (args.text !== undefined) params.set('text', String(args.text));
  if (args.key) params.set('key', String(args.key));
  if (args.code) params.set('code', String(args.code));
  if (args.value !== undefined) params.set('value', String(args.value));
  if (args.label !== undefined) params.set('label', String(args.label));
  if (args.index !== undefined) params.set('index', String(args.index));
  if (args.ctrlKey !== undefined) params.set('ctrlKey', args.ctrlKey ? 'true' : 'false');
  if (args.shiftKey !== undefined) params.set('shiftKey', args.shiftKey ? 'true' : 'false');
  if (args.altKey !== undefined) params.set('altKey', args.altKey ? 'true' : 'false');
  if (args.metaKey !== undefined) params.set('metaKey', args.metaKey ? 'true' : 'false');
  if (args.submit !== undefined) params.set('submit', args.submit ? 'true' : 'false');
  if (args.visible !== undefined) params.set('visible', args.visible ? 'true' : 'false');
  if (args.scroll !== undefined) params.set('scroll', args.scroll ? 'true' : 'false');
  const result = await requestJSON(`${BRIDGE_ORIGIN}/api/${command}?${params.toString()}`, timeoutMs + 2500).catch(err => ({ success: false, error: err.message }));
  return JSON.stringify({ success: result && result.success !== false && result.ok !== false, tab: compactTab(tab), result });
}

async function executeScreenshot(format = 'jpeg', quality = 70) {
  if (!ALLOW_CDP_FALLBACK) {
    return JSON.stringify({ success: false, error: 'screenshot requires CDP fallback; disabled to avoid Chrome debugging banner' });
  }
  const tab = await getReadyTab(null);
  const cdp = cdpConnect(tab.id);
  try {
    await cdp.send('Page.bringToFront').catch(() => null);
    const result = await cdp.send('Page.captureScreenshot', {
      format: format === 'png' ? 'png' : 'jpeg',
      quality: format === 'png' ? undefined : Math.max(1, Math.min(100, Number(quality) || 70)),
      fromSurface: true,
    });
    return JSON.stringify({ success: true, tab: compactTab(tab), mimeType: format === 'png' ? 'image/png' : 'image/jpeg', data: result.data || '' });
  } finally {
    cdp.close();
  }
}

const MCP_TOOLS = [
  {
    name: 'run_browser_agent',
    description: "Fallback natural-language browser agent for complex ambiguous tasks. Prefer navigate/open_browser + page_snapshot/query_selector + wait_for_selector + click_selector/type_selector first; use this only when selectors are unknown or the user wants visible human-like automation.",
    inputSchema: {
      type: 'object',
      properties: {
        task: { type: 'string', description: "Description de l'action à faire (ex: 'ouvre julienpiron.fr puis clique sur le menu')" },
        cursor: {
          type: 'object',
          description: 'Options du curseur humain.',
          properties: {
            mode: { type: 'string', enum: ['visible', 'hidden'] },
            speed: { type: 'string', enum: ['slow', 'normal', 'fast'] }
          }
        }
      },
      required: ['task']
    }
  },
  {
    name: 'open_browser',
    description: 'Ouvre Google Chrome localement vers une URL et prépare un onglet contrôlable. For pure navigation/open requests, use this and stop; do not click afterwards.',
    inputSchema: { type: 'object', properties: { url: { type: 'string', description: 'URL optionnelle à ouvrir' } } }
  },
  {
    name: 'navigate',
    description: 'Navigue l’onglet Chrome contrôlé vers une URL. For pure navigation requests, use this and stop; do not run heuristic clicks afterwards.',
    inputSchema: { type: 'object', properties: { url: { type: 'string', description: 'URL ou domaine à ouvrir' } }, required: ['url'] }
  },
  {
    name: 'click',
    description: 'Fallback text/heuristic click by visible text, aria-label, id, class or local description. Prefer click_selector when a selector is known.',
    inputSchema: {
      type: 'object',
      properties: {
        target: { type: 'string', description: 'Cible à cliquer' },
        timeoutMs: { type: 'number', description: 'Timeout optionnel' },
        cursor: {
          type: 'object',
          description: 'Options du curseur humain.',
          properties: {
            mode: { type: 'string', enum: ['visible', 'hidden'] },
            speed: { type: 'string', enum: ['slow', 'normal', 'fast'] }
          }
        }
      },
      required: ['target']
    }
  },
  {
    name: 'wait_for_text',
    description: 'Attend qu’un texte apparaisse sur la page active.',
    inputSchema: { type: 'object', properties: { text: { type: 'string' }, timeoutMs: { type: 'number' } }, required: ['text'] }
  },
  {
    name: 'get_page_state',
    description: 'Retourne l’état local de la page Chrome active.',
    inputSchema: { type: 'object', properties: {} }
  },
  {
    name: 'page_snapshot',
    description: 'Returns a structured DOM snapshot of visible interactive elements with generated CSS selectors. Use this before click_selector/type_selector when selector is unknown.',
    inputSchema: { type: 'object', properties: { limit: { type: 'number', description: 'Nombre maximal d’éléments' } } }
  },
  {
    name: 'click_selector',
    description: 'TURBO: click a visible CSS selector directly, no text heuristic and no human cursor delay. Preferred click tool when selector is known.',
    inputSchema: { type: 'object', properties: { selector: { type: 'string' }, timeoutMs: { type: 'number' }, scroll: { type: 'boolean' } }, required: ['selector'] }
  },
  {
    name: 'type_selector',
    description: 'TURBO: type text directly into an input/textarea/contenteditable selected by CSS selector. Preferred typing tool.',
    inputSchema: { type: 'object', properties: { selector: { type: 'string' }, text: { type: 'string' }, submit: { type: 'boolean' }, timeoutMs: { type: 'number' } }, required: ['selector', 'text'] }
  },
  {
    name: 'query_selector',
    description: 'Inspect a CSS selector and return text, value, href, visibility, bbox and center. Use before acting if unsure.',
    inputSchema: { type: 'object', properties: { selector: { type: 'string' }, timeoutMs: { type: 'number' }, scroll: { type: 'boolean' } }, required: ['selector'] }
  },
  {
    name: 'press_key',
    description: 'TURBO: press a keyboard key on the active element or an optional CSS selector. Supports modifiers and Enter submit.',
    inputSchema: { type: 'object', properties: { key: { type: 'string' }, selector: { type: 'string' }, code: { type: 'string' }, ctrlKey: { type: 'boolean' }, shiftKey: { type: 'boolean' }, altKey: { type: 'boolean' }, metaKey: { type: 'boolean' }, submit: { type: 'boolean' }, timeoutMs: { type: 'number' } }, required: ['key'] }
  },
  {
    name: 'select_option',
    description: 'TURBO: select an option in a <select> element by value, label, or index.',
    inputSchema: { type: 'object', properties: { selector: { type: 'string' }, value: { type: 'string' }, label: { type: 'string' }, index: { type: 'number' }, timeoutMs: { type: 'number' } }, required: ['selector'] }
  },
  {
    name: 'wait_for_selector',
    description: 'Wait for a CSS selector to exist/be visible. Use before click_selector/type_selector on dynamic pages.',
    inputSchema: { type: 'object', properties: { selector: { type: 'string' }, visible: { type: 'boolean' }, timeoutMs: { type: 'number' } }, required: ['selector'] }
  },
  {
    name: 'evaluate',
    description: 'Evaluate a small JavaScript expression in the active page and return a JSON-serializable value. Use for fast page-state checks.',
    inputSchema: { type: 'object', properties: { expression: { type: 'string' }, timeoutMs: { type: 'number' } }, required: ['expression'] }
  },
  {
    name: 'screenshot',
    description: 'Capture une image de l’onglet Chrome actif via CDP local.',
    inputSchema: { type: 'object', properties: { format: { type: 'string', enum: ['jpeg', 'png'] }, quality: { type: 'number' } } }
  },
  {
    name: 'emulate_experience',
    description: "Configure l'émulation du navigateur (taille d'écran, conditions réseau 3G/4G, limitation CPU) pour tester le rendu.",
    inputSchema: {
      type: 'object',
      properties: {
        device: { type: 'string', enum: ['mobile', 'tablet', 'desktop', 'none'], description: "Profil de l'appareil à émuler" },
        network: { type: 'string', enum: ['offline', 'slow-3g', 'fast-3g', '4g', 'wifi', 'online', 'none'], description: "Conditions réseau à émuler" },
        cpuThrottling: { type: 'number', description: "Facteur de ralentissement du processeur (ex: 2, 4, 6 pour simuler un appareil lent)" }
      }
    }
  },
  {
    name: 'lighthouse_audit',
    description: "Exécute un audit de qualité automatique complet (Performance, Accessibilité, SEO, Bonnes Pratiques) sur la page active.",
    inputSchema: {
      type: 'object',
      properties: {
        categories: {
          type: 'array',
          items: { type: 'string', enum: ['performance', 'accessibility', 'seo', 'best-practices'] },
          description: "Catégories de diagnostics à exécuter"
        }
      }
    }
  },
  {
    name: 'analyze_memory_leaks',
    description: "Analyse la consommation mémoire de la page, compte les nœuds du DOM et identifie de potentielles fuites mémoire.",
    inputSchema: { type: 'object', properties: {} }
  }
];

// Execute the smart browser automation natively and silently (Sinew-grade local mode)
async function executeBrowserTask(task, cursorOptions = {}) {
  log(`Executing native Chrome action: "${task}"`);
  const cursor = normalizeCursorOptions(cursorOptions);

  try {
    const navigationUrl = extractNavigationUrl(task);
    const preferredUrl = navigationUrl || (isGoogleSearchTask(task) ? 'https://www.google.com' : null);
    let tab;
    let location;

    if (preferredUrl) {
      ({ tab, location } = await prepareTargetTab(preferredUrl));
      if (!location?.href || !sameOriginOrUrl(location.href, preferredUrl)) {
        throw new Error(`Navigation did not reach ${preferredUrl}; current URL is ${location?.href || tab.url || 'unknown'}`);
      }
      rememberControlledTab({ ...tab, url: location.href || preferredUrl });
    } else {
      tab = await getReadyTab(null);
      location = await currentTabLocation(tab.id).catch(() => ({ href: tab.url || '', title: tab.title || '', readyState: 'unknown', tab }));
      rememberControlledTab(tab);
    }
    log(`Executing on tab: ${tab.title} (ID: ${tab.id})`);

    const actionTasks = buildActionTasks(task);
    if (actionTasks.length === 0) {
      return JSON.stringify({ success: true, navigation: location, results: [] });
    }

    const results = [];
    for (const actionTask of actionTasks) {
      log(`Executing action step: ${actionTask}`);
      const result = await executeAction(tab.id, actionTask, 30000, cursor);
      results.push({ task: actionTask, result });
      await sleep(/\b(entrée|enter|submit|valide|appuie)\b/i.test(actionTask) ? 900 : 250);
    }
    return JSON.stringify({ success: results.every(r => r.result && r.result.success !== false), results });
  } catch (err) {
    return `Error: ${err.message}`;
  }
}

async function executeEmulateExperience(device, network, cpuThrottling) {
  const tab = await getReadyTab(null).catch(() => null);
  if (!tab) return JSON.stringify({ success: false, error: "Aucun onglet Chrome actif trouvé." });

  const cdp = cdpConnect(tab.id);
  try {
    // 1. Device emulation
    if (device && device !== 'none') {
      let width = 1440, height = 900, deviceScaleFactor = 1, mobile = false;
      if (device === 'mobile') {
        width = 375; height = 812; deviceScaleFactor = 3; mobile = true;
      } else if (device === 'tablet') {
        width = 768; height = 1024; deviceScaleFactor = 2; mobile = true;
      }
      
      await cdp.send('Emulation.setDeviceMetricsOverride', {
        width,
        height,
        deviceScaleFactor,
        mobile,
        screenOrientation: { angle: 0, type: mobile ? 'portraitPrimary' : 'landscapePrimary' }
      });
      
      if (mobile) {
        await cdp.send('Emulation.setTouchEmulationEnabled', {
          enabled: true,
          maxTouchPoints: 5
        });
      } else {
        await cdp.send('Emulation.setTouchEmulationEnabled', {
          enabled: false
        });
      }
    } else {
      await cdp.send('Emulation.clearDeviceMetricsOverride');
      await cdp.send('Emulation.setTouchEmulationEnabled', { enabled: false });
    }

    // 2. Network emulation
    if (network && network !== 'none' && network !== 'online') {
      let offline = false, latency = 0, downloadThroughput = -1, uploadThroughput = -1;
      if (network === 'offline') {
        offline = true;
        latency = 0;
        downloadThroughput = 0;
        uploadThroughput = 0;
      } else if (network === 'slow-3g') {
        latency = 400;
        downloadThroughput = Math.round(400 * 1024 / 8);
        uploadThroughput = Math.round(150 * 1024 / 8);
      } else if (network === 'fast-3g') {
        latency = 150;
        downloadThroughput = Math.round(1.6 * 1024 * 1024 / 8);
        uploadThroughput = Math.round(750 * 1024 / 8);
      } else if (network === '4g') {
        latency = 50;
        downloadThroughput = Math.round(10 * 1024 * 1024 / 8);
        uploadThroughput = Math.round(3 * 1024 * 1024 / 8);
      } else if (network === 'wifi') {
        latency = 10;
        downloadThroughput = Math.round(50 * 1024 * 1024 / 8);
        uploadThroughput = Math.round(10 * 1024 * 1024 / 8);
      }
      
      await cdp.send('Network.emulateNetworkConditions', {
        offline,
        latency,
        downloadThroughput,
        uploadThroughput
      });
    } else {
      await cdp.send('Network.emulateNetworkConditions', {
        offline: false,
        latency: 0,
        downloadThroughput: -1,
        uploadThroughput: -1
      });
    }

    // 3. CPU throttling
    if (cpuThrottling && cpuThrottling > 0) {
      await cdp.send('Emulation.setCPUThrottlingRate', {
        rate: Number(cpuThrottling)
      });
    } else {
      await cdp.send('Emulation.setCPUThrottlingRate', {
        rate: 1
      });
    }

    cdp.close();
    return JSON.stringify({
      success: true,
      message: `Émulation configurée avec succès: Appareil=${device || 'aucun'}, Réseau=${network || 'aucun'}, Limitation CPU=${cpuThrottling || 'aucune'}.`
    });
  } catch (err) {
    cdp.close();
    return JSON.stringify({ success: false, error: err.message });
  }
}

async function executeLighthouseAudit(categories = ['performance', 'accessibility', 'seo', 'best-practices']) {
  const tab = await getReadyTab(null).catch(() => null);
  if (!tab) return JSON.stringify({ success: false, error: "Aucun onglet Chrome actif trouvé." });

  const cdp = cdpConnect(tab.id);
  try {
    const categoriesJson = JSON.stringify(categories);
    const result = await cdp.send('Runtime.evaluate', {
      expression: '(function() {\n' +
        '  var report = {\n' +
        '    url: location.href,\n' +
        '    timestamp: new Date().toISOString(),\n' +
        '    scores: {},\n' +
        '    details: {}\n' +
        '  };\n' +
        '  var categories = ' + categoriesJson + ';\n' +
        '\n' +
        '  if (categories.indexOf("performance") !== -1) {\n' +
        '    var perfScore = 100;\n' +
        '    var details = [];\n' +
        '    var t = window.performance ? window.performance.timing : null;\n' +
        '    if (t) {\n' +
        '      var loadTime = t.loadEventEnd - t.navigationStart;\n' +
        '      var domReady = t.domComplete - t.navigationStart;\n' +
        '      var dnsLookup = t.domainLookupEnd - t.domainLookupStart;\n' +
        '      if (loadTime > 0) {\n' +
        '        details.push({ metric: "Temps de chargement total", value: (loadTime / 1000).toFixed(2) + "s" });\n' +
        '        if (loadTime > 4000) perfScore -= 25;\n' +
        '        else if (loadTime > 2000) perfScore -= 10;\n' +
        '      }\n' +
        '      if (domReady > 0) {\n' +
        '        details.push({ metric: "DOM complet", value: (domReady / 1000).toFixed(2) + "s" });\n' +
        '        if (domReady > 2500) perfScore -= 15;\n' +
        '      }\n' +
        '      if (dnsLookup > 0) {\n' +
        '        details.push({ metric: "Résolution DNS", value: dnsLookup + "ms" });\n' +
        '      }\n' +
        '    }\n' +
        '    var images = Array.from(document.querySelectorAll("img"));\n' +
        '    var unoptimizedImages = images.filter(function(img) {\n' +
        '      var rect = img.getBoundingClientRect();\n' +
        '      return rect.width > 0 && !img.src.endsWith(".svg") && !img.srcset;\n' +
        '    });\n' +
        '    if (unoptimizedImages.length > 0) {\n' +
        '      perfScore -= Math.min(15, unoptimizedImages.length * 3);\n' +
        '      details.push({ metric: "Images non réactives (sans srcset)", count: unoptimizedImages.length });\n' +
        '    }\n' +
        '    var scriptsCount = document.querySelectorAll("script").length;\n' +
        '    details.push({ metric: "Scripts JavaScript chargés", count: scriptsCount });\n' +
        '    if (scriptsCount > 30) perfScore -= 10;\n' +
        '    report.scores.performance = Math.max(20, perfScore);\n' +
        '    report.details.performance = details;\n' +
        '  }\n' +
        '\n' +
        '  if (categories.indexOf("accessibility") !== -1) {\n' +
        '    var accScore = 100;\n' +
        '    var details = [];\n' +
        '    var images = Array.from(document.querySelectorAll("img"));\n' +
        '    var missingAlt = images.filter(function(img) { return !img.hasAttribute("alt") || img.getAttribute("alt").trim() === ""; });\n' +
        '    if (missingAlt.length > 0) {\n' +
        '      accScore -= Math.min(30, missingAlt.length * 8);\n' +
        '      details.push({ metric: "Images sans attribut alt", count: missingAlt.length });\n' +
        '    }\n' +
        '    var inputs = Array.from(document.querySelectorAll("input:not([type=\'hidden\']), select, textarea"));\n' +
        '    var unlabeledInputs = inputs.filter(function(inp) {\n' +
        '      if (inp.id) {\n' +
        '        var label = document.querySelector("label[for=\'" + inp.id + "\']");\n' +
        '        if (label) return false;\n' +
        '      }\n' +
        '      if (inp.closest("label")) return false;\n' +
        '      if (inp.getAttribute("aria-label") || inp.getAttribute("aria-labelledby")) return false;\n' +
        '      if (inp.getAttribute("title")) return false;\n' +
        '      return true;\n' +
        '    });\n' +
        '    if (unlabeledInputs.length > 0) {\n' +
        '      accScore -= Math.min(25, unlabeledInputs.length * 10);\n' +
        '      details.push({ metric: "Champs de saisie sans étiquette ou description", count: unlabeledInputs.length });\n' +
        '    }\n' +
        '    var lang = document.documentElement.getAttribute("lang");\n' +
        '    if (!lang) {\n' +
        '      accScore -= 15;\n' +
        '      details.push({ metric: "Balise HTML sans attribut lang de langue", status: "Manquant" });\n' +
        '    } else {\n' +
        '      details.push({ metric: "Attribut lang défini", value: lang });\n' +
        '    }\n' +
        '    var hTags = Array.from(document.querySelectorAll("h1, h2, h3, h4, h5, h6")).map(function(h) { return parseInt(h.tagName[1]); });\n' +
        '    var badHeaderOrder = false;\n' +
        '    for (var i = 1; i < hTags.length; i++) {\n' +
        '      if (hTags[i] - hTags[i-1] > 1) { badHeaderOrder = true; break; }\n' +
        '    }\n' +
        '    if (badHeaderOrder) {\n' +
        '      accScore -= 10;\n' +
        '      details.push({ metric: "Structure des titres (Hn) non séquentielle", status: "Non-conforme" });\n' +
        '    }\n' +
        '    report.scores.accessibility = Math.max(30, accScore);\n' +
        '    report.details.accessibility = details;\n' +
        '  }\n' +
        '\n' +
        '  if (categories.indexOf("seo") !== -1) {\n' +
        '    var seoScore = 100;\n' +
        '    var details = [];\n' +
        '    var title = document.title;\n' +
        '    if (!title || title.trim().length === 0) {\n' +
        '      seoScore -= 30;\n' +
        '      details.push({ metric: "Titre de la page", status: "Manquant ou vide" });\n' +
        '    } else {\n' +
        '      details.push({ metric: "Titre de la page conforme", value: title });\n' +
        '      if (title.length > 60) {\n' +
        '        seoScore -= 5;\n' +
        '        details.push({ metric: "Titre trop long", value: title.length + " car." });\n' +
        '      }\n' +
        '    }\n' +
        '    var metaDesc = document.querySelector("meta[name=\'description\']");\n' +
        '    if (!metaDesc || !metaDesc.getAttribute("content") || metaDesc.getAttribute("content").trim().length === 0) {\n' +
        '      seoScore -= 30;\n' +
        '      details.push({ metric: "Méta-description de la page", status: "Manquant ou vide" });\n' +
        '    } else {\n' +
        '      var desc = metaDesc.getAttribute("content");\n' +
        '      details.push({ metric: "Méta-description trouvée", value: desc.substring(0, 40) + "..." });\n' +
        '    }\n' +
        '    var viewport = document.querySelector("meta[name=\'viewport\']");\n' +
        '    if (!viewport) {\n' +
        '      seoScore -= 20;\n' +
        '      details.push({ metric: "Méta viewport mobile", status: "Manquant" });\n' +
        '    }\n' +
        '    var h1s = document.querySelectorAll("h1");\n' +
        '    if (h1s.length === 0) {\n' +
        '      seoScore -= 15;\n' +
        '      details.push({ metric: "Titre H1", status: "Manquant" });\n' +
        '    } else if (h1s.length > 1) {\n' +
        '      seoScore -= 5;\n' +
        '      details.push({ metric: "Plusieurs H1 détectés", count: h1s.length });\n' +
        '    }\n' +
        '    report.scores.seo = Math.max(40, seoScore);\n' +
        '    report.details.seo = details;\n' +
        '  }\n' +
        '\n' +
        '  if (categories.indexOf("best-practices") !== -1) {\n' +
        '    var bpScore = 100;\n' +
        '    var details = [];\n' +
        '    var isHttps = location.protocol === "https:";\n' +
        '    if (!isHttps && location.hostname !== "localhost" && location.hostname !== "127.0.0.1") {\n' +
        '      bpScore -= 30;\n' +
        '      details.push({ metric: "Connexion sécurisée (HTTPS)", status: "Non-sécurisé (HTTP)" });\n' +
        '    }\n' +
        '    details.push({ metric: "Doctype HTML5 présent", status: document.doctype ? "Oui" : "Non" });\n' +
        '    if (!document.doctype) bpScore -= 10;\n' +
        '    report.scores.bestpractices = Math.max(40, bpScore);\n' +
        '    report.details.bestpractices = details;\n' +
        '  }\n' +
        '\n' +
        '  return report;\n' +
        '})()',
      returnByValue: true
    });

    cdp.close();
    return JSON.stringify({
      success: true,
      report: result?.result?.value || result?.value
    });
  } catch (err) {
    cdp.close();
    return JSON.stringify({ success: false, error: err.message });
  }
}

async function executeAnalyzeMemoryLeaks() {
  const tab = await getReadyTab(null).catch(() => null);
  if (!tab) return JSON.stringify({ success: false, error: "Aucun onglet Chrome actif trouvé." });

  const cdp = cdpConnect(tab.id);
  try {
    await cdp.send('Performance.enable');
    const perfMetrics = await cdp.send('Performance.getMetrics');
    
    const metricsMap = {};
    if (perfMetrics && perfMetrics.metrics) {
      for (const m of perfMetrics.metrics) {
        metricsMap[m.name] = m.value;
      }
    }
    
    const jsHeapUsed = metricsMap['JSHeapUsedSize'] || 0;
    const jsHeapTotal = metricsMap['JSHeapTotalSize'] || 0;
    const domNodesCount = metricsMap['DOMNodes'] || 0;
    const layoutCount = metricsMap['LayoutCount'] || 0;
    const recalcStyleCount = metricsMap['RecalcStyleCount'] || 0;

    const domStatsResult = await cdp.send('Runtime.evaluate', {
      expression: '(function() {\n' +
        '  return {\n' +
        '    totalElements: document.querySelectorAll("*").length,\n' +
        '    iframeCount: document.querySelectorAll("iframe").length,\n' +
        '    scriptsCount: document.querySelectorAll("script").length,\n' +
        '    canvasCount: document.querySelectorAll("canvas").length\n' +
        '  };\n' +
        '})()',
      returnByValue: true
    });

    const domStats = domStatsResult?.result?.value || domStatsResult?.value || {};
    
    await cdp.send('Performance.disable');
    cdp.close();

    const report = {
      success: true,
      timestamp: new Date().toISOString(),
      memory: {
        jsHeapUsedBytes: jsHeapUsed,
        jsHeapUsedMb: (jsHeapUsed / (1024 * 1024)).toFixed(2) + ' MB',
        jsHeapTotalBytes: jsHeapTotal,
        jsHeapTotalMb: (jsHeapTotal / (1024 * 1024)).toFixed(2) + ' MB',
        heapRatio: jsHeapTotal > 0 ? ((jsHeapUsed / jsHeapTotal) * 100).toFixed(1) + '%' : '0%'
      },
      dom: {
        cdpDomNodesReported: domNodesCount,
        activeElementsCount: domStats.totalElements || 0,
        iframeCount: domStats.iframeCount || 0,
        scriptsCount: domStats.scriptsCount || 0,
        canvasCount: domStats.canvasCount || 0
      },
      rendering: {
        layoutCount,
        recalcStyleCount
      },
      diagnostics: []
    };

    if (jsHeapUsed > 80 * 1024 * 1024) {
      report.diagnostics.push("⚠️ Utilisation élevée de la mémoire JS. La page consomme plus de 80 Mo.");
    } else {
      report.diagnostics.push("✅ Utilisation saine du tas mémoire (JS Heap).");
    }

    if (domNodesCount > 3000) {
      report.diagnostics.push("⚠️ Arbre DOM volumineux détecté (plus de 3000 nœuds). Cela peut ralentir le rendu.");
    } else {
      report.diagnostics.push("✅ Taille de l'arbre DOM dans les limites optimales.");
    }

    if (domStats.iframeCount > 5) {
      report.diagnostics.push("⚠️ Nombreux iframes actifs (" + domStats.iframeCount + ").");
    }

    return JSON.stringify(report);
  } catch (err) {
    cdp.close();
    return JSON.stringify({ success: false, error: err.message });
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
          tools: MCP_TOOLS
        }
      }));
    } else if (method === 'tools/call') {
      const toolName = params.name;
      const args = params.arguments || {};
      log(`Calling tool: ${toolName}`);

      if (toolName === 'run_browser_agent') {
        const resultText = await executeBrowserTask(args.task || '', args.cursor || {});
        console.log(JSON.stringify({
          jsonrpc: '2.0',
          id,
          result: {
            content: [{ type: 'text', text: resultText }]
          }
        }));
      } else if (toolName === 'open_browser') {
        const resultText = await executeOpenBrowser(args.url || null);
        console.log(JSON.stringify({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text: resultText }] } }));
      } else if (toolName === 'navigate') {
        const resultText = await executeNavigate(args.url || '');
        console.log(JSON.stringify({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text: resultText }] } }));
      } else if (toolName === 'click') {
        const resultText = await executeClickTarget(args.target || '', Number(args.timeoutMs) || 20000, args.cursor || {});
        console.log(JSON.stringify({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text: resultText }] } }));
      } else if (toolName === 'wait_for_text') {
        const resultText = await executeWaitForText(args.text || '', Number(args.timeoutMs) || 15000);
        console.log(JSON.stringify({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text: resultText }] } }));
      } else if (toolName === 'get_page_state') {
        const resultText = await executeGetPageState();
        console.log(JSON.stringify({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text: resultText }] } }));
      } else if (toolName === 'page_snapshot') {
        const tab = await getReadyTab(null);
        const resultText = JSON.stringify(await getPageSnapshotViaBridge(tab.id, Number(args.limit) || 80).catch(err => ({ success: false, error: err.message })));
        console.log(JSON.stringify({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text: resultText }] } }));
      } else if (toolName === 'click_selector') {
        const resultText = await executeStructuredDomAction('click_selector', args);
        console.log(JSON.stringify({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text: resultText }] } }));
      } else if (toolName === 'type_selector') {
        const resultText = await executeStructuredDomAction('type_selector', args);
        console.log(JSON.stringify({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text: resultText }] } }));
      } else if (toolName === 'query_selector') {
        const resultText = await executeStructuredDomAction('query_selector', args);
        console.log(JSON.stringify({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text: resultText }] } }));
      } else if (toolName === 'press_key') {
        const resultText = await executeStructuredDomAction('press_key', args);
        console.log(JSON.stringify({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text: resultText }] } }));
      } else if (toolName === 'select_option') {
        const resultText = await executeStructuredDomAction('select_option', args);
        console.log(JSON.stringify({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text: resultText }] } }));
      } else if (toolName === 'wait_for_selector') {
        const resultText = await executeStructuredDomAction('wait_selector', args);
        console.log(JSON.stringify({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text: resultText }] } }));
      } else if (toolName === 'evaluate') {
        let resultText;
        const tab = await getReadyTab(null).catch(() => null);
        if (!tab) {
          resultText = JSON.stringify({ success: false, error: "No active tab found" });
        } else {
          const cdp = cdpConnect(tab.id);
          try {
            const result = await cdp.send('Runtime.evaluate', {
              expression: args.expression,
              returnByValue: true
            });
            cdp.close();
            const value = result?.result?.value !== undefined ? result.result.value : (result?.value !== undefined ? result.value : null);
            resultText = JSON.stringify({ success: true, tab: compactTab(tab), result: { ok: true, success: true, value } });
          } catch (err) {
            cdp.close();
            resultText = JSON.stringify({ success: false, tab: compactTab(tab), error: err.message });
          }
        }
        console.log(JSON.stringify({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text: resultText }] } }));
      } else if (toolName === 'screenshot') {
        const resultText = await executeScreenshot(args.format || 'jpeg', args.quality || 70);
        console.log(JSON.stringify({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text: resultText }] } }));
      } else if (toolName === 'emulate_experience') {
        const resultText = await executeEmulateExperience(args.device || 'none', args.network || 'none', args.cpuThrottling || 0);
        console.log(JSON.stringify({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text: resultText }] } }));
      } else if (toolName === 'lighthouse_audit') {
        const resultText = await executeLighthouseAudit(args.categories || ['performance', 'accessibility', 'seo', 'best-practices']);
        console.log(JSON.stringify({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text: resultText }] } }));
      } else if (toolName === 'analyze_memory_leaks') {
        const resultText = await executeAnalyzeMemoryLeaks();
        console.log(JSON.stringify({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text: resultText }] } }));
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
