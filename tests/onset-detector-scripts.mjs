import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');

export const python = '/Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3';

export const DETECTOR_FAILURE_TAIL = `
assert vad._failed
assert vad.process(np.ones(2560, dtype=np.float32)) == 0.0
print('OK')
`;

export function runCode(code) {
  return spawnSync(python, ['-c', code], { cwd: root, encoding: 'utf8' });
}

export function runArgs(args) {
  return spawnSync(python, args, { cwd: root, encoding: 'utf8' });
}

export function expectDetectorDisabled(result) {
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /OK/);
  assert.match(result.stderr, /listening disabled/);
}
