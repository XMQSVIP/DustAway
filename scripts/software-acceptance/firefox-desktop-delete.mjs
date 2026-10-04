// Real standard-provider UI test, restricted to the dedicated PC and Firefox fixture.
// The fixture's uninstall registration uses Mozilla's documented /S option. This
// is a real app UI + residual deletion test, not a vendor-wizard interaction test.
import assert from 'node:assert/strict';
import { readFile, writeFile, mkdir, readdir, lstat, access } from 'node:fs/promises';
import { join, resolve, relative, dirname, sep } from 'node:path';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { createDesktopCommand } from '../desktop-driver.mjs';

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
await mkdir(out); // Never overwrite a failed or completed run.
const save = (name, value) => writeFile(join(out, name), JSON.stringify(value, null, 2));
const status = (phase, extra = {}) =>
  save('status.json', { phase, time: new Date().toISOString(), ...extra });
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
const driver = spawn(
  join(root, 'bin', 'tauri-driver.exe'),
  ['--native-driver', join(root, 'artifacts', 'webdriver', 'msedgedriver.exe')],
  {
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
    env: {
      ...process.env,
      DUSTAWAY_E2E_ROOT: '',
      DUSTAWAY_E2E_SOFTWARE_DELAY_MS: '',
      DUSTAWAY_E2E_SOFTWARE_RESIDUALS: '',
    },
  },
);
let log = '';
driver.stdout.on('data', (chunk) => {
  log += chunk;
});
driver.stderr.on('data', (chunk) => {
  log += chunk;
});
const command = createDesktopCommand();
let session;
let submitted = false;
let safeToClose = false;
let taskId;
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

try {
  await until(() =>
    fetch('http://127.0.0.1:4444/status')
      .then((r) => r.ok)
      .catch(() => false),
  );
  const created = await command('POST', '/session', {
    capabilities: {
      alwaysMatch: {
        'tauri:options': { application: join(root, 'target', 'debug', 'dustaway.exe') },
      },
    },
  });
  session = created.sessionId;
  await save('session.json', {
    session,
    driverPid: driver.pid,
    application: 'standard',
    started: new Date().toISOString(),
  });
  await until(() =>
    evaluate('return document.querySelector(".app-shell")?.dataset.runtime === "standard"'),
  );
  const boot = await invoke('bootstrap');
  assert(!boot.version.includes('fixture'));
  const initialViewport = await evaluate('return {width:innerWidth,height:innerHeight}');
  const initialRuntime = await invoke('get_runtime_snapshot', { scope: 'user' });
  assert(!initialRuntime.uninstall, 'Preserve a prior pending real task');
  const oldHistory = await invoke('get_uninstall_history');
  await click('//nav/button[normalize-space(.)="软件卸载"]');
  await until(() => evaluate('return !!document.querySelector(".software-count")'));
  const before = await invoke('list_installed_software');
  assert.equal(before.warnings.length, 0);
  const matches = before.items.filter((s) => /firefox/i.test(s.name));
  assert.equal(matches.length, 1);
  const software = matches[0];
  assert.equal(resolve(software.install_location), install);
  assert(software.can_uninstall && software.source === 'desktop');
  assert(
    software.residual_capabilities.every(
      (capability) => capability.cleanup === 'requires_verification',
    ),
  );
  await save('initial-state.json', {
    boot,
    initialViewport,
    initialRuntime,
    oldHistory,
    before,
    software,
  });
  await evaluate(
    `
    const originalFetch = window.fetch.bind(window);
    window.__nativeDeleteAudit = {uninstalls:0, cleanups:0, plans:[], selections:[]};
    const expectedSoftware = arguments[0];
    window.fetch = function(url, options) {
      const name = String(url).split('/').pop();
      const body = options?.body ? JSON.parse(options.body) : {};
      if(name==='prepare_uninstall' && body.softwareId!==expectedSoftware) throw Error('Unexpected target');
      if(name==='execute_uninstall') { if(window.__nativeDeleteAudit.uninstalls++) throw Error('Duplicate uninstall'); }
      if(name==='prepare_uninstall_residual_cleanup') window.__nativeDeleteAudit.selections.push(body.candidateIds);
      if(name==='execute_uninstall_residual_cleanup') { if(window.__nativeDeleteAudit.cleanups++) throw Error('Duplicate deletion'); }
      return originalFetch(url,options).then(async response => {
        if(name==='prepare_uninstall_residual_cleanup' && response.ok) {
          const reply = await response.clone().json();
          if (!reply.last_request?.error) window.__nativeDeleteAudit.plans.push(reply.last_request?.value);
        }
        return response;
      });
    };`,
    [software.id],
  );
  await type('//input[@type="search"]', software.name);
  await until(() => evaluate('return document.querySelectorAll(".software-row").length===1'));
  await click('//button[contains(@class,"software-row")]');
  await screenshot('01-details');
  await click('//button[normalize-space(.)="卸载此软件"]');
  await click('//button[normalize-space(.)="返回检查"]');
  await until(() => evaluate('return document.activeElement?.classList.contains("software-row")'));
  await key('\uE007');
  await click('//button[normalize-space(.)="卸载此软件"]');
  await screenshot('02-uninstall-confirmation');
  await status('submitting_single_uninstall');
  submitted = true;
  await click('//button[normalize-space(.)="确认卸载"]');
  const task = await until(async () => {
    const t = await invoke('get_uninstall_task');
    return t?.software.id === software.id && t;
  });
  taskId = task.id;
  await status('tracking_registered_silent_uninstall', { taskId });
  await click('//nav/button[normalize-space(.)="历史"]');
  await until(() =>
    evaluate('return document.querySelector(".app-shell").textContent.includes(arguments[0])', [
      software.name,
    ]),
  );
  await screenshot('03-cross-page-status');
  const terminal = await until(async () => {
    const current = await invoke('get_uninstall_task');
    return !['uninstalling', 'verifying'].includes(current.phase) && current;
  }, 180000);
  assert(
    ['removed', 'still_installed', 'residuals_ready'].includes(terminal.phase),
    JSON.stringify(terminal),
  );
  await until(async () => !(await exists(join(install, 'firefox.exe'))), 180000);
  await click('//nav/button[normalize-space(.)="软件卸载"]');
  if (terminal.phase === 'still_installed')
    await click('//button[normalize-space(.)="重新核对安装状态"]');
  await until(async () => (await invoke('get_uninstall_task'))?.phase === 'residuals_ready');
  await until(() => evaluate('return document.querySelectorAll(".residual-row").length>0'));
  assert.equal(await checked(), 0);
  const firstScan = await invoke('list_uninstall_residuals', { taskId, offset: 0, search: '' });
  assert(firstScan.supported && firstScan.total > 0);
  const all = [...firstScan.items];
  for (let offset = 200; offset < firstScan.total; offset += 200) {
    const page = await invoke('list_uninstall_residuals', { taskId, offset, search: '' });
    assert.equal(page.scan_id, firstScan.scan_id);
    all.push(...page.items);
  }
  assert(all.some((r) => !r.selectable));
  const cacheFile = all.find(
    (r) =>
      r.selectable &&
      !r.directory &&
      resolve(r.path).startsWith(join(cache, 'cache2', 'entries') + sep) &&
      BigInt(r.bytes) > 0n,
  );
  assert(cacheFile, 'Need a real Firefox cache entry');
  const parent = all.find(
    (r) => r.directory && r.selectable && resolve(r.path) === dirname(resolve(cacheFile.path)),
  );
  const settings = all.find((r) => r.selectable && resolve(r.path) === join(profile, 'user.js'));
  const cookies = all.find(
    (r) => r.selectable && resolve(r.path) === join(profile, 'cookies.sqlite'),
  );
  assert(parent && settings && cookies);
  const selected = [cacheFile, parent, settings, cookies];
  const expectedBytes = [cacheFile, settings, cookies]
    .reduce((n, r) => n + BigInt(r.bytes), 0n)
    .toString();
  await save('selection-state.json', { taskId, firstScan, selected, expectedBytes });
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
      await command('POST', `/session/${session}/window/rect`, { width, height });
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
  // A fresh WebView also refreshes inventory. Do not supersede its list request
  // with the harness's independent before/after inventory audit.
  await until(() => evaluate('return !!document.querySelector(".software-count")'));
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
  await command('POST', `/session/${session}/window/rect`, initialViewport);
  await save('verification.json', {
    passed: true,
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
  safeToClose = true;
} catch (error) {
  if (session) await screenshot('failure').catch(() => {});
  await writeFile(join(out, 'failure.txt'), String(error.stack ?? error));
  await status('failed', { taskId, submitted, session, keptRunning: submitted && !safeToClose });
  if (submitted && !safeToClose) {
    // Keep the scheduled task and its app alive for inspection of this exact
    // session. A thrown top-level error would close the Windows task job and
    // lose the deliberately non-persistent residual authorization.
    await new Promise((done) => {
      const timer = setInterval(async () => {
        if (await exists(join(out, 'inspection-complete'))) {
          clearInterval(timer);
          done();
        }
      }, 1000);
    });
    safeToClose = true;
  }
  throw error;
} finally {
  await writeFile(join(out, 'driver.log'), log);
  if (!submitted || safeToClose) {
    if (session) await command('DELETE', `/session/${session}`).catch(() => {});
    driver.kill(); // Own test server only, after the app/session has closed; never a vendor process.
  } else {
    driver.stdout.destroy();
    driver.stderr.destroy();
    driver.unref();
  }
}
