// Native IPC / UI check using the debug-only in-memory provider. Never deletes host files.
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { spawn } from 'node:child_process';
import { createDesktopCommand } from './desktop-driver.mjs';

const output = resolve(process.env.DUSTAWAY_ATTACHMENT_UI_OUTPUT ?? 'artifacts/qq-attachments-ui');
await mkdir(output, { recursive: true });
const root = await mkdtemp(join(resolve('artifacts'), 'dustaway-e2e-attachments-'));
await writeFile(join(root, '.dustaway-test-only'), 'fixture-only');
await mkdir(join(root, 'cache'));
await mkdir(join(root, 'analysis'));
const driver = spawn(
  'tauri-driver',
  ['--native-driver', resolve('artifacts/webdriver/msedgedriver.exe')],
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
let log = '',
  session;
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
  let last;
  while (Date.now() < end) {
    try {
      const result = await fn();
      if (result) return result;
    } catch (e) {
      last = e;
    }
    await new Promise((r) => setTimeout(r, 150));
  }
  throw last ?? Error('Timed out');
}
async function click(xpath) {
  const el = await until(async () => {
    const items = await command('POST', `/session/${session}/elements`, {
      using: 'xpath',
      value: xpath,
    });
    for (const item of items) {
      const id = item['element-6066-11e4-a52e-4f735466cecf'];
      await js('arguments[0].scrollIntoView({block:"center"});', item);
      if (
        (await command('GET', `/session/${session}/element/${id}/displayed`)) &&
        (await command('GET', `/session/${session}/element/${id}/enabled`))
      )
        return id;
    }
  });
  await js('arguments[0].scrollIntoView({block:"center"});', {
    'element-6066-11e4-a52e-4f735466cecf': el,
  });
  await command('POST', `/session/${session}/element/${el}/click`, {});
}
const button = (text) => click(`//button[normalize-space(.)="${text}"]`);
const snapshot = () => js('return window.__TAURI_INTERNALS__.invoke("get_attachment_snapshot");');
const shots = [];
async function shot(name) {
  await writeFile(
    join(output, `${name}.png`),
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
  shots.push(name);
}
try {
  await until(() => command('GET', '/status'));
  const created = await command('POST', '/session', {
    capabilities: {
      alwaysMatch: { 'tauri:options': { application: resolve('target/debug/dustaway.exe') } },
    },
  });
  session = created.sessionId;
  await until(() =>
    js('return document.querySelector(".app-shell")?.dataset.runtime === "fixture";'),
  );
  await button('微信专项清理');
  assert.equal(await js('return document.querySelector("#weixin-cleanup-details").hidden;'), false);
  for (const theme of ['light', 'dark']) {
    await js('document.documentElement.dataset.theme=arguments[0];', theme);
    await command('POST', `/session/${session}/window/rect`, { width: 760, height: 580 });
    assert.equal(
      await js(
        'const e=document.querySelector(".weixin-cleanup"); return e.scrollWidth>e.clientWidth;',
      ),
      false,
    );
    await shot(`weixin-special-${theme}`);
  }
  await button('整理微信聊天附件');
  assert.equal(await snapshot(), null, 'opening does not scan');
  await until(() =>
    js('return document.querySelectorAll(".attachment-location select")[1]?.options.length===2;'),
  );
  await click('//select/option[contains(.,"附件界面测试")]');
  await button('开始扫描');
  await until(async () => (await snapshot())?.state === 'complete');
  await until(() => js('return document.querySelectorAll(".attachment-card").length===100;'));
  assert.equal(await js('return document.querySelectorAll(".attachment-category").length;'), 5);
  assert.match(
    await js('return document.querySelector(".attachment-filter-summary").textContent;'),
    /当前筛选/,
  );
  assert.equal(
    await js('return document.querySelectorAll(".attachment-card input:checked").length;'),
    0,
  );
  assert.equal(
    await js('return document.querySelectorAll(".attachment-card input:disabled").length;'),
    1,
  );
  await click('//button[@aria-label="预览测试附件-002.jpg"]');
  await until(() => js('return !!document.querySelector(".attachment-image-dialog");'));
  await button('关闭预览');
  assert.equal(
    await js('return document.querySelectorAll(".attachment-card input:checked").length;'),
    0,
  );
  await button('选择本页可选附件');
  assert.match(
    await js('return document.querySelector(".attachment-footer strong").textContent;'),
    /已选 99 个/,
  );
  await button('下一页');
  await until(() =>
    js(
      'return document.querySelector(".attachment-pagination").textContent.includes("第 2 页") && document.querySelectorAll(".attachment-card").length===100;',
    ),
  );
  await click('(//article[contains(@class,"attachment-card")]//input)[1]');
  assert.match(
    await js('return document.querySelector(".attachment-footer strong").textContent;'),
    /已选 100 个/,
  );
  await button('查看全部已选（100）');
  await until(() =>
    js('return document.querySelectorAll(".attachment-card input:checked").length===100;'),
  );
  for (const theme of ['light', 'dark']) {
    await js('document.documentElement.dataset.theme=arguments[0];', theme);
    for (const [width, height] of [
      [1100, 780],
      [760, 580],
    ]) {
      await command('POST', `/session/${session}/window/rect`, { width, height });
      await js('document.querySelector(".attachment-dialog").scrollTop=0;');
      const layout = await js(
        'const d=document.querySelector(".attachment-dialog"), f=document.querySelector(".attachment-footer"), b=f.querySelector("button").getBoundingClientRect(); return {width:innerWidth,height:innerHeight,overflow:d.scrollWidth>d.clientWidth,footer:b.toJSON()};',
      );
      assert.equal(layout.overflow, false);
      assert(layout.footer.top >= 0 && layout.footer.bottom <= height, JSON.stringify(layout));
      await shot(`${theme}-${width}`);
    }
  }
  await button('返回全部附件');
  await click('//div[@class="attachment-filters"]//option[@value="image"]');
  await until(() =>
    js('return document.querySelectorAll(".attachment-card.kind-image").length===100;'),
  );
  assert.match(
    await js('return document.querySelector(".attachment-footer strong").textContent;'),
    /已选 100 个/,
  );
  await command('POST', `/session/${session}/window/rect`, { width: 1100, height: 780 });
  await until(() => js('return document.querySelectorAll(".attachment-preview img").length>1;'));
  await shot('image-grid-filtered');
  await command('POST', `/session/${session}/window/rect`, { width: 760, height: 580 });
  await button('清理所选附件');
  await until(() => js('return !!document.querySelector(".attachment-confirm");'));
  assert.match(
    await js('return document.querySelector(".attachment-confirm").textContent;'),
    /100 个文件/,
  );
  await shot('confirmation-narrow');
  await button('返回选择');
  await until(() => js('return !document.querySelector(".attachment-confirm");'));
  assert.match(
    await js('return document.querySelector(".attachment-footer strong").textContent;'),
    /已选 100 个/,
  );
  for (const provider of ['qq_classic', 'qqnt']) {
    await click(`//select/option[@value="${provider}"]`);
    await until(async () => (await snapshot()) === null);
    assert.match(
      await js('return document.querySelector(".attachment-footer strong").textContent;'),
      /已选 0 个/,
    );
    await until(() =>
      js('return document.querySelectorAll(".attachment-location select")[1]?.options.length===2;'),
    );
    await click('//select/option[contains(.,"附件界面测试")]');
    await button('开始扫描');
    await until(async () => (await snapshot())?.state === 'complete');
    assert.equal((await snapshot()).location.provider, provider);
    await until(() => js('return document.querySelectorAll(".attachment-card").length===100;'));
    assert.equal(
      await js('return document.querySelectorAll(".attachment-card input:checked").length;'),
      0,
    );
    await shot(provider);
  }
  await click('//button[@aria-label="关闭附件整理"]');
  assert.equal(await snapshot(), null);
  await writeFile(
    join(output, 'result.json'),
    JSON.stringify(
      {
        passed: true,
        fixtureOnly: true,
        files: 205,
        pageSize: 100,
        selectedAcrossPages: 100,
        permanentDeletionExecuted: false,
        shots,
      },
      null,
      2,
    ),
  );
  console.log(
    'PASS: native paging, preview, cross-page selection, confirmation cancellation, session release and 4 responsive layouts',
  );
} catch (error) {
  if (session) {
    await shot('failure').catch(() => {});
    await writeFile(
      join(output, 'failure.txt'),
      String(await js('return document.body.innerText;')),
    ).catch(() => {});
  }
  throw error;
} finally {
  if (session) await command('DELETE', `/session/${session}`).catch(() => {});
  driver.kill();
  await writeFile(join(output, 'driver.log'), log);
}
