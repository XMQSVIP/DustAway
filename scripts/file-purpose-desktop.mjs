// Read-only native smoke test. Requires the debug e2e-fixture/custom-protocol build.
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { spawn } from 'node:child_process';
import { createDesktopCommand } from './desktop-driver.mjs';

const output = resolve('artifacts/file-purpose-desktop-20261025');
const application = resolve(process.env.DUSTAWAY_MODEL_TEST_APP ?? 'target/debug/dustaway.exe');
await mkdir(output, { recursive: true });
const root = await mkdtemp(join(resolve('artifacts'), 'dustaway-e2e-model-'));
await writeFile(join(root, '.dustaway-test-only'), 'fixture-only');
await mkdir(join(root, 'cache'));
const samples = [
  ['Unlisted/Cache/ml-entry.cache', 'model fixture cache'],
  ['Unlisted/opaque.qzx', 'model fixture unknown'],
  ['Unlisted/LocalCache/Cache/protected.cache', 'model fixture retained'],
];
for (const [path, content] of samples) {
  const file = join(root, 'analysis', path);
  await mkdir(resolve(file, '..'), { recursive: true });
  await writeFile(file, content);
}
const port = 20000 + Math.floor(Math.random() * 10000) * 2;
const base = `http://127.0.0.1:${port}`;
const command = createDesktopCommand({ base });
const driver = spawn(
  'tauri-driver',
  [
    '--port',
    String(port),
    '--native-port',
    String(port + 1),
    '--native-driver',
    resolve('artifacts/webdriver/msedgedriver.exe'),
  ],
  {
    env: {
      ...process.env,
      TEMP: resolve('artifacts'),
      TMP: resolve('artifacts'),
      DUSTAWAY_E2E_ROOT: root,
    },
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
let session;
async function until(fn, timeout = 30000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    try {
      const value = await fn();
      if (value) return value;
    } catch (error) {
      last = error;
    }
    await new Promise((done) => setTimeout(done, 150));
  }
  throw last ?? new Error('Timed out waiting for model desktop fixture');
}
const script = (source, ...args) =>
  command('POST', `/session/${session}/execute/sync`, { script: source, args });
const ipc = (name, args = {}) =>
  script('return window.__TAURI_INTERNALS__.invoke(arguments[0], arguments[1]);', name, args);
const click = (text) =>
  until(() =>
    script(
      `const b=[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===arguments[0]&&!b.disabled&&b.getClientRects().length); if(!b)return false; b.click(); return true;`,
      text,
    ),
  );
try {
  await until(async () => (await fetch(`${base}/status`)).ok, 15000);
  const created = await command('POST', '/session', {
    capabilities: {
      alwaysMatch: { 'tauri:options': { application } },
    },
  });
  session = created.sessionId;
  await until(() =>
    script('return document.querySelector(".app-shell")?.dataset.runtime === "fixture";'),
  );
  assert.equal(
    await script('return document.querySelector(".app-shell").dataset.runtime;'),
    'fixture',
  );
  const locations = await ipc('get_analysis_locations');
  assert.equal(locations.length, 1);
  assert.equal(resolve(locations[0].path).toLowerCase(), join(root, 'analysis').toLowerCase());
  await click('空间分析');
  await click('开始只读分析');
  const scan = await until(async () => {
    const value = (await ipc('get_runtime_snapshot', { scope: 'user' })).analysis;
    return value?.state === 'completed' && value;
  });
  assert.equal(scan.files, 3);
  const page = await ipc('list_analysis_files', { session: scan.id, search: '', offset: 0 });
  const predicted = page.items.find((item) => item.name === 'ml-entry.cache');
  const unknown = page.items.find((item) => item.name === 'opaque.qzx');
  const protectedFile = page.items.find((item) => item.name === 'protected.cache');
  assert.equal(predicted.advice.model_hint.purpose, 'cache');
  assert.equal(predicted.advice.kind, 'unknown');
  assert.deepEqual(predicted.rule_ids, []);
  assert.equal(predicted.manual.allowed, unknown.manual.allowed);
  assert.equal(unknown.advice.model_hint, undefined);
  assert.equal(protectedFile.advice.model_hint, undefined);
  await writeFile(
    join(output, 'backend-result.json'),
    `${JSON.stringify({ synthetic: true, model: predicted.advice.model_hint.model_id, modelClass: predicted.advice.model_hint.purpose, unknownAndProtectedAbstained: true, cleanupRuleIds: predicted.rule_ids, existingManualCapabilityPreserved: predicted.manual.allowed === unknown.manual.allowed, deletionExecuted: false }, null, 2)}\n`,
  );
  await until(async () => {
    const text = await script('return document.body.innerText;');
    if (text.includes('暂时无法确认任务状态')) await click('重新检查状态');
    return text.includes('分析完成');
  });
  await click('大文件');
  await until(() => script('return document.body.innerText.includes("可能是缓存 · 模型推测");'));
  await until(() =>
    script(
      'const b=document.querySelector(`[data-analysis-review="${arguments[0]}"]`); if(!b)return false; b.click(); return true;',
      predicted.id,
    ),
  );
  await until(() =>
    script(
      'const s=document.querySelector(".review-evidence > summary"); if(!s)return false; s.click(); return true;',
    ),
  );
  assert(
    await script(
      'return document.querySelector("[role=dialog]").innerText.includes("本地模型推测");',
    ),
  );
  assert.equal(
    await script(
      'return document.querySelectorAll("[role=dialog] input[type=checkbox]:checked").length;',
    ),
    0,
  );
  await writeFile(
    join(output, 'model-details.png'),
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
  for (const [path, content] of samples)
    assert.equal(await readFile(join(root, 'analysis', path), 'utf8'), content);
  const result = {
    synthetic: true,
    deletionExecuted: false,
    realMachineModelAccuracy: 'not_evaluated',
    files: 3,
    model: predicted.advice.model_hint.model_id,
    modelHintDisplayed: true,
    protectedAndUnknownAbstained: true,
    existingManualCapabilityPreserved: true,
    filesUnchanged: true,
  };
  await writeFile(join(output, 'result.json'), `${JSON.stringify(result, null, 2)}\n`);
  console.log(JSON.stringify(result, null, 2));
} catch (error) {
  if (session) {
    await writeFile(
      join(output, 'failure-ui.txt'),
      await script('return document.body.innerText;'),
    ).catch(() => {});
    await writeFile(
      join(output, 'failure.png'),
      Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
    ).catch(() => {});
  }
  throw error;
} finally {
  if (session) await command('DELETE', `/session/${session}`).catch(() => {});
  driver.kill();
  await writeFile(join(output, 'driver.log'), log);
}
