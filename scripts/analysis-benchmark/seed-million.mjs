import { mkdir, writeFile } from 'node:fs/promises';
import { resolve, join, basename } from 'node:path';

const root = resolve(process.argv[2]);
if (!basename(root).startsWith('dustaway-analysis-bench-'))
  throw new Error('Dedicated fixture root required');
await mkdir(root); // Never reuse or delete an existing directory.
await writeFile(join(root, '.dustaway-test-only'), 'readonly-analysis-benchmark');
for (let group = 0; group < 3000; group++) {
  const folder = join(
    root,
    '3000000',
    group < 1000 ? 'first-million' : 'remaining',
    `目录${String(group).padStart(4, '0')}`,
  );
  await mkdir(folder, { recursive: true });
  for (let first = 0; first < 1000; first += 32) {
    await Promise.all(
      Array.from({ length: Math.min(32, 1000 - first) }, (_, i) =>
        writeFile(join(folder, `文件${String(first + i).padStart(4, '0')}.dat`), 'fixture'),
      ),
    );
  }
  if ((group + 1) % 100 === 0)
    console.log(JSON.stringify({ files: (group + 1) * 1000, at: new Date().toISOString() }));
}
