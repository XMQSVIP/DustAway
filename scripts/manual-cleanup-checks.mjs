import assert from 'node:assert/strict';
import { mkdir, writeFile, readFile, stat, access, utimes } from 'node:fs/promises';
import { join } from 'node:path';

/** Real Windows deletion, strictly inside the caller's already verified disposable fixture. */
export async function checkManualCleanup({
  command,
  session,
  click,
  until,
  root,
  output,
  mixed = true,
  viewport,
  theme,
}) {
  assert.equal(await readFile(join(root, '.dustaway-test-only'), 'utf8'), 'fixture-only');
  const folder = join(root, 'analysis', '自选清理夹具');
  await mkdir(join(folder, '空文件夹'), { recursive: true });
  await mkdir(join(folder, 'xwechat_files'), { recursive: true });
  await writeFile(join(folder, '删除我.txt'), 'explicit manual fixture');
  await writeFile(join(folder, '保留我.txt'), 'keep explicit child');
  await writeFile(join(folder, 'xwechat_files', 'chat.db'), 'protected synthetic chat');
  const protectedPaths = [join(folder, '保留我.txt'), join(folder, 'xwechat_files', 'chat.db')];
  const protectedBefore = await Promise.all(
    protectedPaths.map(async (path) => ({
      path,
      content: await readFile(path),
      modified: (await stat(path)).mtimeMs,
    })),
  );
  const ipc = (name, args = {}) =>
    command('POST', `/session/${session}/execute/sync`, {
      script:
        'return window.__TAURI_INTERNALS__.invoke(arguments[0],arguments[1]).catch(error=>{throw new Error(typeof error === "string" ? error : JSON.stringify(error));});',
      args: [name, args],
    });
  const refresh = async () => {
    await command('POST', `/session/${session}/refresh`, {});
    await until(() =>
      command('POST', `/session/${session}/execute/sync`, {
        script: 'return !!document.querySelector(".theme-toggle:not([disabled])");',
        args: [],
      }),
    );
    if (theme)
      await command('POST', `/session/${session}/execute/sync`, {
        script: 'document.documentElement.dataset.theme=arguments[0];',
        args: [theme],
      });
  };
  const settleSearch = async () => {
    // Let the view's 250 ms debounce run before clicking a virtualized row;
    // WebDriver can finish a click while that row is being replaced.
    await command('POST', `/session/${session}/execute/sync`, {
      script: 'return new Promise(resolve => setTimeout(() => resolve(true), 350));',
      args: [],
    });
    await until(() =>
      command('POST', `/session/${session}/execute/sync`, {
        script:
          'return document.querySelector(".analysis-list")?.getAttribute("aria-busy") === "false";',
        args: [],
      }),
    );
  };
  const snapshot = () => ipc('get_runtime_snapshot', { scope: 'user' });
  const control = (await snapshot()).control;
  const location = (await ipc('get_analysis_locations')).find(
    (l) => l.path.toLowerCase() === join(root, 'analysis').toLowerCase(),
  );
  assert(location);
  const started = await ipc('start_analysis', {
    token: location.token,
    request: {
      runtime_id: control.runtime_id,
      expected_revision: control.revision,
      request_id: crypto.randomUUID(),
    },
  });
  assert(!started.last_request.error);
  await until(async () => (await snapshot()).analysis.state === 'completed', 90_000);
  await refresh();
  await click('//nav/button[normalize-space(.)="空间分析"]');
  await click('//button[normalize-space(.)="目录浏览"]');
  // Search makes the row reachable with virtualization, independent of other fixture sizes.
  const search = await command('POST', `/session/${session}/element`, {
    using: 'xpath',
    value: '//input[@placeholder="仅搜索当前层级"]',
  });
  await command(
    'POST',
    `/session/${session}/element/${search['element-6066-11e4-a52e-4f735466cecf']}/value`,
    { text: '自选清理夹具' },
  );
  await settleSearch();
  await click('//input[@aria-label="选择自选清理夹具"]');
  await click('//button[normalize-space(.)="自选清理夹具"]');
  await click('//input[@aria-label="选择保留我.txt"]');
  const protectedDisabled = await command('POST', `/session/${session}/execute/sync`, {
    script:
      'const item = document.querySelector(\'input[aria-label="选择xwechat_files"]\'); return item?.disabled && !item.checked;',
    args: [],
  });
  assert(protectedDisabled);
  // Details are a view over the same checkbox state; closing returns to the row.
  await click(
    '//button[@data-analysis-review and ancestor::*[contains(@class,"analysis-entry")][.//input[@aria-label="选择删除我.txt"]]]',
  );
  await click('//*[@role="dialog"]//input[@aria-label="选择删除我.txt"]');
  await click('//*[@role="dialog"]//input[@aria-label="选择删除我.txt"]');
  await command('POST', `/session/${session}/actions`, {
    actions: [
      {
        type: 'key',
        id: 'detail-keyboard',
        actions: [
          { type: 'keyDown', value: '\uE00C' },
          { type: 'keyUp', value: '\uE00C' },
        ],
      },
    ],
  });
  await command('DELETE', `/session/${session}/actions`);
  await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script: `return document.activeElement?.hasAttribute("data-analysis-review") && document.querySelector('input[aria-label="选择删除我.txt"]').checked;`,
      args: [],
    }),
  );
  // The UI deliberately supersedes older previews. Wait for its debounced request
  // before exercising the same read-only IPC independently.
  await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script: 'return /个文件夹/.test(document.querySelector(".task-dock")?.textContent ?? "");',
      args: [],
    }),
  );
  const beforePreview = await snapshot();
  assert.equal(beforePreview.analysis.query_api_version, 2);
  const rootRows = await ipc('list_analysis_children', {
    session: beforePreview.analysis.id,
    node: '0',
    search: '自选清理夹具',
    offset: 0,
    descending: true,
  });
  const readRequest = crypto.randomUUID();
  const diskRows = await ipc('read_analysis_page', {
    query: {
      request_id: readRequest,
      session: beforePreview.analysis.id,
      view: 'directories',
      node: '0',
      search: '自选清理夹具',
      metric: 'allocated',
      descending: true,
      discovery: null,
      cursor: null,
      offset: 0,
    },
  });
  assert.equal(diskRows.items.length, rootRows.items.length);
  assert(diskRows.items.every((item) => item.allocated_bytes !== null));
  assert.equal((await ipc('get_analysis_count', { requestId: readRequest })).finished, true);
  await ipc('cancel_analysis_query', { requestId: readRequest });
  assert(await ipc('get_analysis_storage'));
  const chosenFolder = rootRows.items.find((e) => e.name === '自选清理夹具');
  const childRows = await ipc('list_analysis_children', {
    session: beforePreview.analysis.id,
    node: chosenFolder.id,
    search: '',
    offset: 0,
    descending: true,
  });
  const kept = childRows.items.find((e) => e.name === '保留我.txt');
  const previewId = crypto.randomUUID();
  const preview = await ipc('get_manual_selection_preview', {
    requestId: previewId,
    selection: {
      session: beforePreview.analysis.id,
      selected_ids: [chosenFolder.id],
      excluded_ids: [kept.id],
    },
  });
  assert.equal(preview.request_id, previewId);
  assert.equal(preview.files, 1);
  assert.equal(preview.directories, 2);
  assert.equal(preview.bytes, String((await stat(join(folder, '删除我.txt'))).size));
  assert.equal((await snapshot()).control.revision, beforePreview.control.revision);
  assert.deepEqual((await snapshot()).report, beforePreview.report);
  const beforeConfirmation = await command('POST', `/session/${session}/execute/sync`, {
    script: `return {page:document.querySelector('.app-shell').dataset.page,
      search:document.querySelector('.analysis-panel input[type="search"]')?.value,
      scroll:document.querySelector('.analysis-list')?.scrollTop};`,
    args: [],
  });
  assert.equal(beforeConfirmation.page, 'analysis');
  const primaryVisible = await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script: `const buttons=[...document.querySelectorAll('button')].filter(b=>b.textContent.trim().startsWith('清理已选') && b.getClientRects().length);
      if(buttons.length!==1) return false; const r=buttons[0].getBoundingClientRect();
      return r.left>=0 && r.right<=innerWidth+1 && r.top>=0 && r.bottom<=innerHeight+1;`,
      args: [],
    }),
  );
  assert(primaryVisible, 'The primary cleanup action must be visible before scrolling');
  // CSS-only mixed-source notice: no rule scan, selection or backend state is changed.
  const mixedScopeNoteCss = await command('POST', `/session/${session}/execute/sync`, {
    script: `const note=document.querySelector('.task-dock small');
      const text=note.textContent, attr=note.getAttribute('data-important');
      try {
        note.textContent='包含垃圾扫描中已选的项目'; note.setAttribute('data-important','true');
        const button=[...document.querySelectorAll('.task-dock button')].find(b=>b.textContent.trim()==='清理已选');
        const box=button.getBoundingClientRect();
        return getComputedStyle(note).display!=='none' && note.getBoundingClientRect().height>0
          && box.top>=0 && box.bottom<=innerHeight && box.right<=innerWidth;
      } finally {
        note.textContent=text;
        if(attr===null) note.removeAttribute('data-important'); else note.setAttribute('data-important',attr);
      }`,
    args: [],
  });
  assert(mixedScopeNoteCss, 'Mixed-source notice must remain visible in the fixed action area');
  await writeFile(
    `${output}/manual-selected-analysis.png`,
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
  await click('//button[starts-with(normalize-space(.),"清理已选")]');
  await until(async () =>
    (await command('GET', `/session/${session}/source`)).includes('文件夹可能保留'),
  );
  await click('//button[normalize-space(.)="取消"]');
  await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script: `const previous=arguments[0]; return document.querySelector('.app-shell').dataset.page==='analysis'
      && document.activeElement?.textContent.trim()==='清理已选'
      && document.querySelector('.analysis-panel input[type="search"]')?.value===previous.search
      && document.querySelector('.analysis-list')?.scrollTop===previous.scroll
      && document.querySelector('input[aria-label="选择删除我.txt"]')?.checked;`,
      args: [beforeConfirmation],
    }),
  );
  await access(join(folder, '删除我.txt'));
  await click('//button[starts-with(normalize-space(.),"清理已选")]');
  await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script: 'return !!document.querySelector(\'[role="alertdialog"] .manual-confirm\');',
      args: [],
    }),
  );
  await writeFile(
    `${output}/manual-confirmation.png`,
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
  const manualLayouts = [];
  for (const [width, height] of viewport
    ? [viewport]
    : [
        [760, 580],
        [1100, 780],
      ]) {
    await command('POST', `/session/${session}/window/rect`, { width, height });
    const layout = await command('POST', `/session/${session}/execute/sync`, {
      script: `const dialog = document.querySelector('[role="alertdialog"]');
        const box = dialog.getBoundingClientRect();
        const action=dialog.querySelector('button.danger').getBoundingClientRect();
        return {theme:document.documentElement.dataset.theme,fits:box.left >= 0 && box.right <= innerWidth + 1 && box.top >= 0 && box.bottom <= innerHeight + 1,
          actionVisible:action.top>=box.top && action.bottom<=box.bottom,
          horizontalOverflow:dialog.scrollWidth > dialog.clientWidth + 1,
          scrollable:dialog.scrollHeight > dialog.clientHeight + 1};`,
      args: [],
    });
    assert(
      layout.fits && layout.actionVisible && !layout.horizontalOverflow,
      JSON.stringify(layout),
    );
    if (theme) assert.equal(layout.theme, theme);
    manualLayouts.push({ width, height, ...layout });
    if (width === 760)
      await writeFile(
        `${output}/manual-confirmation-760.png`,
        Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
      );
  }
  await writeFile(join(folder, '确认后新增.txt'), 'never part of the plan');
  await click('//button[normalize-space(.)="永久删除"]');
  const report = await until(async () => {
    const current = await snapshot();
    return current.report?.manual && current.report.status === 'finished' ? current.report : null;
  });
  assert.equal(report.manual.deleted, 1);
  assert.equal(report.directories.deleted, 1);
  assert.equal(report.directories.skipped, 1);
  assert.equal(report.failed, 0);
  assert.equal(report.unknown, 0);
  await assert.rejects(access(join(folder, '删除我.txt')));
  await access(join(folder, '确认后新增.txt'));
  for (const item of protectedBefore) {
    assert.deepEqual(await readFile(item.path), item.content);
    assert.equal((await stat(item.path)).mtimeMs, item.modified);
  }
  await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script:
        'return !!document.querySelector(\'[aria-label="本次结果摘要"]\') && !document.querySelector(\'[role="alertdialog"]\');',
      args: [],
    }),
  );
  assert.equal(
    await command('POST', `/session/${session}/execute/sync`, {
      script: 'return document.querySelector(".cleanup-decision h2")?.textContent;',
      args: [],
    }),
    '清理已结束',
  );
  assert(
    await command('POST', `/session/${session}/execute/sync`, {
      script: `return document.querySelector('.task-dock').hidden
      && ![...document.querySelectorAll('button')].some(b=>b.getClientRects().length && b.textContent.trim()==='重新全面扫描');`,
      args: [],
    }),
  );
  await writeFile(
    `${output}/manual-complete.png`,
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
  await click('//button[normalize-space(.)="完成"]');
  await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script: 'return !!document.querySelector(".home-scan .button");',
      args: [],
    }),
  );
  assert.equal((await snapshot()).report.id, report.id);
  assert.equal((await snapshot()).status.state, 'finished');
  // Zero bytes must still allow a folder-only plan, through the ordinary UI.
  const emptyPath = join(root, 'analysis', '独选空目录');
  await mkdir(emptyPath);
  const emptyControl = (await snapshot()).control;
  const emptyStart = await ipc('start_analysis', {
    token: location.token,
    request: {
      runtime_id: emptyControl.runtime_id,
      expected_revision: emptyControl.revision,
      request_id: crypto.randomUUID(),
    },
  });
  assert(!emptyStart.last_request.error);
  await until(async () => (await snapshot()).analysis.state === 'completed', 90000);
  await refresh();
  await click('//nav/button[normalize-space(.)="空间分析"]');
  await click('//button[normalize-space(.)="目录浏览"]');
  const emptySearch = await command('POST', `/session/${session}/element`, {
    using: 'xpath',
    value: '//input[@placeholder="仅搜索当前层级"]',
  });
  await command(
    'POST',
    `/session/${session}/element/${emptySearch['element-6066-11e4-a52e-4f735466cecf']}/value`,
    { text: '独选空目录' },
  );
  await settleSearch();
  await click('//input[@aria-label="选择独选空目录"]');
  await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script:
        'return document.querySelector(".task-dock")?.textContent.includes("0 个文件 · 1 个文件夹");',
      args: [],
    }),
  );
  await click('//button[starts-with(normalize-space(.),"清理已选")]');
  await click('//button[normalize-space(.)="永久删除"]');
  const emptyReport = await until(async () => {
    const s = await snapshot();
    return s.report?.id !== report.id && s.report?.status === 'finished' ? s.report : null;
  });
  assert.equal(emptyReport.manual.total, 0);
  assert.equal(emptyReport.directories.deleted, 1);
  assert.equal(emptyReport.failed, 0);
  assert.equal(emptyReport.unknown, 0);
  await assert.rejects(access(emptyPath));
  await until(() =>
    command('POST', `/session/${session}/execute/sync`, {
      script: 'return document.querySelector(".cleanup-decision h2")?.textContent === "清理完成";',
      args: [],
    }),
  );
  await writeFile(
    `${output}/empty-folder-complete.png`,
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
  assert(
    await command('POST', `/session/${session}/execute/sync`, {
      script:
        'return !document.body.textContent.includes("已恢复上次清理记录") && !document.body.textContent.includes("请重新选择清理类别");',
      args: [],
    }),
  );
  const evidence = {
    directCleanupFromAnalysis: true,
    mixedScopeNoteCssFixtureVisible: true,
    cancelledConfirmationRestoresAnalysisFocusAndPosition: true,
    completedHidesTaskDockAndRescan: true,
    completedReturnsHomeWithoutNewTask: true,
    resultTitlesAccountForDirectories: true,
    obsoleteRecoveryNoticeCleared: true,
    primaryVisibleWithoutScrolling: true,
    escapeRestoresFocus: true,
    emptyDirectoryOnly: true,
    emptyReport,
    previewReadonly: true,
    preview,
    detailsShareSelection: true,
    synthetic: true,
    realApplicationAcceptance: 'not_run',
    ordinaryWindowsFilesystemBoundary: true,
    manualOnly: report.categories.length === 0,
    oneConfirmation: true,
    cancelledConfirmationPreservedSelection: true,
    newFilePreserved: true,
    protectedSamplesUnchanged: true,
    manualLayouts,
    report,
  };
  if (mixed) {
    const controlled = async (name, args) => {
      const c = (await snapshot()).control;
      const reply = await ipc(name, {
        ...args,
        request: {
          runtime_id: c.runtime_id,
          expected_revision: c.revision,
          request_id: crypto.randomUUID(),
        },
      });
      assert(!reply.last_request.error, JSON.stringify(reply.last_request.error));
      return reply.last_request.value;
    };
    const mixedManual = join(folder, '混合来源.txt');
    const mixedRule = join(root, 'cache', 'manual-mixed.tmp');
    await writeFile(mixedManual, 'manual part');
    await writeFile(mixedRule, 'rule part');
    const old = new Date(Date.now() - 8 * 86400_000);
    await utimes(mixedRule, old, old);
    await controlled('start_analysis', { token: location.token });
    const analyzed = await until(async () => {
      const s = await snapshot();
      return s.analysis.state === 'completed' ? s.analysis : null;
    }, 90_000);
    const files = await ipc('list_analysis_files', {
      session: analyzed.id,
      search: '混合来源.txt',
      offset: 0,
    });
    assert.equal(files.items.length, 1);
    await controlled('start_scan', { ruleIds: ['windows.user_temp'] });
    const scanned = await until(async () => {
      const s = await snapshot();
      return s.status.state === 'ready' ? s : null;
    });
    const plans = await controlled('prepare_cleanup', {
      preparationId: crypto.randomUUID(),
      requests: [
        {
          scope: 'user',
          session: scanned.status.id,
          selection: {
            rule_ids: ['windows.user_temp'],
            excluded_ids: [],
            included_ids: [],
            reviewed_rule_ids: [],
          },
        },
      ],
      manual: { session: analyzed.id, selected_ids: [files.items[0].id], excluded_ids: [] },
    });
    assert.equal(plans.length, 1);
    assert.equal(plans[0].plan.manual.files, 1);
    assert(plans[0].plan.categories.some((c) => c.rule_id === 'windows.user_temp'));
    await controlled('execute_cleanup', { plans });
    const mixedReport = await until(async () => {
      const s = await snapshot();
      return s.report?.id === plans[0].plan.id && s.report.status === 'finished' ? s.report : null;
    });
    assert.equal(mixedReport.manual.deleted, 1);
    assert.equal(
      mixedReport.deleted,
      mixedReport.manual.deleted + mixedReport.categories.reduce((n, c) => n + c.deleted, 0),
    );
    await assert.rejects(access(mixedManual));
    await assert.rejects(access(mixedRule));
    evidence.mixedSources = mixedReport;
  }
  await writeFile(`${output}/manual-cleanup.json`, JSON.stringify(evidence, null, 2) + '\n');
  return evidence;
}
