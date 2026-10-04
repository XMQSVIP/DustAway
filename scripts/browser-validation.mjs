// Real browser acceptance uses only a newly created, marked temporary fixture.
import { mkdtemp, mkdir, writeFile, readFile, readdir, stat } from 'node:fs/promises';
import { join, resolve, relative } from 'node:path';
import { tmpdir } from 'node:os';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
const exec = promisify(execFile);
const root = await mkdtemp(join(tmpdir(), 'dustaway-browser-fixture-'));
await writeFile(join(root, '.dustaway-test-only'), 'browser-fixture-only');
const output = resolve('artifacts/browser-validation-0.9');
const browserAssets = resolve('artifacts/browser-validation-0.6');
await mkdir(output, { recursive: true });
const delay = (ms) => new Promise((r) => setTimeout(r, ms));
const ownedBrowsers = new Set();
async function until(fn, timeout = 60000) {
  const end = Date.now() + timeout;
  let error;
  while (Date.now() < end) {
    try {
      const value = await fn();
      if (value) return value;
    } catch (e) {
      error = e;
    }
    await delay(100);
  }
  throw error ?? new Error('browser fixture timed out');
}
const requests = {};
const script =
  Array.from({ length: 4000 }, (_, i) => `function fixture${i}(x){return x+${i};}\n`).join('') +
  'window.fixtureLoaded=true;';
const server = createServer((req, res) => {
  requests[req.url] = (requests[req.url] ?? 0) + 1;
  if (req.url === '/app.js') {
    res.writeHead(200, {
      'Content-Type': 'application/javascript',
      'Cache-Control': 'public, max-age=86400',
    });
    return res.end(script);
  }
  res.writeHead(200, { 'Content-Type': 'text/html', 'Cache-Control': 'no-store' });
  res.end(
    '<!doctype html><title>DustAway isolated cache fixture</title><script src="/app.js"></script><canvas id="c"></canvas><script>const gl=document.querySelector("canvas").getContext("webgl");if(gl){let s=gl.createShader(gl.VERTEX_SHADER);gl.shaderSource(s,"attribute vec4 p;void main(){gl_Position=p;}");gl.compileShader(s);gl.clearColor(0.2,0.4,0.8,1);gl.clear(gl.COLOR_BUFFER_BIT);}</script>',
  );
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const url = `http://127.0.0.1:${server.address().port}/fixture`;
async function connect(wsUrl) {
  const ws = new WebSocket(wsUrl);
  let id = 0;
  const pending = new Map();
  ws.addEventListener('message', (e) => {
    const m = JSON.parse(e.data);
    if (!m.id) return;
    const p = pending.get(m.id);
    if (p) {
      pending.delete(m.id);
      clearTimeout(p.timer);
      m.error ? p.reject(new Error(JSON.stringify(m.error))) : p.resolve(m.result);
    }
  });
  await new Promise((r, j) => {
    ws.addEventListener('open', r, { once: true });
    ws.addEventListener('error', j, { once: true });
  });
  return {
    close: () => ws.close(),
    call: (method, params = {}) =>
      new Promise((resolve, reject) => {
        const key = ++id;
        const timer = setTimeout(() => {
          pending.delete(key);
          reject(new Error(`CDP timeout: ${method}`));
        }, 15000);
        pending.set(key, { resolve, reject, timer });
        ws.send(JSON.stringify({ id: key, method, params }));
      }),
  };
}
async function launch(exe, userData) {
  const child = spawn(
    exe,
    [
      '--headless=new',
      '--no-first-run',
      '--no-default-browser-check',
      '--disable-background-networking',
      '--disable-component-update',
      '--disable-sync',
      '--enable-unsafe-swiftshader',
      '--remote-debugging-port=0',
      `--user-data-dir=${userData}`,
      '--profile-directory=Default',
      'about:blank',
    ],
    { windowsHide: true, stdio: ['ignore', 'ignore', 'pipe'] },
  );
  let log = '';
  ownedBrowsers.add(child);
  child.once('exit', () => ownedBrowsers.delete(child));
  child.stderr.on('data', (x) => (log += x));
  const port = await until(async () => {
    const p = (await readFile(join(userData, 'DevToolsActivePort'), 'utf8')).split('\n')[0];
    const response = await fetch(`http://127.0.0.1:${p}/json/version`);
    return response.ok ? p : null;
  });
  const version = await (await fetch(`http://127.0.0.1:${port}/json/version`)).json();
  const browser = await connect(version.webSocketDebuggerUrl);
  const pageTarget = await until(async () => {
    const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    return targets.find((target) => target.type === 'page' && target.webSocketDebuggerUrl);
  });
  const page = await connect(pageTarget.webSocketDebuggerUrl);
  const evaluate = async (expression) =>
    (await page.call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true }))
      .result.value;
  const navigate = async () => {
    await page.call('Page.navigate', { url });
    await until(() => evaluate('Boolean(window.fixtureLoaded)'));
  };
  return {
    version,
    evaluate,
    navigate,
    stop: async () => {
      page.close();
      await browser.call('Browser.close').catch(() => {});
      browser.close();
      await until(() => child.exitCode !== null || child.signalCode !== null);
      await delay(500);
      return log;
    },
  };
}
const cachePaths = ['Cache/Cache_Data', 'Code Cache', 'GPUCache'];
async function files(dir) {
  const result = [];
  for (const entry of await readdir(dir, { withFileTypes: true }).catch(() => [])) {
    const p = join(dir, entry.name);
    if (entry.isDirectory()) result.push(...(await files(p)));
    else if (entry.isFile()) result.push(p);
  }
  return result;
}
function isCache(path) {
  const p = path.replaceAll('\\', '/');
  return cachePaths.some((c) => p.startsWith(`Default/${c}/`));
}
async function inventory(dir, cache) {
  const result = {};
  for (const p of await files(dir)) {
    const rel = relative(dir, p);
    if (isCache(rel) !== cache) continue;
    const info = await stat(p);
    result[rel] = {
      size: info.size,
      mtime: info.mtimeMs,
      sha256: createHash('sha256')
        .update(await readFile(p))
        .digest('hex'),
    };
  }
  return result;
}
const build = await exec(
  'cargo',
  ['test', '-p', 'dustaway-windows', '--lib', '--no-run', '--locked', '--message-format=json'],
  { windowsHide: true, maxBuffer: 8 * 1024 * 1024 },
);
const binary = build.stdout
  .split('\n')
  .filter(Boolean)
  .map((s) => {
    try {
      return JSON.parse(s);
    } catch {
      return {};
    }
  })
  .find((m) => m.reason === 'compiler-artifact' && m.profile.test && m.executable)?.executable;
assert(binary, 'test-only acceptance binary missing');
async function engine(app, action) {
  await exec(
    binary,
    [
      'platform::browser_acceptance::real_browser_acceptance',
      '--ignored',
      '--exact',
      '--nocapture',
    ],
    {
      windowsHide: true,
      env: {
        ...process.env,
        DUSTAWAY_BROWSER_FIXTURE: root,
        DUSTAWAY_BROWSER_APP: app,
        DUSTAWAY_BROWSER_ACTION: action,
      },
      maxBuffer: 4 * 1024 * 1024,
    },
  );
  return JSON.parse(await readFile(join(root, `${app}-${action}.json`), 'utf8'));
}
const evidence = {
  version: '0.9.0',
  fixture: root,
  startedAt: new Date().toISOString(),
  passed: false,
  browsers: [],
};
let running;
try {
  for (const product of [
    {
      app: 'brave',
      exe: join(browserAssets, 'brave/brave.exe'),
      data: 'BraveSoftware/Brave-Browser/User Data',
    },
    {
      app: 'vivaldi',
      exe: join(browserAssets, 'vivaldi/Vivaldi-bin/vivaldi.exe'),
      data: 'Vivaldi/User Data',
    },
  ]) {
    const userData = join(root, 'local', product.data);
    await mkdir(userData, { recursive: true });
    running = await launch(product.exe, userData);
    await running.navigate();
    await running.evaluate(
      'document.cookie="dustaway_login=fixture-user; Max-Age=86400; SameSite=Lax"; localStorage.setItem("dustaway_note","protected test note"); true',
    );
    for (let i = 0; i < 3; i++) {
      await running.navigate();
      await delay(1200);
    }
    const blocked = await engine(product.app, 'blocked');
    const version = running.version;
    await running.stop();
    running = null;
    // A real profile bookmark is added offline, then checked after browser restart.
    const bookmarkPath = join(userData, 'Default', 'Bookmarks');
    let bookmarks;
    try {
      bookmarks = JSON.parse(await readFile(bookmarkPath, 'utf8'));
    } catch {
      bookmarks = {
        version: 1,
        roots: {
          bookmark_bar: { children: [], name: 'Bookmarks bar', type: 'folder', id: '1' },
          other: { children: [], name: 'Other bookmarks', type: 'folder', id: '2' },
          synced: { children: [], name: 'Mobile bookmarks', type: 'folder', id: '3' },
        },
      };
    }
    bookmarks.roots.bookmark_bar.children.push({
      id: '987654',
      type: 'url',
      name: 'DustAway protected bookmark',
      url,
      guid: crypto.randomUUID(),
      date_added: '13400000000000000',
    });
    delete bookmarks.checksum;
    await writeFile(bookmarkPath, JSON.stringify(bookmarks));
    const before = await inventory(userData, false);
    const cacheBefore = await inventory(userData, true);
    for (const path of cachePaths)
      assert(
        Object.keys(cacheBefore).some((p) =>
          p.replaceAll('\\', '/').startsWith(`Default/${path}/`),
        ),
        `real ${product.app} ${path} was not generated`,
      );
    const scan = await engine(product.app, 'scan');
    const clean = await engine(product.app, 'clean');
    assert.deepEqual(
      await inventory(userData, false),
      before,
      'protected file contents or timestamps changed during cleanup',
    );
    assert.equal(Object.keys(await inventory(userData, true)).length, 0);
    running = await launch(product.exe, userData);
    await running.navigate();
    const login = await running.evaluate('document.cookie');
    const note = await running.evaluate('localStorage.getItem("dustaway_note")');
    assert(login.includes('dustaway_login=fixture-user'), 'test login was lost');
    assert.equal(note, 'protected test note');
    await delay(2000);
    await running.stop();
    running = null;
    assert(
      (await readFile(bookmarkPath, 'utf8')).includes('DustAway protected bookmark'),
      'bookmark lost on restart',
    );
    const rebuilt = await inventory(userData, true);
    for (const path of cachePaths)
      assert(
        Object.keys(rebuilt).some((p) => p.replaceAll('\\', '/').startsWith(`Default/${path}/`)),
        `real ${product.app} ${path} not rebuilt`,
      );
    evidence.browsers.push({
      app: product.app,
      version,
      blocked,
      scan,
      clean,
      protectedFiles: Object.keys(before).length,
      cacheFiles: Object.keys(cacheBefore).length,
      rebuiltFiles: Object.keys(rebuilt).length,
      loginPreserved: true,
      localStoragePreserved: true,
      bookmarkPreserved: true,
      protectedContentsAndTimesPreserved: true,
    });
    await writeFile(join(output, 'evidence.json'), JSON.stringify(evidence, null, 2));
    console.log(`${product.app}: cleanup, protection and cache rebuild passed`);
  }
  evidence.passed = true;
} finally {
  if (running) await running.stop().catch(() => {});
  for (const child of ownedBrowsers) child.kill();
  server.close();
  evidence.finishedAt = new Date().toISOString();
  await writeFile(join(output, 'evidence.json'), JSON.stringify(evidence, null, 2));
}
