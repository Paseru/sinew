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

// Execute the smart browser automation natively and silently (Codex mode)
async function executeBrowserTask(task) {
  log(`Executing native Chrome silent action: "${task}"`);
  
  try {
    const tabs = await getJSON("http://localhost:29002/json");
    if (!tabs || tabs.length === 0) {
      return "Error: No active tabs found in Chrome. Make sure Chrome is open and the Sinew extension is connected.";
    }
    
    const tab = tabs[0];
    log(`Executing silently on tab: ${tab.title} (ID: ${tab.id})`);
    
    const url = `http://localhost:29002/api/execute_silent_task?tabId=${tab.id}&task=${encodeURIComponent(task)}`;
    const result = await getJSON(url);
    return JSON.stringify(result);
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
