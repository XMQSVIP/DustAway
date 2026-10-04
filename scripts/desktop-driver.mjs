// Shared transport for Windows desktop tests. WebDriver owns page interaction;
// native window sizing must target the owning Tauri HWND, not a Chromium child.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const exec = promisify(execFile);
const probe = fileURLToPath(new URL('./window-state.ps1', import.meta.url));

export function assertMatchingDriver(capabilities) {
  const runtime = capabilities?.browserVersion;
  const driver = capabilities?.msedge?.msedgedriverVersion?.split(' ')[0];
  assert(
    /^\d+\.\d+\.\d+\.\d+$/.test(runtime ?? '') &&
      /^\d+\.\d+\.\d+\.\d+$/.test(driver ?? '') &&
      runtime.split('.').slice(0, 3).join('.') === driver.split('.').slice(0, 3).join('.'),
    `WebView2 ${runtime} / EdgeDriver ${driver}: install a matching driver in artifacts/webdriver/msedgedriver.exe before testing`,
  );
}

export async function readNativeWindow(executable, size) {
  const { stdout } = await exec(
    'powershell.exe',
    [
      '-NoProfile',
      '-ExecutionPolicy',
      'Bypass',
      '-File',
      probe,
      '-Executable',
      resolve(executable),
      ...(size ? ['-Width', String(size.width), '-Height', String(size.height)] : []),
    ],
    { windowsHide: true, timeout: 15000, encoding: 'utf8' },
  );
  return JSON.parse(stdout.replace(/^\uFEFF/, '').trim());
}

export function assertNativeViewport(state, size) {
  assert(state.running && !state.minimized, 'Desktop test window is not visible');
  assert(state.dpi > 0 && state.client?.width > 0 && state.client?.height > 0);
  if (size) {
    assert(
      Math.abs(state.client.width - Math.round((size.width * state.dpi) / 96)) <= 1,
      'Native client width differs from requested size',
    );
    assert(
      Math.abs(state.client.height - Math.round((size.height * state.dpi) / 96)) <= 1,
      'Native client height differs from requested size',
    );
  }
  const views = state.children.filter((child) =>
    ['WRY_WEBVIEW', 'Chrome_WidgetWin_1', 'Chrome_RenderWidgetHostHWND'].includes(child.className),
  );
  assert(
    views.some((view) => view.className === 'Chrome_WidgetWin_1'),
    'WebView2 render window was not found',
  );
  for (const view of views) {
    assert(
      Math.abs(view.x) <= 1 &&
        Math.abs(view.y) <= 1 &&
        Math.abs(view.width - state.client.width) <= 1 &&
        Math.abs(view.height - state.client.height) <= 1,
      `WebView is displaced or clipped inside the native window: ${JSON.stringify(view)}`,
    );
  }
}

export function createDesktopCommand({ base = 'http://127.0.0.1:4444' } = {}) {
  const applications = new Map();
  async function raw(method, path, data) {
    const response = await fetch(base + path, {
      method,
      headers: { 'content-type': 'application/json' },
      body: data === undefined ? undefined : JSON.stringify(data),
      signal: AbortSignal.timeout(180000),
    });
    const json = await response.json();
    if (!response.ok || json.value?.error) throw Error(JSON.stringify(json));
    return json.value;
  }
  return async function command(method, path, data) {
    const resize = /^\/session\/([^/]+)\/window\/rect$/.exec(path);
    if (method === 'POST' && resize) {
      assert(
        Number.isInteger(data?.width) && Number.isInteger(data?.height),
        'Both logical dimensions are required',
      );
      assert(
        data.x === undefined && data.y === undefined,
        'Desktop resize must preserve window position',
      );
      const executable = applications.get(resize[1]);
      assert(executable, 'Resize requires an application owned by this test session');
      const before = await readNativeWindow(executable);
      let state = await readNativeWindow(executable, data);
      const deadline = Date.now() + 5000;
      while (true) {
        try {
          assertNativeViewport(state, data);
          assert.equal(state.handle, before.handle, 'Window changed while resizing');
          assert.equal(
            state.client.x,
            before.client.x,
            'Resize moved the native window horizontally',
          );
          assert.equal(
            state.client.y,
            before.client.y,
            'Resize moved the native window vertically',
          );
          const viewport = await raw('POST', `/session/${resize[1]}/execute/sync`, {
            script: 'return {width:innerWidth,height:innerHeight,dpi:devicePixelRatio};',
            args: [],
          });
          assert(
            Math.abs(viewport.width - data.width) <= 1 &&
              Math.abs(viewport.height - data.height) <= 1,
            'DOM viewport did not follow native resize',
          );
          assert(
            Math.abs(viewport.dpi - state.dpi / 96) < 0.01,
            'Unexpected browser zoom or device emulation',
          );
          return {
            x: state.client.x / viewport.dpi,
            y: state.client.y / viewport.dpi,
            width: viewport.width,
            height: viewport.height,
          };
        } catch (error) {
          if (Date.now() >= deadline) throw error;
          await new Promise((done) => setTimeout(done, 100));
          state = await readNativeWindow(executable);
        }
      }
    }
    const value = await raw(method, path, data);
    if (method === 'POST' && path === '/session') {
      try {
        assertMatchingDriver(value.capabilities);
        const executable = data?.capabilities?.alwaysMatch?.['tauri:options']?.application;
        assert(executable, 'An explicit test executable is required');
        applications.set(value.sessionId, resolve(executable));
      } catch (error) {
        await raw('DELETE', `/session/${value.sessionId}`).catch(() => {});
        throw error;
      }
    }
    if (method === 'DELETE' && /^\/session\/[^/]+$/.test(path))
      applications.delete(path.split('/')[2]);
    return value;
  };
}
