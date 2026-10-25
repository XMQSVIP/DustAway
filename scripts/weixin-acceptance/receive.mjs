import { readFile, copyFile, unlink } from 'node:fs/promises';
import { constants } from 'node:fs';
import { randomUUID } from 'node:crypto';
import assert from 'node:assert/strict';
import { sha256 } from './bundle.mjs';

// An interrupted transfer never occupies the final evidence name. Keep incomplete
// bytes for diagnosis; a resumed run only fetches missing files, never reruns tests.
export async function receiveFile(target, expected, transfer) {
  const verify = (bytes) => {
    assert.equal(bytes.length, expected.bytes, 'Evidence size mismatch');
    assert.equal(sha256(bytes), expected.sha256.toLowerCase(), 'Evidence hash mismatch');
  };
  try {
    verify(await readFile(target));
    return;
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  const partial = `${target}.part-${randomUUID()}`;
  await transfer(partial);
  verify(await readFile(partial));
  await copyFile(partial, target, constants.COPYFILE_EXCL);
  await unlink(partial);
}
