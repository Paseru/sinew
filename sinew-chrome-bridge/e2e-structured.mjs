import { spawn } from 'child_process';

const child = spawn(process.execPath, ['sinew-chrome-bridge/mcp_server.js'], {
  cwd: process.cwd(),
  stdio: ['pipe', 'pipe', 'pipe'],
  windowsHide: true,
});

const results = {};
let stdout = '';
let settled = false;
const handledIds = new Set();

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
  let newlineIndex;
  while ((newlineIndex = stdout.search(/\r?\n/)) >= 0) {
    const line = stdout.slice(0, newlineIndex).trim();
    stdout = stdout.slice(newlineIndex + (stdout[newlineIndex] === '\r' && stdout[newlineIndex + 1] === '\n' ? 2 : 1));
    if (!line) continue;
    let msg;
    try { msg = JSON.parse(line); } catch { continue; }
    if (msg.id && handledIds.has(msg.id)) continue;
    if (msg.id) handledIds.add(msg.id);

    if (msg.id === 2) {
      results.tools = msg.result?.tools?.map(tool => tool.name) || [];
      for (const name of ['open_browser', 'page_snapshot', 'query_selector', 'wait_for_selector', 'click_selector', 'type_selector', 'press_key', 'select_option', 'evaluate']) {
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
      send({ jsonrpc: '2.0', id: 6, method: 'tools/call', params: { name: 'evaluate', arguments: { expression: "(() => { document.body.setAttribute('data-sinew-e2e','ok'); return document.body.getAttribute('data-sinew-e2e'); })()" } } });
    }
    if (msg.id === 6) {
      results.setup = parseContent(msg);
      if (!results.setup.success) fail('evaluate setup failed', results);
      send({ jsonrpc: '2.0', id: 7, method: 'tools/call', params: { name: 'click_selector', arguments: { selector: 'a' } } });
    }

    if (msg.id === 7) {
      results.click = parseContent(msg);
      if (!results.click.success) fail('click_selector failed', results);
      send({ jsonrpc: '2.0', id: 8, method: 'tools/call', params: { name: 'open_browser', arguments: { url: 'https://www.google.com' } } });
    }

    if (msg.id === 8) {
      results.formOpen = parseContent(msg);
      if (!results.formOpen.success) fail('form open failed', results);
      send({ jsonrpc: '2.0', id: 9, method: 'tools/call', params: { name: 'evaluate', arguments: { expression: "(() => { document.body.insertAdjacentHTML('beforeend', '<select id=\"sinew-e2e-select\"><option value=\"one\">One</option><option value=\"two\">Two</option></select><input id=\"sinew-e2e-input\">'); return true; })()" } } });
    }

    if (msg.id === 9) {
      results.formSetup = parseContent(msg);
      if (!results.formSetup.success) fail('form setup failed', results);
      send({ jsonrpc: '2.0', id: 10, method: 'tools/call', params: { name: 'query_selector', arguments: { selector: 'textarea[name="q"], input[name="q"]' } } });
    }

    if (msg.id === 10) {
      results.formQuery = parseContent(msg);
      if (!results.formQuery.success) fail('post-setup query_selector failed', results);
      send({ jsonrpc: '2.0', id: 11, method: 'tools/call', params: { name: 'type_selector', arguments: { selector: 'textarea[name="q"], input[name="q"]', text: 'sinew test' } } });
    }

    if (msg.id === 11) {
      results.type = parseContent(msg);
      if (!results.type.success) fail('type_selector failed', results);
      send({ jsonrpc: '2.0', id: 12, method: 'tools/call', params: { name: 'press_key', arguments: { selector: 'textarea[name="q"], input[name="q"]', key: 'Enter', submit: false } } });
    }

    if (msg.id === 12) {
      results.press = parseContent(msg);
      if (!results.press.success || results.press.result?.key !== 'Enter') fail('press_key failed', results);
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
