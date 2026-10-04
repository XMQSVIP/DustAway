// Generate real Firefox bookmark/session backups in the existing marked test
// profile. Mozilla WebDriver and Firefox's own Places/SessionSaver write the
// files; this script does not synthesize browser database or backup formats.
import assert from 'node:assert/strict';
import { readFile, writeFile, readdir, stat, access } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';

const root = resolve(process.cwd());
assert.equal(process.env.COMPUTERNAME, 'DESKTOP-24NJTT6');
assert.equal(root, join(process.env.LOCALAPPDATA, 'DustAwayAcceptance', '20260930-software'));
assert.equal(
  await readFile(join(root, '.dustaway-isolated-acceptance'), 'utf8'),
  'DustAway.Acceptance.20260930.7c7e25e8',
);
const prefix = process.argv[2];
assert(
  [
    'firefox-history-evidence',
    'firefox-history-evidence-final',
    'firefox-history-production',
  ].includes(prefix),
);
const resumeExisting = process.argv[3] === 'resume-existing-browser';
assert(process.argv[3] === undefined || resumeExisting);
assert(!resumeExisting || prefix === 'firefox-history-evidence');
const json = async (p) => JSON.parse((await readFile(p, 'utf8')).replace(/^\uFEFF/, ''));
const ready = await json(join(root, 'firefox-real-ready.json'));
assert(/^[a-zA-Z0-9_-]+\.DustAwayAcceptance20260930$/.test(ready.ProfileDirectory));
const profile = join(process.env.APPDATA, 'Mozilla', 'Firefox', 'Profiles', ready.ProfileDirectory);
assert.equal(
  await readFile(join(profile, '.dustaway-test-only'), 'utf8'),
  'DustAwayFirefox20260930',
);
if (resumeExisting) {
  const prior = await json(join(root, `${prefix}-seed-status.json`));
  assert.equal(prior.session, '4226e80a-7186-4da5-b816-a2defe12b306');
  assert.equal(prior.cycle, 0);
  await writeFile(join(root, `${prefix}-first-seed-status.json`), JSON.stringify(prior, null, 2), {
    flag: 'wx',
  });
}
await writeFile(
  join(root, `${prefix}-seed${resumeExisting ? '-continued' : ''}.started`),
  new Date().toISOString(),
  { flag: 'wx' },
);
const save = (name, data) =>
  writeFile(join(root, `${prefix}-${name}.json`), JSON.stringify(data, null, 2));
const exists = (p) =>
  access(p).then(
    () => true,
    () => false,
  );
const userFile = join(profile, 'user.js');
const oldUserJs = (await exists(userFile)) ? await readFile(userFile) : null;
const marker = 'DustAwayBookmarkSession20261001';
const url = (part) => `http://127.0.0.1:19838/${marker}/${part}`;
const server = createServer((request, response) => {
  if (!request.url.startsWith(`/${marker}/`)) {
    response.writeHead(404);
    response.end();
    return;
  }
  response.writeHead(200, {
    'Content-Type': 'text/html; charset=utf-8',
    'Cache-Control': 'no-store',
  });
  response.end(
    `<!doctype html><title>${marker}</title><h1>${marker}</h1><p>Dedicated local bookmark and session data.</p>`,
  );
});
await new Promise((ok, fail) => {
  server.once('error', fail);
  server.listen(19838, '127.0.0.1', ok);
});
const startDriver = (connectExisting) =>
  spawn(
    join(root, 'geckodriver.exe'),
    [
      '--host',
      '127.0.0.1',
      '--port',
      '4446',
      '--marionette-port',
      '2829',
      '--allow-system-access',
      ...(connectExisting ? ['--connect-existing'] : []),
    ],
    { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] },
  );
let logs = '';
function recordLogs(child) {
  child.stdout.on('data', (chunk) => {
    logs += chunk;
  });
  child.stderr.on('data', (chunk) => {
    logs += chunk;
  });
}
let driver = startDriver(resumeExisting);
recordLogs(driver);
let session;
let closed = true;
const observations = [];
async function command(method, path, data) {
  const response = await fetch(`http://127.0.0.1:4446${path}`, {
    method,
    headers: { 'content-type': 'application/json' },
    body: data === undefined ? undefined : JSON.stringify(data),
    signal: AbortSignal.timeout(90000),
  });
  const body = await response.json();
  if (!response.ok || body.value?.error) throw Error(JSON.stringify(body));
  return body.value;
}
async function until(fn, ms = 45000) {
  const deadline = Date.now() + ms;
  while (Date.now() < deadline) {
    const value = await fn();
    if (value) return value;
    await new Promise((done) => setTimeout(done, 200));
  }
  throw Error('Observation expired; inspect original browser process, do not replay');
}
async function chrome(script, args = []) {
  await command('POST', `/session/${session}/moz/context`, { context: 'chrome' });
  const result = await command('POST', `/session/${session}/execute/async`, {
    script: `const complete=arguments[arguments.length-1];(async()=>{${script}})().then(value=>complete({ok:true,value}),error=>complete({ok:false,error:String(error)}));`,
    args,
  });
  assert(result.ok, result.error);
  return result.value;
}
try {
  await until(() =>
    fetch('http://127.0.0.1:4446/status')
      .then((r) => r.ok)
      .catch(() => false),
  );
  for (let cycle = 0; cycle < 2; cycle++) {
    if (cycle === 1 && resumeExisting) {
      const driverExit = new Promise((done) => driver.once('exit', done));
      driver.kill(); // Own driver after graceful browser exit, not the browser.
      await driverExit;
      driver = startDriver(false);
      recordLogs(driver);
      await until(() =>
        fetch('http://127.0.0.1:4446/status')
          .then((r) => r.ok)
          .catch(() => false),
      );
    }
    const created = await command('POST', '/session', {
      capabilities: {
        alwaysMatch: {
          browserName: 'firefox',
          'moz:firefoxOptions': {
            binary: join(root, 'firefox-real', 'application', 'firefox.exe'),
            args: ['--headless', '--no-remote', '--profile', profile],
            prefs: {
              'browser.startup.page': 3,
              'browser.startup.homepage': 'about:blank',
              'browser.startup.homepage_override.mstone': 'ignore',
              'browser.shell.checkDefaultBrowser': false,
            },
          },
        },
      },
    });
    session = created.sessionId;
    closed = false;
    assert.equal(resolve(created.capabilities['moz:profile']), profile);
    assert.equal(created.capabilities.browserVersion, '140.17.0');
    await save('seed-status', { phase: 'browser_running', cycle, session, driverPid: driver.pid });
    await command('POST', `/session/${session}/timeouts`, {
      script: 60000,
      pageLoad: 20000,
      implicit: 0,
    });
    await command('POST', `/session/${session}/url`, { url: url('alpha') });
    const second = await command('POST', `/session/${session}/window/new`, { type: 'tab' });
    await command('POST', `/session/${session}/window`, { handle: second.handle });
    await command('POST', `/session/${session}/url`, { url: url('beta') });
    const result = await chrome(
      `
      if(PathUtils.profileDir !== arguments[0]) throw Error('Unexpected active profile');
      const {PlacesUtils}=ChromeUtils.importESModule('resource://gre/modules/PlacesUtils.sys.mjs');
      const {PlacesBackups}=ChromeUtils.importESModule('resource://gre/modules/PlacesBackups.sys.mjs');
      const {SessionSaver}=ChromeUtils.importESModule('resource:///modules/sessionstore/SessionSaver.sys.mjs');
      const {SessionFile}=ChromeUtils.importESModule('resource:///modules/sessionstore/SessionFile.sys.mjs');
      const {TabStateFlusher}=ChromeUtils.importESModule('resource:///modules/sessionstore/TabStateFlusher.sys.mjs');
      const found=await PlacesUtils.bookmarks.fetch({url:arguments[1]});
      if(found && found.title!==arguments[2]) throw Error('Unexpected bookmark');
      if(!found) await PlacesUtils.bookmarks.insert({parentGuid:PlacesUtils.bookmarks.toolbarGuid,url:arguments[1],title:arguments[2]});
      await PlacesBackups.create(undefined,true);
      const backup=await PlacesBackups.getMostRecentBackup();
      const bookmarkData=await IOUtils.readJSON(backup,{decompress:true});
      if(!JSON.stringify(bookmarkData).includes(arguments[2])) throw Error('Browser backup lacks test bookmark');
      await TabStateFlusher.flushWindow(Services.wm.getMostRecentWindow('navigator:browser'));
      await SessionSaver.run();
      const recovery=await IOUtils.readJSON(SessionFile.Paths.recovery,{decompress:true});
      const content=JSON.stringify(recovery);
      if(!content.includes(arguments[1]) || !content.includes(arguments[3])) throw Error('Browser session lacks test tabs');
      let previousVerified=false;
      if(arguments[4]===1){
        const previous=JSON.stringify(await IOUtils.readJSON(SessionFile.Paths.cleanBackup,{decompress:true}));
        if(!previous.includes(arguments[1]) || !previous.includes(arguments[3])) throw Error('Prior session was not retained by Firefox');
        previousVerified=true;
      }
      return {backupName:PathUtils.filename(backup),recoveryName:PathUtils.filename(SessionFile.Paths.recovery),bookmarkVerified:true,testTabsVerified:true,previousVerified};
    `,
      [profile, url('alpha'), marker, url('beta'), cycle],
    );
    observations.push(result);
    await command('DELETE', `/session/${session}`);
    session = undefined;
    closed = true;
  }
  const files = [];
  for (const folder of ['bookmarkbackups', 'sessionstore-backups']) {
    const selected = [];
    for (const name of await readdir(join(profile, folder))) {
      if (!name.endsWith('.jsonlz4')) continue;
      const bytes = await readFile(join(profile, folder, name));
      assert(bytes.length > 8 && bytes.subarray(0, 8).equals(Buffer.from('mozLz40\0')));
      selected.push({
        name,
        bytes: bytes.length,
        sha256: createHash('sha256').update(bytes).digest('hex'),
      });
    }
    assert(selected.length > 0, `${folder} has no browser-generated backup`);
    if (folder === 'sessionstore-backups')
      assert(selected.some((file) => file.name === 'previous.jsonlz4'));
    files.push({ folder, files: selected });
  }
  // Only this marked profile's test preferences may have been changed by the driver.
  if (oldUserJs) await writeFile(userFile, oldUserJs);
  await save('seed-verification', {
    passed: true,
    profileMarkerVerified: true,
    officialDriverVersion: '0.37.1',
    browserVersion: '140.17.0',
    vendorGeneratedBackups: true,
    browserBackupDataDecodedAndVerified: true,
    twoGracefulBrowserExits: true,
    loopbackOnlyTestPages: true,
    observations,
    files,
    existingTestPreferencesRestored: !!oldUserJs,
    driverConfigurationLimitedToMarkedProfile: true,
    resumedConfirmedOriginalBrowser: resumeExisting,
  });
  await save('seed-status', { phase: 'passed' });
} catch (error) {
  await writeFile(join(root, `${prefix}-seed-failure.txt`), String(error.stack ?? error));
  await save('seed-status', {
    phase: 'failed',
    session,
    driverPid: driver.pid,
    keptRunning: !closed,
  });
  if (!closed) {
    // Preserve the confirmed live handle on timeout; no restart or forced exit.
    await new Promise((done) => {
      const timer = setInterval(async () => {
        if (await exists(join(root, `${prefix}-seed-inspection-complete`))) {
          clearInterval(timer);
          closed = true;
          done();
        }
      }, 1000);
    });
  }
  throw error;
} finally {
  await writeFile(join(root, `${prefix}-seed-driver.log`), logs);
  if (closed) {
    driver.kill();
    await new Promise((done) => server.close(done));
  }
}
