import { createDesktopCommand } from './desktop-driver.mjs';
// Release startup/read-only scan. Invalid scope/forged-plan probes must be rejected before work starts.
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { resolve, join } from 'node:path';
import { tmpdir } from 'node:os';
import { mkdtemp, mkdir, copyFile, rename, readdir, writeFile, readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
import { DatabaseSync } from 'node:sqlite';
import { existsSync } from 'node:fs';
const journalPath = join(process.env.LOCALAPPDATA, 'DustAway', 'history.sqlite');
function readScanPreference() {
  if (!existsSync(journalPath)) return null;
  const db = new DatabaseSync(journalPath, { readOnly: true });
  try {
    if (!db.prepare("SELECT 1 FROM sqlite_master WHERE name='preferences'").get()) return null;
    return db.prepare("SELECT data FROM preferences WHERE key='scan'").get()?.data ?? null;
  } finally {
    db.close();
  }
}
const originalPreference = readScanPreference();
let observedPreference;
function restoreOwnPreference() {
  if (observedPreference === undefined || observedPreference === originalPreference) return;
  const db = new DatabaseSync(journalPath);
  try {
    db.exec('BEGIN IMMEDIATE');
    const removed = db
      .prepare("DELETE FROM preferences WHERE key='scan' AND data=?")
      .run(observedPreference);
    if (removed.changes && originalPreference !== null)
      db.prepare("INSERT INTO preferences(key,data) VALUES('scan',?)").run(originalPreference);
    db.exec('COMMIT');
  } finally {
    db.close();
  }
}
const exec = promisify(execFile);
const root = await mkdtemp(join(tmpdir(), 'fuchen-portable-smoke-'));
await mkdir('artifacts/discovery-0.17.2', { recursive: true });
const directory = join(root, '中文 程序目录');
const moved = join(root, '移动 后');
await mkdir(directory);
await mkdir(moved);
let executable = join(directory, '拂尘.exe');
await copyFile(resolve('target/release/dustaway.exe'), executable);
const acl = (restore = false) =>
  exec(
    'powershell.exe',
    [
      '-NoProfile',
      '-ExecutionPolicy',
      'Bypass',
      '-File',
      resolve('scripts/readonly-fixture.ps1'),
      '-Directory',
      directory,
      ...(restore ? ['-Restore'] : []),
    ],
    { windowsHide: true },
  );
const driver = spawn(
  'tauri-driver',
  ['--native-driver', resolve('artifacts/webdriver/msedgedriver.exe')],
  { windowsHide: true, stdio: 'ignore' },
);
let session;
let windowHandle;
let protectedDirectory = false;
const pause = (ms) => new Promise((r) => setTimeout(r, ms));
const request = createDesktopCommand();
async function until(fn, timeout = 120000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    try {
      const value = await fn();
      if (value) return value;
    } catch (e) {
      last = e;
    }
    await pause(250);
  }
  throw last ?? Error('Timed out');
}
async function find(xpath) {
  return (await request('POST', `/session/${session}/element`, { using: 'xpath', value: xpath }))[
    'element-6066-11e4-a52e-4f735466cecf'
  ];
}
async function evaluate(script, ...args) {
  return request('POST', `/session/${session}/execute/sync`, { script, args });
}
async function clickLabel(label) {
  const element = await find(`//button[@aria-label="${label}"]`);
  return request('POST', `/session/${session}/element/${element}/click`, {});
}
async function clickText(label) {
  const element = await until(async () => {
    const matches = await request('POST', `/session/${session}/elements`, {
      using: 'xpath',
      value: `//button[normalize-space(.)="${label}"]`,
    });
    for (const match of matches) {
      const id = match['element-6066-11e4-a52e-4f735466cecf'];
      if (
        (await request('GET', `/session/${session}/element/${id}/displayed`)) &&
        (await request('GET', `/session/${session}/element/${id}/enabled`))
      )
        return id;
    }
  });
  await request('POST', `/session/${session}/execute/sync`, {
    script: 'arguments[0].scrollIntoView({block:"center"});',
    args: [{ 'element-6066-11e4-a52e-4f735466cecf': element }],
  });
  await request('POST', `/session/${session}/element/${element}/click`, {});
}
async function nativeState() {
  const result = await exec(
    'powershell.exe',
    [
      '-NoProfile',
      '-ExecutionPolicy',
      'Bypass',
      '-File',
      resolve('scripts/window-state.ps1'),
      '-Executable',
      executable,
      ...(windowHandle ? ['-WindowHandle', windowHandle] : []),
    ],
    { windowsHide: true },
  );
  const state = JSON.parse(result.stdout);
  if (state.handle) windowHandle = state.handle;
  return state;
}
async function launch() {
  windowHandle = undefined;
  session = (
    await request('POST', '/session', {
      capabilities: { alwaysMatch: { 'tauri:options': { application: executable } } },
    })
  ).sessionId;
  await until(() => find('//button[contains(. ,"开始扫描")]'));
  assert.equal(
    await evaluate("return document.querySelector('.app-shell').dataset.runtime"),
    'standard',
  );
  const boot = await evaluate("return window.__TAURI_INTERNALS__.invoke('bootstrap')");
  assert.equal(boot.version, JSON.parse(await readFile('package.json', 'utf8')).version);
  assert.equal(boot.capabilities.system_cleanup, false);
  assert.equal(boot.rules.length, 23);
  assert(boot.rules.every((r) => !r.requires_admin));
  const support = await evaluate(
    "return window.__TAURI_INTERNALS__.invoke('get_application_support')",
  );
  assert.equal(support.length, 3);
  assert(support.every((a) => a.identity_verified && a.scan_supported && !a.cleanup_validated));
  assert.deepEqual(
    support.flatMap((a) => a.rules.map((r) => r.rule_id)).sort(),
    boot.rules
      .filter((r) => r.app_id)
      .map((r) => r.id)
      .sort(),
  );
  assert(support.every((a) => a.rules.every((r) => r.cleanup_validated === false)));
  assert.equal(boot.rules.filter((r) => r.app_id).length, 6);
  assert(boot.rules.filter((r) => r.app_id).every((r) => !r.default_selected));
  return boot;
}
try {
  await until(async () => (await fetch('http://127.0.0.1:4444/status')).ok);
  await acl();
  protectedDirectory = true;
  await assert.rejects(
    writeFile(join(directory, 'must-not-write.txt'), 'probe'),
    (e) => e.code === 'EACCES' || e.code === 'EPERM',
  );
  const first = await launch();
  await pause(1000);
  const measuredPath = executable.replaceAll("'", "''");
  const resources = await exec(
    'powershell.exe',
    [
      '-NoProfile',
      '-Command',
      `[Console]::OutputEncoding=[System.Text.UTF8Encoding]::new(); $all=Get-CimInstance Win32_Process; $apps=$all|Where-Object {$_.ExecutablePath -eq '${measuredPath}'}; $ids=@($apps.ProcessId); do { $children=@($all|Where-Object {$_.ParentProcessId -in $ids -and $_.ProcessId -notin $ids}); $ids+=@($children.ProcessId) } while($children.Count -gt 0); @($all|Where-Object {$_.ProcessId -in $ids})|ForEach-Object {$p=Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue; [pscustomobject]@{name=$_.Name;workingSetBytes=$p.WorkingSet64;privateBytes=$p.PrivateMemorySize64;cpuSeconds=$p.CPU}}|ConvertTo-Json`,
    ],
    { windowsHide: true },
  );
  await writeFile(
    'artifacts/discovery-0.17.2/release-idle-resources.json',
    JSON.stringify(
      {
        scope: 'Release startup after render, no scan; point-in-time costs, not peak memory',
        processes: JSON.parse(resources.stdout),
      },
      null,
      2,
    ),
  );
  const frontendAssets = await evaluate(
    'return [...document.querySelectorAll("script[src],link[rel=stylesheet]")].map(e=>new URL(e.src||e.href).pathname.split("/").pop()).sort()',
  );
  assert.deepEqual(
    frontendAssets,
    (await readdir(resolve('dist/assets'))).filter((p) => /\.(js|css)$/.test(p)).sort(),
    'release must embed the current frontend build',
  );
  await until(async () =>
    (
      await evaluate(
        'return document.querySelector(".cleanup-page .compact-space-status").textContent',
      )
    ).includes('当前用户可用空间'),
  );
  const homeRefresh = await find(
    '//section[contains(@class,"cleanup-page")]//button[normalize-space(.)="刷新磁盘空间"]',
  );
  await request('POST', `/session/${session}/element/${homeRefresh}/click`, {});
  await until(
    async () =>
      !(await evaluate(
        'return document.querySelector(".cleanup-page .compact-space-status").getAttribute("aria-busy")==="true"',
      )),
  );
  assert.equal(
    await evaluate(
      'return document.querySelector(".cleanup-decision").getClientRects().length > 0',
    ),
    true,
  );
  // Check the embedded CSS with a real pointer hover. Temporary theme attributes do not save settings.
  const originalTheme = await evaluate(
    'return document.documentElement.getAttribute("data-theme")',
  );
  const hoverContrast = [];
  for (const theme of ['light', 'dark']) {
    await evaluate(`document.documentElement.setAttribute('data-theme', '${theme}')`);
    for (const [width, height] of [
      [1100, 780],
      [760, 580],
    ]) {
      await request('POST', `/session/${session}/window/rect`, { width, height });
      const primary = await find(
        '//section[@aria-label="清理决策总览"]//button[contains(@class,"primary")]',
      );
      await request('POST', `/session/${session}/actions`, {
        actions: [
          {
            type: 'pointer',
            id: 'contrast',
            parameters: { pointerType: 'mouse' },
            actions: [
              {
                type: 'pointerMove',
                duration: 0,
                origin: { 'element-6066-11e4-a52e-4f735466cecf': primary },
                x: 0,
                y: 0,
              },
            ],
          },
        ],
      });
      await pause(220);
      const measured = await evaluate(`
        const button = document.querySelector('.idle-scan-action .button.primary');
        const style = getComputedStyle(button);
        const canvas = document.createElement('canvas'); canvas.width = canvas.height = 1;
        const c = canvas.getContext('2d');
        function luminance(color) {
          c.fillStyle = color; c.fillRect(0, 0, 1, 1);
          const rgb = [...c.getImageData(0, 0, 1, 1).data].slice(0, 3).map(v => {
            const s = v / 255; return s <= .04045 ? s / 12.92 : ((s + .055) / 1.055) ** 2.4;
          });
          return rgb[0] * .2126 + rgb[1] * .7152 + rgb[2] * .0722;
        }
        const a = luminance(style.color), b = luminance(style.backgroundColor);
        document.querySelector('main').scrollTop = 0;
        return {ratio: (Math.max(a, b) + .05) / (Math.min(a, b) + .05), hover:button.matches(':hover'), overflow:document.documentElement.scrollWidth > innerWidth};
      `);
      assert(
        measured.hover && measured.ratio >= 4.5 && !measured.overflow,
        JSON.stringify(measured),
      );
      hoverContrast.push({ theme, width, height, ...measured });
      await writeFile(
        `artifacts/discovery-0.17.2/release-${theme}-${width}-hover.png`,
        Buffer.from(await request('GET', `/session/${session}/screenshot`), 'base64'),
      );
    }
  }
  await evaluate(
    originalTheme === null
      ? 'document.documentElement.removeAttribute("data-theme")'
      : `document.documentElement.setAttribute('data-theme', ${JSON.stringify(originalTheme)})`,
  );
  await request('POST', `/session/${session}/window/rect`, { width: 1100, height: 780 });
  const analysisLocations = await evaluate(
    "return window.__TAURI_INTERNALS__.invoke('get_analysis_locations')",
  );
  assert(analysisLocations.some((l) => l.title === '当前用户文件夹' && l.token && l.path));
  const volumes = analysisLocations.filter((l) => l.kind === 'volume');
  assert(volumes.length > 0 && volumes.some((l) => l.is_system && l.supported));
  assert(
    volumes.every((l) => typeof l.supported === 'boolean' && (l.supported || l.unavailable_reason)),
  );
  for (const volume of volumes.filter((l) => l.supported)) {
    const reading = await evaluate(
      `return window.__TAURI_INTERNALS__.invoke('get_analysis_context', {locationToken:${JSON.stringify(volume.token)}})`,
    );
    assert(reading.volume_key && reading.available_bytes !== null && !reading.error);
  }
  const analysisTab = await find('//nav/button[normalize-space(.)="空间分析"]');
  await request('POST', `/session/${session}/element/${analysisTab}/click`, {});
  await until(
    async () =>
      (await evaluate('return document.querySelector(".analysis-location select").value')) ===
      volumes.find((l) => l.is_system).token,
  );
  await until(async () =>
    (await evaluate('return document.querySelector(".space-status").textContent')).includes(
      '当前用户可用空间',
    ),
  );
  await pause(550);
  const display = await evaluate('return {width:innerWidth,height:innerHeight,devicePixelRatio}');
  await writeFile(
    'artifacts/discovery-0.17.2/release-volume-entry.png',
    Buffer.from(await request('GET', `/session/${session}/screenshot`), 'base64'),
  );
  const cleanTab = await find('//nav/button[normalize-space(.)="清理"]');
  await request('POST', `/session/${session}/element/${cleanTab}/click`, {});
  const diskContext = await evaluate(`return (async () => {
    const locations = await window.__TAURI_INTERNALS__.invoke('get_analysis_locations');
    return window.__TAURI_INTERNALS__.invoke('get_analysis_context', {locationToken: locations.find(l=>l.title==='当前用户文件夹').token});
  })()`);
  assert(diskContext.sampled_at > 0 && diskContext.location_token);
  assert(
    diskContext.error
      ? diskContext.available_bytes === null
      : BigInt(diskContext.available_bytes) >= 0n,
  );
  assert.equal(
    (
      await evaluate(
        "return window.__TAURI_INTERNALS__.invoke('get_runtime_snapshot',{scope:'user'})",
      )
    ).analysis,
    null,
  );
  // Explicit, bounded-duration metadata-only probes of system and one other root.
  // No candidate or cleanup request is created from these analysis sessions.
  const rootProbes = [];
  const probeVolumes = [
    volumes.find((l) => l.is_system && l.supported),
    volumes.find((l) => !l.is_system && l.supported),
  ].filter(Boolean);
  for (const volume of probeVolumes) {
    const id = await evaluate(`return (async () => {
      const invoke=window.__TAURI_INTERNALS__.invoke;
      const s=await invoke('get_runtime_snapshot',{scope:'user'});
      const receipt=await invoke('start_analysis',{token:${JSON.stringify(volume.token)},request:{runtime_id:s.control.runtime_id,expected_revision:s.control.revision,request_id:crypto.randomUUID()}});
      if(receipt.last_request.error) throw new Error(receipt.last_request.error.message);
      return receipt.last_request.value;
    })()`);
    await until(async () => {
      const a = await evaluate(
        "return window.__TAURI_INTERNALS__.invoke('get_runtime_snapshot',{scope:'user'}).then(s=>s.analysis)",
      );
      return a?.id === id && (a.entries > 1 || a.state !== 'scanning');
    });
    await evaluate(`return (async () => {
      const invoke=window.__TAURI_INTERNALS__.invoke;
      const s=await invoke('get_runtime_snapshot',{scope:'user'});
      if(s.analysis.state==='scanning') {
        const receipt=await invoke('cancel_analysis',{session:${JSON.stringify(id)},request:{runtime_id:s.control.runtime_id,expected_revision:s.control.revision,request_id:crypto.randomUUID()}});
        if(receipt.last_request.error) throw new Error(receipt.last_request.error.message);
      }
    })()`);
    const result = await until(async () => {
      const a = await evaluate(
        "return window.__TAURI_INTERNALS__.invoke('get_runtime_snapshot',{scope:'user'}).then(s=>s.analysis)",
      );
      return a?.id === id && a.state !== 'scanning' && a;
    });
    assert(result.entries > 1 && result.state !== 'failed');
    rootProbes.push({
      volume: volume.path,
      system: volume.is_system,
      state: result.state,
      entries: result.entries,
      checked: result.checked,
      issues: result.issues,
    });
  }
  await request('POST', `/session/${session}/refresh`, {});
  await until(() => find('//button[contains(. ,"开始扫描")]'));
  // Exercise the shipped analysis review using already-read metadata only.
  const reviewAnalysisTab = await find('//nav/button[normalize-space(.)="空间分析"]');
  await request('POST', `/session/${session}/element/${reviewAnalysisTab}/click`, {});
  await until(
    async () =>
      await evaluate(
        "return !!document.querySelector('.previous-analysis') || !!document.querySelector('[data-analysis-review]')",
      ),
  );
  if (await evaluate("return !!document.querySelector('.previous-analysis')")) {
    const previous = await find('//button[normalize-space(.)="查看上次结果"]');
    await request('POST', `/session/${session}/element/${previous}/click`, {});
  }
  const discoverySession = await evaluate(
    "return window.__TAURI_INTERNALS__.invoke('get_runtime_snapshot',{scope:'user'}).then(s=>s.analysis.id)",
  );
  const discoveryRelease = await evaluate(
    `return Promise.all(['directory','file'].map(kind=>window.__TAURI_INTERNALS__.invoke('list_analysis_discovery',{session:arguments[0],query:{kind,category:'all',min_bytes:'0',search:'',include_nested:false,incomplete_only:false,offset:0}}).then(p=>({kind,total:p.total,items:p.items.length,categoriesPresent:p.items.every(e=>!e.advice?.recognition||!!e.advice.recognition.category)}))))`,
    discoverySession,
  );
  assert(discoveryRelease.every((p) => p.items <= 200 && p.categoriesPresent));
  const discoveryButton = await find('//button[normalize-space(.)="按用途查找"]');
  await request('POST', `/session/${session}/element/${discoveryButton}/click`, {});
  await until(
    async () =>
      await evaluate(
        "return !!document.querySelector('.discovery-filters') && document.querySelector('.analysis-list').getAttribute('aria-busy')==='false'",
      ),
  );
  await writeFile(
    'artifacts/discovery-0.17.2/release-discovery.png',
    Buffer.from(await request('GET', `/session/${session}/screenshot`), 'base64'),
  );
  const directoriesButton = await find('//button[normalize-space(.)="目录排行"]');
  await request('POST', `/session/${session}/element/${directoriesButton}/click`, {});
  assert.equal(
    await evaluate(
      "return window.__TAURI_INTERNALS__.invoke('get_runtime_snapshot',{scope:'user'}).then(s=>s.analysis.id)",
    ),
    discoverySession,
  );
  const reviewTrigger = await until(() => find('(//button[@data-analysis-review])[1]'));
  const reviewSession = await evaluate(
    "return window.__TAURI_INTERNALS__.invoke('get_runtime_snapshot',{scope:'user'}).then(s=>s.analysis.id)",
  );
  await request('POST', `/session/${session}/element/${reviewTrigger}/click`, {});
  await until(() => find('//*[@role="dialog"]//button[normalize-space(.)="返回列表"]'));
  assert.equal(
    await evaluate(
      "return window.__TAURI_INTERNALS__.invoke('get_runtime_snapshot',{scope:'user'}).then(s=>s.analysis.id)",
    ),
    reviewSession,
  );
  const reviewRelease = await evaluate(
    "const d=document.querySelector('[role=dialog]'); return {fileSizeExplained:d.textContent.includes('文件大小，非可释放空间'),noSeparateSpace:!d.querySelector('.space-status'),noPersistentReview:!d.textContent.includes('继续核对')}",
  );
  assert(
    reviewRelease.fileSizeExplained &&
      reviewRelease.noSeparateSpace &&
      reviewRelease.noPersistentReview,
  );
  await writeFile(
    'artifacts/discovery-0.17.2/release-review.png',
    Buffer.from(await request('GET', `/session/${session}/screenshot`), 'base64'),
  );
  // Check reduced-motion styles without changing the saved user preference.
  reviewRelease.reducedMotion = await evaluate(
    "const previous=document.documentElement.dataset.motion; document.documentElement.dataset.motion='reduced'; const seconds=parseFloat(getComputedStyle(document.querySelector('[role=dialog]')).animationDuration); if(previous===undefined)delete document.documentElement.dataset.motion; else document.documentElement.dataset.motion=previous; return seconds<=0.001",
  );
  assert(reviewRelease.reducedMotion);
  const closeReview = await find('//button[@aria-label="关闭文件详情"]');
  await request('POST', `/session/${session}/element/${closeReview}/click`, {});
  await until(
    async () =>
      await evaluate("return document.activeElement?.hasAttribute('data-analysis-review')"),
  );
  const resumeReview = await find('(//button[@data-analysis-review])[1]');
  await request('POST', `/session/${session}/element/${resumeReview}/click`, {});
  const canRefine = await evaluate(
    "return [...document.querySelectorAll('[role=dialog] button')].some(b=>b.textContent.trim()==='选择此目录补充分析')",
  );
  if (canRefine) {
    const refineReview = await find(
      '//*[@role="dialog"]//button[normalize-space(.)="选择此目录补充分析"]',
    );
    await request('POST', `/session/${session}/element/${refineReview}/click`, {});
    await until(
      async () =>
        await evaluate(
          "return !document.querySelector('[role=dialog]') && document.activeElement?.textContent.trim()==='开始只读分析'",
        ),
    );
    assert.equal(
      await evaluate(
        "return window.__TAURI_INTERNALS__.invoke('get_runtime_snapshot',{scope:'user'}).then(s=>s.analysis.id)",
      ),
      reviewSession,
    );
    reviewRelease.refineFocusWithoutStarting = true;
    const resumeAfterRefine = await find('(//button[@data-analysis-review])[1]');
    await request('POST', `/session/${session}/element/${resumeAfterRefine}/click`, {});
  } else reviewRelease.refineFocusWithoutStarting = 'no-partial-directory-at-first-entry';
  const endReview = await find('//*[@role="dialog"]//button[normalize-space(.)="返回列表"]');
  await request('POST', `/session/${session}/element/${endReview}/click`, {});
  await until(
    async () =>
      await evaluate(
        "return !document.querySelector('[role=dialog]') && !document.body.textContent.includes('继续核对')",
      ),
  );
  const returnClean = await find('//nav/button[normalize-space(.)="清理"]');
  await request('POST', `/session/${session}/element/${returnClean}/click`, {});
  // Tao retains WS_CAPTION for Windows behavior but removes its non-client region.
  // Measure the actual client origin instead of treating the style bit as a visible caption.
  const nativeWindow = await nativeState();
  assert(
    nativeWindow.nonClientTopPixels >= 0 && nativeWindow.nonClientTopPixels <= 4,
    `Expected no native titlebar: ${JSON.stringify(nativeWindow)}`,
  );
  await clickLabel('最大化');
  await until(async () => (await nativeState()).maximized);
  await until(() => find('//button[@aria-label="还原窗口"]'));
  await clickLabel('还原窗口');
  await until(async () => !(await nativeState()).maximized);
  // The native drag-region handler also owns double-click maximize/restore.
  const dragArea = await find('//div[@class="window-drag-area"]');
  const pointer = {
    type: 'pointerMove',
    duration: 0,
    origin: { 'element-6066-11e4-a52e-4f735466cecf': dragArea },
    x: 0,
    y: 0,
  };
  await request('POST', `/session/${session}/actions`, {
    actions: [
      {
        type: 'pointer',
        id: 'titlebar-mouse',
        parameters: { pointerType: 'mouse' },
        actions: [
          pointer,
          { type: 'pointerDown', button: 0 },
          { type: 'pointerUp', button: 0 },
          { type: 'pause', duration: 100 },
          { type: 'pointerDown', button: 0 },
          { type: 'pointerUp', button: 0 },
        ],
      },
    ],
  });
  await until(async () => (await nativeState()).maximized);
  await clickLabel('还原窗口');
  await until(async () => !(await nativeState()).maximized);
  const rejections = await evaluate(`return (async () => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    const before = await invoke('get_job_status', { scope: 'user' });
    const selection = { rule_ids: ['windows.user_temp'], excluded_ids: [] };
    const plan = { id: 'forged-plan', session_id: 'forged-session', files: 1, bytes: '1', rule_ids: ['windows.user_temp'], impacts: [], categories: [] };
    const attempts = [
      ['get_job_status', { scope: 'system' }],
      ['get_runtime_snapshot', { scope: 'system' }],
      ['reveal_candidate', { scope: 'system', session:'fake', candidateId:'fake' }],
      ['add_candidate_exclusions', { scope: 'system', session:'fake', items:[] }],
      ['start_scan', { ruleIds: ['windows.user_temp', 'windows.system_temp'] }],
      ['prepare_cleanup', { preparationId: crypto.randomUUID(), requests: [{ scope: 'user', session: 'forged-session', selection }, { scope: 'system', session: 'forged-session', selection }] }],
      ['execute_cleanup', { plans: [{ scope: 'user', plan }, { scope: 'system', plan }] }],
      ['execute_cleanup', { plans: [{ scope: 'user', plan }] }],
      ['start_analysis', { token: 'forged-location' }],
      ['reveal_analysis_entry', { session: 'forged-session', node: '0' }],
      ['list_analysis_files', {session: 'forged-session', search: '', offset: 0}],
      ['get_analysis_context', {locationToken: 'C:\\\\arbitrary-path'}],
    ];
    const codes = [];
    for (const [command, args] of attempts) {
      try {
        if (['add_candidate_exclusions','start_scan','prepare_cleanup','execute_cleanup','start_analysis'].includes(command)) {
          const { control } = await invoke('get_runtime_snapshot', {scope:'user'});
          const reply = await invoke(command, { ...args, request: {runtime_id:control.runtime_id, expected_revision:control.revision, request_id:crypto.randomUUID()} });
          if (reply.last_request.error) throw reply.last_request.error;
        } else await invoke(command, args);
        codes.push('unexpected_success');
      }
      catch (error) { codes.push(error.code); }
    }
    const after = await invoke('get_job_status', { scope: 'user' });
    return { codes, preserved: before.id === after.id && before.state === after.state };
  })()`);
  assert.deepEqual(rejections.codes, [
    'capability_unavailable',
    'capability_unavailable',
    'capability_unavailable',
    'capability_unavailable',
    'capability_unavailable',
    'capability_unavailable',
    'capability_unavailable',
    'invalid_plan',
    'invalid_location',
    'invalid_session',
    'invalid_session',
    'invalid_location',
  ]);
  assert.equal(rejections.preserved, true);
  // Direct boundary probes consume control revisions; attach the UI again.
  await request('POST', `/session/${session}/refresh`, {});
  await until(async () =>
    evaluate(
      "return !!document.querySelector('.action-buttons button:not([disabled])') && !document.body.textContent.includes('正在读取设置并核对后台状态')",
    ),
  );
  await clickLabel('最小化');
  await until(async () => (await nativeState()).minimized);
  const duplicate = spawn(executable, [], { windowsHide: true, stdio: 'ignore' });
  const exited = new Promise((r, reject) => {
    const timer = setTimeout(() => {
      duplicate.kill();
      reject(Error('Duplicate instance did not exit'));
    }, 10000);
    duplicate.once('exit', () => {
      clearTimeout(timer);
      r();
    });
  });
  await exited;
  await until(async () => !(await nativeState()).minimized);
  // Scan the current user's supported locations through the default UI. Never prepare or execute cleanup.
  const scanRuleIds = first.rules
    .filter((r) => !r.requires_admin && !first.settings.excluded_rules.includes(r.id))
    .map((r) => r.id);
  assert(
    first.rules
      .filter((r) => r.root === 'windows_residue')
      .every((r) => r.cleanup_policy === 'review' && r.cleanup_validated === false),
  );
  assert.deepEqual(
    await evaluate(
      "return [...document.querySelectorAll('.rule-list input:checked')].map(e=>e.id)",
    ),
    [],
  );
  const scan = await find('//button[normalize-space(.)="开始扫描"]');
  await request('POST', `/session/${session}/element/${scan}/click`, {});
  await until(async () => {
    const state = await evaluate(
      "return window.__TAURI_INTERNALS__.invoke('get_job_status',{scope:'user'})",
    );
    return state.id && ['scanning', 'ready'].includes(state.state);
  });
  // Capture the preference written by this test as soon as its scan is accepted, even if a later
  // assertion fails. Restore it only while it still equals this observed value.
  observedPreference = readScanPreference();
  await until(() => find('//button[normalize-space(.)="重新全面扫描"]'), 600000);
  const status = await evaluate(
    "return window.__TAURI_INTERNALS__.invoke('get_job_status',{scope:'user'})",
  );
  assert.equal(status.state, 'ready');
  const summary = await evaluate(
    "return window.__TAURI_INTERNALS__.invoke('get_scan_summary',{scope:'user'})",
  );
  assert.deepEqual(summary.rules.map((r) => r.rule_id).sort(), [...scanRuleIds].sort());
  assert(summary.rules.every((r) => ['completed', 'partial', 'failed'].includes(r.phase)));
  assert(summary.rules.filter((r) => r.scan_failed).every((r) => r.blocked && r.scan_issue));
  assert(
    summary.rules
      .filter((r) => /^(weixin|qq|wps)\./.test(r.rule_id))
      .every(
        (r) =>
          r.blocked &&
          ['validation_pending', 'application_running', 'process_unknown'].includes(r.availability),
      ),
  );
  observedPreference = readScanPreference();
  // Refreshing capability metadata remounts decision rows. Closing the guide must find its new origin.
  const beforeGuideSelection = await evaluate(
    "return [...document.querySelectorAll('.rule-list input:checked')].map(e=>e.id).sort()",
  );
  await evaluate(`for(const d of document.querySelectorAll('.result-section:not([open])')) d.querySelector('summary').click();
    for(const b of document.querySelectorAll('.purpose-toggle[aria-expanded="false"]')) b.click();`);
  assert(await evaluate('return !document.querySelector(".scan-scope");'));
  if (summary.rules.some((row) => row.rule_id.startsWith('weixin.') && row.files > 0)) {
    await clickText('查看微信管理指引');
    await clickText('重新核对应用信息');
    await until(async () =>
      evaluate(
        "return !document.body.textContent.includes('正在核对本机应用版本与身份') && !!document.querySelector('.decision-row[data-status=readonly] [data-guide-target^=weixin]')",
      ),
    );
    await clickText('关闭管理指引');
    const guideFocusCheck = await evaluate(
      "const e=document.activeElement,r=e.getBoundingClientRect(); return {target:e.dataset.guideTarget,visible:r.top>=0&&r.bottom<=document.querySelector('.task-dock').getBoundingClientRect().top};",
    );
    assert(guideFocusCheck.target?.startsWith('weixin:') && guideFocusCheck.visible);
    assert.deepEqual(
      await evaluate(
        "return [...document.querySelectorAll('.rule-list input:checked')].map(e=>e.id).sort()",
      ),
      beforeGuideSelection,
    );
  }
  await writeFile(
    'artifacts/discovery-0.17.2/release-guide-focus.png',
    Buffer.from(await request('GET', `/session/${session}/screenshot`), 'base64'),
  );
  const png = await request('GET', `/session/${session}/screenshot`);
  await writeFile('artifacts/portable-readonly-0.17.2.png', Buffer.from(png, 'base64'));
  assert.deepEqual(await readdir(directory), ['拂尘.exe']);
  await request('DELETE', `/session/${session}`);
  session = undefined;
  await acl(true);
  protectedDirectory = false;
  // Move only this fixture's executable, between two absolute paths under its owned root.
  const destination = join(moved, '拂尘.exe');
  await rename(executable, destination);
  executable = destination;
  const second = await launch();
  assert.deepEqual(second.settings, first.settings);
  assert.deepEqual(await readdir(moved), ['拂尘.exe']);
  await clickLabel('关闭窗口').catch(() => {}); // WebView can close before the click response arrives.
  await until(async () => !(await nativeState()).running);
  await writeFile(
    'artifacts/portable-smoke-0.17.2.json',
    JSON.stringify(
      {
        version: second.version,
        frontendAssets,
        homeDiskRefreshWithoutAnalysis: true,
        hoverContrast,
        backend: 'standard',
        display,
        rules: second.rules.length,
        nativeAnalysisLocations: analysisLocations.length,
        fixedVolumes: volumes.map(({ path, is_system, supported, unavailable_reason }) => ({
          path,
          is_system,
          supported,
          unavailable_reason,
        })),
        defaultSystemVolumeWithoutAutoStart: true,
        allSupportedVolumeSamplesVerified: true,
        explicitReadOnlyRootProbes: rootProbes,
        diskContextTokenBound: true,
        largestFileQueryRejectsStaleSession: true,
        noAutomaticAnalysis: true,
        readonlyDirectory: true,
        duplicateInstance: true,
        movePreservesSettings: true,
        readOnlyScan: status.state,
        scanRuleIds,
        scanResults: summary.rules.map((r) => ({
          ruleId: r.rule_id,
          files: r.files,
          scanFailed: r.scan_failed,
          phase: r.phase,
          availability: r.availability,
          note: r.note,
          reasons: r.reasons,
        })),
        cleanupStarted: false,
        reviewRelease,
        discoveryRelease,
        guideFocusAfterSupportRefresh: guideFocusCheck,
        negativePlanRequestsRejected: true,
        customTitlebar: {
          nonClientTopPixels: nativeWindow.nonClientTopPixels,
          maximizeRestore: true,
          doubleClick: true,
          minimizeRestore: true,
          close: true,
        },
        capabilitiesVerified: rejections,
        executableSha256: createHash('sha256')
          .update(await readFile(executable))
          .digest('hex'),
      },
      null,
      2,
    ),
  );
  console.log(
    'PASS: portable release, Chinese/spaced paths, denied-write directory, no helper, duplicate instance, move/settings and read-only scan.',
  );
  console.log('Fixture retained: ' + root);
} catch (error) {
  if (session) {
    await writeFile(
      'artifacts/discovery-0.17.2/release-failure.html',
      await request('GET', `/session/${session}/source`),
    ).catch(() => {});
    const screenshot = await request('GET', `/session/${session}/screenshot`).catch(() => null);
    if (screenshot)
      await writeFile(
        'artifacts/discovery-0.17.2/release-failure.png',
        Buffer.from(screenshot, 'base64'),
      );
  }
  throw error;
} finally {
  if (session) await request('DELETE', `/session/${session}`).catch(() => {});
  driver.kill();
  if (protectedDirectory) await acl(true);
  restoreOwnPreference();
}
