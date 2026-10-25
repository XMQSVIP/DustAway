import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';

/** Completeness check only. Never edits the compiled release registry. */
export function validate(record) {
  const positiveCount = (value) => assert(Number.isSafeInteger(value) && value > 0);
  assert.equal(record.schema, 1);
  assert(['weixin.code', 'weixin.gpu'].includes(record.rule));
  assert.equal(record.host, 'DESKTOP-24NJTT6');
  assert.equal(record.ordinaryUser, true);
  assert.equal(record.layout, 'weixin-radium-web-v1');
  assert.match(record.runId, /^[a-zA-Z0-9-]{8,80}$/);
  assert.match(record.applicationFingerprint, /^[a-f0-9]{64}$/i);
  assert(['automated', 'human-confirmed'].includes(record.verification));
  const phases = [record.baseline, record.cleanup, record.recovery];
  let previous = 0;
  for (const phase of phases) {
    assert(phase && phase.passed === true);
    assert.equal(phase.applicationFingerprint, record.applicationFingerprint);
    assert(Number.isSafeInteger(phase.time) && phase.time > previous);
    previous = phase.time;
    assert.equal(typeof phase.evidence, 'string');
    assert(phase.evidence.trim().length > 0);
  }
  positiveCount(record.baseline.candidates);
  assert.equal(record.baseline.webpageLoaded, true);
  assert.equal(record.cleanup.engine, 'production');
  positiveCount(record.cleanup.deletedFiles);
  assert.equal(record.cleanup.failedFiles, 0);
  assert.equal(record.cleanup.unknownFiles, 0);
  assert.equal(record.cleanup.applicationExited, true);
  positiveCount(record.cleanup.protectedFiles);
  assert.match(record.cleanup.protectedBefore, /^[a-f0-9]{64}$/i);
  assert.equal(record.cleanup.protectedBefore, record.cleanup.protectedAfter);
  assert.equal(record.recovery.firstLoad, true);
  assert.equal(record.recovery.secondLoad, true);
  assert.equal(record.recovery.target, record.baseline.target);
  assert(typeof record.recovery.target === 'string' && record.recovery.target.length > 0);
  positiveCount(record.recovery.regeneratedFiles);
  assert.equal(record.recovery.formatVerified, true);
  return {
    complete: false,
    legacyMetadataValid: true,
    evidenceFilesVerified: false,
    rule: record.rule,
    verification: record.verification,
    releaseModified: false,
  };
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  console.log(
    JSON.stringify(
      validate(JSON.parse((await readFile(process.argv[2], 'utf8')).replace(/^\uFEFF/, ''))),
      null,
      2,
    ),
  );
}
