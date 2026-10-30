// Local, blind annotation and coverage reports. Pending labels are never accuracy labels.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const labels = new Set(['cache', 'logs', 'temporary', 'personal', 'application_data']);
export const fingerprint = (value) =>
  createHash('sha256').update(JSON.stringify(value)).digest('hex');
export function validateObservations(inventory, predictions) {
  assert.equal(inventory.schema_version, 1);
  assert.equal(predictions.schema_version, 1);
  assert.equal(
    predictions.inventory_sha256,
    fingerprint(inventory),
    'Inventory does not match frozen predictions',
  );
  const ids = new Set();
  const groups = new Map();
  for (const row of inventory.observations) {
    assert(!ids.has(row.id), 'Duplicate observation ID');
    ids.add(row.id);
    assert(['development', 'holdout'].includes(row.partition));
    assert(
      !groups.has(row.group) || groups.get(row.group) === row.partition,
      'Group crosses partitions',
    );
    groups.set(row.group, row.partition);
  }
  assert.equal(predictions.predictions.length, ids.size);
  const sources = new Map(inventory.observations.map((row) => [row.id, row]));
  const seen = new Set();
  for (const row of predictions.predictions) {
    assert(ids.has(row.id) && !seen.has(row.id), 'Missing, extra or duplicated prediction');
    seen.add(row.id);
    assert(
      row.prediction || (sources.get(row.id).partition === 'holdout' && row.withheld === true),
      'Development prediction missing',
    );
  }
}

export function coverageReport(inventory, predictions) {
  validateObservations(inventory, predictions);
  const byId = new Map(predictions.predictions.map((p) => [p.id, p.prediction]));
  const groups = {};
  for (const row of inventory.observations.filter((r) => r.partition === 'development')) {
    const key = `${row.group}/${row.reference}/${row.directory ? 'directory' : 'file'}`;
    const stat = (groups[key] ??= {
      observations: 0,
      predicted: 0,
      agrees_with_scope_reference: 0,
      disagrees_with_scope_reference: 0,
      abstention_reasons: {},
      prediction_counts: {},
    });
    const p = byId.get(row.id);
    stat.observations++;
    if (p.hint) {
      stat.predicted++;
      stat.prediction_counts[p.hint.purpose] = (stat.prediction_counts[p.hint.purpose] ?? 0) + 1;
      if (p.hint.purpose === row.reference) stat.agrees_with_scope_reference++;
      else stat.disagrees_with_scope_reference++;
    } else stat.abstention_reasons[p.abstention] = (stat.abstention_reasons[p.abstention] ?? 0) + 1;
  }
  const sum = (field) => Object.values(groups).reduce((n, s) => n + s[field], 0);
  return {
    schema_version: 1,
    model_id: predictions.model_id,
    inventory_sha256: predictions.inventory_sha256,
    scope:
      'Single-machine, bounded application subtrees. Development only; scope references are not reviewed per-file ground truth.',
    development_observations: sum('observations'),
    predicted: sum('predicted'),
    agrees_with_scope_reference: sum('agrees_with_scope_reference'),
    disagrees_with_scope_reference: sum('disagrees_with_scope_reference'),
    holdout_observations_not_evaluated: inventory.observations.filter(
      (r) => r.partition === 'holdout',
    ).length,
    accuracy: null,
    reason: 'All observations are unlabeled; no accuracy can be computed.',
    groups,
  };
}

export function reviewTemplate(inventory, predictions, partition = 'development') {
  validateObservations(inventory, predictions);
  assert(
    ['development', 'holdout'].includes(partition),
    'Explicit valid review partition required',
  );
  // Deterministic within this frozen batch, balanced by group/scope/type. Prediction
  // scores and categories never affect sampling or appear in the annotation file.
  const buckets = new Map();
  for (const row of inventory.observations.filter((r) => r.partition === partition)) {
    const key = `${row.group}/${row.scope}/${row.directory}`;
    const values = buckets.get(key) ?? [];
    values.push(row);
    buckets.set(key, values);
  }
  const reviews = [...buckets.values()]
    .flatMap((rows) => rows.sort((a, b) => a.id.localeCompare(b.id)).slice(0, 4))
    .map((row) => ({
      id: row.id,
      group: row.group,
      path: row.path,
      directory: row.directory,
      review: { status: 'pending', label: null, reviewer: '', method: null, basis: '' },
    }));
  return {
    schema_version: 1,
    inventory_sha256: predictions.inventory_sha256,
    instructions:
      'Assign a purpose only after independent review. Declare method human or source_metadata; source-assisted metadata review is not human-confirmed ground truth. Use label null only for a reviewed ambiguous item. Do not copy model predictions. Paths stay local; do not commit this file.',
    reviews,
  };
}

export function exportReviewed(inventory, predictions, reviewFile, partition) {
  validateObservations(inventory, predictions);
  assert(['development', 'holdout'].includes(partition), 'Explicit partition required');
  assert.equal(reviewFile.schema_version, 1);
  assert.equal(
    reviewFile.inventory_sha256,
    predictions.inventory_sha256,
    'Review belongs to another inventory',
  );
  const rows = new Map(inventory.observations.map((row) => [row.id, row]));
  const seen = new Set();
  const samples = [];
  const methods = { human: 0, source_metadata: 0 };
  for (const item of reviewFile.reviews) {
    assert(!seen.has(item.id), 'Duplicate review ID');
    seen.add(item.id);
    const row = rows.get(item.id);
    assert(row, 'Unknown review ID');
    if (item.review.status === 'pending') continue;
    assert.equal(item.review.status, 'reviewed');
    assert(row.partition === partition, 'Cannot mix development and holdout reviews');
    assert(
      item.review.reviewer?.trim() && item.review.basis?.trim(),
      'Reviewer and independent evidence required',
    );
    assert(item.review.label === null || labels.has(item.review.label), 'Invalid reviewed label');
    assert(
      ['human', 'source_metadata'].includes(item.review.method),
      'Explicit review method required',
    );
    methods[item.review.method]++;
    samples.push({
      group: `${inventory.machine}-${row.group}`,
      split: 'test',
      path: row.path,
      directory: row.directory,
      label: item.review.label,
    });
  }
  assert(samples.length > 0, 'No reviewed labels; pending/null does not mean verified unknown');
  return {
    schema_version: 1,
    provenance: `Reviewed ${partition} evaluation export from inventory ${predictions.inventory_sha256}; review_sha256=${fingerprint(reviewFile)}; methods=${JSON.stringify(methods)}. Source metadata labels are provisional, not human-confirmed ground truth. Retains local paths; do not publish or auto-train.`,
    samples,
  };
}

async function main() {
  const [command, folder, ...args] = process.argv.slice(2);
  assert(
    folder,
    'Usage: report <directory> <new-output.json>; review <directory> <new-review.private.json> [--holdout]; rescore <directory> <new-output-directory>; export <directory> <review.private.json> <development|holdout> <new-dataset.private.json>',
  );
  const inventory = JSON.parse(await readFile(join(folder, 'observations.private.json'), 'utf8'));
  const predictions = JSON.parse(await readFile(join(folder, 'predictions.private.json'), 'utf8'));
  if (command === 'rescore') {
    assert.equal(args.length, 1);
    validateObservations(inventory, predictions);
    await mkdir(args[0]);
    const input = {
      schema_version: 1,
      items: inventory.observations
        .filter((row) => row.partition === 'development')
        .map((row) => ({ id: row.id, path: row.path, directory: row.directory })),
    };
    const batchPath = join(args[0], 'batch.private.json');
    await writeFile(batchPath, JSON.stringify(input), { flag: 'wx' });
    const inferred = JSON.parse(
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
          'predict-batch',
          batchPath,
        ],
        { encoding: 'utf8', windowsHide: true, maxBuffer: 32 * 1024 * 1024 },
      ),
    );
    inferred.inventory_sha256 = predictions.inventory_sha256;
    inferred.predictions.push(
      ...inventory.observations
        .filter((row) => row.partition === 'holdout')
        .map((row) => ({ id: row.id, partition: 'holdout', prediction: null, withheld: true })),
    );
    validateObservations(inventory, inferred);
    await writeFile(
      join(args[0], 'observations.private.json'),
      `${JSON.stringify(inventory, null, 2)}\n`,
      { flag: 'wx' },
    );
    await writeFile(
      join(args[0], 'predictions.private.json'),
      `${JSON.stringify(inferred, null, 2)}\n`,
      { flag: 'wx' },
    );
    const result = coverageReport(inventory, inferred);
    await writeFile(
      join(args[0], 'development-report.json'),
      `${JSON.stringify(result, null, 2)}\n`,
      { flag: 'wx' },
    );
    const { groups, ...summary } = result;
    console.log(JSON.stringify(summary, null, 2));
    return;
  }
  let result;
  let output;
  if (command === 'report' || command === 'review') {
    assert(
      args.length === 1 || (command === 'review' && args.length === 2 && args[1] === '--holdout'),
    );
    output = args[0];
    result =
      command === 'report'
        ? coverageReport(inventory, predictions)
        : reviewTemplate(
            inventory,
            predictions,
            args[1] === '--holdout' ? 'holdout' : 'development',
          );
  } else if (command === 'export') {
    assert.equal(args.length, 3);
    output = args[2];
    result = exportReviewed(
      inventory,
      predictions,
      JSON.parse(await readFile(args[0], 'utf8')),
      args[1],
    );
  } else throw Error('Unknown observation command');
  await writeFile(output, `${JSON.stringify(result, null, 2)}\n`, { flag: 'wx' });
  if (command === 'report') {
    const { groups, ...summary } = result;
    console.log(JSON.stringify(summary, null, 2));
  } else
    console.log(
      `Wrote ${command} output. Paths remain private; no training or deletion performed.`,
    );
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href)
  await main();
