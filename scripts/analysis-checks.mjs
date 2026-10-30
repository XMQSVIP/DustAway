import { mkdir, writeFile, readFile, link, stat } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import assert from 'node:assert/strict';

export async function seedAnalysis(root) {
  const analysis = join(root, 'analysis');
  await mkdir(join(analysis, '大型资料'), { recursive: true });
  const protectedFile = join(analysis, '大型资料', '工作文件.gguf');
  await writeFile(protectedFile, Buffer.alloc(32768, 65));
  await link(protectedFile, join(analysis, '共享文件.txt'));
  // Actual NTFS entries, exclusively inside the marked debug fixture.
  const count = process.env.DUSTAWAY_ANALYSIS_SMALL ? 1000 : 100000;
  for (let group = 0; group < count / 1000; group++) {
    const dir = join(
      analysis,
      group === count / 1000 - 1 ? 'node_modules' : `目录 ${String(group).padStart(3, '0')}`,
    );
    await mkdir(dir);
    for (let first = 0; first < 1000; first += 32) {
      await Promise.all(
        Array.from({ length: Math.min(32, 1000 - first) }, (_, i) =>
          writeFile(join(dir, `文件 ${first + i}.txt`), 'x'),
        ),
      );
    }
  }
  console.log(`Analysis fixture ready: ${count + 2} files`);
  return { count, protectedFile, modified: (await stat(protectedFile)).mtimeMs };
}

export async function checkAnalysis({
  command,
  session,
  click,
  until,
  source,
  root,
  output,
  fixture,
  tabTo,
  key,
}) {
  const version = JSON.parse(await readFile('package.json', 'utf8')).version;
  await mkdir(`${output}/analysis`);
  await mkdir(`${output}/analysis/discovery`);
  const ipc = (name, args = {}) =>
    command('POST', `/session/${session}/execute/sync`, {
      script: 'return window.__TAURI_INTERNALS__.invoke(arguments[0], arguments[1]);',
      args: [name, args],
    });
  const snapshot = () => ipc('get_runtime_snapshot', { scope: 'user' });
  await click('//nav/button[normalize-space(.)="空间分析"]');
  const began = Date.now();
  await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script: `return [...document.querySelectorAll('button')].some(b=>b.textContent.trim()==='开始只读分析'&&!b.disabled)`,
      args: [],
    }),
  );
  await click('//button[normalize-space(.)="开始只读分析"]');
  await until(async () => (await snapshot()).analysis?.state === 'scanning');
  // Another task cannot enter while analysis owns the worker slot.
  const control = (await snapshot()).control;
  const rejected = await ipc('start_scan', {
    ruleIds: ['windows.user_temp'],
    request: {
      runtime_id: control.runtime_id,
      expected_revision: control.revision,
      request_id: crypto.randomUUID(),
    },
  });
  assert.equal(rejected.last_request.error.code, 'busy');
  const complete = await until(async () => {
    const s = (await snapshot()).analysis;
    return s?.state !== 'scanning' && s;
  }, 240000);
  const scanWallMs = Date.now() - began;
  await writeFile(join(root, 'analysis-completed.json'), JSON.stringify(complete, null, 2));
  assert.equal(complete.state, 'completed');
  assert.equal(complete.files, fixture.count + 2);
  assert.equal(complete.unique_bytes, String(fixture.count + 32768));
  assert.equal(complete.logical_bytes, String(fixture.count + 65536));
  const context = await ipc('get_analysis_context', { locationToken: complete.location.token });
  assert(context.volume && BigInt(context.available_bytes) > 0n && !context.error);
  await assert.rejects(ipc('get_analysis_context', { locationToken: root }));
  const ranked = await ipc('list_analysis_files', { session: complete.id, search: '', offset: 0 });
  assert.equal(ranked.total, fixture.count + 2);
  assert.equal(ranked.items.length, 200);
  assert.equal(ranked.items[0].bytes, '32768');
  assert(ranked.items[0].shared && ranked.items[1].shared);
  const deepFiles = await ipc('list_analysis_files', {
    session: complete.id,
    search: '工作文件',
    offset: 0,
  });
  assert.equal(deepFiles.total, 1);
  assert.equal(deepFiles.items[0].advice.kind, 'unknown');
  assert.equal(deepFiles.items[0].rule_ids.length, 0);
  assert.equal(deepFiles.items[0].advice.recognition.rule_id, 'model.gguf');
  const querySamples = [];
  const fileQuery = (search, offset) =>
    ipc('list_analysis_files', { session: complete.id, search, offset });
  for (const [name, search] of [
    ['all', ''],
    ['search', '文件 9'],
  ]) {
    const total = (await fileQuery(search, 0)).total;
    for (const [position, offset] of [
      ['first', 0],
      ['last', Math.floor((total - 1) / 200) * 200],
    ]) {
      await fileQuery(search, offset);
      const samples = [];
      for (let i = 0; i < 50; i++) {
        const began = performance.now();
        const result = await fileQuery(search, offset);
        samples.push(performance.now() - began);
        assert.equal(result.total, total);
        assert(result.items.length > 0 && result.items.length <= 200);
      }
      const p95 = [...samples].sort((a, b) => a - b)[47];
      querySamples.push({
        name: `${name}-${position}`,
        search,
        offset,
        samples_ms: samples,
        p95_ms: p95,
        passed: p95 <= 200,
      });
    }
  }
  await writeFile(
    `${output}/analysis/ipc-queries-${version}.json`,
    JSON.stringify(
      { version, build: 'debug-e2e', files: fixture.count + 2, queries: querySamples },
      null,
      2,
    ),
  );
  await assert.rejects(ipc('list_analysis_files', { session: 'forged', search: '', offset: 0 }));
  const queryStart = performance.now();
  const page = await ipc('list_analysis_children', {
    session: complete.id,
    node: '0',
    search: '',
    offset: 0,
    descending: true,
  });
  const queryMs = performance.now() - queryStart;
  const largest = page.items.find((e) => e.name === '大型资料');
  assert(largest && largest.bytes === '32768');
  const folder = page.items.find((e) => e.name === '目录 000');
  assert(folder);
  const query = (node, offset = 0, search = '', descending = true) =>
    ipc('list_analysis_children', { session: complete.id, node, search, offset, descending });
  const firstPage = await query(folder.id);
  const secondPage = await query(folder.id, 200);
  assert.equal(firstPage.items.length, 200);
  assert.equal(firstPage.total, 1000);
  assert.equal(secondPage.items.length, 200);
  assert(!secondPage.items.some((e) => firstPage.items.some((f) => f.id === e.id)));
  assert.equal((await query(folder.id, 0, '文件 999.txt')).total, 1);
  const ascending = await query('0', 0, '', false);
  assert(BigInt(ascending.items[0].bytes) <= BigInt(ascending.items.at(-1).bytes));
  const planControl = (await snapshot()).control;
  const forbiddenPlan = await ipc('prepare_cleanup', {
    preparationId: crypto.randomUUID(),
    request: {
      runtime_id: planControl.runtime_id,
      expected_revision: planControl.revision,
      request_id: crypto.randomUUID(),
    },
    requests: [
      {
        scope: 'user',
        session: complete.id,
        selection: { rule_ids: ['windows.user_temp'], excluded_ids: [] },
      },
    ],
  });
  assert(forbiddenPlan.last_request.error, 'Analysis session must never authorize a cleanup plan');
  await assert.rejects(
    ipc('reveal_analysis_entry', { session: complete.id, node: '../personal.txt' }),
  );
  // The forged controlled request above changes the backend receipt revision.
  // Reload through the real UI to reconcile it before issuing further UI mutations.
  await command('POST', `/session/${session}/refresh`, {});
  await click('//nav/button[normalize-space(.)="空间分析"]');
  await until(async () => (await source()).includes('分析完成'));
  assert.equal((await snapshot()).analysis.id, complete.id);
  const discoveryQuery = {
    kind: 'directory',
    category: 'development',
    min_bytes: '0',
    search: '',
    include_nested: false,
    incomplete_only: false,
    offset: 0,
  };
  // Default page queries the whole session for files before any purpose filter.
  assert.equal(
    await command('POST', `/session/${session}/execute/sync`, {
      script:
        'return [...document.querySelectorAll("button")].find(b=>b.textContent.trim()==="大文件")?.getAttribute("aria-pressed");',
      args: [],
    }),
    'true',
  );
  const allRanked = await ipc('list_analysis_discovery', {
    session: complete.id,
    query: { ...discoveryQuery, kind: 'file', category: 'all' },
  });
  assert(allRanked.total >= fixture.count);
  assert.equal(allRanked.items[0].bytes, '32768');
  assert(
    allRanked.items.every(
      (e, i) => i === 0 || BigInt(allRanked.items[i - 1].bytes) >= BigInt(e.bytes),
    ),
  );
  const discovered = await ipc('list_analysis_discovery', {
    session: complete.id,
    query: discoveryQuery,
  });
  assert.equal(discovered.total, 1);
  assert.equal(discovered.items[0].name, 'node_modules');
  assert.equal(discovered.items[0].bytes, '1000');
  const discoveredModel = await ipc('list_analysis_discovery', {
    session: complete.id,
    query: { ...discoveryQuery, kind: 'file', category: 'ai' },
  });
  assert.equal(discoveredModel.total, 1);
  assert.equal(discoveredModel.items[0].name, '工作文件.gguf');
  assert.equal(discoveredModel.items[0].advice.recognition.category, 'ai');
  await assert.rejects(ipc('list_analysis_discovery', { session: 'stale', query: discoveryQuery }));
  await click('//button[normalize-space(.)="大目录"]');
  await click('//div[contains(@class,"discovery-filters")]//summary');
  await click('//div[contains(@class,"discovery-filters")]//select/option[@value="development"]');
  await until(
    async () =>
      (await source()).includes('node_modules') && (await source()).includes('discovery-filters'),
  );
  const discoveryLayouts = [];
  for (const theme of ['light', 'dark']) {
    await command('POST', `/session/${session}/execute/sync`, {
      script:
        "document.documentElement.dataset.theme=arguments[0]; document.documentElement.dataset.motion='reduced';",
      args: [theme],
    });
    for (const [width, height] of [
      [1100, 780],
      [760, 580],
    ]) {
      await command('POST', `/session/${session}/window/rect`, { width, height });
      await command('POST', `/session/${session}/execute/sync`, {
        script: "document.querySelector('.analysis-view-switch').scrollIntoView({block:'start'});",
        args: [],
      });
      const layout = await command('POST', `/session/${session}/execute/sync`, {
        script:
          "const f=document.querySelector('.discovery-filters').getBoundingClientRect(); const filtersVisible=f.top>=0&&f.top<innerHeight; const action=document.querySelector('[data-analysis-review]'); action.scrollIntoView({block:'nearest'}); const b=action.getBoundingClientRect(); return {overflow:document.documentElement.scrollWidth>innerWidth,filtersVisible,firstActionVisible:b.top>=0&&b.bottom<=Math.min(innerHeight,document.querySelector('.action-bar').getClientRects().length ? document.querySelector('.action-bar').getBoundingClientRect().top : innerHeight),scrollable:document.querySelector('#main').scrollHeight>document.querySelector('#main').clientHeight};",
        args: [],
      });
      assert(
        !layout.overflow && layout.filtersVisible && layout.firstActionVisible,
        JSON.stringify(layout),
      );
      discoveryLayouts.push({ theme, width, height, ...layout });
      await writeFile(
        `${output}/analysis/discovery/discovery-${theme}-${width}.png`,
        Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
      );
    }
  }
  await command('POST', `/session/${session}/window/rect`, { width: 1100, height: 780 });
  await command('POST', `/session/${session}/execute/sync`, {
    script:
      "document.documentElement.dataset.theme='light'; delete document.documentElement.dataset.motion;",
    args: [],
  });
  await click('//button[@data-analysis-entry and normalize-space(.)="node_modules"]');
  await until(async () => (await source()).includes('返回空间排行'));
  await click('//button[normalize-space(.)="返回空间排行"]');
  await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script: "return document.activeElement?.getAttribute('data-analysis-entry') === arguments[0]",
      args: [discovered.items[0].id],
    }),
  );
  await click('//button[@data-analysis-review]');
  await click('//button[@aria-label="关闭文件详情"]');
  await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script: "return document.activeElement?.hasAttribute('data-analysis-review')",
      args: [],
    }),
  );
  await writeFile(
    `${output}/analysis/discovery/discovery-desktop.json`,
    JSON.stringify(
      {
        layouts: discoveryLayouts,
        wholeScopeQueries: true,
        directoryBytes: '1000',
        modelFound: true,
        returnFocus: true,
        cleanupRules: discovered.items[0].rule_ids,
      },
      null,
      2,
    ),
  );
  // Returning from a detail view remounts the collapsed filters. Open them before
  // selecting an option, just as a keyboard or pointer user must do.
  await click('//div[contains(@class,"discovery-filters")]//summary');
  await click('//div[contains(@class,"discovery-filters")]//select/option[@value="all"]');
  await tabTo('大文件', true);
  await key('\uE007');
  await until(async () => (await source()).includes('搜索本次范围的名称'));
  const searchField = await command('POST', `/session/${session}/element`, {
    using: 'css selector',
    value: '.file-tools input[type="search"]',
  });
  await command('POST', `/session/${session}/element/${Object.values(searchField)[0]}/value`, {
    text: '工作文件',
    value: [...'工作文件'],
  });
  await until(async () => (await source()).includes('共 1 项'));
  await click('//button[@data-analysis-review]');
  await until(async () => (await source()).includes('可能是GGUF 模型文件'));
  await click('//summary[normalize-space(.)="依据与完整路径"]');
  await writeFile(
    `${output}/analysis/largest-files-${version}.png`,
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
  await click('//button[@aria-label="关闭文件详情"]');
  await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script: "return document.activeElement?.hasAttribute('data-analysis-review')",
      args: [],
    }),
  );
  await click('//button[@data-analysis-review]');
  await click('//summary[normalize-space(.)="更多操作"]');
  await click('//button[normalize-space(.)="打开所在目录"]');
  await until(
    async () =>
      (await source()).includes('工作文件.gguf') && (await source()).includes('分析目录层级'),
  );
  await click('//button[@data-analysis-review]');
  await click('//summary[normalize-space(.)="更多操作"]');
  await click('//button[normalize-space(.)="在资源管理器中定位"]');
  const revealed = await promisify(execFile)(
    'powershell.exe',
    ['-NoProfile', '-File', resolve('scripts/check-reveal.ps1'), '-Path', fixture.protectedFile],
    { windowsHide: true },
  );
  assert(revealed.stdout.includes('SELECTED'));
  const reviewLayouts = [];
  for (const theme of ['light', 'dark']) {
    // Visual-only theme override, restored below; settings remain unchanged.
    await command('POST', `/session/${session}/execute/sync`, {
      script: 'document.documentElement.dataset.theme=arguments[0]',
      args: [theme],
    });
    for (const [width, height] of [
      [1100, 780],
      [760, 580],
    ]) {
      await command('POST', `/session/${session}/window/rect`, { width, height });
      const reviewLayout = await command('POST', `/session/${session}/execute/sync`, {
        script: `const d=document.querySelector('[role=dialog]'); const body=d.querySelector('.analysis-review-body'); const action=d.querySelector('.analysis-review-actions button'); const r=action.getBoundingClientRect(); const rect=d.getBoundingClientRect(); return {width:innerWidth,height:innerHeight,overflow:document.documentElement.scrollWidth>innerWidth,bodyScrollable:body.scrollHeight>body.clientHeight,actionVisible:r.top>=0&&r.bottom<=innerHeight,dialogFits:rect.top>=0&&rect.bottom<=innerHeight};`,
        args: [],
      });
      assert(reviewLayout.actionVisible && reviewLayout.dialogFits && !reviewLayout.overflow);
      if (width === 760) assert(reviewLayout.bodyScrollable);
      reviewLayouts.push({ theme, width, height, ...reviewLayout });
      await writeFile(
        `${output}/analysis/discovery/review-${theme}-${width}.png`,
        Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
      );
    }
  }
  await command('POST', `/session/${session}/execute/sync`, {
    script: "document.documentElement.dataset.theme='light'",
    args: [],
  });
  assert(
    !(await command('POST', `/session/${session}/execute/sync`, {
      script: 'return !!document.querySelector("[role=dialog] .space-status")',
      args: [],
    })),
  );
  assert.equal((await snapshot()).analysis.id, complete.id);
  await tabTo('返回列表');
  await key('\uE007');
  await until(
    async () =>
      !(await command('POST', `/session/${session}/execute/sync`, {
        script: 'return !!document.querySelector("[role=dialog]")',
        args: [],
      })),
  );
  await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script: "return document.activeElement?.hasAttribute('data-analysis-review')",
      args: [],
    }),
  );
  await writeFile(
    `${output}/analysis/light-${version}.png`,
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
  await command('POST', `/session/${session}/window/rect`, { width: 760, height: 580 });
  const layout = await command('POST', `/session/${session}/execute/sync`, {
    script: `const main=document.querySelector('.main-content'); main.scrollTop=main.scrollHeight; const list=document.querySelector('.analysis-list'); return {overflow:document.documentElement.scrollWidth>innerWidth,listHeight:list.getBoundingClientRect().height};`,
    args: [],
  });
  assert(!layout.overflow && layout.listHeight >= 180);
  await writeFile(
    `${output}/analysis/minimum-${version}.png`,
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
  await command('POST', `/session/${session}/window/rect`, { width: 1100, height: 780 });
  // Force the origin out of the virtual DOM while its dialog is open, then restore it.
  await click('//button[normalize-space(.)="大文件"]');
  const reviewSearch = await command('POST', `/session/${session}/element`, {
    using: 'css selector',
    value: '.file-tools input[type="search"]',
  });
  await command('POST', `/session/${session}/element/${Object.values(reviewSearch)[0]}/click`, {});
  await command('POST', `/session/${session}/actions`, {
    actions: [
      {
        type: 'key',
        id: 'clear-search',
        actions: [
          { type: 'keyDown', value: '\uE009' },
          { type: 'keyDown', value: 'a' },
          { type: 'keyUp', value: 'a' },
          { type: 'keyUp', value: '\uE009' },
          { type: 'keyDown', value: '\uE003' },
          { type: 'keyUp', value: '\uE003' },
        ],
      },
    ],
  });
  await until(async () =>
    (await source()).includes(`共 ${(fixture.count + 2).toLocaleString()} 项`),
  );
  const originId = await command('POST', `/session/${session}/execute/sync`, {
    script: "return document.querySelector('[data-analysis-review]').dataset.analysisReview",
    args: [],
  });
  await click('(//button[@data-analysis-review])[1]');
  await command('POST', `/session/${session}/execute/sync`, {
    script: "const list=document.querySelector('.analysis-list');list.scrollTop=list.scrollHeight",
    args: [],
  });
  await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script:
        "return ![...document.querySelectorAll('[data-analysis-review]')].some(b=>b.dataset.analysisReview===arguments[0])",
      args: [originId],
    }),
  );
  await click('//button[@aria-label="关闭文件详情"]');
  await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script: 'return document.activeElement?.dataset.analysisReview===arguments[0]',
      args: [originId],
    }),
  );
  await click('(//button[@data-analysis-review])[1]');
  await click('//button[normalize-space(.)="返回列表"]');
  const appPath = resolve('target/debug/dustaway.exe').replaceAll("'", "''");
  const resources = await promisify(execFile)(
    'powershell.exe',
    [
      '-NoProfile',
      '-Command',
      `$all=Get-CimInstance Win32_Process; $apps=$all|Where-Object {$_.ExecutablePath -eq '${appPath}'}; $ids=@($apps.ProcessId); do { $children=@($all|Where-Object {$_.ParentProcessId -in $ids -and $_.ProcessId -notin $ids}); $ids+=@($children.ProcessId) } while($children.Count -gt 0); @($all|Where-Object {$_.ProcessId -in $ids})|ForEach-Object {$p=Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue; [pscustomobject]@{name=$_.Name;workingSetBytes=$p.WorkingSet64;privateBytes=$p.PrivateMemorySize64;cpuSeconds=$p.CPU}}|ConvertTo-Json`,
    ],
    { windowsHide: true },
  );
  // Start, reload the page while the backend continues, then stop the same session.
  await click('//summary[contains(.,"位置与重新分析")]');
  await click('//button[normalize-space(.)="重新分析"]');
  const running = await until(async () => {
    const s = (await snapshot()).analysis;
    return s?.state === 'scanning' && s;
  });
  await command('POST', `/session/${session}/refresh`, {});
  await until(async () => (await source()).includes('查看空间分析'));
  const cancelStart = Date.now();
  await click('//button[normalize-space(.)="停止分析"]');
  const cancelled = await until(async () => {
    const s = (await snapshot()).analysis;
    return s?.state !== 'scanning' && s;
  }, 30000);
  assert.equal(cancelled.id, running.id);
  assert.equal(cancelled.state, 'cancelled');
  const cancelMs = Date.now() - cancelStart;
  await click('//nav/button[normalize-space(.)="空间分析"]');
  await until(async () => (await source()).includes('已停止 · 统计不完整'));
  await click('//nav/button[contains(.,"设置")]');
  await click('//button[contains(.,"深色")]');
  await click('//nav/button[normalize-space(.)="空间分析"]');
  await writeFile(
    `${output}/analysis/dark-${version}.png`,
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
  await click('//nav/button[contains(.,"设置")]');
  await click('//button[contains(.,"浅色")]');
  await click('//nav/button[normalize-space(.)="清理"]');
  assert.equal((await stat(fixture.protectedFile)).mtimeMs, fixture.modified);
  assert.equal((await readFile(fixture.protectedFile)).compare(Buffer.alloc(32768, 65)), 0);
  const result = {
    version,
    files: fixture.count + 2,
    scanWallMs,
    engineElapsedMs: complete.elapsed_ms,
    queryMs,
    cancelMs,
    layout,
    hardlinksDeduplicated: true,
    paginatedSortedSearch: true,
    globalLargestFiles: true,
    purposeBoundaries: true,
    diskContext: true,
    homepageDiagnosticEntry: true,
    keyboardAnalysisFlow: true,
    reviewLayouts,
    reviewFocusAndExternalSpace: true,
    virtualOriginFocusRestored: true,
    analysisPlanRejected: true,
    reloadRecovery: true,
    protectedFiles: true,
    resources: JSON.parse(resources.stdout),
  };
  await writeFile(
    `${output}/analysis/performance-${version}.json`,
    JSON.stringify(result, null, 2),
  );
  console.log(
    'PASS: read-only analysis, hardlinks, navigation, reveal, cancellation, reload and task exclusion',
  );
  return result;
}
