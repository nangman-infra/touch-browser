# Evidence-Grounded Web Research Benchmark Spec

- Status: `Active`
- Version: `v1-seed`
- Last Updated: `2026-05-05`
- Scope: `claim verification / citation precision / abstention routing / false-positive control`

## 1. Purpose

This benchmark measures evidence-grounded web research, not answer generation.

The unit is a claim or citation fixture. A passing system must:

- collect source-linked evidence
- classify support, contradiction, insufficient evidence, and more-browsing routes
- preserve citations and stable references
- avoid unsafe auto-answer behavior on adversarial claims
- beat or match a markdown-only baseline on plausible negative false positives

## 2. Seed Corpus

The current seed benchmark aggregates these generated reports:

- `public-web-benchmark`
- `real-user-research-benchmark`
- `adversarial-benchmark`
- `citation-metrics`
- `tool-comparison-benchmark`

The generated report is:

- [report.json](/Volumes/WD/Developments/touch-browser/fixtures/scenarios/evidence-grounded-web-research-benchmark/report.json)

The generator is:

- [generate-evidence-grounded-web-research-benchmark.mjs](/Volumes/WD/Developments/touch-browser/scripts/generate-evidence-grounded-web-research-benchmark.mjs)

## 3. Quality Gates

`status=seed-validated` requires:

- public-web supported claim rate >= `0.95`
- real-user supported claim rate >= `0.95`
- verified adversarial exact verdict accuracy >= `0.95`
- verified unsafe auto-answer count = `0`
- citation precision >= `0.95`
- unsupported precision >= `0.95`
- support-reference precision >= `0.95`
- touch-browser extract false-positive rate <= markdown baseline false-positive rate

## 4. v1 Target

The benchmark target is `1000` claims across public standards, vendor docs, API docs, pricing/reference pages, ambiguous claims, numeric mismatch cases, and contradiction cases.

The seed benchmark is the executable category surface. The 1000-claim corpus should expand this same contract rather than introducing answer-generation scoring.
