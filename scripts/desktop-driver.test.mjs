import test from 'node:test';
import assert from 'node:assert/strict';
import { assertMatchingDriver, assertNativeViewport } from './desktop-driver.mjs';

test('reject driver/runtime build mismatch before desktop tests', () => {
  assert.throws(
    () =>
      assertMatchingDriver({
        browserVersion: '154.0.4258.37',
        msedge: { msedgedriverVersion: '153.0.4234.48 (build)' },
      }),
    /matching driver/,
  );
  assert.doesNotThrow(() =>
    assertMatchingDriver({
      browserVersion: '154.0.4258.37',
      msedge: { msedgedriverVersion: '154.0.4258.50 (build)' },
    }),
  );
});

test('detect the observed child-window displacement even when DOM layout fits', () => {
  const native = {
    running: true,
    minimized: false,
    dpi: 120,
    client: { width: 1375, height: 975 },
    children: [
      { className: 'WRY_WEBVIEW', x: 0, y: 0, width: 1375, height: 975 },
      { className: 'Chrome_WidgetWin_1', x: 601, y: 208, width: 950, height: 725 },
    ],
  };
  assert.throws(() => assertNativeViewport(native), /displaced or clipped/);
  native.children[1] = { ...native.children[1], x: 0, y: 0, width: 1375, height: 975 };
  assert.doesNotThrow(() => assertNativeViewport(native, { width: 1100, height: 780 }));
  assert.throws(() => assertNativeViewport(native, { width: 760, height: 580 }), /client width/);
});
