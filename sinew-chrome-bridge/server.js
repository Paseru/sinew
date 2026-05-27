// ðŸ§¬ Sinew Chrome Bridge â€” Upgraded WebSocket & HTTP Proxy Server
// Exposes SOTA Sinew-grade browser-level and page-level CDP multiplexing on port 9002.

const isNativeMode = process.argv.includes('--native');

const fs = require('fs');
const http = require('http');
const url = require('url');
const { WebSocketServer, WebSocket } = require('ws');
const os = require('os');
const path = require('path');
const homeDir = os.homedir();

// Configure log file redirection (SINEW_CHROME_BRIDGE_DIR or standard localappdata path)
const STATE_DIR = process.env.SINEW_CHROME_BRIDGE_DIR || path.join(process.env.LOCALAPPDATA || path.join(homeDir, 'AppData', 'Local'), 'Sinew', 'ChromeBridge');
const LOG_FILE_PATH = path.join(STATE_DIR, 'bridge.log');
try {
  fs.mkdirSync(STATE_DIR, { recursive: true });
} catch (e) {
  // Silent fallback if directory creation fails
}

function writeToLogFile(prefix, args) {
  try {
    const timestamp = new Date().toISOString();
    const formatted = args.map(arg => {
      if (arg instanceof Error) return arg.stack || arg.message;
      return typeof arg === 'object' ? JSON.stringify(arg) : String(arg);
    }).join(' ');
    fs.appendFileSync(LOG_FILE_PATH, `[${timestamp}] [${prefix}] ${formatted}\n`, 'utf8');
  } catch (e) {
    // Silent fallback if append fails
  }
}

const originalConsoleLog = console.log;
const originalConsoleInfo = console.info;
const originalConsoleError = console.error;

console.log = function(...args) {
  writeToLogFile('INFO', args);
  if (isNativeMode) {
    originalConsoleError.apply(console, args);
  } else {
    originalConsoleLog.apply(console, args);
  }
};

console.info = function(...args) {
  writeToLogFile('INFO', args);
  if (isNativeMode) {
    originalConsoleError.apply(console, args);
  } else {
    originalConsoleInfo.apply(console, args);
  }
};

console.error = function(...args) {
  writeToLogFile('ERROR', args);
  originalConsoleError.apply(console, args);
};

let scanFolder, sortFile, getFileSample;
try {
  const origPath = path.join(homeDir, '.gemini', 'antigravity', 'scratch', 'antigravity_workspace_organizer.js');
  const organizer = require(origPath);
  scanFolder = organizer.scanFolder;
  sortFile = organizer.sortFile;
  getFileSample = organizer.getFileSample;
} catch (e) {
  scanFolder = () => [];
  sortFile = () => false;
  getFileSample = () => "";
}

const PORT = Number(process.env.SINEW_CHROME_BRIDGE_PORT || 29002);

const LOCK_DIR = path.join(process.env.LOCALAPPDATA || path.join(homeDir, 'AppData', 'Local'), 'Sinew');
const LOCK_PATH = path.join(LOCK_DIR, 'chrome-bridge.lock');
let lockFd = null;
let lockPayloadBase = null;
let runAsBridgeClientOnly = false;

function isPidRunning(pid) {
  const n = Number(pid);
  if (!Number.isInteger(n) || n <= 0) return false;
  if (n === process.pid) return true;
  try {
    process.kill(n, 0);
    return true;
  } catch (err) {
    return err && (err.code === 'EPERM' || err.code === 'EACCES');
  }
}

function readBridgeLock() {
  try {
    return JSON.parse(fs.readFileSync(LOCK_PATH, 'utf8'));
  } catch {
    return null;
  }
}

function writeBridgeLock(fd) {
  if (!lockPayloadBase) {
    lockPayloadBase = {
      pid: process.pid,
      port: PORT,
      native: isNativeMode,
      startedAt: new Date().toISOString(),
      exe: process.execPath,
      argv: process.argv.slice(1),
    };
  }
  const lockPayload = {
    ...lockPayloadBase,
    heartbeatAt: new Date().toISOString(),
  };
  const payload = JSON.stringify(lockPayload, null, 2);
  fs.ftruncateSync(fd, 0);
  fs.writeSync(fd, payload, 0, 'utf8');
}

function acquireBridgeLock() {
  fs.mkdirSync(LOCK_DIR, { recursive: true });
  const openedAt = Date.now();
  while (Date.now() - openedAt < 1500) {
    try {
      lockFd = fs.openSync(LOCK_PATH, 'wx');
      writeBridgeLock(lockFd);
      return { acquired: true };
    } catch (err) {
      if (err.code !== 'EEXIST') throw err;
      const current = readBridgeLock();
      const heartbeatMs = current?.heartbeatAt ? Date.parse(current.heartbeatAt) : 0;
      const freshHeartbeat = Number.isFinite(heartbeatMs) && Date.now() - heartbeatMs < 20000;
      if (current && (isPidRunning(current.pid) || freshHeartbeat)) {
        return { acquired: false, active: true, current };
      }
      try { fs.unlinkSync(LOCK_PATH); } catch (unlinkErr) {
        if (unlinkErr.code !== 'ENOENT') {
          Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 75);
        }
      }
    }
  }
  const current = readBridgeLock();
  return { acquired: false, active: !!current, current };
}

function releaseBridgeLock() {
  if (lockFd !== null) {
    try { fs.closeSync(lockFd); } catch {}
    lockFd = null;
  }
  try {
    const current = readBridgeLock();
    if (!current || Number(current.pid) === process.pid) fs.unlinkSync(LOCK_PATH);
  } catch {}
}

const lockState = acquireBridgeLock();
if (!lockState.acquired) {
  if (isNativeMode) {
    runAsBridgeClientOnly = true;
    console.error(`🧬 [Proxy] Active Sinew Chrome Bridge detected via ${LOCK_PATH} (pid=${lockState.current?.pid || 'unknown'}). Starting native tunnel client.`);
  } else {
    console.error(`🧬 [Proxy] Active Sinew Chrome Bridge already running (pid=${lockState.current?.pid || 'unknown'}). Exiting.`);
    process.exit(0);
  }
}

if (lockFd !== null) {
  const heartbeat = setInterval(() => {
    try { writeBridgeLock(lockFd); } catch {}
  }, 5000);
  heartbeat.unref?.();
}

process.on('exit', releaseBridgeLock);
for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
  process.on(signal, () => {
    releaseBridgeLock();
    process.exit(0);
  });
}

const { spawn } = require('child_process');

function findChromeExecutable() {
  const fs = require('fs');
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
  if (!raw) return 'about:blank';
  const value = String(raw).trim();
  if (/^https?:\/\//i.test(value) || value === 'about:blank') return value;
  if (/^[a-z0-9-]+(\.[a-z0-9-]+)+(\/[^\s]*)?$/i.test(value)) return `https://${value}`;
  return value;
}

function launchChromeDetached(targetUrl = 'about:blank') {
  const chromeExe = findChromeExecutable();
  const args = [
    '--silent-debugger-extension-api',
    '--no-first-run',
    '--no-default-browser-check',
    normalizeUrl(targetUrl),
  ];
  const child = spawn(chromeExe, args, { detached: true, stdio: 'ignore', windowsHide: true });
  child.unref();
  return { chromeExe, args };
}

// Connected clients registries
let extensionSocket = null;
const pageSockets = new Map(); // tabId (string) -> Set of page-scoped playwright sockets
const browserSockets = new Set(); // Set of browser-scoped playwright sockets

let messageCounter = 0;
const pendingRequests = new Map(); // bridgeMsgId -> { playwrightSocket, originalId, sessionId, timeout, resolve }

function isExtensionConnected() {
  return !!(extensionSocket && typeof extensionSocket.send === 'function' && (extensionSocket.readyState === WebSocket.OPEN || extensionSocket.isNative === true));
}

class TabSession {
  constructor(tabId) {
    this.tabId = String(tabId);
    this.createdAt = Date.now();
    this.lastUsedAt = Date.now();
    this.lastDetachedAt = null;
    this.attached = false;
    this.pageSockets = new Set();
  }

  touch() { this.lastUsedAt = Date.now(); }

  addSocket(socket) {
    this.pageSockets.add(socket);
    this.touch();
  }

  removeSocket(socket) {
    this.pageSockets.delete(socket);
    this.touch();
  }

  get activeSocketCount() {
    return Array.from(this.pageSockets).filter(socket => socket.readyState === WebSocket.OPEN).length;
  }

  snapshot() {
    return {
      tabId: this.tabId,
      attached: this.attached,
      sockets: this.activeSocketCount,
      createdAt: this.createdAt,
      lastUsedAt: this.lastUsedAt,
      lastDetachedAt: this.lastDetachedAt,
    };
  }
}

class BrowserSessionManager {
  constructor(cleanupPolicy = {}) {
    this.cleanupPolicy = {
      detachIdleAfterMs: 30000,
      forgetDetachedAfterMs: 120000,
      ...cleanupPolicy,
    };
    this.tabSessions = new Map();
    this.browserSockets = new Set();
    const timer = setInterval(() => this.cleanup(), 10000);
    timer.unref?.();
  }

  getTabSession(tabId) {
    const key = String(tabId);
    let session = this.tabSessions.get(key);
    if (!session) {
      session = new TabSession(key);
      this.tabSessions.set(key, session);
    }
    return session;
  }

  addPageSocket(tabId, socket) { this.getTabSession(tabId).addSocket(socket); }

  removePageSocket(tabId, socket) {
    const session = this.tabSessions.get(String(tabId));
    if (session) session.removeSocket(socket);
  }

  addBrowserSocket(socket) { this.browserSockets.add(socket); }

  removeBrowserSocket(socket) {
    this.browserSockets.delete(socket);
    for (const session of this.tabSessions.values()) session.removeSocket(socket);
  }

  markAttached(tabId) {
    const session = this.getTabSession(tabId);
    session.attached = true;
    session.lastDetachedAt = null;
    session.touch();
  }

  markDetached(tabId) {
    const session = this.tabSessions.get(String(tabId));
    if (!session) return;
    session.attached = false;
    session.lastDetachedAt = Date.now();
    session.touch();
  }

  removeTab(tabId) { this.tabSessions.delete(String(tabId)); }

  cleanup() {
    const now = Date.now();
    for (const [tabId, session] of this.tabSessions.entries()) {
      if (session.activeSocketCount === 0 && session.attached && now - session.lastUsedAt > this.cleanupPolicy.detachIdleAfterMs) {
        if (isExtensionConnected()) {
          extensionSocket.send(JSON.stringify({ id: ++messageCounter, command: 'detach', params: { tabId } }));
        }
        session.attached = false;
        session.lastDetachedAt = now;
      }
      if (session.activeSocketCount === 0 && !session.attached && session.lastDetachedAt && now - session.lastDetachedAt > this.cleanupPolicy.forgetDetachedAfterMs) {
        this.tabSessions.delete(tabId);
      }
    }
  }

  snapshot() {
    return {
      cleanup_policy: this.cleanupPolicy,
      browserSockets: Array.from(this.browserSockets).filter(socket => socket.readyState === WebSocket.OPEN).length,
      tabs: Array.from(this.tabSessions.values()).map(session => session.snapshot()),
    };
  }
}

const sessionManager = new BrowserSessionManager();

function buildDiagnostics() {
  const manifestProbe = probeNativeManifest();
  const lockInfo = readBridgeLock();
  const extensionConnected = isExtensionConnected();
  const nativeVirtual = !!(extensionSocket && extensionSocket.isNative === true);
  const causes = [];

  if (!manifestProbe.registryPath) causes.push('manifest invalid: Native Messaging registry key missing');
  else if (!manifestProbe.manifestExists) causes.push('manifest invalid: manifest file missing');
  else if (!manifestProbe.hostPathExists) causes.push('manifest invalid: native host executable missing');
  else if (!manifestProbe.allowedOriginLikelyOk) causes.push('extension id refused: current extension id is not in allowed_origins');

  if (!extensionConnected) causes.push('host crashed or disconnected: extension is not connected to the proxy');
  if (nativeVirtual && !isNativeMode) causes.push('stdout protocol error: native virtual socket present outside native mode');

  return {
    ok: causes.length === 0 && extensionConnected,
    causes,
    checks: {
      host_crashed: !extensionConnected,
      manifest_invalid: !!manifestProbe.registryPath && (!manifestProbe.manifestExists || !manifestProbe.hostPathExists),
      extension_id_refused: !!manifestProbe.registryPath && manifestProbe.manifestExists && manifestProbe.allowedOriginLikelyOk === false,
      stdout_protocol_error: false,
    },
    native: {
      isNativeMode,
      virtualSocket: nativeVirtual,
      registryPath: manifestProbe.registryPath,
      manifestPath: manifestProbe.manifestPath,
      hostPath: manifestProbe.hostPath,
      manifestExists: manifestProbe.manifestExists,
      hostPathExists: manifestProbe.hostPathExists,
      allowedOrigins: manifestProbe.allowedOrigins,
    },
    lock: {
      path: LOCK_PATH,
      owner: lockInfo,
      thisPid: process.pid,
    },
    sessions: sessionManager.snapshot(),
    chromeExecutable: findChromeExecutable(),
  };
}

function probeNativeManifest() {
  const result = {
    registryPath: null,
    manifestPath: null,
    hostPath: null,
    manifestExists: false,
    hostPathExists: false,
    allowedOrigins: [],
    allowedOriginLikelyOk: null,
  };

  if (process.platform !== 'win32') return result;
  try {
    const { execFileSync } = require('child_process');
    const output = execFileSync('reg', ['query', 'HKCU\\Software\\Google\\Chrome\\NativeMessagingHosts\\com.sinew.chrome_bridge', '/ve'], { encoding: 'utf8', windowsHide: true });
    const match = output.match(/REG_SZ\s+(.+)$/m);
    if (match) {
      result.registryPath = match[1].trim();
      result.manifestPath = result.registryPath;
      result.manifestExists = fs.existsSync(result.manifestPath);
      if (result.manifestExists) {
        const manifest = JSON.parse(fs.readFileSync(result.manifestPath, 'utf8').replace(/^\uFEFF/, ''));
        result.hostPath = manifest.path || null;
        result.hostPathExists = !!(result.hostPath && fs.existsSync(result.hostPath));
        result.allowedOrigins = Array.isArray(manifest.allowed_origins) ? manifest.allowed_origins : [];
        result.allowedOriginLikelyOk = result.allowedOrigins.length > 0;
      }
    }
  } catch {}
  return result;
}

const server = http.createServer((req, res) => {
  const parsedUrl = url.parse(req.url, true);
  let pathname = parsedUrl.pathname;
  
  // Normalize trailing slashes
  if (pathname.endsWith('/') && pathname.length > 1) {
    pathname = pathname.slice(0, -1);
  }

  res.setHeader('Content-Type', 'application/json');
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'GET, OPTIONS');

  if (pathname === '/json/version') {
    // Return standard Chrome version metadata
    const versionInfo = {
      "Browser": "Chrome/120.0.0.0",
      "Protocol-Version": "1.3",
      "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
      "V8-Version": "12.0.267",
      "WebKit-Version": "537.36 (@a06414a2754673bc28ea7c71d60dd4d9c7af4718)",
      "webSocketDebuggerUrl": `ws://localhost:${PORT}/devtools/browser`
    };
    res.writeHead(200);
    res.end(JSON.stringify(versionInfo));
  } 
  else if (pathname === '/api/launch_chrome') {
    try {
      const launched = launchChromeDetached(parsedUrl.query.url || 'about:blank');
      res.writeHead(200);
      res.end(JSON.stringify({ success: true, ...launched }));
    } catch (e) {
      res.writeHead(200);
      res.end(JSON.stringify({ success: false, error: e.message }));
    }
  }
  else if (pathname === '/api/status') {
    const diagnostics = buildDiagnostics();
    res.writeHead(200);
    res.end(JSON.stringify({
      isNativeMode,
      extensionConnected: isExtensionConnected(),
      hasExtensionSocket: isExtensionConnected(),
      extensionSocketState: extensionSocket ? extensionSocket.readyState : null,
      extensionSocketIsVirtual: extensionSocket && extensionSocket.isNative === true,
      chromeExecutable: findChromeExecutable(),
      diagnostics,
      sessions: sessionManager.snapshot(),
    }));
  }
  else if (pathname === '/api/diagnostics') {
    res.writeHead(200);
    res.end(JSON.stringify(buildDiagnostics()));
  } 
  else if (pathname === '/json' || pathname === '/json/list') {
    // Return the list of open tabs that can be debugged
    if (!isExtensionConnected()) {
      console.log("âš ï¸ [Proxy] /json request received, but Chrome Extension is not connected.");
      res.writeHead(200);
      res.end(JSON.stringify([]));
      return;
    }

    // Ask extension for tabs
    const requestId = ++messageCounter;
    const tabPromise = new Promise((resolve) => {
      pendingRequests.set(requestId, {
        resolve: (data) => resolve(data.tabs || []),
        timeout: setTimeout(() => {
          pendingRequests.delete(requestId);
          resolve([]);
        }, 2000) // 2-second timeout
      });
    });

    extensionSocket.send(JSON.stringify({
      id: requestId,
      command: "list_tabs"
    }));

    tabPromise.then((tabs) => {
      // Filter out non-debuggable system tabs
      let debuggableTabs = tabs.filter(t => {
        const u = t.url || "";
        return !u.startsWith("chrome://") && 
               !u.startsWith("chrome-extension://") && 
               !u.startsWith("edge://") && 
               !u.startsWith("view-source:");
      });

      const formatAndSend = (list) => {
        const cdpTabs = list.map(t => ({
          "description": "",
          "devtoolsFrontendUrl": `devtools://devtools/bundled/js_app.html?experiments=true&v8only=true&ws=localhost:${PORT}/devtools/page/${t.id}`,
          "id": String(t.id),
          "title": t.title || "Chrome Tab",
          "type": "page",
          "url": t.url || "about:blank",
          "active": !!t.active,
          "webSocketDebuggerUrl": `ws://localhost:${PORT}/devtools/page/${t.id}`
        }));
        res.writeHead(200);
        res.end(JSON.stringify(cdpTabs));
      };

      if (debuggableTabs.length === 0) {
        console.log("ðŸ§¬ [Proxy] No debuggable tabs found. Programmatically creating a new tab...");
        const createRequestId = ++messageCounter;
        const createPromise = new Promise((resolve) => {
          pendingRequests.set(createRequestId, {
            resolve: (data) => resolve(data.tab ? [data.tab] : []),
            timeout: setTimeout(() => {
              pendingRequests.delete(createRequestId);
              resolve([]);
            }, 3000)
          });
        });

        extensionSocket.send(JSON.stringify({
          id: createRequestId,
          command: "create_tab",
          params: { url: "about:blank" }
        }));

        createPromise.then((newTabs) => {
          formatAndSend(newTabs);
        });
      } else {
        formatAndSend(debuggableTabs);
      }
    });
  } 
  else if (pathname === '/api/scan') {
    res.setHeader('Content-Type', 'application/json');
    try {
      const folderPath = parsedUrl.query.folder || path.join(homeDir, 'Downloads');
      const files = scanFolder(folderPath);
      res.writeHead(200);
      res.end(JSON.stringify({ success: true, files }));
    } catch (e) {
      res.writeHead(500);
      res.end(JSON.stringify({ success: false, error: e.message }));
    }
  }
  else if (pathname === '/api/sort') {
    res.setHeader('Content-Type', 'application/json');
    try {
      const source = parsedUrl.query.source;
      const targetDir = parsedUrl.query.dest || path.join(homeDir, 'OneDrive', 'Documents');
      const name = parsedUrl.query.name;
      
      if (!source || !name) {
        res.writeHead(400);
        res.end(JSON.stringify({ success: false, error: "Missing source or name parameters" }));
        return;
      }
      
      const newPath = sortFile(source, targetDir, name);
      res.writeHead(200);
      res.end(JSON.stringify({ success: true, newPath }));
    } catch (e) {
      res.writeHead(500);
      res.end(JSON.stringify({ success: false, error: e.message }));
    }
  }
  else if (pathname === '/api/sample') {
    res.setHeader('Content-Type', 'application/json');
    try {
      const file = parsedUrl.query.file;
      if (!file) {
        res.writeHead(400);
        res.end(JSON.stringify({ success: false, error: "Missing file parameter" }));
        return;
      }
      const sample = getFileSample(file);
      res.writeHead(200);
      res.end(JSON.stringify({ success: true, sample }));
    } catch (e) {
      res.writeHead(500);
      res.end(JSON.stringify({ success: false, error: e.message }));
    }
  }
  else if (pathname === '/api/macros/list') {
    res.setHeader('Content-Type', 'application/json');
    try {
      const fs = require('fs');
      const path = require('path');
      const macrosDir = path.join(homeDir, '.gemini', 'antigravity', 'scratch', 'macros');
      if (!fs.existsSync(macrosDir)) {
        res.writeHead(200);
        res.end(JSON.stringify({ success: true, macros: [] }));
        return;
      }
      const files = fs.readdirSync(macrosDir).filter(f => f.endsWith('.json'));
      const macros = files.map(file => {
        try {
          const content = JSON.parse(fs.readFileSync(path.join(macrosDir, file), 'utf-8'));
          return {
            name: file,
            title: content.title || file,
            url: content.url || "",
            stepsCount: content.steps ? content.steps.length : 0,
            timestamp: content.timestamp || Date.now()
          };
        } catch(e) {
          return { name: file, error: true };
        }
      });
      res.writeHead(200);
      res.end(JSON.stringify({ success: true, macros }));
    } catch (e) {
      res.writeHead(500);
      res.end(JSON.stringify({ success: false, error: e.message }));
    }
  }
  else if (pathname === '/api/macros/save') {
    res.setHeader('Content-Type', 'application/json');
    try {
      const macroData = parsedUrl.query.macro ? JSON.parse(parsedUrl.query.macro) : null;
      if (!macroData || !macroData.name) {
        res.writeHead(400);
        res.end(JSON.stringify({ success: false, error: "Missing macro payload or macro name" }));
        return;
      }
      
      const fs = require('fs');
      const path = require('path');
      const macrosDir = path.join(homeDir, '.gemini', 'antigravity', 'scratch', 'macros');
      if (!fs.existsSync(macrosDir)) {
        fs.mkdirSync(macrosDir, { recursive: true });
      }
      const filePath = path.join(macrosDir, macroData.name);
      fs.writeFileSync(filePath, JSON.stringify(macroData, null, 2), 'utf-8');
      
      const oneDriveDir = path.join(homeDir, 'OneDrive', 'Documents', 'macros');
      if (!fs.existsSync(oneDriveDir)) {
        fs.mkdirSync(oneDriveDir, { recursive: true });
      }
      fs.writeFileSync(path.join(oneDriveDir, macroData.name), JSON.stringify(macroData, null, 2), 'utf-8');
      
      res.writeHead(200);
      res.end(JSON.stringify({ success: true, path: filePath }));
    } catch(e) {
      res.writeHead(500);
      res.end(JSON.stringify({ success: false, error: e.message }));
    }
  }
  else if (pathname === '/api/macros/replay') {
    res.setHeader('Content-Type', 'application/json');
    try {
      const macroName = parsedUrl.query.name;
      const tabId = parsedUrl.query.tabId;
      
      if (!macroName || !tabId) {
        res.writeHead(400);
        res.end(JSON.stringify({ success: false, error: "Missing macro name or tabId parameter" }));
        return;
      }
      
      const fs = require('fs');
      const path = require('path');
      const macrosDir = path.join(homeDir, '.gemini', 'antigravity', 'scratch', 'macros');
      const macroPath = path.join(macrosDir, macroName);
      
      if (!fs.existsSync(macroPath)) {
        res.writeHead(404);
        res.end(JSON.stringify({ success: false, error: `Macro "${macroName}" not found.` }));
        return;
      }
      
      const macro = JSON.parse(fs.readFileSync(macroPath, 'utf-8'));
      
      executeMacroReplay(tabId, macro);
      res.writeHead(200);
      res.end(JSON.stringify({ success: true, message: `Replay launched for "${macroName}" on Tab ${tabId}` }));
    } catch(e) {
      res.writeHead(500);
      res.end(JSON.stringify({ success: false, error: e.message }));
    }
  }
  else if (pathname === '/api/detach_all') {
    res.setHeader('Content-Type', 'application/json');
    res.setHeader('Access-Control-Allow-Origin', '*');

    if (!isExtensionConnected()) {
      res.writeHead(200);
      res.end(JSON.stringify({ success: true, detached: 0, message: 'Extension not connected' }));
      return;
    }

    const requestId = ++messageCounter;
    pendingRequests.set(requestId, {
      resolve: (data) => {
        res.writeHead(200);
        res.end(JSON.stringify({ success: true, ...(data || {}) }));
      },
      timeout: setTimeout(() => {
        pendingRequests.delete(requestId);
        res.writeHead(200);
        res.end(JSON.stringify({ success: false, error: 'Timeout waiting for detach_all' }));
      }, 5000)
    });

    extensionSocket.send(JSON.stringify({ id: requestId, command: 'detach_all' }));
  }
  else if (pathname === '/api/navigate_tab') {
    res.setHeader('Content-Type', 'application/json');
    res.setHeader('Access-Control-Allow-Origin', '*');

    if (!isExtensionConnected()) {
      res.writeHead(200);
      res.end(JSON.stringify({ success: false, error: "Chrome Extension is not connected." }));
      return;
    }

    const tabId = parseInt(parsedUrl.query.tabId);
    const targetUrl = parsedUrl.query.url || 'about:blank';
    const requestId = ++messageCounter;
    pendingRequests.set(requestId, {
      resolve: (data) => { res.writeHead(200); res.end(JSON.stringify(data)); },
      timeout: setTimeout(() => {
        pendingRequests.delete(requestId);
        res.writeHead(200);
        res.end(JSON.stringify({ success: false, error: 'Timeout waiting for navigation' }));
      }, 7000)
    });
    extensionSocket.send(JSON.stringify({ id: requestId, command: 'navigate_tab', params: { tabId, url: targetUrl } }));
  }
  else if (pathname === '/api/detect_target') {
    res.setHeader('Content-Type', 'application/json');
    res.setHeader('Access-Control-Allow-Origin', '*');

    if (!isExtensionConnected()) {
      res.writeHead(200);
      res.end(JSON.stringify({ success: false, error: "Chrome Extension is not connected." }));
      return;
    }

    const tabId = parseInt(parsedUrl.query.tabId);
    const task = String(parsedUrl.query.task || '');
    const requestId = ++messageCounter;
    pendingRequests.set(requestId, {
      resolve: (data) => { res.writeHead(200); res.end(JSON.stringify(data)); },
      timeout: setTimeout(() => {
        pendingRequests.delete(requestId);
        res.writeHead(200);
        res.end(JSON.stringify({ success: false, error: 'Timeout waiting for target detection' }));
      }, 20000)
    });
    extensionSocket.send(JSON.stringify({ id: requestId, command: 'detect_target', params: { tabId, task } }));
  }
  else if (pathname === '/api/page_snapshot') {
    res.setHeader('Content-Type', 'application/json');
    res.setHeader('Access-Control-Allow-Origin', '*');

    if (!isExtensionConnected()) {
      res.writeHead(200);
      res.end(JSON.stringify({ success: false, error: "Chrome Extension is not connected." }));
      return;
    }

    const tabId = parseInt(parsedUrl.query.tabId);
    const limit = Math.max(1, Math.min(200, Number(parsedUrl.query.limit) || 80));
    const requestId = ++messageCounter;
    pendingRequests.set(requestId, {
      resolve: (data) => { res.writeHead(200); res.end(JSON.stringify(data)); },
      timeout: setTimeout(() => {
        pendingRequests.delete(requestId);
        res.writeHead(200);
        res.end(JSON.stringify({ success: false, error: 'Timeout waiting for page snapshot' }));
      }, 15000)
    });
    extensionSocket.send(JSON.stringify({ id: requestId, command: 'page_snapshot', params: { tabId, limit } }));
  }
  else if (pathname === '/api/query_selector' || pathname === '/api/click_selector' || pathname === '/api/type_selector' || pathname === '/api/wait_selector' || pathname === '/api/evaluate') {
    res.setHeader('Content-Type', 'application/json');
    res.setHeader('Access-Control-Allow-Origin', '*');

    if (!isExtensionConnected()) {
      res.writeHead(200);
      res.end(JSON.stringify({ success: false, error: "Chrome Extension is not connected." }));
      return;
    }

    const tabId = parseInt(parsedUrl.query.tabId);
    const timeoutMs = Math.max(1000, Math.min(60000, Number(parsedUrl.query.timeoutMs) || 12000));
    const requestId = ++messageCounter;
    const command = pathname.slice('/api/'.length);
    const params = {
      tabId,
      selector: parsedUrl.query.selector || '',
      text: parsedUrl.query.text || '',
      expression: parsedUrl.query.expression || '',
      submit: /^(1|true|yes)$/i.test(String(parsedUrl.query.submit || '')),
      visible: !/^(0|false|no)$/i.test(String(parsedUrl.query.visible || '')),
      scroll: !/^(0|false|no)$/i.test(String(parsedUrl.query.scroll || '')),
      timeoutMs
    };
    pendingRequests.set(requestId, {
      resolve: (data) => { res.writeHead(200); res.end(JSON.stringify(data)); },
      timeout: setTimeout(() => {
        pendingRequests.delete(requestId);
        res.writeHead(200);
        res.end(JSON.stringify({ success: false, error: `Timeout waiting for ${command}` }));
      }, timeoutMs + 1000)
    });
    extensionSocket.send(JSON.stringify({ id: requestId, command, params }));
  }
  else if (pathname === '/api/human_click') {
    res.setHeader('Content-Type', 'application/json');
    res.setHeader('Access-Control-Allow-Origin', '*');

    if (!isExtensionConnected()) {
      res.writeHead(200);
      res.end(JSON.stringify({ success: false, error: "Chrome Extension is not connected." }));
      return;
    }

    const tabId = parseInt(parsedUrl.query.tabId);
    let detection = null;
    let cursor = null;
    try { detection = JSON.parse(String(parsedUrl.query.detection || '{}')); } catch {}
    try { cursor = JSON.parse(String(parsedUrl.query.cursor || '{}')); } catch {}
    const requestId = ++messageCounter;
    pendingRequests.set(requestId, {
      resolve: (data) => { res.writeHead(200); res.end(JSON.stringify(data)); },
      timeout: setTimeout(() => {
        pendingRequests.delete(requestId);
        res.writeHead(200);
        res.end(JSON.stringify({ success: false, error: 'Timeout waiting for human click' }));
      }, 20000)
    });
    extensionSocket.send(JSON.stringify({ id: requestId, command: 'human_click', params: { tabId, detection, cursor } }));
  }
  else if (pathname === '/api/create_tab') {
    res.setHeader('Content-Type', 'application/json');
    res.setHeader('Access-Control-Allow-Origin', '*');

    if (!isExtensionConnected()) {
      res.writeHead(200);
      res.end(JSON.stringify({ success: false, error: "Chrome Extension is not connected." }));
      return;
    }

    const targetUrl = parsedUrl.query.url || "https://www.google.com";
    const requestId = ++messageCounter;
    pendingRequests.set(requestId, {
      resolve: (data) => {
        res.writeHead(200);
        res.end(JSON.stringify(data));
      },
      timeout: setTimeout(() => {
        pendingRequests.delete(requestId);
        res.writeHead(200);
        res.end(JSON.stringify({ success: false, error: "Timeout waiting for tab creation" }));
      }, 5000)
    });

    extensionSocket.send(JSON.stringify({
      id: requestId,
      command: "create_tab",
      params: { url: targetUrl }
    }));
  }
  else if (pathname === '/api/execute_silent_task') {
    res.setHeader('Content-Type', 'application/json');
    res.setHeader('Access-Control-Allow-Origin', '*');
    
    const tabId = parsedUrl.query.tabId;
    const task = parsedUrl.query.task;
    let cursor = null;
    try { cursor = parsedUrl.query.cursor ? JSON.parse(String(parsedUrl.query.cursor)) : null; } catch {}
    
    if (!tabId || !task) {
      res.writeHead(400);
      res.end(JSON.stringify({ success: false, error: "Missing tabId or task parameter" }));
      return;
    }
    
    if (!isExtensionConnected()) {
      res.writeHead(200);
      res.end(JSON.stringify({ success: false, error: "Chrome Extension is not connected." }));
      return;
    }
    
    const requestId = ++messageCounter;
    pendingRequests.set(requestId, {
      resolve: (data) => {
        res.writeHead(200);
        res.end(JSON.stringify(data));
      },
      timeout: setTimeout(() => {
        pendingRequests.delete(requestId);
        res.writeHead(200);
        res.end(JSON.stringify({ success: false, error: "Timeout waiting for extension response" }));
      }, 15000)
    });
    
    extensionSocket.send(JSON.stringify({
      id: requestId,
      command: "execute_silent_task",
      params: { tabId: parseInt(tabId), task, cursor }
    }));
  }
  else if (pathname === '/cowork') {
    res.setHeader('Content-Type', 'text/html; charset=utf-8');
    const html = `
<!DOCTYPE html>
<html lang="fr" data-theme="light">
<head>
  <meta charset="UTF-8">
  <title>ðŸ§¬ Sinew Cowork â€” Cockpit de ContrÃ´le</title>
  <style>
    @import url('https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@300;400;500;600;700&family=Outfit:wght@300;400;500;600;700&family=Share+Tech+Mono&display=swap');
    
    :root {
      /* Elegant Warm Ivory Theme (Claude Signature Style) */
      --bg-main: #fbfaf7;
      --bg-sidebar: #f5f2eb;
      --panel-bg: #ffffff;
      --border-color: #e6e2da;
      --text-main: #1f1f1d;
      --text-muted: #6e6b64;
      --accent-color: #cc5a01; /* Warm amber/terracotta */
      --accent-hover: #b34e00;
      --chat-user-bg: #efebe2;
      --chat-agent-bg: #ffffff;
      --shadow-sm: 0 1px 3px rgba(0, 0, 0, 0.05);
      --shadow-md: 0 4px 20px rgba(0, 0, 0, 0.06);
      --radius-lg: 12px;
      --radius-md: 8px;
    }

    [data-theme="dark"] {
      /* Ultra-Premium Dark Velvet Theme */
      --bg-main: #0a0a0c;
      --bg-sidebar: #111115;
      --panel-bg: #17171c;
      --border-color: #26262e;
      --text-main: #e2e8f0;
      --text-muted: #8fa0b0;
      --accent-color: #ff6b00;
      --accent-hover: #ff8533;
      --chat-user-bg: #22222b;
      --chat-agent-bg: #1c1c24;
      --shadow-sm: 0 1px 3px rgba(0, 0, 0, 0.3);
      --shadow-md: 0 8px 30px rgba(0, 0, 0, 0.4);
    }

    * {
      box-sizing: border-box;
      margin: 0;
      padding: 0;
    }

    body {
      background-color: var(--bg-main);
      color: var(--text-main);
      font-family: 'Plus Jakarta Sans', sans-serif;
      height: 100vh;
      display: flex;
      flex-direction: column;
      overflow: hidden;
      transition: background-color 0.3s, color 0.3s;
    }

    header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      padding: 0 30px;
      background: var(--panel-bg);
      border-bottom: 1px solid var(--border-color);
      height: 70px;
      box-shadow: var(--shadow-sm);
      z-index: 10;
    }

    .brand {
      display: flex;
      align-items: center;
      gap: 12px;
    }

    .brand-logo {
      width: 28px;
      height: 28px;
      background: var(--accent-color);
      border-radius: 6px;
      display: flex;
      align-items: center;
      justify-content: center;
      box-shadow: 0 0 10px rgba(204, 90, 1, 0.2);
    }

    .brand-logo-inner {
      width: 12px;
      height: 12px;
      border: 2px solid var(--panel-bg);
      border-radius: 3px;
      transform: rotate(45deg);
    }

    .brand-title {
      font-family: 'Outfit', sans-serif;
      font-size: 18px;
      font-weight: 600;
      letter-spacing: 1px;
    }

    .header-actions {
      display: flex;
      align-items: center;
      gap: 15px;
    }

    .status-badge {
      display: flex;
      align-items: center;
      gap: 8px;
      font-family: 'Share Tech Mono', monospace;
      font-size: 12px;
      color: var(--accent-color);
    }

    .status-dot {
      width: 8px;
      height: 8px;
      background: var(--accent-color);
      border-radius: 50%;
      box-shadow: 0 0 8px var(--accent-color);
      animation: pulse 1.5s infinite alternate;
    }

    @keyframes pulse {
      0% { opacity: 0.4; }
      100% { opacity: 1; }
    }

    .theme-btn {
      background: var(--bg-sidebar);
      border: 1px solid var(--border-color);
      color: var(--text-main);
      padding: 6px 12px;
      border-radius: var(--radius-md);
      cursor: pointer;
      font-family: 'Outfit', sans-serif;
      font-size: 11px;
      font-weight: 600;
      transition: all 0.2s;
    }

    .theme-btn:hover {
      border-color: var(--accent-color);
      color: var(--accent-color);
    }

    .main-container {
      display: flex;
      flex: 1;
      height: calc(100vh - 70px);
      overflow: hidden;
    }

    /* Left Sidebar: Conversational Agent Chat */
    .chat-sidebar {
      width: 380px;
      background: var(--bg-sidebar);
      border-right: 1px solid var(--border-color);
      display: flex;
      flex-direction: column;
      height: 100%;
    }

    .chat-header {
      padding: 20px;
      border-bottom: 1px solid var(--border-color);
    }

    .chat-header-title {
      font-family: 'Outfit', sans-serif;
      font-size: 14px;
      font-weight: 600;
      letter-spacing: 0.5px;
      color: var(--text-muted);
      text-transform: uppercase;
    }

    .chat-messages {
      flex: 1;
      overflow-y: auto;
      padding: 20px;
      display: flex;
      flex-direction: column;
      gap: 12px;
    }

    /* Custom high-end scrollbars for various scroll areas */
    .chat-messages::-webkit-scrollbar, .list-wrapper::-webkit-scrollbar, .sample-container::-webkit-scrollbar {
      width: 6px;
      height: 6px;
    }
    .chat-messages::-webkit-scrollbar-track, .list-wrapper::-webkit-scrollbar-track, .sample-container::-webkit-scrollbar-track {
      background: transparent;
    }
    .chat-messages::-webkit-scrollbar-thumb, .list-wrapper::-webkit-scrollbar-thumb, .sample-container::-webkit-scrollbar-thumb {
      background: var(--border-color);
      border-radius: 3px;
    }
    .chat-messages::-webkit-scrollbar-thumb:hover, .list-wrapper::-webkit-scrollbar-thumb:hover, .sample-container::-webkit-scrollbar-thumb:hover {
      background: var(--accent-color);
    }

    code {
      font-family: 'Share Tech Mono', monospace;
      font-size: 11.5px;
      background: rgba(0, 0, 0, 0.05);
      padding: 2px 6px;
      border-radius: 4px;
      color: var(--accent-color);
      border: 1px solid var(--border-color);
    }
    [data-theme="dark"] code {
      background: rgba(255, 255, 255, 0.08);
      color: var(--accent-color);
    }

    .chat-bubble {
      padding: 12px 16px;
      border-radius: var(--radius-md);
      font-size: 13.5px;
      line-height: 1.5;
      max-width: 85%;
      box-shadow: var(--shadow-sm);
      word-break: break-word;
      overflow-wrap: anywhere;
    }

    .chat-bubble.user {
      align-self: flex-end;
      background: var(--chat-user-bg);
      color: var(--text-main);
      border-bottom-right-radius: 2px;
      border: 1px solid var(--border-color);
    }

    .chat-bubble.agent {
      align-self: flex-start;
      background: var(--chat-agent-bg);
      color: var(--text-main);
      border-bottom-left-radius: 2px;
      border: 1px solid var(--border-color);
    }

    .chat-bubble.log-entry {
      align-self: center;
      background: rgba(0, 0, 0, 0.02);
      border: 1px dashed var(--border-color);
      color: var(--text-muted);
      font-family: 'Share Tech Mono', monospace;
      font-size: 11px;
      max-width: 100%;
      width: 100%;
      border-radius: 4px;
      box-shadow: none;
      word-break: break-all;
      white-space: normal;
    }

    [data-theme="dark"] .chat-bubble.log-entry {
      background: rgba(255, 255, 255, 0.02);
    }

    .typing-indicator {
      display: flex;
      gap: 4px;
      padding: 12px 16px;
      align-self: flex-start;
      background: var(--chat-agent-bg);
      border-radius: var(--radius-md);
      border: 1px solid var(--border-color);
      box-shadow: var(--shadow-sm);
    }

    .typing-dot {
      width: 6px;
      height: 6px;
      background: var(--text-muted);
      border-radius: 50%;
      animation: typing 1.4s infinite ease-in-out both;
    }

    .typing-dot:nth-child(2) { animation-delay: 0.2s; }
    .typing-dot:nth-child(3) { animation-delay: 0.4s; }

    @keyframes typing {
      0%, 80%, 100% { transform: scale(0); }
      40% { transform: scale(1); }
    }

    .chat-pills {
      padding: 10px 20px;
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      border-top: 1px solid var(--border-color);
    }

    .chat-pill {
      background: var(--panel-bg);
      border: 1px solid var(--border-color);
      color: var(--text-main);
      padding: 6px 12px;
      border-radius: 16px;
      font-size: 11px;
      cursor: pointer;
      font-weight: 500;
      transition: all 0.2s;
    }

    .chat-pill:hover {
      border-color: var(--accent-color);
      color: var(--accent-color);
      background: var(--bg-main);
    }

    .chat-input-container {
      padding: 15px 20px;
      border-top: 1px solid var(--border-color);
      display: flex;
      gap: 8px;
    }

    .chat-input {
      flex: 1;
      background: var(--panel-bg);
      border: 1px solid var(--border-color);
      border-radius: var(--radius-md);
      color: var(--text-main);
      font-family: inherit;
      font-size: 13.5px;
      padding: 10px 14px;
      outline: none;
      transition: border-color 0.2s;
      resize: none;
      height: 40px;
    }

    .chat-input:focus {
      border-color: var(--accent-color);
    }

    .chat-send-btn {
      background: var(--accent-color);
      border: none;
      color: #fff;
      padding: 0 16px;
      border-radius: var(--radius-md);
      cursor: pointer;
      font-weight: 600;
      font-size: 12px;
      transition: background 0.2s;
    }

    .chat-send-btn:hover {
      background: var(--accent-hover);
    }

    /* Right Main Panel: Workspace tabs */
    .workspace-area {
      flex: 1;
      display: flex;
      flex-direction: column;
      height: 100%;
      overflow: hidden;
    }

    .tabs-bar {
      display: flex;
      background: var(--panel-bg);
      border-bottom: 1px solid var(--border-color);
      padding: 0 30px;
      gap: 25px;
      height: 50px;
    }

    .tab-btn {
      background: none;
      border: none;
      color: var(--text-muted);
      font-family: 'Outfit', sans-serif;
      font-size: 13px;
      font-weight: 600;
      cursor: pointer;
      position: relative;
      height: 100%;
      display: flex;
      align-items: center;
      transition: color 0.2s;
      letter-spacing: 0.5px;
    }

    .tab-btn:hover {
      color: var(--text-main);
    }

    .tab-btn.active {
      color: var(--accent-color);
    }

    .tab-btn.active::after {
      content: "";
      position: absolute;
      bottom: 0;
      left: 0;
      right: 0;
      height: 2px;
      background: var(--accent-color);
    }

    .tab-content {
      flex: 1;
      padding: 30px;
      overflow-y: auto;
      display: none;
    }

    .tab-content.active {
      display: block;
    }

    /* Premium Grid & Panels */
    .grid-layout {
      display: grid;
      grid-template-columns: 1.8fr 1.2fr;
      gap: 30px;
    }

    .panel-card {
      background: var(--panel-bg);
      border: 1px solid var(--border-color);
      border-radius: var(--radius-lg);
      padding: 25px;
      box-shadow: var(--shadow-sm);
      margin-bottom: 30px;
    }

    .panel-card-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      border-bottom: 1px solid var(--border-color);
      padding-bottom: 15px;
      margin-bottom: 20px;
    }

    .panel-card-title {
      font-family: 'Outfit', sans-serif;
      font-size: 15px;
      font-weight: 600;
      letter-spacing: 0.5px;
      color: var(--text-main);
      display: flex;
      align-items: center;
      gap: 8px;
    }

    .btn-action {
      background: var(--bg-sidebar);
      border: 1px solid var(--border-color);
      color: var(--text-main);
      padding: 8px 16px;
      border-radius: var(--radius-md);
      cursor: pointer;
      font-family: 'Outfit', sans-serif;
      font-size: 11px;
      font-weight: 600;
      transition: all 0.2s;
    }

    .btn-action:hover {
      border-color: var(--accent-color);
      color: var(--accent-color);
      background: var(--panel-bg);
    }

    .btn-primary {
      background: var(--accent-color);
      border: 1px solid transparent;
      color: #fff;
    }

    .btn-primary:hover {
      background: var(--accent-hover);
      color: #fff;
    }

    /* Lists and items */
    .list-wrapper {
      display: flex;
      flex-direction: column;
      gap: 12px;
      max-height: 450px;
      overflow-y: auto;
    }

    .item-card {
      display: flex;
      justify-content: space-between;
      align-items: center;
      background: var(--panel-bg);
      border: 1px solid var(--border-color);
      padding: 14px 20px;
      border-radius: var(--radius-md);
      transition: all 0.2s;
      cursor: pointer;
    }

    .item-card:hover {
      border-color: var(--accent-color);
      box-shadow: var(--shadow-sm);
      transform: translateY(-1px);
    }

    .item-card.active {
      border-color: var(--accent-color);
      background: var(--bg-sidebar);
    }

    .item-info {
      display: flex;
      flex-direction: column;
      gap: 4px;
    }

    .item-title {
      font-size: 13.5px;
      font-weight: 600;
      color: var(--text-main);
    }

    .item-meta {
      font-family: 'Share Tech Mono', monospace;
      font-size: 11px;
      color: var(--text-muted);
    }

    .form-group {
      display: flex;
      flex-direction: column;
      gap: 8px;
      margin-bottom: 18px;
    }

    label {
      font-family: 'Outfit', sans-serif;
      font-size: 11px;
      font-weight: 600;
      color: var(--text-muted);
      letter-spacing: 0.5px;
      text-transform: uppercase;
    }

    input, select {
      background: var(--panel-bg);
      border: 1px solid var(--border-color);
      border-radius: var(--radius-md);
      color: var(--text-main);
      font-family: inherit;
      font-size: 13px;
      padding: 10px 14px;
      width: 100%;
      outline: none;
    }

    input:focus, select:focus {
      border-color: var(--accent-color);
    }

    .sample-container {
      background: var(--bg-sidebar);
      border: 1px solid var(--border-color);
      border-radius: var(--radius-md);
      font-family: 'Share Tech Mono', monospace;
      font-size: 11.5px;
      padding: 15px;
      max-height: 200px;
      overflow-y: auto;
      color: var(--text-main);
      white-space: pre-wrap;
      word-break: break-all;
    }

    .empty-state {
      text-align: center;
      padding: 50px 20px;
      color: var(--text-muted);
      font-family: 'Plus Jakarta Sans', sans-serif;
      font-size: 13px;
      border: 1px dashed var(--border-color);
      border-radius: var(--radius-md);
    }

    /* Premium Responsive Adaptation (avoid squishing the layout) */
    @media (max-width: 1024px) {
      .main-container {
        flex-direction: column;
        overflow-y: auto;
      }
      .chat-sidebar {
        width: 100%;
        height: 480px;
        border-right: none;
        border-bottom: 1px solid var(--border-color);
      }
      .workspace-area {
        height: auto;
        overflow: visible;
      }
      .grid-layout {
        grid-template-columns: 1fr;
        gap: 20px;
      }
    }
  </style>
</head>
<body>

  <header>
    <div class="brand">
      <div class="brand-logo">
        <div class="brand-logo-inner"></div>
      </div>
      <h1 class="brand-title">Sinew COWORK</h1>
    </div>
    <div class="header-actions">
      <div class="status-badge">
        <div class="status-dot"></div>
        <span>COMPAGNON SYMBIOTIQUE ACTIF</span>
      </div>
      <button class="theme-btn" id="themeToggleBtn" onclick="toggleTheme()">ðŸŒ™ SOMBRE</button>
    </div>
  </header>

  <div class="main-container">
    <!-- Left Sidebar: Sinew conversational chat -->
    <div class="chat-sidebar">
      <div class="chat-header">
        <h2 class="chat-header-title">Console Assistant</h2>
      </div>
      <div class="chat-messages" id="chatMessages">
        <div class="chat-bubble agent">
          Bonjour Julien. Je suis <strong>Sinew</strong>, votre co-worker sÃ©mantique souverain. J'ai un accÃ¨s direct Ã  vos fichiers tÃ©lÃ©chargÃ©s, votre OneDrive et votre session Chrome. Que souhaitez-vous accomplir aujourd'hui ?
        </div>
      </div>
      <div class="chat-pills">
        <button class="chat-pill" onclick="triggerPill('scan')">ðŸ“¥ Scanner Downloads</button>
        <button class="chat-pill" onclick="triggerPill('macros')">ðŸŽ¬ Charger mes Macros</button>
        <button class="chat-pill" onclick="triggerPill('aide')">ðŸ’¡ Aide Widget</button>
      </div>
      <div class="chat-input-container">
        <input type="text" class="chat-input" id="chatInput" placeholder="Demandez une action..." onkeydown="if(event.key==='Enter') handleChatSubmit()" />
        <button class="chat-send-btn" onclick="handleChatSubmit()">Envoyer</button>
      </div>
    </div>

    <!-- Right Workspace Area -->
    <div class="workspace-area">
      <div class="tabs-bar">
        <button class="tab-btn active" id="tab-organizer" onclick="switchTab('organizer')">ðŸ“ ORGANISATEUR DE DISQUE</button>
        <button class="tab-btn" id="tab-playbooks" onclick="switchTab('playbooks')">ðŸŽ¬ PLAYBOOKS DE NAVIGATION</button>
      </div>

      <!-- Tab 1: Disk Organizer -->
      <div class="tab-content active" id="content-organizer">
        <div class="grid-layout">
          <!-- Files scanner -->
          <div class="panel-card">
            <div class="panel-card-header">
              <h3 class="panel-card-title">ðŸ“¥ File Janitor â€” Fichiers RÃ©cents</h3>
              <button class="btn-action" onclick="loadFiles()">Scanner ðŸ”„</button>
            </div>
            <div id="fileContainer" class="list-wrapper">
              <div class="empty-state">Cliquez sur Scanner pour analyser le rÃ©pertoire de tÃ©lÃ©chargements.</div>
            </div>
          </div>

          <!-- File actions / sorting drawer -->
          <div class="panel-card">
            <div class="panel-card-header">
              <h3 class="panel-card-title">ðŸŽ¯ Tri & Rangement SÃ©mantique</h3>
            </div>
            <div id="sortForm" style="display: none;">
              <div class="form-group">
                <label>Fichier sÃ©lectionnÃ©</label>
                <input type="text" id="selectedFileName" disabled />
              </div>

              <div class="form-group">
                <label>Ã‰chantillon (OCR/Text)</label>
                <div id="fileSample" class="sample-container">Chargement...</div>
              </div>

              <div class="form-group">
                <label>Nouveau Nom NormalisÃ©</label>
                <input type="text" id="newFileName" placeholder="EDF_Facture_Mai_2026.pdf" />
              </div>

              <div class="form-group">
                <label>Dossier Cible (OneDrive)</label>
                <select id="targetDir">
                  <option value="C:\\Users\\julie\\OneDrive\\Documents\\Factures">ðŸ“ OneDrive / Documents / Factures</option>
                  <option value="C:\\Users\\julie\\OneDrive\\Documents\\Contrats">ðŸ“ OneDrive / Documents / Contrats</option>
                  <option value="C:\\Users\\julie\\OneDrive\\Documents\\Projets">ðŸ“ OneDrive / Documents / Projets</option>
                  <option value="C:\\Users\\julie\\OneDrive\\Documents\\Images">ðŸ“ OneDrive / Documents / Images</option>
                  <option value="C:\\Users\\julie\\OneDrive\\Documents">ðŸ“ OneDrive / Documents (Racine)</option>
                </select>
              </div>

              <button class="btn-action btn-primary" style="width: 100%; padding: 12px;" onclick="executeSort()">Classer & Archiver ðŸš€</button>
            </div>
            <div id="noSelectionMessage" class="empty-state">
              SÃ©lectionnez un fichier pour l'analyser et l'archiver sÃ©mantiquement.
            </div>
          </div>
        </div>
      </div>

      <!-- Tab 2: Macro Automation -->
      <div class="tab-content" id="content-playbooks">
        <div class="panel-card">
          <div class="panel-card-header">
            <h3 class="panel-card-title">ðŸŽ¥ Macro Automation â€” Playbooks</h3>
            <div style="display: flex; gap: 15px; align-items: center;">
              <label style="margin:0;">Cible de rejeu :</label>
              <select id="replayTabId" style="width: 250px; padding: 6px 12px; background: var(--panel-bg); border: 1px solid var(--border-color); border-radius: var(--radius-md); color: var(--text-main);">
                <!-- Populated dynamically -->
              </select>
              <button class="btn-action" onclick="loadMacros()">Actualiser ðŸ”„</button>
            </div>
          </div>

          <div id="macroContainer" class="list-wrapper">
            <div class="empty-state">Aucun playbook enregistrÃ© dans le workspace local.</div>
          </div>
        </div>
      </div>
    </div>
  </div>

  <script>
    let activeFiles = [];
    let selectedFile = null;

    // Conversational Chat helper
    function appendChatMessage(sender, text, isLog = false) {
      const container = document.getElementById('chatMessages');
      const bubble = document.createElement('div');
      bubble.className = 'chat-bubble ' + sender + (isLog ? ' log-entry' : '');
      bubble.innerHTML = text;
      container.appendChild(bubble);
      container.scrollTop = container.scrollHeight;
    }

    function showTypingIndicator() {
      const container = document.getElementById('chatMessages');
      const ind = document.createElement('div');
      ind.className = 'typing-indicator';
      ind.id = 'chatTypingIndicator';
      ind.innerHTML = '<div class="typing-dot"></div><div class="typing-dot"></div><div class="typing-dot"></div>';
      container.appendChild(ind);
      container.scrollTop = container.scrollHeight;
    }

    function hideTypingIndicator() {
      const ind = document.getElementById('chatTypingIndicator');
      if (ind) ind.remove();
    }

    async function handleChatSubmit() {
      const input = document.getElementById('chatInput');
      const text = input.value.trim();
      if (!text) return;
      
      input.value = '';
      appendChatMessage('user', text);
      showTypingIndicator();
      
      const lower = text.toLowerCase();
      setTimeout(async () => {
        if (lower.includes('scan') || lower.includes('tÃ©lÃ©chargement') || lower.includes('downloads')) {
          appendChatMessage('agent', 'ðŸ” DÃ©marrage du scan sÃ©mantique de vos tÃ©lÃ©chargements...');
          await loadFiles();
        } else if (lower.includes('macro') || lower.includes('playbook') || lower.includes('rejeu')) {
          appendChatMessage('agent', 'ðŸŽ¬ RÃ©cupÃ©ration de la liste de vos playbooks dans le workspace...');
          switchTab('playbooks');
          await loadMacros();
        } else if (lower.includes('aide') || lower.includes('widget') || lower.includes('enregistrer')) {
          appendChatMessage('agent', 'ðŸ’¡ <strong>Guide Macro Widget :</strong><br>1. Allez sur n\'importe quel site web (ex: Google).<br>2. Cliquez sur le widget nÃ©on en bas Ã  droite.<br>3. Cliquez sur <strong>REC</strong>, rÃ©alisez vos clics et saisies, puis cliquez sur <strong>STOP</strong>.<br>4. Nommez la macro et enregistrez-la !');
        } else {
          appendChatMessage('agent', 'Je reste Ã  votre Ã©coute pour trier vos fichiers dans le OneDrive ou orchestrer votre navigateur Chrome.');
        }
        hideTypingIndicator();
      }, 700);
    }

    async function triggerPill(action) {
      if (action === 'scan') {
        appendChatMessage('user', 'Scanner mes tÃ©lÃ©chargements');
        showTypingIndicator();
        setTimeout(async () => {
          appendChatMessage('agent', 'ðŸ” Scan sÃ©mantique en cours...');
          switchTab('organizer');
          await loadFiles();
          hideTypingIndicator();
        }, 500);
      } else if (action === 'macros') {
        appendChatMessage('user', 'Charger mes Macros');
        showTypingIndicator();
        setTimeout(async () => {
          appendChatMessage('agent', 'ðŸŽ¬ Chargement de vos playbooks de navigation...');
          switchTab('playbooks');
          await loadMacros();
          hideTypingIndicator();
        }, 500);
      } else if (action === 'aide') {
        appendChatMessage('user', 'Comment utiliser le widget macro ?');
        showTypingIndicator();
        setTimeout(() => {
          appendChatMessage('agent', 'ðŸ’¡ <strong>Guide Macro Widget :</strong><br>1. Allez sur n\'importe quel site web.<br>2. Cliquez sur le widget nÃ©on en bas Ã  droite.<br>3. Cliquez sur <strong>REC</strong>, rÃ©alisez vos actions, puis cliquez sur <strong>STOP</strong>.<br>4. Nommez et cliquez sur <strong>SAUVER</strong> pour la retrouver ici !');
          hideTypingIndicator();
        }, 500);
      }
    }

    function toggleTheme() {
      const doc = document.documentElement;
      const current = doc.getAttribute('data-theme');
      const next = current === 'dark' ? 'light' : 'dark';
      doc.setAttribute('data-theme', next);
      
      const btn = document.getElementById('themeToggleBtn');
      btn.textContent = next === 'dark' ? 'â˜€ï¸ CLAIR' : 'ðŸŒ™ SOMBRE';
    }

    function switchTab(tab) {
      document.querySelectorAll('.tab-btn').forEach(btn => btn.classList.remove('active'));
      document.querySelectorAll('.tab-content').forEach(content => content.classList.remove('active'));
      
      if (tab === 'organizer') {
        document.getElementById('tab-organizer').classList.add('active');
        document.getElementById('content-organizer').classList.add('active');
      } else if (tab === 'playbooks') {
        document.getElementById('tab-playbooks').classList.add('active');
        document.getElementById('content-playbooks').classList.add('active');
      }
    }

    async function loadFiles() {
      const container = document.getElementById('fileContainer');
      container.innerHTML = '<div class="empty-state">Scan en cours...</div>';
      
      try {
        const response = await fetch('/api/scan');
        const data = await response.json();
        
        if (data.success && data.files.length > 0) {
          activeFiles = data.files;
          container.innerHTML = '';
          
          data.files.forEach(file => {
            const sizeKb = Math.round(file.sizeBytes / 1024);
            const dateStr = new Date(file.modifiedTime).toLocaleDateString('fr-FR', {
              hour: '2-digit', minute: '2-digit'
            });

            const item = document.createElement('div');
            item.className = 'item-card';
            item.onclick = () => selectFile(file, item);
            item.innerHTML = \`
              <div class="item-info">
                <span class="item-title">\${file.name}</span>
                <span class="item-meta">\${file.extension.toUpperCase()} | \${sizeKb} KB | \${dateStr}</span>
              </div>
              <button class="btn-action" style="padding: 4px 10px; font-size: 10px;">SÃ©lectionner</button>
            \`;
            container.appendChild(item);
          });
          appendChatMessage('agent', 'âœ… Scan complÃ©tÃ©. J\'ai trouvÃ© <strong>' + data.files.length + ' fichiers</strong> non classÃ©s dans Downloads.', true);
        } else {
          container.innerHTML = '<div class="empty-state">Downloads vide. ZÃ©ro fichier non classÃ© ! ðŸŽ‰</div>';
          document.getElementById('sortForm').style.display = 'none';
          document.getElementById('noSelectionMessage').style.display = 'block';
          appendChatMessage('agent', 'ðŸ§¹ Votre rÃ©pertoire de tÃ©lÃ©chargements est propre. Aucun fichier non classÃ© !', true);
        }
      } catch (e) {
        container.innerHTML = '<div class="empty-state">Erreur lors du scan : ' + e.message + '</div>';
        appendChatMessage('agent', 'âŒ Ã‰chec du scan : ' + e.message, true);
      }
    }

    async function selectFile(file, element) {
      selectedFile = file;
      document.querySelectorAll('.item-card').forEach(el => el.classList.remove('active'));
      element.classList.add('active');

      document.getElementById('noSelectionMessage').style.display = 'none';
      document.getElementById('sortForm').style.display = 'block';
      
      document.getElementById('selectedFileName').value = file.name;
      document.getElementById('newFileName').value = file.name;

      const sampleBox = document.getElementById('fileSample');
      sampleBox.textContent = "Analyse et extraction textuelle...";

      try {
        const res = await fetch('/api/sample?file=' + encodeURIComponent(file.path));
        const data = await res.json();
        if (data.success) {
          sampleBox.textContent = data.sample || "[Fichier vide]";
          appendChatMessage('agent', 'ðŸ“ Contenu de <strong>' + file.name + '</strong> analysÃ©. Remplissez le nom normalisÃ© pour classer.', true);
        } else {
          sampleBox.textContent = "Erreur d\'Ã©chantillonnage : " + data.error;
        }
      } catch (e) {
        sampleBox.textContent = "Erreur rÃ©seau d\'Ã©chantillonnage.";
      }
    }

    async function executeSort() {
      if (!selectedFile) return;

      const newName = document.getElementById('newFileName').value.trim();
      const dest = document.getElementById('targetDir').value;

      if (!newName) {
        alert("Veuillez saisir un nom propre.");
        return;
      }

      appendChatMessage('agent', 'ðŸ’¾ DÃ©placement sÃ©mantique du fichier vers votre OneDrive...');
      try {
        const res = await fetch('/api/sort?source=' + encodeURIComponent(selectedFile.path) + '&dest=' + encodeURIComponent(dest) + '&name=' + encodeURIComponent(newName));
        const data = await res.json();

        if (data.success) {
          appendChatMessage('agent', 'âœ… Fichier archivÃ© avec succÃ¨s : <strong>' + newName + '</strong> dans <code>' + dest.split('\\\\').pop() + '</code>', true);
          loadFiles();
        } else {
          appendChatMessage('agent', 'âŒ Erreur d\'archivage : ' + data.error, true);
          alert("Erreur lors du classement : " + data.error);
        }
      } catch (e) {
        appendChatMessage('agent', 'âŒ Erreur rÃ©seau lors de l\'archivage.', true);
      }
    }

    async function loadMacros() {
      const container = document.getElementById('macroContainer');
      container.innerHTML = '<div class="empty-state">Chargement des playbooks...</div>';
      
      try {
        const tabsRes = await fetch('/json/list');
        const tabs = await tabsRes.json();
        const tabSelect = document.getElementById('replayTabId');
        tabSelect.innerHTML = '';
        if (tabs.length === 0) {
          const opt = document.createElement('option');
          opt.value = '';
          opt.textContent = 'Aucun onglet debuggable';
          tabSelect.appendChild(opt);
        } else {
          tabs.forEach(t => {
            const opt = document.createElement('option');
            opt.value = t.id;
            opt.textContent = t.title.slice(0,35) + '... (' + t.id + ')';
            tabSelect.appendChild(opt);
          });
        }
        
        const res = await fetch('/api/macros/list');
        const data = await res.json();
        
        if (data.success && data.macros.length > 0) {
          container.innerHTML = '';
          data.macros.forEach(macro => {
            if (macro.error) return;
            const dateStr = new Date(macro.timestamp).toLocaleDateString('fr-FR', {
              hour: '2-digit', minute: '2-digit'
            });
            
            const item = document.createElement('div');
            item.className = 'item-card';
            item.innerHTML = \`
              <div class="item-info">
                <span class="item-title" style="color: var(--accent-color);">\${macro.name}</span>
                <span class="item-meta">
                  URL: \${macro.url.slice(0, 70)}... | <b>\${macro.stepsCount} Ã‰TAPES</b> | \${dateStr}
                </span>
              </div>
              <button class="btn-action btn-primary" onclick="replayMacro('\${macro.name}')">Rejouer âš¡</button>
            \`;
            container.appendChild(item);
          });
          appendChatMessage('agent', 'âœ… J\'ai chargÃ© <strong>' + data.macros.length + ' playbooks</strong> de navigation.', true);
        } else {
          container.innerHTML = '<div class="empty-state">Aucun playbook enregistrÃ©. Lancez REC depuis l\'extension Chrome pour enregistrer votre premier flux ! ðŸŽ¥</div>';
        }
      } catch (e) {
        container.innerHTML = '<div class="empty-state">Erreur lors de la rÃ©cupÃ©ration des macros : ' + e.message + '</div>';
      }
    }

    async function replayMacro(name) {
      const tabId = document.getElementById('replayTabId').value;
      if (!tabId) {
        alert("Veuillez connecter un onglet valide pour le rejeu.");
        return;
      }
      
      appendChatMessage('agent', 'âš¡ Lancement du rejeu de la macro <strong>' + name + '</strong> sur l\'onglet ' + tabId + '...');
      appendChatMessage('agent', 'ðŸ› ï¸ <span style="font-family:\'Share Tech Mono\';">Moteur CDP : DÃ©placement virtuel fluide...</span>', true);
      
      try {
        const res = await fetch('/api/macros/replay?name=' + encodeURIComponent(name) + '&tabId=' + encodeURIComponent(tabId));
        const data = await res.json();
        if (data.success) {
          appendChatMessage('agent', 'âœ… Rejeu complÃ©tÃ© avec succÃ¨s !', true);
        } else {
          appendChatMessage('agent', 'âŒ Ã‰chec de rejeu : ' + data.error, true);
        }
      } catch (e) {
        appendChatMessage('agent', 'âŒ Erreur rÃ©seau de rejeu.', true);
      }
    }

    window.onload = () => {
      loadFiles();
      loadMacros();
    };
  </script>
</body>
</html>
`;
    res.writeHead(200);
    res.end(html);
  }
  else {
    res.writeHead(404);
    res.end(JSON.stringify({ error: "Not Found" }));
  }
});

// Central Extension Message Handler (Shared by Native Messaging and WebSocket fallback)
function handleExtensionMessage(msg) {
  try {
    if (msg.type === "ping") {
      if (extensionSocket && extensionSocket.send) {
        extensionSocket.send(JSON.stringify({ type: "pong" }));
      }
      return;
    }

    // Quiet logging of routine events to keep console clean
    if (msg.type !== "pong" && msg.type !== "event") {
      console.error(`ðŸ§¬ [Proxy] Extension -> Proxy: type=${msg.type}`, msg);
    }

    if (msg.type === "response") {
      const reqState = pendingRequests.get(msg.id);
      if (reqState) {
        pendingRequests.delete(msg.id);
        if (reqState.timeout) clearTimeout(reqState.timeout);
        
        if (reqState.resolve) {
          reqState.resolve(msg.data);
        } else if (reqState.playwrightSocket && reqState.playwrightSocket.readyState === WebSocket.OPEN) {
          // Map back to Playwright's original message format
          const response = {
            id: reqState.originalId,
            result: msg.data.result || {},
            error: msg.data.error ? { message: msg.data.error } : undefined
          };
          if (reqState.sessionId) {
            response.sessionId = reqState.sessionId;
          }
          reqState.playwrightSocket.send(JSON.stringify(response));
        }
      }
    } 
    else if (msg.type === "event") {
      // CDP page-level event forwarding
      const tabIdStr = String(msg.tabId);
      
      // A. Forward to direct page-level Playwright sockets
      const pSockets = pageSockets.get(tabIdStr);
      if (pSockets) {
        const eventPayload = JSON.stringify({
          method: msg.method,
          params: msg.params
        });
        for (const client of pSockets) {
          if (client.readyState === WebSocket.OPEN) {
            client.send(eventPayload);
          }
        }
      }

      // B. Forward to browser-level Playwright sockets (requires sessionId mapping!)
      if (browserSockets.size > 0) {
        const browserEventPayload = JSON.stringify({
          method: msg.method,
          params: msg.params,
          sessionId: `session-${tabIdStr}`
        });
        for (const client of browserSockets) {
          if (client.readyState === WebSocket.OPEN) {
            client.send(browserEventPayload);
          }
        }
      }
    }
    else if (msg.type === "target_event") {
      // Broadcast real-time tab discovery events to browser-level Playwright sockets
      if (browserSockets.size > 0) {
        const cdpEvent = JSON.stringify({
          method: msg.method, // Target.targetCreated or Target.targetInfoChanged
          params: {
            targetInfo: {
              targetId: String(msg.tab.id),
              type: "page",
              title: msg.tab.title || "Chrome Tab",
              url: msg.tab.url || "about:blank",
              attached: false,
              canAccessOpener: false,
              browserContextId: "default"
            }
          }
        });
        for (const client of browserSockets) {
          if (client.readyState === WebSocket.OPEN) {
            client.send(cdpEvent);
          }
        }
      }
    }
    else if (msg.type === "target_destroyed") {
      sessionManager.removeTab(msg.tabId);
      // Broadcast real-time tab deletion events to browser-level Playwright sockets
      if (browserSockets.size > 0) {
        const cdpEvent = JSON.stringify({
          method: "Target.targetDestroyed",
          params: {
            targetId: String(msg.tabId)
          }
        });
        for (const client of browserSockets) {
          if (client.readyState === WebSocket.OPEN) {
            client.send(cdpEvent);
          }
        }
      }
    }
    else if (msg.type === "detached") {
      sessionManager.markDetached(msg.tabId);
    }
    else if (msg.type === "save_macro") {
      console.error("ðŸ§¬ [Proxy] Save macro request received:", msg.macro.name);
      const fs = require('fs');
      const path = require('path');
      const macrosDir = path.join(homeDir, '.gemini', 'antigravity', 'scratch', 'macros');
      try {
        if (!fs.existsSync(macrosDir)) {
          fs.mkdirSync(macrosDir, { recursive: true });
        }
        const filePath = path.join(macrosDir, msg.macro.name);
        fs.writeFileSync(filePath, JSON.stringify(msg.macro, null, 2), 'utf-8');
        console.error(`ðŸ§¬ [Proxy] Macro successfully written to: ${filePath}`);
        
        // Mirror in OneDrive
        const oneDriveMacrosDir = path.join(homeDir, 'OneDrive', 'Documents', 'macros');
        if (!fs.existsSync(oneDriveMacrosDir)) {
          fs.mkdirSync(oneDriveMacrosDir, { recursive: true });
        }
        const oneDriveFilePath = path.join(oneDriveMacrosDir, msg.macro.name);
        fs.writeFileSync(oneDriveFilePath, JSON.stringify(msg.macro, null, 2), 'utf-8');
        console.error(`ðŸ§¬ [Proxy] Macro successfully copied to OneDrive: ${oneDriveFilePath}`);
      } catch(e) {
        console.error("âš ï¸ [Proxy] Failed to write macro file:", e);
      }
    }
  } catch (err) {
    console.error("âš ï¸ [Proxy] Error handling extension message:", err);
  }
}

// ----------------------------------------------------
// NATIVE MESSAGING STANDARD I/O ENGINES
// ----------------------------------------------------
if (isNativeMode && !runAsBridgeClientOnly) {
  console.error("ðŸ§¬ [Proxy] Native Messaging session active. Initializing standard I/O streams...");

  const virtualSocket = {
    isNative: true,
    readyState: WebSocket.OPEN, // OPEN
    send: (data) => {
      try {
        const payload = typeof data === 'string' ? JSON.parse(data) : data;
        sendNativeMessage(payload);
      } catch (e) {
        console.error("ðŸ§¬ [Proxy] Failed to encode Native Message output:", e);
      }
    }
  };

  // The native virtual socket becomes active only after Chrome actually sends a native message.
  // This avoids false-positive connectivity when the bridge is launched manually for local HTTP control.

  // Stdin parsing
  let inputBuffer = Buffer.alloc(0);
  process.stdin.on('readable', () => {
    let chunk;
    while ((chunk = process.stdin.read()) !== null) {
      inputBuffer = Buffer.concat([inputBuffer, chunk]);
    }

    while (inputBuffer.length >= 4) {
      const msgLen = inputBuffer.readUInt32LE(0);
      if (inputBuffer.length < 4 + msgLen) {
        break; // Message body not fully buffered yet
      }

      const msgBytes = inputBuffer.subarray(4, 4 + msgLen);
      inputBuffer = inputBuffer.subarray(4 + msgLen);

      try {
        const msgJson = msgBytes.toString('utf8');
        const msg = JSON.parse(msgJson);
        if (extensionSocket == null) extensionSocket = virtualSocket;
        handleExtensionMessage(msg);
      } catch (e) {
        console.error("ðŸ§¬ [Proxy] Stdin Native decoding error:", e);
      }
    }
  });

  process.stdin.on('end', () => {
    console.error("🧬 [Proxy] process.stdin closed. Keeping HTTP bridge alive for reconnectable local control.");
    if (extensionSocket === virtualSocket) extensionSocket = null;
    updateHeartbeatAfterNativeDisconnect();
  });

  function updateHeartbeatAfterNativeDisconnect() {
    try {
      if (lockFd !== null) writeBridgeLock(lockFd);
    } catch {}
  }

  function sendNativeMessage(msg) {
    const msgJson = JSON.stringify(msg);
    const msgBytes = Buffer.from(msgJson, 'utf8');
    const header = Buffer.alloc(4);
    header.writeUInt32LE(msgBytes.length, 0);
    process.stdout.write(header);
    process.stdout.write(msgBytes);
  }
}

// Setup WebSocket server
const wss = new WebSocketServer({ server });

wss.on('connection', (ws, req) => {
  const parsedUrl = url.parse(req.url, true);
  const pathname = parsedUrl.pathname;

  // ============================================
  // 1. Chrome Extension WebSocket Tunnel (Fallback)
  // ============================================
  if (pathname === '/extension') {
    const isBridgeClient = parsedUrl.query.nativeBridge === 'true';
    if (isNativeMode && !isBridgeClient && extensionSocket?.isNative) {
      console.error("🧬 [Proxy] WebSocket extension connected but Native Messaging is active. Rejecting to avoid collisions.");
      ws.close(1008, "Native Messaging is active");
      return;
    }

    console.log("ðŸ§¬ [Proxy] Chrome Extension connected successfully via WebSocket fallback!");
    extensionSocket = ws;

    ws.on('message', (message) => {
      try {
        const msg = JSON.parse(message);
        handleExtensionMessage(msg);
      } catch (err) {
        console.error("ðŸ§¬ [Proxy] WebSocket fallback parse error:", err);
      }
    });

    ws.on('close', () => {
      console.log("ðŸ§¬ [Proxy] Chrome Extension WebSocket disconnected.");
      if (extensionSocket === ws) {
        extensionSocket = null;
      }
    });
  } 
  
  // ============================================
  // 2. Playwright Page-Level WebSocket (Direct)
  // ============================================
  else if (pathname.startsWith('/devtools/page/')) {
    const tabId = pathname.split('/').pop();
    console.log(`ðŸ§¬ [Proxy] Playwright page connected directly to tab ${tabId}`);

    if (!pageSockets.has(tabId)) {
      pageSockets.set(tabId, new Set());
    }
    pageSockets.get(tabId).add(ws);
    sessionManager.addPageSocket(tabId, ws);

    // Auto-attach tab in extension immediately on socket open
    if (isExtensionConnected()) {
      sessionManager.markAttached(tabId);
      extensionSocket.send(JSON.stringify({
        id: ++messageCounter,
        command: "attach",
        params: { tabId }
      }));
    }

    ws.on('message', (message) => {
      try {
        const msg = JSON.parse(message);
        
        // INTERCEPT BROWSER-SCOPED COMMANDS IN PAGE SESSION (Avoids Playwright crash)
        if (msg.method && msg.method.startsWith("Browser.")) {
          if (msg.method === "Browser.getVersion") {
            ws.send(JSON.stringify({
              id: msg.id,
              result: {
                protocolVersion: "1.3",
                product: "Chrome/120.0.0.0",
                revision: "@a06414a2754673bc28ea7c71d60dd4d9c7af4718",
                userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                jsVersion: "12.0.267"
              }
            }));
          } else {
            // Quietly mock setDownloadBehavior and other setup options as success
            ws.send(JSON.stringify({
              id: msg.id,
              result: {}
            }));
          }
          return;
        }

        // MOCK KEY TARGET COMMANDS
        if (msg.method === "Target.setAutoAttach") {
          ws.send(JSON.stringify({ id: msg.id, result: {} }));
          return;
        }

        if (msg.method === "Target.getTargetInfo") {
          ws.send(JSON.stringify({
            id: msg.id,
            result: {
              targetInfo: {
                targetId: tabId,
                type: "page",
                title: "Chrome Tab",
                url: "about:blank",
                attached: true,
                canAccessOpener: false,
                browserContextId: "default"
              }
            }
          }));
          return;
        }

        if (!isExtensionConnected()) {
          ws.send(JSON.stringify({
            id: msg.id,
            error: { message: "Chrome Extension is not connected. Launch your browser with the bridge!" }
          }));
          return;
        }

        // Bridge raw CDP packet to extension
        const requestId = ++messageCounter;
        pendingRequests.set(requestId, {
          playwrightSocket: ws,
          originalId: msg.id
        });

        extensionSocket.send(JSON.stringify({
          id: requestId,
          command: "cdp_command",
          params: {
            tabId,
            method: msg.method,
            cdpParams: msg.params
          }
        }));
      } catch (err) {
        console.error(`âš ï¸ [Proxy] Error handling page message on tab ${tabId}:`, err);
      }
    });

    ws.on('close', () => {
      console.log(`ðŸ§¬ [Proxy] Playwright disconnected from tab ${tabId}`);
      const sockets = pageSockets.get(tabId);
      sessionManager.removePageSocket(tabId, ws);
      if (sockets) {
        sockets.delete(ws);
        if (sockets.size === 0) {
          pageSockets.delete(tabId);
          // Detach debugger in extension to release resources
          if (isExtensionConnected()) {
            sessionManager.markDetached(tabId);
            extensionSocket.send(JSON.stringify({
              id: ++messageCounter,
              command: "detach",
              params: { tabId }
            }));
          }
        }
      }
    });
  }
  
  // ============================================
  // 3. Playwright Browser-Level WebSocket (Multiplexed)
  // ============================================
  else if (pathname === '/devtools/browser') {
    console.log("ðŸ§¬ [Proxy] Playwright browser-level connected!");
    browserSockets.add(ws);
    sessionManager.addBrowserSocket(ws);

    ws.on('message', async (message) => {
      try {
        const msg = JSON.parse(message);
        console.log(`ðŸ“¥ [CDP In] ${msg.method || 'SessionMsg'} (id=${msg.id})`, JSON.stringify(msg));

        // 3A. Handling session-routed commands (contain sessionId!)
        if (msg.sessionId) {
          const tabId = msg.sessionId.replace("session-", "");
          
          if (!isExtensionConnected()) {
            const errRes = {
              id: msg.id,
              error: { message: "Chrome Extension is not connected." },
              sessionId: msg.sessionId
            };
            console.log(`ðŸ“¤ [CDP Out Error] SessionMsg`, errRes);
            ws.send(JSON.stringify(errRes));
            return;
          }

          const requestId = ++messageCounter;
          pendingRequests.set(requestId, {
            playwrightSocket: ws,
            originalId: msg.id,
            sessionId: msg.sessionId
          });

          extensionSocket.send(JSON.stringify({
            id: requestId,
            command: "cdp_command",
            params: {
              tabId,
              method: msg.method,
              cdpParams: msg.params
            }
          }));
          return;
        }

        // 3B. Handling global browser-level commands
        if (msg.method === "Browser.getVersion") {
          const resVal = {
            id: msg.id,
            result: {
              protocolVersion: "1.3",
              product: "Chrome/120.0.0.0",
              revision: "@a06414a2754673bc28ea7c71d60dd4d9c7af4718",
              userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
              jsVersion: "12.0.267"
            }
          };
          console.log(`ðŸ“¤ [CDP Out] Browser.getVersion`, resVal);
          ws.send(JSON.stringify(resVal));
        }
        else if (msg.method === "Target.getTargets") {
          if (!isExtensionConnected()) {
            ws.send(JSON.stringify({ id: msg.id, result: { targetInfos: [] } }));
            return;
          }

          const requestId = ++messageCounter;
          const tabPromise = new Promise(resolve => {
            pendingRequests.set(requestId, {
              resolve: (data) => resolve(data.tabs || []),
              timeout: setTimeout(() => resolve([]), 2000)
            });
          });

          extensionSocket.send(JSON.stringify({ id: requestId, command: "list_tabs" }));
          const tabs = await tabPromise;

          const targetInfos = tabs.map(t => ({
            targetId: String(t.id),
            type: "page",
            title: t.title || "Chrome Tab",
            url: t.url || "about:blank",
            attached: false,
            canAccessOpener: false,
            browserContextId: "default"
          }));

          ws.send(JSON.stringify({
            id: msg.id,
            result: { targetInfos }
          }));
        }
        else if (msg.method === "Target.attachToTarget") {
          const tabId = String(msg.params.targetId);
          const sessionId = `session-${tabId}`;
          
          // Request debugger attach in the extension
          if (isExtensionConnected()) {
            sessionManager.markAttached(tabId);
            extensionSocket.send(JSON.stringify({
              id: ++messageCounter,
              command: "attach",
              params: { tabId }
            }));
          }

          // Acknowledge attachment with unique session id
          ws.send(JSON.stringify({
            id: msg.id,
            result: { sessionId }
          }));

          // Send explicit attachedToTarget event so Playwright constructs its internal structures immediately
          ws.send(JSON.stringify({
            method: "Target.attachedToTarget",
            params: {
              sessionId,
              targetInfo: {
                targetId: tabId,
                type: "page",
                title: "Chrome Tab",
                url: "about:blank",
                attached: true,
                canAccessOpener: false,
                browserContextId: "default"
              },
              waitingForDebugger: false
            }
          }));
        }
        else if (msg.method === "Target.setDiscoverTargets" || msg.method === "Target.setAutoAttach") {
          ws.send(JSON.stringify({ id: msg.id, result: {} }));

          // IMMEDIATELY query open tabs and broadcast Target.targetCreated + Target.attachedToTarget for the active tab!
          if (isExtensionConnected()) {
            const requestId = ++messageCounter;
            pendingRequests.set(requestId, {
              resolve: (data) => {
                let tabs = data.tabs || [];
                
                // Extra filter layer in proxy to ignore restricted system pages
                tabs = tabs.filter(t => {
                  const u = t.url || "";
                  return !u.startsWith("chrome://") && 
                         !u.startsWith("chrome-extension://") && 
                         !u.startsWith("edge://") && 
                         !u.startsWith("view-source:");
                });

                if (tabs.length === 0) {
                  // If all tabs are restricted settings/extensions pages, programmatically open a debuggable target
                  console.log("ðŸ§¬ [Proxy] setDiscoverTargets: No debuggable tabs found. Creating 'about:blank' target...");
                  extensionSocket.send(JSON.stringify({
                    id: ++messageCounter,
                    command: "create_tab",
                    params: { url: "about:blank" }
                  }));
                  return;
                }
                
                // Find active or first tab
                const activeTab = tabs.find(t => t.active) || tabs[0];
                const tabIdStr = String(activeTab.id);
                const sessionId = `session-${tabIdStr}`;
                
                // Track this tab connection in our map
                if (!pageSockets.has(tabIdStr)) {
                  pageSockets.set(tabIdStr, new Set());
                }
                pageSockets.get(tabIdStr).add(ws);
                sessionManager.addPageSocket(tabIdStr, ws);

                // Auto-attach tab in extension
                sessionManager.markAttached(tabIdStr);
                extensionSocket.send(JSON.stringify({
                  id: ++messageCounter,
                  command: "attach",
                  params: { tabId: tabIdStr }
                }));

                // 1. Send targetCreated event
                ws.send(JSON.stringify({
                  method: "Target.targetCreated",
                  params: {
                    targetInfo: {
                      targetId: tabIdStr,
                      type: "page",
                      title: activeTab.title || "Chrome Tab",
                      url: activeTab.url || "about:blank",
                      attached: true,
                      canAccessOpener: false,
                      browserContextId: "default"
                    }
                  }
                }));

                // 2. Send attachedToTarget event
                ws.send(JSON.stringify({
                  method: "Target.attachedToTarget",
                  params: {
                    sessionId,
                    targetInfo: {
                      targetId: tabIdStr,
                      type: "page",
                      title: activeTab.title || "Chrome Tab",
                      url: activeTab.url || "about:blank",
                      attached: true,
                      canAccessOpener: false,
                      browserContextId: "default"
                    },
                    waitingForDebugger: false
                  }
                }));
              },
              timeout: setTimeout(() => pendingRequests.delete(requestId), 2000)
            });
            extensionSocket.send(JSON.stringify({ id: requestId, command: "list_tabs" }));
          }
        }
        else if (msg.method === "Target.createTarget") {
          if (!isExtensionConnected()) {
            ws.send(JSON.stringify({
              id: msg.id,
              error: { message: "Chrome Extension is not connected." }
            }));
            return;
          }

          const requestId = ++messageCounter;
          pendingRequests.set(requestId, {
            resolve: (data) => {
              if (data && data.success && data.tab) {
                const tabIdStr = String(data.tab.id);
                const sessionId = `session-${tabIdStr}`;

                // Register socket in map
                if (!pageSockets.has(tabIdStr)) {
                  pageSockets.set(tabIdStr, new Set());
                }
                pageSockets.get(tabIdStr).add(ws);
                sessionManager.addPageSocket(tabIdStr, ws);

                ws.send(JSON.stringify({
                  id: msg.id,
                  result: {
                    targetId: tabIdStr
                  }
                }));
                
                // Send targetCreated to all browser clients
                const targetCreatedEvent = {
                  method: "Target.targetCreated",
                  params: {
                    targetInfo: {
                      targetId: tabIdStr,
                      type: "page",
                      title: data.tab.title || "Chrome Tab",
                      url: data.tab.url || "about:blank",
                      attached: true,
                      canAccessOpener: false,
                      browserContextId: "default"
                    }
                  }
                };
                for (const client of browserSockets) {
                  if (client.readyState === WebSocket.OPEN) {
                    client.send(JSON.stringify(targetCreatedEvent));
                  }
                }

                // Send attachedToTarget event to the requesting browser socket
                ws.send(JSON.stringify({
                  method: "Target.attachedToTarget",
                  params: {
                    sessionId,
                    targetInfo: {
                      targetId: tabIdStr,
                      type: "page",
                      title: data.tab.title || "Chrome Tab",
                      url: data.tab.url || "about:blank",
                      attached: true,
                      canAccessOpener: false,
                      browserContextId: "default"
                    },
                    waitingForDebugger: false
                  }
                }));
              } else {
                ws.send(JSON.stringify({
                  id: msg.id,
                  error: { message: data.error || "Failed to create tab" }
                }));
              }
            },
            timeout: setTimeout(() => {
              pendingRequests.delete(requestId);
              ws.send(JSON.stringify({
                id: msg.id,
                error: { message: "Timeout creating tab" }
              }));
            }, 3000)
          });

          extensionSocket.send(JSON.stringify({
            id: requestId,
            command: "create_tab",
            params: { url: msg.params.url || "about:blank" }
          }));
        }
        else if (msg.method === "Target.getTargetInfo") {
          const targetId = msg.params && msg.params.targetId ? msg.params.targetId : "browser";
          ws.send(JSON.stringify({
            id: msg.id,
            result: {
              targetInfo: {
                targetId,
                type: targetId === "browser" ? "browser" : "page",
                title: targetId === "browser" ? "Browser" : "Chrome Tab",
                url: "",
                attached: true,
                canAccessOpener: false,
                browserContextId: "default"
              }
            }
          }));
        }
        else {
          // Return generic success for setup commands (prevents client timeout/crashes)
          ws.send(JSON.stringify({
            id: msg.id,
            result: {}
          }));
        }
      } catch (err) {
        console.error("âš ï¸ [Proxy] Error handling browser-level command:", err);
      }
    });

    ws.on('close', () => {
      console.log("ðŸ§¬ [Proxy] Playwright browser-level disconnected.");
      browserSockets.delete(ws);
      sessionManager.removeBrowserSocket(ws);
    });
  } 
  
  else {
    ws.close(4004, "Invalid Path");
  }
});

// ==========================================================
// Cyber Macro Replayer SOTA Engine
// ==========================================================
const sleep = (ms) => new Promise(resolve => setTimeout(resolve, ms));

function sendCDP(tabId, method, params) {
  if (!isExtensionConnected()) {
    console.error("âš ï¸ [Replayer] Extension not connected.");
    return Promise.reject(new Error("Extension not connected"));
  }
  
  const id = ++messageCounter;
  return new Promise((resolve, reject) => {
    pendingRequests.set(id, {
      resolve: (res) => resolve(res),
      timeout: setTimeout(() => {
        pendingRequests.delete(id);
        reject(new Error(`CDP Command ${method} timeout`));
      }, 5000)
    });
    
    extensionSocket.send(JSON.stringify({
      id,
      command: "cdp_command",
      params: {
        tabId: String(tabId),
        method,
        cdpParams: params
      }
    }));
  });
}

async function executeMacroReplay(tabId, macro) {
  console.log(`ðŸ§¬ [Replayer] Starting replay of macro "${macro.name}" on tab ${tabId}...`);
  let lastX = 200;
  let lastY = 200;
  
  try {
    // Set tab active
    await sendCDP(tabId, "Page.bringToFront", {});
  } catch(e) {
    console.log("âš ï¸ [Replayer] Page.bringToFront failed, continuing.");
  }
  
  for (const step of macro.steps) {
    console.log(`ðŸ§¬ [Replayer] Replaying step: ${step.type} on ${step.selector || 'coordinates'}`);
    
    if (step.type === 'click') {
      const targetX = step.x;
      const targetY = step.y;
      
      // Calculate smooth cursor sliding path
      const stepsCount = 12;
      for (let i = 1; i <= stepsCount; i++) {
        const t = i / stepsCount;
        const easeT = t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
        const cx = Math.round(lastX + (targetX - lastX) * easeT);
        const cy = Math.round(lastY + (targetY - lastY) * easeT);
        
        await sendCDP(tabId, "Input.dispatchMouseEvent", {
          type: "mouseMoved",
          x: cx,
          y: cy,
          button: "none",
          clickCount: 0
        });
        await sleep(15);
      }
      
      lastX = targetX;
      lastY = targetY;
      await sleep(120); // target lock pause
      
      // Trigger mouse pressed
      await sendCDP(tabId, "Input.dispatchMouseEvent", {
        type: "mousePressed",
        x: targetX,
        y: targetY,
        button: "left",
        clickCount: 1
      });
      
      await sleep(50);
      
      // Trigger mouse released
      await sendCDP(tabId, "Input.dispatchMouseEvent", {
        type: "mouseReleased",
        x: targetX,
        y: targetY,
        button: "left",
        clickCount: 1
      });
      
      await sleep(400); // Wait for page actions
    } 
    else if (step.type === 'input') {
      try {
        await sendCDP(tabId, "Runtime.evaluate", {
          expression: `(() => {
            const el = document.querySelector('${step.selector}');
            if (el) {
              el.focus();
              el.value = '${step.value.replace(/'/g, "\\'")}';
              el.dispatchEvent(new Event('input', { bubbles: true }));
              el.dispatchEvent(new Event('change', { bubbles: true }));
              return true;
            }
            return false;
          })()`
        });
      } catch (e) {
        console.error("âš ï¸ [Replayer] Input evaluate failed:", e.message);
      }
      await sleep(300);
    }
  }
  
  console.log(`ðŸ§¬ [Replayer] Replay of macro "${macro.name}" completed successfully!`);
}

let bridgeClientModeStarted = false;
function handleListenError(err) {
  if (err.code === 'EADDRINUSE') {
    if (bridgeClientModeStarted) return;
    bridgeClientModeStarted = true;
    if (isNativeMode) {
      releaseBridgeLock();
      console.error("🧬 [Proxy] Port 29002 is already occupied. Starting in Bridge Client Mode...");
      startBridgeClientMode();
    } else {
      releaseBridgeLock();
      console.error("🧬 [Proxy] Port 29002 is already in use. Another instance of Sinew Chrome Bridge is running. Exiting silently.");
      process.exit(0);
    }
  } else {
    throw err;
  }
}

server.on('error', handleListenError);
wss.on('error', handleListenError);

function startBridgeClientMode() {
  const ws = new WebSocket("ws://localhost:29002/extension?nativeBridge=true");
  
  ws.on('open', () => {
    console.error("🧬 [Bridge Client] Tunnel established with active server!");
  });
  
  let inputBuffer = Buffer.alloc(0);
  process.stdin.on('readable', () => {
    let chunk;
    while ((chunk = process.stdin.read()) !== null) {
      inputBuffer = Buffer.concat([inputBuffer, chunk]);
    }
    while (inputBuffer.length >= 4) {
      const msgLen = inputBuffer.readUInt32LE(0);
      if (inputBuffer.length < 4 + msgLen) break;
      const msgBytes = inputBuffer.subarray(4, 4 + msgLen);
      inputBuffer = inputBuffer.subarray(4 + msgLen);
      
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(msgBytes.toString('utf8'));
      }
    }
  });

  process.stdin.on('end', () => {
    console.error("🧬 [Bridge Client] Stdin closed. Terminating...");
    ws.close();
    process.exit(0);
  });

  ws.on('message', (data) => {
    const msgJson = typeof data === 'string' ? data : data.toString('utf8');
    const msgBytes = Buffer.from(msgJson, 'utf8');
    const header = Buffer.alloc(4);
    header.writeUInt32LE(msgBytes.length, 0);
    process.stdout.write(header);
    process.stdout.write(msgBytes);
  });

  ws.on('close', () => {
    console.error("🧬 [Bridge Client] Tunnel closed. Exiting.");
    process.exit(0);
  });

  ws.on('error', (err) => {
    console.error("🧬 [Bridge Client] Tunnel error:", err.message);
    process.exit(1);
  });
}

if (runAsBridgeClientOnly) {
  startBridgeClientMode();
} else {
  server.listen(PORT, () => {
    console.log(`ðŸ§¬ [Proxy] UPGRADED Sinew Chrome Bridge listening on http://localhost:${PORT}`);
    console.log(`ðŸ§¬ [Proxy] Standard CDP browser endpoint: ws://localhost:${PORT}/devtools/browser`);
    console.log(`ðŸ§¬ [Proxy] Waiting for Chrome Extension to connect on /extension...`);
  });
}
