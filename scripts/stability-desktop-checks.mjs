// Run only after building the ordinary custom-protocol desktop executable.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
const output = 'artifacts/stability-desktop';
await mkdir(output, { recursive: true });
const results = [];
async function run(name, script, env = {}) {
  console.log(`Starting ${name}`);
  const started = Date.now();
  const child = spawn(process.execPath, [script], {
    env: { ...process.env, ...env },
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let log = '';
  child.stdout.on('data', (chunk) => {
    log += chunk;
  });
  child.stderr.on('data', (chunk) => {
    log += chunk;
  });
  const code = await new Promise((resolve, reject) => {
    child.on('error', reject);
    child.on('close', resolve);
  });
  await writeFile(`${output}/${name}.log`, log);
  results.push({ name, code, elapsed_ms: Date.now() - started });
  await writeFile(
    `${output}/results.json`,
    JSON.stringify({ synthetic: true, realApplicationAcceptance: 'not_run', results }, null, 2),
  );
  assert.equal(code, 0, `${name} failed: ${log.slice(-4000)}`);
  console.log(`PASS ${name}`);
}
await run('disks', 'scripts/disk-selection-checks.mjs');
for (const theme of ['light', 'dark']) {
  for (const width of [760, 1100]) {
    await run(`${theme}-${width}`, 'scripts/manual-standard-check.mjs', {
      DUSTAWAY_TEST_THEME: theme,
      DUSTAWAY_TEST_WIDTH: String(width),
    });
  }
}
await run('native-window', 'scripts/desktop-window-check.mjs');
