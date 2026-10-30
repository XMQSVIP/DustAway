// Evidence intake only. No command here changes production permission or deletes files.
import assert from 'node:assert/strict';
import { randomUUID, createHash } from 'node:crypto';
import { mkdir, readFile, writeFile, lstat } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';

export const rules = [
  'weixin.code',
  'weixin.gpu',
  'appcache.dingtalk_logs',
  'appcache.todesk_logs',
];
export const phases = [
  'preflight',
  'protected_before',
  'cleanup',
  'protected_after',
  'recovery',
  'human_observation',
];
const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');
export function validateStages(request, stages) {
  assert.equal(request.schema_version, 1);
  assert(rules.includes(request.rule_id));
  assert.match(request.run_id, /^[a-zA-Z0-9-]{8,80}$/);
  assert.equal(request.permission, 'evidence_only');
  assert.deepEqual(
    stages.map((s) => s.phase),
    phases,
    'Missing or reordered phases',
  );
  let last = -1;
  for (const stage of stages) {
    assert.equal(stage.run_id, request.run_id);
    assert.equal(stage.rule_id, request.rule_id);
    assert.equal(stage.synthetic, false, 'Synthetic evidence cannot authorize real acceptance');
    assert.equal(stage.status, 'passed');
    assert(Number.isSafeInteger(stage.completed_at) && stage.completed_at >= last);
    last = stage.completed_at;
    assert.match(stage.binary_sha256, /^[a-f0-9]{64}$/);
    assert.match(stage.installation_fingerprint, /^[a-f0-9]{64}$/);
    assert.equal(stage.binary_sha256, stages[0].binary_sha256);
    assert.equal(stage.installation_fingerprint, stages[0].installation_fingerprint);
  }
  const [pre, before, cleanup, after, recovery, human] = stages;
  assert.equal(pre.ordinary_user, true);
  assert.equal(pre.application_stopped, true);
  assert.equal(pre.layout_verified, true);
  assert(Number.isSafeInteger(pre.candidates) && pre.candidates > 0);
  assert.match(pre.candidate_sha256, /^[a-f0-9]{64}$/);
  assert.equal(cleanup.candidate_sha256, pre.candidate_sha256);
  assert(
    Number.isSafeInteger(cleanup.deleted) &&
      cleanup.deleted > 0 &&
      cleanup.deleted <= pre.candidates,
  );
  for (const field of ['failed', 'unknown', 'unprocessed']) assert.equal(cleanup[field], 0);
  assert.match(before.protected_sha256, /^[a-f0-9]{64}$/);
  assert.equal(after.protected_sha256, before.protected_sha256);
  assert.equal(recovery.new_files_observed, true);
  assert.equal(human.observer_kind, 'human');
  assert.equal(human.normal_function_confirmed, true);
  if (request.rule_id.startsWith('weixin.')) {
    assert.equal(recovery.first_load_ok, true);
    assert.equal(recovery.second_load_ok, true);
    assert.equal(typeof recovery.target_id, 'string');
    assert(recovery.target_id.length > 0);
    assert.equal(human.target_id, recovery.target_id);
  } else {
    assert.equal(pre.min_age_seconds, 7 * 86400);
    assert.equal(pre.timestamps_modified, false);
    assert.equal(recovery.new_log_observed, true);
  }
  return { rule_id: request.rule_id, evidence_complete: true, release_permission_changed: false };
}
export async function checkEvidence(directory) {
  const read = async (name) =>
    JSON.parse((await readFile(join(directory, name), 'utf8')).replace(/^\uFEFF/, ''));
  const request = await read('request.json');
  const index = await read('evidence.json');
  assert.equal(index.run_id, request.run_id);
  assert.deepEqual(
    index.files.map((f) => f.name),
    phases.map((p) => `${p}.json`),
  );
  const stages = [];
  for (const f of index.files) {
    const path = join(directory, f.name);
    assert(!(await lstat(path)).isSymbolicLink());
    const data = await readFile(path);
    assert.equal(data.length, f.bytes);
    assert.equal(hash(data), f.sha256);
    stages.push(JSON.parse(data.toString('utf8').replace(/^\uFEFF/, '')));
  }
  return validateStages(request, stages);
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [command, target, rule] = process.argv.slice(2);
  assert(target, 'Supply a new run directory');
  const directory = resolve(target);
  if (command === 'init') {
    assert(rules.includes(rule), 'Only the four reviewed candidate rules are supported');
    await mkdir(directory);
    await writeFile(
      join(directory, 'request.json'),
      JSON.stringify(
        {
          schema_version: 1,
          run_id: randomUUID(),
          rule_id: rule,
          permission: 'evidence_only',
          required_phases: phases,
        },
        null,
        2,
      ) + '\n',
      { flag: 'wx' },
    );
    console.log(
      'Created evidence request. No tests or deletion executed; missing phases remain unverified.',
    );
  } else {
    assert.equal(command, 'check');
    console.log(JSON.stringify(await checkEvidence(directory)));
  }
}
