// Real-provider UI acceptance. Hard-scoped to the prepared Firefox test installation.
// Never import this from production or reuse it with another machine/profile.
import assert from 'node:assert/strict';
import { readFile, writeFile, mkdir, readdir, lstat, access } from 'node:fs/promises';
import { join, resolve, relative } from 'node:path';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { createDesktopCommand } from '../desktop-driver.mjs';

const root = resolve(process.cwd());
assert.equal(process.env.COMPUTERNAME, 'DESKTOP-24NJTT6');
assert.equal(
  root,
  join(process.env.LOCALAPPDATA, 'DustAwayAcceptance', '20260930-software', 'firefox-desktop-real'),
);
const acceptance = resolve(root, '..');
assert.equal(
  await readFile(join(root, '.dustaway-real-ui-test'), 'utf8'),
  'DustAway.Acceptance.20260930.7c7e25e8',
);
const json = async (path) => JSON.parse((await readFile(path, 'utf8')).replace(/^\uFEFF/, ''));
const ready = await json(join(acceptance, 'firefox-desktop-prepare-result.json'));
assert.equal(ready.ExitCode, 0);
assert.equal(ready.OrdinaryUser, true);
const install = join(acceptance, 'firefox-real', 'application');
const profileName = 'q27dwpy5.DustAwayAcceptance20260930';
const profile = join(process.env.APPDATA, 'Mozilla', 'Firefox', 'Profiles', profileName);
const cacheProfile = join(process.env.LOCALAPPDATA, 'Mozilla', 'Firefox', 'Profiles', profileName);
assert.equal(
  await readFile(join(profile, '.dustaway-test-only'), 'utf8'),
  'DustAwayFirefox20260930',
);
const out = join(root, 'evidence');
await mkdir(out); // A completed or failed run must never be replayed in place.
const status = async (phase, extra = {}) =>
  writeFile(
    join(out, 'status.json'),
    JSON.stringify({ phase, time: new Date().toISOString(), ...extra }, null, 2),
  );
const exists = async (path) =>
  access(path).then(
    () => true,
    () => false,
  );
const wait = (ms) => new Promise((done) => setTimeout(done, ms));
async function until(fn, timeout = 45000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const value = await fn();
    if (value) return value;
    await wait(300);
  }
  throw new Error('Acceptance wait expired; do not repeat the uninstall');
}
async function manifest(directory) {
  const result = {};
  const pending = [directory];
  while (pending.length) {
    const path = pending.pop();
    const stat = await lstat(path);
    assert(!stat.isSymbolicLink(), 'Unexpected link in test profile manifest');
    if (stat.isDirectory()) for (const name of await readdir(path)) pending.push(join(path, name));
    else
      result[relative(directory, path)] = createHash('sha256')
        .update(await readFile(path))
        .digest('hex');
  }
  return result;
}
const driver = spawn(
  join(root, 'bin', 'tauri-driver.exe'),
  ['--native-driver', join(root, 'artifacts', 'webdriver', 'msedgedriver.exe')],
  {
    env: { ...process.env, DUSTAWAY_E2E_ROOT: '', DUSTAWAY_E2E_SOFTWARE_DELAY_MS: '' },
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
  },
);
let log = '';
driver.stdout.on('data', (data) => {
  log += data;
});
driver.stderr.on('data', (data) => {
  log += data;
});
const command = createDesktopCommand();
let session;
let submitted = false;
let completed = false;
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
    assert(matches.length <= 1, 'Ambiguous acceptance UI target');
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
  await command('POST', `/session/${session}/element/${id}/clear`, {});
  await command('POST', `/session/${session}/element/${id}/value`, { text });
}
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
  await until(() =>
    evaluate('return document.querySelector(".app-shell")?.dataset.runtime === "standard"'),
  );
  const boot = await invoke('bootstrap');
  assert(!boot.version.includes('fixture'));
  const runtime = await invoke('get_runtime_snapshot', { scope: 'user' });
  assert(!runtime.uninstall, 'A prior real-provider uninstall task exists; preserve it');
  const historyBefore = await invoke('get_uninstall_history');
  await click('//nav/button[normalize-space(.)="软件卸载"]');
  await until(() => evaluate('return !!document.querySelector(".software-count")'));
  const before = await invoke('list_installed_software');
  assert.equal(before.warnings.length, 0);
  const firefox = before.items.filter((item) =>
    item.name.toLowerCase().startsWith('mozilla firefox'),
  );
  assert.equal(firefox.length, 1);
  const item = firefox[0];
  assert.equal(resolve(item.install_location).toLowerCase(), install.toLowerCase());
  assert.equal(item.version, '140.17.0');
  assert.equal(item.publisher, 'Mozilla');
  assert(item.can_uninstall && item.residual_supported && item.source === 'desktop');
  // Observe the Windows IPC fetch transport. Tauri's invoke property is not
  // replaceable in this runtime; assigning to it silently misses frontend calls.
  await evaluate(
    `const originalFetch=window.fetch.bind(window);
    const expectedSoftwareId=arguments[0];
    window.__firefoxUi={executeCalls:0,verifyCalls:0,scanCalls:0};
    window.fetch=function(url,options){
      const name=String(url).split('/').pop();
      if(name==='prepare_uninstall' && JSON.parse(options.body).softwareId!==expectedSoftwareId) return Promise.reject(Error('Unexpected uninstall identity'));
      if(name==='execute_uninstall') {
        if(window.__firefoxUi.executeCalls!==0) return Promise.reject(Error('Do not dispatch another uninstall'));
        window.__firefoxUi.executeCalls++;
      }
      if(name==='verify_uninstall') window.__firefoxUi.verifyCalls++;
      if(name==='scan_uninstall_residuals') window.__firefoxUi.scanCalls++;
      return originalFetch(url,options);
    };`,
    [item.id],
  );
  await type('//input[@type="search"]', item.name);
  await until(() => evaluate('return document.querySelectorAll(".software-row").length===1'));
  await click('//button[contains(@class,"software-row")]');
  assert(
    (await evaluate('return document.querySelector("[role=dialog]").textContent')).includes(
      item.install_location,
    ),
  );
  await screenshot('01-real-details');
  await click('//button[normalize-space(.)="卸载此软件"]');
  await until(() => evaluate('return !!document.querySelector("[role=alertdialog]")'));
  await screenshot('02-confirmation');
  await status('ready_for_confirmation', {
    software: item.name,
    version: item.version,
    softwareId: item.id,
    installDirectory: item.install_location,
  });
  await writeFile(
    join(out, 'pre-uninstall-identities.json'),
    JSON.stringify(before.items.map((s) => s.id).sort()),
  );
  // A human can confirm the visible app dialog while the harness is paused.
  // Observe that original task and skip the click instead of submitting again.
  await until(async () => {
    if (await exists(join(root, 'begin.signal'))) return true;
    const current = await invoke('get_uninstall_task');
    if (!current) return false;
    assert.equal(current.software.id, item.id);
    return true;
  }, 1800000);
  const existingTask = await invoke('get_uninstall_task');
  if (existingTask) assert.equal(existingTask.software.id, item.id);
  await writeFile(join(root, 'uninstall-submitted.once'), existingTask?.id ?? item.id, {
    flag: 'wx',
  });
  submitted = true;
  if (!existingTask) await click('//button[normalize-space(.)="确认卸载"]');
  const task = await until(async () => {
    const task = await invoke('get_uninstall_task');
    return task?.software.id === item.id && task;
  });
  await status('waiting_for_vendor_wizard', { taskId: task.id, software: item.name });
  await click('//nav/button[normalize-space(.)="历史"]');
  await until(() =>
    evaluate('return document.querySelector(".app-shell").textContent.includes(arguments[0])', [
      item.name,
    ]),
  );
  await screenshot('03-task-on-other-page');
  // The operator writes this signal only after the user finishes the vendor wizard
  // and read-only process/registration checks show it is gone. No timed relaunch.
  await until(() => exists(join(root, 'wizard-completed.signal')), 1800000);
  assert(!(await exists(join(install, 'firefox.exe'))));
  const profileBeforeScan = await manifest(profile);
  const cacheBeforeScan = await manifest(cacheProfile);
  const shared = await readFile(
    join(process.env.APPDATA, 'Mozilla', 'dustaway-shared-witness.txt'),
    'utf8',
  );
  const phaseAfterWizard = (await invoke('get_uninstall_task')).phase;
  await click('//nav/button[normalize-space(.)="软件卸载"]');
  if (['still_installed', 'unknown', 'failed'].includes(phaseAfterWizard))
    await click('//button[normalize-space(.)="重新核对安装状态"]');
  await until(async () => (await invoke('get_uninstall_task'))?.phase === 'residuals_ready');
  await until(() => evaluate('return document.querySelectorAll(".residual-row").length>0'));
  const residuals = await invoke('list_uninstall_residuals', {
    taskId: task.id,
    offset: 0,
    search: '',
  });
  assert(residuals.supported && residuals.total > 0);
  assert.equal(
    residuals.complete,
    false,
    'Existing hardlink witnesses must make inspection incomplete',
  );
  assert(residuals.warnings.some((text) => text.includes('链接')));
  assert(residuals.items.every((record) => !record.selectable));
  assert(
    await evaluate(
      'return [...document.querySelectorAll(".residual-row input")].every(input=>input.disabled&&!input.checked)',
    ),
  );
  assert(
    await evaluate(
      'return [...document.querySelectorAll("button")].find(b=>b.textContent.trim()==="清理已选残留").disabled',
    ),
  );
  await screenshot('04-readonly-residuals');
  await type('//label[span="搜索残留路径"]/input', 'dustaway-validation');
  await until(() =>
    evaluate(
      'return document.querySelectorAll(".residual-row").length>0 && [...document.querySelectorAll(".residual-row strong")].every(e=>e.textContent.includes("dustaway-validation"))',
    ),
  );
  await screenshot('05-filtered-witnesses');
  const after = await invoke('list_installed_software');
  assert.equal(after.warnings.length, 0);
  assert.deepEqual(
    after.items.map((s) => s.id).sort(),
    before.items
      .filter((s) => s.id !== item.id)
      .map((s) => s.id)
      .sort(),
  );
  const finalTask = await invoke('get_uninstall_task');
  assert.equal(finalTask.id, task.id);
  assert.equal(finalTask.residual.deleted_bytes, '0');
  assert.equal(finalTask.residual.deleted_files, 0);
  const calls = await evaluate('return window.__firefoxUi');
  assert.equal(calls.executeCalls, 1);
  assert(calls.scanCalls >= 1);
  await click('//nav/button[normalize-space(.)="历史"]');
  await click('//button[@aria-label="卸载记录"]');
  await until(() =>
    evaluate(
      'return document.querySelector(".receipt-archive")?.textContent.includes(arguments[0])',
      [item.name],
    ),
  );
  const history = await invoke('get_uninstall_history');
  assert.deepEqual(
    history.filter((h) => h.id !== task.id),
    historyBefore,
  );
  assert.equal(history.filter((h) => h.id === task.id).length, 1);
  await screenshot('06-real-history');
  assert.deepEqual(await manifest(profile), profileBeforeScan);
  assert.deepEqual(await manifest(cacheProfile), cacheBeforeScan);
  assert.equal(
    await readFile(join(process.env.APPDATA, 'Mozilla', 'dustaway-shared-witness.txt'), 'utf8'),
    shared,
  );
  completed = true;
  await writeFile(
    join(out, 'verification.json'),
    JSON.stringify(
      {
        passed: true,
        provider: 'standard WindowsSoftware',
        appVersion: boot.version,
        software: item.name,
        version: item.version,
        taskId: task.id,
        phaseAfterWizard,
        finalPhase: finalTask.phase,
        calls,
        confirmationByUserBeforeSignal: !!existingTask,
        unrelatedSoftwareUnchanged: true,
        priorHistoryUnchanged: true,
        uniqueHistoryRecord: true,
        actualResidualDeletedBytes: '0',
        residualRecords: residuals.total,
        partialInspectionShown: true,
        allResidualsReadonly: true,
        productionDeletionEnabled: false,
        profileAndCacheUnchangedByReadonlyScan: true,
        sharedParentPreserved: true,
        scope:
          'real application UI through official interactive uninstall, verification, readonly residual inspection and history; no residual deletion',
      },
      null,
      2,
    ),
  );
  await status('passed', { taskId: task.id });
} catch (error) {
  if (session) await screenshot('failure').catch(() => {});
  await writeFile(join(out, 'failure.txt'), String(error.stack ?? error));
  await status('failed', { submitted, completed, keptRunning: submitted && !completed });
  throw error;
} finally {
  await writeFile(join(out, 'driver.log'), log);
  if (!submitted || completed) {
    if (session) await command('DELETE', `/session/${session}`).catch(() => {});
    driver.kill();
  } else {
    // Preserve the real app/uninstaller for manual inspection after unknown outcomes.
    driver.stdout.destroy();
    driver.stderr.destroy();
    driver.unref();
  }
}
