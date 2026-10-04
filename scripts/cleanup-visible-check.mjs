// Read-only production scan: never prepares or executes a cleanup plan.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { createDesktopCommand } from './desktop-driver.mjs';

const executable = resolve(process.env.DUSTAWAY_TEST_EXECUTABLE);
const output = resolve(process.env.DUSTAWAY_TEST_OUTPUT);
await mkdir(output, { recursive: true });
const binarySha256 = createHash('sha256')
  .update(await readFile(executable))
  .digest('hex');
const command = createDesktopCommand();
const driver = spawn(
  'tauri-driver',
  ['--native-driver', resolve('artifacts/webdriver/msedgedriver.exe')],
  {
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
  },
);
let session,
  log = '';
driver.stdout.on('data', (s) => (log += s));
driver.stderr.on('data', (s) => (log += s));
async function until(fn, timeout = 180000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    try {
      const result = await fn();
      if (result) return result;
    } catch (error) {
      last = error;
    }
    await new Promise((done) => setTimeout(done, 200));
  }
  throw last ?? new Error('Desktop condition timed out');
}
const js = (script, ...args) =>
  command('POST', `/session/${session}/execute/sync`, { script, args });
const ipc = (name, args = {}) =>
  js('return window.__TAURI_INTERNALS__.invoke(arguments[0],arguments[1]);', name, args);
const capture = async (name) =>
  writeFile(
    join(output, `${name}.png`),
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
const layouts = [];
async function checkLayouts(stage) {
  for (const theme of ['light', 'dark']) {
    for (const [width, height] of [
      [760, 580],
      [1100, 780],
    ]) {
      await js(
        'document.documentElement.dataset.theme=arguments[0];document.querySelector("main").scrollTop=0;',
        theme,
      );
      await command('POST', `/session/${session}/window/rect`, { width, height });
      const state = await js(`
        const main=document.querySelector('main');main.scrollTop=0;
        const visible=e=>!!e&&e.getClientRects().length>0;
        const buttons=[...document.querySelectorAll('button')].filter(visible);
        const action=buttons.find(e=>/^(开始扫描|清理已选)/.test(e.textContent.trim()));
        const rect=action?.getBoundingClientRect();
        return {catalog:!!document.querySelector('.scan-scope'),
          catalogAction:buttons.some(e=>/查看(全部检查|扫描)范围/.test(e.textContent)),
          emptyList:document.body.textContent.includes('未发现候选的检查项'),
          overflow:document.documentElement.scrollWidth>innerWidth,
          action:action?.textContent.trim(),actionVisible:!!rect&&rect.top>=0&&rect.bottom<=innerHeight,
          diskPicker:!!document.querySelector('.scan-volumes')};`);
      assert(
        !state.catalog && !state.catalogAction && !state.emptyList && !state.overflow,
        JSON.stringify(state),
      );
      assert(state.diskPicker && state.actionVisible, JSON.stringify(state));
      layouts.push({ stage, theme, width, height, ...state });
      await capture(`${stage}-${theme}-${width}`);
    }
  }
}
try {
  await until(async () => (await fetch('http://127.0.0.1:4444/status')).ok);
  session = (
    await command('POST', '/session', {
      capabilities: { alwaysMatch: { 'tauri:options': { application: executable } } },
    })
  ).sessionId;
  await until(() =>
    js(
      'return [...document.querySelectorAll("button")].some(b=>b.textContent.trim()==="开始扫描"&&!b.disabled);',
    ),
  );
  assert.equal(
    await js('return document.querySelector(".app-shell").dataset.runtime;'),
    'standard',
  );
  const historyBefore = await ipc('get_history');
  const installedSoftware = await ipc('list_installed_software');
  await checkLayouts('home');
  const scan = await command('POST', `/session/${session}/element`, {
    using: 'xpath',
    value: '//button[normalize-space(.)="开始扫描"]',
  });
  await command(
    'POST',
    `/session/${session}/element/${scan['element-6066-11e4-a52e-4f735466cecf']}/click`,
    {},
  );
  const snapshot = await until(async () => {
    const state = await ipc('get_runtime_snapshot', { scope: 'user' });
    return state.status?.state === 'ready' ? state : false;
  });
  await until(() => js('return !!document.querySelector(".desk-results");'));
  await checkLayouts('results');
  await js(`for(const d of document.querySelectorAll('.result-section:not([open])')) d.querySelector('summary').click();
    for(const b of document.querySelectorAll('.purpose-toggle[aria-expanded="false"]')) b.click();`);
  const summary = await ipc('get_scan_summary', { scope: 'user' });
  const visibleRules = await js(
    'return [...document.querySelectorAll(".purpose-group .rule-row")].map(e=>e.dataset.rule);',
  );
  const foundRules = summary.rules
    .filter((r) => r.files > 0)
    .map((r) => r.rule_id)
    .sort();
  assert.deepEqual([...new Set(visibleRules)].sort(), foundRules);
  assert(!visibleRules.some((id) => summary.rules.some((r) => r.rule_id === id && r.files === 0)));
  // This spare PC has only leftover Brave/Firefox profiles, and no Vivaldi install.
  if (process.env.COMPUTERNAME === 'DESKTOP-24NJTT6') {
    assert(
      !installedSoftware.items.some((item) =>
        /^(Mozilla Firefox|Brave$|Vivaldi$)/i.test(item.name),
      ),
    );
    assert(summary.rules.some((rule) => rule.rule_id === 'firefox.http' && rule.files > 0));
    assert(
      visibleRules.includes('firefox.http'),
      'Uninstalled Firefox cache residues must remain visible',
    );
  }
  await capture('results-expanded');
  assert.deepEqual(
    await ipc('get_history'),
    historyBefore,
    'Read-only acceptance must not create cleanup history',
  );
  await writeFile(
    join(output, 'result.json'),
    JSON.stringify(
      {
        passed: true,
        binarySha256,
        readOnly: true,
        layouts,
        visibleRules,
        summary,
        snapshot,
        installedSoftware,
      },
      null,
      2,
    ),
  );
  console.log(
    JSON.stringify({ passed: true, binarySha256, layouts: layouts.length, visibleRules }),
  );
} catch (error) {
  if (session) await capture('failure').catch(() => {});
  throw error;
} finally {
  if (session) await command('DELETE', `/session/${session}`).catch(() => {});
  driver.kill();
  await writeFile(join(output, 'driver.log'), log);
}
