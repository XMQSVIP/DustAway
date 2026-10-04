import { mkdir, writeFile } from 'node:fs/promises';
import { resolve, join, basename, dirname } from 'node:path';

const root = resolve(process.argv[2] ?? 'artifacts/dustaway-analysis-bench-demo');
if (!basename(root).startsWith('dustaway-analysis-bench-')) {
  throw new Error('Use a new, dedicated dustaway-analysis-bench-* directory on a test volume');
}
await mkdir(dirname(root), { recursive: true });
await mkdir(root); // Refuse existing directories, including earlier fixtures.
await writeFile(join(root, '.dustaway-test-only'), 'readonly-analysis-benchmark');
for (const count of [100000, 500000]) {
  for (let group = 0; group < count / 1000; group++) {
    const folder = join(root, String(count), `目录${String(group).padStart(3, '0')}`);
    await mkdir(folder, { recursive: true });
    for (let first = 0; first < 1000; first += 32) {
      await Promise.all(
        Array.from({ length: Math.min(32, 1000 - first) }, (_, i) =>
          writeFile(join(folder, `文件${String(first + i).padStart(4, '0')}.dat`), 'fixture'),
        ),
      );
    }
  }
  console.log(`Seeded ${count} real files at ${join(root, String(count))}`);
}
