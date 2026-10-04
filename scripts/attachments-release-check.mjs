// Read-only smoke check of the packaged production executable. Does not start a scan.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { createDesktopCommand } from './desktop-driver.mjs';
const application = resolve(process.env.DUSTAWAY_UI_APP ?? 'target/release/dustaway.exe');
const output = resolve(process.env.DUSTAWAY_UI_OUTPUT ?? 'artifacts/attachments-release-ui');
await mkdir(output, { recursive: true });
const driver = spawn(
  'tauri-driver',
  ['--native-driver', resolve('artifacts/webdriver/msedgedriver.exe')],
  { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] },
);
let session,
  log = '';
driver.stdout.on('data', (b) => {
  log += b;
});
driver.stderr.on('data', (b) => {
  log += b;
});
const command = createDesktopCommand();
const js = (script, ...args) =>
  command('POST', `/session/${session}/execute/sync`, { script, args });
async function until(fn) {
  const end = Date.now() + 30000;
  while (Date.now() < end) {
    try {
      const value = await fn();
      if (value) return value;
    } catch {}
    await new Promise((r) => setTimeout(r, 150));
  }
  throw Error('Release UI timeout');
}
async function click(xpath) {
  const items = await command('POST', `/session/${session}/elements`, {
    using: 'xpath',
    value: xpath,
  });
  assert.equal(items.length, 1);
  await js('arguments[0].scrollIntoView({block:"center"});', items[0]);
  await command(
    'POST',
    `/session/${session}/element/${items[0]['element-6066-11e4-a52e-4f735466cecf']}/click`,
    {},
  );
}
const ipc = (name, args = {}) =>
  js('return window.__TAURI_INTERNALS__.invoke(arguments[0],arguments[1]);', name, args);
try {
  await until(() => command('GET', '/status'));
  session = (
    await command('POST', '/session', {
      capabilities: { alwaysMatch: { 'tauri:options': { application } } },
    })
  ).sessionId;
  await until(() =>
    js('return document.querySelector(".app-shell")?.dataset.runtime === "standard";'),
  );
  const before = await ipc('get_runtime_snapshot', { scope: 'user' });
  await click('//button[normalize-space(.)="微信专项清理"]');
  assert.equal(await js('return document.querySelector("#weixin-cleanup-details").hidden;'), false);
  for (const [key, hidden] of [
    ['\uE00D', true],
    ['\uE007', false],
  ]) {
    await command('POST', `/session/${session}/actions`, {
      actions: [
        {
          type: 'key',
          id: 'keyboard',
          actions: [
            { type: 'keyDown', value: key },
            { type: 'keyUp', value: key },
          ],
        },
      ],
    });
    await until(() =>
      js(
        'return document.querySelector("#weixin-cleanup-details").hidden === arguments[0];',
        hidden,
      ),
    );
  }
  await click('//button[normalize-space(.)="整理微信聊天附件"]');
  await until(() => js('return !!document.querySelector(".attachment-dialog");'));
  assert.equal(await ipc('get_attachment_snapshot'), null, 'no automatic media scan');
  const observed = await js(
    'return {selected:document.querySelector(".attachment-footer strong").textContent,provider:document.querySelectorAll(".attachment-location select")[0].value,location:document.querySelectorAll(".attachment-location select")[1].value,scanDisabled:[...document.querySelectorAll(".attachment-location button")].find(b=>b.textContent.trim()==="开始扫描").disabled};',
  );
  assert.match(observed.selected, /已选 0 个/);
  assert.equal(observed.location, '');
  assert.equal(observed.provider, 'weixin');
  assert.equal(observed.scanDisabled, true);
  for (const provider of ['qq_classic', 'qqnt']) {
    const discovery = await ipc('get_attachment_locations', { provider });
    assert.equal(discovery.provider, provider);
    assert(discovery.locations.every((l) => l.provider === provider));
    assert(
      discovery.locations.every((l) => l.cleanup_block || /^\d+\.\d+\.\d+\.\d+$/.test(l.version)),
    );
    assert.equal(await ipc('get_attachment_snapshot'), null);
  }
  await command('POST', `/session/${session}/window/rect`, { width: 760, height: 580 });
  await writeFile(
    `${output}/entry-760.png`,
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
  await click('//button[@aria-label="关闭附件整理"]');
  const after = await ipc('get_runtime_snapshot', { scope: 'user' });
  assert.equal(after.status?.id, before.status?.id);
  assert.equal(after.report?.id, before.report?.id);
  await writeFile(
    `${output}/result.json`,
    JSON.stringify(
      {
        passed: true,
        application,
        runtime: 'standard',
        automaticScan: false,
        observed,
        existingReportUnchanged: true,
        weixinKeyboardDisclosure: true,
      },
      null,
      2,
    ),
  );
  console.log(
    'PASS: production startup, attachment entry, no implicit scan/selection, existing cleanup report unchanged',
  );
} finally {
  if (session) await command('DELETE', `/session/${session}`).catch(() => {});
  driver.kill();
  await writeFile(`${output}/driver.log`, log);
}
