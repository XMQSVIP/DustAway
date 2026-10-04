// A loopback-only functional smoke test using the existing Chrome installation.
// It starts only when Chrome is closed and requests normal window closure.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { execFile, spawn } from 'node:child_process';
import { promisify } from 'node:util';
import { readFile, writeFile, readdir, lstat } from 'node:fs/promises';
import { join } from 'node:path';
const exec = promisify(execFile);
assert.equal(process.env.COMPUTERNAME, 'DESKTOP-24NJTT6');
assert.equal(await readFile('.dustaway-completion', 'utf8'), 'DustAway.CleanupCompletion.20261001');
const ps = async (source) =>
  (
    await exec('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', source], {
      windowsHide: true,
    })
  ).stdout.trim();
assert.equal(
  Number(await ps('@(Get-Process -Name chrome -ErrorAction SilentlyContinue).Count')),
  0,
);
const installed = JSON.parse(
  await ps(
    `$p='C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe'; if(!(Test-Path -LiteralPath $p)){$p='C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe'}; $s=Get-AuthenticodeSignature -LiteralPath $p; if($s.Status -ne 'Valid' -or $s.SignerCertificate.Subject -notmatch 'O=Google LLC'){throw 'Unverified Chrome'}; @{path=$p;version=(Get-Item -LiteralPath $p).VersionInfo.FileVersion;sha256=(Get-FileHash -LiteralPath $p).Hash} | ConvertTo-Json -Compress`,
  ),
);
const hits = [],
  ready = [];
const server = createServer((request, response) => {
  hits.push(request.url);
  response.setHeader('Cache-Control', 'public, max-age=3600');
  if (request.url.startsWith('/ready')) {
    ready.push(request.url);
    response.end('ok');
  } else if (request.url === '/app.js') {
    response.setHeader('Content-Type', 'application/javascript');
    response.end(
      `const values=[${Array.from({ length: 20000 }, (_, i) => i).join(',')}]; document.getElementById('result').textContent='Chrome 缓存清理后功能检查通过：'+values.reduce((a,b)=>a+b,0); fetch('/ready'+location.search); if(location.search==='?first') setTimeout(()=>location.href='/?second',2000);`,
    );
  } else {
    response.setHeader('Content-Type', 'text/html; charset=utf-8');
    response.end(
      '<!doctype html><meta charset="utf-8"><title>DustAway Chrome 功能检查</title><h1 id="result">正在检查…</h1><script src="/app.js"></script>',
    );
  }
});
await new Promise((done) => server.listen(0, '127.0.0.1', done));
let functional = false;
try {
  const browser = spawn(
    installed.path,
    ['--new-window', `http://127.0.0.1:${server.address().port}/?first`],
    { windowsHide: false, stdio: 'ignore' },
  );
  await new Promise((done, reject) => {
    browser.once('spawn', done);
    browser.once('error', reject);
  });
  browser.unref();
  const deadline = Date.now() + 60000;
  while (Date.now() < deadline && ready.length < 2)
    await new Promise((done) => setTimeout(done, 500));
  functional = ready.includes('/ready?first') && ready.includes('/ready?second');
} finally {
  // Every Chrome window belongs to this launch: preflight required no Chrome process.
  await ps(
    'Get-Process -Name chrome -ErrorAction SilentlyContinue | Where-Object {$_.MainWindowHandle -ne 0} | ForEach-Object {[void]$_.CloseMainWindow()}',
  );
  server.closeAllConnections();
  server.close();
}
const deadline = Date.now() + 30000;
let processes;
do {
  processes = Number(await ps('@(Get-Process -Name chrome -ErrorAction SilentlyContinue).Count'));
  if (!processes) break;
  await new Promise((done) => setTimeout(done, 500));
} while (Date.now() < deadline);
async function measure(path) {
  let files = 0,
    bytes = 0;
  try {
    for (const entry of await readdir(path, { withFileTypes: true })) {
      const child = join(path, entry.name),
        stat = await lstat(child);
      assert(!stat.isSymbolicLink());
      if (stat.isDirectory()) {
        const nested = await measure(child);
        files += nested.files;
        bytes += nested.bytes;
      } else {
        files++;
        bytes += stat.size;
      }
    }
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  return { files, bytes };
}
const caches = {};
for (const leaf of ['Cache/Cache_Data', 'Code Cache', 'GPUCache'])
  caches[leaf] = await measure(
    join(process.env.LOCALAPPDATA, 'Google/Chrome/User Data/Default', leaf),
  );
await writeFile(
  join(process.env.DUSTAWAY_TEST_OUTPUT, 'chrome-reopen.json'),
  JSON.stringify(
    {
      installed,
      functional,
      hits,
      ready,
      remaining_processes: processes,
      caches,
      forced_termination: false,
    },
    null,
    2,
  ),
);
assert(functional, 'Chrome did not execute both local functional checks');
assert.equal(processes, 0, 'Chrome background process remains; no forced termination');
