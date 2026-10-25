import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import {
  checkBundle,
  expectedRules,
  legacyRules,
  rulesForVersion,
  stages,
  binaryNames,
  sha256,
} from './bundle.mjs';
const id = 'fixture-coverage-022';
async function fixture(mutate = () => {}, version = '0.24.0') {
  const scope = rulesForVersion(version);
  const root = await mkdtemp(join(tmpdir(), 'dustaway-coverage-evidence-'));
  const files = new Map(
    [...binaryNames, 'worker.ps1'].map((n) => [n, Buffer.from('isolated ' + n)]),
  );
  const set = (n, v) => files.set(n, Buffer.from(JSON.stringify(v)));
  const hashes = Object.fromEntries([...files].map(([n, b]) => [n, sha256(b)]));
  set('request.json', { schema: 3, version, runId: id, hashes });
  set('started.json', { runId: id, requestSha256: sha256(files.get('request.json')) });
  set('status.json', {
    schema: 3,
    runId: id,
    host: 'DESKTOP-24NJTT6',
    ordinaryUser: true,
    status: 'passed',
    realDeletionAttempted: false,
    webpageRecoveryVerified: false,
    stages: stages.map((s, i) => ({
      id: s,
      startedAt: i * 10 + 1,
      finishedAt: i * 10 + 2,
      status: 'passed',
      exitCode: 0,
    })),
  });
  for (const [i, s] of stages.entries()) {
    files.set(s + '.log', Buffer.from('test result: ok. 1 passed; 0 failed;'));
    files.set(s + '.err', Buffer.from(''));
    set(s + '.intent.json', {
      runId: id,
      stage: s,
      startedAt: i * 10 + 1,
      binarySha256:
        hashes[
          s === 'preflight' ? 'probe.exe' : s === 'core' ? 'core-tests.exe' : 'windows-tests.exe'
        ],
    });
  }
  set('preflight.log', {
    schema: 3,
    version,
    runId: id,
    status: 'completed',
    ordinaryUser: true,
    mode: 'read_only',
    deletionAttempted: false,
    webpageRecoveryVerified: false,
    rejectedRulePlans: scope,
    rules: scope.map((rule) => ({
      rule,
      knownBytes: '100',
      retainedBytes: '100',
      selectableBytes: '0',
      unknownCapacity: false,
      readFailed: false,
      availability: 'validation_pending',
    })),
    ...(version === '0.23.1'
      ? { observedKnownBytes: String(scope.length * 100), recognitionDeltaBytes: '0' }
      : { newlyRecognizedKnownBytes: String(scope.length * 100) }),
    retainedKnownBytes: String(scope.length * 100),
    selectableDeltaBytes: '0',
  });
  const edit = (name, fn) => {
    const v = JSON.parse(files.get(name));
    fn(v);
    set(name, v);
  };
  mutate(files, edit);
  for (const [n, b] of files) await writeFile(join(root, n), b);
  await writeFile(
    join(root, 'manifest.json'),
    JSON.stringify({
      schema: 3,
      runId: id,
      mode: 'noninteractive',
      files: [...files].map(([name, b]) => ({ name, bytes: b.length, sha256: sha256(b) })),
    }),
  );
  return root;
}
test('valid coverage evidence proves only readonly scope and zero cleanup increment', async () => {
  const root = await fixture();
  try {
    assert.deepEqual(await checkBundle(root), {
      schema: 3,
      runId: id,
      version: '0.24.0',
      status: 'passed',
      noninteractivePassed: true,
      functionalRecovery: 'not_verified',
      releaseModified: false,
    });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
test('coverage evidence rejects altered bytes, missing logs, scope changes and cleanup claims', async () => {
  for (const mutate of [
    (f) => f.delete('states.log'),
    (f) => f.delete('links.intent.json'),
    (f, e) => e('preflight.log', (v) => v.rules.pop()),
    (f, e) => e('preflight.log', (v) => (v.rules[0].selectableBytes = '1')),
    (f, e) => e('preflight.log', (v) => (v.retainedKnownBytes = '0')),
    (f, e) => e('preflight.log', (v) => (v.deletionAttempted = true)),
    (f, e) => e('preflight.log', (v) => (v.webpageRecoveryVerified = true)),
    (f, e) => e('status.json', (v) => (v.stages[0].exitCode = 1)),
    (f, e) => e('status.json', (v) => v.stages.reverse()),
    (f) => f.set('probe.exe', Buffer.from('replacement')),
  ]) {
    const root = await fixture(mutate);
    try {
      await assert.rejects(checkBundle(root));
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  }
  const root = await fixture();
  try {
    await writeFile(join(root, 'preflight.log'), 'tampered');
    await assert.rejects(checkBundle(root));
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
test('manifest cannot escape its run directory', async () => {
  const root = await fixture();
  try {
    const m = JSON.parse(await readFile(join(root, 'manifest.json')));
    m.files[0].name = '../outside';
    await writeFile(join(root, 'manifest.json'), JSON.stringify(m));
    await assert.rejects(checkBundle(root));
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('legacy batch remains verifiable and cannot be relabeled as current', async () => {
  const root = await fixture(() => {}, '0.22.0');
  try {
    const result = await checkBundle(root);
    assert.equal(result.version, '0.22.0');
    assert.equal(result.noninteractivePassed, true);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
  for (const change of [
    (f, e) =>
      e('preflight.log', (v) => {
        v.version = '0.22.0';
      }),
    (f, e) =>
      e('request.json', (v) => {
        v.version = '0.25.0';
      }),
    (f, e) =>
      e('preflight.log', (v) => {
        v.rules[0].rule = legacyRules[0];
      }),
    (f, e) =>
      e('preflight.log', (v) => {
        v.rules.push(v.rules[0]);
      }),
  ]) {
    const folder = await fixture(change);
    try {
      await assert.rejects(checkBundle(folder));
    } finally {
      await rm(folder, { recursive: true, force: true });
    }
  }
});

test('0.23 evidence stays readable and patch evidence cannot claim added recognition', async () => {
  const old = await fixture(() => {}, '0.23.0');
  try {
    assert.equal((await checkBundle(old)).version, '0.23.0');
  } finally {
    await rm(old, { recursive: true, force: true });
  }
  const wrong = await fixture(
    (f, e) =>
      e('preflight.log', (v) => {
        v.recognitionDeltaBytes = '1';
      }),
    '0.23.1',
  );
  try {
    await assert.rejects(checkBundle(wrong));
  } finally {
    await rm(wrong, { recursive: true, force: true });
  }
});

test('0.24 batch cannot reuse 0.23.1 rules and all older batches retain their own versions', async () => {
  for (const version of ['0.22.0', '0.23.0', '0.23.1']) {
    const root = await fixture(() => {}, version);
    try {
      assert.equal((await checkBundle(root)).version, version);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  }
  for (const mutate of [
    (f, e) =>
      e('preflight.log', (v) => {
        v.rules[0].rule = rulesForVersion('0.23.1')[0];
      }),
    (f, e) =>
      e('preflight.log', (v) => {
        v.newlyRecognizedKnownBytes = '0';
      }),
    (f, e) =>
      e('preflight.log', (v) => {
        v.rejectedRulePlans = rulesForVersion('0.23.1');
      }),
  ]) {
    const root = await fixture(mutate);
    try {
      await assert.rejects(checkBundle(root));
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  }
});
