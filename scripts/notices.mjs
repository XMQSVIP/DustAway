// Inventory installed, locked dependencies, including build/test tools; do not fetch source.
import { execFileSync } from 'node:child_process';
import { readdir, readFile, writeFile, mkdir, lstat } from 'node:fs/promises';
import { dirname, join } from 'node:path';

const packages = [];
const reviews = JSON.parse(await readFile('licenses/review.json', 'utf8'));
async function licenseText(directory) {
  const names = (await readdir(directory)).filter((n) =>
    /^(licen[cs]e|copying|copyright|notice|unlicen[cs]e)(\b|[-_.])/i.test(n),
  );
  const texts = [];
  for (const name of names.sort()) {
    const file = join(directory, name);
    if ((await lstat(file)).isFile()) texts.push(`${name}\n${await readFile(file, 'utf8')}`);
  }
  return texts.join('\n\n');
}
const metadata = JSON.parse(
  execFileSync(
    'cargo',
    [
      'metadata',
      '--locked',
      '--format-version',
      '1',
      '--filter-platform',
      'x86_64-pc-windows-msvc',
    ],
    { maxBuffer: 32 * 1024 * 1024, encoding: 'utf8' },
  ),
);
for (const p of metadata.packages.filter((p) => p.source)) {
  const directory = dirname(p.manifest_path);
  let text = await licenseText(directory);
  if (p.license_file && !text) text = await readFile(join(directory, p.license_file), 'utf8');
  if (!text) {
    try {
      text = await readFile(`licenses/overrides/cargo-${p.name}-${p.version}.txt`, 'utf8');
    } catch {}
  }
  packages.push({
    ecosystem: 'cargo',
    name: p.name,
    version: p.version,
    license: p.license ?? 'see license file',
    source: p.repository ?? `https://crates.io/crates/${p.name}/${p.version}`,
    text,
  });
}
async function npmPackage(directory) {
  if ((await lstat(directory)).isSymbolicLink()) return;
  let p;
  try {
    p = JSON.parse(await readFile(join(directory, 'package.json'), 'utf8'));
  } catch {
    return;
  }
  if (
    !p.name ||
    !p.version ||
    packages.some((v) => v.ecosystem === 'npm' && v.name === p.name && v.version === p.version)
  )
    return;
  let text = await licenseText(directory);
  if (!text) {
    try {
      text = await readFile(
        `licenses/overrides/npm-${p.name.replaceAll('/', '__')}-${p.version}.txt`,
        'utf8',
      );
    } catch {}
  }
  packages.push({
    ecosystem: 'npm',
    name: p.name,
    version: p.version,
    license:
      typeof p.license === 'string'
        ? p.license
        : JSON.stringify(p.license ?? p.licenses ?? 'UNSPECIFIED'),
    source: `https://www.npmjs.com/package/${p.name}/v/${p.version}`,
    text,
  });
}
for (const entry of await readdir('node_modules/.pnpm', { withFileTypes: true })) {
  if (!entry.isDirectory() || entry.name === 'node_modules') continue;
  const directory = join('node_modules/.pnpm', entry.name, 'node_modules');
  for (const p of await readdir(directory, { withFileTypes: true })) {
    if (p.name.startsWith('@') && p.isDirectory()) {
      for (const sub of await readdir(join(directory, p.name)))
        await npmPackage(join(directory, p.name, sub));
    } else if (p.isDirectory()) await npmPackage(join(directory, p.name));
  }
}
// Package archives and local overrides can use different line endings.
// Emit canonical LF while preserving the license wording.
for (const p of packages) p.text = p.text.replace(/\r\n/g, '\n');
packages.sort((a, b) =>
  `${a.ecosystem}/${a.name}/${a.version}`.localeCompare(`${b.ecosystem}/${b.name}/${b.version}`),
);
await mkdir('licenses', { recursive: true });
await writeFile(
  'licenses/inventory.json',
  JSON.stringify(
    packages.map(({ text, ...p }) => ({
      ...p,
      licenseTextIncluded: !!text,
      ...(!text && reviews[`${p.ecosystem}/${p.name}/${p.version}`]
        ? { review: reviews[`${p.ecosystem}/${p.name}/${p.version}`] }
        : {}),
    })),
    null,
    2,
  ) + '\n',
);
const header =
  '拂尘 (DustAway) third-party notices\nGenerated from Cargo.lock, pnpm-lock.yaml and installed package sources.\nIncludes development/test and cross-platform dependencies; inclusion does not imply shipment.\nOriginal license texts follow with line endings normalized to LF. Packages without packaged license text and documented development-only exceptions are listed in inventory.json.\nWebView2 is a separately installed Microsoft runtime: https://developer.microsoft.com/microsoft-edge/webview2/\nNSIS components (experimental installer only): https://nsis.sourceforge.io/License\n\n';
await writeFile(
  'licenses/THIRD-PARTY-NOTICES.txt',
  header +
    packages
      .map(
        (p) =>
          `${'='.repeat(72)}\n${p.ecosystem}: ${p.name} ${p.version}\nLicense: ${p.license}\nSource: ${p.source}\n\n${p.text || reviews[`${p.ecosystem}/${p.name}/${p.version}`]?.reason || '(No top-level license text shipped in the installed package; see source.)'}\n`,
      )
      .join('\n'),
);
console.log(
  `Generated inventory: ${packages.length} packages; ${packages.filter((p) => !p.text && !reviews[`${p.ecosystem}/${p.name}/${p.version}`]).length} need license-text review; ${packages.filter((p) => !p.text && reviews[`${p.ecosystem}/${p.name}/${p.version}`]).length} documented development-only exceptions.`,
);
