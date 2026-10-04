import { createDesktopCommand } from './desktop-driver.mjs';
// Native UI check. Only an explicitly marked debug fixture may reach scan/confirmation.
import { mkdtemp, mkdir, writeFile, utimes, access } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { spawn } from 'node:child_process';
import assert from 'node:assert/strict';

const output = resolve('artifacts/sculpture-native');
await mkdir(output, { recursive: true });
const root = await mkdtemp(join(output, 'dustaway-e2e-'));
await mkdir(join(root, 'cache'));
await mkdir(join(root, 'analysis'));
await writeFile(join(root, '.dustaway-test-only'), 'fixture-only');
const file = join(root, 'cache', 'old.tmp');
await writeFile(file, 'Disposable homepage test cache');
const old = new Date(Date.now() - 8 * 86400_000);
await utimes(file, old, old);
const driver = spawn(
  'tauri-driver',
  ['--native-driver', resolve('artifacts/webdriver/msedgedriver.exe')],
  {
    env: { ...process.env, TEMP: output, TMP: output, DUSTAWAY_E2E_ROOT: root },
    windowsHide: true,
    stdio: 'ignore',
  },
);
let session;
const delay = (ms) => new Promise((r) => setTimeout(r, ms));
const request = createDesktopCommand();
async function until(fn) {
  const deadline = Date.now() + 30000;
  let error;
  while (Date.now() < deadline) {
    try {
      const result = await fn();
      if (result) return result;
    } catch (e) {
      error = e;
    }
    await delay(150);
  }
  throw error ?? Error('Condition timed out');
}
const evaluate = (script, ...args) =>
  request('POST', `/session/${session}/execute/sync`, { script, args });
async function click(text) {
  const id = await until(async () => {
    const matches = await request('POST', `/session/${session}/elements`, {
      using: 'xpath',
      value: `//button[normalize-space(.)="${text}" or starts-with(normalize-space(.),"${text} ·")]`,
    });
    for (const match of matches) {
      const id = match['element-6066-11e4-a52e-4f735466cecf'];
      if (
        (await request('GET', `/session/${session}/element/${id}/displayed`)) &&
        (await request('GET', `/session/${session}/element/${id}/enabled`))
      )
        return id;
    }
  });
  await evaluate('arguments[0].scrollIntoView({block:"center"});', {
    'element-6066-11e4-a52e-4f735466cecf': id,
  });
  await request('POST', `/session/${session}/element/${id}/click`, {});
}
async function screenshot(name) {
  await writeFile(
    join(output, `${name}.png`),
    Buffer.from(await request('GET', `/session/${session}/screenshot`), 'base64'),
  );
}
const layouts = [];
try {
  await until(async () => (await fetch('http://127.0.0.1:4444/status')).ok);
  session = (
    await request('POST', '/session', {
      capabilities: {
        alwaysMatch: { 'tauri:options': { application: resolve('target/debug/dustaway.exe') } },
      },
    })
  ).sessionId;
  await until(() =>
    evaluate('return document.querySelector(".home-scan button:not(:disabled)") != null;'),
  );
  assert.equal(
    await evaluate('return document.querySelector(".app-shell").dataset.runtime;'),
    'fixture',
  );
  for (const theme of ['light', 'dark']) {
    await evaluate('document.documentElement.setAttribute("data-theme",arguments[0]);', theme);
    for (const [width, height] of [
      [1100, 780],
      [760, 580],
    ]) {
      await request('POST', `/session/${session}/window/rect`, { width, height });
      await delay(200);
      const layout = await evaluate(`
        document.querySelector('main').scrollTop=0;
        const visible=e=>!!e&&e.getClientRects().length>0, panel=document.querySelector('.cleanup-decision');
        const buttons=[...document.querySelectorAll('button')].filter(e=>visible(e)&&e.textContent.trim()==='开始扫描');
        const r=buttons[0]?.getBoundingClientRect();
        return {count:buttons.length,title:panel.querySelector('h2').textContent,footer:visible(document.querySelector('footer')),
          book:visible(document.querySelector('.scan-scope')),overflow:document.documentElement.scrollWidth>innerWidth,
          actionVisible:!!r&&r.top>=0&&r.bottom<=innerHeight&&r.right<=innerWidth};`);
      assert.equal(layout.count, 1);
      assert.equal(layout.title, '把空间，慢慢展开。');
      assert(
        !layout.footer && !layout.book && !layout.overflow && layout.actionVisible,
        JSON.stringify(layout),
      );
      layouts.push({ theme, width, height, ...layout });
      await screenshot(`home-${theme}-${width}`);
    }
  }
  assert(await evaluate('return !document.querySelector(".scan-scope");'));
  await click('开始扫描');
  await until(() =>
    evaluate(
      'return [...document.querySelectorAll("button")].some(b=>b.textContent.startsWith("清理已选")&&!b.disabled);',
    ),
  );
  assert(
    await evaluate(
      'return !document.querySelector(".scan-scope") && document.querySelector("footer").getClientRects().length>0;',
    ),
  );
  await screenshot('results-dark-760');
  await click('查看文件');
  await until(() =>
    evaluate('return document.querySelector("[role=dialog]")?.textContent.includes("old.tmp");'),
  );
  await request('POST', `/session/${session}/actions`, {
    actions: [
      {
        type: 'key',
        id: 'keyboard',
        actions: [
          { type: 'keyDown', value: '\uE00C' },
          { type: 'keyUp', value: '\uE00C' },
        ],
      },
    ],
  });
  await until(() => evaluate('return !document.querySelector("[role=dialog]");'));
  await until(() => evaluate('return document.activeElement.textContent.trim() === "查看文件";'));
  await click('清理已选');
  await until(() => evaluate('return !!document.querySelector("[role=alertdialog]");'));
  await screenshot('confirm-dark-760');
  assert(
    await evaluate(
      'return document.querySelector("[role=alertdialog]").textContent.includes("永久删除");',
    ),
  );
  await request('POST', `/session/${session}/actions`, {
    actions: [
      {
        type: 'key',
        id: 'keyboard',
        actions: [
          { type: 'keyDown', value: '\uE00C' },
          { type: 'keyUp', value: '\uE00C' },
        ],
      },
    ],
  });
  await until(() => evaluate('return !document.querySelector("[role=alertdialog]");'));
  await access(file);
  await writeFile(
    join(output, 'result.json'),
    JSON.stringify(
      {
        passed: true,
        fixture: root,
        layouts,
        checks: [
          'scope-expand-collapse',
          'scan-result-collapsed',
          'file-details',
          'escape-focus-return',
          'confirmation-cancel-preserves-file',
        ],
      },
      null,
      2,
    ),
  );
  console.log('Homepage native fixture checks passed');
} catch (error) {
  if (session) await screenshot('failure').catch(() => {});
  throw error;
} finally {
  if (session) await request('DELETE', `/session/${session}`).catch(() => {});
  driver.kill();
}
