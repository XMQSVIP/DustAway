// Local development tool. No production permission or validated flag is written here.
import { readFile, writeFile, mkdir, lstat, realpath } from 'node:fs/promises';
import { resolve, join, dirname, relative, isAbsolute } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import assert from 'node:assert/strict';

const project = resolve(dirname(fileURLToPath(import.meta.url)), '..');
export const legacyCatalog = JSON.parse(
  await readFile(join(project, 'scripts/application-catalog.json'), 'utf8'),
);
export const v2Catalog = JSON.parse(
  await readFile(join(project, 'scripts/application-catalog-v2.json'), 'utf8'),
);
export const currentSnapshot = JSON.parse(
  await readFile(join(project, 'scripts/application-catalog-v3.json'), 'utf8'),
);
export const catalog = currentSnapshot.rules;
const codes = new Set(
  'recent in_use access_denied changed link_or_cloud protected excluded browser_running application_running application_unverified unsupported_version validation_pending process_unknown unsupported_layout location_missing directory_inaccessible unsupported_volume scan_failed delete_failed unverifiable unknown'.split(
    ' ',
  ),
);
const integer = (value) => Number.isSafeInteger(value) && value >= 0;
const version = (value) => typeof value === 'string' && /^\d+\.\d+\.\d+(?:\.\d+)?$/.test(value);
const reason = (value) => value === null || codes.has(value);
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
function keys(value, names) {
  assert(value && typeof value === 'object' && !Array.isArray(value), 'Expected object');
  assert.deepEqual(
    Object.keys(value).sort(),
    names.split(' ').sort(),
    'Missing or non-whitelisted field',
  );
}
function reasons(items) {
  assert(Array.isArray(items), 'Missing reasons');
  const seen = new Set();
  for (const item of items) {
    keys(item, 'code count');
    assert(
      codes.has(item.code) && integer(item.count) && item.count > 0 && !seen.has(item.code),
      'Invalid reason',
    );
    seen.add(item.code);
  }
}

export function validateEvidence(evidence) {
  const v3 = evidence?.schema_version === 3;
  const v2 = evidence?.schema_version === 2 || v3;
  const expectedCatalog = v3 ? catalog : v2 ? v2Catalog : legacyCatalog;
  const expectedApps = new Set(expectedCatalog.map((r) => r.app_id));
  const appRules = new Map();
  keys(
    evidence,
    'schema_version build_version sampled_at mode applications deletion_performed acceptance_status rules scan_scope' +
      (v3 ? ' catalog_id' : ''),
  );
  if (v3) assert.equal(evidence.catalog_id, currentSnapshot.catalog_id, 'Unknown catalog');
  assert(
    (evidence.schema_version === 1 || v2) &&
      version(evidence.build_version) &&
      integer(evidence.sampled_at) &&
      evidence.sampled_at > 0,
    'Invalid evidence version/time',
  );
  assert(
    evidence.mode === 'read_only' &&
      evidence.deletion_performed === false &&
      evidence.acceptance_status === 'not_run',
    'Read-only evidence cannot establish real deletion acceptance',
  );
  assert.equal(evidence.scan_scope, 'builtin_application_rules_without_user_exclusions');
  assert(
    Array.isArray(evidence.applications) && evidence.applications.length === expectedApps.size,
    'Application inventory does not match compiled catalog',
  );
  assert(
    Array.isArray(evidence.rules) && evidence.rules.length === expectedCatalog.length,
    'Rule inventory does not match catalog',
  );
  const appIds = new Set();
  for (const app of evidence.applications) {
    keys(
      app,
      'app_id version supported_version identity_verified scan_supported status running_component_count rules',
    );
    const expected = expectedCatalog.filter((r) => r.app_id === app.app_id);
    assert(expected.length && !appIds.has(app.app_id), 'Unknown/duplicate app');
    appIds.add(app.app_id);
    assert.equal(
      app.supported_version,
      expected[0].supported_version,
      'Supported version mismatch',
    );
    assert(app.version === null || version(app.version), 'Invalid app version');
    assert(
      typeof app.identity_verified === 'boolean' &&
        typeof app.scan_supported === 'boolean' &&
        reason(app.status) &&
        integer(app.running_component_count),
      'Missing application result',
    );
    assert(
      !app.scan_supported ||
        (app.identity_verified && (v2 || app.version === app.supported_version)),
      'Conflicting supported version',
    );
    assert(
      Array.isArray(app.rules) && app.rules.length === expected.length,
      'Missing per-rule capability',
    );
    assert.deepEqual(app.rules.map((r) => r.rule_id).sort(), expected.map((r) => r.rule_id).sort());
    for (const rule of app.rules) {
      keys(
        rule,
        v2 ? 'rule_id cleanup_validated scan_basis layout_id' : 'rule_id cleanup_validated',
      );
      assert.equal(rule.cleanup_validated, false, 'Application rules remain readonly');
      if (v2) {
        const spec = expected.find((r) => r.rule_id === rule.rule_id);
        assert(
          ['verified_version', 'recognized_layout', 'unverified'].includes(rule.scan_basis),
          'Invalid scan basis',
        );
        assert.equal(
          rule.layout_id,
          rule.scan_basis === 'unverified' ? null : spec.layout_id,
          'Layout does not match compiled rule',
        );
        assert(
          rule.scan_basis === 'unverified' || (app.identity_verified && app.version !== null),
          'Untrusted layout claim',
        );
        if (rule.scan_basis === 'verified_version')
          assert.equal(app.version, app.supported_version, 'Observed version mismatch');
        appRules.set(rule.rule_id, rule);
      }
    }
    if (v2)
      assert.equal(
        app.scan_supported,
        app.rules.some((r) => r.scan_basis !== 'unverified'),
        'Conflicting scan support',
      );
  }
  const seen = new Set();
  for (const row of evidence.rules) {
    keys(
      row,
      'rule_id rule_version app_id min_age_days data_kind checked files logical_bytes retained issues notices coverage_complete error_code blocked elapsed_ms inspection_status acceptance_status deletion_performed' +
        (v2 ? ' scan_basis layout_id' : ''),
    );
    const expected = expectedCatalog.find((r) => r.rule_id === row.rule_id);
    assert(expected && !seen.has(row.rule_id), 'Unknown/duplicate rule');
    seen.add(row.rule_id);
    assert(
      row.app_id === expected.app_id &&
        row.min_age_days === expected.min_age_days &&
        row.rule_version === (v2 ? expected.rule_version : 1),
      'Rule scope mismatch',
    );
    assert.equal(
      row.data_kind,
      v2 ? expected.data_kind : row.app_id === 'qq' ? 'cache' : 'diagnostics',
    );
    if (v2) {
      assert.equal(
        row.scan_basis,
        appRules.get(row.rule_id)?.scan_basis,
        'Inconsistent scan basis',
      );
      assert.equal(row.layout_id, appRules.get(row.rule_id)?.layout_id, 'Inconsistent layout');
    }
    assert(
      integer(row.checked) &&
        integer(row.files) &&
        integer(row.elapsed_ms) &&
        typeof row.logical_bytes === 'string' &&
        /^(0|[1-9]\d*)$/.test(row.logical_bytes),
      'Invalid scan counts',
    );
    assert(
      BigInt(row.logical_bytes) <= 18446744073709551615n &&
        (row.files > 0 || row.logical_bytes === '0'),
      'Inconsistent bytes',
    );
    [row.retained, row.issues, row.notices].forEach(reasons);
    assert(reason(row.error_code) && reason(row.blocked), 'Non-whitelisted error');
    assert.equal(
      row.coverage_complete,
      row.error_code === null && !row.issues.length,
      'Coverage mismatch',
    );
    assert.equal(row.inspection_status, row.error_code === null ? 'read_only' : 'failed');
    assert(
      row.acceptance_status === 'not_run' && row.deletion_performed === false,
      'A scan is not real deletion acceptance',
    );
  }
  return {
    evidence_complete: true,
    inspection_status: evidence.rules.some((r) => r.inspection_status === 'failed')
      ? 'failed'
      : 'read_only',
    acceptance_status: 'not_run',
    may_enable_cleanup: false,
    rules: evidence.rules.map((r) => ({
      rule_id: r.rule_id,
      inspection_status: r.inspection_status,
      acceptance_status: 'not_run',
      candidate_files: r.files,
    })),
    version_mismatches: evidence.applications
      .filter((a) => a.version !== a.supported_version)
      .map((a) => a.app_id),
  };
}

export async function checkBundle(folder) {
  const root = await realpath(folder);
  const manifest = JSON.parse(await readFile(join(root, 'manifest.json'), 'utf8'));
  keys(manifest, 'schema_version attachments');
  assert(
    manifest.schema_version === 1 &&
      Array.isArray(manifest.attachments) &&
      manifest.attachments.length === 1,
    'Invalid manifest',
  );
  const attachment = manifest.attachments[0];
  keys(attachment, 'name bytes sha256');
  assert(
    attachment.name === 'evidence.json' &&
      integer(attachment.bytes) &&
      /^[0-9a-f]{64}$/.test(attachment.sha256),
    'Invalid attachment',
  );
  const path = join(root, attachment.name);
  assert(!(await lstat(path)).isSymbolicLink(), 'Symlink evidence is not supported');
  const relativePath = relative(root, await realpath(path));
  assert(!relativePath.startsWith('..') && !isAbsolute(relativePath), 'Attachment escaped bundle');
  const bytes = await readFile(path);
  assert(
    bytes.length === attachment.bytes && sha256(bytes) === attachment.sha256,
    'Attachment hash mismatch',
  );
  return validateEvidence(JSON.parse(bytes.toString('utf8')));
}

async function main() {
  const [operation, folder] = process.argv.slice(2);
  if (operation === 'catalog') {
    console.log(JSON.stringify(currentSnapshot, null, 2));
    return;
  }
  assert(
    ['collect', 'check'].includes(operation) && folder,
    'Usage: node scripts/application-evidence.mjs catalog | collect|check <local-folder>',
  );
  if (operation === 'collect') {
    // Refuse to overwrite an existing evidence package.
    await mkdir(resolve(folder), { recursive: false });
    const { stdout } = await promisify(execFile)(
      'cargo',
      ['run', '--locked', '--quiet', '-p', 'dustaway-windows', '--example', 'application_scan'],
      { cwd: project, windowsHide: true, maxBuffer: 8 * 1024 * 1024 },
    );
    const evidence = JSON.parse(stdout.replace(/^\uFEFF/, ''));
    validateEvidence(evidence);
    const bytes = Buffer.from(JSON.stringify(evidence, null, 2) + '\n');
    await writeFile(join(folder, 'evidence.json'), bytes, { flag: 'wx' });
    await writeFile(
      join(folder, 'manifest.json'),
      JSON.stringify(
        {
          schema_version: 1,
          attachments: [{ name: 'evidence.json', bytes: bytes.length, sha256: sha256(bytes) }],
        },
        null,
        2,
      ) + '\n',
      { flag: 'wx' },
    );
  }
  console.log(JSON.stringify(await checkBundle(folder), null, 2));
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main().catch((error) => {
    // Do not echo child stdout/stderr, paths, or arbitrary object values.
    console.error(
      'Evidence command failed. Existing files are preserved; check schema, hash and build status.',
    );
    if (error instanceof assert.AssertionError) console.error(String(error.message).split('\n')[0]);
    process.exitCode = 1;
  });
}
