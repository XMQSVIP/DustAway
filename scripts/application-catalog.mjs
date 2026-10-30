// Generated projections of the compiled Rust registry. --check never rewrites files.
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { readFile, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';

const project = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const { stdout } = await promisify(execFile)(
  'cargo',
  ['run', '--quiet', '--locked', '-p', 'dustaway-core', '--example', 'application_catalog'],
  { cwd: project, windowsHide: true, maxBuffer: 4 * 1024 * 1024 },
);
const compiled = JSON.parse(stdout.replace(/^\uFEFF/, ''));
assert.equal(compiled.schema_version, 3);
const evidence = JSON.parse(
  await readFile(resolve(project, 'rules/cleanup-evidence.json'), 'utf8'),
);
const ledger = [...compiled.file_cleanup_rule_ids, ...compiled.official_tool_rule_ids].map((id) => {
  const rule = compiled.preview_rules.find((r) => r.id === id);
  const record = evidence.find((e) => e.rule_id === id);
  const release = compiled.release_evidence.filter((e) => e.rule_id === id);
  const official = compiled.official_tool_rule_ids.includes(id);
  const focusedTests = official
    ? id === 'dev.uv'
      ? ['crates/windows/src/uv_cache.rs']
      : ['crates/windows/src/maintenance.rs', 'crates/windows/src/maintenance_tools.rs']
    : rule.root === 'developer'
      ? ['crates/windows/src/developer_tests.rs']
      : rule.root === 'application_cache'
        ? ['crates/windows/src/app_cache_tests.rs', 'crates/windows/src/documented_cache_tests.rs']
        : rule.app_id
          ? ['crates/windows/src/application_acceptance_tests.rs']
          : ['crates/windows/src/scan_tests.rs'];
  return {
    rule_id: id,
    rule_version: rule.version,
    title: rule.title,
    operation: official ? 'official_tool' : 'files',
    sources: record?.sources ?? [],
    registered_operation_validation:
      record?.real_acceptance ?? (release.length ? 'passed' : 'not_indexed'),
    release_evidence: release,
    isolated_tests: [
      ...focusedTests,
      'crates/core/tests/engine.rs',
      'crates/windows/tests/cleanup_release.rs',
    ],
    limitation:
      '测试路径表示覆盖入口，不是本次执行结果。历史操作验证不等于本次完整应用恢复验收；not_indexed 表示尚未归档到结构化台账。',
  };
});
for (const [name, value] of [
  [
    'scripts/application-catalog-v3.json',
    { catalog_id: compiled.catalog_id, rules: compiled.rules },
  ],
  ['src/preview-rules.json', compiled.preview_rules],
  ['docs/current-capabilities.json', compiled.support_summary],
  [
    'docs/acceptance-ledger.json',
    { schema_version: 1, version: compiled.support_summary.version, rules: ledger },
  ],
]) {
  const path = resolve(project, name);
  if (process.argv.includes('--check')) {
    assert.deepEqual(
      JSON.parse(await readFile(path, 'utf8')),
      value,
      `Registry projection is stale: ${name}`,
    );
  } else {
    await writeFile(path, JSON.stringify(value, null, 2) + '\n');
  }
}
console.log(
  `Compiled application registry: ${compiled.rules.length} rules; projections ${process.argv.includes('--check') ? 'verified' : 'updated'}.`,
);
