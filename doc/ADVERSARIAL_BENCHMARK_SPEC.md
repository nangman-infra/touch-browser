# Adversarial Benchmark Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `official public documentation cases that stress numeric, scope, and ambiguous-overview claim handling`

## 1. Overview

This benchmark exists to answer a different question from the broad public-web and comparison benchmarks:

- does `touch-browser` stay on the correct side of plausible but risky claims

The benchmark intentionally uses claims that often fool lexical matching:

- exact numeric mismatches such as `15 minutes` vs `24 hours`
- overview-page claims that should trigger `needs-more-browsing` instead of a premature answer
- direct contradictions on official public sources

Generated report:

- [report.json](../fixtures/scenarios/adversarial-benchmark/report.json)

Runner:

- `pnpm run fixtures:adversarial`

## 2. Source Set

The current benchmark uses official documentation pages from:

- AWS Lambda documentation
- AWS ECS documentation
- IANA

These are intentionally the kinds of public sources that an AI research agent would cite in practice.

## 3. What Is Measured

- exact raw verdict accuracy from `extract`
- exact verified verdict accuracy from `extract --verifier-command 'node scripts/example-verifier.mjs'`
- whether ambiguous overview pages stay unresolved as `needs-more-browsing`
- whether direct official-source contradictions land in `contradicted`

## 4. Current Interpretation

Current generated baseline on `2026-04-05`:

- sample count: `5`
- raw exact verdict accuracy: `1.00`
- verified exact verdict accuracy: `1.00`
- overview claims that need a more specific source page stay in `needs-more-browsing`
- numeric mismatches on exact limits pages are `contradicted`
- direct public contradictions such as IANA registration claims are `contradicted`

This benchmark is not trying to prove universal truth judgment.

It is trying to prove the more practical property:

- `touch-browser` should avoid premature certainty and should tell the agent when to keep browsing
