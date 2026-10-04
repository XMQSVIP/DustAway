import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFile, writeFile, open, utimes, realpath, lstat, mkdir, link } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { createDesktopCommand } from '../desktop-driver.mjs';

assert.equal(process.env.COMPUTERNAME, 'DESKTOP-24NJTT6');
assert.equal(process.env.USERNAME.toLowerCase(), 'msn');
assert.equal(
  await readFile('.dustaway-readonly-unlock', 'utf8'),
  'DustAway.ReadonlyUnlock.20261002.V1',
);
const output = resolve(process.env.DUSTAWAY_TEST_OUTPUT);
const executable = resolve(process.env.DUSTAWAY_TEST_EXECUTABLE);
const binarySha256 = createHash('sha256')
  .update(await readFile(executable))
  .digest('hex');
const protectedParent = resolve(process.env.LOCALAPPDATA, 'CrashDumps');
await mkdir(protectedParent, { recursive: true });
assert.equal((await realpath(protectedParent)).toLowerCase(), protectedParent.toLowerCase());
const protectedLog = join(protectedParent, 'dustaway-readonly-unlock-protected-20261002-v1.dmp');
function dump() {
  const b = Buffer.alloc(48);
  b.write('MDMP');
  b.writeUInt32LE(0xa793, 4);
  b.writeUInt32LE(1, 8);
  b.writeUInt32LE(32, 12);
  b.writeUInt32LE(4, 36);
  b.writeUInt32LE(44, 40);
  return b;
}
const protectedContents = dump();
await writeFile(protectedLog, protectedContents, { flag: 'wx' });
const protectedSamples = [
  { path: protectedLog, sha256: createHash('sha256').update(protectedContents).digest('hex') },
];
await writeFile(
  join(output, 'fixtures.json'),
  JSON.stringify({ protected: protectedSamples }, null, 2),
  { flag: 'wx' },
);
// New read-only application fixture; never overwrite an existing application's data.
const applicationFixture = resolve(process.env.APPDATA, 'HBuilder X');
assert.equal(
  await lstat(applicationFixture).catch(() => null),
  null,
  'Preserve existing HBuilder data',
);
await mkdir(join(applicationFixture, 'Cache'), { recursive: true });
assert.equal((await realpath(applicationFixture)).toLowerCase(), applicationFixture.toLowerCase());
for (const [relative, contents] of [
  ['Cache/data_0', 'application cache fixture'],
  ['Cache/conversations.db', 'protected chat database'],
  ['User/settings.json', '{"protected":true}'],
  ['Backups/work.txt', 'unsaved editor work'],
]) {
  const path = join(applicationFixture, relative);
  await mkdir(resolve(path, '..'), { recursive: true });
  await writeFile(path, contents, { flag: 'wx' });
  await utimes(path, new Date(Date.now() - 20 * 86400000), new Date(Date.now() - 20 * 86400000));
  protectedSamples.push({ path, sha256: createHash('sha256').update(contents).digest('hex') });
}
await writeFile(
  join(output, 'application-fixture.json'),
  JSON.stringify({ root: applicationFixture, protected: protectedSamples }, null, 2),
  { flag: 'wx' },
);

const command = createDesktopCommand();
const driver = spawn(
  'tauri-driver',
  ['--native-driver', resolve('artifacts/webdriver/msedgedriver.exe')],
  { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] },
);
let session,
  log = '';
driver.stdout.on('data', (s) => (log += s));
driver.stderr.on('data', (s) => (log += s));
async function until(fn, timeout = 180000) {
  const end = Date.now() + timeout;
  let last;
  while (Date.now() < end) {
    try {
      const value = await fn();
      if (value) return value;
    } catch (e) {
      last = e;
    }
    await new Promise((done) => setTimeout(done, 200));
  }
  throw last ?? new Error('Desktop condition timed out');
}
const js = (script, ...args) =>
  command('POST', `/session/${session}/execute/sync`, { script, args });
const ipc = (name, args = {}) =>
  js('return window.__TAURI_INTERNALS__.invoke(arguments[0],arguments[1]);', name, args);
const snapshot = () => ipc('get_runtime_snapshot', { scope: 'user' });
async function click(xpath) {
  const id = await until(async () => {
    for (const row of await command('POST', `/session/${session}/elements`, {
      using: 'xpath',
      value: xpath,
    })) {
      const id = row['element-6066-11e4-a52e-4f735466cecf'];
      if (
        (await command('GET', `/session/${session}/element/${id}/displayed`)) &&
        (await command('GET', `/session/${session}/element/${id}/enabled`))
      )
        return id;
    }
  });
  await command('POST', `/session/${session}/element/${id}/click`, {});
}
const capture = async (name) =>
  writeFile(
    join(output, `${name}.png`),
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
const results = [];
async function protect() {
  for (const sample of protectedSamples)
    assert.equal(
      createHash('sha256')
        .update(await readFile(sample.path))
        .digest('hex'),
      sample.sha256,
    );
}
async function visiblePrimary(prefix) {
  const state = await js(
    `const e=[...document.querySelectorAll('button')].find(e=>e.getClientRects().length&&e.textContent.trim().startsWith(arguments[0]));const r=e?.getBoundingClientRect();return {visible:!!r&&r.top>=0&&r.bottom<=innerHeight,overflow:document.documentElement.scrollWidth>innerWidth};`,
    prefix,
  );
  assert(state.visible && !state.overflow, JSON.stringify(state));
}
try {
  await until(async () => (await fetch('http://127.0.0.1:4444/status')).ok);
  session = (
    await command('POST', '/session', {
      capabilities: { alwaysMatch: { 'tauri:options': { application: executable } } },
    })
  ).sessionId;
  const boot = await until(() => ipc('bootstrap'));
  assert(!boot.capabilities.fixture && !boot.capabilities.system_cleanup);
  let round = 0;
  for (const theme of ['light', 'dark'])
    for (const [width, height] of [
      [760, 580],
      [1100, 780],
    ]) {
      const name = `${theme}-${width}`;
      const rule = 'windows.crash_dumps';
      const parent = resolve(process.env.LOCALAPPDATA, 'CrashDumps');
      assert.equal((await realpath(parent)).toLowerCase(), parent.toLowerCase());
      const fixture = join(parent, `dustaway-readonly-unlock-20261002-v1-${round++}.dmp`);
      const contents = dump();
      const file = await open(fixture, 'wx');
      await file.writeFile(contents);
      await file.sync();
      await file.close();
      const old = new Date(Date.now() - 40 * 86400000);
      await utimes(fixture, old, old);
      const uvCache = resolve(process.env.UV_CACHE_DIR);
      assert.equal(uvCache, resolve('uv-fixture-cache'));
      const uvFile = join(uvCache, 'archive-v0', 'dustaway-round-' + round, 'module.py');
      await mkdir(resolve(uvFile, '..'), { recursive: true });
      await writeFile(uvFile, 'DustAway uv orphan ' + round, { flag: 'wx' });
      await writeFile(
        join(uvCache, 'CACHEDIR.TAG'),
        'Signature: 8a477f597d28d172789f06886806bc55\n',
      );
      await writeFile(join(uvCache, '.gitignore'), '*');
      const uvProtected = join(output, 'uv-protected-' + round + '.py');
      await link(uvFile, uvProtected);
      const uvProtectedHash = createHash('sha256')
        .update(await readFile(uvProtected))
        .digest('hex');
      await command('POST', `/session/${session}/window/rect`, { width, height });
      await js(
        'document.documentElement.dataset.theme=arguments[0];document.querySelector("main").scrollTop=0;',
        theme,
      );
      await visiblePrimary('开始扫描');
      await capture(`${name}-home`);
      await click('//button[starts-with(normalize-space(.),"开始扫描")]');
      const scanned = await until(async () => {
        const state = await snapshot();
        return state.status.state === 'ready' ? state : null;
      });
      const page = await ipc('list_candidates', {
        scope: 'user',
        session: scanned.status.id,
        rule,
        offset: 0,
        limit: 200,
      });
      assert.equal(page.total, 1, 'Only the dedicated CrashDumps fixture may be selected');
      assert.equal(resolve(page.items[0].path).toLowerCase(), fixture.toLowerCase());
      const application = scanned.summary.rules.find((r) => r.rule_id === 'appcache.hbuilder');
      assert.equal(
        application.files,
        1,
        'Only a recognized cache file; chat database is protected',
      );
      assert.equal(application.availability, 'validation_pending');
      assert.equal(application.observation.application.identity_status, 'unverified_data');
      await writeFile(
        join(output, name + '-application-scan.json'),
        JSON.stringify(application, null, 2),
        { flag: 'wx' },
      );
      const npm = scanned.summary.rules.find((r) => r.rule_id === 'dev.npm_cache');
      assert(
        npm?.blocked && npm.availability === 'process_unknown',
        'Generic Node must block npm without claiming confirmed occupancy',
      );
      await until(() =>
        js('return !!document.querySelector(".cleanup-decision[data-stage=review]");'),
      );
      assert.equal(
        await js(
          'return document.body.textContent.includes("类检查未完成") || document.body.textContent.includes("查看未完成的检查");',
        ),
        false,
      );
      await visiblePrimary('清理已选');
      await js(
        `document.querySelectorAll('.review-workspace details').forEach(d=>d.open=true);document.querySelectorAll('.purpose-toggle[aria-expanded="false"]').forEach(b=>b.click());`,
      );
      assert.equal(
        await js('return !!document.getElementById("choose-"+arguments[0])?.checked;', rule),
        false,
        'Diagnostics rules default to unselected',
      );
      for (const id of await js(
        'return [...document.querySelectorAll(".rule-row input:checked")].map(e=>e.id);',
      ))
        await click(`//input[@id="${id}"]`);
      await click(`//input[@id="choose-${rule}"]`);
      await click('//button[normalize-space(.)="重新检查运行状态"]');
      await until(() =>
        js(
          'return [...document.querySelectorAll("button")].some(e=>e.textContent.trim()==="重新检查运行状态"&&!e.disabled);',
        ),
      );
      assert.equal(
        await js('return document.getElementById("choose-"+arguments[0])?.checked;', rule),
        true,
      );
      assert.equal(
        await js(`return !!document.querySelector('[data-rule="appcache.hbuilder"] input');`),
        false,
      );
      await capture(`${name}-selected`);
      // A read-only refresh can be stopped without discarding other candidate identities.
      await click(
        '//section[@aria-label="官方工具整理"]//button[normalize-space(.)="重新检查这一项"]',
      );
      const firstRefresh = await until(async () => {
        const s = await snapshot();
        return s.status.rule_refresh ? s.status.rule_refresh : null;
      });
      if (firstRefresh.state === 'scanning') {
        // The read-only scan may finish between polling and this click.
        const cancelAvailable = await js(
          'const b=[...document.querySelectorAll("button")].find(e=>e.textContent.trim()==="取消此项检查"&&!e.disabled); if(b){b.focus();b.click();return true;}return false;',
        );
        await until(async () => (await snapshot()).status.rule_refresh.state !== 'scanning');
        const stopped = await snapshot();
        assert.equal(stopped.status.id, scanned.status.id);
        assert.equal(
          await js('return document.getElementById("choose-"+arguments[0])?.checked;', rule),
          true,
        );
        await writeFile(
          join(output, name + '-refresh-cancel.json'),
          JSON.stringify(stopped, null, 2),
          { flag: 'wx' },
        );
        await writeFile(
          join(output, name + '-cancel-observed.json'),
          JSON.stringify({ cancelAvailable, state: stopped.status.rule_refresh.state }),
          { flag: 'wx' },
        );
        if (cancelAvailable) {
          await until(() =>
            js(
              'return document.activeElement?.getAttribute("aria-label")==="此项检查状态"||document.activeElement?.id==="official-tools-heading";',
            ),
          );
        }
        await capture(name + '-refresh-cancel');
      }
      if ((await snapshot()).summary.rules.find((r) => r.rule_id === 'dev.uv').stale) {
        await click(
          '//section[@aria-label="官方工具整理"]//button[normalize-space(.)="重新检查这一项"]',
        );
        await until(async () => (await snapshot()).status.rule_refresh?.state === 'finished');
      }
      const uvRevisionBeforeTool = (await snapshot()).summary.rules.find(
        (r) => r.rule_id === 'dev.uv',
      ).revision;
      // Dedicated uv cache only: cancellation, native confirmation, official prune, receipt.
      await click('//button[normalize-space(.)="整理 uv 闲置缓存"]');
      await until(() => js('return !!document.querySelector("[role=dialog].uv-dialog");'));
      await visiblePrimary('确认整理闲置缓存');
      await capture(name + '-uv-confirmation');
      await click('//*[@role="dialog"]//button[normalize-space(.)="取消"]');
      assert.equal(await readFile(uvFile, 'utf8'), 'DustAway uv orphan ' + round);
      assert.equal(
        await js('return document.activeElement?.textContent.trim();'),
        '整理 uv 闲置缓存',
      );
      await click('//button[normalize-space(.)="整理 uv 闲置缓存"]');
      await until(() => js('return !!document.querySelector("[role=dialog].uv-dialog");'));
      const uvPlan = (await snapshot()).control.last_request;
      assert.equal(uvPlan.command, 'prepare_uv_cache');
      assert.equal(uvPlan.error, null);
      assert.equal(resolve(uvPlan.value.cache), uvCache);
      await writeFile(
        join(output, name + '-uv-started.json'),
        JSON.stringify({ binarySha256, plan: uvPlan.value }, null, 2),
        { flag: 'wx' },
      );
      await click('//*[@role="dialog"]//button[normalize-space(.)="确认整理闲置缓存"]');
      const uvReport = await until(async () => {
        const t = await ipc('get_uv_task');
        return t && t.id === uvPlan.value.id && t.status !== 'running' ? t : null;
      });
      await writeFile(join(output, name + '-uv-report.json'), JSON.stringify(uvReport, null, 2), {
        flag: 'wx',
      });
      assert.equal(uvReport.status, 'completed', uvReport.message + ' ' + uvReport.output);
      await assert.rejects(lstat(uvFile), { code: 'ENOENT' });
      assert.equal(
        createHash('sha256')
          .update(await readFile(uvProtected))
          .digest('hex'),
        uvProtectedHash,
      );
      assert(
        (await ipc('get_uv_history')).some((r) => r.id === uvReport.id && r.status === 'completed'),
      );
      assert.equal(
        await js('return document.getElementById("choose-"+arguments[0])?.checked;', rule),
        true,
      );
      const refreshed = await until(async () => {
        const value = await snapshot();
        return value.status.rule_refresh?.state === 'finished' &&
          value.status.rule_refresh.rule_id === 'dev.uv' &&
          value.summary.rules.find((r) => r.rule_id === 'dev.uv').revision ===
            uvRevisionBeforeTool + 1
          ? value
          : null;
      });
      assert.equal(
        refreshed.status.id,
        scanned.status.id,
        'Partial refresh preserves the scan session',
      );
      assert.equal(
        refreshed.summary.rules.find((r) => r.rule_id === 'dev.uv').revision,
        uvRevisionBeforeTool + 1,
      );
      const preserved = await ipc('list_candidates', {
        scope: 'user',
        session: scanned.status.id,
        rule,
        offset: 0,
        limit: 200,
      });
      assert.deepEqual(
        preserved.items.map((r) => r.id),
        page.items.map((r) => r.id),
        'Other candidate identities stay stable',
      );
      await writeFile(join(output, name + '-refresh.json'), JSON.stringify(refreshed, null, 2), {
        flag: 'wx',
      });
      await until(() => js('return document.body.textContent.includes("此项检查已更新");'));
      await until(() =>
        js(
          'const b=[...document.querySelectorAll("button")].find(b=>b.getClientRects().length&&b.textContent.trim().startsWith("清理已选"));return !!b&&!b.disabled;',
        ),
      );
      await visiblePrimary('清理已选');
      await capture(name + '-uv-complete');
      await click('//button[starts-with(normalize-space(.),"清理已选")]');
      await until(() => js('return !!document.querySelector("[role=alertdialog]");'));
      await click('//*[@role="alertdialog"]//button[normalize-space(.)="取消"]');
      await until(() => js('return !document.querySelector("[role=alertdialog]");'));
      assert.deepEqual(await readFile(fixture), contents);
      assert.equal(
        await js('return document.activeElement?.textContent.trim().startsWith("清理已选");'),
        true,
      );
      assert.equal(
        await js('return document.getElementById("choose-"+arguments[0])?.checked;', rule),
        true,
      );
      await click('//button[starts-with(normalize-space(.),"清理已选")]');
      await until(() => js('return !!document.querySelector("[role=alertdialog]");'));
      const receipt = (await snapshot()).control.last_request;
      assert.equal(receipt.command, 'prepare_cleanup');
      assert.equal(receipt.error, null);
      const plans = receipt.value;
      assert.equal(plans.length, 1);
      assert.deepEqual(plans[0].plan.rule_ids, [rule]);
      assert.equal(plans[0].plan.files, 1);
      assert.equal(Number(plans[0].plan.bytes), Buffer.byteLength(contents));
      await writeFile(
        join(output, `${name}-plan.json`),
        JSON.stringify({ binarySha256, fixture, plans }, null, 2),
        { flag: 'wx' },
      );
      await capture(`${name}-confirmation`);
      await writeFile(
        join(output, `${name}-delete-started.json`),
        JSON.stringify({ plan: plans[0].plan.id, binarySha256 }),
        { flag: 'wx' },
      );
      await click('//*[@role="alertdialog"]//button[normalize-space(.)="永久删除"]');
      const report = await until(async () => {
        const state = await snapshot();
        return state.report && state.status.state !== 'cleaning' ? state.report : null;
      });
      await writeFile(join(output, `${name}-report.json`), JSON.stringify(report, null, 2));
      assert.equal(report.deleted, 1);
      assert.equal(report.failed + report.skipped + report.unknown, 0);
      assert.equal(Number(report.deleted_bytes), Buffer.byteLength(contents));
      await assert.rejects(lstat(fixture), { code: 'ENOENT' });
      await protect();
      await until(() => js('return !!document.querySelector("[aria-label=本次结果摘要]");'));
      await capture(`${name}-complete`);
      const history = await ipc('get_history');
      assert(
        history.some(
          (item) => item.id === report.id && item.deleted_bytes === report.deleted_bytes,
        ),
      );
      results.push({
        theme,
        width,
        height,
        fixture,
        report,
        binarySha256,
        recheckKeptSelection: true,
        cancelKeptSelectionAndFocus: true,
        protected: protectedSamples.length,
      });
      await writeFile(
        join(output, 'desktop-results.json'),
        JSON.stringify({ passed: results.length === 4, results }, null, 2),
      );
      await click('//button[normalize-space(.)="完成"]');
      await click('//nav//button[normalize-space(.)="历史"]');
      await until(() =>
        js(
          'return [...document.querySelectorAll(".history-receipt")].some(e=>e.textContent.includes("uv 缓存整理 · 已完成"));',
        ),
      );
      await click(
        '(//details[contains(@class,"history-receipt")][contains(.,"uv 缓存整理 · 已完成")]/summary)[1]',
      );
      assert(
        await js(
          'return document.querySelector(".history-page").textContent.includes("差值不代表实际释放磁盘空间");',
        ),
      );
      await capture(name + '-history');
      await click('//nav//button[normalize-space(.)="清理"]');
    }
  console.log(JSON.stringify({ passed: true, binarySha256, flows: results.length }));
} catch (error) {
  if (session) await capture('failure').catch(() => {});
  if (session)
    await snapshot()
      .then((value) =>
        writeFile(join(output, 'failure-snapshot.json'), JSON.stringify(value, null, 2), {
          flag: 'wx',
        }),
      )
      .catch(() => {});
  throw error;
} finally {
  if (session) await command('DELETE', `/session/${session}`).catch(() => {});
  driver.kill();
  await writeFile(join(output, 'driver.log'), log);
}
