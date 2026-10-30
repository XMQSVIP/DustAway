import { createDesktopCommand } from './desktop-driver.mjs';
// Run against a DEBUG build with --features e2e-fixture. The release app cannot load this fixture.
import {
  mkdtemp,
  mkdir,
  writeFile,
  utimes,
  access,
  readFile,
  unlink,
  stat,
} from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { seedAnalysis, checkAnalysis } from './analysis-checks.mjs';
import { checkManualCleanup } from './manual-cleanup-checks.mjs';
import { checkPurposeResults } from './purpose-result-checks.mjs';

const version = JSON.parse(await readFile('package.json', 'utf8')).version;
const output = resolve(process.env.DUSTAWAY_E2E_OUTPUT ?? `artifacts/desktop-e2e-${version}`);
const cleanupOnly = process.argv.includes('--cleanup-only');
if (cleanupOnly) process.env.DUSTAWAY_ANALYSIS_SMALL = '1';
await mkdir(dirname(output), { recursive: true });
await mkdir(output); // A run must never overwrite an earlier evidence directory.

// Keep the large fixture on the project volume, away from a space-starved system disk.
const root = await mkdtemp(join(resolve('artifacts'), 'dustaway-e2e-'));
await mkdir(join(root, 'cache'));
await writeFile(join(root, '.dustaway-test-only'), 'fixture-only');
await writeFile(join(root, '.simulate-unreadable'), 'debug-only coverage fixture');
await writeFile(join(root, 'cache', 'old.tmp'), 'old disposable test cache');
await writeFile(join(root, 'cache', 'recent.tmp'), 'recent test file must stay');
await writeFile(join(root, 'personal.txt'), 'protected test document');
const analysisFixture = await seedAnalysis(root);
const old = new Date(Date.now() - 8 * 86400_000);
await utimes(join(root, 'cache', 'old.tmp'), old, old);
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
let driverLog = '';
driver.stdout.on('data', (d) => {
  driverLog += d;
});
driver.stderr.on('data', (d) => {
  driverLog += d;
});
const base = 'http://127.0.0.1:4444';
let session;
const delay = (ms) => new Promise((r) => setTimeout(r, ms));
const command = createDesktopCommand();
async function until(fn, timeout = 30_000) {
  const deadline = Date.now() + timeout;
  let error;
  while (Date.now() < deadline) {
    try {
      const result = await fn();
      if (result) return result;
    } catch (e) {
      error = e;
    }
    await delay(150);
  }
  throw error ?? new Error('Condition timed out');
}
async function element(xpath) {
  const value = await command('POST', `/session/${session}/element`, {
    using: 'xpath',
    value: xpath,
  });
  return value['element-6066-11e4-a52e-4f735466cecf'];
}
async function click(xpath) {
  // Inactive pages remain mounted so their tasks and navigation survive tab changes.
  // Choose an actually displayed match (the shared guides appear on both pages).
  const id = await until(async () => {
    const matches = await command('POST', `/session/${session}/elements`, {
      using: 'xpath',
      value: xpath,
    });
    for (const match of matches) {
      const candidate = match['element-6066-11e4-a52e-4f735466cecf'];
      // A visible control can still be disabled while a completed backend scan is
      // being reconciled by the UI. Native clicks on disabled buttons are silent no-ops.
      if (
        (await command('GET', `/session/${session}/element/${candidate}/displayed`)) &&
        (await command('GET', `/session/${session}/element/${candidate}/enabled`))
      )
        return candidate;
      // A delayed scan reconciliation may collapse the result group after the
      // previous poll. Reopen its visible heading, as a user would, then locate
      // the control again. Never reveal controls from inactive pages.
      await command('POST', `/session/${session}/execute/sync`, {
        script: `const heading=arguments[0].closest('.purpose-group')?.querySelector('.purpose-toggle');
          if(heading?.getClientRects().length && heading.getAttribute('aria-expanded')==='false') heading.click();`,
        args: [{ 'element-6066-11e4-a52e-4f735466cecf': candidate }],
      });
    }
    return false;
  });
  // Scroll the real content container before a native click; EdgeDriver can otherwise
  // choose a clipped row's center underneath the sibling action bar.
  await command('POST', `/session/${session}/execute/sync`, {
    script: 'arguments[0].scrollIntoView({block:"center",inline:"nearest"});',
    args: [{ 'element-6066-11e4-a52e-4f735466cecf': id }],
  });
  await command('POST', `/session/${session}/element/${id}/click`, {});
}
async function key(value) {
  await command('POST', `/session/${session}/actions`, {
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
}
async function tabTo(label, reverse = false) {
  for (let i = 0; i < 50; i++) {
    const active = await command('POST', `/session/${session}/execute/sync`, {
      script: `
      const e=document.activeElement, r=e.getBoundingClientRect(), clip=e.closest('.file-list,.draft-list,.analysis-list,.decision-list,.main-content');
      const c=clip?.getBoundingClientRect();
      return {label:e.getAttribute('aria-label')||e.textContent.trim(),visible:r.top>=0&&r.bottom<=innerHeight&&(!c||(r.top>=c.top-2&&r.bottom<=c.bottom+2))};`,
      args: [],
    });
    if (active.label === label) {
      assert(active.visible, 'Keyboard focus must be visible: ' + label);
      return;
    }
    if (reverse)
      await command('POST', `/session/${session}/actions`, {
        actions: [
          {
            type: 'key',
            id: 'reverse-tab',
            actions: [
              { type: 'keyDown', value: '\uE008' },
              { type: 'keyDown', value: '\uE004' },
              { type: 'keyUp', value: '\uE004' },
              { type: 'keyUp', value: '\uE008' },
            ],
          },
        ],
      });
    else await key('\uE004');
  }
  throw new Error('Keyboard target not reached: ' + label);
}
async function openWorkbench() {
  await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script:
        'return ["review","complete","attention"].includes(document.querySelector(".cleanup-decision")?.dataset.stage);',
      args: [],
    }),
  );
  await command('POST', `/session/${session}/execute/sync`, {
    script: `for(const d of document.querySelectorAll('details.result-section:not([open]), details.remaining-results:not([open])')) d.querySelector('summary')?.click();`,
    args: [],
  });
  await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script: `const collapsed=[...document.querySelectorAll('.purpose-toggle[aria-expanded="false"]')];
        for(const button of collapsed) button.click();
        return collapsed.length===0 && !!document.querySelector('.purpose-group .rule-row');`,
      args: [],
    }),
  );
}
async function finishResult() {
  const finish =
    '//section[contains(@class,"cleanup-decision")]//button[normalize-space(.)="完成" or normalize-space(.)="返回首页"]';
  const visible = await command('POST', `/session/${session}/execute/sync`, {
    script:
      'return [...document.querySelectorAll(".cleanup-decision button")].some(b=>["完成","返回首页"].includes(b.textContent.trim())&&b.getClientRects().length);',
    args: [],
  });
  if (visible) await click(finish);
}
async function scanAgain() {
  await finishResult();
  // Reload resets the production default to C:. This marked fixture exposes only
  // its project volume, so explicitly select that volume again through the UI.
  const selection = await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script: `const fieldset=document.querySelector('.scan-scope-summary .scan-volumes')
          || [...document.querySelectorAll('.scan-volumes')].find(f=>f.getClientRects().length);
        const input=fieldset?.querySelector('.scan-volume-option:first-child input');
        return input && !fieldset.disabled ? {checked:input.checked,collapsed:fieldset.closest('details')?.open===false} : false;`,
      args: [],
    }),
  );
  if (!selection.checked) {
    if (selection.collapsed) await click('//summary[normalize-space(.)="调整下次扫描磁盘"]');
    await click('//label[contains(normalize-space(.),"全部磁盘")]/input');
  }
  await click('//button[normalize-space(.)="开始扫描" or normalize-space(.)="重新全面扫描"]');
}
const completed = () =>
  command('POST', `/session/${session}/execute/sync`, {
    script: 'return !!document.querySelector(".cleanup-decision[data-stage=complete]");',
    args: [],
  });
const invoke = (name, args = {}) =>
  command('POST', `/session/${session}/execute/sync`, {
    script: 'return window.__TAURI_INTERNALS__.invoke(arguments[0],arguments[1]);',
    args: [name, args],
  });
async function openApp(name) {
  await openWorkbench();
  if (name === '微信') {
    const collapsed = await command('POST', `/session/${session}/execute/sync`, {
      script:
        'return document.querySelector(".weixin-heading .group-heading")?.getAttribute("aria-expanded")==="false";',
      args: [],
    });
    if (collapsed)
      await click(
        '//section[@aria-label="微信专项清理"]//button[contains(@class,"group-heading")]',
      );
  }
  const id = {
    Windows: 'windows',
    'Microsoft Edge': 'edge',
    'Google Chrome': 'chrome',
    'Mozilla Firefox': 'firefox',
    Brave: 'brave',
    Vivaldi: 'vivaldi',
    微信: 'weixin',
    '经典 QQ': 'qq',
    WPS: 'wps',
    飞书: 'feishu',
    钉钉: 'dingtalk',
    百度网盘: 'baidu_netdisk',
    夸克网盘: 'quark_netdisk',
    迅雷: 'thunder',
  }[name];
  const found = await command('POST', `/session/${session}/execute/sync`, {
    script: `return [...document.querySelectorAll('.rule-row[data-rule^="'+arguments[0]+'."]')].some(row=>row.getClientRects().length);`,
    args: [id],
  });
  assert(found, `Expected discovered results for ${name}`);
}
const decisionLayouts = [];
async function captureDecision(name) {
  // Dismiss an unrelated appearance/status receipt before evaluating the task stage.
  const noticeVisible = await command('POST', `/session/${session}/execute/sync`, {
    script: 'return !!document.querySelector(arguments[0])?.getClientRects().length;',
    args: ['button[aria-label="关闭提示"]'],
  });
  if (noticeVisible) await click('//button[@aria-label="关闭提示"]');
  await mkdir(`${output}/discovery-${version}`, { recursive: true });
  for (const [width, height] of [
    [1100, 780],
    [760, 580],
  ]) {
    await command('POST', `/session/${session}/window/rect`, { width, height });
    await command('POST', `/session/${session}/execute/sync`, {
      script: 'document.querySelector(".main-content").scrollTop = 0;',
      args: [],
    });
    await delay(150);
    const layout = await command('POST', `/session/${session}/execute/sync`, {
      script: `const panel=document.querySelector('.cleanup-decision'), dock=document.querySelector('.task-dock'), shown=dock.getClientRects().length>0, action=panel.querySelector('.button.primary'), heading=panel.querySelector('.desk-hero-copy h2,.home-copy h2'), statuses=panel.querySelector('.decision-outcomes');
        return {width:innerWidth,height:innerHeight,dpi:devicePixelRatio,overflow:document.documentElement.scrollWidth>innerWidth,headingBottom:heading.getBoundingClientRect().bottom,issuesBottom:statuses?.getBoundingClientRect().bottom ?? 0,dockTop:shown?dock.getBoundingClientRect().top:innerHeight,action:action?.textContent.trim(),actionBottom:action?.getBoundingClientRect().bottom,actionRight:action?.getBoundingClientRect().right};`,
      args: [],
    });
    assert(
      !layout.overflow &&
        layout.headingBottom <= layout.dockTop &&
        layout.issuesBottom <= layout.dockTop &&
        layout.actionBottom <= layout.height + 0.01 &&
        layout.actionRight <= layout.width + 0.01,
      JSON.stringify({ name, ...layout }),
    );
    decisionLayouts.push({ name, ...layout });
    await writeFile(
      `${output}/discovery-${version}/${name}-${width}.png`,
      Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
    );
  }
  await command('POST', `/session/${session}/window/rect`, { width: 1100, height: 780 });
}
async function source() {
  return command('GET', `/session/${session}/source`);
}
acceptance: try {
  await until(async () => (await fetch(base + '/status')).ok, 15_000);
  const created = await command('POST', '/session', {
    capabilities: {
      alwaysMatch: { 'tauri:options': { application: resolve('target/debug/dustaway.exe') } },
    },
  });
  session = created.sessionId;
  await until(async () => (await source()).includes('全部磁盘'));
  // Fail before scanning unless the BACKEND confirms its compiled-in fixture provider.
  // A normal debug build must never let this test reach a real cleanup confirmation.
  const runtime = await element('//div[contains(@class,"app-shell")]');
  assert.equal(
    await command('GET', `/session/${session}/element/${runtime}/attribute/data-runtime`),
    'fixture',
    'Refusing to test against a non-fixture application',
  );
  assert(!(await source()).includes('网页预览'));
  assert(!(await source()).includes('系统临时文件'));
  assert((await source()).includes('拂尘'));
  await writeFile(
    `${output}/home-diagnostic-${version}.png`,
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
  // This build exposes one isolated directory rather than real disks. Explicitly use
  // all fixture roots; ordinary builds independently default to the registered C drive.
  await click('//label[contains(normalize-space(.),"全部磁盘")]/input');
  await captureDecision('idle');
  assert.equal(
    await command('POST', `/session/${session}/execute/sync`, {
      script: 'return document.querySelectorAll(".scan-scope").length;',
      args: [],
    }),
    0,
  );
  await click('//button[normalize-space(.)="帮助与保留范围"]');
  assert.equal(
    await command('POST', `/session/${session}/execute/sync`, {
      script: 'return document.querySelectorAll(".application-guides").length;',
      args: [],
    }),
    0,
  );
  await command('POST', `/session/${session}/window/rect`, { width: 760, height: 580 });
  await click('//nav/button[contains(.,"设置")]');
  assert(
    await command('POST', `/session/${session}/execute/sync`, {
      script:
        'const main=document.querySelector("main");return main.scrollTop===0&&document.querySelector(".settings-book h1").getBoundingClientRect().top>=main.getBoundingClientRect().top;',
      args: [],
    }),
    'First visit to Settings must start at the top',
  );

  await writeFile(
    `${output}/discovery-${version}/settings.png`,
    Buffer.from(await command('GET', '/session/' + session + '/screenshot'), 'base64'),
  );
  await click('//button[contains(.,"深色")]');
  await click('//nav/button[normalize-space(.)="清理"]');
  await click('//nav/button[contains(.,"设置")]');
  await click('//button[contains(.,"浅色")]');
  await click('//nav/button[normalize-space(.)="清理"]');
  await command('POST', `/session/${session}/window/rect`, { width: 1100, height: 780 });
  const analysisEvidence = cleanupOnly
    ? { separateSuite: true }
    : await checkAnalysis({
        tabTo,
        key,
        command,
        session,
        click,
        until,
        source,
        root,
        output,
        fixture: analysisFixture,
      });
  // The analysis recovery check reloads the WebView. The synthetic disk is not C:,
  // so the new controller correctly starts with no selected disk.
  if (!cleanupOnly) await click('//label[contains(normalize-space(.),"全部磁盘")]/input');
  await click('//button[@aria-label="最大化"]');
  await until(() => element('//button[@aria-label="还原窗口"]'));
  await click('//button[@aria-label="还原窗口"]');
  await until(() => element('//button[@aria-label="最大化"]'));
  assert.equal(
    await command('POST', `/session/${session}/execute/sync`, {
      script: 'return document.querySelectorAll("input[type=range]").length;',
      args: [],
    }),
    0,
  );
  await click('//button[contains(.,"开始扫描")]');
  await until(async () => (await source()).includes('清理已选'));
  await captureDecision('review');
  const purposeResultEvidence = await checkPurposeResults({
    command,
    session,
    click,
    until,
    ipc: invoke,
    output,
    js: (script) => command('POST', `/session/${session}/execute/sync`, { script, args: [] }),
  });
  await writeFile(`${output}/purpose-results.json`, JSON.stringify(purposeResultEvidence, null, 2));
  await access(join(root, 'cache', 'old.tmp'));
  assert((await source()).includes('部分位置未读取'));
  await openWorkbench();
  const retentionSession = (await invoke('get_runtime_snapshot', { scope: 'user' })).status.id;
  const retainedPage = await invoke('list_scan_observations', {
    scope: 'user',
    session: retentionSession,
    rule: 'windows.user_temp',
    offset: 0,
    limit: 50,
  });
  assert(
    retainedPage.items.some((file) => file.path.endsWith('recent.tmp') && file.reason === 'recent'),
  );
  await click(
    '//article[@data-rule="windows.user_temp"]//summary[normalize-space(.)="查看已保留文件"]',
  );
  await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script: 'return !!document.querySelector(".scan-observations article");',
      args: [],
    }),
  );
  assert.equal(
    await command('POST', `/session/${session}/execute/sync`, {
      script: 'return document.querySelectorAll(".scan-observations input").length;',
      args: [],
    }),
    0,
  );
  await writeFile(
    `${output}/retained-files.png`,
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
  await click(
    '//article[@data-rule="windows.user_temp"]//summary[normalize-space(.)="查看已保留文件"]',
  );
  await click('//button[@aria-label="查看用户临时文件的文件"]');
  await until(async () => (await source()).includes('old.tmp'));
  // The shared transport resizes the native Tauri host and checks WebView alignment.
  await command('POST', `/session/${session}/window/rect`, { width: 760, height: 580 });
  const compact = await command('POST', `/session/${session}/execute/sync`, {
    script: `
    const d=document.querySelector('.files-dialog'), p=d.querySelector('.pagination'), list=d.querySelector('.file-list');
    return {height:innerHeight, width:innerWidth, dialog:d.getBoundingClientRect().bottom, pagination:p.getBoundingClientRect().bottom, listHeight:list.getBoundingClientRect().height, overflow:document.documentElement.scrollWidth>innerWidth};`,
    args: [],
  });
  assert(
    compact.pagination <= compact.dialog &&
      compact.dialog <= compact.height &&
      compact.listHeight >= 70 &&
      !compact.overflow,
    JSON.stringify(compact),
  );
  await writeFile(
    `${output}/desktop-minimum-${version}.png`,
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
  await command('POST', `/session/${session}/window/rect`, { width: 1100, height: 780 });
  const lightPng = await command('GET', `/session/${session}/screenshot`);
  await writeFile(
    `${output}/desktop-details-light-${version}.png`,
    Buffer.from(lightPng, 'base64'),
  );
  await click('//button[normalize-space(.)="取消本页选择"]');
  await click('//button[@aria-label="关闭文件明细"]');
  await until(async () => (await source()).includes('尚未选择可清理文件'));
  assert.equal(
    await command('POST', `/session/${session}/execute/sync`, {
      script:
        "return [...document.querySelectorAll('button')].some(b => b.textContent.trim().startsWith('清理已选') && !b.disabled)",
      args: [],
    }),
    false,
  );
  await click('//nav/button[contains(.,"设置")]');
  await click('//button[contains(.,"深色")]');
  await until(async () => (await source()).includes('扫描结果与选择保持不变'));
  await click('//nav/button[contains(.,"清理")]');
  await openWorkbench();
  await click('//button[@aria-label="查看用户临时文件的文件"]');
  await click('//button[normalize-space(.)="选择本页"]');
  const search = await element('//div[contains(@class,"files-dialog")]//input[@type="search"]');
  await command('POST', `/session/${session}/element/${search}/value`, { text: 'old.tmp' });
  await until(async () => (await source()).includes('old.tmp'));
  await click('//div[contains(@class,"files-dialog")]//summary[normalize-space(.)="完整路径"]');
  assert((await source()).includes(root));
  const detailPng = await command('GET', `/session/${session}/screenshot`);
  await writeFile(`${output}/desktop-details-${version}.png`, Buffer.from(detailPng, 'base64'));
  const layout = await command('POST', `/session/${session}/execute/sync`, {
    script: `const d=document.querySelector('.files-dialog');const p=d.querySelector('.pagination');return {dialog:d.getBoundingClientRect().bottom,pagination:p.getBoundingClientRect().bottom,height:innerHeight,overflow:document.documentElement.scrollWidth>innerWidth};`,
    args: [],
  });
  assert(
    layout.pagination <= layout.dialog && layout.dialog <= layout.height && !layout.overflow,
    'File pagination must remain visible',
  );
  await click('//button[@aria-label="关闭文件明细"]');
  await click('//button[contains(.,"清理已选")]');
  await until(async () => (await source()).includes('确认永久删除这些文件'));
  await writeFile(
    `${output}/discovery-${version}/confirmation.png`,
    Buffer.from(await command('GET', '/session/' + session + '/screenshot'), 'base64'),
  );
  assert((await source()).includes('扫描结果不完整；清理范围仅包含已通过安全校验并确认的候选'));
  assert((await source()).includes('用户临时文件'));
  await click('//*[@role="alertdialog"]//button[normalize-space(.)="取消"]');
  await access(join(root, 'cache', 'old.tmp'));
  await click('//button[contains(.,"清理已选")]');
  await click('//button[normalize-space(.)="永久删除"]');
  await until(completed);
  await assert.rejects(access(join(root, 'cache', 'old.tmp')));
  await captureDecision('completed');
  await access(join(root, 'cache', 'recent.tmp'));
  assert.equal(await readFile(join(root, 'personal.txt'), 'utf8'), 'protected test document');
  await click('//nav/button[contains(.,"历史")]');
  await until(async () => (await source()).includes('用户清理'));
  await click('//details[contains(@class,"history-receipt")]/summary');
  assert((await source()).includes('扫描结果不完整'));
  const png = await command('GET', `/session/${session}/screenshot`);
  await mkdir('artifacts', { recursive: true });
  await writeFile(`${output}/desktop-e2e-${version}.png`, Buffer.from(png, 'base64'));
  const appPath = resolve('target/debug/dustaway.exe').replaceAll("'", "''");
  const resources = await promisify(execFile)(
    'powershell.exe',
    [
      '-NoProfile',
      '-Command',
      `$all=Get-CimInstance Win32_Process; $app=$all|Where-Object {$_.ExecutablePath -eq '${appPath}'}; $ids=@($app.ProcessId); do { $children=@($all|Where-Object {$_.ParentProcessId -in $ids -and $_.ProcessId -notin $ids}); $ids+=@($children.ProcessId) } while($children.Count -gt 0); @($all|Where-Object {$_.ProcessId -in $ids})|ForEach-Object {$p=Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue; [pscustomobject]@{name=$_.Name;workingSetBytes=$p.WorkingSet64;privateBytes=$p.PrivateMemorySize64;cpuSeconds=$p.CPU}}|ConvertTo-Json`,
    ],
    { windowsHide: true },
  );
  await writeFile(`${output}/resource-snapshot-${version}.json`, resources.stdout);
  // New actions use the same guarded fixture provider; no real cache is ever deleted here.
  const keep = join(root, 'cache', '保留 空格.tmp');
  const disposable = join(root, 'cache', 'second.tmp');
  await writeFile(keep, 'keep by persistent exclusion');
  await writeFile(disposable, 'second disposable fixture');
  await utimes(keep, old, old);
  await utimes(disposable, old, old);
  await click('//nav/button[contains(.,"清理")]');
  await scanAgain();
  await until(async () => (await source()).includes('清理已选'));
  await openWorkbench();
  await click('//button[@aria-label="查看用户临时文件的文件"]');
  const searchAgain = await element(
    '//div[contains(@class,"files-dialog")]//input[@type="search"]',
  );
  await command('POST', `/session/${session}/element/${searchAgain}/value`, { text: '保留 空格' });
  await until(async () => (await source()).includes('共 1 项'));
  await tabTo('文件操作');
  await key('\uE007');
  await tabTo('在文件资源管理器中定位');
  await key('\uE007');
  await until(async () => {
    const button = await element('//button[normalize-space(.)="在文件资源管理器中定位"]');
    return command('GET', `/session/${session}/element/${button}/enabled`);
  });
  const revealError = await command('POST', `/session/${session}/execute/sync`, {
    script: "return document.querySelector('.files-dialog .error-text')?.textContent || '';",
    args: [],
  });
  assert.equal(revealError, '', 'Native reveal must succeed');
  const revealed = await until(async () => {
    const result = await promisify(execFile)(
      'powershell.exe',
      [
        '-NoProfile',
        '-ExecutionPolicy',
        'Bypass',
        '-File',
        resolve('scripts/check-reveal.ps1'),
        '-Path',
        keep,
      ],
      { windowsHide: true },
    );
    return result.stdout.includes('SELECTED');
  });
  assert(revealed, 'Explorer must select the exact fixture file');
  await tabTo('以后跳过此文件');
  await key('\uE007');
  await key('\uE00C');
  await tabTo('核对排除草稿');
  await key('\uE007');
  assert((await source()).includes('保留 空格.tmp'));
  await tabTo('保存排除并重新扫描');
  await key('\uE007');
  await until(async () => (await source()).includes('排除已保存'));
  await openWorkbench();
  await click('//button[@aria-label="查看用户临时文件的文件"]');
  await until(async () => (await source()).includes('second.tmp'));
  assert(!(await source()).includes('保留 空格.tmp'));
  await click('//button[@aria-label="关闭文件明细"]');
  await click('//button[starts-with(normalize-space(.),"清理已选")]');
  await click('//button[normalize-space(.)="永久删除"]');
  await until(completed);
  await assert.rejects(access(disposable));
  await access(keep);
  await access(join(root, 'cache', 'recent.tmp'));
  await access(join(root, 'personal.txt'));
  // Reload reconnects to the backend, clears UI choices and never repeats the completed plan.
  await command('POST', `/session/${session}/refresh`, {});
  await until(async () => (await source()).includes('已恢复上次清理记录'));
  await finishResult();
  await click('//label[contains(normalize-space(.),"全部磁盘")]/input');
  await access(keep);
  // Drop real IPC replies after backend acceptance. Only the guarded fixture is touched.
  const third = join(root, 'cache', 'third.tmp');
  const unselected = join(root, 'cache', 'unselected.tmp');
  for (const path of [third, unselected]) {
    await writeFile(path, '0.4 recovery fixture');
    await utimes(path, old, old);
  }
  await command('POST', `/session/${session}/execute/sync`, {
    script: `
    const original = window.fetch.bind(window);
    window.__fixtureOriginalFetch = original;
    window.__fixtureLost = { start_scan: 0, execute_cleanup: 0 };
    window.fetch = (url, options) => {
      const cmd = decodeURIComponent(new URL(url).pathname.slice(1));
      if (cmd in window.__fixtureLost && window.__fixtureLost[cmd]++ === 0)
        return original(url, options).then(() => new Promise(() => {}));
      return original(url, options);
    };`,
    args: [],
  });
  await scanAgain();
  await until(() => element('//button[starts-with(normalize-space(.),"清理已选")]'), 45_000);
  await openWorkbench();
  await click('//button[@aria-label="查看用户临时文件的文件"]');
  const excludeSearch = await element(
    '//div[contains(@class,"files-dialog")]//input[@type="search"]',
  );
  await command('POST', `/session/${session}/element/${excludeSearch}/value`, {
    text: 'unselected',
  });
  await until(async () => (await source()).includes('共 1 项'));
  await click('//button[normalize-space(.)="取消本页选择"]');
  await click('//button[@aria-label="关闭文件明细"]');
  await click('//button[starts-with(normalize-space(.),"清理已选")]');
  await click('//button[normalize-space(.)="永久删除"]');
  await until(completed, 45_000);
  await assert.rejects(access(third));
  await access(unselected);
  await access(keep);
  const lostCounts = await command('POST', `/session/${session}/execute/sync`, {
    script: 'return window.__fixtureLost',
    args: [],
  });
  assert.deepEqual(lostCounts, { start_scan: 1, execute_cleanup: 1 });
  assert((await source()).includes('扫描时占用'));
  await click('//details[contains(@class,"result-fold")]/summary');
  await tabTo('已删除 1 个文件');
  await key('\uE007');
  await until(async () => (await source()).includes('third.tmp'));
  await tabTo('关闭文件明细');
  await key('\uE007');
  await click(
    '//section[@aria-label="各类别与未选择文件"]//button[@aria-label="查看临时文件清理结果"]',
  );
  await click('//button[contains(.,"查看未纳入计划的 1 个文件")]');
  await until(async () => (await source()).includes('unselected.tmp'));
  assert(!(await source()).includes('third.tmp'));
  await command('POST', `/session/${session}/window/rect`, { width: 760, height: 580 });
  const resultLayout = await command('POST', `/session/${session}/execute/sync`, {
    script: `
    const d=document.querySelector('.files-dialog'),p=d.querySelector('.pagination');
    return {dialog:d.getBoundingClientRect().bottom,pagination:p.getBoundingClientRect().bottom,height:innerHeight,overflow:document.documentElement.scrollWidth>innerWidth};`,
    args: [],
  });
  assert(
    resultLayout.pagination <= resultLayout.dialog &&
      resultLayout.dialog <= resultLayout.height &&
      !resultLayout.overflow,
  );
  await tabTo('关闭文件明细');
  await key('\uE007');
  await writeFile(
    `${output}/desktop-results-${version}.png`,
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
  // Hold an invocation BEFORE the backend sees it. Recovery must fence it.
  await command('POST', `/session/${session}/execute/sync`, {
    script: `
    window.fetch = (url,options) => {
      const cmd = decodeURIComponent(new URL(url).pathname.slice(1));
      if (cmd==='start_scan') { window.__fixtureHeld=[url,options]; return new Promise(()=>{}); }
      return window.__fixtureOriginalFetch(url,options);
    };`,
    args: [],
  });
  await scanAgain();
  await until(async () => (await source()).includes('请求未受理'), 45_000);
  const lateCode = await command('POST', `/session/${session}/execute/sync`, {
    script: `
    return window.__fixtureOriginalFetch(...window.__fixtureHeld).then(r=>r.json()).then(e=>e.code || 'unexpected_success');`,
    args: [],
  });
  assert.equal(lateCode, 'request_expired');
  const finalSnapshot = await command('POST', `/session/${session}/execute/sync`, {
    script: "return window.__TAURI_INTERNALS__.invoke('get_runtime_snapshot',{scope:'user'});",
    args: [],
  });
  assert.equal(finalSnapshot.report.deleted, 1, 'Expired scan must retain the previous receipt');
  if (!cleanupOnly)
    assert(finalSnapshot.analysis.stale, 'Cleanup must mark the earlier analysis as stale');
  assert.equal(finalSnapshot.report.volume_space_changes.length, 1);
  assert.notEqual(finalSnapshot.report.volume_space_changes[0].before_bytes, null);
  assert.notEqual(finalSnapshot.report.volume_space_changes[0].after_bytes, null);
  assert.notEqual(finalSnapshot.report.volume_space_changes[0].change_bytes, null);
  await access(unselected);
  // Closing a running analysis must stop the backend and allow the process to exit.
  if (cleanupOnly) {
    await writeFile(
      `${output}/cleanup-desktop-${version}.json`,
      JSON.stringify(
        {
          passed: true,
          version,
          fixture: root,
          retainedReadOnly: retainedPage.items.length > 0,
          purposeResultEvidence,
          resultLayout,
          minimumWindow: compact,
          keyboardCandidateFlow: true,
          protectedFiles: true,
          lostReplies: lostCounts,
          lateUnacceptedRequest: lateCode,
          volumeSpaceChanges: finalSnapshot.report.volume_space_changes,
          executableSha256: createHash('sha256')
            .update(await readFile(resolve('target/debug/dustaway.exe')))
            .digest('hex'),
        },
        null,
        2,
      ),
    );
    console.log(
      `PASS: ${version} cleanup desktop, retained records, minimum window, keyboard, confirmation, isolated deletion and recovery`,
    );
    break acceptance;
  }
  // Eighteen application adapters use disposable fixture files; the real-application release gate stays closed.
  const appFixtures = [
    ['weixin.logs', 'old.xlog'],
    ['weixin.dumps', 'old.dmp'],
    ['qq.http', 'resource.bin'],
    ['qq.code', 'script.bin'],
    ['qq.gpu', 'shader.bin'],
    ['wps.logs', 'old.log'],
    ['feishu.http', 'resource.bin'],
    ['feishu.code', 'script.bin'],
    ['feishu.gpu', 'shader.bin'],
    ['dingtalk.http', 'resource.bin'],
    ['dingtalk.code', 'script.bin'],
    ...[
      'baidu_netdisk.http',
      'baidu_netdisk.code',
      'baidu_netdisk.gpu',
      'quark_netdisk.code',
      'quark_netdisk.gpu',
      'thunder.code',
      'thunder.gpu',
    ].map((id) => [id, 'fixture.bin']),
  ];
  const protectedAppFiles = [];
  for (const [id, name] of appFixtures) {
    const folder = join(root, 'applications', id);
    await mkdir(folder, { recursive: true });
    const path = join(folder, name);
    await writeFile(path, 'disposable application fixture');
    const age = new Date(Date.now() - 40 * 86400_000);
    await utimes(path, age, age);
    protectedAppFiles.push({
      path,
      bytes: await readFile(path),
      modified: (await stat(path)).mtimeMs,
    });
    if (['weixin.logs', 'weixin.dumps', 'wps.logs'].includes(id)) {
      const guard = join(folder, 'protected.ini');
      await writeFile(guard, 'must stay');
      protectedAppFiles.push({
        path: guard,
        bytes: await readFile(guard),
        modified: (await stat(guard)).mtimeMs,
      });
    }
  }
  await writeFile(join(root, '.application-running'), 'fixture only');
  await command('POST', `/session/${session}/refresh`, {});
  await until(async () => (await source()).includes('已恢复上次清理记录'));
  await finishResult();
  await click('//label[contains(normalize-space(.),"全部磁盘")]/input');
  for (const [id] of appFixtures)
    assert.equal(
      await command('POST', `/session/${session}/execute/sync`, {
        script: 'return !!document.getElementById("choose-"+arguments[0]);',
        args: [id],
      }),
      false,
    );
  await scanAgain();
  const readRuntime = () =>
    command('POST', `/session/${session}/execute/sync`, {
      script: "return window.__TAURI_INTERNALS__.invoke('get_runtime_snapshot',{scope:'user'});",
      args: [],
    });
  const scanBoot = await invoke('bootstrap');
  const expectedScannedRuleIds = scanBoot.rules
    .filter((rule) => !rule.requires_admin && !scanBoot.settings.excluded_rules.includes(rule.id))
    .map((rule) => rule.id)
    .sort();
  await until(async () => {
    const value = await readRuntime();
    return (
      value.status.state === 'ready' &&
      value.summary?.rules.length === expectedScannedRuleIds.length
    );
  });
  assert.deepEqual(
    (await readRuntime()).summary.rules.map((rule) => rule.rule_id).sort(),
    expectedScannedRuleIds,
  );
  await until(() => element('//*[@data-rule="weixin.logs"]'));
  // The result groups reset their expansion when the new scan reaches the UI.
  // Select through the visible group control, as in the current homepage flow.
  const tempSelected = await command('POST', `/session/${session}/execute/sync`, {
    script: 'return document.querySelector(\'input[aria-label="选择临时文件建议项"]\')?.checked;',
    args: [],
  });
  if (!tempSelected) await click('//input[@aria-label="选择临时文件建议项"]');
  await openApp('微信');
  const runningApps = await readRuntime();
  assert(
    runningApps.summary.rules
      .filter((r) => appFixtures.some(([id]) => id === r.rule_id))
      .every((r) => r.files === 1 && r.availability === 'application_running'),
  );
  const applicationDecision = await command('POST', `/session/${session}/execute/sync`, {
    script: `const panel=document.querySelector('.cleanup-decision'); return {text:panel.textContent,quitAction:[...panel.querySelectorAll('button')].some(b=>/重新检查.*状态/.test(b.textContent)),primary:document.querySelector('.action-bar .primary')?.textContent};`,
    args: [],
  });
  assert(/尚未开放清理|当前不支持清理/.test(applicationDecision.text));
  const cleanupCapabilities = await command('POST', `/session/${session}/execute/sync`, {
    script: "return window.__TAURI_INTERNALS__.invoke('get_application_support');",
    args: [],
  });
  const compiledCatalogue = JSON.parse(
    await readFile('scripts/application-catalog-v3.json', 'utf8'),
  );
  assert.deepEqual(
    cleanupCapabilities
      .flatMap((app) => app.rules)
      .map((rule) => rule.rule_id)
      .sort(),
    compiledCatalogue.rules.map((rule) => rule.rule_id).sort(),
  );
  for (const rule of cleanupCapabilities.flatMap((app) => app.rules)) {
    if (['dingtalk.http', 'dingtalk.code'].includes(rule.rule_id)) {
      assert.equal(rule.cleanup_validated, true);
      assert.deepEqual(rule.cleanup_versions, ['8.2.0.119']);
      assert.notEqual(rule.cleanup_block, null, 'The fixture host has no accepted running version');
    } else {
      assert.equal(rule.cleanup_validated, false);
      assert.deepEqual(rule.cleanup_versions, []);
      assert.equal(rule.cleanup_block, 'validation_pending');
    }
  }
  assert(applicationDecision.text.includes('已保留'));
  assert.equal(applicationDecision.quitAction, false);
  await captureDecision('readonly-running');
  for (const [name, appId] of [
    ['飞书', 'feishu'],
    ['钉钉', 'dingtalk'],
    ['百度网盘', 'baidu_netdisk'],
    ['夸克网盘', 'quark_netdisk'],
    ['迅雷', 'thunder'],
  ]) {
    await openApp(name);
    await captureDecision(
      {
        飞书: 'feishu',
        钉钉: 'dingtalk',
        百度网盘: 'baidu_netdisk',
        夸克网盘: 'quark_netdisk',
        迅雷: 'thunder',
      }[name] + '-readonly',
    );
    const state = await command('POST', `/session/${session}/execute/sync`, {
      script: `const row=document.querySelector('.retained-results .rule-row[data-rule^="'+arguments[0]+'."]'); return {locked:!!row?.querySelector('.rule-lock'), selectable:!!row?.querySelector('input[type="checkbox"]'), retained:!!row, reasonVisible:/尚未开放清理|尚未通过核验|暂不开放删除|需退出应用/.test(row?.textContent??'')};`,
      args: [appId],
    });
    assert(
      state.retained && state.locked && !state.selectable && state.reasonVisible,
      `${name} must remain blocked: ${JSON.stringify(state)}`,
    );
  }
  const forgedApplicationRequest = await command('POST', `/session/${session}/execute/sync`, {
    script: `return window.__TAURI_INTERNALS__.invoke('get_runtime_snapshot',{scope:'user'}).then(s=>window.__TAURI_INTERNALS__.invoke('prepare_cleanup',{preparationId:crypto.randomUUID(),requests:[{scope:'user',session:s.summary.session_id,selection:{rule_ids:['baidu_netdisk.http','quark_netdisk.code','thunder.gpu'],excluded_ids:[],included_ids:[],reviewed_rule_ids:['baidu_netdisk.http','quark_netdisk.code','thunder.gpu']}}],request:{runtime_id:s.control.runtime_id,expected_revision:s.control.revision,request_id:crypto.randomUUID()}}));`,
    args: [],
  });
  assert.equal(forgedApplicationRequest.last_request?.command, 'prepare_cleanup');
  assert.equal(forgedApplicationRequest.last_request?.value, null);
  assert.match(forgedApplicationRequest.last_request?.error?.message ?? '', /真实应用验收尚未完成/);
  // Reattach the page to reconcile the control revision consumed by the deliberate rejection.
  await command('POST', `/session/${session}/refresh`, {});
  await until(() => element('//*[@data-rule="weixin.logs"]'));
  await openApp('微信');
  await unlink(join(root, '.application-running'));
  await click('//*[@data-rule="weixin.logs"]//summary[normalize-space(.)="技术详情"]');
  await click('//button[normalize-space(.)="重新核对应用信息"]');
  await until(async () => (await source()).includes('应用状态已重新检查'));
  const pendingApps = await readRuntime();
  assert(
    pendingApps.summary.rules
      .filter((r) => appFixtures.some(([id]) => id === r.rule_id))
      .every((r) => r.availability === 'validation_pending' && r.blocked),
  );
  assert.equal(
    await command('POST', `/session/${session}/execute/sync`, {
      script:
        "return [...document.querySelectorAll('button')].some(b => b.textContent.trim().startsWith('清理已选') && !b.disabled)",
      args: [],
    }),
    false,
  );
  // Capability refresh is a separate read. While it is pending the UI must stay unknown/read-only.
  await until(() =>
    element(
      '//div[contains(@class,"application-support")]//p[contains(.,"关闭应用不会解除验收限制")]',
    ),
  );
  for (const file of protectedAppFiles) {
    assert.deepEqual(await readFile(file.path), file.bytes);
    assert.equal((await stat(file.path)).mtimeMs, file.modified);
  }
  await command('POST', `/session/${session}/window/rect`, { width: 760, height: 580 });
  await openApp('微信');

  const applicationLayout = await command('POST', `/session/${session}/execute/sync`, {
    script:
      'document.querySelector(arguments[0]).scrollIntoView({block:"start"}); return {overflow:document.documentElement.scrollWidth>innerWidth,barBottom:document.querySelector(".action-bar").getBoundingClientRect().bottom,height:innerHeight};',
    args: ['.weixin-cleanup [data-rule="weixin.logs"]'],
  });
  assert(
    !applicationLayout.overflow && applicationLayout.barBottom <= applicationLayout.height + 0.01,
  );
  await writeFile(
    `${output}/applications-gated-${version}.png`,
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
  // Change only a disposable candidate after scanning to verify partial-success guidance.
  const changedCandidate = join(root, 'cache', 'changed-after-scan.tmp');
  await writeFile(changedCandidate, 'original disposable fixture');
  await utimes(changedCandidate, old, old);
  await openApp('Windows');
  await scanAgain();
  await until(() => element('//button[starts-with(normalize-space(.),"清理已选")]'));
  await captureDecision('mixed-readonly');
  await writeFile(changedCandidate, 'changed after scanning and therefore must be preserved');
  await click('//button[starts-with(normalize-space(.),"清理已选")]');
  await click('//button[normalize-space(.)="永久删除"]');
  await until(() => element('.//section[@data-stage="attention"]'));
  const partialSnapshot = await readRuntime();
  assert.equal(partialSnapshot.report.deleted, 1);
  assert.equal(partialSnapshot.report.skipped, 1);
  assert.equal(partialSnapshot.report.failed, 0);
  assert.equal(partialSnapshot.report.unknown, 0);
  await access(changedCandidate);
  await captureDecision('partial');
  await click('//details[contains(@class,"result-fold")]/summary');
  await click('//*[@aria-label="按结果查看文件"]//button[contains(.,"保留")]');
  await until(async () => (await source()).includes('changed-after-scan.tmp'));
  const reviewDetail = await command('POST', `/session/${session}/execute/sync`, {
    script: `const d=document.querySelector('[role="dialog"]'); return {status:d.querySelector('select[aria-label="筛选结果"]')?.value ?? [...d.querySelectorAll('select')].at(-1).value, text:d.textContent, editable:!!d.querySelector('input[type="checkbox"]')};`,
    args: [],
  });
  assert.equal(reviewDetail.status, 'skipped');
  assert(reviewDetail.text.includes('文件或所在目录已变化'));
  assert(!reviewDetail.editable);
  await command('POST', `/session/${session}/window/rect`, { width: 760, height: 580 });
  await writeFile(
    `${output}/discovery-${version}/filtered-skipped-760.png`,
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
  await click('//button[@aria-label="关闭文件明细"]');
  const reviewFocus = await command('POST', `/session/${session}/execute/sync`, {
    script: 'return document.activeElement.textContent.trim()',
    args: [],
  });
  assert.match(reviewFocus, /保留/);
  await command('POST', `/session/${session}/window/rect`, { width: 1100, height: 780 });
  await click('//nav/button[contains(.,"设置")]');
  await click('//button[contains(.,"深色")]');
  await click('//button[normalize-space(.)="减少动效"]');
  await click('//nav/button[normalize-space(.)="清理"]');
  await captureDecision('partial-dark-reduced');
  await writeFile(
    `${output}/discovery-${version}/layouts.json`,
    JSON.stringify(decisionLayouts, null, 2),
  );
  const manualEvidence = await checkManualCleanup({ command, session, click, until, root, output });
  console.log(
    'PASS: homepage decisions, mixed readonly results, partial cleanup and manual cleanup',
  );
  const closeSnapshot = await readRuntime();
  const closeControl = closeSnapshot.control;
  await command('POST', `/session/${session}/execute/sync`, {
    script: "return window.__TAURI_INTERNALS__.invoke('start_analysis', arguments[0]);",
    args: [
      {
        token: finalSnapshot.analysis.location.token,
        request: {
          runtime_id: closeControl.runtime_id,
          expected_revision: closeControl.revision,
          request_id: crypto.randomUUID(),
        },
      },
    ],
  });
  await click('//button[@aria-label="关闭窗口"]').catch(() => {});
  const debugPath = resolve('target/debug/dustaway.exe').replaceAll("'", "''");
  await promisify(execFile)(
    'powershell.exe',
    [
      '-NoProfile',
      '-Command',
      `$deadline=(Get-Date).AddSeconds(15); do { $running=@(Get-Process dustaway -ErrorAction SilentlyContinue | Where-Object {$_.Path -eq '${debugPath}'}); if($running.Count -eq 0) {exit 0}; Start-Sleep -Milliseconds 100 } while((Get-Date) -lt $deadline); exit 1`,
    ],
    { windowsHide: true },
  );
  await writeFile(
    `${output}/desktop-e2e-${version}.json`,
    JSON.stringify(
      {
        version,
        developmentStage: 'manual-cleanup',
        manualEvidence,
        decisionLayouts,
        partialResultPrioritized: true,
        partialCoverageConfirmedAndPersisted: true,
        ordinaryRulesScanned: finalSnapshot.summary.rules.length,
        readonlyRulesHaveNoSelectionControls: true,
        domesticRulesScanned: cleanupCapabilities.flatMap((app) => app.rules).length,
        domesticReadonlyStatusRefresh: true,
        readonlyRunningDecision: applicationDecision,
        domesticReleaseGatePreserved: true,
        perRuleCleanupCapabilityVerified: true,
        applicationLayout,
        cleanupSelectionAbsentBeforeScan: true,
        purposeResultEvidence,
        analysis: analysisEvidence,
        executableSha256: createHash('sha256')
          .update(await readFile(resolve('target/debug/dustaway.exe')))
          .digest('hex')
          .toUpperCase(),
        fixture: root,
        passed: true,
        explorerSelected: true,
        persistentExclusion: true,
        protectedFiles: true,
        staleAnalysisAfterCleanup: true,
        closeDuringAnalysis: true,
        volumeSpaceChanges: finalSnapshot.report.volume_space_changes,
        reloadRecovery: true,
        lostScanAndCleanupReplies: lostCounts,
        lateUnacceptedRequest: lateCode,
        unplannedView: true,
        resultLayout,
        keyboardCandidateFlow: true,
        minimumWindow: compact,
      },
      null,
      2,
    ),
  );
  console.log(
    'PASS: real Tauri IPC, scan, preview, cancel confirmation, fixture deletion, preservation and history',
  );
  console.log(`Fixture retained for inspection: ${root}`);
} catch (error) {
  // Preserve the original failure even when the close-window check has already
  // disconnected WebDriver and gathering optional diagnostics also fails.
  console.error(error);
  if (session) {
    const html = await source().catch(() => null);
    if (html) await writeFile(`${output}/desktop-failure-${version}.html`, html).catch(() => {});
    const png = await command('GET', `/session/${session}/screenshot`).catch(() => null);
    if (png)
      await writeFile(`${output}/desktop-failure-${version}.png`, Buffer.from(png, 'base64'));
  }
  console.error(driverLog.slice(-6000));
  throw error;
} finally {
  if (session) await command('DELETE', `/session/${session}`).catch(() => {});
  driver.kill();
}
