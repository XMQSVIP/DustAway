// Runs the official browser CLI against a loopback-only test page, using a new named profile.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';

const root = resolve(process.cwd());
assert.equal(process.env.COMPUTERNAME, 'DESKTOP-24NJTT6');
assert.equal(root, join(process.env.LOCALAPPDATA, 'DustAwayAcceptance', '20260930-software'));
assert.equal(
  await readFile(join(root, '.dustaway-isolated-acceptance'), 'utf8'),
  'DustAway.Acceptance.20260930.7c7e25e8',
);
const mode = process.argv[2];
assert(['seed', 'retained', 'reinstalled'].includes(mode));
const outputPrefix = process.argv[3] ?? 'firefox';
assert(/^[a-z0-9-]+$/.test(outputPrefix), 'Output prefix must be a bounded file name');
const marker = 'DustAwayFirefox20260930';
const events = [];
const server = createServer((request, response) => {
  const url = new URL(request.url, 'http://127.0.0.1:19837');
  if (url.pathname === '/asset.css') {
    events.push({ type: 'asset' });
    response.writeHead(200, {
      'Content-Type': 'text/css',
      'Cache-Control': 'public, max-age=86400',
    });
    response.end(
      'body{background:#eaf0eb;color:#1b3832;font:24px sans-serif;padding:70px}h1{color:#244fff}',
    );
  } else if (url.pathname === '/signal') {
    events.push({
      type: 'signal',
      stored: url.searchParams.get('stored') === marker,
      cookie: (request.headers.cookie ?? '').includes(`dustaway=${marker}`),
    });
    response.writeHead(200);
    response.end('ok');
  } else {
    const headers = { 'Content-Type': 'text/html; charset=utf-8', 'Cache-Control': 'no-store' };
    if (mode === 'seed')
      headers['Set-Cookie'] = `dustaway=${marker}; Max-Age=86400; SameSite=Strict; Path=/`;
    response.writeHead(200, headers);
    response.end(
      `<!doctype html><meta charset="utf-8"><title>DustAway Firefox acceptance</title><link rel="stylesheet" href="/asset.css"><h1>DustAway Firefox acceptance</h1><p id="result">Checking test-only local data…</p><script>${mode === 'seed' ? `localStorage.setItem('dustaway', '${marker}');` : ''}const stored=localStorage.getItem('dustaway');document.querySelector('#result').textContent='Local storage: '+(stored==='${marker}'?'preserved':'missing')+' | Cookie: '+(document.cookie.includes('dustaway=${marker}')?'preserved':'missing');fetch('/signal?stored='+encodeURIComponent(stored));</script>`,
    );
  }
});
await new Promise((ok, fail) => {
  server.once('error', fail);
  server.listen(19837, '127.0.0.1', ok);
});
try {
  const firefox = spawn(
    join(root, 'firefox-real', 'application', 'firefox.exe'),
    [
      '-wait-for-browser',
      '-no-remote',
      '-P',
      'DustAwayAcceptance20260930',
      '--screenshot',
      join(root, `${outputPrefix}-${mode}.png`),
      '--window-size',
      '1000,700',
      'http://127.0.0.1:19837/',
    ],
    { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] },
  );
  let output = '';
  firefox.stdout.on('data', (chunk) => {
    output += chunk;
  });
  firefox.stderr.on('data', (chunk) => {
    output += chunk;
  });
  const code = await new Promise((ok, fail) => {
    firefox.once('error', fail);
    firefox.once('exit', ok);
  });
  await writeFile(join(root, `${outputPrefix}-${mode}-browser.log`), output);
  assert.equal(code, 0, 'Browser CLI failed');
  assert(
    events.some((event) => event.type === 'signal' && event.stored && event.cookie),
    'Test-only local data was not available',
  );
  assert((await readFile(join(root, `${outputPrefix}-${mode}.png`))).length > 1000);
  await writeFile(
    join(root, `${outputPrefix}-${mode}-verification.json`),
    JSON.stringify(
      {
        passed: true,
        mode,
        loopbackOnly: true,
        profile: 'DustAwayAcceptance20260930',
        localStorageAndCookieAvailable: true,
        events,
      },
      null,
      2,
    ),
  );
} finally {
  await new Promise((done) => server.close(done));
}
