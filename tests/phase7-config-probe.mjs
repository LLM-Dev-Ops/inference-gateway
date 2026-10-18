import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';

const executable = path.resolve(process.argv[2]);
const values = {
  RUVECTOR_SERVICE_URL: 'http://127.0.0.1:9',
  RUVECTOR_API_KEY: 'synthetic-probe-value-no-live-credential',
  AGENT_NAME: 'test-agent',
  AGENT_DOMAIN: 'routing',
  AGENT_PHASE: 'phase7',
  AGENT_LAYER: 'layer2',
  AGENT_VERSION: '1.0.0',
};
const binarySha256 = createHash('sha256').update(readFileSync(executable)).digest('hex');
const invoke = input => {
  const run = spawnSync(executable, [], {
    env: { PATH: process.env.PATH, RUST_BACKTRACE: '0', ...input },
    encoding: 'utf8', timeout: 10000, maxBuffer: 65536,
  });
  assert.equal(run.signal, null, 'timeout/signal is not an evaluator verdict');
  assert.notEqual(run.status, null, 'native binary must execute');
  return { exit: run.status, stdout: run.stdout, stderr: run.stderr };
};
const holdout = [], anchor = [];
for (const name of Object.keys(values)) {
  for (const blank of ['', ' \t ']) {
    const result = invoke({ ...values, [name]: blank });
    holdout.push({ id: `${name}:${blank === '' ? 'empty' : 'whitespace'}`, ...result, pass: result.exit !== 0 && result.stderr.includes(name) });
  }
}
const valid = invoke(values);
const accepted = valid.exit === 0 ? JSON.parse(valid.stdout) : null;
holdout.push({ id: 'debug-redacts-api-key', exit: valid.exit, pass: accepted?.debug_redacted === true });
anchor.push({ id: 'valid-phase7-layer2', exit: valid.exit, pass: valid.exit === 0 && accepted?.accepted === true && accepted.phase === 'phase7' && accepted.layer === 'layer2' });
for (const [name, value] of [['AGENT_PHASE', 'phase6'], ['AGENT_LAYER', 'layer1']]) {
  const result = invoke({ ...values, [name]: value });
  anchor.push({ id: `reject-${name}`, ...result, pass: result.exit !== 0 && result.stderr.includes(name) });
}
for (const name of Object.keys(values)) {
  const input = { ...values }; delete input[name];
  const result = invoke(input);
  anchor.push({ id: `missing-${name}`, ...result, pass: result.exit !== 0 && result.stderr.includes(name) });
}
console.log(JSON.stringify({ binarySha256, holdout, anchor, passed: holdout.filter(x => x.pass).length, total: holdout.length }));
assert(holdout.every(x => x.pass) && anchor.every(x => x.pass));
