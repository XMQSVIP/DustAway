import { test } from 'node:test';
import assert from 'node:assert/strict';
import { rules, phases, validateStages } from './stability-acceptance.mjs';

function fixture(rule) {
  const request = {
    schema_version: 1,
    run_id: 'fixture-run-025',
    rule_id: rule,
    permission: 'evidence_only',
  };
  const stages = phases.map((phase, i) => ({
    ...request,
    phase,
    synthetic: false,
    status: 'passed',
    completed_at: i,
    binary_sha256: 'a'.repeat(64),
    installation_fingerprint: 'b'.repeat(64),
  }));
  Object.assign(stages[0], {
    ordinary_user: true,
    application_stopped: true,
    layout_verified: true,
    candidates: 2,
    candidate_sha256: 'c'.repeat(64),
    min_age_seconds: 604800,
    timestamps_modified: false,
  });
  Object.assign(stages[1], { protected_sha256: 'd'.repeat(64) });
  Object.assign(stages[2], {
    candidate_sha256: 'c'.repeat(64),
    deleted: 1,
    failed: 0,
    unknown: 0,
    unprocessed: 0,
  });
  Object.assign(stages[3], { protected_sha256: 'd'.repeat(64) });
  Object.assign(stages[4], {
    new_files_observed: true,
    first_load_ok: true,
    second_load_ok: true,
    target_id: 'test-page',
    new_log_observed: true,
  });
  Object.assign(stages[5], {
    observer_kind: 'human',
    normal_function_confirmed: true,
    target_id: 'test-page',
  });
  return { request, stages };
}
for (const rule of rules)
  test(`${rule}: requires independent complete evidence`, () => {
    const { request, stages } = fixture(rule);
    assert.equal(validateStages(request, stages).release_permission_changed, false);
    for (const mutate of [
      (s) => s.pop(),
      (s) => {
        s[0].candidates = 0;
      },
      (s) => {
        s[0].ordinary_user = false;
      },
      (s) => {
        s[2].unknown = 1;
      },
      (s) => {
        s[2].deleted = 0;
      },
      (s) => {
        s[2].rule_id = 'foreign';
      },
      (s) => {
        s[3].protected_sha256 = 'e'.repeat(64);
      },
      (s) => {
        s[4].installation_fingerprint = 'e'.repeat(64);
      },
      (s) => {
        s[5].observer_kind = 'automation';
      },
      (s) => {
        s[2].synthetic = true;
      },
    ]) {
      const copy = structuredClone(stages);
      mutate(copy);
      assert.throws(() => validateStages(request, copy));
    }
    const copy = structuredClone(stages);
    if (rule.startsWith('weixin.')) copy[4].first_load_ok = false;
    else copy[0].timestamps_modified = true;
    assert.throws(() => validateStages(request, copy));
  });
