// Read-only backend comparison. Never uses application data or deletes fixture files.
import assert from 'node:assert/strict';
import { mkdir, writeFile, readFile, utimes } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
const output = resolve(
  process.env.DUSTAWAY_HOME_PERF_OUTPUT ?? 'artifacts/home-cleanup-real-20261001/scan-performance',
);
const root = join(output, 'dustaway-perf-home');
await mkdir(output); // Existing evidence is never overwritten.
await mkdir(root);
await writeFile(join(root, '.dustaway-test-only'), 'performance-fixture-only');
const binaries = {
  baseline: resolve(
    process.env.DUSTAWAY_HOME_PERF_BASELINE ??
      'artifacts/home-cleanup-real-20261001/performance-baseline.exe',
  ),
  current: resolve(
    process.env.DUSTAWAY_HOME_PERF_CURRENT ??
      'artifacts/home-cleanup-real-20261001/performance-current.exe',
  ),
};
const sha = (bytes) => createHash('sha256').update(bytes).digest('hex');
const hashes = Object.fromEntries(
  await Promise.all(
    Object.entries(binaries).map(async ([name, path]) => [name, sha(await readFile(path))]),
  ),
);
const old = new Date(Date.now() - 30 * 86400_000);
// Same fixed 10,000 ordinary files for both binaries, all inside the new marked fixture.
for (let group = 0; group < 100; group++) {
  const directory = join(root, 'old/local/Temp', `group-${group}`);
  await mkdir(directory, { recursive: true });
  for (let file = 0; file < 100; file++) {
    const path = join(directory, `${file}.tmp`);
    await writeFile(path, 'readonly-performance-fixture');
    await utimes(path, old, old);
  }
}
const rows = [];
for (let round = -1; round < 5; round++) {
  for (const name of round % 2 === 0 ? ['baseline', 'current'] : ['current', 'baseline']) {
    await promisify(execFile)(
      binaries[name],
      ['platform::scan_benchmark::performance_fixture', '--exact', '--ignored'],
      {
        windowsHide: true,
        env: {
          ...process.env,
          DUSTAWAY_PERF_ROOT: root,
          DUSTAWAY_PERF_MODE: 'scan',
          DUSTAWAY_PERF_CASE: 'old',
        },
      },
    );
    const result = JSON.parse(await readFile(join(root, 'result.json'), 'utf8'));
    assert.equal(result.summary.rules[0].files, 10000);
    rows.push({
      name,
      round,
      warmup: round < 0,
      candidates_sha256: sha(await readFile(join(root, 'candidates.json'))),
      ...result,
    });
    await writeFile(join(output, 'measurements.json'), JSON.stringify(rows, null, 2));
    console.log(`${name}/${round}: ${result.elapsed_ms.toFixed(2)} ms`);
  }
}
const median = (values) => values.sort((a, b) => a - b)[Math.floor(values.length / 2)];
const baseline = median(
  rows.filter((r) => r.name === 'baseline' && !r.warmup).map((r) => r.elapsed_ms),
);
const current = median(
  rows.filter((r) => r.name === 'current' && !r.warmup).map((r) => r.elapsed_ms),
);
const equal = new Set(rows.map((r) => r.candidates_sha256)).size === 1;
const report = {
  scope:
    '10,000 synthetic user-temp files; backend scan only, not frontend rendering or real-app throughput',
  binary_sha256: hashes,
  binary_paths: binaries,
  baseline_ms: baseline,
  current_ms: current,
  ratio: current / baseline,
  candidates_equal: equal,
  passed: equal && current / baseline <= 1.1,
};
await writeFile(join(output, 'summary.json'), JSON.stringify(report, null, 2));
console.log(JSON.stringify(report));
assert(report.passed, 'Performance comparison failed; retain evidence');
