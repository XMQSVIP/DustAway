import { mkdir, writeFile, readFile, readdir, utimes } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import assert from 'node:assert/strict';

// Development-only fixtures. Never point an official operation at a user's cache.
const root = resolve('artifacts/rule-refresh-20261002/tool-fixtures');
await mkdir(root);
await writeFile(
  join(root, 'package.json'),
  JSON.stringify({ private: true, packageManager: 'yarn@1.22.22' }),
  { flag: 'wx' },
); // Refuse reuse: interrupted operations must never be replayed.
const node = process.execPath;
const pnpm = 'C:/Users/Administrator/AppData/Roaming/npm/node_modules/pnpm/bin/pnpm.cjs';
const go = 'D:/Go/bin/go.exe';
const ccache = 'C:/mingw64/bin/ccache.exe';
const gcc = 'C:/mingw64/bin/gcc.exe';
const env = {
  ...process.env,
  NODE_OPTIONS: '',
  NODE_PATH: '',
  COREPACK_ENABLE_NETWORK: '0',
  npm_config_manage_package_manager_versions: 'false',
  npm_config_ignore_pnpmfile: 'true',
  GOTOOLCHAIN: 'local',
  GOENV: 'off',
  GOWORK: 'off',
  GOPROXY: 'off',
  GOSUMDB: 'off',
  GOCACHEPROG: '',
};
const transcript = [];
function run(exe, args, cwd, extra = {}) {
  const r = spawnSync(exe, args, {
    cwd,
    env: { ...env, ...extra },
    windowsHide: true,
    encoding: 'utf8',
    timeout: 180000,
  });
  transcript.push({ exe, args, status: r.status, stdout: r.stdout, stderr: r.stderr });
  assert.equal(r.status, 0, `${exe}: ${r.error ?? r.stderr}`);
  return r.stdout.trim();
}
const manifest = [];
try {
  const yarn = resolve(
    'artifacts/cleanup-closure-20261002/tool-fixtures-v3/yarn-tool/package/bin/yarn.js',
  );
  const metadata = JSON.parse(await readFile(resolve(yarn, '../../package.json'), 'utf8'));
  assert.equal(metadata.version, '1.22.22');
  const pkg = join(root, 'package');
  await mkdir(pkg);
  await writeFile(
    join(pkg, 'package.json'),
    JSON.stringify({ name: 'dustaway-isolated-fixture', version: '1.0.0', main: 'index.js' }),
  );
  await writeFile(join(pkg, 'index.js'), 'module.exports = 42;\n');
  const archive = join(root, 'fixture.tgz');
  run('tar', ['-czf', archive, 'package'], root);
  for (const kind of ['yarn']) {
    const project = join(root, kind + '-project');
    await mkdir(project);
    const store = join(root, kind + '-cache');
    await mkdir(store);
    await writeFile(
      join(project, 'package.json'),
      JSON.stringify({
        name: 'fixture-consumer',
        version: '1.0.0',
        dependencies: { 'dustaway-isolated-fixture': 'file:' + archive.replaceAll('\\', '/') },
      }),
    );
    const entry = kind === 'pnpm' ? pnpm : yarn;
    const args =
      kind === 'pnpm'
        ? [
            'install',
            '--offline',
            '--ignore-scripts',
            '--store-dir',
            store,
            '--package-import-method',
            'hardlink',
          ]
        : [
            'install',
            '--offline',
            '--ignore-scripts',
            '--non-interactive',
            '--cache-folder',
            store,
          ];
    run(node, [entry, ...args], project, { YARN_IGNORE_PATH: '1' });
    const target = run(
      node,
      [
        entry,
        ...(kind === 'pnpm'
          ? ['store', 'path', '--store-dir', store]
          : ['--cache-folder', store, 'cache', 'dir']),
      ],
      project,
      { YARN_IGNORE_PATH: '1' },
    );
    const protectedPath = join(project, 'node_modules/dustaway-isolated-fixture/index.js');
    manifest.push({ kind, exe: node, entry, root: target, protected: [protectedPath], project });
    // These markers stand for project state and must not be touched by global-cache operations.
    await mkdir(join(project, '.yarn'));
    await writeFile(join(project, '.pnp.cjs'), 'protected project configuration');
    manifest.at(-1).protected.push(join(project, '.pnp.cjs'));
  }
  const gp = join(root, 'go-project');
  const gc = join(root, 'go-cache');
  await mkdir(gp);
  await mkdir(gc);
  await writeFile(join(gp, 'main.go'), 'package main\nfunc main() {}\n');
  run(go, ['build', '-o', join(gp, 'fixture.exe'), join(gp, 'main.go')], gp, {
    GOCACHE: gc,
    GOPATH: join(root, 'gopath'),
    GOMODCACHE: join(root, 'gomod'),
  });
  await mkdir(join(gc, 'fuzz'), { recursive: true });
  const fuzz = join(gc, 'fuzz', 'keep');
  await writeFile(fuzz, 'protected fuzz corpus');
  await mkdir(join(root, 'gomod'), { recursive: true });
  const mod = join(root, 'gomod', 'keep');
  await writeFile(mod, 'protected module');
  manifest.push({
    kind: 'go',
    exe: go,
    entry: null,
    root: gc,
    protected: [fuzz, mod, join(gp, 'main.go'), join(gp, 'fixture.exe')],
    project: gp,
  });
  const cc = join(root, 'ccache-cache');
  await mkdir(cc);
  await writeFile(join(cc, 'ccache.conf'), 'max_size = 100M\n');
  const source = join(root, 'fixture.c');
  await writeFile(source, 'int fixture(void) { return 42; }\n');
  run(ccache, [gcc, '-c', source, '-o', join(root, 'old.o')], root, {
    CCACHE_DIR: cc,
    CCACHE_CONFIGPATH: join(cc, 'ccache.conf'),
    CCACHE_NOSTATS: '1',
  });
  async function files(dir) {
    const a = [];
    for (const e of await readdir(dir, { withFileTypes: true })) {
      const p = join(dir, e.name);
      if (e.isDirectory()) a.push(...(await files(p)));
      else a.push(p);
    }
    return a;
  }
  const old = (await files(cc)).filter((p) => /[MR]$/.test(p));
  assert.ok(old.length > 0);
  for (const p of old)
    await utimes(p, new Date(Date.now() - 31 * 86400000), new Date(Date.now() - 31 * 86400000));
  await writeFile(source, 'int fixture(void) { return 99; }\n');
  run(ccache, [gcc, '-c', source, '-o', join(root, 'recent.o')], root, {
    CCACHE_DIR: cc,
    CCACHE_CONFIGPATH: join(cc, 'ccache.conf'),
    CCACHE_NOSTATS: '1',
  });
  const recent = (await files(cc)).filter((p) => /[MR]$/.test(p) && !old.includes(p));
  assert.ok(recent.length > 0);
  manifest.push({
    kind: 'ccache',
    exe: ccache,
    entry: null,
    root: cc,
    protected: [join(cc, 'ccache.conf'), ...recent],
    removed: old,
  });
  for (const row of manifest)
    row.protected = await Promise.all(
      row.protected.map(async (path) => ({
        path,
        sha256: createHash('sha256')
          .update(await readFile(path))
          .digest('hex'),
      })),
    );
  await writeFile(join(root, 'manifest.json'), JSON.stringify(manifest, null, 2), { flag: 'wx' });
} finally {
  await writeFile(join(root, 'prepare-log.json'), JSON.stringify(transcript, null, 2), {
    flag: 'wx',
  });
}
