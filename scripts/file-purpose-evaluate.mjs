// Evaluate the actual Rust predictor; never reimplement inference in JavaScript.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';

const check = process.argv.includes('--check');
const folder = 'models/file-purpose';
const baseline = JSON.parse(await readFile(`${folder}/baseline-v1.json`, 'utf8'));
const run = (command, path) =>
  JSON.parse(
    execFileSync(
      'cargo',
      [
        'run',
        '-q',
        '-p',
        'dustaway-core',
        '--locked',
        '--example',
        'file_purpose',
        '--',
        command,
        path,
      ],
      { encoding: 'utf8', windowsHide: true, maxBuffer: 16 * 1024 * 1024 },
    ),
  );
const comparisons = {};
let challenge;
const summarize = (expected, predicted) => ({
  samples: expected.length,
  predicted: predicted.filter((p) => p !== null).length,
  correct_predictions: predicted.filter((p, i) => p !== null && p === expected[i]).length,
  correct_unknown_abstentions: predicted.filter((p, i) => p === null && expected[i] === null)
    .length,
  incorrect_cache_log_temp_predictions: predicted.filter(
    (p, i) => ['cache', 'logs', 'temporary'].includes(p) && p !== expected[i],
  ).length,
});
for (const name of ['seed', 'challenge']) {
  const file = `${folder}/${name}.json`;
  const samples = JSON.parse(await readFile(file, 'utf8')).samples.filter(
    (s) => s.split === 'test',
  );
  assert.equal(
    createHash('sha256').update(JSON.stringify(samples)).digest('hex'),
    baseline.datasets[name].test_sha256,
    `${name} comparison set changed; do not compare different samples`,
  );
  const current = run('evaluate', file);
  const predicted = current.outcomes.map((r) => r.predicted);
  assert.equal(predicted.length, samples.length);
  comparisons[name] = {
    baseline: summarize(
      samples.map((s) => s.label),
      baseline.datasets[name].predictions,
    ),
    current: summarize(
      samples.map((s) => s.label),
      predicted,
    ),
  };
  if (name === 'challenge') challenge = current;
}
const grouped = run('cross-validate', `${folder}/seed.json`);
// Freeze the added regression independently of the original v1 comparison sets.
const databaseBaseline = JSON.parse(await readFile(`${folder}/baseline-v3-database.json`, 'utf8'));
const databaseSamples = JSON.parse(
  await readFile(`${folder}/database-boundary.json`, 'utf8'),
).samples;
assert.equal(
  createHash('sha256').update(JSON.stringify(databaseSamples)).digest('hex'),
  databaseBaseline.test_sha256,
  'Database boundary comparison set changed',
);
const databaseEvaluation = run('evaluate', `${folder}/database-boundary.json`);
assert.deepEqual(
  databaseEvaluation.outcomes.map((r) => r.predicted),
  databaseSamples.map((s) => s.label),
  'Database protection or positive controls regressed',
);
const { folds, ...summary } = grouped;
const report = {
  scope:
    'Synthetic development regressions and leave-one-training-group-out. No real-machine or deletion accuracy claim.',
  baseline_model_id: baseline.model_id,
  model_id: challenge.model_id,
  comparisons,
  challenge,
  database_boundary: {
    baseline_model_id: databaseBaseline.model_id,
    baseline: summarize(
      databaseSamples.map((s) => s.label),
      databaseBaseline.predictions,
    ),
    current: summarize(
      databaseSamples.map((s) => s.label),
      databaseEvaluation.outcomes.map((r) => r.predicted),
    ),
    evaluation: databaseEvaluation,
  },
  grouped_validation: {
    ...summary,
    folds: folds.map((f) => ({
      group: f.held_out_group,
      samples: f.evaluation.test_samples,
      predicted: f.evaluation.predicted,
      correct_predictions: f.evaluation.correct_predictions,
      incorrect_cache_log_temp_predictions: f.evaluation.incorrect_cache_log_temp_predictions,
    })),
  },
};
const output = `${JSON.stringify(report, null, 2)}\n`;
if (check)
  assert.equal(
    (await readFile(`${folder}/validation.json`, 'utf8')).replaceAll('\r\n', '\n'),
    output,
    'Validation report is stale; run pnpm model:evaluate',
  );
else await writeFile(`${folder}/validation.json`, output);
console.log(
  JSON.stringify(
    {
      mode: check ? 'check' : 'write',
      model_id: report.model_id,
      comparisons,
      database_boundary: {
        baseline: report.database_boundary.baseline,
        current: report.database_boundary.current,
      },
      grouped_validation: summary,
    },
    null,
    2,
  ),
);
