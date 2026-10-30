import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { lstat, readdir, readFile, writeFile } from 'node:fs/promises';
import { resolve, relative, join, dirname, isAbsolute, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const project = fileURLToPath(new URL('../', import.meta.url));
const digest = (value) => createHash('sha256').update(value).digest('hex');
const readJson = async (path) => JSON.parse((await readFile(path, 'utf8')).replace(/^\uFEFF/, ''));
export async function sourceSnapshot(root = project) {
  const paths = [
    ...new Set(
      execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z'], {
        cwd: root,
      })
        .toString('utf8')
        .split('\0')
        .filter(Boolean),
    ),
  ].sort();
  const files = [];
  for (const path of paths) {
    const full = resolve(root, path);
    let stat;
    try {
      stat = await lstat(full);
    } catch (e) {
      if (e.code === 'ENOENT') continue;
      throw e;
    }
    assert(stat.isFile() && !stat.isSymbolicLink(), `Not a regular source file: ${path}`);
    const data = await readFile(full);
    files.push({ path, bytes: data.length, sha256: digest(data) });
  }
  return {
    schema_version: 1,
    commit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root }).toString().trim(),
    source_sha256: digest(JSON.stringify(files)),
    files,
  };
}
export async function packageFiles(root, path = root) {
  assert(!(await lstat(path)).isSymbolicLink(), 'Package links are forbidden');
  const files = [];
  for (const item of await readdir(path, { withFileTypes: true })) {
    const full = join(path, item.name);
    const key = relative(root, full).split(sep).join('/');
    assert(!item.isSymbolicLink(), `Package link: ${key}`);
    if (item.isDirectory()) files.push(...(await packageFiles(root, full)));
    else {
      assert(item.isFile(), `Not a package file: ${key}`);
      if (key === 'build-manifest.json') continue;
      const data = await readFile(full);
      files.push({ path: key, bytes: data.length, sha256: digest(data) });
    }
  }
  return files.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
}
export function signature(path) {
  const literal = path.replaceAll("'", "''");
  return execFileSync(
    'powershell.exe',
    [
      '-NoProfile',
      '-NonInteractive',
      '-Command',
      `$ErrorActionPreference='Stop'; $env:PSModulePath=$PSHOME+'\\Modules'; (Get-AuthenticodeSignature -LiteralPath '${literal}').Status.ToString()`,
    ],
    { windowsHide: true, encoding: 'utf8' },
  ).trim();
}
export function validateManifest(value) {
  assert.equal(value.schema_version, 1);
  assert(['portable', 'installer'].includes(value.profile));
  assert.deepEqual(
    value.features,
    value.profile === 'installer' ? ['custom-protocol', 'system-cleanup'] : ['custom-protocol'],
  );
  assert.match(value.version, /^\d+\.\d+\.\d+$/);
  assert.match(value.source_sha256, /^[a-f0-9]{64}$/);
  assert.match(value.catalog_id, /^sha256:[a-f0-9]{64}$/);
  assert.match(value.rule_catalog_id, /^sha256:[a-f0-9]{64}$/);
  assert(
    value.files.length > 0 && new Set(value.files.map((f) => f.path)).size === value.files.length,
  );
  for (const f of value.files) {
    assert(
      !isAbsolute(f.path) &&
        !f.path.includes('\\') &&
        !f.path.split('/').some((s) => !s || s === '.' || s === '..'),
      'Invalid manifest path',
    );
    assert.match(f.sha256, /^[a-f0-9]{64}$/);
    assert(Number.isSafeInteger(f.bytes) && f.bytes >= 0);
  }
  assert.deepEqual(
    value.signatures.map((s) => s.path).sort(),
    value.files
      .filter((f) => /\.exe$/i.test(f.path))
      .map((f) => f.path)
      .sort(),
  );
  assert(
    value.signatures.every((s) => s.status === 'NotSigned'),
    'This candidate workflow only permits explicitly unsigned executables',
  );
  assert.equal(value.validation, 'candidate_not_stability_accepted');
}
export async function createManifest(directory, snapshotPath, profile) {
  const before = await readJson(snapshotPath);
  const current = await sourceSnapshot();
  assert.deepEqual(current, before, 'Source changed after snapshot; rebuild from a new snapshot');
  const config = await readJson(join(project, 'package.json'));
  const catalogue = await readJson(join(project, 'scripts/application-catalog-v3.json'));
  const files = await packageFiles(directory);
  const binaries = files.filter((f) => /\.exe$/i.test(f.path));
  assert(binaries.length, 'No executable in candidate package');
  if (profile === 'portable')
    assert(!files.some((f) => /helper/i.test(f.path)), 'Portable package contains helper');
  const manifest = {
    schema_version: 1,
    version: config.version,
    profile,
    features: profile === 'installer' ? ['custom-protocol', 'system-cleanup'] : ['custom-protocol'],
    commit: before.commit,
    source_sha256: before.source_sha256,
    catalog_id: catalogue.catalog_id,
    rule_catalog_id: `sha256:${digest(await readFile(join(project, 'src/preview-rules.json')))}`,
    signatures: binaries.map((f) => ({ path: f.path, status: signature(join(directory, f.path)) })),
    validation: 'candidate_not_stability_accepted',
    files,
  };
  validateManifest(manifest);
  await writeFile(
    join(directory, 'build-manifest.json'),
    JSON.stringify(manifest, null, 2) + '\n',
    { flag: 'wx' },
  );
  return manifest;
}
export async function checkManifest(path, signatureReader = signature) {
  const value = await readJson(path);
  validateManifest(value);
  assert.deepEqual(await packageFiles(dirname(path)), value.files, 'Package content changed');
  for (const s of value.signatures)
    assert.equal(signatureReader(resolve(dirname(path), s.path)), s.status, 'Signature changed');
  return value;
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [command, target, snapshot, profile] = process.argv.slice(2);
  if (command === 'snapshot' && target)
    await writeFile(resolve(target), JSON.stringify(await sourceSnapshot(), null, 2) + '\n', {
      flag: 'wx',
    });
  else if (command === 'create' && target && snapshot && profile)
    await createManifest(resolve(target), resolve(snapshot), profile);
  else if (command === 'check' && target) await checkManifest(resolve(target));
  else
    throw new Error(
      'Usage: snapshot <new-file> | create <package-dir> <source-snapshot> <portable|installer> | check <manifest>',
    );
  console.log(`Release ${command}: verified`);
}
