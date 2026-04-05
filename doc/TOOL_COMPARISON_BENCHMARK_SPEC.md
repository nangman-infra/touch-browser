# Tool Comparison Benchmark Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `reproducible comparison between touch-browser and a markdown-only baseline on official public documentation sources`

## 1. Overview

This benchmark is designed to answer a practical adoption question:

- why use `touch-browser` instead of a simple markdown fetch tool shape

It does that with a reproducible local baseline rather than a vendor API benchmark. The reason is simple: a local baseline can run in CI, does not require private API credentials, and can be audited end to end.

Generated report:

- [report.json](../fixtures/scenarios/tool-comparison-benchmark/report.json)

Runner:

- `pnpm run fixtures:tool-comparison`

## 2. Comparison Surfaces

The benchmark compares four surfaces on the same official public pages and the same claim set:

- `markdownBaseline`: a web-fetch-style main-content markdown baseline
- `touchBrowserReadView`: readable Markdown from `read-view`
- `touchBrowserCompact`: low-token semantic state from `compact-view`
- `touchBrowserExtract`: structured evidence retrieval with citations and support refs

## 3. What Is Measured

- average output tokens
- positive-claim support rate on claims expected to be directly grounded in the source page
- plausible-negative false-positive rate on claims that sound reasonable but are not supported by the source page
- structured citation coverage
- stable-ref coverage

## 4. Source Set

The benchmark uses official public documentation pages, including:

- AWS docs
- IANA
- MDN
- Node.js

The point is not consumer search ranking. The point is grounded retrieval on official sources that a research agent would actually cite.

## 5. Why This Benchmark Is Useful

The external baseline already tells us that Markdown-first retrieval is common:

- Exa: [Markdown Contents as Default](https://docs.exa.ai/changelog/markdown-contents-as-default)
- Firecrawl: [Scrape API Reference](https://docs.firecrawl.dev/api-reference/v1-endpoint/scrape)

This benchmark therefore focuses on the next question:

- can `touch-browser` keep or improve claim reliability while also reducing tokens and adding structured citations, stable refs, and session-ready outputs

## 6. Interpretation

Current generated baseline on `2026-04-05`:

- `markdownBaseline`: `1908.75` average tokens, `1.00` positive-claim support rate, `0.33` plausible-negative false-positive rate
- `touchBrowserReadView`: `1852.75` average tokens, `1.00` positive-claim support rate, `0.33` plausible-negative false-positive rate
- `touchBrowserCompact`: `606.5` average tokens, `0.50` positive-claim support rate, `0.00` plausible-negative false-positive rate
- `touchBrowserExtract`: `1.00` positive-claim support rate, `0.00` plausible-negative false-positive rate, `1.00` citation coverage, `1.00` stable-ref coverage, `0.94` average support score

This means:

- a markdown-only baseline remains adequate for readable single-page review
- `read-view` reaches that same baseline without leaving the `touch-browser` runtime surface
- `compact-view` is the low-token routing surface, not the final claim-judgment surface
- `extract` is where `touch-browser` materially separates from markdown-only fetchers by keeping positive support while removing the false positives seen in the markdown baseline and adding structured citations plus stable refs
- the harder verdict-boundary questions are covered separately by [ADVERSARIAL_BENCHMARK_SPEC.md](ADVERSARIAL_BENCHMARK_SPEC.md), which checks contradiction and `needs-more-browsing` behavior on official docs

The intended decision pattern is:

- if you only need readable text, a markdown-only baseline may be enough
- if you need lower-token agent loops, structured citations, stable refs, and multi-page research integration, `touch-browser` should show a measurable advantage
