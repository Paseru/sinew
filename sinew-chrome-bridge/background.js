// 🧬 Sinew Chrome Bridge — Service Worker (Manifest V3)
// Upgraded to SOTA Sinew-grade sequential execution queue and automatic biological cursor physics injection.
// Pure Native Messaging connection (No fallback to match Codex standard).

let nativePort = null;
let reconnectTimer = null;
let lastNativeError = null;

// Registry of active attached debuggers
const attachedTabs = new Set();
let cursorMoveSeq = 0;

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
    
    const { id, command, params } = msg;
    if (!command) return;
    
    console.log(`ðŸ§¬ [Bridge] Command received: ${command} (id=${id})`, msg);
    
    switch (command) {
      case "list_tabs":
        reportOpenTabs(id);
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
                    ensureCursorInjected(attachTabId).then(() => {
                      chrome.tabs.sendMessage(attachTabId, { type: "AGENT_STATUS_CHANGE", status: "active" }).catch(() => {});
                    });
                  } else {
                    console.error("âš ï¸ [Bridge] Debugger attachment failed:", errMsg);
                    sendResponse(id, { success: false, error: errMsg });
                  }
                } else {
                  attachedTabs.add(attachTabId);
                  console.log(`ðŸ§¬ [Bridge] Debugger attached to tab ${attachTabId}`);
                  sendResponse(id, { success: true });
                  ensureCursorInjected(attachTabId).then(() => {
                    chrome.tabs.sendMessage(attachTabId, { type: "AGENT_STATUS_CHANGE", status: "active" }).catch(() => {});
                  });
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

      case "cdp_command":
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
                    ensureCursorInjected(cdpTabId).then(() => {
                      sendCDPCommand(id, cdpTabId, method, cdpParams);
                      resolve();
                    });
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
        const { task: silentTask } = params;
        
        runLocked(async () => {
          return new Promise(async (resolve) => {
            const urlRegex = /(https?:\/\/[^\s]+)/g;
            let match = silentTask.match(urlRegex);
            let urlToNavigate = match ? match[0] : null;

            if (!urlToNavigate) {
              const domainRegex = /\b([a-zA-Z0-9-]+\.[a-zA-Z]{2,})(?:\/[^\s]*)?\b/i;
              const domainMatch = silentTask.match(domainRegex);
              if (domainMatch) {
                urlToNavigate = "https://" + domainMatch[0];
              }
            }

            if (urlToNavigate) {
              console.log(`🧬 [Bridge] Silent navigating tab ${silentTabId} to ${urlToNavigate}`);
              chrome.tabs.update(silentTabId, { url: urlToNavigate });
              await new Promise(r => setTimeout(r, 4500));
            }

            const actionTasks = buildSilentActionTasks(silentTask);

            const runAction = (taskText) => new Promise((actionResolve) => {
              ensureCursorInjected(silentTabId).then(() => {
                chrome.tabs.sendMessage(silentTabId, { type: "RUN_SILENT_TASK", task: taskText }, async (response) => {
                  if (chrome.runtime.lastError) {
                    actionResolve({ success: false, error: chrome.runtime.lastError.message, task: taskText });
                    return;
                  }
                  if (!response || response.success === false) {
                    actionResolve({ ...(response || { success: false, error: 'No target response' }), task: taskText });
                    return;
                  }
                  try {
                    const performed = await performHumanCdpAction(silentTabId, response, taskText);
                    actionResolve({ ...performed, task: taskText, target: response.target });
                  } catch (err) {
                    actionResolve({ success: false, error: err.message, task: taskText, target: response.target });
                  }
                });
              }).catch((err) => {
                actionResolve({ success: false, error: err.message, task: taskText });
              });
            });

            const results = [];
            for (const taskText of actionTasks) {
              results.push(await runAction(taskText));
              await new Promise(r => setTimeout(r, 900));
            }

            const failed = results.find(r => r && r.success === false);
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
  const points = [];
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const dist = Math.max(1, Math.hypot(dx, dy));
  const curve = Math.min(120, Math.max(24, dist * 0.18));
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
  return points;
}

async function showCursor(tabId, x, y, moveSequence = ++cursorMoveSeq) {
  await chrome.tabs.sendMessage(tabId, {
    type: "AGENT_CURSOR_STATE",
    state: { x, y, visible: true, moveSequence, sessionId: "session-" + tabId, turnId: "turn-human-cdp" }
  }).catch(() => {});
}

async function performHumanCdpAction(tabId, detection, taskText) {
  if (detection.action === 'scroll') {
    await attachDebuggerIfNeeded(tabId);
    await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseWheel", x: 500, y: 500, deltaX: 0, deltaY: detection.scrollY || 500 });
    return { success: true, action: 'scroll', message: 'Scroll CDP humain effectué.' };
  }

  const target = detection.target;
  if (!target || !Number.isFinite(target.x) || !Number.isFinite(target.y)) {
    throw new Error('Invalid target bounding box');
  }

  await attachDebuggerIfNeeded(tabId);
  await ensureCursorInjected(tabId);

  const start = { x: Math.max(24, target.x - 260), y: Math.max(24, target.y + 160) };
  const end = { x: target.x, y: target.y };
  const points = humanPath(start, end, 38);
  const sequence = ++cursorMoveSeq;

  for (const p of points) {
    const x = Math.round(p.x);
    const y = Math.round(p.y);
    await showCursor(tabId, x, y, sequence);
    await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseMoved", x, y, button: "none" });
    await new Promise(r => setTimeout(r, 12 + Math.random() * 18));
  }

  await new Promise(r => setTimeout(r, 140 + Math.random() * 90));
  await chrome.tabs.sendMessage(tabId, { type: "AGENT_CLICK_EVENT", event: { x: end.x, y: end.y, type: "mousePressed", button: "left" } }).catch(() => {});
  await cdp(tabId, "Input.dispatchMouseEvent", { type: "mousePressed", x: end.x, y: end.y, button: "left", clickCount: 1 });
  await new Promise(r => setTimeout(r, 55 + Math.random() * 50));
  await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseReleased", x: end.x, y: end.y, button: "left", clickCount: 1 });
  await chrome.tabs.sendMessage(tabId, { type: "AGENT_CURSOR_STATE", state: { x: end.x, y: end.y, visible: true, moveSequence: sequence, sessionId: "session-" + tabId, turnId: "turn-human-cdp" } }).catch(() => {});
  await new Promise(r => setTimeout(r, 220));

  return {
    success: true,
    action: detection.action || 'click',
    element: target.element,
    message: `Clic humain CDP effectué à (${end.x}, ${end.y}) sur ${target.element?.tagName || 'target'}.`
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
  chrome.storage.local.set({
    connected: isBridgeConnected(),
    attachedCount: attachedTabs.size,
    lastNativeError
  });
}

// Keep connection state fresh for popup UI
chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
  if (request.action === "get_status") {
    sendResponse({
      connected: isBridgeConnected(),
      attachedCount: attachedTabs.size,
      lastNativeError
    });
  } else if (request.action === "reconnect") {
    connect();
    sendResponse({ success: true });
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

