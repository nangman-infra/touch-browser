# Adversarial Benchmark Spec

- Status: `Active`
- Version: `v2`
- Last Updated: `2026-04-10`
- Scope: `official public documentation cases that stress numeric, scope, ambiguous-overview claim handling, and selective prediction safety`

## 1. Overview

This benchmark exists to answer a different question from the broad public-web and comparison benchmarks:

- does `touch-browser` stay on the correct side of plausible but risky claims
- does it avoid unsafe `high` auto-answer states on the wrong claims
- does it route review cases into verifier or more-browsing paths instead of overclaiming

The benchmark intentionally uses claims that often fool lexical matching:

- exact numeric mismatches such as `15 minutes` vs `24 hours`
- overview-page claims that should trigger `needs-more-browsing` instead of a premature answer
- explicit contradictions on official public sources

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
- raw high-band precision for `evidence-supported` claims that are marked safe for direct reuse
- raw and verified unsafe auto-answer count
- raw and verified review capture rate on scenarios tagged as `review`
- raw and verified explainability coverage (`verdictExplanation` plus `supportSnippets` on supported claims)
- whether ambiguous overview pages stay unresolved as `needs-more-browsing`
- whether false but source-ambiguous claims stay on the `review` path instead of becoming unsafe auto-answer states

## 4. Current Interpretation

Current generated baseline on `2026-04-10` should satisfy all of the following:

- sample count: `5`
- raw exact verdict accuracy: tracked, but not used alone as the release decision
- verified exact verdict accuracy: `1.00`
- raw unsafe auto-answer count: `0`
- verified unsafe auto-answer count: `0`
- raw review capture rate: `1.00`
- verified review capture rate: `1.00`
- raw explainability coverage: `1.00`
- verified explainability coverage: `1.00`
- overview claims that need a more specific source page stay in `needs-more-browsing`
- false numeric limit claims on exact limits pages are at least `review`/`needs-more-browsing` unless the extracted block is explicit enough to justify `contradicted`
- implicit non-availability pages such as IANA example domains stay on the `review` path unless the page states the negative claim explicitly
- explicit public contradictions remain covered by targeted unit and fixture tests where the contradictory sentence is unambiguous

This benchmark is not trying to prove universal truth judgment.

It is trying to prove the more practical property:

- `touch-browser` should avoid premature certainty and should tell the agent when to keep browsing
- `touch-browser` should reserve `high` auto-answer states for the small curated subset that is safe to reuse directly
