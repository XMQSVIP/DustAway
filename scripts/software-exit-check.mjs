// Debug fixture only: exercise the real native close prompt with a human response.
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, writeFile, readFile } from 'node:fs/promises';
import { resolve, join, basename, dirname } from 'node:path';
import { spawn } from 'node:child_process';
import { createDesktopCommand, readNativeWindow } from './desktop-driver.mjs';

const recoveryRoot = process.argv[2] === '--recover-existing' ? process.argv[3] : undefined;
assert(process.argv.length === 2 || (process.argv.length === 4 && recoveryRoot));
const recoveryOnly = Boolean(recoveryRoot);
const output = resolve(
  recoveryOnly
    ? 'artifacts/software-exit-recovery-verification'
    : 'artifacts/software-exit-verification',
);
await mkdir(output, { recursive: true });
const root = recoveryOnly
  ? resolve(recoveryRoot)
  : await mkdtemp(join(resolve('artifacts'), 'dustaway-e2e-exit-'));
if (recoveryOnly) {
  assert.equal(dirname(root), resolve('artifacts'));
  assert(/^dustaway-e2e-exit-[A-Za-z0-9_-]+$/.test(basename(root)));
  assert.equal(await readFile(join(root, '.dustaway-test-only'), 'utf8'), 'fixture-only');
} else {
  await writeFile(join(root, '.dustaway-test-only'), 'fixture-only');
  await mkdir(join(root, 'cache'));
  await mkdir(join(root, 'analysis'));
  // An earlier run's human acknowledgement must never advance a new prompt.
  await writeFile(join(output, 'cancel-confirmed.txt'), '');
}
const application = resolve('target/debug/dustaway.exe');
const driver = spawn(
  'tauri-driver',
  ['--native-driver', resolve('artifacts/webdriver/msedgedriver.exe')],
  {
    env: {
      ...process.env,
      TEMP: resolve('artifacts'),
      TMP: resolve('artifacts'),
      DUSTAWAY_E2E_ROOT: root,
      DUSTAWAY_E2E_SOFTWARE_DELAY_MS: '180000',
    },
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
  },
);
let logs = '';
driver.stdout.on('data', (b) => {
  logs += b;
});
driver.stderr.on('data', (b) => {
  logs += b;
});
const command = createDesktopCommand();
const delay = (ms) => new Promise((done) => setTimeout(done, ms));
const status = (phase) =>
  writeFile(join(output, 'status.json'), JSON.stringify({ phase, time: new Date().toISOString() }));
let session;
const evaluate = (script, args = []) =>
  command('POST', `/session/${session}/execute/sync`, { script, args });
async function until(fn, timeoutMs = 30000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const value = await fn();
      if (value) return value;
    } catch {}
    await delay(150);
  }
  throw Error('Exit check timed out');
}
async function click(xpath) {
  const element = await until(async () => {
    const items = await command('POST', `/session/${session}/elements`, {
      using: 'xpath',
      value: xpath,
    });
    for (const item of items) {
      const id = item['element-6066-11e4-a52e-4f735466cecf'];
      if (
        (await command('GET', `/session/${session}/element/${id}/displayed`)) &&
        (await command('GET', `/session/${session}/element/${id}/enabled`))
      )
        return id;
    }
  });
  await command('POST', `/session/${session}/element/${element}/click`, {});
}
async function open() {
  const result = await command('POST', '/session', {
    capabilities: { alwaysMatch: { 'tauri:options': { application } } },
  });
  session = result.sessionId;
  await until(() =>
    evaluate('return document.querySelector(".app-shell")?.dataset.runtime==="fixture"'),
  );
}
try {
  await until(() => fetch('http://127.0.0.1:4444/status').then((r) => r.ok));
  await open();
  let task;
  if (recoveryOnly) {
    task = await evaluate('return window.__TAURI_INTERNALS__.invoke("get_uninstall_task")');
    assert.equal(task?.phase, 'unknown', 'Recovery check requires a pending fixture task');
  } else {
    await click('//nav/button[normalize-space(.)="软件卸载"]');
    await click('//button[contains(@class,"software-row")]');
    await click('//button[normalize-space(.)="卸载此软件"]');
    await click('//button[normalize-space(.)="确认卸载"]');
    task = await until(async () => {
      const value = await evaluate(
        'return window.__TAURI_INTERNALS__.invoke("get_uninstall_task")',
      );
      return value?.phase === 'uninstalling' && value;
    });
    await status('waiting_for_user_to_cancel_exit');
    await click('//button[@aria-label="关闭窗口"]');
    // WebView2 may return the click before the native modal is dismissed. Runtime
    // calls are intentionally awaited until the user returns to the webview.
    const responseDeadline = Date.now() + 120000;
    while (!(await readFile(join(output, 'cancel-confirmed.txt'), 'utf8').catch(() => ''))) {
      assert(
        Date.now() < responseDeadline,
        'Waiting for the human cancellation acknowledgement timed out',
      );
      await delay(250);
    }
    const continuing = await evaluate(
      'return window.__TAURI_INTERNALS__.invoke("get_uninstall_task")',
    );
    assert.equal(continuing.id, task.id);
    assert.equal(
      continuing.phase,
      'uninstalling',
      'Human response exceeded the bounded fixture delay',
    );
    await status('waiting_for_user_to_confirm_exit');
    await click('//button[@aria-label="关闭窗口"]').catch(() => {});
    await until(async () => !(await readNativeWindow(application)).running, 120000);
    await command('DELETE', `/session/${session}`).catch(() => {});
    session = undefined;
    await open();
  }
  const recovered = await evaluate(
    'return window.__TAURI_INTERNALS__.invoke("get_uninstall_task")',
  );
  assert.equal(recovered.id, task.id);
  assert.equal(recovered.phase, 'unknown');
  await delay(1500);
  const still = await evaluate('return window.__TAURI_INTERNALS__.invoke("get_uninstall_task")');
  assert.equal(still.id, task.id);
  assert.equal(still.phase, 'unknown', 'Restart must not resume the mock uninstall');
  const history = await evaluate(
    'return window.__TAURI_INTERNALS__.invoke("get_uninstall_history")',
  );
  assert.equal(history.length, 1);
  const verification = {
    provider: 'synthetic fixture',
    nativeUninstallExecuted: false,
    recoveryOnly,
    ...(!recoveryOnly && {
      cancelExitPreservesRunningTask: true,
      confirmedExitPersistsUnknown: true,
    }),
    relaunchPreservesTaskId: true,
    noAutomaticReplay: true,
    historyCount: history.length,
    taskId: task.id,
    recoveredMessage: recovered.message,
  };
  await writeFile(join(output, 'runtime-verification.json'), JSON.stringify(verification, null, 2));
  await click('//nav/button[normalize-space(.)="软件卸载"]');
  await until(() =>
    evaluate(
      'return document.querySelector(".software-task")?.textContent.includes(arguments[0])',
      [recovered.message],
    ),
  );
  assert.match(recovered.message, /已停止跟踪|跟踪已中断/);
  const actions = await evaluate(
    'return Array.from(document.querySelectorAll(".software-task button"), b => b.textContent.trim())',
  );
  assert(actions.includes('重新核对安装状态'));
  assert(!actions.includes('检查残留'), 'Unknown uninstall must not offer residual scanning');
  await writeFile(
    join(output, 'recovered.png'),
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
  await writeFile(
    join(output, 'verification.json'),
    JSON.stringify(
      {
        passed: true,
        ...verification,
        pendingTaskDisplayed: true,
        residualScanUnavailable: true,
      },
      null,
      2,
    ),
  );
  await status('passed');
} catch (error) {
  await status('failed');
  await writeFile(join(output, 'error.txt'), String(error.stack ?? error));
  console.error(logs.slice(-2000));
  throw error;
} finally {
  if (session) await command('DELETE', `/session/${session}`).catch(() => {});
  driver.kill(); // Only this test's driver and synthetic app, never a real uninstaller.
}
