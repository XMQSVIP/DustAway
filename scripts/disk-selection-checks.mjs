import { createDesktopCommand } from './desktop-driver.mjs';
// Ordinary desktop build. Read-only scan validation; never prepares or executes deletion.
import assert from 'node:assert/strict';
import { checkPurposeResults } from './purpose-result-checks.mjs';
import { spawn } from 'node:child_process';
import { resolve } from 'node:path';
import { mkdir, writeFile, readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
const output = resolve('artifacts/disk-selection-desktop');
await mkdir(output, { recursive: true });
const driver = spawn(
  'tauri-driver',
  ['--native-driver', resolve('artifacts/webdriver/msedgedriver.exe')],
  { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] },
);
let session,
  log = '';
driver.stdout.on('data', (s) => {
  log += s;
});
driver.stderr.on('data', (s) => {
  log += s;
});
const command = createDesktopCommand();
async function until(fn, timeout = 30000) {
  const end = Date.now() + timeout;
  let last;
  while (Date.now() < end) {
    try {
      const value = await fn();
      if (value) return value;
    } catch (e) {
      last = e;
    }
    await new Promise((r) => setTimeout(r, 150));
  }
  throw last ?? Error('Timed out');
}
const js = (script, args = []) =>
  command('POST', `/session/${session}/execute/sync`, { script, args });
const ipc = (name, args = {}) =>
  js(
    'return window.__TAURI_INTERNALS__.invoke(arguments[0],arguments[1]).catch(error=>{throw new Error(typeof error === "string" ? error : JSON.stringify(error));});',
    [name, args],
  );
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
async function controlled(name, args) {
  const control = (await snapshot()).control;
  return ipc(name, {
    ...args,
    request: {
      runtime_id: control.runtime_id,
      expected_revision: control.revision,
      request_id: crypto.randomUUID(),
    },
  });
}
try {
  await until(async () => (await fetch('http://127.0.0.1:4444/status')).ok);
  session = (
    await command('POST', '/session', {
      capabilities: {
        alwaysMatch: { 'tauri:options': { application: resolve('target/debug/dustaway.exe') } },
      },
    })
  ).sessionId;
  await until(() =>
    js(
      'return document.querySelector(".app-shell")?.dataset.runtime === "standard" && !!document.querySelector(".scan-volumes input:checked");',
    ),
  );
  const volumes = (await ipc('get_analysis_locations')).filter(
    (l) => l.kind === 'volume' && l.supported !== false,
  );
  const c = volumes.find((l) => /^C:\\$/i.test(l.path));
  assert(c, 'C is required for this machine-specific check');
  const readChecked = () =>
    js(
      'return [...document.querySelectorAll(".scan-volumes input[aria-label]:checked")].map(e=>e.getAttribute("aria-label").slice(3));',
    );
  assert.deepEqual(await readChecked(), [c.path]);
  const homeReadings = () =>
    js(
      'return [...document.querySelectorAll(".home-disk-reading")].map(e=>({label:e.getAttribute("aria-label"),loading:e.getAttribute("aria-busy"),value:e.querySelector(".storage-value").textContent}));',
    );
  await until(async () => (await homeReadings()).every((r) => r.loading === 'false'));
  assert.deepEqual(
    (await homeReadings()).map((r) => r.label),
    [`${c.path}可用空间`],
  );
  const otherReading = volumes.find((v) => v.token !== c.token);
  if (otherReading) {
    await click(`//input[@aria-label="扫描 ${otherReading.path}"]`);
    await until(
      async () =>
        (await homeReadings()).length === 2 &&
        (await homeReadings()).every((r) => r.loading === 'false'),
    );
    assert.deepEqual(
      new Set((await homeReadings()).map((r) => r.label)),
      new Set([c, otherReading].map((v) => `${v.path}可用空间`)),
    );
    await click(`//input[@aria-label="扫描 ${c.path}"]`);
    assert.deepEqual(
      (await homeReadings()).map((r) => r.label),
      [`${otherReading.path}可用空间`],
    );
    await click(`//input[@aria-label="扫描 ${c.path}"]`);
    await click(`//input[@aria-label="扫描 ${otherReading.path}"]`);
  }
  await click('//label[normalize-space(.)="全部磁盘"]/input');
  await until(
    async () =>
      (await homeReadings()).length === volumes.length &&
      (await homeReadings()).every((r) => r.loading === 'false'),
  );
  const allReadings = await homeReadings();
  await command('POST', `/session/${session}/window/rect`, { width: 760, height: 580 });
  const allLayout = await js(
    `const main=document.querySelector('.main-content'), button=document.querySelector('.home-scan .button').getBoundingClientRect(), picker=document.querySelector('.scan-volumes').getBoundingClientRect(), viewport=main.getBoundingClientRect();return {overflow:main.scrollWidth>main.clientWidth+1, actionVisible:button.top>=viewport.top&&button.bottom<=viewport.bottom,pickerVisible:picker.bottom<=viewport.bottom,foldVisible:!document.querySelector('.home-more-disks')||document.querySelector('.home-more-disks summary').getBoundingClientRect().bottom<=viewport.bottom,folded:!document.querySelector('.home-more-disks')?.open};`,
  );
  assert(
    !allLayout.overflow &&
      allLayout.actionVisible &&
      allLayout.pickerVisible &&
      allLayout.foldVisible &&
      allLayout.folded,
    JSON.stringify(allLayout),
  );

  await writeFile(
    `${output}/all-disk-readings.png`,
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
  await click('//label[normalize-space(.)="全部磁盘"]/input');
  await click(`//input[@aria-label="扫描 ${c.path}"]`);

  assert.equal((await snapshot()).status.id, '');
  assert.equal(await js('return document.querySelectorAll(".application-guides").length;'), 0);
  const layouts = [];
  for (const [width, height] of [
    [760, 580],
    [1100, 780],
  ]) {
    await command('POST', `/session/${session}/window/rect`, { width, height });
    const layout =
      await js(`const main=document.querySelector('.main-content'), picker=document.querySelector('.scan-volumes'), action=document.querySelector('.home-scan .button');
      const box=picker.getBoundingClientRect(), button=action.getBoundingClientRect(), viewport=main.getBoundingClientRect(), art=document.querySelector('.sculpture-home>.layer-scene').getBoundingClientRect(), control=document.querySelector('.home-control').getBoundingClientRect();
      return {overflow:main.scrollWidth>main.clientWidth+1, pickerVisible:box.width>0&&box.bottom<=viewport.bottom, actionVisible:button.top>=viewport.top&&button.bottom<=viewport.bottom, artworkClear:art.bottom<=control.top+1};`);
    layouts.push({ width, height, ...layout });
    await writeFile(
      `${output}/home-${width}.png`,
      Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
    );
    assert(
      !layout.overflow && layout.pickerVisible && layout.actionVisible && layout.artworkClear,
      JSON.stringify(layout),
    );
  }
  await click('//button[normalize-space(.)="开始扫描"]');
  const first = await until(async () => {
    const s = await snapshot();
    return s.status.id && s.status.state === 'ready' ? s : null;
  }, 180000);
  assert.deepEqual(first.status.scan_volumes, [c.path]);
  let checkedCandidates = 0;
  for (const rule of first.summary.rules.filter((r) => r.files > 0)) {
    const page = await ipc('list_candidates', {
      scope: 'user',
      session: first.status.id,
      rule: rule.rule_id,
      offset: 0,
      limit: 200,
      query: { status: 'candidate' },
    });
    for (const file of page.items) {
      assert(file.path.toUpperCase().startsWith(c.path.toUpperCase()));
      checkedCandidates++;
    }
  }
  await until(() =>
    js(
      'return !![...document.querySelectorAll("summary")].find(e=>e.textContent==="调整下次扫描磁盘");',
    ),
  );
  const purpose = await checkPurposeResults({ command, session, js, click, until, ipc, output });
  await click('//summary[normalize-space(.)="调整下次扫描磁盘"]');
  const other = volumes.find((l) => l.token !== c.token);
  if (other) {
    await click(`//input[@aria-label="扫描 ${other.path}"]`);
    assert.deepEqual(new Set(await readChecked()), new Set([c.path, other.path]));
    await click(`//input[@aria-label="扫描 ${c.path}"]`);
    assert.deepEqual(await readChecked(), [other.path]);
  }
  // Forged paths and empty selections are rejected before replacing the valid scan.
  for (const volumeTokens of [[], ['C:\\']]) {
    const rejected = await controlled('start_scan', {
      ruleIds: ['windows.user_temp'],
      volumeTokens,
    });
    assert(rejected.last_request.error);
    assert.equal((await snapshot()).status.id, first.status.id);
  }
  const scopes = [...(other ? [[other], [c, other]] : []), null];
  const receipts = [];
  for (const scope of scopes) {
    const reply = await controlled('start_scan', {
      ruleIds: ['windows.user_temp'],
      ...(scope ? { volumeTokens: scope.map((v) => v.token) } : {}),
    });
    assert(!reply.last_request.error, JSON.stringify(reply.last_request.error));
    const complete = await until(async () => {
      const s = await snapshot();
      return s.status.state === 'ready' ? s : null;
    }, 60000);
    assert.deepEqual(complete.status.scan_volumes, scope?.map((v) => v.path) ?? null);
    receipts.push({
      volumes: complete.status.scan_volumes,
      files: complete.summary.rules.reduce((n, r) => n + r.files, 0),
    });
  }
  assert.equal((await snapshot()).report, null);
  const evidence = {
    purpose,
    defaultC: true,
    perSelectedVolumeReadings: true,
    allReadings,
    allLayout,
    multiSelection: !!other,
    layouts,
    checkedCandidates,
    receipts,
    readonly: true,
    deleteExecuted: false,
    executableSha256: createHash('sha256')
      .update(await readFile(resolve('target/debug/dustaway.exe')))
      .digest('hex'),
  };
  await writeFile(`${output}/verification.json`, JSON.stringify(evidence, null, 2) + '\n');
  console.log('PASS: default C, single/multiple/all scope, forged requests rejected, no deletion');
} catch (error) {
  if (session)
    await writeFile(
      `${output}/failure.png`,
      Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
    ).catch(() => {});
  throw error;
} finally {
  if (session) await command('DELETE', `/session/${session}`).catch(() => {});
  driver.kill();
  await writeFile(`${output}/driver.log`, log);
}
