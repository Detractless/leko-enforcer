import assert from 'node:assert/strict';
import { runPythonSync } from './python-runner.mjs';

export const DETECTOR_FAILURE_TAIL = `
assert vad._failed
assert vad.process(np.ones(2560, dtype=np.float32)) == 0.0
print('OK')
`;

export function runCode(code) {
  return runPythonSync(['-c', code]);
}

export function runArgs(args) {
  return runPythonSync(args);
}

export function expectDetectorDisabled(result) {
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /OK/);
  assert.match(result.stderr, /listening disabled/);
}
