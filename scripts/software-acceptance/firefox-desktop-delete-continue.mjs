// Real standard-provider UI test, restricted to the dedicated PC and Firefox fixture.
// The fixture's uninstall registration uses Mozilla's documented /S option. This
// is a real app UI + residual deletion test, not a vendor-wizard interaction test.
import assert from 'node:assert/strict';
import { readFile, writeFile, mkdir, readdir, lstat, access } from 'node:fs/promises';
import { join, resolve, relative, dirname, sep } from 'node:path';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import {
  createDesktopCommand,
  readNativeWindow,
  assertNativeViewport,
} from '../desktop-driver.mjs';

const root = resolve(process.cwd());
const acceptance = join(process.env.LOCALAPPDATA, 'DustAwayAcceptance', '20260930-software');
assert.equal(process.env.COMPUTERNAME, 'DESKTOP-24NJTT6');
assert.equal(root, join(acceptance, 'firefox-desktop-delete-20261001-v2'));
assert.equal(
  await readFile(join(root, '.dustaway-real-ui-test'), 'utf8'),
  'DustAway.Acceptance.20260930.7c7e25e8',
);
const json = async (path) => JSON.parse((await readFile(path, 'utf8')).replace(/^\uFEFF/, ''));
const ready = await json(join(acceptance, 'firefox-desktop-delete-v2-ready.json'));
assert(
  ready.OrdinaryUser && ready.SilentFixtureRegistration && ready.SignedVersion === '140.17.0.1158',
);
assert(/^[a-zA-Z0-9_-]+\.DustAwayAcceptance20260930$/.test(ready.ProfileDirectory));
const profile = join(process.env.APPDATA, 'Mozilla', 'Firefox', 'Profiles', ready.ProfileDirectory);
const cache = join(
  process.env.LOCALAPPDATA,
  'Mozilla',
  'Firefox',
  'Profiles',
  ready.ProfileDirectory,
);
const install = join(acceptance, 'firefox-real', 'application');
assert.equal(
  await readFile(join(profile, '.dustaway-test-only'), 'utf8'),
  'DustAwayFirefox20260930',
);
const out = join(root, 'evidence');
// Continue the exact live session; preserve the original failure evidence.
const save = (name, value) => writeFile(join(out, name), JSON.stringify(value, null, 2));
const status = (phase, extra = {}) =>
  save('continuation-status.json', { phase, time: new Date().toISOString(), ...extra });
const exists = (path) =>
  access(path).then(
    () => true,
    () => false,
  );
async function until(fn, timeout = 60000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const result = await fn();
    if (result) return result;
    await new Promise((done) => setTimeout(done, 200));
  }
  throw Error('Observation timed out; inspect this same session, never replay uninstall');
}
async function manifest(base) {
  const result = {};
  const pending = [base];
  while (pending.length) {
    const path = pending.pop();
    const stat = await lstat(path);
    assert(!stat.isSymbolicLink(), 'Do not traverse a linked fixture path');
    if (stat.isDirectory()) for (const name of await readdir(path)) pending.push(join(path, name));
    else
      result[relative(base, path)] = createHash('sha256')
        .update(await readFile(path))
        .digest('hex');
  }
  return result;
}
const command = createDesktopCommand();
const session = 'b573a9a035519d7fbb4b4cc2e9519bb5';

const evaluate = (script, args = []) =>
  command('POST', `/session/${session}/execute/sync`, { script, args });
const invoke = (name, args = {}) =>
  evaluate('return window.__TAURI_INTERNALS__.invoke(arguments[0], arguments[1])', [name, args]);
const screenshot = async (name) =>
  writeFile(
    join(out, `${name}.png`),
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
async function element(xpath) {
  return until(async () => {
    const matches = await command('POST', `/session/${session}/elements`, {
      using: 'xpath',
      value: xpath,
    });
    assert(matches.length <= 1, 'Ambiguous target');
    const id = matches[0]?.['element-6066-11e4-a52e-4f735466cecf'];
    return (
      id &&
      (await command('GET', `/session/${session}/element/${id}/displayed`)) &&
      (await command('GET', `/session/${session}/element/${id}/enabled`)) &&
      id
    );
  });
}
async function click(xpath) {
  const id = await element(xpath);
  await evaluate('arguments[0].scrollIntoView({block:"center"})', [
    { 'element-6066-11e4-a52e-4f735466cecf': id },
  ]);
  await command('POST', `/session/${session}/element/${id}/click`, {});
}
async function type(xpath, text) {
  const id = await element(xpath);
  await command('POST', '/session/' + session + '/element/' + id + '/click', {});
  await command('POST', '/session/' + session + '/actions', {
    actions: [
      {
        type: 'key',
        id: 'keyboard',
        actions: [
          { type: 'keyDown', value: '\uE009' },
          { type: 'keyDown', value: 'a' },
          { type: 'keyUp', value: 'a' },
          { type: 'keyUp', value: '\uE009' },
          { type: 'keyDown', value: '\uE003' },
          { type: 'keyUp', value: '\uE003' },
        ],
      },
    ],
  });
  for (const char of text) {
    await command('POST', '/session/' + session + '/element/' + id + '/value', { text: char });
    await new Promise((done) => setTimeout(done, 15));
  }
  assert.equal(
    await command('GET', '/session/' + session + '/element/' + id + '/property/value'),
    text,
  );
}

const key = (value) =>
  command('POST', `/session/${session}/actions`, {
    actions: [
      {
        type: 'key',
        id: 'keyboard',
        actions: [
          { type: 'keyDown', value },
          { type: 'keyUp', value },
        ],
      },
    ],
  });
const checked = () =>
  evaluate('return document.querySelectorAll(".residual-row input:checked").length');

const app = join(root, 'target', 'debug', 'dustaway.exe');
const { boot, initialViewport, initialRuntime, oldHistory, before, software } = await json(
  join(out, 'initial-state.json'),
);
const { taskId, firstScan, selected, expectedBytes } = await json(
  join(out, 'selection-state.json'),
);
const [cacheFile, parent, settings, cookies] = selected;
assert.equal(taskId, '8451a694-498b-42db-b7b1-1145428c91c9');
assert.equal((await json(join(out, 'session.json'))).session, session);
assert.equal((await json(join(out, 'status.json'))).phase, 'failed');
const existing = await invoke('get_uninstall_task');
assert.equal(existing.id, taskId);
assert.equal(existing.residual_scan_id, firstScan.scan_id);
assert.equal(existing.phase, 'residuals_ready');
assert.equal(existing.uninstall_status, 'removed');
assert.equal(existing.residual.deleted_files, 0);
const priorAudit = await evaluate('return window.__nativeDeleteAudit');
assert.equal(priorAudit.uninstalls, 1);
assert.equal(priorAudit.cleanups, 0);
assert.equal(priorAudit.plans.length, 0);
assert.equal(await checked(), 0);
assert(!(await exists(join(install, 'firefox.exe'))));
await writeFile(join(out, 'continuation.started'), new Date().toISOString(), { flag: 'wx' });
await status('continuing_same_task_without_replaying_uninstall', { taskId, session });
try {
  const layouts = [];
  for (const theme of ['light', 'dark']) {
    const label = theme === 'dark' ? '切换为深色' : '切换为浅色';
    if (
      await evaluate('return !!document.querySelector(`[aria-label="${arguments[0]}"]`)', [label])
    )
      await click(`//button[@aria-label="${label}"]`);
    for (const [width, height] of [
      [1100, 780],
      [760, 580],
    ]) {
      assertNativeViewport(await readNativeWindow(app, { width, height }), { width, height });
      await until(() =>
        evaluate('return innerWidth===arguments[0] && innerHeight===arguments[1]', [width, height]),
      );
      const layout = await evaluate(
        'return {width:innerWidth,height:innerHeight,overflow:document.documentElement.scrollWidth>innerWidth,rowsFit:[...document.querySelectorAll(".residual-row")].every(r=>r.getBoundingClientRect().right<=innerWidth)}',
      );
      assert(!layout.overflow && layout.rowsFit, JSON.stringify(layout));
      layouts.push({ theme, ...layout });
      await screenshot(`04-residuals-${theme}-${width}`);
    }
  }
  for (const item of selected) {
    await type('//label[span="搜索残留路径"]/input', item.path);
    const checkbox = `//input[@aria-label="选择 ${item.path}"]`;
    await click(checkbox);
    assert(
      await evaluate(
        'return [...document.querySelectorAll(".residual-row input")].find(e=>e.getAttribute("aria-label")===arguments[0]).checked',
        [`选择 ${item.path}`],
      ),
    );
  }
  await type('//label[span="搜索残留路径"]/input', '');
  await until(() =>
    evaluate('return document.querySelector(".software-tools").textContent.includes("已选 4 项")'),
  );
  const beforeFiles = { profile: await manifest(profile), cache: await manifest(cache) };
  const sharedPath = join(process.env.APPDATA, 'Mozilla', 'dustaway-shared-witness.txt');
  const shared = await readFile(sharedPath, 'utf8');
  await click('//button[normalize-space(.)="清理已选残留"]');
  const dialog = await until(() =>
    evaluate('return document.querySelector("[role=alertdialog]")?.textContent'),
  );
  assert(dialog.includes('3 个文件、1 个空文件夹') && dialog.includes('不会放入回收站'));
  assert(dialog.includes('可能失去登录状态、浏览记录、离线内容或文档备份'));
  for (const item of selected) assert(dialog.includes(item.path));
  await screenshot('05-data-confirmation-dark-760');
  await click('//button[normalize-space(.)="返回检查"]');
  await until(() =>
    evaluate('return document.activeElement?.textContent.includes("清理已选残留")'),
  );
  assert.deepEqual({ profile: await manifest(profile), cache: await manifest(cache) }, beforeFiles);
  assert.equal((await invoke('get_uninstall_task')).residual.deleted_files, 0);
  assert.equal(await evaluate('return window.__nativeDeleteAudit.cleanups'), 0);
  await click('//button[normalize-space(.)="清理已选残留"]');
  await until(() => evaluate('return !!document.querySelector("[role=alertdialog]")'));
  const audit = await evaluate('return window.__nativeDeleteAudit');
  assert.equal(audit.plans.length, 2);
  assert.notEqual(audit.plans[0].id, audit.plans[1].id);
  assert.equal(audit.plans[1].bytes, expectedBytes);
  assert(audit.plans[1].includes_data);
  for (const ids of audit.selections)
    assert.deepEqual([...ids].sort(), selected.map((r) => r.id).sort());
  const newFile = join(parent.path, '.dustaway-ui-created-after-confirmation');
  assert(newFile.startsWith(join(cache, 'cache2', 'entries') + sep));
  await writeFile(newFile, 'new file is outside the confirmed frozen list', { flag: 'wx' });
  beforeFiles.cache[relative(cache, newFile)] = createHash('sha256')
    .update(await readFile(newFile))
    .digest('hex');
  await status('submitting_single_residual_cleanup', {
    taskId,
    files: 3,
    directories: 1,
    expectedBytes,
  });
  await save('before-delete.json', { beforeFiles, shared, layouts, audit, newFile });
  await click('//button[normalize-space(.)="确认永久删除"]');
  const cleaned = await until(async () => {
    const t = await invoke('get_uninstall_task');
    return t.phase === 'finished' && t;
  });
  assert.equal(cleaned.uninstall_status, 'removed');
  assert.equal(cleaned.residual.deleted_files, 3);
  assert.equal(cleaned.residual.deleted_directories, 0);
  assert.equal(cleaned.residual.deleted_bytes, expectedBytes);
  assert.equal(cleaned.residual.skipped, 1); // Parent remains nonempty, including the new file.
  assert.equal(
    cleaned.residual.failed + cleaned.residual.unknown + cleaned.residual.unprocessed,
    0,
  );
  for (const item of [cacheFile, settings, cookies]) {
    assert(!(await exists(item.path)));
    const base = resolve(item.path).startsWith(profile + sep) ? 'profile' : 'cache';
    delete beforeFiles[base][relative(base === 'profile' ? profile : cache, item.path)];
  }
  assert.deepEqual({ profile: await manifest(profile), cache: await manifest(cache) }, beforeFiles);
  assert.equal(await readFile(sharedPath, 'utf8'), shared);
  await until(() =>
    evaluate(
      'return document.querySelector(".software-task").textContent.includes("实际删除 3 个文件、0 个空文件夹")',
    ),
  );
  await screenshot('06-actual-results');
  const finalAudit = await evaluate('return window.__nativeDeleteAudit');
  assert.equal(finalAudit.uninstalls, 1);
  assert.equal(finalAudit.cleanups, 1);
  await save('final-task.json', cleaned);
  await click('//nav/button[normalize-space(.)="历史"]');
  await click('//button[@aria-label="卸载记录"]');
  await until(() =>
    evaluate(
      'return document.querySelector(".receipt-archive")?.textContent.includes("Mozilla Firefox")',
    ),
  );
  const history = await invoke('get_uninstall_history');
  assert.equal(history.length, oldHistory.length + 1);
  assert.deepEqual(
    history.filter((h) => h.id !== taskId),
    oldHistory,
  );
  const record = history.find((h) => h.id === taskId);
  assert.equal(record.residual.deleted_bytes, expectedBytes);
  assert.equal(record.can_scan_residuals, false);
  assert.equal(record.residual_scan_id, null);
  assert.equal(record.software.install_location, null);
  await screenshot('07-real-history');
  await command('POST', `/session/${session}/refresh`, {});
  await until(() =>
    evaluate('return document.querySelector(".app-shell")?.dataset.runtime==="standard"'),
  );
  await click('//nav/button[normalize-space(.)="软件卸载"]');
  await until(() =>
    evaluate(
      'return document.querySelector(".software-task")?.textContent.includes("实际删除 3 个文件")',
    ),
  );
  const rebound = await invoke('get_runtime_snapshot', { scope: 'user' });
  assert.equal(rebound.control.runtime_id, initialRuntime.control.runtime_id);
  assert.equal(rebound.uninstall.id, taskId);
  await click('//button[normalize-space(.)="检查残留"]');
  const rescanned = await until(async () => {
    const t = await invoke('get_uninstall_task');
    return t.phase === 'residuals_ready' && t.residual_scan_id !== firstScan.scan_id && t;
  });
  assert.equal(rescanned.id, taskId);
  await until(() => evaluate('return document.querySelectorAll(".residual-row").length>0'));
  assert.equal(await checked(), 0);
  assert(await exists(newFile));
  assert.equal((await invoke('get_uninstall_task')).residual.deleted_bytes, expectedBytes);
  const afterInventory = await invoke('list_installed_software');
  assert.equal(afterInventory.warnings.length, 0);
  assert.deepEqual(
    afterInventory.items.map((s) => s.id).sort(),
    before.items
      .filter((s) => s.id !== software.id)
      .map((s) => s.id)
      .sort(),
  );
  await screenshot('08-reconnected-rescan');
  // Restore the user's previous theme through the actual settings page.
  await click('//nav/button[normalize-space(.)="设置"]');
  await click(
    `//button[normalize-space(.)="${{ system: '跟随系统', light: '浅色', dark: '深色' }[boot.settings.theme]}"]`,
  );
  await until(async () => (await invoke('bootstrap')).settings.theme === boot.settings.theme);
  assertNativeViewport(await readNativeWindow(app, initialViewport), initialViewport);
  await save('verification.json', {
    passed: true,
    continuedSameSessionAfterInputFailure: true,
    slowWebDriverTyping: true,
    provider: 'standard WindowsSoftware',
    appVersion: boot.version,
    taskId,
    nativeUninstallExecuted: true,
    registeredOfficialSilentOption: true,
    vendorWizardInteractionTest: false,
    realResidualDeletion: true,
    selectedFileCount: 3,
    selectedDirectoryCount: 1,
    deletedBytes: expectedBytes,
    deletedDirectoryCount: 0,
    nonemptyParentPreserved: true,
    newFilePreserved: true,
    defaultUnchecked: true,
    settingsAndDataImpactConfirmed: true,
    cancelDidNotDelete: true,
    cancelRestoredFocus: true,
    preparationAfterCancelUsesNewPlan: true,
    oneUninstallDispatch: true,
    oneDeleteDispatch: true,
    onlyBackendCandidateIdsSubmitted: true,
    unselectedHashesUnchanged: true,
    sharedParentUnchanged: true,
    parentChildBytesCountedOnce: true,
    estimatedSizeNotCounted: true,
    standardUninstallStatusPreserved: true,
    priorHistoryUnchanged: true,
    historyAuthorityRemoved: true,
    sameRuntimeWebviewReconnect: true,
    rescanClearedSelection: true,
    oldScanId: firstScan.scan_id,
    newScanId: rescanned.residual_scan_id,
    layouts,
    themeRestored: true,
  });
  await status('passed', { taskId });
  await writeFile(join(out, 'inspection-complete'), 'DustAway.Acceptance.20260930.7c7e25e8', {
    flag: 'wx',
  });
} catch (error) {
  await screenshot('continuation-failure').catch(() => {});
  await writeFile(join(out, 'continuation-failure.txt'), String(error.stack ?? error));
  await status('failed', { taskId, session, keptRunning: true });
  throw error;
}
