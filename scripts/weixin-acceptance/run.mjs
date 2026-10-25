import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir, readFile, writeFile, copyFile } from 'node:fs/promises';
import { constants } from 'node:fs';
import assert from 'node:assert/strict';
import { resolve, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkBundle, runId, basename, sha256, validateStatus, binaryNames } from './bundle.mjs';
import { receiveFile } from './receive.mjs';
const exec = promisify(execFile);
const project = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const [operation, inputId, ...inputs] = process.argv.slice(2);
const id = runId(inputId);
if (!['run', 'resume', 'status', 'cancel'].includes(operation))
  throw Error('Use run|resume|status|cancel RUN_ID [windows-tests core-tests probe]');
const folder = join(project, 'artifacts/weixin-0211', id);
const home = process.env.USERPROFILE.replaceAll('\\', '/');
const hosts = `${home}/.ssh/dustaway-desktop-24njtt6-known-hosts`;
const key = `${home}/.ssh/dustaway-desktop-24njtt6-ed25519`;
const git = 'C:/Program Files/Git/usr/bin';
const host = 'MSN@100.69.113.49';
const remote = `${host}:C:/Users/MSN/AppData/Local/DustAwayAcceptance/weixin-0211/${id}/`;
const options = [
  '-F',
  '/dev/null',
  '-i',
  key,
  '-o',
  'IdentitiesOnly=yes',
  '-o',
  'BatchMode=yes',
  '-o',
  'ConnectTimeout=8',
  '-o',
  'StrictHostKeyChecking=yes',
  '-o',
  `UserKnownHostsFile=${hosts}`,
  '-o',
  'GlobalKnownHostsFile=/dev/null',
  '-o',
  'HostKeyAlgorithms=ssh-ed25519',
];
const posix = (path) =>
  resolve(path)
    .replaceAll('\\', '/')
    .replace(/^([A-Za-z]):\//, (_, drive) => `/${drive.toLowerCase()}/`);
async function remoteAction(action) {
  const quote = (s) => "'" + s.replaceAll("'", "''") + "'";
  const invocation = `& ([ScriptBlock]::Create([IO.File]::ReadAllText(${quote(join(project, 'scripts/weixin-acceptance/Invoke-Remote.ps1'))}))) -ScriptFile ${quote(join(project, 'scripts/weixin-acceptance/Remote.ps1'))} -ParametersBase64 '${Buffer.from(JSON.stringify({ Action: action, RunId: id })).toString('base64')}'`;
  const { stdout } = await exec(
    'powershell.exe',
    [
      '-NoProfile',
      '-NonInteractive',
      '-EncodedCommand',
      Buffer.from(invocation, 'utf16le').toString('base64'),
    ],
    { windowsHide: true, timeout: 60000, maxBuffer: 4 * 1024 * 1024 },
  );
  return JSON.parse(stdout.replace(/^\uFEFF/, ''));
}
async function collect() {
  const manifest = await remoteAction('Collect');
  if (manifest.schema !== 2 || manifest.runId !== id) throw Error('Wrong remote manifest');
  await mkdir(folder, { recursive: true });
  try {
    assert.deepEqual(
      JSON.parse(await readFile(join(folder, 'manifest.json'), 'utf8')),
      manifest,
      'Remote manifest changed',
    );
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
    await writeFile(join(folder, 'manifest.json'), JSON.stringify(manifest, null, 2), {
      flag: 'wx',
    });
  }
  for (const file of manifest.files) {
    basename(file.name);
    const target = join(folder, file.name);
    await receiveFile(target, file, async (partial) => {
      await exec(`${git}/scp.exe`, [...options, remote + file.name, posix(partial)], {
        windowsHide: true,
        timeout: 60000,
      });
    });
  }
  const result = await checkBundle(folder);
  console.log(JSON.stringify(result, null, 2));
  await remoteAction('Finish');
  if (!result.noninteractivePassed) process.exitCode = 2;
}
try {
  const fingerprint = await exec(`${git}/ssh-keygen.exe`, ['-lf', hosts], { windowsHide: true });
  if (!fingerprint.stdout.includes('SHA256:qTe49uQ4Pc/szc7HFgm8ICcDMDO8laK/zLVPLhLsYIE'))
    throw Error('Host key fingerprint mismatch');
  if (operation === 'cancel') {
    console.log(JSON.stringify(await remoteAction('Cancel')));
  } else if (operation === 'status') {
    console.log(JSON.stringify(await remoteAction('Status'), null, 2));
  } else {
    if (operation === 'run') {
      if (inputs.length !== 3) throw Error('Three test binary paths required');
      await mkdir(folder, { recursive: false });
      for (let i = 0; i < inputs.length; i++)
        await copyFile(resolve(inputs[i]), join(folder, binaryNames[i]), constants.COPYFILE_EXCL);
      await copyFile(
        join(project, 'scripts/weixin-acceptance/Worker.ps1'),
        join(folder, 'worker.ps1'),
        constants.COPYFILE_EXCL,
      );
      const hashes = {};
      for (const name of [...binaryNames, 'worker.ps1'])
        hashes[name] = sha256(await readFile(join(folder, name)));
      await writeFile(
        join(folder, 'request.json'),
        JSON.stringify({ schema: 2, runId: id, version: '0.21.1', hashes }, null, 2),
        { flag: 'wx' },
      );
      await remoteAction('Prepare');
      await exec(
        `${git}/scp.exe`,
        [
          ...options,
          ...[...binaryNames, 'worker.ps1', 'request.json'].map((name) =>
            posix(join(folder, name)),
          ),
          remote,
        ],
        { windowsHide: true, timeout: 120000 },
      );
      await remoteAction('Start');
    }
    let last = '',
      ended = false;
    const until = Date.now() + 43 * 60 * 1000;
    while (Date.now() < until) {
      const status = validateStatus(await remoteAction('Status'), id);
      const progress = `${status.status}:${status.stages.length}`;
      if (progress !== last) {
        console.log(`${id}: ${progress}`);
        last = progress;
      }
      if (status.status !== 'running') {
        ended = true;
        break;
      }
      await new Promise((r) => setTimeout(r, 2000));
    }
    if (!ended) throw Error('Run still active; query status, do not restart');
    await collect();
  }
} catch (error) {
  console.error(
    `Run not confirmed: ${error.message.split('\n')[0]}. Resume uses the existing run; it never dispatches again.`,
  );
  process.exitCode = 1;
}
