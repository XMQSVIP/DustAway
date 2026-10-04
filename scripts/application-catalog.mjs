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
for (const [name, value] of [
  [
    'scripts/application-catalog-v3.json',
    { catalog_id: compiled.catalog_id, rules: compiled.rules },
  ],
  ['src/preview-rules.json', compiled.preview_rules],
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
