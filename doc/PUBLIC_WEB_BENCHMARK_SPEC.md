# Public Web Benchmark Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `live public-web benchmark for runtime/browser/session synthesis`

## 1. Overview

This document records how `touch-browser` behaves on real public web documents rather than local fixtures or deterministic sample apps.

Current scope:

- runtime live open
- browser-backed live open
- tokenizer-based compact reduction
- must-contain recall
- daemon session multi-tab synthesis
- public research task proof

## 2. Why This Benchmark Matters

The external baseline is already crowded:

- Exa now returns clean Markdown by default for webpage contents and explicitly positions that format as AI-friendly: [Exa changelog](https://docs.exa.ai/changelog/markdown-contents-as-default)
- Firecrawl exposes Markdown output and `onlyMainContent` as standard scrape controls: [Firecrawl scrape docs](https://docs.firecrawl.dev/features/scrape), [Scrape API Reference](https://docs.firecrawl.dev/api-reference/v1-endpoint/scrape)
- Browserbase Agent Browser exposes accessibility-tree snapshots with refs for deterministic agent interactions: [Browserbase docs](https://docs.browserbase.com/integrations/agent-browser/introduction)

This benchmark is therefore not trying to prove that `touch-browser` can merely fetch readable text.

It is trying to prove a stricter claim:

- the same public pages can be reduced into lower-token semantic state
- recall can stay high while doing that reduction
- both runtime and browser-backed paths can preserve the same behavior
- the public pages can still be turned into claim-level evidence and multi-tab synthesis

For the broader comparison boundary, see [EXTERNAL_BASELINE_AND_POSITIONING.md](EXTERNAL_BASELINE_AND_POSITIONING.md).

## 3. Artifacts

Generated report:

- [report.json](../fixtures/scenarios/public-web-benchmark/report.json)

Runner:

- `pnpm run fixtures:public-web`

## 4. Current Baseline

Regenerated baseline on `2026-04-05`:

- public sample count: `5`
- successful sample count: `5`
- runtime HTML tokenizer reduction ratio: `20.17`
- runtime cleaned DOM tokenizer reduction ratio: `11.58`
- runtime reading-surface HTML tokenizer reduction ratio: `23.44`
- runtime reading-surface cleaned DOM tokenizer reduction ratio: `13.67`
- browser HTML tokenizer reduction ratio: `20.17`
- browser cleaned DOM tokenizer reduction ratio: `11.58`
- browser reading-surface HTML tokenizer reduction ratio: `23.44`
- browser reading-surface cleaned DOM tokenizer reduction ratio: `13.67`
- runtime/browser must-contain recall: `1.00 / 1.00`
- synthesis status: `ok`
- synthesis tab count: `5`
- task-proof extracted claims: `4`
- task-proof supported claims: `4`
- task-proof supported claim rate: `1.00`

Current sample set:

- `https://www.iana.org/domains/reserved`
- `https://www.iana.org/domains/example`
- `https://www.rfc-editor.org/rfc/rfc9309.html`
- `https://www.rfc-editor.org/rfc/rfc2606.html`
- `https://www.rfc-editor.org/rfc/rfc6761.html`

These are intentionally official, public, document-style sources where content quality matters more than consumer search ranking.

## 5. Interpretation

- this baseline is measured on real public pages, not deterministic fixtures
- all five samples kept `1.00` must-contain recall on both runtime and browser-backed paths
- the average cleaned DOM reduction stayed above `8x` for both runtime and browser-backed compact surfaces
- the same sample set also supported a public multi-tab research proof with `4/4` evidence-supported claims
- this is stronger than proving “we can fetch Markdown” because it keeps evidence extraction and session synthesis in the loop

## 6. Limits

- the sample count is still small and biased toward stable document pages
- JS-heavy applications are measured separately in [JS_RENDERER_BENCHMARK_SPEC.md](JS_RENDERER_BENCHMARK_SPEC.md)
- authenticated apps and anti-bot pages are intentionally excluded
- this benchmark is not a head-to-head competitive scrape benchmark against Exa or Firecrawl outputs
- the live public baseline is not part of the default `pnpm test` gate because network volatility and upstream page changes should stay separate from deterministic regression signals
