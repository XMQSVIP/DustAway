// Development records only. This program never writes release permissions or touches app data.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { readFile, writeFile, mkdir, lstat, realpath } from 'node:fs/promises';
import { dirname, join, resolve, relative, isAbsolute } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { currentSnapshot } from './application-evidence.mjs';

const project = resolve(dirname(fileURLToPath(import.meta.url)), '..');
export const targetIds = [
  'weixin.logs',
  'dingtalk.http',
  'dingtalk.code',
  'baidu_netdisk.http',
  'baidu_netdisk.code',
  'baidu_netdisk.gpu',
];
const specs = targetIds.map((id) => currentSnapshot.rules.find((r) => r.rule_id === id));
const hash = (data) => createHash('sha256').update(data).digest('hex');
const sha = (value) => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
const uint = (value) => Number.isSafeInteger(value) && value >= 0;
const version = (value) => typeof value === 'string' && /^\d+\.\d+\.\d+\.\d+$/.test(value);
const sourceFiles = [
  'rules/builtin.toml',
  'Cargo.lock',
  'crates/core/src/cleanup_acceptance.rs',
  'crates/core/src/applications.rs',
  'crates/core/src/model.rs',
  'crates/core/src/engine.rs',
  'crates/windows/src/platform.rs',
  'crates/windows/src/applications.rs',
  'crates/windows/src/application_layouts.rs',
  'crates/windows/src/fs.rs',
  'crates/windows/src/application_acceptance_tests.rs',
  'scripts/application-acceptance.mjs',
];
function keys(value, names) {
  assert(value && typeof value === 'object' && !Array.isArray(value), 'Object required');
  assert.deepEqual(
    Object.keys(value).sort(),
    names.split(' ').sort(),
    'Unexpected or missing fields',
  );
}
function protectedSamples(items) {
  assert(
    Array.isArray(items) && items.length > 0 && items.length <= 10000,
    'Protection samples required',
  );
  for (const sample of items) {
    assert(
      Array.isArray(sample) && sample.length === 2 && sha(sample[0]),
      'Invalid protected hash',
    );
    // Windows FILETIME exceeds JS safe integer precision. Store it as a decimal string.
    assert(
      typeof sample[1] === 'string' && /^\d{1,20}$/.test(sample[1]),
      'Invalid protected timestamp',
    );
  }
}
function validateReceipt(receipt, spec) {
  keys(
    receipt,
    'rule_id rule_version layout_id application_version candidate_count selected_count deleted_count deleted_bytes retained_count failed_count unknown_count protected_count protection_before protection_after single_use_plan result_status',
  );
  assert.equal(receipt.rule_id, spec.rule_id);
  assert.equal(receipt.rule_version, spec.rule_version);
  assert.equal(receipt.layout_id, spec.layout_id);
  assert.equal(receipt.application_version, '0.0.0.1', 'Synthetic version must be explicit');
  for (const key of [
    'candidate_count',
    'selected_count',
    'deleted_count',
    'retained_count',
    'failed_count',
    'unknown_count',
    'protected_count',
  ])
    assert(uint(receipt[key]), 'Invalid count');
  assert(receipt.candidate_count > 0 && receipt.selected_count > 0 && receipt.protected_count > 0);
  assert.equal(receipt.selected_count + receipt.retained_count, receipt.candidate_count);
  assert.equal(receipt.deleted_count, receipt.selected_count);
  assert.equal(receipt.failed_count + receipt.unknown_count, 0);
  assert(typeof receipt.deleted_bytes === 'string' && /^[1-9]\d*$/.test(receipt.deleted_bytes));
  protectedSamples(receipt.protection_before);
  protectedSamples(receipt.protection_after);
  assert.equal(receipt.protection_before.length, receipt.protected_count);
  assert.deepEqual(
    receipt.protection_before,
    receipt.protection_after,
    'Protected samples changed',
  );
  assert.equal(receipt.single_use_plan, true);
  assert.equal(receipt.result_status, 'finished');
}
export function pendingReal() {
  return {
    status: 'not_run',
    os_build: null,
    application_version: null,
    executable_sha256: null,
    signature_verified: null,
    candidate_count: null,
    deleted_count: null,
    failed_count: null,
    unknown_count: null,
    protection_before: [],
    protection_after: [],
    restart: 'not_run',
    functionality: 'not_run',
    regeneration: 'not_run',
  };
}
export function createRecord(sources, receipts = null) {
  return {
    schema_version: 1,
    kind: 'application_cleanup_development',
    catalog_id: currentSnapshot.catalog_id,
    created_at: new Date().toISOString(),
    mode: receipts === null ? 'preparation' : 'synthetic',
    source_hashes: { ...sources },
    may_enable_cleanup: false,
    rules: specs.map((spec) => ({
      rule_id: spec.rule_id,
      rule_version: spec.rule_version,
      layout_id: spec.layout_id,
      data_kind: spec.data_kind,
      synthetic: receipts?.find((row) => row.rule_id === spec.rule_id) ?? null,
      real: pendingReal(),
    })),
  };
}
export function validateRecord(record) {
  keys(
    record,
    'schema_version kind catalog_id created_at mode source_hashes may_enable_cleanup rules',
  );
  assert.equal(record.schema_version, 1);
  assert.equal(record.kind, 'application_cleanup_development');
  assert.equal(record.catalog_id, currentSnapshot.catalog_id, 'Unknown rule catalog');
  assert.equal(record.may_enable_cleanup, false, 'Records cannot authorize deletion');
  assert(
    ['preparation', 'synthetic', 'manual_record'].includes(record.mode),
    'Unknown record mode',
  );
  assert(typeof record.created_at === 'string' && Number.isFinite(Date.parse(record.created_at)));
  keys(record.source_hashes, sourceFiles.join(' '));
  assert(Object.values(record.source_hashes).every(sha), 'Source hashes required');
  assert(
    Array.isArray(record.rules) && record.rules.length === specs.length,
    'All six rules required',
  );
  const seen = new Set();
  let syntheticPassed = 0;
  let realRecordedPassed = 0;
  for (const row of record.rules) {
    keys(row, 'rule_id rule_version layout_id data_kind synthetic real');
    const spec = specs.find((r) => r.rule_id === row.rule_id);
    assert(spec && !seen.has(row.rule_id), 'Unknown or duplicate rule');
    seen.add(row.rule_id);
    for (const key of ['rule_version', 'layout_id', 'data_kind']) assert.equal(row[key], spec[key]);
    if (record.mode === 'synthetic') assert(row.synthetic !== null, 'Missing synthetic receipt');
    if (record.mode === 'preparation') assert.equal(row.synthetic, null);
    if (row.synthetic !== null) {
      validateReceipt(row.synthetic, spec);
      syntheticPassed++;
    }
    const real = row.real;
    keys(real, Object.keys(pendingReal()).join(' '));
    if (real.status === 'not_run') {
      assert.deepEqual(real, pendingReal(), 'Unperformed real checks must remain empty');
      continue;
    }
    assert.equal(record.mode, 'manual_record', 'Synthetic evidence is not real acceptance');
    assert(['passed', 'failed'].includes(real.status));
    assert(typeof real.os_build === 'string' && /^\d+(\.\d+){1,3}$/.test(real.os_build));
    assert(version(real.application_version) && sha(real.executable_sha256));
    assert.equal(typeof real.signature_verified, 'boolean');
    for (const key of ['candidate_count', 'deleted_count', 'failed_count', 'unknown_count'])
      assert(uint(real[key]));
    assert(real.deleted_count + real.failed_count + real.unknown_count <= real.candidate_count);
    for (const key of ['restart', 'functionality', 'regeneration'])
      assert(['not_run', 'passed', 'failed'].includes(real[key]));
    protectedSamples(real.protection_before);
    protectedSamples(real.protection_after);
    if (real.status === 'passed') {
      assert(real.signature_verified && real.candidate_count > 0 && real.deleted_count > 0);
      assert.equal(real.failed_count + real.unknown_count, 0);
      assert.deepEqual(real.protection_before, real.protection_after);
      assert(['restart', 'functionality', 'regeneration'].every((key) => real[key] === 'passed'));
      realRecordedPassed++;
    }
  }
  return {
    may_enable_cleanup: false,
    synthetic_passed: syntheticPassed,
    real_recorded_passed: realRecordedPassed,
    real_acceptance_review: 'not_performed',
  };
}
export async function sourceHashes() {
  return Object.fromEntries(
    await Promise.all(
      sourceFiles.map(async (name) => [name, hash(await readFile(join(project, name)))]),
    ),
  );
}
export async function writeBundle(folder, record) {
  validateRecord(record);
  const root = resolve(folder);
  await mkdir(root, { recursive: false });
  const bytes = Buffer.from(JSON.stringify(record, null, 2) + '\n');
  await writeFile(join(root, 'acceptance.json'), bytes, { flag: 'wx' });
  await writeFile(
    join(root, 'manifest.json'),
    JSON.stringify(
      { schema_version: 1, name: 'acceptance.json', bytes: bytes.length, sha256: hash(bytes) },
      null,
      2,
    ) + '\n',
    { flag: 'wx' },
  );
}
export async function checkBundle(folder) {
  const root = await realpath(folder);
  const manifestPath = join(root, 'manifest.json');
  assert(!(await lstat(manifestPath)).isSymbolicLink(), 'Linked manifest refused');
  const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
  keys(manifest, 'schema_version name bytes sha256');
  assert.equal(manifest.schema_version, 1);
  assert.equal(manifest.name, 'acceptance.json');
  assert(uint(manifest.bytes) && manifest.bytes <= 4 * 1024 * 1024 && sha(manifest.sha256));
  const path = join(root, manifest.name);
  assert(!(await lstat(path)).isSymbolicLink(), 'Linked evidence refused');
  const rel = relative(root, await realpath(path));
  assert(!rel.startsWith('..') && !isAbsolute(rel), 'Evidence escaped bundle');
  assert((await lstat(path)).size <= 4 * 1024 * 1024, 'Evidence too large');
  const bytes = await readFile(path);
  assert.equal(bytes.length, manifest.bytes);
  assert.equal(hash(bytes), manifest.sha256, 'Evidence checksum mismatch');
  return validateRecord(JSON.parse(bytes.toString('utf8')));
}
async function main() {
  const [operation, folder, input] = process.argv.slice(2);
  assert(
    ['init', 'synthetic', 'pack', 'check'].includes(operation) && folder,
    'Usage: node scripts/application-acceptance.mjs init|synthetic|check <new-folder> | pack <new-folder> <manual-record.json>',
  );
  if (operation === 'pack') {
    assert(input, 'Manual record file required');
    const record = JSON.parse(await readFile(input, 'utf8'));
    assert.equal(record.mode, 'manual_record');
    await writeBundle(folder, record);
  } else if (operation === 'init' || operation === 'synthetic') {
    // Check before expensive work, and check again with mkdir before publishing the record.
    await assert.rejects(lstat(resolve(folder)), { code: 'ENOENT' });
    const before = await sourceHashes();
    let receipts = null;
    if (operation === 'synthetic') {
      const { stdout } = await promisify(execFile)(
        'cargo',
        [
          'test',
          '--locked',
          '-p',
          'dustaway-windows',
          '--lib',
          'platform::application_acceptance_tests::six_rule_cleanup_receipts',
          '--',
          '--exact',
          '--nocapture',
        ],
        // First builds and another Cargo check can spend minutes waiting for the build lock.
        { cwd: project, windowsHide: true, maxBuffer: 4 * 1024 * 1024, timeout: 900000 },
      );
      const lines = stdout
        .split(/\r?\n/)
        .filter((line) => line.startsWith('DUSTAWAY_SYNTHETIC_RECEIPTS='));
      assert.equal(lines.length, 1, 'Missing or duplicate test receipt');
      receipts = JSON.parse(lines[0].slice('DUSTAWAY_SYNTHETIC_RECEIPTS='.length));
      assert(Array.isArray(receipts) && receipts.length === targetIds.length);
      assert.deepEqual(new Set(receipts.map((r) => r.rule_id)), new Set(targetIds));
    }
    assert.deepEqual(await sourceHashes(), before, 'Source changed during test');
    await writeBundle(folder, createRecord(before, receipts));
  }
  console.log(JSON.stringify(await checkBundle(folder), null, 2));
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main().catch((error) => {
    console.error(
      error.killed
        ? 'Synthetic build/test timed out. Finish other Cargo tasks and retry; existing evidence is preserved.'
        : 'Acceptance command failed; existing evidence is preserved. Check build, schema and checksum.',
    );
    process.exitCode = 1;
  });
}
