# External Baseline And Positioning

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `external public references that explain why touch-browser exists and where it is stronger than markdown-only or browser-control-only tools`

## 1. Why This Document Exists

Repository-local metrics are useful, but they are not enough on their own.

This document grounds `touch-browser` against public, authoritative references from adjacent tools and protocols so readers can answer two practical questions:

- why not just use a markdown scraper or search API
- why not just use full browser control

## 2. External Baseline

### 2.1 Markdown-first web retrieval is now table stakes

Official product docs show that the market has already standardized around AI-friendly page reduction:

- Exa switched webpage contents to clean Markdown by default and states that this format is better for AI applications, RAG, and text processing: [Markdown Contents as Default](https://docs.exa.ai/changelog/markdown-contents-as-default)
- Firecrawl documents Markdown as a primary scrape output and exposes `onlyMainContent` to scope extraction to the main content region: [Scrape](https://docs.firecrawl.dev/features/scrape), [Scrape API Reference](https://docs.firecrawl.dev/api-reference/v1-endpoint/scrape)

Implication:

- returning readable main-content Markdown is necessary
- it is not, by itself, enough to explain why `touch-browser` should exist

## 2.2 Browser-control tools already expose semantic interaction layers

Official browser-agent docs also show that the market has already standardized around semantic interaction layers instead of raw pixel control alone:

- Browserbase Agent Browser documents accessibility-tree snapshots with element refs for deterministic interactions: [Agent Browser Integration](https://docs.browserbase.com/integrations/agent-browser/introduction)

Implication:

- structured refs for agent control are also necessary
- refs alone do not answer evidence, citation, policy, or replay requirements

## 2.3 MCP is moving toward stricter structured tool contracts

The official MCP spec now emphasizes structured tool output, output schemas, security, and protocol clarity:

- MCP changelog: [Key Changes](https://modelcontextprotocol.io/specification/2025-06-18/changelog)
- MCP tools docs: [Tools](https://modelcontextprotocol.io/docs/concepts/tools)

Implication:

- returning plain text alone is no longer enough for serious agent integrations
- typed outputs, stable contracts, and validation matter

## 2.4 Full computer-use loops are powerful, but expensive and higher-risk

Anthropic’s official computer-use docs describe a sandboxed environment, prompt-injection defenses, user confirmation flows, and non-trivial tool overhead:

- security and user confirmation guidance: [Computer use tool](https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool)

Inference from those docs:

- full UI control is valuable, but it is not the cheapest or safest default for document-heavy research tasks
- there is room for a text-first research surface that keeps browser control available only when needed

## 3. What touch-browser adds on top of that baseline

`touch-browser` is not trying to win on a single axis such as “Markdown output” or “browser control.”

It combines four layers in one self-hosted runtime:

| External baseline | touch-browser addition | Why it matters |
| --- | --- | --- |
| Markdown main-content extraction | `read-view` plus `compact-view` | one surface for human review, one for low-token agent loops |
| Accessibility-tree or semantic refs | `extract` with block refs, citations, and `supportScore` | retrieved evidence can be traced back to a specific source block |
| Tool protocol integration | `serve` + MCP bridge + typed outputs | easier to embed into agent systems without inventing a custom protocol |
| Browser control | policy, allowlists, checkpoint/approve, replay, session synthesis | interaction stays auditable and review-gated instead of becoming a black box |

## 4. What the current repository artifacts prove

Current generated artifacts show the following:

- live public-doc benchmark: `5/5` successful samples, `11.58x` average cleaned DOM reduction, `1.00` must-contain recall on both runtime and browser paths
  - artifact: [PUBLIC_WEB_BENCHMARK_SPEC.md](PUBLIC_WEB_BENCHMARK_SPEC.md)
- real-user MCP benchmark: `3/3` public research scenarios passed, `8/8` extracted claims were evidence-supported, `4` official public domains covered
  - artifact: [REAL_USER_RESEARCH_BENCHMARK_SPEC.md](REAL_USER_RESEARCH_BENCHMARK_SPEC.md)

What these artifacts prove:

- `touch-browser` works on real public documentation sources, not just fixtures
- it preserves enough structure to support claim extraction and session synthesis
- it keeps a self-hosted MCP integration surface while supporting multi-tab research

What they do not prove:

- superiority on arbitrary consumer search traffic
- superiority on JS-heavy authenticated apps
- final truth judgment without a higher-level verifier or human review

## 5. When touch-browser is the better fit

Use `touch-browser` when you need:

- multi-page research sessions with citations and replay
- a low-token view for agent loops plus a readable view for audit or second-pass verification
- self-hosted MCP or JSON-RPC integration
- allowlists and supervised interaction for risky flows

## 6. When a simpler tool is enough

Use a markdown-only or search-only tool when you only need:

- a single page converted to readable text
- search ranking instead of document-grounded evidence
- no session state, no replay, and no policy controls

That is the correct comparison boundary.

`touch-browser` is strongest when the task is not just “fetch one page,” but “run a traceable research workflow across many pages.”
