import { createDesktopCommand } from './desktop-driver.mjs';
// Focused, read-only desktop check. Reuses only the marked fixture from a completed E2E run.
import assert from 'node:assert/strict';
import { readFile, writeFile, realpath, mkdir } from 'node:fs/promises';
import { dirname, resolve, basename } from 'node:path';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { pathToFileURL } from 'node:url';

export async function captureApplicationGroup(command, session, appId, output) {
  await command('POST', `/session/${session}/execute/sync`, {
    script:
      'const d=document.querySelector(".retained-results"); if(d&&!d.open)d.querySelector("summary").click();',
    args: [],
  });
  const layouts = [];
  for (const [width, height] of [
    [1100, 780],
    [760, 580],
  ]) {
    await command('POST', `/session/${session}/window/rect`, { width, height });
    const state = await command('POST', `/session/${session}/execute/sync`, {
      script: `const row=document.querySelector('.retained-results .rule-row[data-rule^="'+arguments[0]+'."]'); const group=row?.closest('.purpose-group');
        if(!group) throw new Error('Missing application group');
        const toggle=group.querySelector('.purpose-toggle');
        if(toggle?.getAttribute('aria-expanded')==='false') toggle.click();
        return !!group;`,
      args: [appId],
    });
    assert(state);
    await new Promise((r) => setTimeout(r, 150));
    const layout = await command('POST', `/session/${session}/execute/sync`, {
      script: `const row=document.querySelector('.retained-results .rule-row[data-rule^="'+arguments[0]+'."]'); const group=row?.closest('.purpose-group');
        group.scrollIntoView({block:'start',behavior:'instant'});
        const heading=group.querySelector('.purpose-heading').getBoundingClientRect();
        const main=document.querySelector('.main-content').getBoundingClientRect();
        const dockElement=document.querySelector('.task-dock'), dockTop=dockElement.getClientRects().length?dockElement.getBoundingClientRect().top:innerHeight;
        const headingVisible=heading.top>=main.top-1&&heading.bottom<dockTop;
        // Several applications now share one purpose group. Check its heading,
        // then scroll to this application's row instead of requiring every row
        // to fit next to that heading in the same viewport.
        row.scrollIntoView({block:'center',behavior:'instant'});
        const rect=row.getBoundingClientRect();
        return {width:innerWidth,height:innerHeight,overflow:document.documentElement.scrollWidth>innerWidth,
          headingVisible, headingAndRowCheckedSeparately:true,
          rowVisible:rect.top>=main.top-1&&rect.bottom<=dockTop+1,
          readonly:!!row?.querySelector('.readonly-label'),locked:!!row?.querySelector('.rule-lock'),
          versionRestricted:!!row?.textContent.includes('当前应用版本或资料位置暂不支持清理'),
          selectable:!!group.querySelector('input[type="checkbox"]')};`,
      args: [appId],
    });
    assert(
      !layout.overflow &&
        layout.headingVisible &&
        layout.rowVisible &&
        layout.locked &&
        (layout.readonly || layout.versionRestricted) &&
        !layout.selectable,
      JSON.stringify({ appId, ...layout }),
    );
    layouts.push({ appId, ...layout });
    await writeFile(
      `${output}/${appId}-group-${width}.png`,
      Buffer.from(await command('GET', `/session/${session}/screenshot`), 'base64'),
    );
  }
  await command('POST', `/session/${session}/window/rect`, { width: 1100, height: 780 });
  return layouts;
}

async function main() {
  const ruleIds = [
    'weixin.logs',
    'dingtalk.http',
    'dingtalk.code',
    'baidu_netdisk.http',
    'baidu_netdisk.code',
    'baidu_netdisk.gpu',
    'quark_netdisk.code',
    'quark_netdisk.gpu',
    'thunder.code',
    'thunder.gpu',
  ];
  const output = resolve(process.env.DUSTAWAY_E2E_OUTPUT ?? 'artifacts/cleanup-preparation-e2e');
  const currentBuild = process.argv.includes('--current-fixture-build');
  const captures = resolve(process.env.DUSTAWAY_APPLICATION_OUTPUT ?? output);
  if (currentBuild) {
    assert.notEqual(captures, output, 'Preserve the original E2E evidence');
    await mkdir(captures); // New read-only evidence must not overwrite an earlier run.
  }
  const report = JSON.parse(await readFile(`${output}/desktop-e2e-0.18.0.json`, 'utf8'));
  assert(
    report.passed && ['cleanup-preparation', 'manual-cleanup'].includes(report.developmentStage),
  );
  const root = await realpath(report.fixture);
  assert.equal(dirname(root).toLowerCase(), (await realpath('artifacts')).toLowerCase());
  assert(basename(root).startsWith('dustaway-e2e-'));
  assert.equal(await readFile(resolve(root, '.dustaway-test-only'), 'utf8'), 'fixture-only');
  const executable = resolve('target/debug/dustaway.exe');
  const executableSha256 = createHash('sha256')
    .update(await readFile(executable))
    .digest('hex')
    .toUpperCase();
  if (!currentBuild) assert.equal(executableSha256, report.executableSha256);
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
      stdio: 'ignore',
    },
  );
  let session;
  const command = createDesktopCommand();
  const until = async (fn) => {
    const deadline = Date.now() + 45000;
    while (Date.now() < deadline) {
      try {
        if (await fn()) return;
      } catch {}
      await new Promise((r) => setTimeout(r, 150));
    }
    throw new Error('Desktop check timed out');
  };
  try {
    await until(() => command('GET', '/status'));
    session = (
      await command('POST', '/session', {
        capabilities: { alwaysMatch: { 'tauri:options': { application: executable } } },
      })
    ).sessionId;
    await until(() =>
      command('POST', `/session/${session}/execute/sync`, {
        script: 'return !!window.__TAURI_INTERNALS__;',
        args: [],
      }),
    );
    // The follow-up mode may use a newer binary, but must still run against the
    // marked fixture. Refuse an ordinary build before issuing even a scan.
    const boot = await command('POST', `/session/${session}/execute/sync`, {
      script: "return window.__TAURI_INTERNALS__.invoke('bootstrap');",
      args: [],
    });
    assert(boot.version.endsWith('+e2e-fixture'), 'Refusing to scan a non-fixture build');
    await until(() =>
      command('POST', `/session/${session}/execute/sync`, {
        script: 'return document.querySelector(".app-shell")?.dataset.runtime === "fixture";',
        args: [],
      }),
    );
    let capabilities;
    if (currentBuild) {
      capabilities = await command('POST', `/session/${session}/execute/sync`, {
        script: "return window.__TAURI_INTERNALS__.invoke('get_application_support');",
        args: [],
      });
      assert.equal(capabilities.flatMap((app) => app.rules).length, 18);
      for (const rule of capabilities.flatMap((app) => app.rules)) {
        const accepted = ['dingtalk.http', 'dingtalk.code'].includes(rule.rule_id);
        assert.equal(rule.cleanup_validated, accepted);
        assert.deepEqual(rule.cleanup_versions, accepted ? ['8.2.0.119'] : []);
        assert.notEqual(rule.cleanup_block, null, 'Fixture host must not expose live cleanup');
      }
    }
    await command('POST', `/session/${session}/execute/sync`, {
      script: `const ruleIds=arguments[0]; return window.__TAURI_INTERNALS__.invoke('get_runtime_snapshot',{scope:'user'}).then(s=>window.__TAURI_INTERNALS__.invoke('start_scan',{ruleIds,request:{runtime_id:s.control.runtime_id,expected_revision:s.control.revision,request_id:crypto.randomUUID()}}));`,
      args: [ruleIds],
    });
    await until(async () => {
      const s = await command('POST', `/session/${session}/execute/sync`, {
        script: "return window.__TAURI_INTERNALS__.invoke('get_runtime_snapshot',{scope:'user'});",
        args: [],
      });
      return s.status.state === 'ready' && s.summary?.rules.length === ruleIds.length;
    });
    await command('POST', `/session/${session}/refresh`, {});
    await until(() =>
      command('POST', `/session/${session}/execute/sync`, {
        script: `const tab=[...document.querySelectorAll('nav button')].find(b=>b.textContent.trim()==='清理'); if(!tab)return false; tab.click(); return true;`,
        args: [],
      }),
    );
    await until(() =>
      command('POST', `/session/${session}/execute/sync`, {
        script: 'return !!document.querySelector(\'.cleanup-group[data-app-id="baidu_netdisk"]\');',
        args: [],
      }),
    );
    const layouts = [];
    for (const id of ['weixin', 'dingtalk', 'baidu_netdisk', 'quark_netdisk', 'thunder'])
      layouts.push(...(await captureApplicationGroup(command, session, id, captures)));
    await writeFile(
      `${captures}/application-group-layouts.json`,
      JSON.stringify(
        {
          executableSha256,
          fixtureE2ESha256: report.executableSha256,
          currentBuild,
          exactCapabilitiesChecked: !!capabilities,
          readonly: true,
          passed: true,
          layouts,
        },
        null,
        2,
      ) + '\n',
    );
    console.log(
      'PASS: application group headings, readonly rows and controls visible at both window sizes',
    );
  } finally {
    if (session) await command('DELETE', `/session/${session}`).catch(() => {});
    driver.kill();
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href)
  await main();
