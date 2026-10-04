// Read-only regression for the native host / WebView offset during automated resize.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { createDesktopCommand, readNativeWindow, assertNativeViewport } from './desktop-driver.mjs';

const executable = resolve(process.env.DUSTAWAY_UI_APP ?? 'target/debug/dustaway.exe');
const output = resolve('artifacts/window-offset-regression');
await mkdir(output, { recursive: true });
const driver = spawn(
  'tauri-driver',
  ['--native-driver', resolve('artifacts/webdriver/msedgedriver.exe')],
  {
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
  },
);
let log = '',
  session;
driver.stdout.on('data', (data) => {
  log += data;
});
driver.stderr.on('data', (data) => {
  log += data;
});
const command = createDesktopCommand();
const js = (script, ...args) =>
  command('POST', `/session/${session}/execute/sync`, { script, args });
const ipc = (name, args = {}) =>
  js('return window.__TAURI_INTERNALS__.invoke(arguments[0], arguments[1]);', name, args);
async function until(fn) {
  const deadline = Date.now() + 30000;
  let last;
  while (Date.now() < deadline) {
    try {
      const result = await fn();
      if (result) return result;
    } catch (error) {
      last = error;
    }
    await new Promise((done) => setTimeout(done, 150));
  }
  throw last ?? Error('Timed out waiting for the desktop test');
}
try {
  await until(() => command('GET', '/status'));
  const created = await command('POST', '/session', {
    capabilities: { alwaysMatch: { 'tauri:options': { application: executable } } },
  });
  session = created.sessionId;
  await until(() => js('return !!document.querySelector(".app-shell");'));
  const before = await ipc('get_runtime_snapshot', { scope: 'user' });
  const initial = await readNativeWindow(executable);
  assertNativeViewport(initial);
  const samples = [];
  for (const theme of ['light', 'dark']) {
    await js('document.documentElement.dataset.theme=arguments[0];', theme);
    // Repeat the large/small transition: the previous bug accumulated offset on every call.
    for (const [width, height] of [
      [760, 580],
      [1100, 780],
      [760, 580],
      [1100, 780],
    ]) {
      const viewport = await command('POST', `/session/${session}/window/rect`, { width, height });
      const native = await readNativeWindow(executable);
      assertNativeViewport(native, { width, height });
      assert.equal(native.client.x, initial.client.x);
      assert.equal(native.client.y, initial.client.y);
      const root = await js(
        'return document.querySelector(".app-shell").getBoundingClientRect().toJSON();',
      );
      assert.equal(root.x, 0);
      assert.equal(root.y, 0);
      assert.equal(root.width, width);
      assert.equal(root.height, height);
      samples.push({ theme, viewport, native, root });
      await writeFile(
        `${output}/${samples.length}-${theme}-${width}.png`,
        Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
      );
    }
  }
  const after = await ipc('get_runtime_snapshot', { scope: 'user' });
  assert.equal(after.control.revision, before.control.revision);
  assert.equal(after.status?.id, before.status?.id);
  assert.equal(after.report?.id, before.report?.id);
  await writeFile(
    `${output}/result.json`,
    JSON.stringify(
      {
        passed: true,
        capabilities: created.capabilities,
        initial,
        samples,
        backendRevisionUnchanged: true,
      },
      null,
      2,
    ),
  );
  console.log(
    `PASS: ${samples.length} native resizes; host position preserved; WebView fills client; no backend changes`,
  );
} finally {
  if (session) await command('DELETE', `/session/${session}`).catch(() => {});
  driver.kill();
  await writeFile(`${output}/driver.log`, log);
}
