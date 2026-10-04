import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import {
  legacyCatalog as catalog,
  v2Catalog,
  catalog as currentCatalog,
  currentSnapshot,
  validateEvidence,
  checkBundle,
} from './application-evidence.mjs';

function evidence() {
  return {
    schema_version: 1,
    build_version: '0.14.0',
    sampled_at: 1,
    mode: 'read_only',
    acceptance_status: 'not_run',
    deletion_performed: false,
    scan_scope: 'builtin_application_rules_without_user_exclusions',
    applications: ['weixin', 'qq', 'wps'].map((app_id) => {
      const rules = catalog.filter((r) => r.app_id === app_id);
      return {
        app_id,
        version: rules[0].supported_version,
        supported_version: rules[0].supported_version,
        identity_verified: true,
        scan_supported: true,
        status: 'validation_pending',
        running_component_count: 0,
        rules: rules.map((r) => ({ rule_id: r.rule_id, cleanup_validated: false })),
      };
    }),
    rules: catalog.map((r) => ({
      rule_id: r.rule_id,
      app_id: r.app_id,
      min_age_days: r.min_age_days,
      rule_version: 1,
      data_kind: r.app_id === 'qq' ? 'cache' : 'diagnostics',
      checked: 0,
      files: 0,
      logical_bytes: '0',
      retained: [],
      issues: [],
      notices: [],
      coverage_complete: true,
      error_code: null,
      blocked: 'validation_pending',
      elapsed_ms: 1,
      inspection_status: 'read_only',
      acceptance_status: 'not_run',
      deletion_performed: false,
    })),
  };
}

function evidenceV2(specs = v2Catalog) {
  const result = evidence();
  result.schema_version = 2;
  result.applications = [...new Set(specs.map((r) => r.app_id))].map((app_id) => {
    const rules = specs.filter((r) => r.app_id === app_id);
    return {
      app_id,
      version: '99.0.0.1',
      supported_version: rules[0].supported_version,
      identity_verified: true,
      scan_supported: true,
      status: 'validation_pending',
      running_component_count: 0,
      rules: rules.map((r) => ({
        rule_id: r.rule_id,
        cleanup_validated: false,
        scan_basis: 'recognized_layout',
        layout_id: r.layout_id,
      })),
    };
  });
  const template = result.rules[0];
  result.rules = specs.map((r) => ({
    ...template,
    rule_id: r.rule_id,
    app_id: r.app_id,
    min_age_days: r.min_age_days,
    data_kind: r.data_kind,
    scan_basis: 'recognized_layout',
    layout_id: r.layout_id,
  }));
  return result;
}

function evidenceV3() {
  return {
    ...evidenceV2(currentCatalog),
    schema_version: 3,
    catalog_id: currentSnapshot.catalog_id,
  };
}
test('v3 binds all twenty-one rules to a known compiled catalog and keeps old snapshots frozen', () => {
  assert.equal(catalog.length, 6);
  assert.equal(v2Catalog.length, 11);
  assert.equal(currentCatalog.length, 21);
  const result = validateEvidence(evidenceV3());
  assert.equal(result.rules.length, 21);
  assert.equal(result.version_mismatches.length, 8);
  assert.equal(result.may_enable_cleanup, false);
  assert.equal(result.acceptance_status, 'not_run');
});
test('v3 rejects catalog substitution, downgraded current packages and forged capability', () => {
  for (const mutate of [
    (e) => delete e.catalog_id,
    (e) => (e.catalog_id = 'sha256:' + '0'.repeat(64)),
    (e) => (e.catalog = currentCatalog),
    (e) => e.rules.pop(),
    (e) => (e.rules[0] = e.rules[1]),
    (e) => (e.schema_version = 2),
    (e) => {
      e.schema_version = 2;
      delete e.catalog_id;
    },
    (e) => (e.applications[0].rules[0].cleanup_validated = true),
    (e) => (e.rules[0].deletion_performed = true),
    (e) => (e.rules[0].layout_id = 'unknown'),
    (e) => (e.rules[0].coverage_complete = false),
  ]) {
    const value = evidenceV3();
    mutate(value);
    assert.throws(() => validateEvidence(value));
  }
});
test('v3 retains empty, missing, partial and failed scan distinctions', () => {
  const value = evidenceV3();
  value.rules[0].issues = [{ code: 'access_denied', count: 1 }];
  value.rules[0].coverage_complete = false;
  value.rules[1].error_code = 'scan_failed';
  value.rules[1].inspection_status = 'failed';
  value.rules[1].coverage_complete = false;
  assert.equal(validateEvidence(value).inspection_status, 'failed');
  assert.equal(validateEvidence(value).may_enable_cleanup, false);
});
test('v2 accepts cross-version readonly layouts for its frozen application rules', () => {
  const result = validateEvidence(evidenceV2());
  assert.equal(result.rules.length, v2Catalog.length);
  assert.equal(result.may_enable_cleanup, false);
  assert.equal(result.acceptance_status, 'not_run');
  assert.equal(result.version_mismatches.length, 5);
});
test('v2 rejects unknown layouts, conflicting claims, missing new rules and private fields', () => {
  for (const mutate of [
    (e) => e.rules.pop(),
    (e) => (e.applications[3].rules[0].cleanup_validated = true),
    (e) => (e.applications[3].rules[0].layout_id = 'C:\\private\\account'),
    (e) => (e.applications[3].rules[0].scan_basis = 'verified_version'),
    (e) => (e.applications[3].identity_verified = false),
    (e) => (e.applications[3].scan_supported = false),
    (e) => (e.rules[6].scan_basis = 'unverified'),
    (e) => (e.rules[6].path = 'private'),
  ]) {
    const e = evidenceV2();
    mutate(e);
    assert.throws(() => validateEvidence(e));
  }
});
test('v2 preserves missing layout and partial read distinctions without claiming deletion', () => {
  const e = evidenceV2();
  const app = e.applications[4];
  app.scan_supported = false;
  app.status = 'location_missing';
  for (const rule of app.rules) {
    rule.scan_basis = 'unverified';
    rule.layout_id = null;
  }
  for (const row of e.rules.filter((r) => r.app_id === app.app_id)) {
    row.scan_basis = 'unverified';
    row.layout_id = null;
    row.notices = [{ code: 'location_missing', count: 1 }];
  }
  e.rules[0].issues = [{ code: 'directory_inaccessible', count: 1 }];
  e.rules[0].coverage_complete = false;
  assert.equal(validateEvidence(e).may_enable_cleanup, false);
});
test('complete and empty readonly scans cannot pass real acceptance', () => {
  const result = validateEvidence(evidence());
  assert.equal(result.evidence_complete, true);
  assert.equal(result.acceptance_status, 'not_run');
  assert.equal(result.may_enable_cleanup, false);
  assert(result.rules.every((r) => r.candidate_files === 0 && r.acceptance_status === 'not_run'));
});
test('fails missing/duplicate rules, fields, contradictory coverage and claimed acceptance', () => {
  for (const mutate of [
    (e) => e.rules.pop(),
    (e) => (e.rules[0] = e.rules[1]),
    (e) => delete e.rules[0].coverage_complete,
    (e) => (e.rules[0].coverage_complete = false),
    (e) => (e.acceptance_status = 'passed'),
    (e) => (e.rules[0].acceptance_status = 'passed'),
    (e) => (e.applications[0].rules[0].cleanup_validated = true),
    (e) => (e.rules[0].logical_bytes = '-1'),
  ]) {
    const e = evidence();
    mutate(e);
    assert.throws(() => validateEvidence(e));
  }
});
test('separates unsupported installed versions from invalid catalog and claimed support', () => {
  const e = evidence();
  e.applications[0].version = '4.2.0.1';
  assert.throws(() => validateEvidence(e));
  e.applications[0].scan_supported = false;
  e.applications[0].status = 'unsupported_version';
  assert.deepEqual(validateEvidence(e).version_mismatches, ['weixin']);
  e.applications[0].supported_version = '4.2.0.1';
  assert.throws(() => validateEvidence(e));
});
test('rejects raw errors, paths, extra fields and unknown reasons', () => {
  for (const mutate of [
    (e) => (e.rules[0].error = 'C:\\Users\\private\\file'),
    (e) => (e.applications[0].locations = ['private']),
    (e) => (e.rules[0].error_code = 'private path'),
    (e) => (e.rules[0].retained = [{ code: 'unknown', count: 1, path: 'private' }]),
  ]) {
    const e = evidence();
    mutate(e);
    assert.throws(() => validateEvidence(e));
  }
});
test('reports inspection failure without fabricating real execution', () => {
  const e = evidence();
  Object.assign(e.rules[0], {
    error_code: 'access_denied',
    coverage_complete: false,
    inspection_status: 'failed',
  });
  const result = validateEvidence(e);
  assert.equal(result.inspection_status, 'failed');
  assert.equal(result.acceptance_status, 'not_run');
});
test('verifies attachment hash and rejects altered bytes or escaped attachment', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'dustaway-evidence-test-'));
  const bytes = Buffer.from(JSON.stringify(evidence()));
  const manifest = {
    schema_version: 1,
    attachments: [
      {
        name: 'evidence.json',
        bytes: bytes.length,
        sha256: createHash('sha256').update(bytes).digest('hex'),
      },
    ],
  };
  await writeFile(join(dir, 'evidence.json'), bytes);
  await writeFile(join(dir, 'manifest.json'), JSON.stringify(manifest));
  assert.equal((await checkBundle(dir)).may_enable_cleanup, false);
  await writeFile(
    join(dir, 'evidence.json'),
    (await readFile(join(dir, 'evidence.json'), 'utf8')) + ' ',
  );
  await assert.rejects(checkBundle(dir), /hash mismatch/);
  manifest.attachments[0].name = '../evidence.json';
  await writeFile(join(dir, 'manifest.json'), JSON.stringify(manifest));
  await assert.rejects(checkBundle(dir), /Invalid attachment/);
});
