// Final production binary: real read-only scans and cancellation, plus a labeled
// presentation-only fixture for long results and a determinate cleanup bar.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { createDesktopCommand } from '../desktop-driver.mjs';

assert.equal(process.env.COMPUTERNAME, 'DESKTOP-24NJTT6');
assert.equal(process.env.USERNAME.toLowerCase(), 'msn');
const executable = resolve(process.env.DUSTAWAY_TEST_EXECUTABLE);
const output = resolve(process.env.DUSTAWAY_TEST_OUTPUT);
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
  ownedScan,
  log = '';
driver.stdout.on('data', (data) => {
  log += data;
});
driver.stderr.on('data', (data) => {
  log += data;
});
const js = (script, ...args) =>
  command('POST', `/session/${session}/execute/sync`, { script, args });
const ipc = (name, args = {}) =>
  js('return window.__TAURI_INTERNALS__.invoke(arguments[0],arguments[1]);', name, args);
const snapshot = () => ipc('get_runtime_snapshot', { scope: 'user' });
async function until(fn, timeout = 60000) {
  const end = Date.now() + timeout;
  let last;
  while (Date.now() < end) {
    try {
      const value = await fn();
      if (value) return value;
    } catch (error) {
      last = error;
    }
    await new Promise((done) => setTimeout(done, 100));
  }
  throw last ?? Error('Progress check timed out');
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
const capture = async (name) =>
  writeFile(
    join(output, `${name}.png`),
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
async function measure() {
  return js(`
    const hero = document.querySelector('.task-hero');
    const bar = hero?.querySelector('[role="progressbar"]');
    const button = document.querySelector('.task-dock .primary');
    const main = document.querySelector('main');
    const rect = e => e?.getBoundingClientRect().toJSON();
    const inside = e => { const r = rect(e), m = rect(main); return !!r && r.top >= m.top && r.bottom <= m.bottom && r.left >= 0 && r.right <= innerWidth; };
    const r = rect(button);
    return {hero:rect(hero),bar:rect(bar),button:r,main:rect(main),scroll:main.scrollTop,
      barVisible:inside(bar),buttonVisible:!!r && r.top >= main.getBoundingClientRect().bottom && r.bottom <= innerHeight && r.left >= 0 && r.right <= innerWidth,
      heroPosition:hero && getComputedStyle(hero).position,
      hit:!!r && button.contains(document.elementFromPoint(r.x+r.width/2,r.y+r.height/2)),
      overflow:document.documentElement.scrollWidth>innerWidth,
      percent:bar?.getAttribute('aria-valuenow'),indeterminate:bar?.dataset.indeterminate};
  `);
}
async function check(name, barVisible = true) {
  const layout = await measure();
  assert(
    layout.barVisible === barVisible &&
      layout.buttonVisible &&
      layout.hit &&
      !layout.overflow &&
      !['sticky', 'fixed'].includes(layout.heroPosition),
    JSON.stringify(layout),
  );
  await capture(name);
  return layout;
}
const rows = [];
try {
  await until(() => command('GET', '/status'));
  for (const theme of ['light', 'dark'])
    for (const [width, height] of [
      [1100, 780],
      [760, 580],
    ]) {
      session = (
        await command('POST', '/session', {
          capabilities: { alwaysMatch: { 'tauri:options': { application: executable } } },
        })
      ).sessionId;
      const boot = await until(() => ipc('bootstrap'));
      assert(!boot.capabilities.fixture && !boot.capabilities.system_cleanup);
      await until(() => js('return !!document.querySelector(".home-scan button:not(:disabled)");'));
      await command('POST', `/session/${session}/window/rect`, { width, height });
      await js('document.documentElement.dataset.theme=arguments[0];', theme);
      await click('.home-scan button');
      await until(() => js('return !!document.querySelector(".task-hero [role=progressbar]");'));
      const active = await snapshot();
      assert.equal(active.status.state, 'scanning');
      ownedScan = active.status.id;
      const name = `${theme}-${width}`;
      const top = await check(`${name}-scanning`);
      assert(
        await js(
          'return document.querySelector(".task-hero").textContent.includes("扫描只检查文件，不会删除，可以随时停止。");',
        ),
      );
      // Only extends the result surface for deterministic scroll/focus coverage.
      await js(
        'document.querySelector(".review-workspace").style.minHeight="1500px";document.querySelector("main").scrollTop=550;',
      );
      const scrolled = await check(`${name}-scrolled`, false);
      assert(scrolled.scroll > 300);
      assert(
        scrolled.hero.bottom <= scrolled.main.top,
        'Task details must scroll out of the result area',
      );
      await js('document.querySelector(".footnote-links button").focus({preventScroll:true});');
      for (let step = 0; step < 10; step++) {
        await command('POST', `/session/${session}/actions`, {
          actions: [
            {
              type: 'key',
              id: 'keyboard',
              actions: [
                { type: 'keyDown', value: '\uE008' },
                { type: 'keyDown', value: '\uE004' },
                { type: 'keyUp', value: '\uE004' },
                { type: 'keyUp', value: '\uE008' },
              ],
            },
          ],
        });
        if (
          await js('return document.activeElement===document.querySelector(".task-dock .primary");')
        )
          break;
      }
      assert(
        await js('return document.activeElement===document.querySelector(".task-dock .primary");'),
      );
      assert(await js('return document.activeElement.matches(":focus-visible");'));
      await check(`${name}-keyboard`, false);
      // The determinate bar uses the same in-flow layout. No deletion is submitted.
      await js(`
      document.querySelector('main').scrollTop=0;
      const bar=document.querySelector('.task-hero .live-progress');
      window.__progressOriginal=bar.outerHTML;
      bar.dataset.indeterminate='false';bar.setAttribute('aria-label','文件处理进度');
      bar.setAttribute('aria-valuemax','100');bar.setAttribute('aria-valuenow','25');
      bar.firstElementChild.style.width='25%';
    `);
      const determinate = await check(`${name}-determinate-layout-fixture`);
      assert.equal(determinate.percent, '25');
      await js(
        'document.querySelector(".task-hero .live-progress").outerHTML=window.__progressOriginal;',
      );
      await click('.task-dock .primary');
      const end = await until(async () => {
        const value = await snapshot();
        return ['ready', 'cancelled', 'failed'].includes(value.status.state) ? value : null;
      });
      assert.equal(end.status.id, ownedScan);
      assert.equal(end.report, null, 'A read-only scan must not create a deletion report');
      ownedScan = null;
      await until(() => js('return !document.querySelector(".task-hero");'));
      await capture(`${name}-stopped`);
      rows.push({
        theme,
        width,
        height,
        top,
        scrolled,
        determinate,
        scan: end.status,
        readOnly: true,
      });
      await writeFile(
        join(output, 'results.json'),
        JSON.stringify(
          {
            passed: rows.length === 4,
            binarySha256,
            layoutFixtures: ['1500px result surface', 'determinate 25% bar; no cleanup execution'],
            rows,
          },
          null,
          2,
        ),
      );
      await command('DELETE', `/session/${session}`);
      session = null;
    }
} catch (error) {
  if (session) await capture('failure').catch(() => {});
  throw error;
} finally {
  if (session && ownedScan) {
    const value = await snapshot().catch(() => null);
    if (value?.status.id === ownedScan && value.status.state === 'scanning') {
      await ipc('cancel_job', {
        targets: [{ scope: 'user', id: ownedScan }],
        request: {
          runtime_id: value.control.runtime_id,
          expected_revision: value.control.revision,
          request_id: randomUUID(),
        },
      }).catch(() => {});
    }
  }
  if (session) await command('DELETE', `/session/${session}`).catch(() => {});
  driver.kill();
  await writeFile(join(output, 'driver.log'), log);
}
