// Actual installed Edge, isolated localhost content/profile; no vendor binaries modified.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdir, readFile, writeFile, readdir, realpath, lstat, unlink } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { once } from 'node:events';

const root = resolve('artifacts/readonly-unlock-20261002/edge-gpu-recovery-v1');
await mkdir(root); // Never replay or overwrite an earlier test.
const profile = join(root, 'profile');
const payload = Buffer.alloc(2 * 1024 * 1024, 90);
let requests = 0;
const server = createServer((req, res) => {
  if (req.url !== '/cache-probe') {
    res.writeHead(204);
    res.end();
    return;
  }
  requests++;
  res.writeHead(200, {
    'Cache-Control': 'public,max-age=31536000',
    'Content-Type': 'text/plain',
    'Content-Length': payload.length,
  });
  res.end(payload);
});
server.listen(49237, '127.0.0.1');
await once(server, 'listening');
const results = [];
async function run(phase) {
  const before = requests;
  const browser = spawn(
    'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
    [
      '--headless=new',
      '--no-first-run',
      '--no-default-browser-check',
      '--disable-background-networking',
      '--remote-debugging-address=127.0.0.1',
      '--remote-debugging-port=49337',
      `--user-data-dir=${profile}`,
      'about:blank',
    ],
    { windowsHide: true, stdio: 'ignore' },
  );
  let ws;
  try {
    let meta;
    for (let i = 0; i < 150; i++) {
      try {
        meta = await (await fetch('http://127.0.0.1:49337/json/version')).json();
        break;
      } catch {
        await new Promise((r) => setTimeout(r, 200));
      }
    }
    assert(meta?.webSocketDebuggerUrl);
    ws = new WebSocket(meta.webSocketDebuggerUrl);
    await once(ws, 'open');
    let sequence = 0;
    const pending = new Map();
    const events = [];
    ws.addEventListener('message', (e) => {
      const value = JSON.parse(e.data);
      if (value.id) {
        const p = pending.get(value.id);
        pending.delete(value.id);
        value.error ? p.reject(Error(JSON.stringify(value.error))) : p.resolve(value.result);
      } else events.push(value);
    });
    const send = (method, params = {}, sessionId) =>
      new Promise((resolve, reject) => {
        const id = ++sequence;
        pending.set(id, { resolve, reject });
        ws.send(JSON.stringify({ id, method, params, sessionId }));
      });
    const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
    await send('Page.enable', {}, sessionId);
    await send('Network.enable', {}, sessionId);
    const navigation = await send(
      'Page.navigate',
      { url: 'http://127.0.0.1:49237/cache-probe' },
      sessionId,
    );
    for (
      let i = 0;
      i < 150 &&
      !events.some((e) => ['Page.loadEventFired', 'Network.loadingFailed'].includes(e.method));
      i++
    )
      await new Promise((r) => setTimeout(r, 100));
    const graphic = await send(
      'Runtime.evaluate',
      {
        expression: `(()=>{const c=document.createElement('canvas');c.width=c.height=2;document.body.append(c);const g=c.getContext('webgl');if(!g)throw Error('WebGL unavailable');const shader=(kind,source)=>{const s=g.createShader(kind);g.shaderSource(s,source);g.compileShader(s);if(!g.getShaderParameter(s,g.COMPILE_STATUS))throw Error(g.getShaderInfoLog(s));return s};const program=g.createProgram();g.attachShader(program,shader(g.VERTEX_SHADER,'attribute vec2 p;void main(){gl_Position=vec4(p,0.,1.);}'));g.attachShader(program,shader(g.FRAGMENT_SHADER,'precision mediump float;void main(){gl_FragColor=vec4(1.,0.,0.,1.);}'));g.linkProgram(program);if(!g.getProgramParameter(program,g.LINK_STATUS))throw Error('link failed');g.useProgram(program);g.bindBuffer(g.ARRAY_BUFFER,g.createBuffer());g.bufferData(g.ARRAY_BUFFER,new Float32Array([-1,-1,3,-1,-1,3]),g.STATIC_DRAW);const a=g.getAttribLocation(program,'p');g.vertexAttribPointer(a,2,g.FLOAT,false,0,0);g.enableVertexAttribArray(a);g.viewport(0,0,2,2);g.drawArrays(g.TRIANGLES,0,3);g.finish();const pixel=new Uint8Array(4);g.readPixels(0,0,1,1,g.RGBA,g.UNSIGNED_BYTE,pixel);return {pixel:[...pixel],error:g.getError()};})()`,
        returnByValue: true,
      },
      sessionId,
    );
    assert.deepEqual(graphic.result?.value, { pixel: [255, 0, 0, 255], error: 0 });
    const body = await send(
      'Runtime.evaluate',
      { expression: 'document.body.innerText.length', returnByValue: true },
      sessionId,
    );
    results.push({
      phase,
      graphic: graphic.result.value,
      browser: meta.Browser,
      requests: requests - before,
      navigation,
      bodyLength: body.result?.value,
      responses: events
        .filter((e) => e.method === 'Network.responseReceived')
        .map((e) => ({
          status: e.params.response.status,
          fromDiskCache: e.params.response.fromDiskCache,
        })),
      failures: events
        .filter((e) => e.method === 'Network.loadingFailed')
        .map((e) => e.params.errorText),
    });
    await send('Browser.close').catch(() => {});
  } finally {
    ws?.close();
  }
  // Browser.close only terminates our profile's browser. Never kill other browser processes.
  await Promise.race([
    once(browser, 'exit'),
    new Promise((_, reject) => setTimeout(() => reject(Error('Own browser did not exit')), 15000)),
  ]);
}
try {
  await run('seed');
  await run('warm');
  assert.equal(results[1].requests, 0, 'Must establish a real cache hit before deleting a fixture');
  const cache = join(profile, 'Default/GPUCache');
  assert.equal((await realpath(cache)).toLowerCase(), cache.toLowerCase());
  const target = join(cache, 'data_1');
  const info = await lstat(target);
  assert(info.isFile() && !info.isSymbolicLink());
  const data = await readFile(target);
  assert.equal(data.readUInt32LE(0), 0xc104cac3);
  assert.equal(data.readUInt32LE(4), 0x20000);
  assert.equal(data.readUInt32LE(12), 256);
  // Test partial selection: other GPU block files and the index remain present.
  await unlink(target);
  await run('after');
  await run('retry');
} finally {
  await writeFile(join(root, 'result.json'), JSON.stringify({ results, requests }, null, 2), {
    flag: 'wx',
  });
  server.close();
}
