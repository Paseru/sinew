import { spawn } from 'child_process';

const task = process.argv.slice(2).join(' ') || 'ouvre julienpiron.fr puis clic sur le hamburger puis referme le puis clic sur la carte trinity';
const child = spawn(process.execPath, ['sinew-chrome-bridge/mcp_server.js'], {
  cwd: process.cwd(),
  stdio: ['pipe', 'pipe', 'pipe'],
  windowsHide: true,
});

let stdout = '';
let stderr = '';
let settled = false;

const timeout = setTimeout(() => {
  if (settled) return;
  settled = true;
  child.kill();
  console.error('E2E timeout');
  console.error(stderr);
  process.exit(1);
}, 120000);

function send(message) {
  child.stdin.write(JSON.stringify(message) + '\n');
}

child.stdout.on('data', chunk => {
  stdout += chunk.toString();
  const lines = stdout.split(/\r?\n/).filter(Boolean);
  for (const line of lines) {
    let msg;
    try { msg = JSON.parse(line); } catch { continue; }
    if (msg.id === 2) {
      clearTimeout(timeout);
      settled = true;
      child.kill();
      const text = msg.result?.content?.[0]?.text || '';
      let payload;
      try { payload = JSON.parse(text); } catch { payload = null; }
      if (!payload?.success) {
        console.error('E2E failed: browser task returned unsuccessful result');
        console.error(text || line);
        process.exit(1);
      }
      const serialized = JSON.stringify(payload);
      for (const expected of ['menu-button', 'trinity-card', 'human_cdp_click']) {
        if (!serialized.includes(expected)) {
          console.error(`E2E failed: expected ${expected} in result`);
          console.error(serialized);
          process.exit(1);
        }
      }
      console.log('E2E OK: Sinew Chrome local MCP controlled Chrome successfully.');
      console.log(text);
      process.exit(0);
    }
  }
});

child.stderr.on('data', chunk => {
  stderr += chunk.toString();
});

child.on('error', err => {
  clearTimeout(timeout);
  console.error(err);
  process.exit(1);
});

send({ jsonrpc: '2.0', id: 1, method: 'initialize', params: {} });
send({
  jsonrpc: '2.0',
  id: 2,
  method: 'tools/call',
  params: {
    name: 'run_browser_agent',
    arguments: { task },
  },
});
