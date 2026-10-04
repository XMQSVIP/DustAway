// One-time refresh for packages whose archives omit license texts. Review the resulting diff.
// Each saved file records its upstream commit URL; normal builds only use the saved files.
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
const exec = promisify(execFile);
const cache = new Map();
async function download(url) {
  if (!cache.has(url))
    cache.set(
      url,
      exec(
        'curl.exe',
        [
          '-sS',
          '-L',
          '--fail',
          '--max-time',
          '30',
          ...(process.env.DUSTAWAY_BUILD_PROXY
            ? ['--proxy', process.env.DUSTAWAY_BUILD_PROXY]
            : []),
          url,
        ],
        { maxBuffer: 8 * 1024 * 1024, windowsHide: true },
      ).then((r) => r.stdout),
    );
  return cache.get(url);
}
const inventory = JSON.parse(await readFile('licenses/inventory.json', 'utf8'));
const metadata = JSON.parse(await readFile('artifacts/cargo-metadata.json', 'utf8'));
await mkdir('licenses/overrides', { recursive: true });
for (const p of inventory.filter((p) => !p.licenseTextIncluded)) {
  try {
    let repository, sha;
    if (p.ecosystem === 'cargo') {
      const pack = metadata.packages.find((v) => v.name === p.name && v.version === p.version);
      repository = pack.repository;
      sha = JSON.parse(
        await readFile(join(dirname(pack.manifest_path), '.cargo_vcs_info.json'), 'utf8'),
      ).git.sha1;
    } else {
      const parent =
        p.name === '@tauri-apps/cli-win32-x64-msvc'
          ? '@tauri-apps/cli'
          : p.name === '@rolldown/binding-win32-x64-msvc'
            ? 'rolldown'
            : p.name;
      const pack = JSON.parse(
        await download(`https://registry.npmjs.org/${encodeURIComponent(parent)}/${p.version}`),
      );
      repository = typeof pack.repository === 'string' ? pack.repository : pack.repository?.url;
      sha = pack.gitHead;
    }
    const repo = repository?.match(/github\.com[/:]([^/]+\/[^/#]+)/)?.[1].replace(/\.git$/, '');
    if (!repo || !sha) throw new Error('No pinned GitHub provenance');
    const files = JSON.parse(
      await download(`https://api.github.com/repos/${repo}/contents?ref=${sha}`),
    );
    const selected = files.filter(
      (f) =>
        f.type === 'file' && /^(license|licence|copying|copyright|notice)(\b|[-_.])/i.test(f.name),
    );
    if (!selected.length) throw new Error('No root license found');
    const texts = [];
    for (const f of selected) {
      const url = `https://raw.githubusercontent.com/${repo}/${sha}/${f.name}`;
      texts.push(`Source: ${url}\n\n${await download(url)}`);
    }
    await writeFile(
      `licenses/overrides/${p.ecosystem}-${p.name.replaceAll('/', '__')}-${p.version}.txt`,
      texts.join('\n\n'),
    );
    console.log(`Resolved ${p.name} ${p.version}`);
  } catch (e) {
    console.error(`Needs manual review: ${p.name} (${e.message.slice(0, 160)})`);
    process.exitCode = 1;
  }
}
