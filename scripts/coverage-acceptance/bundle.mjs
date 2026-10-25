import assert from 'node:assert/strict';
import { readFile, lstat, realpath } from 'node:fs/promises';
import { resolve, join, relative, isAbsolute } from 'node:path';
import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';

export const stages = ['preflight', 'boundaries', 'states', 'links', 'core'];
export const binaryNames = ['windows-tests.exe', 'core-tests.exe', 'probe.exe'];
export const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
export function runId(value) {
  assert(typeof value === 'string' && /^[A-Za-z0-9-]{8,80}$/.test(value), 'Invalid run ID');
  return value;
}
export function basename(value) {
  assert(
    typeof value === 'string' &&
      /^[A-Za-z0-9][A-Za-z0-9_.-]{0,119}$/.test(value) &&
      !value.includes('..'),
    'Invalid evidence filename',
  );
  return value;
}
export function validateStatus(value, id) {
  assert.equal(value.schema, 3);
  assert.equal(value.runId, runId(id));
  assert.equal(value.host, 'DESKTOP-24NJTT6');
  assert(
    ['running', 'passed', 'failed', 'blocked', 'cancelled', 'uncertain'].includes(value.status),
  );
  assert.equal(value.webpageRecoveryVerified, false);
  assert.equal(value.realDeletionAttempted, false);
  assert(Array.isArray(value.stages) && value.stages.length <= stages.length);
  let previous = 0;
  for (const [i, phase] of value.stages.entries()) {
    assert.equal(phase.id, stages[i], 'Stage order changed');
    assert(Number.isSafeInteger(phase.startedAt) && phase.startedAt >= previous);
    assert(Number.isSafeInteger(phase.finishedAt) && phase.finishedAt >= phase.startedAt);
    previous = phase.finishedAt;
    assert(['passed', 'failed', 'cancelled', 'uncertain'].includes(phase.status));
    assert(phase.exitCode === null || Number.isSafeInteger(phase.exitCode));
    if (phase.status === 'passed') assert.equal(phase.exitCode, 0);
    if (phase.failureKind !== undefined) {
      assert.equal(phase.status, 'failed');
      assert(['process_start_failed', 'test_process_nonzero_exit'].includes(phase.failureKind));
      if (phase.failureKind === 'process_start_failed') assert.equal(phase.exitCode, null);
      else assert(Number.isSafeInteger(phase.exitCode) && phase.exitCode !== 0);
    }
    if (i < value.stages.length - 1)
      assert.equal(phase.status, 'passed', 'Work continued after failure');
  }
  if (value.status === 'passed') {
    assert.equal(value.ordinaryUser, true);
    assert.equal(value.stages.length, stages.length);
    assert(value.stages.every((s) => s.status === 'passed'));
  }
  return value;
}
async function safeRead(folder, name) {
  basename(name);
  const path = join(folder, name),
    info = await lstat(path);
  assert(info.isFile() && !info.isSymbolicLink() && info.nlink === 1, 'Linked evidence refused');
  const rel = relative(folder, await realpath(path));
  assert(rel && !rel.startsWith('..') && !isAbsolute(rel), 'Evidence escaped bundle');
  return readFile(path);
}
export const legacyRules = [
  'shadowbot_web',
  'shadowbot_browser',
  'shadowbot_logs',
  'codex_web',
  'claude_web',
  'docker_web',
  'docker_logs',
  'reqable_logs',
].map((id) => 'appcache.' + id);
export const previousRules = [
  'appcache.codex_logs',
  'appcache.trae_logs',
  'appcache.trae_cn_logs',
  'appcache.kiro_logs',
  'appcache.workbuddy_logs',
  'appcache.doubao_web',
  'appcache.gameviewer_web',
  'appcache.ccswitch_web',
  'appcache.ima_web',
  'appcache.easyspider_web',
];
export const expectedRules = [
  'appcache.codebuddy_extension_logs',
  'appcache.workbuddy_extension_logs',
  'appcache.todesk_logs',
  'appcache.todesk_web',
  'appcache.dingtalk_logs',
  'appcache.webtoapi_web',
  'appcache.webtoapi_logs',
  'appcache.dbx_web',
];
export function rulesForVersion(version) {
  assert(['0.22.0', '0.23.0', '0.23.1', '0.24.0'].includes(version), 'Unsupported coverage batch');
  return version === '0.22.0' ? legacyRules : version === '0.24.0' ? expectedRules : previousRules;
}
export function validateProbe(probe, version = probe.version) {
  assert.equal(probe.version, version, 'Probe and request versions differ');
  const expectedRules = rulesForVersion(version);
  assert.deepEqual([...probe.rejectedRulePlans].sort(), [...expectedRules].sort());
  assert.deepEqual(probe.rules.map((r) => r.rule).sort(), [...expectedRules].sort());
  let total = 0n;
  for (const row of probe.rules) {
    assert.match(row.knownBytes, /^\d+$/);
    assert.equal(row.knownBytes, row.retainedBytes);
    assert.equal(row.selectableBytes, '0');
    assert.equal(typeof row.unknownCapacity, 'boolean');
    assert.equal(typeof row.readFailed, 'boolean');
    assert.equal(row.availability, 'validation_pending');
    assert(!('path' in row));
    total += BigInt(row.knownBytes);
  }
  if (version === '0.23.1') {
    assert.equal(probe.observedKnownBytes, total.toString());
    assert.equal(probe.recognitionDeltaBytes, '0');
    assert(
      !('newlyRecognizedKnownBytes' in probe),
      'Unchanged scopes cannot claim new recognition',
    );
  } else assert.equal(probe.newlyRecognizedKnownBytes, total.toString());
  assert.equal(probe.retainedKnownBytes, total.toString());
  assert.equal(probe.selectableDeltaBytes, '0');
}
export async function checkBundle(directory) {
  const supplied = resolve(directory);
  assert(!(await lstat(supplied)).isSymbolicLink(), 'Linked bundle refused');
  const folder = await realpath(supplied);
  const json = (bytes) => JSON.parse(bytes.toString('utf8').replace(/^\uFEFF/, ''));
  const manifest = json(await safeRead(folder, 'manifest.json'));
  assert.equal(manifest.schema, 3);
  runId(manifest.runId);
  assert.equal(manifest.mode, 'noninteractive');
  assert(Array.isArray(manifest.files));
  const files = new Map();
  for (const file of manifest.files) {
    basename(file.name);
    assert(!files.has(file.name), 'Duplicate evidence file');
    assert(Number.isSafeInteger(file.bytes) && file.bytes >= 0);
    assert.match(file.sha256, /^[a-f0-9]{64}$/i);
    const bytes = await safeRead(folder, file.name);
    assert.equal(bytes.length, file.bytes, 'Evidence size mismatch');
    assert.equal(sha256(bytes), file.sha256.toLowerCase(), 'Evidence hash mismatch');
    files.set(file.name, bytes);
  }
  for (const name of ['request.json', 'started.json', 'status.json', 'worker.ps1', ...binaryNames])
    assert(files.has(name), `Missing ${name}`);
  const request = json(files.get('request.json'));
  assert.equal(request.schema, 3);
  assert.equal(request.runId, manifest.runId);
  rulesForVersion(request.version);
  const dispatch = json(files.get('started.json'));
  assert.equal(dispatch.runId, manifest.runId);
  assert.equal(dispatch.requestSha256.toLowerCase(), sha256(files.get('request.json')));
  for (const name of [...binaryNames, 'worker.ps1'])
    assert.equal(
      sha256(files.get(name)),
      request.hashes[name].toLowerCase(),
      'Input binary/worker changed',
    );
  const status = validateStatus(json(files.get('status.json')), manifest.runId);
  for (const stage of status.stages) {
    assert(files.has(`${stage.id}.intent.json`), 'Stage intent missing');
    const intent = json(files.get(`${stage.id}.intent.json`));
    assert.equal(intent.runId, manifest.runId);
    assert.equal(intent.stage, stage.id);
    assert.equal(intent.startedAt, stage.startedAt);
    const binary =
      stage.id === 'preflight'
        ? 'probe.exe'
        : stage.id === 'core'
          ? 'core-tests.exe'
          : 'windows-tests.exe';
    assert.equal(intent.binarySha256.toLowerCase(), request.hashes[binary].toLowerCase());
    for (const suffix of ['log', 'err'])
      assert(files.has(`${stage.id}.${suffix}`), 'Stage log missing');
    if (stage.status === 'passed' && stage.id !== 'preflight') {
      const log = files.get(`${stage.id}.log`).toString('utf8');
      assert(/test result: ok\. [1-9]\d* passed; 0 failed;/.test(log), 'No executed passing tests');
    }
  }
  if (status.status === 'passed') {
    const probe = json(files.get('preflight.log'));
    assert.equal(probe.schema, 3);
    assert.equal(probe.runId, manifest.runId);
    assert.equal(probe.status, 'completed');
    assert.equal(probe.ordinaryUser, true);
    assert.equal(probe.mode, 'read_only');
    assert.equal(probe.deletionAttempted, false);
    assert.equal(probe.webpageRecoveryVerified, false);
    validateProbe(probe, request.version);
  }

  return {
    schema: 3,
    runId: manifest.runId,
    version: request.version,
    status: status.status,
    noninteractivePassed: status.status === 'passed',
    functionalRecovery: 'not_verified',
    releaseModified: false,
  };
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  checkBundle(process.argv[2])
    .then((r) => {
      console.log(JSON.stringify(r, null, 2));
      if (!r.noninteractivePassed) process.exitCode = 2;
    })
    .catch((error) => {
      console.error(error.message);
      process.exitCode = 1;
    });
}
