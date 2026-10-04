// Real browser data generation, confined to the previously absent marked Brave root.
// Browser/DevTools APIs create the files; no database or bookmark file is synthesized.
import assert from 'node:assert/strict';
import { readFile, writeFile, mkdir, readdir, lstat, access } from 'node:fs/promises';
import { join, resolve, relative } from 'node:path';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';

const root = resolve(process.cwd());
assert.equal(process.env.COMPUTERNAME, 'DESKTOP-24NJTT6');
assert.equal(root, join(process.env.LOCALAPPDATA, 'DustAwayAcceptance', '20260930-software'));
assert.equal(
  await readFile(join(root, '.dustaway-isolated-acceptance'), 'utf8'),
  'DustAway.Acceptance.20260930.7c7e25e8',
);
const prefix = process.argv[2];
assert(['brave-evidence', 'brave-production'].includes(prefix));
const base = join(process.env.LOCALAPPDATA, 'BraveSoftware');
assert.equal(await readFile(join(base, '.dustaway-test-only'), 'utf8'), 'DustAwayBrave20261001');
assert(!(await lstat(base)).isSymbolicLink());
const json = async (p) => JSON.parse((await readFile(p, 'utf8')).replace(/^\uFEFF/, ''));
const ready = await json(join(root, 'brave-real-ready.json'));
assert.equal(ready.SignedVersion, '154.1.96.60');
assert.equal(ready.OrdinaryUser, true);
const exe = join(base, 'Brave-Browser', 'Application', 'brave.exe');
assert.equal(
  createHash('sha256')
    .update(await readFile(exe))
    .digest('hex')
    .toUpperCase(),
  ready.BrowserSha256,
);
const userData = join(base, 'Brave-Browser', 'User Data');
const profile = join(userData, 'Default');
const exists = (p) =>
  access(p).then(
    () => true,
    () => false,
  );
if (await exists(userData)) {
  assert(!(await lstat(userData)).isSymbolicLink());
  assert.equal(
    await readFile(join(userData, '.dustaway-test-only'), 'utf8'),
    'DustAwayBrave20261001',
  );
} else {
  await mkdir(userData);
  await writeFile(join(userData, '.dustaway-test-only'), 'DustAwayBrave20261001', { flag: 'wx' });
}
if (await exists(profile)) {
  assert(!(await lstat(profile)).isSymbolicLink());
  assert.equal(
    await readFile(join(profile, '.dustaway-test-only'), 'utf8'),
    'DustAwayBrave20261001',
  );
}
await writeFile(join(root, `${prefix}-seed.started`), new Date().toISOString(), { flag: 'wx' });
const save = (name, value) =>
  writeFile(join(root, `${prefix}-${name}.json`), JSON.stringify(value, null, 2));
const delay = (ms) => new Promise((r) => setTimeout(r, ms));
async function until(fn, ms = 60000) {
  const end = Date.now() + ms;
  let error;
  while (Date.now() < end) {
    try {
      const value = await fn();
      if (value) return value;
    } catch (e) {
      error = e;
    }
    await delay(200);
  }
  throw error ?? new Error('Observation timed out; inspect original handle');
}
const marker = 'DustAwayBraveData20261001';
const origin = 'http://127.0.0.1:19839';
const url = `${origin}/${marker}/alpha`;
const script =
  Array.from({ length: 4000 }, (_, i) => `function fixture${i}(x){return x+${i};}`).join('\n') +
  '\nwindow.fixtureLoaded=true;';
const server = createServer((req, res) => {
  if (req.url === `/${marker}/app.js`) {
    res.writeHead(200, {
      'Content-Type': 'application/javascript',
      'Cache-Control': 'public, max-age=86400',
    });
    res.end(script);
  } else if (req.url?.startsWith(`/${marker}/`)) {
    res.writeHead(200, { 'Content-Type': 'text/html', 'Cache-Control': 'no-store' });
    res.end(`<!doctype html><title>${marker}</title><h1>Dedicated browser data</h1><script src="/${marker}/app.js"></script><canvas id="c"></canvas><script>
      const gl=document.querySelector('canvas').getContext('webgl');
      if(gl){const s=gl.createShader(gl.VERTEX_SHADER);gl.shaderSource(s,'attribute vec4 p;void main(){gl_Position=p;}');gl.compileShader(s);gl.clearColor(.2,.4,.8,1);gl.clear(gl.COLOR_BUFFER_BIT);}
    </script>`);
  } else {
    res.writeHead(404);
    res.end();
  }
});
await new Promise((ok, fail) => {
  server.once('error', fail);
  server.listen(19839, '127.0.0.1', ok);
});
async function connect(wsUrl) {
  assert.equal(new URL(wsUrl).hostname, '127.0.0.1');
  const ws = new WebSocket(wsUrl);
  const pending = new Map();
  let id = 0;
  ws.addEventListener('message', ({ data }) => {
    const message = JSON.parse(data);
    const item = pending.get(message.id);
    if (!item) return;
    pending.delete(message.id);
    clearTimeout(item.timer);
    if (message.error) item.reject(new Error(JSON.stringify(message.error)));
    else item.resolve(message.result);
  });
  await new Promise((ok, fail) => {
    ws.addEventListener('open', ok, { once: true });
    ws.addEventListener('error', fail, { once: true });
  });
  return {
    close: () => ws.close(),
    call: (method, params = {}) =>
      new Promise((resolve, reject) => {
        const key = ++id;
        const timer = setTimeout(() => {
          pending.delete(key);
          reject(new Error(`CDP timeout: ${method}`));
        }, 30000);
        pending.set(key, { resolve, reject, timer });
        ws.send(JSON.stringify({ id: key, method, params }));
      }),
  };
}
let child, browser, page, port, version;
let stderr = '';
const observations = [];
const evaluate = async (expression) => {
  const result = await page.call('Runtime.evaluate', {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  assert(!result.exceptionDetails, JSON.stringify(result.exceptionDetails));
  return result.result.value;
};
async function navigate(target) {
  await page.call('Page.navigate', { url: target });
  await until(() =>
    evaluate(`location.href === ${JSON.stringify(target)} && document.readyState === 'complete'`),
  );
}
async function bookmarks(create) {
  await navigate('brave://bookmarks/');
  return evaluate(`(async()=>{
    const {BookmarksApiProxyImpl}=await import('./bookmarks_api_proxy.js');
    const api=BookmarksApiProxyImpl.getInstance();
    const nodes=Object.values(await api.getTree());
    const old=nodes.find(n=>n.title===${JSON.stringify(marker)} && n.url===${JSON.stringify(url)});
    if(old) return {verified:true,created:false};
    if(!${create}) throw Error('Test bookmark missing after reopen');
    const parent=nodes.find(n=>n.parentId && !n.url && Array.isArray(n.children) && !n.unmodifiable);
    if(!parent) throw Error('No editable bookmark folder');
    const item=await api.create(parent.id,null,${JSON.stringify(marker)},${JSON.stringify(url)});
    if(item.title!==${JSON.stringify(marker)} || item.url!==${JSON.stringify(url)}) throw Error('Bookmark creation mismatch');
    return {verified:true,created:true};
  })()`);
}
try {
  for (let cycle = 0; cycle < 2; cycle++) {
    let cycleStderr = '';
    let spawnError;
    child = spawn(
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
    child.on('error', (error) => {
      spawnError = error;
    });
    child.stderr.on('data', (data) => {
      stderr += data;
      cycleStderr += data;
    });
    await save('seed-status', { phase: 'browser_started', cycle, pid: child.pid });
    port = await until(async () => {
      if (spawnError) throw spawnError;
      assert.equal(child.exitCode, null, 'Original browser exited before DevTools became ready');
      assert.equal(child.signalCode, null, 'Original browser was interrupted');
      // Use the endpoint emitted by this exact child, never a stale profile port file.
      const endpoint = cycleStderr.match(
        /DevTools listening on (ws:\/\/127\.0\.0\.1:\d+\/devtools\/browser\/[a-zA-Z0-9-]+)/,
      )?.[1];
      if (!endpoint) return false;
      const candidate = new URL(endpoint).port;
      const response = await fetch(`http://127.0.0.1:${candidate}/json/version`, {
        signal: AbortSignal.timeout(3000),
      });
      version = await response.json();
      assert.equal(
        version.webSocketDebuggerUrl,
        endpoint,
        'Debug endpoint no longer belongs to the original child',
      );
      return candidate;
    });
    browser = await connect(version.webSocketDebuggerUrl);
    const target = await until(async () =>
      (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find(
        (t) => t.type === 'page' && t.webSocketDebuggerUrl,
      ),
    );
    page = await connect(target.webSocketDebuggerUrl);
    await save('seed-status', {
      phase: 'connected',
      cycle,
      pid: child.pid,
      port,
      browserWebSocket: version.webSocketDebuggerUrl,
      pageWebSocket: target.webSocketDebuggerUrl,
    });
    await navigate('brave://version/');
    assert.equal(
      (await evaluate("document.querySelector('#profile_path').textContent")).trim().toLowerCase(),
      profile.toLowerCase(),
    );
    await navigate(url);
    await until(() => evaluate('Boolean(window.fixtureLoaded)'));
    if (cycle === 0) {
      await evaluate(`(async()=>{
        document.cookie='dustaway_login=fixture-only; Max-Age=86400; SameSite=Lax';
        localStorage.setItem('dustaway_note',${JSON.stringify(marker)});
        const db=await new Promise((ok,fail)=>{const r=indexedDB.open('DustAwayResidualAcceptance',1);r.onupgradeneeded=()=>r.result.createObjectStore('notes');r.onerror=()=>fail(r.error);r.onsuccess=()=>ok(r.result)});
        await new Promise((ok,fail)=>{const t=db.transaction('notes','readwrite');t.objectStore('notes').put(${JSON.stringify(marker)},'only-test-note');t.oncomplete=ok;t.onerror=()=>fail(t.error)});db.close();return true;
      })()`);
    }
    const data = await evaluate(`(async()=>{
      const db=await new Promise((ok,fail)=>{const r=indexedDB.open('DustAwayResidualAcceptance',1);r.onerror=()=>fail(r.error);r.onsuccess=()=>ok(r.result)});
      const note=await new Promise((ok,fail)=>{const r=db.transaction('notes').objectStore('notes').get('only-test-note');r.onsuccess=()=>ok(r.result);r.onerror=()=>fail(r.error)});db.close();
      return {cookie:document.cookie.includes('dustaway_login=fixture-only'),local:localStorage.getItem('dustaway_note'),indexed:note};
    })()`);
    assert.deepEqual(data, { cookie: true, local: marker, indexed: marker });
    for (let i = 0; i < 3; i++) {
      await navigate(url + `?cache=${i}`);
      await until(() => evaluate('Boolean(window.fixtureLoaded)'));
      await delay(1200);
    }
    const bookmark = await bookmarks(cycle === 0);
    await navigate(url);
    await browser.call('Target.createTarget', { url: `${origin}/${marker}/beta` });
    await delay(1500);
    observations.push({
      cycle,
      cookieLocalIndexedVerified: true,
      bookmark,
      exactProfileVerified: true,
    });
    page.close();
    page = undefined;
    await browser.call('Browser.close').catch(() => {});
    browser.close();
    browser = undefined;
    await until(() => child.exitCode !== null || child.signalCode !== null);
    await save('seed-status', { phase: 'browser_exited', cycle, exitCode: child.exitCode });
    assert.equal(child.exitCode, 0, 'Browser did not exit successfully');
    assert.equal(child.signalCode, null, 'Browser was terminated by a signal');
    child = undefined;
  }
  if (await exists(join(profile, '.dustaway-test-only'))) {
    assert.equal(
      await readFile(join(profile, '.dustaway-test-only'), 'utf8'),
      'DustAwayBrave20261001',
    );
  } else {
    await writeFile(join(profile, '.dustaway-test-only'), 'DustAwayBrave20261001', { flag: 'wx' });
  }
  const scopes = [];
  for (const scope of [
    'Cache/Cache_Data',
    'Code Cache',
    'GPUCache',
    'Preferences',
    'Secure Preferences',
    'Bookmarks',
    'History',
    'Network/Cookies',
    'Local Storage',
    'IndexedDB',
    'Sessions',
  ]) {
    const files = [];
    const pending = [join(profile, scope)];
    while (pending.length) {
      const path = pending.pop();
      const info = await lstat(path);
      assert(!info.isSymbolicLink());
      if (info.isDirectory())
        for (const name of await readdir(path)) pending.push(join(path, name));
      else if (info.isFile()) {
        assert(info.size < 64 * 1024 * 1024 && files.length < 2000);
        files.push({
          name: relative(join(profile, scope), path),
          bytes: info.size,
          sha256: createHash('sha256')
            .update(await readFile(path))
            .digest('hex'),
        });
      }
    }
    assert(
      files.some((f) => f.bytes > 0),
      `No real browser data for ${scope}`,
    );
    scopes.push({ scope, files });
  }
  await save('seed-verification', {
    passed: true,
    ordinaryUser: true,
    signedVersion: ready.SignedVersion,
    browserVersion: version.Browser,
    sameMarkedDefaultProfile: true,
    twoGracefulExits: true,
    browserGeneratedFiles: true,
    bookmarkViaBrowserService: true,
    loopbackOnlyPages: true,
    observations,
    scopes,
  });
  await save('seed-status', { phase: 'passed' });
} catch (error) {
  await writeFile(join(root, `${prefix}-seed-error.log`), String(error.stack ?? error));
  await save('seed-status', {
    phase: 'failed',
    pid: child?.pid,
    port,
    browserWebSocket: version?.webSocketDebuggerUrl,
    keptRunning: !!child && child.exitCode === null && child.signalCode === null,
  });
  if (child && child.exitCode === null && child.signalCode === null) {
    await until(
      () => exists(join(root, `${prefix}-seed-inspection-complete`)),
      24 * 60 * 60 * 1000,
    );
  }
  throw error;
} finally {
  await writeFile(join(root, `${prefix}-seed-browser.log`), stderr);
  page?.close();
  browser?.close();
  await new Promise((done) => server.close(done));
}
