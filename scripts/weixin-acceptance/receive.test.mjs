import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { receiveFile } from './receive.mjs';
import { sha256 } from './bundle.mjs';

test('interrupted evidence download resumes without publishing or overwriting partial bytes', async () => {
  const root = await mkdtemp(join(tmpdir(), 'dustaway-transfer-'));
  const target = join(root, 'code.log');
  const data = Buffer.from('verified evidence');
  const expected = { bytes: data.length, sha256: sha256(data) };
  try {
    await assert.rejects(
      receiveFile(target, expected, async (path) => {
        await writeFile(path, 'partial');
        throw Error('disconnected');
      }),
      /disconnected/,
    );
    await assert.rejects(readFile(target), { code: 'ENOENT' });
    await receiveFile(target, expected, (path) => writeFile(path, data));
    assert.deepEqual(await readFile(target), data);
    assert.equal((await readdir(root)).filter((name) => name.includes('.part-')).length, 1);
    await receiveFile(target, expected, () => {
      throw Error('must not download twice');
    });
    await writeFile(target, 'changed');
    await assert.rejects(
      receiveFile(target, expected, () => {
        throw Error('must not overwrite');
      }),
      /mismatch/,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('transferred evidence with an incorrect hash is retained but never accepted', async () => {
  const root = await mkdtemp(join(tmpdir(), 'dustaway-transfer-'));
  try {
    const target = join(root, 'code.log');
    await assert.rejects(
      receiveFile(target, { bytes: 3, sha256: sha256('yes') }, (path) => writeFile(path, 'bad')),
      /hash mismatch/,
    );
    await assert.rejects(readFile(target), { code: 'ENOENT' });
    assert.equal((await readdir(root)).length, 1);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
