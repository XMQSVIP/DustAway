import { createDesktopCommand } from './desktop-driver.mjs';
// Normal desktop build. No rule scan or application-data deletion is performed.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { resolve, join, dirname } from 'node:path';
import {
  mkdir,
  mkdtemp,
  writeFile,
  readFile,
  readdir,
  unlink,
  rmdir,
  realpath,
} from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { DatabaseSync } from 'node:sqlite';
import { checkManualCleanup } from './manual-cleanup-checks.mjs';

const theme = process.env.DUSTAWAY_TEST_THEME;
const executable = resolve(process.env.DUSTAWAY_TEST_EXECUTABLE || 'target/debug/dustaway.exe');
const width = Number(process.env.DUSTAWAY_TEST_WIDTH);
assert(!theme || ['light', 'dark'].includes(theme));
assert(!width || [760, 1100].includes(width));
const viewport = width ? [width, width === 760 ? 580 : 780] : undefined;
const output = resolve(
  process.env.DUSTAWAY_TEST_OUTPUT ||
    'artifacts/manual-standard-desktop' + (theme && width ? `/${theme}-${width}` : ''),
);
await mkdir(output, { recursive: true });
const driver = spawn(
  'tauri-driver',
  ['--native-driver', resolve('artifacts/webdriver/msedgedriver.exe')],
  {
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
    env: { ...process.env, DUSTAWAY_E2E_ROOT: 'deliberately-invalid-test-authorization' },
  },
);
let log = '',
  session,
  root,
  evidence;
driver.stdout.on('data', (s) => {
  log += s;
});
driver.stderr.on('data', (s) => {
  log += s;
});
const base = 'http://127.0.0.1:4444';
const command = createDesktopCommand();
async function until(fn, timeout = 30000) {
  const end = Date.now() + timeout;
  let last;
  while (Date.now() < end) {
    try {
      const result = await fn();
      if (result) return result;
    } catch (error) {
      last = error;
    }
    await new Promise((r) => setTimeout(r, 150));
  }
  throw last ?? new Error('Desktop fixture timed out');
}
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
const ipc = (name, args = {}) =>
  command('POST', `/session/${session}/execute/sync`, {
    script:
      'return window.__TAURI_INTERNALS__.invoke(arguments[0],arguments[1]).catch(error=>{throw new Error(typeof error === "string" ? error : JSON.stringify(error));});',
    args: [name, args],
  });
const snapshot = () => ipc('get_runtime_snapshot', { scope: 'user' });
async function controlled(name, args) {
  const c = (await snapshot()).control;
  const result = await ipc(name, {
    ...args,
    request: {
      runtime_id: c.runtime_id,
      expected_revision: c.revision,
      request_id: crypto.randomUUID(),
    },
  });
  assert(!result.last_request.error, JSON.stringify(result.last_request.error));
  return result.last_request.value;
}
async function analyze(token) {
  await controlled('start_analysis', { token });
  return until(async () => {
    const s = (await snapshot()).analysis;
    return s?.state === 'completed' || s?.state === 'partial' ? s : null;
  }, 120000);
}
try {
  await until(async () => (await fetch(base + '/status')).ok);
  session = (
    await command('POST', '/session', {
      capabilities: {
        alwaysMatch: { 'tauri:options': { application: executable } },
      },
    })
  ).sessionId;
  await until(async () => (await ipc('bootstrap')).version);
  if (viewport)
    await command('POST', `/session/${session}/window/rect`, {
      width: viewport[0],
      height: viewport[1],
    });
  const runtime = await command('POST', `/session/${session}/execute/sync`, {
    script: 'return document.querySelector(".app-shell")?.dataset.runtime;',
    args: [],
  });
  assert.equal(runtime, 'standard', 'Only an ordinary build may run this test');
  const location = (await ipc('get_analysis_locations')).find((l) => l.title === '桌面');
  assert(location?.supported !== false && location?.path);
  root = await mkdtemp(join(location.path, 'dustaway-standard-fixture-'));
  await writeFile(join(root, '.dustaway-test-only'), 'fixture-only');
  await mkdir(join(root, 'analysis'));
  // Only read the parent index to obtain opaque locations; no arbitrary-path IPC is used.
  const parent = await analyze(location.token);
  const own = (
    await ipc('list_analysis_children', {
      session: parent.id,
      node: '0',
      search: root.split(/[\\/]/).at(-1),
      offset: 0,
      descending: true,
    })
  ).items;
  assert.equal(own.length, 1);
  assert.equal(own[0].path.toLowerCase(), root.toLowerCase());
  const target = await ipc('refine_analysis_location', { session: parent.id, node: own[0].id });
  const scoped = await analyze(target.token);
  const folder = (
    await ipc('list_analysis_children', {
      session: scoped.id,
      node: '0',
      search: 'analysis',
      offset: 0,
      descending: true,
    })
  ).items;
  assert.equal(folder.length, 1);
  await ipc('refine_analysis_location', { session: scoped.id, node: folder[0].id });
  evidence = await checkManualCleanup({
    command,
    session,
    click,
    until,
    root,
    output,
    mixed: false,
    viewport,
    theme,
  });
  const apps = await ipc('get_application_support');
  for (const app of apps) {
    for (const rule of app.rules) {
      const accepted = ['dingtalk.http', 'dingtalk.code'].includes(rule.rule_id);
      assert.equal(rule.cleanup_validated, accepted);
      assert.deepEqual(rule.cleanup_versions, accepted ? ['8.2.0.119'] : []);
    }
  }
  evidence.ordinaryBuild = true;
  evidence.theme = theme;
  evidence.viewport = viewport;
  evidence.environmentCannotEnableFixture = true;
  evidence.applicationPermitsUnchanged = true;
  evidence.executableSha256 = createHash('sha256')
    .update(await readFile(executable))
    .digest('hex');
  await writeFile(join(output, 'standard-desktop.json'), JSON.stringify(evidence, null, 2) + '\n');
  console.log(
    'PASS: ordinary desktop, explicit fixture folder, one confirmation, app gates closed',
  );
} catch (error) {
  if (session) {
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
  // Remove only our own synthetic receipt after the app is closed; never reset user history.
  for (const ownedReport of [evidence?.report, evidence?.emptyReport].filter(Boolean)) {
    const db = new DatabaseSync(join(process.env.LOCALAPPDATA, 'DustAway', 'history.sqlite'));
    try {
      db.exec('BEGIN IMMEDIATE');
      const row = db.prepare('SELECT data FROM reports WHERE id=?').get(ownedReport.id);
      if (
        row &&
        JSON.parse(row.data).id === ownedReport.id &&
        JSON.parse(row.data).manual?.total === ownedReport.manual.total
      ) {
        db.prepare('DELETE FROM intents WHERE job=?').run(ownedReport.id);
        db.prepare('DELETE FROM reports WHERE id=?').run(ownedReport.id);
      }
      db.exec('COMMIT');
    } finally {
      db.close();
    }
  }
  // Keep failed fixtures for diagnosis. Successful teardown touches only exact created files.
  if (root && evidence?.ordinaryBuild) {
    const absolute = await realpath(root);
    assert(absolute.startsWith((await realpath(dirname(root))) + '\\'));
    assert.equal(await readFile(join(absolute, '.dustaway-test-only'), 'utf8'), 'fixture-only');
    const content = join(absolute, 'analysis', '自选清理夹具');
    for (const relative of ['保留我.txt', '确认后新增.txt', 'xwechat_files/chat.db']) {
      const target = await realpath(join(content, relative));
      assert(target.startsWith(absolute + '\\'));
      await unlink(target);
    }
    for (const directory of [join(content, 'xwechat_files'), content, join(absolute, 'analysis')]) {
      assert.equal((await readdir(directory)).length, 0);
      await rmdir(directory);
    }
    await unlink(join(absolute, '.dustaway-test-only'));
    assert.equal((await readdir(absolute)).length, 0);
    await rmdir(absolute);
  }
}
