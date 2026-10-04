// Continues the existing real UI task after the user confirmed before the harness signal.
// No install, prepare, uninstall or residual-delete call exists in this script.
import assert from 'node:assert/strict';
import { readFile, writeFile, access, readdir, lstat } from 'node:fs/promises';
import { join, resolve, relative } from 'node:path';
import { createHash } from 'node:crypto';
import { createDesktopCommand } from '../desktop-driver.mjs';

const root = resolve(process.cwd());
assert.equal(process.env.COMPUTERNAME, 'DESKTOP-24NJTT6');
assert.equal(
  root,
  join(process.env.LOCALAPPDATA, 'DustAwayAcceptance', '20260930-software', 'firefox-desktop-real'),
);
assert.equal(
  await readFile(join(root, '.dustaway-real-ui-test'), 'utf8'),
  'DustAway.Acceptance.20260930.7c7e25e8',
);
const acceptance = resolve(root, '..');
const json = async (path) => JSON.parse((await readFile(path, 'utf8')).replace(/^\uFEFF/, ''));
const observation = await json(join(acceptance, 'firefox-desktop-user-dispatch-observation.json'));
const completion = await json(join(acceptance, 'firefox-desktop-vendor-completed.json'));
assert(
  completion.OrdinaryUser && completion.RegistrationAbsent && completion.VendorProcessesAbsent,
);
const taskId = 'fad47b25-0400-44da-9b3f-557099b9e25f';
const softwareId = 'desktop:07272f175fd3386686c362a315f29c42b3c85c3cf14479a3d77146b09ec98baa';
assert.equal(observation.task.id, taskId);
assert.equal(observation.task.software.id, softwareId);
assert.equal(observation.task.phase, 'still_installed');
assert.equal(observation.runtime, 'standard');
const out = join(root, 'evidence');
await writeFile(join(root, 'continue-existing.once'), taskId, { flag: 'wx' });
const command = createDesktopCommand();
const sessions = await command('GET', '/sessions');
assert.equal(sessions.length, 1);
const session = sessions[0].id;
assert.equal(session, 'b9594689a80b7d22a4ee38c7b9e7ecf2');
const evaluate = (script, args = []) =>
  command('POST', `/session/${session}/execute/sync`, { script, args });
const invoke = (name, args = {}) =>
  evaluate('return window.__TAURI_INTERNALS__.invoke(arguments[0], arguments[1])', [name, args]);
const screenshot = async (name) =>
  writeFile(
    join(out, `${name}.png`),
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
async function until(fn) {
  const deadline = Date.now() + 60000;
  while (Date.now() < deadline) {
    const value = await fn();
    if (value) return value;
    await new Promise((done) => setTimeout(done, 250));
  }
  throw Error('Live-task UI verification timed out; never relaunch uninstall');
}
async function find(xpath) {
  return until(async () => {
    const matches = await command('POST', `/session/${session}/elements`, {
      using: 'xpath',
      value: xpath,
    });
    assert(matches.length <= 1);
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
  const id = await find(xpath);
  await evaluate('arguments[0].scrollIntoView({block:"center"})', [
    { 'element-6066-11e4-a52e-4f735466cecf': id },
  ]);
  await command('POST', `/session/${session}/element/${id}/click`, {});
}
async function manifest(directory) {
  const result = {};
  const pending = [directory];
  while (pending.length) {
    const path = pending.pop();
    const stat = await lstat(path);
    assert(!stat.isSymbolicLink());
    if (stat.isDirectory()) for (const name of await readdir(path)) pending.push(join(path, name));
    else
      result[relative(directory, path)] = createHash('sha256')
        .update(await readFile(path))
        .digest('hex');
  }
  return result;
}
try {
  assert.equal(
    await evaluate('return document.querySelector(".app-shell")?.dataset.runtime'),
    'standard',
  );
  assert.equal(
    await access(join(acceptance, 'firefox-real/application/firefox.exe')).then(
      () => true,
      () => false,
    ),
    false,
  );
  const runtimeBefore = await invoke('get_runtime_snapshot', { scope: 'user' });
  assert.equal(runtimeBefore.uninstall.id, taskId);
  assert.equal(runtimeBefore.uninstall.phase, 'still_installed');
  assert.equal(runtimeBefore.control.last_request.command, 'execute_uninstall');
  assert.equal(runtimeBefore.control.last_request.error, null);
  assert.equal(runtimeBefore.control.last_request.value, taskId);
  await writeFile(
    join(out, 'continuation-before-runtime.json'),
    JSON.stringify(runtimeBefore, null, 2),
  );
  const beforeInventory = await invoke('list_installed_software');
  assert.equal(beforeInventory.warnings.length, 0);
  assert(!beforeInventory.items.some((item) => item.id === softwareId));
  const profileName = 'q27dwpy5.DustAwayAcceptance20260930';
  const profile = join(process.env.APPDATA, 'Mozilla/Firefox/Profiles', profileName);
  const cache = join(process.env.LOCALAPPDATA, 'Mozilla/Firefox/Profiles', profileName);
  assert.equal(
    await readFile(join(profile, '.dustaway-test-only'), 'utf8'),
    'DustAwayFirefox20260930',
  );
  const profileBefore = await manifest(profile);
  const cacheBefore = await manifest(cache);
  const sharedPath = join(process.env.APPDATA, 'Mozilla/dustaway-shared-witness.txt');
  const sharedBefore = await readFile(sharedPath, 'utf8');
  await writeFile(
    join(out, 'continuation-file-baseline.json'),
    JSON.stringify({ profile: profileBefore, cache: cacheBefore }, null, 2),
  );
  await screenshot('03-existing-task-before-verify');
  await click('//button[normalize-space(.)="重新核对安装状态"]');
  await until(async () => (await invoke('get_uninstall_task')).phase === 'residuals_ready');
  await until(() => evaluate('return document.querySelectorAll(".residual-row").length>0'));
  const residuals = await invoke('list_uninstall_residuals', { taskId, offset: 0, search: '' });
  assert(residuals.supported && residuals.total > 0 && !residuals.complete);
  assert(residuals.warnings.some((message) => message.includes('链接')));
  assert(residuals.items.every((item) => !item.selectable));
  assert(
    await evaluate(
      'return [...document.querySelectorAll(".residual-row input")].every(e=>e.disabled&&!e.checked)',
    ),
  );
  assert(
    await evaluate(
      'return [...document.querySelectorAll("button")].find(b=>b.textContent.trim()==="清理已选残留").disabled',
    ),
  );
  assert(
    await evaluate('return document.querySelector(".software-task").textContent.includes("链接")'),
  );
  await screenshot('04-readonly-residuals');
  const search = await find('//label[span="搜索残留路径"]/input');
  await command('POST', `/session/${session}/element/${search}/value`, {
    text: 'dustaway-validation',
  });
  await until(() =>
    evaluate(
      'return document.querySelectorAll(".residual-row").length>0 && [...document.querySelectorAll(".residual-row strong")].every(e=>e.textContent.includes("dustaway-validation"))',
    ),
  );
  await screenshot('05-filtered-witnesses');
  const finalTask = await invoke('get_uninstall_task');
  assert.equal(finalTask.id, taskId);
  assert.equal(finalTask.software.id, softwareId);
  assert.equal(finalTask.residual.deleted_bytes, '0');
  assert.equal(finalTask.residual.deleted_files, 0);
  await click('//nav/button[normalize-space(.)="历史"]');
  await click('//button[@aria-label="卸载记录"]');
  await until(() =>
    evaluate(
      'return document.querySelector(".receipt-archive")?.textContent.includes("Mozilla Firefox ESR")',
    ),
  );
  const history = await invoke('get_uninstall_history');
  assert.equal(history.length, 1);
  assert.equal(history[0].id, taskId);
  assert.equal(history[0].phase, 'residuals_ready');
  assert.equal(history[0].software.install_location, null);
  await screenshot('06-real-history');
  assert.deepEqual(await manifest(profile), profileBefore);
  assert.deepEqual(await manifest(cache), cacheBefore);
  assert.equal(await readFile(sharedPath, 'utf8'), sharedBefore);
  const afterInventory = await invoke('list_installed_software');
  assert.equal(afterInventory.warnings.length, 0);
  assert.deepEqual(
    afterInventory.items.map((item) => item.id).sort(),
    beforeInventory.items.map((item) => item.id).sort(),
  );
  const runtimeAfter = await invoke('get_runtime_snapshot', { scope: 'user' });
  assert.equal(runtimeAfter.control.runtime_id, runtimeBefore.control.runtime_id);
  await writeFile(join(out, 'continuation-final-task.json'), JSON.stringify(finalTask, null, 2));
  await writeFile(
    join(out, 'continuation-verification.json'),
    JSON.stringify(
      {
        passed: true,
        taskId,
        runtimeId: runtimeBefore.control.runtime_id,
        provider: 'standard WindowsSoftware',
        ordinaryUser: true,
        confirmation: 'user confirmed the original app dialog before the harness begin signal',
        originalHarnessCompleted: false,
        continuationUsedSameLiveAppAndPreparedContext: true,
        uninstallReceiptId: runtimeBefore.control.last_request.request_id,
        uninstallDispatchCountMeasured: false,
        continuationUninstallCalls: 0,
        phaseBefore: runtimeBefore.uninstall.phase,
        phaseAfter: finalTask.phase,
        uniqueHistoryRecord: true,
        historyInstallationPathRedacted: true,
        residualRecords: residuals.total,
        partialInspectionWarningVisible: true,
        allResidualsReadonlyAndUnchecked: true,
        cleanupButtonDisabled: true,
        actualResidualDeletedBytes: '0',
        profileAndCacheUnchangedByReadonlyScan: true,
        sharedParentPreserved: true,
        installationIdentitiesUnchangedDuringContinuation: true,
        productionDeletionEnabled: false,
        scope:
          'original standard app UI confirmation and vendor wizard followed by live-task verification, readonly residual UI and history; no residual deletion',
      },
      null,
      2,
    ),
  );
  console.log('PASS: continued the same real app task, readonly residual UI and unique history.');
} catch (error) {
  await screenshot('continuation-failure').catch(() => {});
  await writeFile(join(out, 'continuation-failure.txt'), String(error.stack ?? error));
  throw error;
}
// The coordinator closes the inactive test app and removes tasks after collecting evidence.
