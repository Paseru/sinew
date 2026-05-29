const http = require('http');
const WebSocket = require('./node_modules/ws');
const fs = require('fs');
const path = require('path');
const os = require('os');

const STATE_DIR = process.env.SINEW_CHROME_BRIDGE_DIR || path.join(process.env.LOCALAPPDATA || path.join(os.homedir(), 'AppData', 'Local'), 'Sinew', 'ChromeBridge');
let BRIDGE_SECRET = '';
try {
  const secretPath = path.join(STATE_DIR, 'bridge-secret.txt');
  if (fs.existsSync(secretPath)) {
    BRIDGE_SECRET = fs.readFileSync(secretPath, 'utf8').trim();
  }
} catch (e) {}

function getActiveTab() {
  return new Promise((resolve, reject) => {
    let url = 'http://localhost:29002/json';
    if (BRIDGE_SECRET) {
      url += `?token=${encodeURIComponent(BRIDGE_SECRET)}`;
    }
    http.get(url, (res) => {
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => {
        try {
          const list = JSON.parse(data);
          if (list.length > 0) {
            resolve(list[0]);
          } else {
            reject(new Error("No active tabs found. Please open Chrome."));
          }
        } catch (e) {
          reject(e);
        }
      });
    }).on('error', reject);
  });
}

async function run() {
  try {
    console.log("🧬 [Automation] Locating active Chrome tab...");
    const tab = await getActiveTab();
    console.log(`🧬 [Automation] Connected to Tab: "${tab.title}" (${tab.url})`);
    console.log(`🧬 [Automation] Connecting via WebSocket to: ${tab.webSocketDebuggerUrl}`);

    const ws = new WebSocket(tab.webSocketDebuggerUrl);

    let messageId = 0;
    const send = (method, params = {}) => {
      const id = ++messageId;
      const payload = { id, method, params };
      console.log(`🛰️ [CDP Send] ${method} (id=${id})`);
      ws.send(JSON.stringify(payload));
      return id;
    };

    ws.on('open', () => {
      console.log("🔌 [WebSocket] Connection established.");

      // 1. Enable Page commands
      send("Page.enable");

      // 2. Navigate to julienpiron.fr
      console.log("🌐 [CDP] Navigating to http://julienpiron.fr...");
      send("Page.navigate", { url: "http://julienpiron.fr" });

      // Wait for navigation and page load
      setTimeout(() => {
        // 3. Click the Hamburger menu
        console.log("🍔 [CDP] Clicking Hamburger menu...");
        send("Runtime.evaluate", {
          expression: `(() => {
            // Find hamburger or mobile menu button
            const btn = document.querySelector('.menu-toggle, .hamburger, button[aria-expanded], button, a') || 
                        Array.from(document.querySelectorAll('button, a')).find(el => el.innerText.toLowerCase().includes('menu'));
            if (btn) {
              btn.click();
              btn.dispatchEvent(new Event('click', { bubbles: true }));
              return "Hamburger clicked successfully";
            }
            return "No hamburger button found";
          })()`,
          returnByValue: true
        });

        // Wait 2.5 seconds, then close it
        setTimeout(() => {
          console.log("❌ [CDP] Closing Hamburger menu...");
          send("Runtime.evaluate", {
            expression: `(() => {
              const btn = document.querySelector('.menu-toggle, .hamburger, button[aria-expanded], button, a') || 
                          Array.from(document.querySelectorAll('button, a')).find(el => el.innerText.toLowerCase().includes('menu'));
              if (btn) {
                btn.click();
                btn.dispatchEvent(new Event('click', { bubbles: true }));
                return "Hamburger closed successfully";
              }
              return "No hamburger button found to close";
            })()`,
            returnByValue: true
          });

          // Wait 2.5 seconds, then click Trinity Card
          setTimeout(() => {
            console.log("🃏 [CDP] Clicking Trinity Card...");
            send("Runtime.evaluate", {
              expression: `(() => {
                const links = Array.from(document.querySelectorAll('a, h2, h3, p, div'));
                const trinity = links.find(el => el.textContent.toLowerCase().includes('trinity'));
                if (trinity) {
                  trinity.click();
                  trinity.dispatchEvent(new Event('click', { bubbles: true }));
                  // If it's a link or element, try triggering navigation or click directly
                  const parentLink = trinity.closest('a');
                  if (parentLink) parentLink.click();
                  return "Trinity card clicked successfully";
                }
                return "Trinity card not found";
              })()`,
              returnByValue: true
            });

            // Close connection after completed run
            setTimeout(() => {
              console.log("🏁 [CDP] Completed successfully. Closing WebSocket.");
              ws.close();
              process.exit(0);
            }, 3000);

          }, 2500);

        }, 2500);

      }, 4000); // Allow 4 seconds for complete initial page load
    });

    ws.on('message', (data) => {
      const msg = JSON.parse(data);
      if (msg.result) {
        console.log(`📥 [CDP Response] id=${msg.id}:`, JSON.stringify(msg.result));
      } else if (msg.error) {
        console.error(`⚠️ [CDP Error Response] id=${msg.id}:`, msg.error.message);
      }
    });

    ws.on('error', (err) => {
      console.error("❌ [WebSocket Error]:", err);
    });

  } catch (e) {
    console.error("❌ [Error in automation]:", e.message);
  }
}

run();
