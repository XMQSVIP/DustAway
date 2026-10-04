// Ordinary production UI. Read the native preparation receipt before clicking
// confirmation; all native identity/plan checks remain unchanged.
import assert from 'node:assert/strict';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createHash } from 'node:crypto';
import { readFile, writeFile, mkdir, readdir, lstat, realpath, open } from 'node:fs/promises';
import { resolve, join, relative, sep } from 'node:path';
import { createDesktopCommand } from '../desktop-driver.mjs';

assert.equal(process.env.COMPUTERNAME, 'DESKTOP-24NJTT6');
assert.equal(process.env.USERNAME.toLowerCase(), 'msn');
const workspace = process.cwd();
assert.equal(
  await readFile(join(workspace, '.dustaway-completion'), 'utf8'),
  'DustAway.CleanupCompletion.20261001',
);
const output = resolve(process.env.DUSTAWAY_TEST_OUTPUT);
const executable = resolve(process.env.DUSTAWAY_TEST_EXECUTABLE);
const profile = join(process.env.LOCALAPPDATA, 'Google/Chrome/User Data/Default');
const userData = resolve(profile, '..');
const exec = promisify(execFile);
const sha = (data) => createHash('sha256').update(data).digest('hex');
const binarySha256 = sha(await readFile(executable));
const results = [];
async function chromeClosed() {
  const { stdout } = await exec(
    'powershell.exe',
    [
      '-NoProfile',
      '-NonInteractive',
      '-Command',
      '@(Get-Process -Name chrome -ErrorAction SilentlyContinue).Count',
    ],
    { windowsHide: true },
  );
  assert.equal(Number(stdout.trim()), 0, 'Chrome must exit normally before cleanup');
}
async function protection(cache) {
  const rows = [];
  async function walk(path) {
    if (path.toLowerCase() === cache.toLowerCase()) return;
    const stat = await lstat(path, { bigint: true });
    assert(!stat.isSymbolicLink(), 'Protection snapshot never follows links');
    if (stat.isDirectory()) {
      for (const name of (await readdir(path)).sort()) await walk(join(path, name));
    } else {
      rows.push({
        path: relative(userData, path),
        bytes: String(stat.size),
        modified: String(stat.mtimeNs),
        sha256: sha(await readFile(path)),
      });
    }
  }
  await walk(userData);
  return rows;
}
const command = createDesktopCommand();
const driver = spawn(
  'tauri-driver',
  ['--native-driver', resolve('artifacts/webdriver/msedgedriver.exe')],
  { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] },
);
let log = '',
  session;
driver.stdout.on('data', (s) => (log += s));
driver.stderr.on('data', (s) => (log += s));
async function until(fn, timeout = 120000) {
  const end = Date.now() + timeout;
  let last;
  while (Date.now() < end) {
    try {
      const value = await fn();
      if (value) return value;
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
const snapshot = () => ipc('get_runtime_snapshot', { scope: 'user' });
async function click(xpath) {
  const id = await until(async () => {
    const rows = await command('POST', `/session/${session}/elements`, {
      using: 'xpath',
      value: xpath,
    });
    for (const row of rows) {
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
const capture = async (file) =>
  writeFile(file, Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'));
try {
  await chromeClosed();
  assert.equal((await realpath(userData)).toLowerCase(), userData.toLowerCase());
  const profiles = (await readdir(userData)).filter(
    (name) => name === 'Default' || /^Profile [0-9]+$/.test(name),
  );
  assert.deepEqual(profiles, ['Default'], 'Review other profiles before expanding the test');
  await until(async () => (await fetch('http://127.0.0.1:4444/status')).ok);
  session = (
    await command('POST', '/session', {
      capabilities: { alwaysMatch: { 'tauri:options': { application: executable } } },
    })
  ).sessionId;
  const boot = await until(() => ipc('bootstrap'));
  assert.equal(boot.capabilities.system_cleanup, false);
  assert(!boot.capabilities.fixture);
  await command('POST', `/session/${session}/window/rect`, { width: 1100, height: 780 });
  for (const [rule, leaf] of [
    ['chrome.http', 'Cache/Cache_Data'],
    ['chrome.code', 'Code Cache'],
    ['chrome.gpu', 'GPUCache'],
  ]) {
    const directory = join(output, rule);
    await mkdir(directory);
    const cache = resolve(profile, leaf);
    assert(cache.startsWith(profile + sep));
    await chromeClosed();
    await click('//button[starts-with(normalize-space(.),"开始扫描")]');
    const scanned = await until(async () => {
      const state = await snapshot();
      return state.status.state === 'ready' ? state : null;
    });
    const row = scanned.summary.rules.find((item) => item.rule_id === rule);
    assert(row?.files > 0 && !row.blocked && row.phase === 'completed', JSON.stringify(row));
    const candidates = [];
    for (let offset = 0; offset < row.files; offset += 200) {
      const page = await ipc('list_candidates', {
        scope: 'user',
        session: scanned.status.id,
        rule,
        offset,
        limit: 200,
      });
      candidates.push(...page.items);
    }
    assert.equal(candidates.length, row.files);
    for (const file of candidates) {
      assert.equal(file.rule_id, rule);
      assert(
        resolve(file.path)
          .toLowerCase()
          .startsWith(cache.toLowerCase() + sep),
        'Candidate outside exact cache',
      );
    }
    await writeFile(join(directory, 'scan.json'), JSON.stringify({ row, candidates }, null, 2));
    const before = await protection(cache);
    await writeFile(join(directory, 'protection-before.json'), JSON.stringify(before, null, 2));
    // Expand the ordinary result controls, then use their native click handlers.
    await js(`document.querySelectorAll('.review-workspace details').forEach(d=>d.open=true);
      document.querySelectorAll('.purpose-toggle[aria-expanded="false"]').forEach(b=>b.click());`);
    await until(() => js('return !!document.getElementById("choose-"+arguments[0]);', rule));
    for (const id of await js(
      'return [...document.querySelectorAll(".rule-row input:checked")].map(e=>e.id);',
    )) {
      await click(`//input[@id="${id}"]`);
    }
    await click(`//input[@id="choose-${rule}"]`);
    assert.deepEqual(
      await js('return [...document.querySelectorAll(".rule-row input:checked")].map(e=>e.id);'),
      [`choose-${rule}`],
    );
    await capture(join(directory, 'selected.png'));
    await click('//button[starts-with(normalize-space(.),"清理已选")]');
    await until(() => js('return !!document.querySelector("[role=alertdialog]");'));
    await click('//*[@role="alertdialog"]//button[normalize-space(.)="取消"]');
    await until(() => js('return !document.querySelector("[role=alertdialog]");'));
    assert.equal(
      await js('return document.getElementById("choose-"+arguments[0])?.checked;', rule),
      true,
    );
    assert.equal((await snapshot()).report, null);
    await click('//button[starts-with(normalize-space(.),"清理已选")]');
    await until(() => js('return !!document.querySelector("[role=alertdialog]");'));
    const prepared = (await snapshot()).control;
    assert.equal(prepared.last_request?.command, 'prepare_cleanup');
    assert.equal(prepared.last_request.error, null);
    const plans = prepared.last_request.value;
    assert.equal(plans.length, 1);
    assert.equal(plans[0].scope, 'user');
    assert(!plans[0].plan.manual);
    assert.deepEqual(plans[0].plan.rule_ids, [rule]);
    assert.equal(plans[0].plan.files, candidates.length);
    assert.equal(plans[0].plan.bytes, row.bytes);
    await writeFile(join(directory, 'prepared.json'), JSON.stringify(prepared, null, 2));
    await capture(join(directory, 'confirmation.png'));
    await chromeClosed();
    const once = await open(join(workspace, `${rule}.cleanup-started`), 'wx');
    try {
      await once.writeFile(JSON.stringify({ rule, plan: plans[0].plan.id, binarySha256 }));
      await once.sync();
    } finally {
      await once.close();
    }
    await click('//*[@role="alertdialog"]//button[normalize-space(.)="永久删除"]');
    const report = await until(async () => {
      const state = await snapshot();
      return state.report && state.status.state !== 'cleaning' ? state.report : null;
    });
    await writeFile(join(directory, 'report.json'), JSON.stringify(report, null, 2));
    await chromeClosed();
    const after = await protection(cache);
    await writeFile(join(directory, 'protection-after.json'), JSON.stringify(after, null, 2));
    assert.deepEqual(after, before, 'Protected profile files changed');
    assert.equal(report.deleted, candidates.length);
    assert.equal(report.deleted_bytes, row.bytes);
    assert.equal(report.failed + report.skipped + report.unknown, 0);
    assert.deepEqual(
      report.categories.map((item) => item.rule_id),
      [rule],
    );
    for (const file of candidates) await assert.rejects(lstat(file.path), { code: 'ENOENT' });
    await until(() => js('return !!document.querySelector("[aria-label=本次结果摘要]");'));
    await capture(join(directory, 'completed.png'));
    const history = await ipc('get_history');
    assert(
      history.some(
        (entry) => entry.id === report.id && entry.deleted_bytes === report.deleted_bytes,
      ),
    );
    results.push({
      rule,
      report_id: report.id,
      deleted: report.deleted,
      bytes: report.deleted_bytes,
      protected_files: before.length,
      protected_sha256: sha(JSON.stringify(before)),
      ui_cancel_preserved_selection: true,
      binarySha256,
    });
    await writeFile(join(output, 'results.json'), JSON.stringify(results, null, 2));
    await click('//button[normalize-space(.)="完成"]');
  }
  console.log(JSON.stringify({ passed: true, results }));
} catch (error) {
  if (session) await capture(join(output, 'failure.png')).catch(() => {});
  throw error;
} finally {
  if (session) await command('DELETE', `/session/${session}`).catch(() => {});
  driver.kill();
  await writeFile(join(output, 'driver.log'), log);
}
