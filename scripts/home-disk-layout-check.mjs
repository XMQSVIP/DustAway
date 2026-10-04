// Read-only native layout regression. Loading/error markup is a local CSS fixture, not a disk failure.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';
import { createDesktopCommand } from './desktop-driver.mjs';

const baseline = process.env.DUSTAWAY_LAYOUT_BASELINE === '1';
const output = resolve(
  process.env.DUSTAWAY_TEST_OUTPUT || `artifacts/home-disk-layout/${baseline ? 'before' : 'after'}`,
);
const executable = resolve(process.env.DUSTAWAY_TEST_EXECUTABLE || 'target/debug/dustaway.exe');
await mkdir(output, { recursive: true });
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
driver.stdout.on('data', (data) => {
  log += data;
});
driver.stderr.on('data', (data) => {
  log += data;
});
const command = createDesktopCommand();
const js = (script, ...args) =>
  command('POST', `/session/${session}/execute/sync`, { script, args });
async function until(fn) {
  const end = Date.now() + 30000;
  while (Date.now() < end) {
    try {
      if (await fn()) return;
    } catch {
      /* wait for startup */
    }
    await new Promise((done) => setTimeout(done, 100));
  }
  throw Error('Desktop layout check timed out');
}
async function click(selector) {
  const element = await command('POST', `/session/${session}/element`, {
    using: 'css selector',
    value: selector,
  });
  await command(
    'POST',
    `/session/${session}/element/${element['element-6066-11e4-a52e-4f735466cecf']}/click`,
    {},
  );
}
try {
  await until(() => command('GET', '/status'));
  session = (
    await command('POST', '/session', {
      capabilities: { alwaysMatch: { 'tauri:options': { application: executable } } },
    })
  ).sessionId;
  await until(() =>
    js(
      'return document.querySelector(".scan-volumes input:checked") && !document.querySelector(".scan-volumes").disabled;',
    ),
  );
  const disks = await js(
    'return [...document.querySelectorAll(".scan-volumes input[aria-label]:not(:disabled)")].map(e=>e.getAttribute("aria-label"));',
  );
  assert(disks.length > 1, 'Requires multiple supported disks');
  const c = disks.find((name) => name === '扫描 C:\\');
  assert(c, 'Requires C for default selection check');
  const before = await js(
    'return window.__TAURI_INTERNALS__.invoke("get_runtime_snapshot", {scope:"user"});',
  );
  await js(`
    window.__readingState = state => {
      const rows = [...document.querySelectorAll('.home-disk-reading')];
      const originals = rows.map(row => [row, row.dataset.status, row.querySelector('small').textContent]);
      rows.forEach(row => {
        row.dataset.status = state;
        row.querySelector('small').textContent = state === 'loading' ? '正在查询 · 上次读数' : '读取失败 · 上次读数';
      });
      window.__restoreReadings = () => originals.forEach(([row, status, text]) => {
        row.dataset.status = status; row.querySelector('small').textContent = text;
      });
    };
    window.__readLayout = () => {
      const box = selector => document.querySelector(selector).getBoundingClientRect();
      const main = document.querySelector('.main-content');
      const action = box('.home-scan .button');
      const viewport = main.getBoundingClientRect();
      return {homeHeight:box('.sculpture-home').height, controlHeight:box('.home-control').height,
        storageHeight:box('.home-storage').height, actionTop:action.top, pickerTop:box('.scan-volumes').top,
        scrollHeight:main.scrollHeight, width:main.clientWidth, scrollTop:main.scrollTop,
        actionVisible:action.top>=viewport.top && action.bottom<=viewport.bottom};
    };
    window.__captureLayout = () => {
      window.__frames = []; window.__capturing = true;
      const start = performance.now();
      const frame = () => {
        window.__frames.push(window.__readLayout());
        if(performance.now()-start<1300) requestAnimationFrame(frame);
        else window.__capturing = false;
      };
      requestAnimationFrame(frame);
    };
  `);
  const results = [];
  for (const theme of ['light', 'dark']) {
    for (const [width, height] of [
      [760, 580],
      [1100, 780],
    ]) {
      await command('POST', `/session/${session}/window/rect`, { width, height });
      await until(() =>
        js('return !document.querySelector(".home-disk-reading[aria-busy=true]");'),
      );
      await js(
        'document.documentElement.dataset.theme=arguments[0]; document.querySelector(".main-content").scrollTop=0;',
        theme,
      );
      const initial = await js('return window.__readLayout();');
      const steps = [];
      const all = '.scan-volumes input:not([aria-label])';
      const actions = [
        ['all', all],
        ['none', all],
        ['single', `.scan-volumes input[aria-label=${JSON.stringify(c)}]`],
        ['multiple', all],
        ['refresh', '[aria-label="刷新磁盘空间"]'],
        ['loading-markup', null],
        ['error-markup', null],
      ];
      for (const [name, selector] of actions) {
        await js('window.__captureLayout();');
        if (selector) await click(selector);
        else await js('window.__readingState(arguments[0]);', name.split('-')[0]);
        await until(() =>
          js(
            'return !window.__capturing && !document.querySelector(".home-disk-reading[aria-busy=true]");',
          ),
        );
        const frames = await js('return window.__frames;');
        const deltas = Object.fromEntries(
          Object.keys(initial)
            .filter((key) => key !== 'actionVisible')
            .map((key) => [key, Math.max(...frames.map((f) => Math.abs(f[key] - initial[key])))]),
        );
        steps.push({ name, frames: frames.length, deltas });
        if (!baseline) {
          assert(
            Object.values(deltas).every((delta) => delta <= 1),
            `${theme} ${width} ${name}: ${JSON.stringify(deltas)}`,
          );
          assert(
            frames.every((frame) => frame.actionVisible),
            `${theme} ${width}: primary action outside viewport`,
          );
        }
        if (!selector) {
          await writeFile(
            `${output}/${theme}-${width}-${name}.png`,
            Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
          );
          await js('window.__restoreReadings();');
        }
      }
      if (disks.length > 3) {
        await click('.home-more-disks summary');
        const expanded = await js('return window.__readLayout();');
        if (!baseline)
          assert.deepEqual(expanded, initial, 'Expanding extra disks must not move the controls');
        await click('.home-more-disks summary');
      }
      await writeFile(
        `${output}/${theme}-${width}.png`,
        Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
      );
      results.push({ theme, width, height, initial, steps });
      await click(all);
      await click(`.scan-volumes input[aria-label=${JSON.stringify(c)}]`);
    }
  }
  const after = await js(
    'return window.__TAURI_INTERNALS__.invoke("get_runtime_snapshot", {scope:"user"});',
  );
  assert.equal(after.control.revision, before.control.revision);
  assert.equal(after.status.id, before.status.id);
  assert.deepEqual(after.report, before.report);
  await writeFile(
    `${output}/results.json`,
    JSON.stringify(
      {
        baseline,
        results,
        readonly: true,
        executableSha256: createHash('sha256')
          .update(await readFile(executable))
          .digest('hex'),
      },
      null,
      2,
    ),
  );
  console.log(
    `${baseline ? 'BASELINE' : 'PASS'}: four window/theme combinations, five real transitions and two reading-state markup fixtures; no scan or deletion`,
  );
} finally {
  if (session) await command('DELETE', `/session/${session}`).catch(() => {});
  driver.kill();
  await writeFile(`${output}/driver.log`, log);
}
