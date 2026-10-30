import test from 'node:test';
import assert from 'node:assert/strict';
import {
  fingerprint,
  coverageReport,
  reviewTemplate,
  exportReviewed,
  validateObservations,
} from './file-purpose-observations.mjs';

function fixture() {
  const inventory = {
    schema_version: 1,
    machine: 'test-machine',
    observations: [
      {
        id: 'a',
        group: 'app',
        partition: 'development',
        scope: 0,
        path: 'D:/private-app/Cache/item.cache',
        directory: false,
        reference: 'cache',
      },
      {
        id: 'b',
        group: 'app',
        partition: 'development',
        scope: 1,
        path: 'D:/private-app/data.db',
        directory: false,
        reference: 'application_data',
      },
      {
        id: 'c',
        group: 'other',
        partition: 'holdout',
        scope: 2,
        path: 'D:/private-other/Cache/item.cache',
        directory: false,
        reference: 'cache',
      },
    ],
  };
  const predictions = {
    schema_version: 1,
    model_id: 'test-model',
    inventory_sha256: fingerprint(inventory),
    predictions: [
      { id: 'a', prediction: { hint: { purpose: 'cache' }, abstention: null } },
      { id: 'b', prediction: { hint: null, abstention: 'sensitive_location' } },
      { id: 'c', prediction: null, withheld: true },
    ],
  };
  return { inventory, predictions };
}
test('coverage has no raw paths or accuracy and excludes holdout results', () => {
  const { inventory, predictions } = fixture();
  const report = coverageReport(inventory, predictions);
  assert.equal(report.development_observations, 2);
  assert.equal(report.predicted, 1);
  assert.equal(report.accuracy, null);
  assert.equal(report.holdout_observations_not_evaluated, 1);
  assert(!JSON.stringify(report).includes('private-app'));
  assert(!JSON.stringify(report).includes('private-other'));
});
test('annotation is blind, pending is not verified unknown, and no automatic export occurs', () => {
  const { inventory, predictions } = fixture();
  const template = reviewTemplate(inventory, predictions);
  assert.equal(template.reviews.length, 2);
  const holdout = reviewTemplate(inventory, predictions, 'holdout');
  assert.equal(holdout.reviews.length, 1);
  assert.equal(holdout.reviews[0].id, 'c');
  assert(template.reviews.every((r) => r.review.status === 'pending'));
  assert(!JSON.stringify(template).includes('test-model'));
  assert.throws(
    () => exportReviewed(inventory, predictions, template, 'development'),
    /No reviewed labels/,
  );
  const changed = structuredClone(predictions);
  changed.predictions[0].prediction.hint.purpose = 'logs';
  assert.deepEqual(reviewTemplate(inventory, changed), template);
});
test('review needs evidence and is bound to the frozen inventory, not edited paths', () => {
  const { inventory, predictions } = fixture();
  const template = reviewTemplate(inventory, predictions);
  template.reviews[0].review = {
    status: 'reviewed',
    label: 'cache',
    reviewer: 'tester',
    method: 'human',
    basis: 'independent fixture evidence',
  };
  template.reviews[0].path = 'D:/tampered/path';
  const exported = exportReviewed(inventory, predictions, template, 'development');
  assert.equal(exported.samples[0].path, inventory.observations[0].path);
  assert.equal(exported.samples[0].split, 'test');
  assert(exported.provenance.includes('"human":1'));
  assert(exported.provenance.includes(`review_sha256=${fingerprint(template)}`));
  template.reviews[0].review.method = 'source_metadata';
  assert(
    exportReviewed(inventory, predictions, template, 'development').provenance.includes(
      '"source_metadata":1',
    ),
  );
  template.reviews[0].review.method = null;
  assert.throws(
    () => exportReviewed(inventory, predictions, template, 'development'),
    /review method/,
  );
  template.reviews[0].review.method = 'human';
  template.reviews[0].review.label = null;
  assert.equal(
    exportReviewed(inventory, predictions, template, 'development').samples[0].label,
    null,
  );
  template.reviews[0].review.basis = '';
  assert.throws(
    () => exportReviewed(inventory, predictions, template, 'development'),
    /independent evidence/,
  );
  template.inventory_sha256 = 'wrong';
  assert.throws(
    () => exportReviewed(inventory, predictions, template, 'development'),
    /another inventory/,
  );
});
test('cross-partition labels, duplicate predictions and changed inventories are rejected', () => {
  const { inventory, predictions } = fixture();
  const template = reviewTemplate(inventory, predictions);
  template.reviews = [
    {
      id: 'c',
      review: { status: 'reviewed', label: 'cache', reviewer: 'tester', basis: 'fixture' },
    },
  ];
  assert.throws(() => exportReviewed(inventory, predictions, template, 'development'), /mix/);
  const duplicate = structuredClone(predictions);
  duplicate.predictions[1] = duplicate.predictions[0];
  assert.throws(() => validateObservations(inventory, duplicate), /duplicated/);
  inventory.observations[0].path = 'D:/changed';
  assert.throws(() => validateObservations(inventory, predictions), /does not match/);
});
