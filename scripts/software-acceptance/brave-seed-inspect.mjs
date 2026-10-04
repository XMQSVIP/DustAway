// Inspect only the retained first seed browser; no new browser or data generation.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
const root = resolve(process.cwd());
assert.equal(process.env.COMPUTERNAME, 'DESKTOP-24NJTT6');
assert.equal(root, join(process.env.LOCALAPPDATA, 'DustAwayAcceptance', '20260930-software'));
assert.equal(
  await readFile(join(root, '.dustaway-isolated-acceptance'), 'utf8'),
  'DustAway.Acceptance.20260930.7c7e25e8',
);
const status = JSON.parse(await readFile(join(root, 'brave-evidence-seed-status.json'), 'utf8'));
assert.equal(status.phase, 'failed');
assert.equal(status.pid, 19160);
assert.equal(status.port, '49563');
const endpoint = new URL(status.browserWebSocket);
assert.equal(endpoint.hostname, '127.0.0.1');
assert.equal(endpoint.port, status.port);
const version = await (
  await fetch(`http://127.0.0.1:${status.port}/json/version`, { signal: AbortSignal.timeout(3000) })
).json();
assert.equal(version.webSocketDebuggerUrl, status.browserWebSocket);
const pages = await (
  await fetch(`http://127.0.0.1:${status.port}/json/list`, { signal: AbortSignal.timeout(3000) })
).json();
const page = pages.find((p) => p.id === '8DE41511F4A9BA9EDEDFC2DE46D65E92');
assert(page && page.type === 'page');
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((ok, fail) => {
  ws.addEventListener('open', ok, { once: true });
  ws.addEventListener('error', fail, { once: true });
});
try {
  const result = await new Promise((ok, fail) => {
    const timer = setTimeout(() => fail(Error('Read timeout')), 5000);
    ws.addEventListener('message', ({ data }) => {
      const m = JSON.parse(data);
      if (m.id !== 1) return;
      clearTimeout(timer);
      m.error ? fail(Error(JSON.stringify(m.error))) : ok(m.result);
    });
    ws.send(
      JSON.stringify({
        id: 1,
        method: 'Runtime.evaluate',
        params: {
          expression:
            "({url:location.href,ready:document.readyState,profile:document.querySelector('#profile_path')?.textContent?.trim()})",
          returnByValue: true,
        },
      }),
    );
  });
  assert(!result.exceptionDetails);
  const report = { ...result.result.value, pid: status.pid, readOnly: true, restarted: false };
  await writeFile(join(root, 'brave-seed-live-inspection.json'), JSON.stringify(report, null, 2), {
    flag: 'wx',
  });
  console.log(JSON.stringify(report));
} finally {
  ws.close();
}
