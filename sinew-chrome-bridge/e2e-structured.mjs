import { spawn } from 'child_process';

const child = spawn(process.execPath, ['sinew-chrome-bridge/mcp_server.js'], {
  cwd: process.cwd(),
  stdio: ['pipe', 'pipe', 'pipe'],
  windowsHide: true,
});

const results = {};
let stdout = '';
let settled = false;

function send(message) {
  child.stdin.write(JSON.stringify(message) + '\n');
}

function parseContent(msg) {
  const text = msg.result?.content?.[0]?.text || '';
  try { return JSON.parse(text); } catch { return { raw: text }; }
}

function fail(reason, payload = null) {
  if (settled) return;
  settled = true;
  child.kill();
  console.error(reason);
  if (payload) console.error(JSON.stringify(payload, null, 2));
  process.exit(1);
}

const timeout = setTimeout(() => fail('Structured MCP E2E timeout', results), 60000);

child.stdout.on('data', chunk => {
  stdout += chunk.toString();
  const lines = stdout.split(/\r?\n/).filter(Boolean);
  for (const line of lines) {
    let msg;
    try { msg = JSON.parse(line); } catch { continue; }

    if (msg.id === 2) {
      results.tools = msg.result?.tools?.map(tool => tool.name) || [];
      for (const name of ['open_browser', 'page_snapshot', 'query_selector', 'wait_for_selector', 'click_selector', 'type_selector', 'evaluate']) {
        if (!results.tools.includes(name)) fail(`Missing MCP tool: ${name}`, results);
      }
      send({ jsonrpc: '2.0', id: 3, method: 'tools/call', params: { name: 'open_browser', arguments: { url: 'https://example.com' } } });
    }

    if (msg.id === 3) {
      results.open = parseContent(msg);
      if (!results.open.success) fail('open_browser failed', results);
      send({ jsonrpc: '2.0', id: 4, method: 'tools/call', params: { name: 'wait_for_selector', arguments: { selector: 'h1', timeoutMs: 5000 } } });
    }

    if (msg.id === 4) {
      results.wait = parseContent(msg);
      if (!results.wait.success) fail('wait_for_selector failed', results);
      send({ jsonrpc: '2.0', id: 5, method: 'tools/call', params: { name: 'query_selector', arguments: { selector: 'h1' } } });
    }

    if (msg.id === 5) {
      results.query = parseContent(msg);
      if (!results.query.success || results.query.result?.text !== 'Example Domain') fail('query_selector failed', results);
      clearTimeout(timeout);
      settled = true;
      child.kill();
      console.log('Structured MCP E2E OK');
      console.log(JSON.stringify(results, null, 2));
      process.exit(0);
    }
  }
});

child.stderr.on('data', () => {});
child.on('error', err => fail(err.message, results));

send({ jsonrpc: '2.0', id: 1, method: 'initialize', params: {} });
send({ jsonrpc: '2.0', id: 2, method: 'tools/list', params: {} });
