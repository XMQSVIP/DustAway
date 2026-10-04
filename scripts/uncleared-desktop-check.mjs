// Isolated native UI regression. Scans only a marked debug fixture; never deletes.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, writeFile, utimes } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { createDesktopCommand } from './desktop-driver.mjs';

const output = resolve('artifacts/uncleared-desktop-check');
await mkdir(output, { recursive: true });
const root = await mkdtemp(join(resolve('artifacts'), 'dustaway-e2e-'));
await writeFile(join(root, '.dustaway-test-only'), 'fixture-only');
for (const [directory, name] of [
  ['cache', 'old.tmp'],
  ['applications/weixin.logs', 'old.xlog'],
]) {
  await mkdir(join(root, directory), { recursive: true });
  const file = join(root, directory, name);
  await writeFile(file, 'disposable read-only UI sample');
  const old = new Date(Date.now() - 40 * 86400_000);
  await utimes(file, old, old);
}
const driver = spawn(
  'tauri-driver',
  ['--native-driver', resolve('artifacts/webdriver/msedgedriver.exe')],
  {
    env: {
      ...process.env,
      TEMP: resolve('artifacts'),
      TMP: resolve('artifacts'),
      DUSTAWAY_E2E_ROOT: root,
    },
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
  },
);
let session,
  log = '';
driver.stdout.on('data', (b) => {
  log += b;
});
driver.stderr.on('data', (b) => {
  log += b;
});
const command = createDesktopCommand();
const js = (script, ...args) =>
  command('POST', `/session/${session}/execute/sync`, { script, args });
const ipc = (name, args = {}) =>
  js('return window.__TAURI_INTERNALS__.invoke(arguments[0],arguments[1]);', name, args);
async function until(fn) {
  const end = Date.now() + 45000;
  let last;
  while (Date.now() < end) {
    try {
      const result = await fn();
      if (result) return result;
    } catch (error) {
      last = error;
    }
    await new Promise((done) => setTimeout(done, 150));
  }
  throw last ?? Error('Native UI timeout');
}
try {
  await until(() => command('GET', '/status'));
  session = (
    await command('POST', '/session', {
      capabilities: {
        alwaysMatch: { 'tauri:options': { application: resolve('target/debug/dustaway.exe') } },
      },
    })
  ).sessionId;
  await until(() =>
    js('return document.querySelector(".app-shell")?.dataset.runtime === "fixture";'),
  );
  assert((await ipc('bootstrap')).version.endsWith('+e2e-fixture'));
  const beforeHistory = await ipc('get_history');
  const before = await ipc('get_runtime_snapshot', { scope: 'user' });
  await ipc('start_scan', {
    ruleIds: ['windows.user_temp', 'weixin.logs'],
    request: {
      runtime_id: before.control.runtime_id,
      expected_revision: before.control.revision,
      request_id: crypto.randomUUID(),
    },
  });
  await until(
    async () => (await ipc('get_runtime_snapshot', { scope: 'user' })).status?.state === 'ready',
  );
  await command('POST', `/session/${session}/refresh`, {});
  await until(() => js('return !!document.querySelector(".retained-results summary");'));
  assert.equal(
    await js('return document.querySelector(".retained-results summary").textContent;'),
    '本次未清理 · 1 类',
  );
  const choices = () =>
    js(
      'return [...document.querySelectorAll("input[type=checkbox]:checked")].map(e=>e.getAttribute("aria-label"));',
    );
  const selected = await choices();
  // Enter opens the native details element without changing the selected files.
  await js('document.querySelector(".retained-results summary").focus();');
  await command('POST', `/session/${session}/actions`, {
    actions: [
      {
        type: 'key',
        id: 'keyboard',
        actions: [
          { type: 'keyDown', value: '\uE007' },
          { type: 'keyUp', value: '\uE007' },
        ],
      },
    ],
  });
  await until(() => js('return document.querySelector(".retained-results").open;'));
  await js('document.querySelector(".retained-results .purpose-toggle").click();');
  const samples = [];
  for (const [width, height] of [
    [1100, 780],
    [760, 580],
  ]) {
    await command('POST', `/session/${session}/window/rect`, { width, height });
    const layout = await js(`
      const section=document.querySelector('.retained-results');
      const row=section.querySelector('.rule-row');
      row.scrollIntoView({block:'center',behavior:'instant'});
      const next=[...row.querySelectorAll('p')].find(p=>p.textContent.startsWith('后续处理：'));
      const rect=next.getBoundingClientRect();
      return {width:innerWidth,height:innerHeight,overflow:document.documentElement.scrollWidth>innerWidth,
        text:section.textContent, nextVisible:rect.top>=0&&rect.bottom<=innerHeight,
        selectable:!!row.querySelector('input[type=checkbox]')};`);
    assert(!layout.overflow && layout.nextVisible && !layout.selectable, JSON.stringify(layout));
    assert(layout.text.includes('不是文件数量') && layout.text.includes('重复扫描不会解除此限制'));
    assert.deepEqual(await choices(), selected);
    samples.push(layout);
    await writeFile(
      join(output, `uncleared-${width}.png`),
      Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
    );
  }
  assert.deepEqual(await ipc('get_history'), beforeHistory);
  await writeFile(
    join(output, 'result.json'),
    JSON.stringify(
      { passed: true, fixture: root, deletionAttempted: false, selectionUnchanged: true, samples },
      null,
      2,
    ),
  );
  console.log(
    'PASS: category counts, next steps, keyboard expansion, unchanged selection/history, 1100/760 layouts',
  );
} finally {
  if (session) await command('DELETE', `/session/${session}`).catch(() => {});
  driver.kill();
  await writeFile(join(output, 'driver.log'), log);
}
