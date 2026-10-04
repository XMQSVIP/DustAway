// One authorized interactive removal through the ordinary DustAway UI.
// No vendor silent flags, residual approval overrides, or cleanup commands.
import assert from 'node:assert/strict';
import { readFile, writeFile, mkdir, access } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';

const root = resolve(process.cwd());
assert.equal(process.env.COMPUTERNAME, 'DESKTOP-24NJTT6');
assert.equal(root, join(process.env.LOCALAPPDATA, 'DustAwayAcceptance', '20261001-home-cleanup'));
assert.equal(
  await readFile(join(root, '.dustaway-test-only'), 'utf8'),
  'DustAway.HomeCleanup.20261001',
);
const continuation = process.argv[2] === '--continue-path-fix';
assert(process.argv.length === (continuation ? 3 : 2));
const app = join(
  root,
  continuation ? 'dustaway-uninstall-compatible.exe' : 'dustaway-home-cleanup.exe',
);
const expectedHash = continuation
  ? JSON.parse(await readFile(join(root, 'dingtalk-path-build.json'), 'utf8')).sha256
  : 'E971634972A2E331A4A61EDC9EA0C65339B3F4C0A3E2C42E92BC6F0FEB0C1245';
assert.equal(
  createHash('sha256')
    .update(await readFile(app))
    .digest('hex')
    .toUpperCase(),
  expectedHash,
);
if (continuation) {
  const prior = JSON.parse(
    await readFile(join(root, 'dingtalk-removal-20261001', 'status.json'), 'utf8'),
  );
  assert(prior.phase === 'needs_review' && prior.submitted === false);
  assert.equal(
    await access(join(root, 'dingtalk-removal-20261001', 'uninstall-submit.started')).then(
      () => true,
      () => false,
    ),
    false,
  );
}
const out = join(root, continuation ? 'dingtalk-removal-path-fix' : 'dingtalk-removal-20261001');
await mkdir(out); // Single attempt: never overwrite or resubmit after a timeout.
const save = (name, data) => writeFile(join(out, name), JSON.stringify(data, null, 2));
const status = (phase, details = {}) =>
  save('status.json', { time: new Date().toISOString(), phase, ...details });
const old = join(
  process.env.LOCALAPPDATA,
  'DustAwayAcceptance',
  '20260930-software',
  'firefox-desktop-delete-20261001-v2',
);
const driver = continuation
  ? null
  : spawn(
      join(old, 'bin', 'tauri-driver.exe'),
      ['--native-driver', join(old, 'artifacts', 'webdriver', 'msedgedriver.exe')],
      { windowsHide: true, stdio: 'ignore', env: { ...process.env, DUSTAWAY_E2E_ROOT: '' } },
    );
let session,
  taskId,
  submitted = false;
async function command(method, path, body) {
  const response = await fetch('http://127.0.0.1:4444' + path, {
    method,
    headers: { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(30000),
  });
  const data = await response.json();
  if (!response.ok || data.value?.error) throw Error(JSON.stringify(data));
  return data.value;
}
const evaluate = (script, args = []) =>
  command('POST', `/session/${session}/execute/sync`, { script, args });
const invoke = (name, args = {}) =>
  evaluate('return window.__TAURI_INTERNALS__.invoke(arguments[0],arguments[1])', [name, args]);
async function until(fn, ms = 45000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const result = await fn();
    if (result) return result;
    await new Promise((r) => setTimeout(r, 400));
  }
  throw Error('Timed out: inspect the existing session; do not rerun removal');
}
async function element(xpath) {
  return until(async () => {
    const found = await command('POST', `/session/${session}/elements`, {
      using: 'xpath',
      value: xpath,
    });
    assert(found.length <= 1, 'Ambiguous UI target');
    return found[0]?.['element-6066-11e4-a52e-4f735466cecf'];
  });
}
async function click(xpath) {
  const id = await element(xpath);
  await evaluate('arguments[0].scrollIntoView({block:"center"})', [
    { 'element-6066-11e4-a52e-4f735466cecf': id },
  ]);
  await command('POST', `/session/${session}/element/${id}/click`, {});
}
try {
  await until(() =>
    fetch('http://127.0.0.1:4444/status')
      .then((r) => r.ok)
      .catch(() => false),
  );
  if (continuation) {
    const prior = JSON.parse(
      await readFile(join(root, 'dingtalk-removal-20261001', 'session.json'), 'utf8'),
    );
    // Prior attempt never prepared or submitted uninstall; close only its ordinary app.
    await command('DELETE', `/session/${prior.session}`);
  }
  const created = await command('POST', '/session', {
    capabilities: { alwaysMatch: { 'tauri:options': { application: app } } },
  });
  session = created.sessionId;
  await save('session.json', { session, driverPid: driver?.pid, application: app, ordinary: true });
  await until(() =>
    evaluate('return !!window.__TAURI_INTERNALS__ && !!document.querySelector(".app-shell")'),
  );
  const boot = await invoke('bootstrap');
  assert(!boot.version.includes('fixture'));
  const previous = await invoke('get_uninstall_task');
  assert(
    !previous ||
      !['uninstalling', 'verifying', 'scanning_residuals', 'cleaning_residuals'].includes(
        previous.phase,
      ),
    'Another task is active',
  );
  await click('//nav/button[normalize-space(.)="软件卸载"]');
  await until(() => evaluate('return !!document.querySelector(".software-count")'));
  const list = await invoke('list_installed_software');
  const targets = list.items.filter((s) => s.name === '钉钉');
  assert.equal(targets.length, 1);
  const target = targets[0];
  assert.equal(target.version, '8.2.0-Release.260119001');
  assert.equal(target.publisher, 'Alibaba (China) Network Technology Co.,Ltd.');
  assert(target.can_uninstall && target.source === 'desktop');
  await save('target.json', {
    software: target,
    version: boot.version,
    uninstall_requested_by_user: true,
    residual_cleanup_requested: true,
    residual_deletion_approved: false,
  });
  const search = await element('//input[@type="search"]');
  await command('POST', `/session/${session}/element/${search}/value`, {
    text: '钉钉',
    value: ['钉', '钉'],
  });
  await click('//button[contains(@class,"software-row")][.//strong[normalize-space(.)="钉钉"]]');
  await click('//button[normalize-space(.)="卸载此软件"]');
  await until(() =>
    evaluate(
      'return document.querySelector("[role=alertdialog]")?.textContent.includes("卸载 钉钉")',
    ),
  );
  await writeFile(join(out, 'uninstall-submit.started'), new Date().toISOString(), { flag: 'wx' });
  submitted = true;
  await click('//button[normalize-space(.)="确认卸载"]');
  const task = await until(async () => {
    const t = await invoke('get_uninstall_task');
    return t?.software.id === target.id && t;
  });
  taskId = task.id;
  await status('waiting_for_official_uninstaller', { taskId, task });
  const end = Date.now() + 30 * 60 * 1000;
  let lastPhase;
  while (Date.now() < end) {
    const current = await invoke('get_uninstall_task');
    assert.equal(current?.id, taskId, 'Task was replaced');
    if (current.phase !== lastPhase) {
      await status(current.phase, { taskId, task: current });
      lastPhase = current.phase;
    }
    if (current.phase === 'still_installed') {
      const installed = await access(
        'C:\\Program Files (x86)\\DingDing\\main\\current\\DingTalk.exe',
      ).then(
        () => true,
        () => false,
      );
      if (!installed) await click('//button[normalize-space(.)="检查卸载结果"]');
    }
    if (current.phase === 'residuals_ready') {
      const first = await invoke('list_uninstall_residuals', { taskId, offset: 0, search: '' });
      const rows = [...first.items];
      for (let offset = rows.length; offset < first.total; offset += 200) {
        const page = await invoke('list_uninstall_residuals', { taskId, offset, search: '' });
        assert.equal(page.scan_id, first.scan_id);
        rows.push(...page.items);
      }
      // The actual frozen paths remain only in the app's memory, never in this evidence.
      const files = rows.filter((r) => !r.directory);
      const summary = {
        task: current,
        total: first.total,
        complete: first.complete,
        supported: first.supported,
        warnings: first.warnings,
        files: files.length,
        bytes: files.reduce((n, r) => n + BigInt(r.bytes), 0n).toString(),
        selectable: rows.filter((r) => r.selectable).length,
        by_category: ['cache', 'data'].map((category) => ({
          category,
          files: files.filter((r) => r.category === category).length,
          bytes: files
            .filter((r) => r.category === category)
            .reduce((n, r) => n + BigInt(r.bytes), 0n)
            .toString(),
        })),
        deletion_executed: false,
      };
      await save('scan-summary.json', summary);
      await status('residual_scan_complete', { taskId, summary });
      break;
    }
    if (['cancelled', 'failed', 'unknown', 'restart_required'].includes(current.phase)) break;
    await new Promise((r) => setTimeout(r, 1500));
  }
} catch (error) {
  await status('needs_review', { error: String(error), submitted, taskId });
}
// Leave the ordinary UI and its live uninstall context open for follow-up. No replay.
await new Promise(() => {});
