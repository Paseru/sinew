// 🧬 Sinew Chrome Bridge — Service Worker (Manifest V3)
// Upgraded to SOTA Sinew-grade sequential execution queue and automatic biological cursor physics injection.
// Pure Native Messaging connection (Optimized for Sinew Native Messenger).

let nativePort = null;
let reconnectTimer = null;
let lastNativeError = null;
let lastConnectedAt = null;
let bridgeSecret = '';

// Registry of active attached debuggers
const attachedTabs = new Set();
const ALLOW_DEBUGGER_ATTACH = true;
let cursorMoveSeq = 0;
const lastCursorPositionByTabId = new Map();

function normalizeCursorOptions(options = {}) {
  const mode = ['visible', 'hidden'].includes(String(options.mode || 'visible').toLowerCase())
    ? String(options.mode || 'visible').toLowerCase()
    : 'visible';
  const speed = ['slow', 'normal', 'fast'].includes(String(options.speed || 'normal').toLowerCase())
    ? String(options.speed || 'normal').toLowerCase()
    : 'normal';
  const timing = {
    slow: { steps: 58, minDelay: 18, jitter: 24, pause: 220 },
    normal: { steps: 38, minDelay: 12, jitter: 18, pause: 140 },
    fast: { steps: 22, minDelay: 5, jitter: 10, pause: 60 },
  }[speed];
  return { mode, speed, timing };
}

// Promise-based locking mechanism for race-free sequential execution
let lifecycleQueue = Promise.resolve();

function runLocked(fn) {
  const next = lifecycleQueue.then(() => fn());
  lifecycleQueue = next.catch((err) => {
    console.error("⚠️ [Bridge Queue Error]:", err);
  });
  return next;
}

// Utility to check if a URL is a restricted system page
function isSystemTab(tab) {
  const u = tab.url || "";
  return u.startsWith("chrome://") || 
         u.startsWith("chrome-extension://") || 
         u.startsWith("edge://") ||
         u.startsWith("view-source:");
}

// Reusable central message sender
function sendMsg(msg) {
  if (nativePort) {
    try {
      nativePort.postMessage(msg);
    } catch (e) {
      console.error("🧬 [Bridge background] Failed to send via Native Port:", e);
    }
  }
}

function isBridgeConnected() {
  return !!nativePort;
}

// Reusable response sender
function sendResponse(id, data) {
  sendMsg({ type: "response", id, data });
}

function sendTabMessage(tabId, message, timeoutMs = 12000) {
  return new Promise((resolve) => {
    let settled = false;
    const done = (value) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      resolve(value);
    };
    const timer = setTimeout(() => {
      done({ success: false, ok: false, error: `Timeout waiting for content script response: ${message?.type || 'unknown'}` });
    }, Math.max(1000, timeoutMs));

    try {
      chrome.tabs.sendMessage(tabId, message, (response) => {
        if (chrome.runtime.lastError) {
          done({ success: false, ok: false, error: chrome.runtime.lastError.message });
        } else {
          done(response || { success: false, ok: false, error: "No target response" });
        }
      });
    } catch (err) {
      done({ success: false, ok: false, error: err.message });
    }
  });
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
  if (domain && /julienpiron|google|recherche|search|tape|écris|ecris|saisis|type/i.test(original)) return cleanSearchQuery(domain[0]);

  const generic = original.match(/(?:tape|écris|ecris|saisis|type|recherche(?:\s+sur\s+google)?|search)\s+(?:exactement\s+)?(.+?)(?:\s+(?:puis|et)\b|[,;]\s*(?:valide|valides|appuie|clique|clic|click)|$)/i);
  return cleanSearchQuery(generic && generic[1] ? generic[1] : '');
}

function extractTaskUrl(task) {
  const explicit = String(task || '').match(/https?:\/\/[^\s)]+/i);
  if (explicit) return explicit[0].replace(/[)\],.;!?]+$/g, '');
  const domain = String(task || '').match(/\b[a-z0-9-]+(?:\.[a-z0-9-]+)+(\/[^\s)]*)?/i);
  return domain ? `https://${domain[0].replace(/[)\],.;!?]+$/g, '')}` : null;
}

function shouldAutoNavigateTask(task) {
  if (isGoogleSearchTask(task)) return false;
  return !!extractTaskUrl(task) && hasNavigationIntent(task);
}

function waitForTabReady(tabId, timeoutMs = 8000) {
  return new Promise((resolve) => {
    let settled = false;
    let pollTimer = null;

    const done = (tab = null) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeoutTimer);
      if (pollTimer) clearInterval(pollTimer);
      chrome.tabs.onUpdated.removeListener(onUpdated);
      resolve(tab);
    };

    const isReady = (tab) => tab && tab.status === 'complete' && !isSystemTab(tab);

    const check = () => {
      chrome.tabs.get(tabId, (tab) => {
        if (chrome.runtime.lastError) return;
        if (isReady(tab)) done(tab);
      });
    };

    const onUpdated = (updatedTabId, changeInfo, tab) => {
      if (Number(updatedTabId) !== Number(tabId)) return;
      if (changeInfo.status === 'complete' || isReady(tab)) done(tab);
    };

    const timeoutTimer = setTimeout(() => {
      chrome.tabs.get(tabId, (tab) => done(chrome.runtime.lastError ? null : tab));
    }, Math.max(1000, timeoutMs));

    chrome.tabs.onUpdated.addListener(onUpdated);
    pollTimer = setInterval(check, 150);
    check();
  });
}

function clampCursorPoint(point, viewport = {}) {
  const width = Number.isFinite(viewport.width) ? viewport.width : 1280;
  const height = Number.isFinite(viewport.height) ? viewport.height : 720;
  return {
    x: Math.round(Math.min(Math.max(Number(point.x) || 24, 16), Math.max(16, width - 16))),
    y: Math.round(Math.min(Math.max(Number(point.y) || 24, 16), Math.max(16, height - 16)))
  };
}

function inferViewport(target = {}) {
  const rect = target.rect || {};
  const width = target.viewport?.width || target.viewportWidth || Math.max(1280, (Number(target.x) || 0) + (Number(rect.width) || 0) + 80);
  const height = target.viewport?.height || target.viewportHeight || Math.max(720, (Number(target.y) || 0) + (Number(rect.height) || 0) + 80);
  return { width, height };
}

function randomStartNearTarget(target) {
  const viewport = inferViewport(target);
  const side = Math.floor(Math.random() * 4);
  const margin = 24 + Math.random() * 96;
  const edgePoint = [
    { x: margin, y: viewport.height * (0.12 + Math.random() * 0.76) },
    { x: viewport.width - margin, y: viewport.height * (0.12 + Math.random() * 0.76) },
    { x: viewport.width * (0.12 + Math.random() * 0.76), y: margin },
    { x: viewport.width * (0.12 + Math.random() * 0.76), y: viewport.height - margin }
  ][side];

  const targetVicinity = {
    x: (Number(target.x) || viewport.width / 2) + (Math.random() - 0.5) * (260 + Math.random() * 360),
    y: (Number(target.y) || viewport.height / 2) + (Math.random() - 0.5) * (180 + Math.random() * 300)
  };

  return clampCursorPoint(Math.random() < 0.55 ? targetVicinity : edgePoint, viewport);
}

function getHumanCursorStart(tabId, target) {
  const viewport = inferViewport(target);
  const saved = lastCursorPositionByTabId.get(String(tabId));
  if (saved && Number.isFinite(saved.x) && Number.isFinite(saved.y)) {
    return clampCursorPoint(saved, viewport);
  }
  return randomStartNearTarget(target);
}

function rememberHumanCursor(tabId, point, target = {}) {
  if (!Number.isFinite(point?.x) || !Number.isFinite(point?.y)) return;
  lastCursorPositionByTabId.set(String(tabId), clampCursorPoint(point, inferViewport(target)));
}

function connect() {
  if (nativePort) return;
  try {
    console.log("🧬 [Bridge background] Connecting to Native Host com.sinew.chrome_bridge...");
    const port = chrome.runtime.connectNative("com.sinew.chrome_bridge");
    nativePort = port;

    port.onMessage.addListener((msg) => {
      handleMessage(msg);
    });

    port.onDisconnect.addListener(() => {
      const err = chrome.runtime.lastError;
      lastNativeError = err ? err.message : "Native host disconnected without details";
      console.warn("🧬 [Bridge background] Native Host disconnected:", lastNativeError);
      nativePort = null;
      updateStorageState();

      if (reconnectTimer) clearTimeout(reconnectTimer);
      reconnectTimer = setTimeout(connect, 3000);
    });

    lastNativeError = null;
    lastConnectedAt = Date.now();
    // Native connection succeeded: register and sync
    sendMsg({ type: "register", role: "extension" });
    reportOpenTabs();
    updateStorageState();
  } catch (e) {
    lastNativeError = e.message;
    console.warn("🧬 [Bridge background] Native Port crash on initialize:", e);
    if (reconnectTimer) clearTimeout(reconnectTimer);
    reconnectTimer = setTimeout(connect, 3000);
  }
}


// Unified central command coordinator
async function handleMessage(msg) {
  try {
    if (msg.type === "pong") return;
    if (msg.type === "init_secret") {
      bridgeSecret = msg.token;
      console.log("🧬 [Bridge background] Secure bridge secret initialized successfully!");
      return;
    }
    
    const { id, command, params } = msg;
    if (!command) return;
    
    console.log(`ðŸ§¬ [Bridge] Command received: ${command} (id=${id})`, msg);
    
    switch (command) {
      case "list_tabs":
        reportOpenTabs(id);
        break;

      case "navigate_tab":
        runLocked(async () => {
          return new Promise((resolve) => {
            const tabId = parseInt(params.tabId);
            chrome.tabs.update(tabId, { url: params.url || "about:blank", active: true }, (tab) => {
              if (chrome.runtime.lastError) sendResponse(id, { success: false, error: chrome.runtime.lastError.message });
              else sendResponse(id, { success: true, tab: { id: tab.id, title: tab.title, url: tab.url, active: tab.active } });
              resolve();
            });
          });
        });
        break;

      case "detect_target":
        runLocked(async () => {
          const tabId = parseInt(params.tabId);
          try {
            await ensureCursorInjected(tabId);
            const response = await sendTabMessage(tabId, { type: "RUN_SILENT_TASK", task: params.task || "" }, 12000);
            sendResponse(id, response || { success: false, error: "No target response" });
          } catch (err) {
            sendResponse(id, { success: false, error: err.message });
          }
        });
        break;

      case "page_snapshot":
        runLocked(async () => {
          const tabId = parseInt(params.tabId);
          try {
            await ensureCursorInjected(tabId);
            const response = await sendTabMessage(tabId, { type: "AGENT_PAGE_SNAPSHOT", limit: params.limit || 80 }, 12000);
            sendResponse(id, response || { success: false, error: "No snapshot response" });
          } catch (err) {
            sendResponse(id, { success: false, error: err.message });
          }
        });
        break;

      case "evaluate":
        runLocked(async () => {
          const tabId = parseInt(params.tabId);
          try {
            const expression = String(params.expression || 'undefined');
            const injections = await chrome.scripting.executeScript({
              target: { tabId },
              args: [expression],
              func: (source) => {
                let value;
                try {
                  value = (new Function(`return (${source});`))();
                } catch {
                  value = (new Function(`return (() => { ${source} })();`))();
                }
                if (value && typeof value.then === 'function') return value;
                if (value === undefined) return null;
                return { __sinewValue: value };
              }
            });
            const rawValue = injections && injections[0] ? injections[0].result : null;
            const value = rawValue && typeof rawValue === 'object' && Object.prototype.hasOwnProperty.call(rawValue, '__sinewValue') ? rawValue.__sinewValue : rawValue;
            sendResponse(id, { ok: true, success: true, value });
          } catch (err) {
            sendResponse(id, { ok: false, success: false, error: err.message });
          }
        });
        break;

      case "query_selector":
      case "click_selector":
      case "type_selector":
      case "press_key":
      case "select_option":
      case "wait_selector":
        runLocked(async () => {
          const tabId = parseInt(params.tabId);
          const typeByCommand = {
            query_selector: "AGENT_QUERY_SELECTOR",
            click_selector: "AGENT_CLICK_SELECTOR",
            type_selector: "AGENT_TYPE_SELECTOR",
            press_key: "AGENT_PRESS_KEY",
            select_option: "AGENT_SELECT_OPTION",
            wait_selector: "AGENT_WAIT_SELECTOR"
          };
          try {
            await ensureCursorInjected(tabId);
            const response = await sendTabMessage(tabId, { type: typeByCommand[command], ...params }, Math.max(1000, Number(params.timeoutMs) || 12000));
            sendResponse(id, response || { success: false, error: "No structured action response" });
          } catch (err) {
            sendResponse(id, { success: false, error: err.message });
          }
        });
        break;

      case "human_click":
        runLocked(async () => {
          const tabId = parseInt(params.tabId);
          try {
            const performed = await performHumanCdpAction(tabId, params.detection || {}, "", normalizeCursorOptions(params.cursor || {}));
            sendResponse(id, performed);
          } catch (err) {
            sendResponse(id, { success: false, error: err.message });
          }
        });
        break;

      case "create_tab":
        runLocked(async () => {
          return new Promise((resolve) => {
            chrome.tabs.create({ url: params.url || "about:blank" }, (tab) => {
              sendResponse(id, {
                success: true,
                tab: {
                  id: tab.id,
                  title: tab.title,
                  url: tab.url,
                  active: tab.active
                }
              });
              resolve();
            });
          });
        });
        break;

      case "attach":
        if (!ALLOW_DEBUGGER_ATTACH) {
          sendResponse(id, { success: false, error: "chrome.debugger attach disabled to avoid Chrome debugging banner" });
          break;
        }
        const attachTabId = parseInt(params.tabId);
        runLocked(async () => {
          return new Promise((resolve) => {
            // Safety guard: refuse to attach debugger to chrome:// system tabs
            chrome.tabs.get(attachTabId, (tab) => {
              if (chrome.runtime.lastError || !tab || isSystemTab(tab)) {
                sendResponse(id, { success: false, error: "Cannot attach debugger to restricted system page" });
                resolve();
                return;
              }

              if (attachedTabs.has(attachTabId)) {
                sendResponse(id, { success: true, message: "Already attached" });
                resolve();
                return;
              }
              
              chrome.debugger.attach({ tabId: attachTabId }, "1.3", () => {
                if (chrome.runtime.lastError) {
                  const errMsg = chrome.runtime.lastError.message;
                  if (errMsg.includes("already attached") || errMsg.includes("Already attached")) {
                    attachedTabs.add(attachTabId);
                    sendResponse(id, { success: true });
                    chrome.tabs.sendMessage(attachTabId, { type: "AGENT_STATUS_CHANGE", status: "active" }).catch(() => {});
                  } else {
                    console.error("⚠️ [Bridge] Debugger attachment failed:", errMsg);
                    sendResponse(id, { success: false, error: errMsg });
                  }
                } else {
                  attachedTabs.add(attachTabId);
                  console.log(`🧬 [Bridge] Debugger attached to tab ${attachTabId}`);
                  sendResponse(id, { success: true });
                  chrome.tabs.sendMessage(attachTabId, { type: "AGENT_STATUS_CHANGE", status: "active" }).catch(() => {});
                }
                updateStorageState();
                resolve();
              });
            });
          });
        });
        break;

      case "detach":
        const detachTabId = parseInt(params.tabId);
        runLocked(async () => {
          return new Promise((resolve) => {
            chrome.tabs.sendMessage(detachTabId, { type: "AGENT_STATUS_CHANGE", status: "detached" }).catch(() => {});
            chrome.debugger.detach({ tabId: detachTabId }, () => {
              attachedTabs.delete(detachTabId);
              sendResponse(id, { success: true });
              updateStorageState();
              resolve();
            });
          });
        });
        break;

      case "detach_all":
        runLocked(async () => {
          const ids = Array.from(attachedTabs);
          await Promise.all(ids.map(tabId => new Promise((resolve) => {
            chrome.tabs.sendMessage(tabId, { type: "AGENT_STATUS_CHANGE", status: "detached" }).catch(() => {});
            chrome.debugger.detach({ tabId }, () => {
              attachedTabs.delete(tabId);
              resolve();
            });
          })));
          updateStorageState();
          sendResponse(id, { success: true, detached: ids.length });
        });
        break;

      case "cdp_command":
        if (!ALLOW_DEBUGGER_ATTACH) {
          sendResponse(id, { success: false, error: "CDP commands disabled to avoid Chrome debugging banner" });
          break;
        }
        const cdpTabId = parseInt(params.tabId);
        const { method, cdpParams } = params;
        
        runLocked(async () => {
          return new Promise((resolve) => {
            chrome.tabs.get(cdpTabId, (tab) => {
              if (chrome.runtime.lastError || !tab || isSystemTab(tab)) {
                sendResponse(id, { success: false, error: "Restricted system tab" });
                resolve();
                return;
              }

              // Synchronous auto-attachment layer
              if (!attachedTabs.has(cdpTabId)) {
                chrome.debugger.attach({ tabId: cdpTabId }, "1.3", () => {
                  if (chrome.runtime.lastError && 
                      !chrome.runtime.lastError.message.includes("already attached") && 
                      !chrome.runtime.lastError.message.includes("Already attached")) {
                    sendResponse(id, { success: false, error: "Auto-attach failed: " + chrome.runtime.lastError.message });
                    resolve();
                  } else {
                    attachedTabs.add(cdpTabId);
                    updateStorageState();
                    chrome.tabs.sendMessage(cdpTabId, { type: "AGENT_STATUS_CHANGE", status: "active" }).catch(() => {});
                    sendCDPCommand(id, cdpTabId, method, cdpParams);
                    resolve();
                  }
                });
              } else {
                sendCDPCommand(id, cdpTabId, method, cdpParams);
                resolve();
              }
            });
          });
        });
        break;

      case "execute_silent_task":
        const silentTabId = parseInt(params.tabId);
        const { task: silentTask, cursor: silentCursor } = params;
        const silentCursorOptions = normalizeCursorOptions(silentCursor || {});
        
        runLocked(async () => {
          return new Promise(async (resolve) => {
            let urlToNavigate = null;
            if (shouldAutoNavigateTask(silentTask)) {
              urlToNavigate = extractTaskUrl(silentTask);
            }

            if (urlToNavigate) {
              console.log(`🧬 [Bridge] Silent navigating tab ${silentTabId} to ${urlToNavigate}`);
              chrome.tabs.update(silentTabId, { url: urlToNavigate, active: true }, () => {});
              await waitForTabReady(silentTabId, 8000);
            }

            const actionTasks = buildSilentActionTasks(silentTask);

            const runAction = (taskText) => new Promise((actionResolve) => {
              ensureCursorInjected(silentTabId).then(async () => {
                const response = await sendTabMessage(silentTabId, { type: "RUN_SILENT_TASK", task: taskText }, 12000);
                if (!response || response.success === false || response.ok === false) {
                  actionResolve({ ...(response || { success: false, error: 'No target response' }), task: taskText });
                  return;
                }
                try {
                  const performed = await performHumanCdpAction(silentTabId, response, taskText, silentCursorOptions);
                  actionResolve({ ...performed, task: taskText, target: response.target });
                } catch (err) {
                  actionResolve({ success: false, error: err.message, task: taskText, target: response.target });
                }
              }).catch((err) => {
                actionResolve({ success: false, error: err.message, task: taskText });
              });
            });

            const results = [];
            for (let i = 0; i < actionTasks.length; i++) {
              const taskText = actionTasks[i];
              results.push(await runAction(taskText));
              if (i < actionTasks.length - 1) {
                await new Promise(r => setTimeout(r, /\b(entrée|enter|submit|valide|appuie)\b/i.test(taskText) ? 700 : 250));
              }
            }

            const failed = results.find(r => r && r.success === false);
            if (!failed) {
              chrome.tabs.sendMessage(silentTabId, { type: "AGENT_STATUS_CHANGE", status: "completed" }).catch(() => {});
              setTimeout(() => {
                chrome.tabs.sendMessage(silentTabId, { type: "AGENT_STATUS_CHANGE", status: "detached" }).catch(() => {});
              }, 4000);
            }
            sendResponse(id, failed ? { success: false, results, error: failed.error } : { success: true, results });
            resolve();
          });
        });
        break;

      default:
        sendResponse(id, { success: false, error: `Unknown command: ${command}` });
    }
  } catch (err) {
    console.error("âš ï¸ [Bridge message error]:", err);
  }
}

function buildSilentActionTasks(task) {
  const text = String(task || '').toLowerCase();
  const actions = [];

  if (isGoogleSearchTask(task)) {
    actions.push('clique dans le champ de recherche Google');
    const query = extractSearchQuery(task);
    if (query) {
      actions.push(`tape ${query} puis appuie sur Entrée`);
      if (/\b(clique|clic|click|ouvrir|ouvre|open)\b/i.test(text) && /\b(lien|résultat|resultat|site)\b/i.test(text)) {
        actions.push(`clique le résultat ${query}`);
      }
    }
  }

  if (text.includes('hamburger') || text.includes('menu')) {
    actions.push('clique le bouton menu hamburger');
    if (text.includes('referme') || text.includes('ferme') || text.includes('close')) {
      actions.push('clique le bouton menu hamburger');
    }
  }

  const trinityMatch = text.includes('trinity');
  if (trinityMatch) {
    actions.push('clique la carte Trinity');
  }

  if (actions.length === 0 && shouldAutoNavigateTask(task)) {
    return [];
  }
  return actions.length > 0 ? actions : [task];
}

function cdp(tabId, method, params = {}) {
  return new Promise((resolve, reject) => {
    chrome.debugger.sendCommand({ tabId }, method, params, (result) => {
      if (chrome.runtime.lastError) reject(new Error(chrome.runtime.lastError.message));
      else resolve(result || {});
    });
  });
}

async function attachDebuggerIfNeeded(tabId) {
  if (!ALLOW_DEBUGGER_ATTACH) throw new Error('chrome.debugger attach disabled to avoid Chrome debugging banner');
  if (attachedTabs.has(tabId)) return;
  await new Promise((resolve, reject) => {
    chrome.debugger.attach({ tabId }, "1.3", () => {
      if (chrome.runtime.lastError && !chrome.runtime.lastError.message.includes("already attached") && !chrome.runtime.lastError.message.includes("Already attached")) {
        reject(new Error(chrome.runtime.lastError.message));
      } else {
        attachedTabs.add(tabId);
        resolve();
      }
    });
  });
}

function humanPath(start, end, steps = 34) {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const dist = Math.max(1, Math.hypot(dx, dy));
  
  // Generate 6 candidates with varying curve scales and directions
  const candidates = [];
  const multipliers = [0.4, 0.8, 1.2, -0.4, -0.8, -1.2];
  
  for (const mult of multipliers) {
    const points = [];
    const curve = Math.min(130, Math.max(20, dist * 0.20)) * mult;
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
  // Default bounds safety margins
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

async function showCursor(tabId, x, y, moveSequence = ++cursorMoveSeq, cursorOptions = normalizeCursorOptions()) {
  rememberHumanCursor(tabId, { x, y });
  await chrome.tabs.sendMessage(tabId, {
    type: "AGENT_CURSOR_STATE",
    state: { x, y, visible: cursorOptions.mode !== 'hidden', moveSequence, sessionId: "session-" + tabId, turnId: "turn-human-cdp" }
  }).catch(() => {});
}

async function performHumanCdpAction(tabId, detection, taskText, cursorOptions = normalizeCursorOptions()) {
  if (detection.action === 'scroll') {
    await ensureCursorInjected(tabId);
    const amount = detection.scrollY || 500;
    await chrome.tabs.sendMessage(tabId, { type: "AGENT_DOM_SCROLL", scrollY: amount }).catch(() => {});
    return { success: true, action: 'scroll', message: 'Scroll humain DOM effectué.' };
  }

  if (detection.action === 'type') {
    await ensureCursorInjected(tabId);
    const target = detection.target;
    if (!target || !Number.isFinite(target.x) || !Number.isFinite(target.y)) {
      throw new Error('Invalid target bounding box');
    }
    const textToType = detection.text || '';
    const sequence = ++cursorMoveSeq;
    const start = getHumanCursorStart(tabId, target);
    const end = { x: target.x, y: target.y };
    for (const p of humanPath(start, end, cursorOptions.timing.steps)) {
      await showCursor(tabId, Math.round(p.x), Math.round(p.y), sequence, cursorOptions);
      await new Promise(r => setTimeout(r, cursorOptions.timing.minDelay + Math.random() * cursorOptions.timing.jitter));
    }
    const typeResult = await sendTabMessage(tabId, { type: 'AGENT_DOM_TYPE', x: end.x, y: end.y, text: textToType, submit: !!detection.submit, delayMs: cursorOptions.speed === 'slow' ? 120 : cursorOptions.speed === 'fast' ? 35 : 70 }, 20000);
    rememberHumanCursor(tabId, end, target);
    if (!typeResult || typeResult.ok === false || typeResult.success === false) throw new Error(typeResult?.error || 'DOM type failed');
    return { success: true, action: 'type', element: target.element, message: `Saisie humaine DOM effectuée: ${textToType}` };
  }

  const target = detection.target;
  if (!target || !Number.isFinite(target.x) || !Number.isFinite(target.y)) {
    throw new Error('Invalid target bounding box');
  }

  await ensureCursorInjected(tabId);

  const start = getHumanCursorStart(tabId, target);
  const end = { x: target.x, y: target.y };
  const points = humanPath(start, end, cursorOptions.timing.steps);
  const sequence = ++cursorMoveSeq;

  for (const p of points) {
    const x = Math.round(p.x);
    const y = Math.round(p.y);
    await showCursor(tabId, x, y, sequence, cursorOptions);
    await new Promise(r => setTimeout(r, cursorOptions.timing.minDelay + Math.random() * cursorOptions.timing.jitter));
  }

  await new Promise(r => setTimeout(r, cursorOptions.timing.pause + Math.random() * 90));
  await chrome.tabs.sendMessage(tabId, { type: "AGENT_CLICK_EVENT", event: { x: end.x, y: end.y, type: "mousePressed", button: "left" } }).catch(() => {});
  const clickResult = await sendTabMessage(tabId, { type: "AGENT_DOM_CLICK", x: end.x, y: end.y }, 12000);
  await chrome.tabs.sendMessage(tabId, { type: "AGENT_CURSOR_STATE", state: { x: end.x, y: end.y, visible: cursorOptions.mode !== 'hidden', moveSequence: sequence, sessionId: "session-" + tabId, turnId: "turn-human-dom" } }).catch(() => {});
  rememberHumanCursor(tabId, end, target);
  await new Promise(r => setTimeout(r, 250));

  if (!clickResult || clickResult.ok === false || clickResult.success === false) {
    throw new Error(clickResult?.error || 'DOM click failed');
  }

  return {
    success: true,
    action: detection.action || 'click',
    element: target.element,
    message: `Clic humain DOM effectué à (${end.x}, ${end.y}) sur ${target.element?.tagName || 'target'}.`
  };
}

function sendCDPCommand(msgId, tabId, method, cdpParams) {
  // ORGANIC CURSOR SYNERGY INTERCEPTOR
  if (method === "Input.dispatchMouseEvent" && cdpParams) {
    const { x, y, type } = cdpParams;
    if (type === "mouseMoved") {
      chrome.tabs.sendMessage(tabId, {
        type: "AGENT_CURSOR_STATE",
        state: {
          x,
          y,
          visible: true,
          moveSequence: ++cursorMoveSeq,
          sessionId: "session-" + tabId,
          turnId: "turn-cdp"
        }
      }).catch(() => {
        // Auto-inject and retry in case cursor DOM was purged by navigation
        ensureCursorInjected(tabId).then(() => {
          chrome.tabs.sendMessage(tabId, {
            type: "AGENT_CURSOR_STATE",
            state: { x, y, visible: true, moveSequence: cursorMoveSeq, sessionId: "session-" + tabId, turnId: "turn-cdp" }
          }).catch(() => {});
        });
      });
    } else if (type === "mousePressed" || type === "mouseReleased") {
      chrome.tabs.sendMessage(tabId, {
        type: "AGENT_CLICK_EVENT",
        event: {
          x,
          y,
          type,
          button: cdpParams.button || "left"
        }
      }).catch(() => {
        ensureCursorInjected(tabId).then(() => {
          chrome.tabs.sendMessage(tabId, {
            type: "AGENT_CLICK_EVENT",
            event: { x, y, type, button: cdpParams.button || "left" }
          }).catch(() => {});
        });
      });
    }
  }

  // Execute standard CDP command via debugger
  chrome.debugger.sendCommand({ tabId: tabId }, method, cdpParams || {}, (result) => {
    if (chrome.runtime.lastError) {
      console.error(`âš ï¸ [CDP Error] ${method}:`, chrome.runtime.lastError.message);
      sendResponse(msgId, { success: false, error: chrome.runtime.lastError.message });
    } else {
      sendResponse(msgId, { success: true, result });
    }
  });
}

// Injects the custom spring-physics cursor script if not already present
async function ensureCursorInjected(tabId) {
  const tabInfo = await chrome.tabs.get(tabId);
  if (!tabInfo || isSystemTab(tabInfo)) {
    throw new Error(`Cannot inject cursor into restricted tab: ${tabInfo?.url || tabId}`);
  }

  const ping = async () => await new Promise((resolve) => {
    chrome.tabs.sendMessage(tabId, { type: "CONTENT_PING" }, (res) => {
      resolve(!(chrome.runtime.lastError || !res || !res.ok));
    });
  });

  for (let attempt = 0; attempt < 20; attempt++) {
    const currentTab = await chrome.tabs.get(tabId);
    if (currentTab.status !== 'loading') break;
    await new Promise(r => setTimeout(r, 250));
  }

  if (await ping()) return;

  await chrome.scripting.executeScript({
    target: { tabId },
    files: ["sinew_cursor.js"]
  });

  for (let attempt = 0; attempt < 10; attempt++) {
    if (await ping()) {
      console.log(`ðŸ§¬ [Bridge] Injected biological cursor overlay into tab ${tabId}`);
      return;
    }
    await new Promise(r => setTimeout(r, 200));
  }

  throw new Error(`Content script injection failed for tab ${tabId}`);
}

function reportOpenTabs(responseId = null) {
  chrome.tabs.query({}, (tabs) => {
    // Filter out restricted pages before sending to proxy
    const debuggableTabs = tabs.filter(t => !isSystemTab(t));
    
    const tabList = debuggableTabs.map(t => ({
      id: t.id,
      title: t.title,
      url: t.url,
      active: t.active
    }));
    
    if (responseId) {
      sendResponse(responseId, { tabs: tabList });
    } else {
      sendMsg({ type: "tabs_report", tabs: tabList });
    }
  });
}

// Listen to browser-level events and report in real-time to Node.js proxy
chrome.tabs.onCreated.addListener((tab) => {
  if (!isSystemTab(tab)) {
    broadcastTabEvent("Target.targetCreated", tab);
  }
});

chrome.tabs.onUpdated.addListener((tabId, changeInfo, tab) => {
  if (!isSystemTab(tab)) {
    broadcastTabEvent("Target.targetInfoChanged", tab);
  } else {
    // If it became a system tab, destroy it in the proxy client list
    sendMsg({
      type: "target_destroyed",
      tabId: tabId
    });
  }
});

chrome.tabs.onRemoved.addListener((tabId) => {
  attachedTabs.delete(tabId);
  updateStorageState();
  sendMsg({
    type: "target_destroyed",
    tabId: tabId
  });
});

function broadcastTabEvent(method, tab) {
  sendMsg({
    type: "target_event",
    method: method,
    tab: {
      id: tab.id,
      title: tab.title,
      url: tab.url,
      active: tab.active
    }
  });
}

// Forward raw debugger events to the local proxy server
chrome.debugger.onEvent.addListener((source, method, params) => {
  sendMsg({
    type: "event",
    tabId: source.tabId,
    method: method,
    params: params
  });
});

chrome.debugger.onDetach.addListener((source, reason) => {
  console.log(`ðŸ§¬ [Bridge] Detached from tab ${source.tabId} because: ${reason}`);
  attachedTabs.delete(source.tabId);
  updateStorageState();
  
  sendMsg({
    type: "detached",
    tabId: source.tabId,
    reason
  });
});

// Cursor arrival notification from tab content script
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.type === "AGENT_CURSOR_ARRIVED") {
    // Notify proxy of completion
    sendMsg({
      type: "cursor_arrived",
      tabId: sender.tab.id,
      moveSequence: message.moveSequence
    });
    sendResponse({ ok: true });
  }
  else if (message.type === "TAB_LOADED") {
    const tabId = sender.tab.id;
    if (attachedTabs.has(tabId)) {
      ensureCursorInjected(tabId);
    }
    sendResponse({ ok: true });
  }
  else if (message.type === "AGENT_SAVE_MACRO") {
    sendMsg({
      type: "save_macro",
      tabId: sender.tab ? sender.tab.id : null,
      macro: message.macro
    });
    sendResponse({ ok: true });
  }
  return true;
});



function updateStorageState() {
  getDiagnosticsViaProxy().then((diagnostics) => {
    chrome.storage.local.set({
      connected: isBridgeConnected(),
      attachedCount: attachedTabs.size,
      lastNativeError,
      lastConnectedAt,
      diagnostics
    });
  });
}

function getDiagnosticsViaProxy() {
  return new Promise((resolve) => {
    try {
      const url = bridgeSecret ? `http://localhost:29002/api/diagnostics?token=${encodeURIComponent(bridgeSecret)}` : 'http://localhost:29002/api/diagnostics';
      fetch(url, { cache: 'no-store' })
        .then(res => res.ok ? res.json() : null)
        .then(data => resolve(data || null))
        .catch(() => resolve(null));
    } catch {
      resolve(null);
    }
  });
}

async function restartNativeBridge() {
  lastNativeError = null;
  try {
    if (nativePort) {
      try { nativePort.disconnect(); } catch {}
      nativePort = null;
    }
    if (reconnectTimer) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
    await new Promise(resolve => setTimeout(resolve, 250));
    connect();
    await new Promise(resolve => setTimeout(resolve, 750));
    updateStorageState();
    return { success: true };
  } catch (err) {
    lastNativeError = err.message;
    updateStorageState();
    return { success: false, error: err.message };
  }
}

// Keep connection state fresh for popup UI
chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
  if (request.action === "get_status") {
    getDiagnosticsViaProxy().then((diagnostics) => {
      sendResponse({
        connected: isBridgeConnected(),
        attachedCount: attachedTabs.size,
        lastNativeError,
        lastConnectedAt,
        diagnostics
      });
    });
  } else if (request.action === "reconnect") {
    if (reconnectTimer) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
    connect();
    setTimeout(() => {
      updateStorageState();
      sendResponse({ success: true, connected: isBridgeConnected() });
    }, 250);
  } else if (request.action === "restart_bridge") {
    restartNativeBridge().then(sendResponse);
  }
  return true;
});

// Setup periodic alarm to keep the background service worker alive and ensure connection
chrome.alarms.create("keep_alive_alarm", { periodInMinutes: 0.2 });
chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === "keep_alive_alarm") {
    if (!isBridgeConnected()) {
      console.log("🧬 [Bridge background] Connection inactive. Reconnecting via alarm...");
      connect();
    }
  }
});

chrome.runtime.onStartup?.addListener(() => {
  console.log("🧬 [Bridge background] Chrome startup detected. Connecting native host...");
  connect();
});

chrome.runtime.onInstalled?.addListener(() => {
  console.log("🧬 [Bridge background] Extension installed/updated. Connecting native host...");
  connect();
});

// Auto connect immediately whenever the service worker is loaded
connect();

// ==========================================================
// Hot Reload / Auto-Update Lifecycle (Sinew style)
// ==========================================================
if (chrome.runtime.onUpdateAvailable) {
  chrome.runtime.onUpdateAvailable.addListener((details) => {
    console.log(`🧬 [Bridge] New version available: ${details.version}. Hot-reloading when idle...`);
    const checkIdleAndReload = () => {
      if (attachedTabs.size === 0) {
        console.log("🧬 [Bridge] System idle. Reloading extension now.");
        chrome.runtime.reload();
      } else {
        setTimeout(checkIdleAndReload, 10000);
      }
    };
    checkIdleAndReload();
  });
}

// Keep-alive port listener from content scripts
chrome.runtime.onConnect.addListener((port) => {
  if (port.name === "sinew-keep-alive") {
    port.onDisconnect.addListener(() => {
      // Automatic re-connection handled by client tabs
    });
  }
});

