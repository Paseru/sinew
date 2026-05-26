const readline = require('readline');
const http = require('http');
const WebSocket = require('ws');

// Log errors to stderr so they don't corrupt the stdout JSON-RPC stream
function log(msg) {
  console.error(`[MCP Log] ${msg}`);
}

// Helper to query active Chrome tabs
function getJSON(url) {
  return new Promise((resolve, reject) => {
    http.get(url, (res) => {
      let data = '';
      res.on('data', (chunk) => data += chunk);
      res.on('end', () => {
        try {
          resolve(JSON.parse(data));
        } catch (e) {
          reject(e);
        }
      });
    }).on('error', reject);
  });
}

function extractUrl(text) {
  const urlRegex = /(https?:\/\/[^\s]+)/g;
  const domainRegex = /([a-zA-Z0-9-]+\.[a-zA-Z]{2,6})/g;
  
  let match = text.match(urlRegex);
  if (match) return match[0];
  
  match = text.match(domainRegex);
  if (match) {
    const domain = match[0];
    if (domain.toLowerCase() !== 'localhost' && !domain.startsWith('api.')) {
      return `https://${domain}`;
    }
  }
  return null;
}

let globalMouseX = 500;
let globalMouseY = 400;

function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

function generateBezierPath(startX, startY, endX, endY, steps = 15) {
  const points = [];
  const ctrlX1 = startX + (endX - startX) * 0.25 + (Math.random() - 0.5) * 80;
  const ctrlY1 = startY + (endY - startY) * 0.25 + (Math.random() - 0.5) * 80;
  const ctrlX2 = startX + (endX - startX) * 0.75 + (Math.random() - 0.5) * 80;
  const ctrlY2 = startY + (endY - startY) * 0.75 + (Math.random() - 0.5) * 80;

  for (let i = 0; i <= steps; i++) {
    const t = i / steps;
    const x = Math.round(
      Math.pow(1 - t, 3) * startX +
      3 * Math.pow(1 - t, 2) * t * ctrlX1 +
      3 * (1 - t) * Math.pow(t, 2) * ctrlX2 +
      Math.pow(t, 3) * endX
    );
    const y = Math.round(
      Math.pow(1 - t, 3) * startY +
      3 * Math.pow(1 - t, 2) * t * ctrlY1 +
      3 * (1 - t) * Math.pow(t, 2) * ctrlY2 +
      Math.pow(t, 3) * endY
    );
    points.push({ x, y });
  }
  return points;
}

// Execute the smart browser automation natively
async function executeBrowserTask(task) {
  log(`Executing native Chrome action: "${task}"`);
  let ws = null;
  
  try {
    const tabs = await getJSON("http://localhost:29002/json");
    if (!tabs || tabs.length === 0) {
      return "Error: No active tabs found in Chrome. Make sure Chrome is open and the Sinew extension is connected.";
    }
    
    const tab = tabs[0];
    log(`Connecting to tab: ${tab.title} (ID: ${tab.id})`);
    
    return await new Promise((resolve, reject) => {
      ws = new WebSocket(tab.webSocketDebuggerUrl);
      const pendingCDPRequests = new Map();
      let messageId = 1;

      const sendCDP = (method, params = {}) => {
        return new Promise((res, rej) => {
          const id = messageId++;
          pendingCDPRequests.set(id, { resolve: res, reject: rej });
          ws.send(JSON.stringify({ id, method, params }));
        });
      };

      ws.on('message', (data) => {
        try {
          const msg = JSON.parse(data);
          if (msg.id && pendingCDPRequests.has(msg.id)) {
            const { resolve: res, reject: rej } = pendingCDPRequests.get(msg.id);
            pendingCDPRequests.delete(msg.id);
            if (msg.error) {
              rej(new Error(msg.error.message || JSON.stringify(msg.error)));
            } else {
              res(msg.result);
            }
          }
        } catch (e) {
          log(`Error parsing ws message: ${e}`);
        }
      });

      ws.on('error', (err) => {
        reject(err);
      });

      ws.on('open', async () => {
        try {
          // Navigation check
          const urlToNavigate = extractUrl(task);
          if (urlToNavigate) {
            log(`Navigating to: ${urlToNavigate}`);
            await sendCDP("Page.navigate", { url: urlToNavigate });
            await sleep(4500); // sleep to let the page load
          }

          log("Running SOTA element locator...");
          const expression = `
            (() => {
              const taskText = ${JSON.stringify(task).toLowerCase()};
              
              let action = "click";
              if (taskText.includes("type") || taskText.includes("tape") || taskText.includes("saisir") || taskText.includes("écrire") || taskText.includes("ecris") || taskText.includes("saisis")) {
                action = "type";
              } else if (taskText.includes("scroll") || taskText.includes("défiler") || taskText.includes("descendre") || taskText.includes("monter")) {
                action = "scroll";
              }

              let textToType = "";
              if (action === "type") {
                const quoteMatch = ${JSON.stringify(task)}.match(/["'(]([^"')]+)["')]/);
                if (quoteMatch) {
                  textToType = quoteMatch[1];
                } else {
                  const words = ${JSON.stringify(task)}.split(/\\b(?:type|tape|saisir|écrire|ecris|saisis)\\b/i);
                  if (words.length > 1) {
                    textToType = words[1].trim().replace(/^dans\\s+|^sur\\s+|\\w+\\s+/i, "");
                  }
                }
              }

              if (action === "scroll") {
                const direction = (taskText.includes("up") || taskText.includes("monter") || taskText.includes("haut")) ? -1 : 1;
                const amount = Math.round(window.innerHeight * 0.6 * direction);
                window.scrollBy({ top: amount, behavior: "smooth" });
                return { found: true, action: "scroll", scrolledAmount: amount, message: "Défilement effectué avec succès." };
              }

              const elements = Array.from(document.querySelectorAll('button, a, input, select, textarea, [role="button"], [onclick], div, span, svg, li, summary'));
              
              const urlToNavigate = ${JSON.stringify(urlToNavigate)};
              let targetText = taskText;
              if (urlToNavigate) {
                targetText = targetText.replace(urlToNavigate.toLowerCase(), "");
              }
              
              const cleanTask = targetText
                .replace(/\\b(cliquez|clique|cliquer|click|ouvrir|ouvre|open|press|selectionne|sélectionne|va sur|aller|type|tape|saisir|écrire|ecris|saisis|dans|sur|le|la|les|un|une|et|du|de|des|site|web|page|url|navigate|navigue)\\b/g, " ")
                .trim();
              const queryWordsRaw = cleanTask.split(/\\s+/).filter(w => w.length >= 1);
              const semanticWords = [];
              if (queryWordsRaw.some(w => w === "hamburger" || w === "burger" || w === "menu")) {
                semanticWords.push("menu", "hamburger", "burger", "nav", "toggle");
              }
              if (queryWordsRaw.some(w => w === "bouton" || w === "button")) {
                semanticWords.push("btn", "button", "bouton");
              }
              if (queryWordsRaw.some(w => w === "recherche" || w === "chercher" || w === "search")) {
                semanticWords.push("search", "query", "q", "recherche", "find");
              }
              const queryWords = Array.from(new Set([...queryWordsRaw, ...semanticWords]));

              let bestEl = null;
              let bestScore = -1;

              elements.forEach(el => {
                const rect = el.getBoundingClientRect();
                if (rect.width === 0 || rect.height === 0) return;
                
                if (rect.width * rect.height > window.innerWidth * window.innerHeight * 0.4) return;

                const style = window.getComputedStyle(el);
                if (style.display === "none" || style.visibility === "hidden" || style.opacity === "0") return;

                const text = (el.innerText || el.textContent || "").toLowerCase().trim();
                const placeholder = (el.getAttribute("placeholder") || "").toLowerCase();
                const ariaLabel = (el.getAttribute("aria-label") || "").toLowerCase();
                const id = (el.id || "").toLowerCase();
                const className = (typeof el.className === "string" ? el.className : "").toLowerCase();
                const value = (el.value || "").toLowerCase();
                const name = (el.getAttribute("name") || "").toLowerCase();
                const role = (el.getAttribute("role") || "").toLowerCase();

                let score = 0;

                queryWords.forEach(word => {
                  if (text.includes(word)) score += 50;
                  if (placeholder.includes(word)) score += 60;
                  if (ariaLabel.includes(word)) score += 60;
                  if (id.includes(word)) score += 40;
                  if (name.includes(word)) score += 40;
                  if (value.includes(word)) score += 30;
                  if (className.includes(word)) score += 10;
                });

                if (action === "type" && (el.tagName === "INPUT" || el.tagName === "TEXTAREA")) {
                  score += 30;
                }
                if (action === "click" && (el.tagName === "BUTTON" || el.tagName === "A" || role === "button" || style.cursor === "pointer")) {
                  score += 20;
                }

                if (score > bestScore && score > 0) {
                  bestScore = score;
                  bestEl = el;
                }
              });

              if (bestEl) {
                const rect = bestEl.getBoundingClientRect();
                const x = Math.round(rect.left + rect.width / 2);
                const y = Math.round(rect.top + rect.height / 2);
                
                return {
                  found: true,
                  action: action,
                  x: x,
                  y: y,
                  text: (bestEl.innerText || bestEl.value || "").slice(0, 100),
                  tagName: bestEl.tagName,
                  id: bestEl.id,
                  className: typeof bestEl.className === 'string' ? bestEl.className : "",
                  textToType: textToType,
                  score: bestScore
                };
              }

              return { found: false, action: action, message: "Aucun élément interactif pertinent trouvé pour cette tâche." };
            })()
          `;

          const evalRes = await sendCDP("Runtime.evaluate", { expression, returnByValue: true });
          const result = evalRes.result?.value;

          if (!result || !result.found) {
            const msg = result ? result.message : "Aucun élément interactif pertinent trouvé pour cette tâche.";
            resolve(JSON.stringify({ success: false, message: msg }));
            return;
          }

          if (result.action === "scroll") {
            resolve(JSON.stringify({ success: true, action: "scroll", message: result.message }));
            return;
          }

          log(`Found target element: ${result.tagName} (Score: ${result.score}) at coordinates: (${result.x}, ${result.y})`);

          // Generate smooth Bezier path to target coordinates
          const path = generateBezierPath(globalMouseX, globalMouseY, result.x, result.y, 15);
          for (const pt of path) {
            await sendCDP("Input.dispatchMouseEvent", {
              type: "mouseMoved",
              x: pt.x,
              y: pt.y
            });
            await sleep(20);
          }
          globalMouseX = result.x;
          globalMouseY = result.y;

          if (result.action === "click") {
            // CDP click simulation (Bot-evasive & organic)
            await sendCDP("Input.dispatchMouseEvent", { type: "mousePressed", x: result.x, y: result.y, button: "left", clickCount: 1 });
            await sleep(60);
            await sendCDP("Input.dispatchMouseEvent", { type: "mouseReleased", x: result.x, y: result.y, button: "left", clickCount: 1 });

            resolve(JSON.stringify({
              success: true,
              action: "click",
              element: { tagName: result.tagName, id: result.id, className: result.className },
              message: `Déplacement fluide effectué et clic physique simulé à (${result.x}, ${result.y}) sur ${result.tagName}.`
            }));
          } 
          else if (result.action === "type") {
            // Click to focus first
            await sendCDP("Input.dispatchMouseEvent", { type: "mousePressed", x: result.x, y: result.y, button: "left", clickCount: 1 });
            await sleep(60);
            await sendCDP("Input.dispatchMouseEvent", { type: "mouseReleased", x: result.x, y: result.y, button: "left", clickCount: 1 });

            log(`Simulating organic typing for text: "${result.textToType}"`);
            const typeExpression = `
              new Promise((resolve) => {
                const el = document.elementFromPoint(${result.x}, ${result.y});
                if (!el) {
                  resolve({ success: false, error: "Element not found at coordinates" });
                  return;
                }
                
                el.focus();
                if (el.tagName === "INPUT" || el.tagName === "TEXTAREA") {
                  el.value = "";
                }
                
                const text = ${JSON.stringify(result.textToType)};
                const chars = text.split("");
                let index = 0;
                
                function typeNext() {
                  if (index < chars.length) {
                    const char = chars[index++];
                    if (el.tagName === "INPUT" || el.tagName === "TEXTAREA") {
                      el.value += char;
                    }
                    el.dispatchEvent(new Event("input", { bubbles: true }));
                    el.dispatchEvent(new Event("change", { bubbles: true }));
                    
                    const keyOpts = { key: char, charCode: char.charCodeAt(0), keyCode: char.charCodeAt(0), bubbles: true };
                    el.dispatchEvent(new KeyboardEvent("keydown", keyOpts));
                    el.dispatchEvent(new KeyboardEvent("keypress", keyOpts));
                    el.dispatchEvent(new KeyboardEvent("keyup", keyOpts));
                    
                    setTimeout(typeNext, 35 + Math.random() * 40);
                  } else {
                    resolve({ success: true, typed: text });
                  }
                }
                
                if (chars.length === 0) {
                  resolve({ success: true, typed: "" });
                } else {
                  typeNext();
                }
              })
            `;

            const typeRes = await sendCDP("Runtime.evaluate", { expression: typeExpression, awaitPromise: true, returnByValue: true });
            const typeResult = typeRes.result?.result?.value;

            resolve(JSON.stringify({
              success: typeResult ? typeResult.success : false,
              action: "type",
              typedText: result.textToType,
              element: { tagName: result.tagName, id: result.id, className: result.className },
              message: `Déplacement fluide effectué et texte "${result.textToType}" saisi caractère par caractère dans ${result.tagName}.`
            }));
          }
        } catch (e) {
          reject(e);
        }
      });
    });
    
  } catch (err) {
    return `Error: ${err.message}`;
  } finally {
    if (ws) {
      try {
        ws.close();
      } catch (e) {
        log(`Error closing websocket: ${e}`);
      }
    }
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
      const response = {
        jsonrpc: "2.0",
        id: id,
        result: {
          protocolVersion: "2024-11-05",
          capabilities: {
            tools: {}
          },
          serverInfo: {
            name: "sinew-chrome-mcp-native",
            version: "1.0.0"
          }
        }
      };
      console.log(JSON.stringify(response));
    } 
    else if (method === 'tools/list') {
      const response = {
        jsonrpc: "2.0",
        id: id,
        result: {
          tools: [
            {
              name: "run_browser_agent",
              description: "Exécute une tâche de navigation ou d'interaction avec le navigateur Chrome de l'utilisateur de manière 100% native et locale sans clé API externe.",
              inputSchema: {
                type: "object",
                properties: {
                  task: {
                    type: "string",
                    description: "Description de l'action à faire (ex: 'navigue vers julienpiron.fr et ouvre le menu')"
                  }
                },
                required: ["task"]
              }
            }
          ]
        }
      };
      console.log(JSON.stringify(response));
    } 
    else if (method === 'tools/call') {
      const toolName = params.name;
      const args = params.arguments || {};
      
      log(`Calling tool: ${toolName}`);
      
      if (toolName === 'run_browser_agent') {
        const resultText = await executeBrowserTask(args.task);
        const response = {
          jsonrpc: "2.0",
          id: id,
          result: {
            content: [
              {
                type: "text",
                text: resultText
              }
            ]
          }
        };
        console.log(JSON.stringify(response));
      } else {
        const response = {
          jsonrpc: "2.0",
          id: id,
          error: {
            code: -32601,
            message: `Method not found: ${toolName}`
          }
        };
        console.log(JSON.stringify(response));
      }
    } 
    else {
      // Mock unhandled requests as success or generic empty response to prevent client hang
      if (id !== undefined) {
        console.log(JSON.stringify({ jsonrpc: "2.0", id: id, result: {} }));
      }
    }
  } catch (err) {
    log(`Error handling line: ${err.message}`);
  }
});
