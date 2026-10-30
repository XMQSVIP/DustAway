// Shared local/CI verification. Deliberate generators are followed by a source drift check.
import { execFileSync } from 'node:child_process';
import { readFileSync, existsSync, realpathSync, mkdirSync, mkdtempSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
// A managed cache may be a junction. Run test binaries through its real path,
// rather than weakening the filesystem tests' link rejection.
if (!process.env.CARGO_TARGET_DIR && existsSync(resolve(root, 'target'))) {
  process.env.CARGO_TARGET_DIR = realpathSync(resolve(root, 'target'));
}
// Keep disposable indices on the workspace volume; do not relax the 2 GiB reserve
// just to run tests on a nearly full system drive. These variables affect children only.
const taskTempParent = resolve(process.env.DUSTAWAY_TEST_TEMP_ROOT ?? resolve(root, 'artifacts'));
mkdirSync(taskTempParent, { recursive: true });
const taskTemp = mkdtempSync(resolve(taskTempParent, 'quality-temp-'));
process.env.TEMP = taskTemp;
process.env.TMP = taskTemp;
const pnpmCli = process.env.npm_execpath;
if (!pnpmCli) throw new Error('Run with pnpm check:quality');
const paths = () =>
  execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z'], {
    cwd: root,
  })
    .toString()
    .split('\0')
    .filter(Boolean);
const hashes = () =>
  new Map(
    paths().map((p) => [
      p,
      createHash('sha256')
        .update(readFileSync(resolve(root, p)))
        .digest('hex'),
    ]),
  );
const before = hashes();
const checks = [
  ['cargo', ['fmt', '--all', '--check']],
  ['cargo', ['clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings']],
  [process.execPath, [pnpmCli, 'check']],
  [process.execPath, [pnpmCli, 'test:release']],
  [process.execPath, [pnpmCli, 'test:stability-evidence']],
];
for (const [bin, args] of checks)
  execFileSync(bin, args, { cwd: root, stdio: 'inherit', windowsHide: true });
const after = hashes();
const changed = [...new Set([...before.keys(), ...after.keys()])].filter(
  (p) => before.get(p) !== after.get(p),
);
if (changed.length) throw new Error(`Checks rewrote tracked source: ${changed.join(', ')}`);
console.log('Quality checks passed; environment-specific ignored tests are not counted as passed.');
