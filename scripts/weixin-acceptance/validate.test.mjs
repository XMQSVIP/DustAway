import { test } from 'node:test';
import assert from 'node:assert/strict';
import { validate } from './validate.mjs';
function evidence() {
  const fingerprint = 'a'.repeat(64);
  const phase = {
    passed: true,
    applicationFingerprint: fingerprint,
    evidence: 'synthetic-test-only',
  };
  return {
    schema: 1,
    host: 'DESKTOP-24NJTT6',
    ordinaryUser: true,
    layout: 'weixin-radium-web-v1',
    runId: 'test-only-001',
    rule: 'weixin.code',
    verification: 'automated',
    applicationFingerprint: fingerprint,
    baseline: { ...phase, time: 1, candidates: 1, webpageLoaded: true, target: 'public-test-page' },
    cleanup: {
      ...phase,
      time: 2,
      engine: 'production',
      deletedFiles: 1,
      failedFiles: 0,
      unknownFiles: 0,
      applicationExited: true,
      protectedFiles: 10,
      protectedBefore: fingerprint,
      protectedAfter: fingerprint,
    },
    recovery: {
      ...phase,
      time: 3,
      firstLoad: true,
      secondLoad: true,
      target: 'public-test-page',
      regeneratedFiles: 1,
      formatVerified: true,
    },
  };
}
test('valid evidence remains metadata, never a release permit', () => {
  for (const rule of ['weixin.code', 'weixin.gpu']) {
    const e = evidence();
    e.rule = rule;
    assert.equal(validate(e).releaseModified, false);
    assert.equal(validate(e).complete, false);
    assert.equal(validate(e).legacyMetadataValid, true);
  }
});
test('missing recovery, partial failures and unrelated identity never pass', () => {
  for (const change of [
    (e) => (e.recovery = null),
    (e) => (e.recovery.regeneratedFiles = 0),
    (e) => (e.recovery.firstLoad = false),
    (e) => (e.recovery.secondLoad = false),
    (e) => (e.cleanup.protectedAfter = 'b'.repeat(64)),
    (e) => (e.cleanup.failedFiles = 1),
    (e) => (e.cleanup.unknownFiles = 1),
    (e) => (e.cleanup.deletedFiles = 0),
    (e) => (e.cleanup.deletedFiles = '1'),
    (e) => (e.baseline.candidates = Infinity),
    (e) => (e.recovery.regeneratedFiles = 1.5),
    (e) => (e.rule = 'weixin.http'),
    (e) => (e.cleanup.applicationFingerprint = 'b'.repeat(64)),
    (e) => (e.cleanup.engine = 'synthetic'),
    (e) => (e.recovery.time = 1),
    (e) => (e.recovery.target = 'other-page'),
  ]) {
    const e = evidence();
    change(e);
    assert.throws(() => validate(e));
  }
});
