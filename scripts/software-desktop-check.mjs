// Debug-only synthetic removal. Verifies the native IPC/UI path without touching installed software.
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { spawn } from 'node:child_process';
import { createDesktopCommand } from './desktop-driver.mjs';

function option(name, fallback) {
  const index = process.argv.indexOf(name);
  if (index < 0) return fallback;
  const value = process.argv[index + 1];
  assert(value && !value.startsWith('--'), `Missing value for ${name}`);
  return value;
}
const output = resolve(option('--output-dir', 'artifacts/software-verification'));
const application = resolve(option('--app-executable', 'target/debug/dustaway.exe'));
const reliability = process.argv.includes('--reliability');
const residualScenario = process.argv.includes('--residuals');
const residualReadyText = residualScenario
  ? '残留检查完成，请选择需要清理的项目'
  : '没有发现已支持范围内的残留';
await mkdir(output, { recursive: true });
const root = await mkdtemp(join(resolve('artifacts'), 'dustaway-e2e-software-'));
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
      DUSTAWAY_E2E_SOFTWARE_DELAY_MS: reliability ? '6000' : '0',
      DUSTAWAY_E2E_SOFTWARE_RESIDUALS: residualScenario ? '1' : '0',
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
let session;
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function until(fn) {
  const deadline = Date.now() + 30000;
  let last;
  while (Date.now() < deadline) {
    try {
      const result = await fn();
      if (result) return result;
    } catch (error) {
      last = error;
    }
    await delay(150);
  }
  throw last ?? new Error('Desktop assertion timed out');
}
const evaluate = (script, args = []) =>
  command('POST', `/session/${session}/execute/sync`, { script, args });
async function click(xpath) {
  const element = await until(async () => {
    const matches = await command('POST', `/session/${session}/elements`, {
      using: 'xpath',
      value: xpath,
    });
    for (const match of matches) {
      const id = match['element-6066-11e4-a52e-4f735466cecf'];
      if (
        (await command('GET', `/session/${session}/element/${id}/displayed`)) &&
        (await command('GET', `/session/${session}/element/${id}/enabled`))
      )
        return id;
    }
  });
  await evaluate('arguments[0].scrollIntoView({block:"center"})', [
    { 'element-6066-11e4-a52e-4f735466cecf': element },
  ]);
  await command('POST', `/session/${session}/element/${element}/click`, {});
}
const screenshot = async (name) =>
  writeFile(
    join(output, `${name}.png`),
    Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
  );
async function disclosure(label, open) {
  const expanded = await evaluate(
    'return [...document.querySelectorAll("summary")].find(s=>s.textContent.trim()===arguments[0])?.parentElement.open',
    [label],
  );
  if (expanded !== open) await click(`//summary[normalize-space(.)="${label}"]`);
}
try {
  await until(() => fetch('http://127.0.0.1:4444/status').then((r) => r.ok));
  const created = await command('POST', '/session', {
    capabilities: {
      alwaysMatch: { 'tauri:options': { application } },
    },
  });
  session = created.sessionId;
  await until(() =>
    evaluate('return document.querySelector(".app-shell")?.dataset.runtime === "fixture"'),
  );
  assert.equal(
    await evaluate('return document.querySelector(".app-shell").dataset.runtime'),
    'fixture',
    'Never execute against a real provider',
  );
  await click('//nav/button[normalize-space(.)="软件卸载"]');
  await until(() => evaluate('return document.querySelectorAll(".software-row").length===1'));
  const layouts = [];
  const detailLayouts = [];
  const residualLayouts = [];
  for (const theme of ['light', 'dark']) {
    const desired = theme === 'dark' ? '切换为深色' : '切换为浅色';
    if (
      await evaluate('return !!document.querySelector(`[aria-label="${arguments[0]}"]`)', [desired])
    )
      await click(`//button[@aria-label="${desired}"]`);
    for (const [width, height] of [
      [1100, 780],
      [760, 580],
    ]) {
      await command('POST', `/session/${session}/window/rect`, { width, height });
      await delay(200);
      const layout = await evaluate(
        'const nav=document.querySelector(".chapter-navigation").getBoundingClientRect(), row=document.querySelector(".software-row").getBoundingClientRect(); return {width:innerWidth,height:innerHeight,overflow:document.documentElement.scrollWidth>innerWidth,navRight:nav.right,rowRight:row.right};',
      );
      assert(
        !layout.overflow && layout.navRight <= layout.width && layout.rowRight <= layout.width,
        JSON.stringify(layout),
      );
      layouts.push({ theme, ...layout });
      await screenshot(`software-${theme}-${width}`);
      await click('//button[contains(@class,"software-row")]');
      const primaryVisible = await evaluate(`
        const action=document.querySelector('.software-uninstall-action').getBoundingClientRect();
        return !document.querySelector('.software-detail-more').open && action.top>=0 && action.bottom<=innerHeight;
      `);
      assert.equal(primaryVisible, true, 'Primary uninstall action must precede collapsed details');
      await screenshot(`software-simple-detail-${theme}-${width}`);
      await disclosure('软件信息与清理范围', true);
      await until(() =>
        evaluate('return document.querySelectorAll(".software-capabilities tbody tr").length===3'),
      );
      const detail = await evaluate(`
        const panel=document.querySelector('.software-detail'), table=panel.querySelector('.software-capabilities');
        table.scrollIntoView({block:'center'});
        const rect=table.getBoundingClientRect();
        return {width:innerWidth,height:innerHeight,overflow:panel.scrollWidth>panel.clientWidth,tableLeft:rect.left,tableRight:rect.right,
          kinds:[...table.querySelectorAll('tbody th')].map(n=>n.textContent),
          missing:panel.querySelector('.software-missing-information')?.textContent,
          dataImpact:panel.querySelector('.software-capability-note')?.textContent};
      `);
      assert(
        !detail.overflow && detail.tableLeft >= 0 && detail.tableRight <= detail.width,
        JSON.stringify(detail),
      );
      assert.deepEqual(detail.kinds, ['缓存与日志', '设置', '本地资料']);
      assert(detail.missing.includes('安装日期、安装位置'));
      assert(detail.dataImpact.includes('登录状态、浏览记录、离线内容或文档备份'));
      detailLayouts.push({ theme, ...detail });
      await screenshot(`software-capabilities-${theme}-${width}`);
      await click('//button[@aria-label="关闭软件详情"]');
      await until(() =>
        evaluate('return document.activeElement?.classList.contains("software-row")'),
      );
    }
  }
  await click('//button[contains(@class,"software-row")]');
  await screenshot('software-details-760');
  await click('//button[normalize-space(.)="卸载此软件"]');
  await click('//button[normalize-space(.)="返回检查"]');
  assert.equal(await evaluate('return document.querySelectorAll(".software-row").length'), 1);
  await until(() => evaluate('return document.activeElement?.classList.contains("software-row")'));
  await command('POST', `/session/${session}/actions`, {
    actions: [
      {
        type: 'key',
        id: 'keyboard',
        actions: [
          { type: 'keyDown', value: '\uE007' },
          { type: 'keyUp', value: '\uE007' },
        ],
      },
    ],
  });
  await click('//button[normalize-space(.)="卸载此软件"]');
  if (reliability) {
    await evaluate(`
      const original=window.__TAURI_INTERNALS__.invoke.bind(window.__TAURI_INTERNALS__);
      const originalFetch=window.fetch.bind(window);
      window.__dustawayTest={original,executeCalls:0,reconciles:0,lost:false};
      window.fetch=function(url,options){
        const test=window.__dustawayTest;
        const command=String(url).split('/').pop();
        if(command==='reconcile_task_request') test.reconciles++;
        if(command==='execute_uninstall'){
          if(!test.injectedRepeat) test.executeCalls++;
          if(!test.lost){
            test.lost=true;
            test.request=JSON.parse(options.body);
            test.startedAt=Date.now();
            originalFetch(url,options).then(response=>response.json()).then(reply=>{test.receipt=reply},error=>{test.error=error});
            return new Promise(()=>{}); // Lose the reply after actual native acceptance.
          }
        }
        return originalFetch(url,options);
      };`);
  }
  await click('//button[normalize-space(.)="确认卸载"]');
  if (reliability) {
    await until(() => evaluate('return !!window.__dustawayTest.receipt'));
    const duplicate = await evaluate(`
      const test=window.__dustawayTest;
      test.injectedRepeat=true;
      const duplicate=test.original('execute_uninstall',test.request);
      test.injectedRepeat=false;
      return duplicate.then(reply=>({
        sameRequest:reply.last_request.request_id===test.receipt.last_request.request_id,
        sameTask:reply.last_request.value===test.receipt.last_request.value,
        error:reply.last_request.error,
      }));`);
    assert.deepEqual(duplicate, { sameRequest: true, sameTask: true, error: null });
    const busy = await evaluate(
      `return window.__dustawayTest.original('list_installed_software').then(()=>false,()=>true)`,
    );
    assert.equal(busy, true, 'Native list must refuse refresh while uninstall is running');
    await click('//nav/button[normalize-space(.)="历史"]');
    await until(() =>
      evaluate('return document.querySelector(".app-shell")?.textContent.includes("隔离测试软件")'),
    );
    await screenshot('software-active-other-page');
    await click('//nav/button[normalize-space(.)="软件卸载"]');
    await until(() => evaluate('return window.__dustawayTest.reconciles>0'));
    assert.equal(await evaluate('return window.__dustawayTest.executeCalls'), 1);
    assert(await evaluate('return Date.now()-window.__dustawayTest.startedAt>=15000'));
  }
  await until(() =>
    evaluate(
      'return document.querySelector(".software-task")?.textContent.includes(arguments[0])',
      [residualReadyText],
    ),
  );
  assert.equal(await evaluate('return document.querySelectorAll(".software-row").length'), 0);
  const firstScan = await evaluate(
    'return window.__TAURI_INTERNALS__.invoke("get_uninstall_task")',
  );
  assert.equal(firstScan.can_scan_residuals, true);
  assert.equal(firstScan.uninstall_status, 'removed');
  assert.equal(typeof firstScan.residual_scan_id, 'string');
  if (residualScenario) {
    await until(() =>
      evaluate('return document.querySelectorAll(".residual-row input").length===4'),
    );
    await click('//input[@type="checkbox" and contains(@aria-label,"Preferences")]');
    assert.equal(
      await evaluate('return document.querySelectorAll(".residual-row input:checked").length'),
      1,
    );
  }
  await disclosure('查看处理详情', true);
  await click('//button[normalize-space(.)="重新检查残留"]');
  const secondScan = await until(async () => {
    const task = await evaluate('return window.__TAURI_INTERNALS__.invoke("get_uninstall_task")');
    return task?.phase === 'residuals_ready' &&
      task.residual_scan_id &&
      task.residual_scan_id !== firstScan.residual_scan_id
      ? task
      : null;
  });
  assert.equal(secondScan.id, firstScan.id, 'Rescan must keep the original uninstall task');
  const residuals = await evaluate(
    'return window.__TAURI_INTERNALS__.invoke("list_uninstall_residuals", {taskId:arguments[0],offset:0,search:""})',
    [secondScan.id],
  );
  assert.equal(residuals.scan_id, secondScan.residual_scan_id);
  assert.equal(residuals.task_id, secondScan.id);
  await disclosure('查看处理详情', false);
  if (residualScenario) {
    assert.equal(residuals.total, 4);
    await until(() =>
      evaluate('return document.querySelectorAll(".residual-row input").length===4'),
    );
    assert.equal(
      await evaluate('return document.querySelectorAll(".residual-row input:checked").length'),
      0,
    );
    assert.equal(
      await evaluate('return document.querySelectorAll(".residual-row input:disabled").length'),
      1,
    );
    for (const theme of ['light', 'dark']) {
      const label = theme === 'dark' ? '切换为深色' : '切换为浅色';
      if (
        await evaluate('return !!document.querySelector(`[aria-label="${arguments[0]}"]`)', [label])
      ) {
        await click(`//button[@aria-label="${label}"]`);
      }
      for (const [width, height] of [
        [1100, 780],
        [760, 580],
      ]) {
        await command('POST', `/session/${session}/window/rect`, { width, height });
        await delay(200);
        const layout = await evaluate(`
          const task = document.querySelector('.software-task').getBoundingClientRect();
          const actions = document.querySelector('.software-cleanup-actions').getBoundingClientRect();
          const rows = [...document.querySelectorAll('.residual-row')].filter(r => r.getClientRects().length && !r.closest('details:not([open])'));
          return {width: innerWidth, height: innerHeight,
            overflow: document.documentElement.scrollWidth > innerWidth,
            taskRight: task.right,
            actionsOverlapRows: rows.some(r => {const rect=r.getBoundingClientRect(); return rect.bottom > actions.top && rect.top < actions.bottom;}),
            rowsFit: rows.every(r => r.getBoundingClientRect().right <= task.right)};
        `);
        assert(
          !layout.overflow &&
            layout.taskRight <= layout.width &&
            layout.rowsFit &&
            !layout.actionsOverlapRows,
          JSON.stringify(layout),
        );
        residualLayouts.push({ theme, ...layout });
        await screenshot(`software-residuals-${theme}-${width}`);
      }
    }
    // Select the parent, frozen cache file and data file. The read-only sibling stays unselected.
    for (const record of residuals.items.filter((item) => item.selectable)) {
      await click(`//input[@aria-label="选择 ${record.path}"]`);
    }
    await click('//button[normalize-space(.)="清理已选残留"]');
    await until(() =>
      evaluate(
        'return document.querySelector("[role=alertdialog]")?.textContent.includes("永久删除已选残留")',
      ),
    );
    const confirmation = await evaluate(
      'return document.querySelector("[role=alertdialog]").textContent',
    );
    assert(confirmation.includes('2 个文件、1 个空文件夹'));
    assert(confirmation.includes('不会放入回收站'));
    assert(confirmation.includes('可能失去登录状态、浏览记录、离线内容或文档备份'));
    for (const record of residuals.items.filter((item) => item.selectable))
      assert(confirmation.includes(record.path));
    assert(!confirmation.includes('keep.txt'));
    await screenshot('software-residual-confirmation-760');
    await click('//button[normalize-space(.)="返回检查"]');
    await until(() =>
      evaluate('return document.activeElement?.textContent.includes("清理已选残留")'),
    );
    const beforeDelete = await evaluate(
      'return window.__TAURI_INTERNALS__.invoke("get_uninstall_task")',
    );
    assert.equal(beforeDelete.residual.deleted_files, 0);
    assert.equal(beforeDelete.residual.deleted_bytes, '0');
    await click('//button[normalize-space(.)="清理已选残留"]');
    await click('//button[normalize-space(.)="确认永久删除"]');
    const cleaned = await until(async () => {
      const current = await evaluate(
        'return window.__TAURI_INTERNALS__.invoke("get_uninstall_task")',
      );
      return current?.phase === 'finished' ? current : null;
    });
    assert.equal(cleaned.uninstall_status, 'removed');
    assert.equal(cleaned.residual.deleted_files, 2);
    assert.equal(cleaned.residual.deleted_directories, 1);
    assert.equal(cleaned.residual.deleted_bytes, '1280');
    assert.equal(cleaned.residual.failed + cleaned.residual.skipped + cleaned.residual.unknown, 0);
    assert.equal(cleaned.software.estimated_bytes, '10240');
    await until(() =>
      evaluate(
        'return document.querySelector(".software-task")?.textContent.includes("实际删除 2 个文件、1 个空文件夹")',
      ),
    );
  }
  await screenshot('software-completed-760');
  if (residualScenario) {
    await click('//button[normalize-space(.)="返回软件列表"]');
    await until(() =>
      evaluate(
        'return document.activeElement?.type==="search" && !document.querySelector(".software-task")',
      ),
    );
    await screenshot('software-back-to-list-760');
    await click('//button[normalize-space(.)="查看处理结果"]');
    await until(() =>
      evaluate('return document.activeElement?.classList.contains("software-task")'),
    );
    assert.equal(
      (await evaluate('return window.__TAURI_INTERNALS__.invoke("get_uninstall_task")')).residual
        .deleted_bytes,
      '1280',
    );
  }
  await click('//nav/button[normalize-space(.)="历史"]');
  await click('//button[@aria-label="卸载记录"]');
  await until(() =>
    evaluate(
      'return document.querySelector(".receipt-archive")?.textContent.includes("隔离测试软件")',
    ),
  );
  const savedHistory = await evaluate(
    'return window.__TAURI_INTERNALS__.invoke("get_uninstall_history")',
  );
  assert.equal(savedHistory.length, 1);
  assert.equal(savedHistory[0].can_scan_residuals, false);
  assert.equal(savedHistory[0].residual_scan_id, null);
  assert.equal(savedHistory[0].uninstall_status, 'removed');
  if (residualScenario) assert.equal(savedHistory[0].residual.deleted_bytes, '1280');
  await screenshot('software-history-760');
  if (reliability) {
    const history = await evaluate(
      'return window.__dustawayTest.original("get_uninstall_history")',
    );
    assert.equal(history.length, 1, 'Lost response must not create another uninstall');
    await command('POST', `/session/${session}/refresh`, {});
    await until(() =>
      evaluate('return document.querySelector(".app-shell")?.dataset.runtime==="fixture"'),
    );
    await click('//nav/button[normalize-space(.)="软件卸载"]');
    await until(() =>
      evaluate(
        'return document.querySelector(".software-task")?.textContent.includes(arguments[0])',
        [residualScenario ? '实际删除 2 个文件、1 个空文件夹' : residualReadyText],
      ),
    );
    assert.equal(await evaluate('return document.querySelectorAll(".software-row").length'), 0);
    assert.equal(
      (await evaluate('return window.__TAURI_INTERNALS__.invoke("get_uninstall_history")')).length,
      1,
    );
    await screenshot('software-after-reconnect');
  }
  if (residualScenario) {
    await click('//nav/button[normalize-space(.)="软件卸载"]');
    await disclosure('查看处理详情', true);
    await click('//button[normalize-space(.)="重新检查残留"]');
    await until(() =>
      evaluate(
        'return document.querySelector(".software-task")?.textContent.includes("所有项目均已保留")',
      ),
    );
    await until(() =>
      evaluate('return document.querySelectorAll(".residual-row input").length===1'),
    );
    const retained = await evaluate(
      'return window.__TAURI_INTERNALS__.invoke("list_uninstall_residuals", {taskId:arguments[0], offset:0, search:""})',
      [secondScan.id],
    );
    assert.equal(retained.total, 1);
    assert.equal(retained.items[0].id, 'shared');
    assert.equal(retained.items[0].selectable, false);
    assert.equal(
      await evaluate('return document.querySelectorAll(".residual-row input:checked").length'),
      0,
    );
    await screenshot('software-residual-retained-760');
  }
  await writeFile(
    join(output, 'verification.json'),
    JSON.stringify(
      {
        passed: true,
        provider: 'synthetic fixture',
        nativeUninstallExecuted: false,
        realFileDeletionExecuted: false,
        layouts,
        detailLayouts,
        residualLayouts,
        confirmationCancelled: true,
        keyboardAndFocusRestore: true,
        requestAndReport: true,
        rescanReplacesManifestOnSameTask: true,
        liveResidualCapabilityNotPersisted: true,
        lostResponseRecoveredWithoutReplay: reliability,
        duplicateRequestReturnedSameTask: reliability,
        activeTaskVisibleAcrossPages: reliability,
        webviewReloadRecoveredExistingTask: reliability,
        standardUninstallResultPreserved: true,
        syntheticResidualSelectionAndCleanup: residualScenario,
        dataUnselectedAndSharedDisabled: residualScenario,
        rescanClearsSelection: residualScenario,
        cleanupCancelRestoresFocus: residualScenario,
        selectedParentChildCountedOnce: residualScenario,
        estimatedSizeExcludedFromDeletedBytes: residualScenario,
        primaryUninstallVisibleBeforeDetails: true,
        technicalDetailsCollapsedByDefault: true,
        returnToListAndReopenWithoutReplay: residualScenario,
        readonlySurvivesCleanupAndRescan: residualScenario,
        fixture: root,
      },
      null,
      2,
    ),
  );
  console.log(
    'PASS: software list, four layouts, confirmation, synthetic uninstall, residual inspection, history.',
  );
} catch (error) {
  if (session) {
    await screenshot('failure').catch(() => {});
    await writeFile(
      join(output, 'failure.html'),
      await command('GET', `/session/${session}/source`),
    ).catch(() => {});
  }
  console.error(logs.slice(-3000));
  throw error;
} finally {
  if (session) await command('DELETE', `/session/${session}`).catch(() => {});
  driver.kill();
}
