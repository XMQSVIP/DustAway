import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { resolve, join } from 'node:path';
const root = resolve('artifacts/rule-refresh-20261002/tool-fixtures');
const rows = JSON.parse(await readFile(join(root, 'manifest.json'), 'utf8'));
const results = [];
for (const row of rows) {
  const receipt = JSON.parse(await readFile(join(root, `${row.kind}-result.json`), 'utf8'));
  assert(receipt.ok);
  for (const sample of row.protected) {
    assert.equal(
      createHash('sha256')
        .update(await readFile(sample.path))
        .digest('hex'),
      sample.sha256,
    );
  }
  if (row.kind === 'go') {
    execFileSync(join(row.project, 'fixture.exe'), [], { windowsHide: true });
  }
  if (row.kind === 'yarn') {
    execFileSync(row.exe, ['-e', 'if(require("dustaway-isolated-fixture")!==42)process.exit(1)'], {
      cwd: row.project,
      windowsHide: true,
    });
  }
  results.push({
    ...receipt,
    protected_unchanged: row.protected.length,
    normal_use: row.kind === 'go' || row.kind === 'yarn',
  });
}
await writeFile(join(root, 'postcheck.json'), JSON.stringify(results, null, 2), { flag: 'wx' });
console.log(JSON.stringify(results, null, 2));
