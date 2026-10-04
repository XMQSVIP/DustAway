import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import {
  createRecord,
  sourceHashes,
  validateRecord,
  writeBundle,
  checkBundle,
  targetIds,
} from './application-acceptance.mjs';
import { currentSnapshot } from './application-evidence.mjs';

const sources = await sourceHashes();
function receipt(id) {
  const spec = currentSnapshot.rules.find((r) => r.rule_id === id);
  return {
    rule_id: id,
    rule_version: spec.rule_version,
    layout_id: spec.layout_id,
    application_version: '0.0.0.1',
    candidate_count: 3,
    selected_count: 2,
    deleted_count: 2,
    deleted_bytes: '9007199254740993',
    retained_count: 1,
    failed_count: 0,
    unknown_count: 0,
    protected_count: 1,
    protection_before: [['a'.repeat(64), '133700000000000000']],
    protection_after: [['a'.repeat(64), '133700000000000000']],
    single_use_plan: true,
    result_status: 'finished',
  };
}
const synthetic = () => createRecord(sources, targetIds.map(receipt));

test('preparation and synthetic results always keep real acceptance unperformed', () => {
  assert.deepEqual(validateRecord(createRecord(sources)), {
    may_enable_cleanup: false,
    synthetic_passed: 0,
    real_recorded_passed: 0,
    real_acceptance_review: 'not_performed',
  });
  assert.equal(validateRecord(synthetic()).synthetic_passed, 6);
});
test('rejects missing, duplicate or out-of-scope rules and changed catalog/layout', () => {
  for (const modify of [
    (r) => r.rules.pop(),
    (r) => {
      r.rules[1] = r.rules[0];
    },
    (r) => {
      r.rules[0].rule_id = 'weixin.dumps';
    },
    (r) => {
      r.rules[0].layout_id = 'unverified';
    },
    (r) => {
      r.catalog_id = 'unknown';
    },
  ]) {
    const r = synthetic();
    modify(r);
    assert.throws(() => validateRecord(r));
  }
});
test('rejects synthetic claims of real acceptance, source omissions and runtime permission', () => {
  for (const modify of [
    (r) => {
      r.may_enable_cleanup = true;
    },
    (r) => {
      r.rules[0].real.status = 'passed';
    },
    (r) => {
      r.rules[0].real.restart = 'passed';
    },
    (r) => {
      delete r.source_hashes['Cargo.lock'];
    },
    (r) => {
      r.rules[0].real.raw_path = 'private data';
    },
  ]) {
    const r = synthetic();
    modify(r);
    assert.throws(() => validateRecord(r));
  }
});
test('requires successful single-use deletion and unchanged protected hashes and exact timestamps', () => {
  for (const modify of [
    (s) => {
      s.candidate_count = 0;
    },
    (s) => {
      s.deleted_count = 0;
    },
    (s) => {
      s.failed_count = 1;
    },
    (s) => {
      s.unknown_count = 1;
    },
    (s) => {
      s.protection_after[0][0] = 'b'.repeat(64);
    },
    (s) => {
      s.protection_after[0][1] = '133700000000000001';
    },
    (s) => {
      s.protection_before[0][1] = 133700000000000000;
    },
    (s) => {
      s.single_use_plan = false;
    },
    (s) => {
      s.result_status = 'cancelled';
    },
  ]) {
    const r = synthetic();
    modify(r.rules[0].synthetic);
    assert.throws(() => validateRecord(r));
  }
});
test('manual real records require all functional checks but never authorize a release', () => {
  const record = createRecord(sources);
  record.mode = 'manual_record';
  Object.assign(record.rules[0].real, {
    status: 'passed',
    os_build: '10.0.19045',
    application_version: '4.1.13.65',
    executable_sha256: 'c'.repeat(64),
    signature_verified: true,
    candidate_count: 2,
    deleted_count: 1,
    failed_count: 0,
    unknown_count: 0,
    protection_before: [['d'.repeat(64), '133700000000000000']],
    protection_after: [['d'.repeat(64), '133700000000000000']],
    restart: 'passed',
    functionality: 'passed',
    regeneration: 'passed',
  });
  assert.equal(validateRecord(record).real_recorded_passed, 1);
  assert.equal(validateRecord(record).may_enable_cleanup, false);
  record.rules[0].real.regeneration = 'not_run';
  assert.throws(() => validateRecord(record));
});
test('bundles refuse overwrite, corrupt hashes and traversal attachment names', async () => {
  const root = await mkdtemp(join(tmpdir(), 'dustaway-record-test-'));
  try {
    const folder = join(root, 'bundle');
    await writeBundle(folder, synthetic());
    assert.equal((await checkBundle(folder)).synthetic_passed, 6);
    await assert.rejects(writeBundle(folder, synthetic()), { code: 'EEXIST' });
    const manifest = JSON.parse(await readFile(join(folder, 'manifest.json'), 'utf8'));
    await writeFile(
      join(folder, 'manifest.json'),
      JSON.stringify({ ...manifest, name: '../acceptance.json' }),
    );
    await assert.rejects(checkBundle(folder));
    await writeFile(join(folder, 'manifest.json'), JSON.stringify(manifest));
    await writeFile(join(folder, 'acceptance.json'), '{}');
    await assert.rejects(checkBundle(folder));
  } finally {
    await rm(root, { recursive: true });
  }
});
