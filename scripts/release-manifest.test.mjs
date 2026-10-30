import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { packageFiles, checkManifest, validateManifest, signature } from './release-manifest.mjs';

test(
  'Windows signature inspection isolates inherited PowerShell module paths',
  { skip: process.platform !== 'win32' },
  () => {
    const previous = process.env.PSModulePath;
    try {
      process.env.PSModulePath = 'Z:\\dustaway-nonexistent-modules';
      assert(signature(process.execPath).length > 0);
    } finally {
      if (previous === undefined) delete process.env.PSModulePath;
      else process.env.PSModulePath = previous;
    }
  },
);

test('package verification rejects modified, omitted and extra payloads', async () => {
  const root = await mkdtemp(join(tmpdir(), 'dustaway-manifest-'));
  await writeFile(join(root, 'app.exe'), 'synthetic executable');
  const value = {
    schema_version: 1,
    version: '0.25.0',
    profile: 'portable',
    features: ['custom-protocol'],
    source_sha256: 'a'.repeat(64),
    catalog_id: 'sha256:' + 'b'.repeat(64),
    rule_catalog_id: 'sha256:' + 'c'.repeat(64),
    validation: 'candidate_not_stability_accepted',
    signatures: [{ path: 'app.exe', status: 'NotSigned' }],
    files: await packageFiles(root),
  };
  const path = join(root, 'build-manifest.json');
  await writeFile(path, JSON.stringify(value));
  await checkManifest(path, () => 'NotSigned');
  await assert.rejects(
    checkManifest(path, () => 'Valid'),
    /Signature changed/,
  );
  await writeFile(join(root, 'extra.txt'), 'unexpected');
  await assert.rejects(
    checkManifest(path, () => 'NotSigned'),
    /Package content changed/,
  );
  await writeFile(join(root, 'app.exe'), 'changed');
  await assert.rejects(checkManifest(path, () => 'NotSigned'));
  assert.throws(() => validateManifest({ ...value, features: ['system-cleanup'] }));
  assert.throws(() =>
    validateManifest({ ...value, files: [{ ...value.files[0], path: '../escape.exe' }] }),
  );
  assert.throws(() => validateManifest({ ...value, signatures: [] }));
  assert.throws(() => validateManifest({ ...value, validation: 'passed' }));
});
