import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile, rm, link } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { checkBundle, sha256, stages, binaryNames, validateStatus } from './bundle.mjs';
const id = 'test-fixture-0211';
const state = () => ({
  schema: 2,
  runId: id,
  host: 'DESKTOP-24NJTT6',
  ordinaryUser: true,
  status: 'passed',
  realDeletionAttempted: false,
  webpageRecoveryVerified: false,
  stages: stages.map((stage, i) => ({
    id: stage,
    startedAt: i * 10 + 1,
    finishedAt: i * 10 + 2,
    exitCode: 0,
    status: 'passed',
  })),
});
async function fixture(change = () => {}) {
  const root = await mkdtemp(join(tmpdir(), 'dustaway-evidence-'));
  const files = new Map(
    [...binaryNames, 'worker.ps1'].map((name) => [name, Buffer.from('synthetic ' + name)]),
  );
  files.set(
    'request.json',
    Buffer.from(
      JSON.stringify({
        schema: 2,
        runId: id,
        version: '0.21.1',
        hashes: Object.fromEntries([...files].map(([name, bytes]) => [name, sha256(bytes)])),
      }),
    ),
  );
  files.set('status.json', Buffer.from(JSON.stringify(state())));
  files.set(
    'started.json',
    Buffer.from(JSON.stringify({ runId: id, requestSha256: sha256(files.get('request.json')) })),
  );
  for (const [index, stage] of stages.entries()) {
    files.set(stage + '.log', Buffer.from('test result: ok. 1 passed; 0 failed;\n'));
    files.set(stage + '.err', Buffer.from(''));
    const binary =
      stage === 'preflight'
        ? 'probe.exe'
        : stage === 'attachments'
          ? 'core-tests.exe'
          : 'weixin-tests.exe';
    files.set(
      stage + '.intent.json',
      Buffer.from(
        JSON.stringify({
          runId: id,
          stage,
          startedAt: index * 10 + 1,
          binarySha256: sha256(files.get(binary)),
        }),
      ),
    );
  }
  files.set(
    'preflight.log',
    Buffer.from(
      JSON.stringify({
        schema: 2,
        runId: id,
        status: 'completed',
        ordinaryUser: true,
        mode: 'read_only',
        deletionAttempted: false,
        webpageRecoveryVerified: false,
      }),
    ),
  );
  files.set(
    'fixtures.log',
    Buffer.from(
      'DUSTAWAY_SYNTHETIC_RECEIPTS=' +
        JSON.stringify(
          ['weixin.code', 'weixin.gpu'].map((rule_id) => ({
            rule_id,
            selected_count: 1,
            deleted_count: 1,
            failed_count: 0,
            unknown_count: 0,
            retained_count: 2,
            protected_count: 1,
            single_use_plan: true,
            result_status: 'finished',
            protection_before: [['a', '1']],
            protection_after: [['a', '1']],
          })),
        ) +
        '\ntest result: ok. 1 passed; 0 failed;',
    ),
  );
  for (const rule of ['weixin.code', 'weixin.gpu'])
    files.set(
      rule + '.json',
      Buffer.from(
        JSON.stringify({
          rule,
          runId: id,
          status: 'blocked',
          reason: 'validation_pending',
          productionGateRespected: true,
          deletionAttempted: false,
          webpageRecoveryVerified: false,
        }),
      ),
    );
  change(files);
  for (const [name, bytes] of files) await writeFile(join(root, name), bytes);
  await writeFile(
    join(root, 'manifest.json'),
    JSON.stringify({
      schema: 2,
      runId: id,
      mode: 'noninteractive',
      files: [...files].map(([name, bytes]) => ({
        name,
        bytes: bytes.length,
        sha256: sha256(bytes),
      })),
    }),
  );
  return root;
}
test('checked SSH bundle never claims functional recovery or grants release permission', async () => {
  const root = await fixture();
  try {
    const r = await checkBundle(root);
    assert.equal(r.noninteractivePassed, true);
    assert.equal(r.functionalRecovery, 'not_verified');
    assert.equal(r.releaseModified, false);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
test('hashes, missing logs, changed run, nonzero exit, wrong order, claimed recovery and input substitution fail', async () => {
  const changeJson = (files, name, change) => {
    const data = JSON.parse(files.get(name));
    change(data);
    files.set(name, Buffer.from(JSON.stringify(data)));
  };
  for (const change of [
    (f) => f.delete('code.log'),
    (f) => f.delete('code.intent.json'),
    (f) => changeJson(f, 'started.json', (s) => (s.requestSha256 = '0'.repeat(64))),
    (f) => changeJson(f, 'code.intent.json', (s) => (s.runId = 'different-run')),
    (f) => changeJson(f, 'code.intent.json', (s) => (s.binarySha256 = '0'.repeat(64))),
    (f) => changeJson(f, 'status.json', (s) => (s.runId = 'different-run')),
    (f) => changeJson(f, 'status.json', (s) => (s.stages[1].exitCode = 1)),
    (f) => changeJson(f, 'status.json', (s) => s.stages.reverse()),
    (f) => changeJson(f, 'status.json', (s) => (s.stages[1].startedAt = 0)),
    (f) => changeJson(f, 'preflight.log', (s) => (s.webpageRecoveryVerified = true)),
    (f) => changeJson(f, 'weixin.code.json', (s) => (s.deletionAttempted = true)),
    (f) => f.set('probe.exe', Buffer.from('substituted')),
    (f) => f.set('attachments.log', Buffer.from('test result: ok. 0 passed; 0 failed;')),
  ]) {
    const root = await fixture(change);
    try {
      await assert.rejects(checkBundle(root));
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  }
  const root = await fixture();
  try {
    await writeFile(join(root, 'code.log'), 'modified');
    await assert.rejects(checkBundle(root), /mismatch/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
test('path escape, duplicate entries and linked evidence are rejected', async () => {
  for (const change of [
    (m) => (m.files[0].name = '../outside'),
    (m) => m.files.push(m.files[0]),
    (m) => (m.mode = 'full_recovery'),
  ]) {
    const root = await fixture();
    try {
      const m = JSON.parse(await readFile(join(root, 'manifest.json')));
      change(m);
      await writeFile(join(root, 'manifest.json'), JSON.stringify(m));
      await assert.rejects(checkBundle(root));
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  }
  const root = await fixture();
  try {
    await link(join(root, 'code.log'), join(root, 'linked.log'));
    await assert.rejects(checkBundle(root), /Linked/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
test('incomplete, timeout and disconnected runs cannot become passed', () => {
  for (const status of ['running', 'blocked', 'failed', 'cancelled', 'uncertain']) {
    const s = { ...state(), status, stages: [] };
    assert.equal(validateStatus(s, id).status, status);
    s.status = 'passed';
    assert.throws(() => validateStatus(s, id));
  }
});

test('process launch failures and nonzero test exits remain distinct failures', () => {
  for (const [failureKind, exitCode] of [
    ['process_start_failed', null],
    ['test_process_nonzero_exit', 101],
  ]) {
    const s = {
      ...state(),
      status: 'failed',
      stages: [{ ...state().stages[0], status: 'failed', failureKind, exitCode }],
    };
    assert.equal(validateStatus(s, id).stages[0].failureKind, failureKind);
    s.stages[0].exitCode = 0;
    assert.throws(() => validateStatus(s, id));
  }
});
