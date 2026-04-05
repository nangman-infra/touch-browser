# Real User Research Benchmark Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `public multi-task proof for user-like AI research workflows`

## 1. Overview

This benchmark goes beyond a sample workflow and repeatedly exercises public-document research questions that a real user might hand to an AI research agent.

The goal is not search-engine ranking quality. The goal is to prove:

- real public URLs
- question-driven multi-source research
- an external-agent surface through MCP
- multi-tab control
- end-to-end extract, synthesize, and close behavior

## 2. Artifacts

Generated report:

- [report.json](../fixtures/scenarios/real-user-research-benchmark/report.json)

Runner:

- [run-real-user-research-benchmark.mjs](../scripts/run-real-user-research-benchmark.mjs)

Run:

- `pnpm run fixtures:real-user-research`
- `pnpm run pilot:real-user-research`

## 3. Current Baseline

The benchmark currently covers:

- public standards research
- public web API documentation research
- public Node.js runtime documentation research

Generated baseline on `2026-04-05`:

- scenario count: `3`
- passed scenario count: `3`
- total extracted claims: `8`
- total supported claims: `8`
- average supported claim rate: `1.00`
- average listed tab count: `3.00`
- unique public domains: `4`

Passing conditions:

- scenario count `3+`
- passed scenario count equals scenario count
- average supported claim rate `1.00`
- average listed tab count `2+`
- unique public domains `4+`
- each scenario closes the session cleanly

## 4. Interpretation

- this artifact targets real public documentation sources rather than a local sample app
- it is therefore closer to a real AI research environment than a fixture-only proof
- the current baseline spans IANA, RFC Editor, MDN, and Node.js docs, which gives it more source diversity than a single-domain demo
- it is still a curated task suite rather than a proof of arbitrary consumer search traffic

## 5. Notes

- the benchmark is URL- and domain-curated because search-query discovery is not the product surface being measured
- authenticated apps, anti-bot pages, and private enterprise systems are intentionally excluded
- public documentation content and network behavior can still change underneath the benchmark
