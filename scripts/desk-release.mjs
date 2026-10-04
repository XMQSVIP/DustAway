import { createDesktopCommand } from './desktop-driver.mjs';
// Read-only packaged-build inspection. No scan, plan, cleanup or settings mutation.
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { spawn } from 'node:child_process';
import assert from 'node:assert/strict';
const { version } = JSON.parse(await readFile('package.json', 'utf8'));
const output = resolve(process.env.DUSTAWAY_UI_OUTPUT ?? 'artifacts/sculpture-release-' + version);
await mkdir(output, { recursive: true });
const app = resolve(
  process.env.DUSTAWAY_UI_APP ?? `artifacts/portable/拂尘_${version}_windows-x64/拂尘.exe`,
);
const driver = spawn(
  'tauri-driver',
  ['--native-driver', resolve('artifacts/webdriver/msedgedriver.exe')],
  { windowsHide: true, stdio: 'ignore' },
);
let session;
const pause = (ms) => new Promise((r) => setTimeout(r, ms));
const request = createDesktopCommand();
async function until(fn) {
  const end = Date.now() + 30000;
  let error;
  while (Date.now() < end) {
    try {
      const v = await fn();
      if (v) return v;
    } catch (e) {
      error = e;
    }
    await pause(200);
  }
  throw error ?? Error('Timed out');
}
const js = (script, ...args) =>
  request('POST', `/session/${session}/execute/sync`, { script, args });
const ipc = (name, args = {}) =>
  js('return window.__TAURI_INTERNALS__.invoke(arguments[0], arguments[1]);', name, args);
async function clickNav(label) {
  const matches = await request('POST', `/session/${session}/elements`, {
    using: 'xpath',
    value: `//div[@class="chapter-navigation"]/nav/button[normalize-space(.)="${label}"]`,
  });
  const id = Object.values(matches[0])[0];
  await request('POST', `/session/${session}/element/${id}/click`, {});
  await pause(280);
}
async function screenshot(name) {
  await writeFile(
    join(output, name + '.png'),
    Buffer.from(await request('GET', `/session/${session}/screenshot`), 'base64'),
  );
}
const layouts = [];
const emulated = [];
try {
  await until(async () => (await fetch('http://127.0.0.1:4444/status')).ok);
  session = (
    await request('POST', '/session', {
      capabilities: { alwaysMatch: { 'tauri:options': { application: app } } },
    })
  ).sessionId;
  await until(() => js('return !!document.querySelector(".home-scan button:not(:disabled)");'));
  const boot = await ipc('bootstrap');
  assert.equal(boot.version, version);
  assert.equal(
    await js('return document.querySelector(".app-shell").dataset.runtime;'),
    'standard',
  );
  assert.equal(boot.rules.length, 23);
  assert.equal(boot.capabilities.system_cleanup, false);
  const before = await ipc('get_runtime_snapshot', { scope: 'user' });
  await js('document.documentElement.dataset.motion="system";');
  const motionSample = () =>
    js(
      `return [...document.querySelectorAll('.sculpture-home .sculpture-layer')].map(e=>({name:e.className,transform:getComputedStyle(e).transform,animation:getComputedStyle(e).animationName}));`,
    );
  const motionBefore = await motionSample();
  await new Promise((resolve) => setTimeout(resolve, 350));
  const motionAfter = await motionSample();
  const osReduced = await js('return matchMedia("(prefers-reduced-motion: reduce)").matches;');
  assert.equal(motionAfter.length, 4);
  assert.equal(motionAfter[0].transform, 'none');
  if (!osReduced)
    for (let i = 1; i < 4; i++)
      assert.notEqual(motionBefore[i].transform, motionAfter[i].transform);
  await js('document.documentElement.dataset.motion="reduced";');
  const reducedMotion = await motionSample();
  assert(reducedMotion.every((layer) => layer.animation === 'none' && layer.transform === 'none'));
  const animationCheck = { osReduced, motionBefore, motionAfter, reducedMotion };
  const inspect = () =>
    js(
      `const page=document.querySelector('.app-shell').dataset.page, main=document.querySelector('main'), r=main.getBoundingClientRect(), b=document.querySelector('.home-scan button')?.getBoundingClientRect(); return {page,width:innerWidth,height:innerHeight,dpi:devicePixelRatio,overflow:document.documentElement.scrollWidth>innerWidth+1||main.scrollWidth>main.clientWidth+1,mainFits:r.bottom<=innerHeight+1,mainScrollHeight:main.scrollHeight,mainClientHeight:main.clientHeight,scanVisible:page!=='clean'||(b.top>=r.top&&b.bottom<=r.bottom),focusOutline:getComputedStyle(document.activeElement).outlineStyle};`,
    );
  for (const theme of ['light', 'dark']) {
    await js(
      'document.documentElement.dataset.theme=arguments[0];document.documentElement.dataset.motion="reduced";',
      theme,
    );
    for (const [width, height] of [
      [1100, 780],
      [760, 580],
    ]) {
      await request('POST', `/session/${session}/window/rect`, { width, height });
      for (const page of ['清理', '空间分析', '历史', '设置']) {
        await clickNav(page);
        await js('document.querySelector("main").scrollTop=0;');
        const layout = await inspect();
        assert(!layout.overflow && layout.mainFits && layout.scanVisible, JSON.stringify(layout));
        layouts.push({ theme, ...layout });
        await screenshot(`${layout.page}-${theme}-${width}`);
      }
    }
  }
  // Chromium emulation checks 100/125/150% reflow, respecting the native 760×580 minimum.
  // It does not change Windows settings or certify a different monitor DPI.
  let scaleCheck;
  try {
    for (const scale of [1, 1.25, 1.5]) {
      await request('POST', `/session/${session}/ms/cdp/execute`, {
        cmd: 'Emulation.setDeviceMetricsOverride',
        params: {
          width: Math.max(760, Math.round(1100 / scale)),
          height: Math.max(580, Math.round(780 / scale)),
          deviceScaleFactor: scale,
          mobile: false,
        },
      });
      for (const page of ['清理', '空间分析', '历史', '设置']) {
        await clickNav(page);
        await js('document.querySelector("main").scrollTop=0;');
        const layout = await inspect();
        assert(!layout.overflow && layout.mainFits && layout.scanVisible, JSON.stringify(layout));
        emulated.push({ scale, ...layout });
      }
    }
    scaleCheck = 'Chromium device emulation passed; native monitor DPI unchanged';
  } catch (e) {
    scaleCheck = 'Device emulation unavailable or failed: ' + e.message;
  }
  await request('POST', `/session/${session}/ms/cdp/execute`, {
    cmd: 'Emulation.clearDeviceMetricsOverride',
    params: {},
  }).catch(() => {});
  const after = await ipc('get_runtime_snapshot', { scope: 'user' });
  assert.equal(
    after.control.revision,
    before.control.revision,
    'Read-only check changed backend revision',
  );
  assert.equal(after.status?.id, before.status?.id);
  const checked = {
    version,
    packagedExecutable: app,
    passed: true,
    nativeLayouts: layouts,
    emulatedLayouts: emulated,
    scaleCheck,
    animationCheck,
    backendRevisionUnchanged: true,
    scanStarted: false,
    cleanupStarted: false,
  };
  await writeFile(join(output, 'result.json'), JSON.stringify(checked, null, 2));
  console.log(JSON.stringify(checked, null, 2));
} catch (e) {
  if (session) await screenshot('failure').catch(() => {});
  throw e;
} finally {
  if (session) await request('DELETE', `/session/${session}`).catch(() => {});
  driver.kill();
}
