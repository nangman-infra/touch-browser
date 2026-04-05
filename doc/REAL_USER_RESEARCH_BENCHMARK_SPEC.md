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

This benchmark should be read together with [EXTERNAL_BASELINE_AND_POSITIONING.md](EXTERNAL_BASELINE_AND_POSITIONING.md). The point is not that `touch-browser` can fetch a page. The point is that it can run a multi-tab, MCP-driven, evidence-traceable research workflow on official public sources.

## 2. Artifacts

Generated report:

- [report.json](../fixtures/scenarios/real-user-research-benchmark/report.json)

Runner:

- [run-real-user-research-benchmark.mjs](../scripts/run-real-user-research-benchmark.mjs)

Run:

- `pnpm run fixtures:real-user-research`
- `pnpm run pilot:real-user-research`

## 3. Source Set

The current scenarios use official public documentation sources:

- IANA reserved and example domain documentation: [IANA Reserved Domains](https://www.iana.org/domains/reserved), [IANA Example Domains](https://www.iana.org/help/example-domains)
- RFC Editor standards pages: [RFC 9309](https://www.rfc-editor.org/rfc/rfc9309.html), [RFC 2606](https://www.rfc-editor.org/rfc/rfc2606.html), [RFC 6761](https://www.rfc-editor.org/rfc/rfc6761.html)
- MDN Web API docs: [Fetch API](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API), [AbortController](https://developer.mozilla.org/en-US/docs/Web/API/AbortController)
- Node.js docs: [Path](https://nodejs.org/api/path.html), [URL](https://nodejs.org/api/url.html)

This source set is intentionally official and citation-friendly.

## 4. Current Baseline

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

## 5. Why This Benchmark Matters

The external baseline already includes:

- Markdown-first retrieval systems such as Exa and Firecrawl: [Exa](https://docs.exa.ai/changelog/markdown-contents-as-default), [Firecrawl](https://docs.firecrawl.dev/features/scrape)
- MCP as a structured tool protocol for agents: [MCP changelog](https://modelcontextprotocol.io/specification/2025-06-18/changelog), [MCP tools](https://modelcontextprotocol.io/docs/concepts/tools)

This benchmark adds a stronger product claim on top of that baseline:

- the agent can keep multiple official sources open at once
- the workflow remains within a typed MCP tool surface
- claims can still be extracted with citations
- the session can be synthesized and closed cleanly at the end
- harder contradiction and unresolved-claim cases are covered separately by [ADVERSARIAL_BENCHMARK_SPEC.md](ADVERSARIAL_BENCHMARK_SPEC.md)

## 6. Interpretation

- this artifact targets real public documentation sources rather than a local sample app
- it is therefore closer to a real AI research environment than a fixture-only proof
- the current baseline spans IANA, RFC Editor, MDN, and Node.js docs, which gives it more source diversity than a single-domain demo
- it is still a curated task suite rather than a proof of arbitrary consumer search traffic

## 7. Notes

- the benchmark is URL- and domain-curated because search-query discovery is not the product surface being measured
- authenticated apps, anti-bot pages, and private enterprise systems are intentionally excluded
- public documentation content and network behavior can still change underneath the benchmark
