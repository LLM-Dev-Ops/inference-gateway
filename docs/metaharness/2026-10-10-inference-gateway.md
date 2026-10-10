# MetaHarness evaluation: inference gateway boundary

## Frozen contract

- Base commit: `a07a604dc14a3eeb2b9a07e85e9c41212156de9a`.
- Preserve the existing Node test workload and the Rust workspace.
- Require zero production `npm audit` findings from the locked dependency graph.
- Reject unsafe correlation IDs, non-JSON or malformed upstream responses, responses over the configured byte limit, and upstream calls beyond the configured timeout.
- Preserve the upstream HTTP status for bounded non-JSON responses.
- Require Rust 1.83.0 formatting, all-target/all-feature clippy with warnings denied, and locked all-feature workspace tests before merge.
- Do not change persistence, provider routing, or the Rust/TypeScript architecture.

## Pinned tools and real invocations

- MetaHarness repository: `ruvnet/metaharness@e0dfd44da72b7adfb7c57fdd42d124be58ed7086`.
- `metaharness@0.4.17`: `npx --yes metaharness@0.4.17 score . --json` returned harness fit 76, compile confidence 100, task coverage 100, tool safety 95, and `vertical:devops` as the recommended template.
- `@metaharness/darwin@0.10.3`: one real-sandbox generation, two children, concurrency 1, deterministic seed 20261010. Baseline and both reviewer variants scored 0.985; neither variant cleared the 0.05 promotion delta, so the baseline harness was retained.
- `@metaharness/flywheel@0.1.12`: one live generation over a frozen six-check evaluator. The Ed25519-signed replay passed with gate fingerprint `c5942e7c9bf8f28fe7af47a6ec3887d11eeb3f8c8e8601cc6a7a54969ea0b981` and chain `gen1(sourceVariant) -> gen0(root)`.

## Measured result

| Metric | Baseline | Candidate |
| --- | ---: | ---: |
| Existing/new Node tests | 9/9 | 14/14 |
| Production dependencies reported by audit | 416 | 1 (the root package; zero direct runtime dependencies) |
| Production audit findings | 36 (2 critical, 25 high) | 0 |
| Frozen boundary checks passed | 1/6 | 6/6 |
| Flywheel primary score | 0.1667 | 1.0000 |
| Flywheel cost-per-win proxy | 6.0 | 1.0 |
| Flywheel regression flag | false | false |

The removed `claude-flow` package was not imported by repository source. The candidate adds bounded response streaming (default 1 MiB, configurable from 1 KiB through 10 MiB), a bounded gateway timeout (default 30 seconds, configurable from 100 ms through 120 seconds), safe upstream error envelopes, and correlation-ID validation.

Rust was not available in the local execution image (`cargo: command not found`). The pinned Linux CI workflow is therefore a mandatory promotion gate rather than inferred evidence.

## Rollback

Revert the merge commit for this change. That restores the previous lockfile, dependency graph, proxy behavior, and absence of the CI workflow as one atomic operation.
