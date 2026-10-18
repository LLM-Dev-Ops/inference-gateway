import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';

// node tests/evaluate-phase7.mjs BASELINE CANDIDATE PINNED_TOOLS OUTPUT
const [baseline, candidate, tools, output] = process.argv.slice(2).map(x => resolve(x));
const fw = await import(pathToFileURL(join(tools, 'node_modules/@metaharness/flywheel/dist/index.js')));
assert.equal(JSON.parse(readFileSync(join(tools, 'node_modules/@metaharness/flywheel/package.json'))).version, '0.1.12');
const source = 'crates/gateway-agents/src/phase7/config.rs';
const contractBytes = readFileSync(join(candidate, 'docs/metaharness/2026-10-18/contract.json'));
const contract = JSON.parse(contractBytes);
const fixture = join(candidate, 'tests/phase7-config-probe.mjs');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const sourceHash = dir => hash(readFileSync(join(dir, source)));
const rootHash = sourceHash(baseline), fixedHash = sourceHash(candidate);
assert.notEqual(rootHash, fixedHash);
const lockHash = dir => hash(readFileSync(join(dir, 'Cargo.lock')));
assert.equal(lockHash(baseline), lockHash(candidate));
assert.equal(hash(readFileSync(join(baseline, 'crates/gateway-agents/examples/phase7-config-probe.rs'))), hash(readFileSync(join(candidate, 'crates/gateway-agents/examples/phase7-config-probe.rs'))));
const parentBytes = readFileSync(join(baseline, source), 'utf8');
const measurements = [], builds = [];
function run(dir, command, args) {
  const r = spawnSync(command, args, {
    cwd: dir, encoding: 'utf8', maxBuffer: 8 * 1024 * 1024, timeout: 120000,
    // Isolate the existing unreachable-upstream unit fixture from desktop-wide
    // gateway configuration. Apply the identical endpoint to both revisions.
    env: { ...process.env, GATEWAY_INTERNAL_URL: 'http://127.0.0.1:9', CARGO_PROFILE_DEV_DEBUG: '0', CARGO_PROFILE_TEST_DEBUG: '0', CARGO_INCREMENTAL: '0', CARGO_BUILD_JOBS: '3' },
  });
  assert.equal(r.signal, null, 'signals/timeouts are not completed evaluations');
  assert.notEqual(r.status, null, 'command must execute');
  return { command: [command, ...args], exit: r.status, stdout: r.stdout, stderr: r.stderr };
}
// Rebuild against each exact production source, not a substitute test module.
for (const dir of [baseline, candidate]) {
  const build = run(dir, 'cargo', ['+1.88.0', 'build', '-p', 'gateway-agents', '--example', 'phase7-config-probe', '--locked']);
  assert.equal(build.exit, 0);
  builds.push({ source: sourceHash(dir), command: build.command, exit: build.exit, stderr: build.stderr.slice(-4000) });
}
let existingNames;
async function evaluator(policy, suite) {
  const dir = policy.startup === rootHash ? baseline : candidate;
  assert.equal(sourceHash(dir), policy.startup);
  const native = run(dir, process.execPath, [fixture, join(dir, 'target/debug/examples/phase7-config-probe')]);
  assert([0, 1].includes(native.exit));
  const report = JSON.parse(native.stdout);
  assert.equal(report.holdout.length, 15);
  assert.equal(report.anchor.length, 10);
  const commands = [{ command: native.command, exit: native.exit, stderr: native.stderr.slice(-1000) }];
  let itemWins, testsDetail;
  if (suite.id === 'holdout') itemWins = report.holdout.map(x => x.pass);
  else {
    const tests = run(dir, process.execPath, ['--test', '--test-reporter=tap', 'test/index.test.js']);
    assert.match(tests.stdout, /# pass 14\b/);
    assert.match(tests.stdout, /# fail 0\b/);
    assert.match(tests.stdout, /# skipped 0\b/);
    const names = tests.stdout.split('\n').filter(x => x.includes('# Subtest:')).map(x => x.trim()).sort();
    if (dir === baseline) existingNames = names;
    assert(existingNames?.length > 0);
    const retained = existingNames.every(x => names.includes(x));
    itemWins = [...report.anchor.map(x => x.pass), tests.exit === 0 && retained];
    testsDetail = { passed: 14, skipped: 0, retainedNames: retained, existingNamesSha256: hash(JSON.stringify(existingNames)) };
    commands.push({ command: tests.command, exit: tests.exit, stdout: tests.stdout, stderr: tests.stderr });
  }
  const primary = itemWins.filter(Boolean).length / itemWins.length;
  const score = { primary, noopRate: 1 - primary, costPerWin: 0, regressed: suite.id === 'anchor' && itemWins.some(x => !x), itemWins };
  measurements.push({ source: policy.startup, suite: suite.id, score, report, tests: testsDetail, commands });
  assert.equal(sourceHash(dir), policy.startup);
  assert.equal(lockHash(dir), lockHash(baseline));
  return score;
}
const fingerprint = fw.gateFingerprint(fw.meetsPromotionRule);
const result = await fw.runFlywheelGenerations({
  rootPolicy: { startup: rootHash },
  proposer: async () => ({ value: fixedHash, summary: 'Reject blank startup values, redact authentication diagnostics, isolate parser tests without global env writes', inverse: { path: source, parentBytes, hash: hash(parentBytes) } }),
  evaluator,
  holdout: { id: 'holdout', items: Array.from({ length: 15 }, (_, i) => `frozen-startup-${i}`) },
  anchor: { id: 'anchor', items: Array.from({ length: 11 }, (_, i) => `frozen-compatibility-${i}`) },
  maxGenerations: 1, signer: fw.makeSigner(), promotionRule: fw.meetsPromotionRule,
  dataSource: 'LIVE_RUST_PRODUCTION_CONFIG_BINARIES_SEPARATE_PROCESS_ENVIRONMENTS_AND_NODE_TESTS',
});
const replay = fw.verifyReplayBundle(result.replayBundle, { pinnedGateFingerprint: fingerprint });
assert.equal(replay.pass, true);
assert.equal(result.promotions.length, 1);
const evidence = { contract, contractSha256: hash(contractBytes), fixtureSha256: hash(readFileSync(fixture)), baselineSource: rootHash, candidateSource: fixedHash, cargoLock: lockHash(candidate), builds, measurements, promotions: result.promotions.length, fingerprint, replay, replayBundle: result.replayBundle, boundary: 'Component configuration validation only, not whole-repository approval. No network calls, live secrets or RuVector persistence claims. Cost/latency/energy axes not measured; zero costPerWin is a neutral gate input, not a measurement. Full parallel Rust tests, strict Clippy, builds, formatting, audit and CI are independent merge gates.' };
writeFileSync(output, JSON.stringify(evidence, null, 2) + '\n');
console.log(JSON.stringify({ promotions: evidence.promotions, replay, measurements: measurements.map(m => ({ suite: m.suite, primary: m.score.primary, nativeHoldout: m.report.passed, nativeAnchor: m.report.anchor.filter(x => x.pass).length, tests: m.tests })) }));
