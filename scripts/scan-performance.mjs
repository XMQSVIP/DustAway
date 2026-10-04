// Both binaries contain this same opt-in, read-only fixture bridge.
import { mkdir, readFile, writeFile, appendFile, copyFile, access } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
const output = resolve('artifacts/performance-0.8');
const root = resolve('artifacts/dustaway-perf-0.7');
await mkdir(output, { recursive: true });
await mkdir(root, { recursive: true });
await writeFile(join(root, '.dustaway-test-only'), 'performance-fixture-only');
const binaries = { baseline: join(output, 'baseline.exe'), current: join(output, 'current.exe') };
const cases = ['recent', 'mixed', 'old', 'deep', 'excluded'];
const hash = (x) => createHash('sha256').update(x).digest('hex');
async function run(version, mode, dataset) {
  const start = Date.now();
  await new Promise((resolve, reject) => {
    const child = spawn(
      binaries[version],
      ['platform::scan_benchmark::performance_fixture', '--ignored', '--exact', '--nocapture'],
      {
        windowsHide: true,
        env: {
          ...process.env,
          DUSTAWAY_PERF_ROOT: root,
          DUSTAWAY_PERF_MODE: mode,
          DUSTAWAY_PERF_CASE: dataset,
        },
        stdio: ['ignore', 'pipe', 'pipe'],
      },
    );
    let log = '';
    child.stdout.on('data', (x) => (log += x));
    child.stderr.on('data', (x) => (log += x));
    child.on('error', reject);
    child.on('exit', (code) =>
      code === 0 ? resolve() : reject(new Error(`${version}/${mode}/${dataset}: ${code}\n${log}`)),
    );
  });
  if (mode === 'seed') return;
  const result = JSON.parse(await readFile(join(root, 'result.json'), 'utf8'));
  const row = {
    version,
    mode,
    dataset,
    at: new Date().toISOString(),
    wall_ms: Date.now() - start,
    ...result,
  };
  if (mode !== 'analysis') {
    row.candidates_sha256 = hash(await readFile(join(root, 'candidates.json')));
    if (mode === 'scan')
      await copyFile(
        join(root, 'candidates.json'),
        join(output, `${version}-${dataset}-candidates.json`),
      );
  }
  return row;
}
if (process.argv.includes('--seed')) {
  for (const dataset of cases) {
    let exists = false;
    try {
      await access(join(root, dataset, 'local/Temp'));
      exists = true;
    } catch {
      /* new fixture */
    }
    assert(!exists, `Existing fixture: ${dataset}; preserve it and use --measure`);
    await run('baseline', 'seed', dataset);
    console.log(`Seeded ${dataset}`);
  }
} else if (process.argv.includes('--measure')) {
  const logFile = join(output, 'measurements.jsonl');
  try {
    await access(logFile);
    throw new Error('Preserve existing measurements before starting a new run');
  } catch (e) {
    if (e.code !== 'ENOENT') throw e;
  }
  const binary_hashes = {};
  for (const [version, path] of Object.entries(binaries))
    binary_hashes[version] = hash(await readFile(path));
  await writeFile(
    join(output, 'environment.json'),
    JSON.stringify(
      {
        at: new Date().toISOString(),
        node: process.version,
        architecture: process.arch,
        binary_hashes,
        fixture: root,
        sample_interval_ms: 5,
        sequence:
          'analysis first; one warm-up/version/case, then five alternating paired samples; no concurrent builds/tests',
      },
      null,
      2,
    ),
  );
  for (const [mode, dataset] of [['analysis', 'old'], ...cases.map((c) => ['scan', c])]) {
    for (let round = -1; round < 5; round++) {
      for (const version of round % 2 === 0 ? ['baseline', 'current'] : ['current', 'baseline']) {
        const row = { round, warmup: round < 0, ...(await run(version, mode, dataset)) };
        await appendFile(logFile, JSON.stringify(row) + '\n');
        console.log(`${mode}/${dataset}/${version}/${round}: ${row.elapsed_ms.toFixed(1)} ms`);
      }
    }
  }
  for (let round = 0; round < 20; round++) {
    const row = { round, warmup: false, ...(await run('current', 'cancel', 'old')) };
    await appendFile(logFile, JSON.stringify(row) + '\n');
    console.log(`Cancel ${round}: ${row.cancel_ms.toFixed(1)} ms`);
  }
} else if (!process.argv.includes('--summarize')) {
  throw new Error('Use --seed, --measure, or --summarize');
}
if (process.argv.includes('--measure') || process.argv.includes('--summarize')) {
  const rows = (await readFile(join(output, 'measurements.jsonl'), 'utf8'))
    .trim()
    .split('\n')
    .map(JSON.parse);
  const median = (values) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
  const comparisons = [];
  for (const [mode, dataset] of [...cases.map((c) => ['scan', c]), ['analysis', 'old']]) {
    const samples = rows.filter((r) => r.mode === mode && r.dataset === dataset && !r.warmup);
    const versions = {};
    for (const version of ['baseline', 'current']) {
      const selected = samples.filter((r) => r.version === version);
      assert.equal(selected.length, 5);
      versions[version] = {
        median_ms: median(selected.map((r) => r.elapsed_ms)),
        min_ms: Math.min(...selected.map((r) => r.elapsed_ms)),
        max_ms: Math.max(...selected.map((r) => r.elapsed_ms)),
        working_set: median(selected.map((r) => r.resources.working_set)),
        private_commit: median(selected.map((r) => r.resources.private_commit)),
        working_set_peak: Math.max(...selected.map((r) => r.resources.working_set)),
        private_commit_peak: Math.max(...selected.map((r) => r.resources.private_commit)),
        handles: Math.max(...selected.map((r) => r.resources.handles)),
      };
    }
    const duration_ratio = versions.current.median_ms / versions.baseline.median_ms;
    const median_peak_memory_ratio = Math.max(
      versions.current.working_set / versions.baseline.working_set,
      versions.current.private_commit / versions.baseline.private_commit,
    );
    const memory_ratio = Math.max(
      versions.current.working_set_peak / versions.baseline.working_set_peak,
      versions.current.private_commit_peak / versions.baseline.private_commit_peak,
    );
    const candidate_sets_equal =
      mode === 'analysis' || new Set(samples.map((r) => r.candidates_sha256)).size === 1;
    comparisons.push({
      mode,
      dataset,
      ...versions,
      duration_ratio,
      memory_ratio,
      median_peak_memory_ratio,
      candidate_sets_equal,
      passed: duration_ratio <= 1.1 && memory_ratio <= 1.1 && candidate_sets_equal,
    });
  }
  const cancellations = rows
    .filter((r) => r.mode === 'cancel')
    .map((r) => r.cancel_ms)
    .sort((a, b) => a - b);
  assert.equal(cancellations.length, 20);
  const summary = {
    comparisons,
    cancellation: {
      samples: cancellations,
      p95_ms: cancellations[18],
      passed: cancellations[18] <= 1000,
    },
    passed: comparisons.every((r) => r.passed) && cancellations[18] <= 1000,
  };
  await writeFile(join(output, 'summary.json'), JSON.stringify(summary, null, 2));
  console.log(JSON.stringify(summary, null, 2));
}
