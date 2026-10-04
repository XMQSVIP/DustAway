// Gracefully close the exact failed seed instance after independent inspection.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
const root = resolve(process.cwd());
assert.equal(process.env.COMPUTERNAME, 'DESKTOP-24NJTT6');
assert.equal(root, join(process.env.LOCALAPPDATA, 'DustAwayAcceptance', '20260930-software'));
const report = JSON.parse(await readFile(join(root, 'brave-seed-v2-live-inspection.json'), 'utf8'));
const status = JSON.parse(await readFile(join(root, 'brave-evidence-v2-seed-status.json'), 'utf8'));
assert.equal(report.pid, 7460);
assert.equal(status.pid, 7460);
assert.equal(status.phase, 'failed');
assert.equal(
  await readFile(
    join(
      process.env.LOCALAPPDATA,
      'BraveSoftware',
      'Brave-Browser',
      'User Data',
      'Default',
      '.dustaway-test-only',
    ),
    'utf8',
  ),
  'DustAwayBrave20261001',
);
assert.equal(report.url, 'chrome://bookmarks/');
const url = new URL(status.browserWebSocket);
assert.equal(url.hostname, '127.0.0.1');
assert.equal(url.port, '64445');
const version = await (
  await fetch('http://127.0.0.1:64445/json/version', { signal: AbortSignal.timeout(3000) })
).json();
assert.equal(version.webSocketDebuggerUrl, status.browserWebSocket);
const ws = new WebSocket(status.browserWebSocket);
await new Promise((ok, fail) => {
  ws.addEventListener('open', ok, { once: true });
  ws.addEventListener('error', fail, { once: true });
});
await new Promise((ok, fail) => {
  const timer = setTimeout(
    () => fail(Error('Close observation timeout; inspect original handle')),
    10000,
  );
  ws.addEventListener(
    'close',
    () => {
      clearTimeout(timer);
      ok();
    },
    { once: true },
  );
  ws.addEventListener('message', ({ data }) => {
    const m = JSON.parse(data);
    if (m.id === 1 && m.error) {
      clearTimeout(timer);
      fail(Error(JSON.stringify(m.error)));
    }
  });
  ws.send(JSON.stringify({ id: 1, method: 'Browser.close' }));
});
console.log('Graceful close requested; original process handle must confirm exit.');
