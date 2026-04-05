# touch-browser

[![License: BUSL-1.1](https://img.shields.io/badge/license-BUSL--1.1-blue.svg)](LICENSE)
[![Status: pilot-ready](https://img.shields.io/badge/status-pilot--ready-2d7d46.svg)](doc/RELEASE_READINESS_SPEC.md)

Turn any web page into structured, citable evidence for AI agents.

![Terminal demo](demo/terminal-demo.gif)

`touch-browser` turns a page into:

- readable Markdown for higher-level review with `read-view`
- compact semantic state for agent loops with `compact-view`
- traceable evidence with citations, four-state claim outcomes, and optional verifier adjudication
- replayable, policy-gated browser sessions for multi-page research

Evidence-first, not fact-final:

- `touch-browser` helps an AI collect evidence and trace where it came from.
- A higher-level model or human still decides what is true.

## Why It Exists

| Problem | touch-browser |
| --- | --- |
| Raw HTML wastes tokens | `compact-view` emits a compact semantic snapshot instead of a full DOM dump |
| Answers lose their sources | `extract` returns block refs plus URL, retrieved time, and source metadata |
| Agents click risky controls too early | policy reports and supervised actions keep risky steps review-gated |
| Multi-page research is hard to audit | session synthesis keeps visited URLs, notes, citations, and replayable traces |

Repository proof points from the current generated artifacts:

- public web benchmark: `5/5` live public-doc samples succeeded, `11.58x` average cleaned DOM token reduction. See [doc/PUBLIC_WEB_BENCHMARK_SPEC.md](doc/PUBLIC_WEB_BENCHMARK_SPEC.md).
- real-user research benchmark: `3/3` MCP-driven public research scenarios passed, `8/8` extracted claims were evidence-backed, `4` public domains covered. See [doc/REAL_USER_RESEARCH_BENCHMARK_SPEC.md](doc/REAL_USER_RESEARCH_BENCHMARK_SPEC.md).
- tool comparison benchmark: compare `touch-browser` against a reproducible markdown-only baseline on the same official public pages. See [doc/TOOL_COMPARISON_BENCHMARK_SPEC.md](doc/TOOL_COMPARISON_BENCHMARK_SPEC.md).
- adversarial benchmark: `5/5` official-doc contradiction and `needs-more-browsing` cases now land on the expected verdict. See [doc/ADVERSARIAL_BENCHMARK_SPEC.md](doc/ADVERSARIAL_BENCHMARK_SPEC.md).
- documentation trust gate: tracked Markdown links are checked against real files, anchors, and live external docs. See [doc/DOC_LINK_INTEGRITY_SPEC.md](doc/DOC_LINK_INTEGRITY_SPEC.md).

Current comparison benchmark on official AWS, IANA, MDN, and Node.js docs:

- markdown baseline: `1908.75` average tokens, `1.00` positive-claim support rate, `0.33` plausible-negative false-positive rate
- `touch-browser read-view`: `1852.75` average tokens, `1.00` positive-claim support rate, `0.33` plausible-negative false-positive rate
- `touch-browser compact-view`: `606.5` average tokens
- `touch-browser extract`: `1.00` positive-claim support rate, `0.00` plausible-negative false-positive rate, `1.00` citation coverage, `1.00` stable-ref coverage

These numbers make more sense in the context of the wider tool landscape. See [doc/EXTERNAL_BASELINE_AND_POSITIONING.md](doc/EXTERNAL_BASELINE_AND_POSITIONING.md) for the external baseline behind the benchmark claims.

## Why Not Just Use Markdown Fetch?

Official docs from adjacent products already show that AI-friendly page reduction is table stakes:

- Exa now returns clean Markdown by default for webpage contents: [Exa changelog](https://docs.exa.ai/changelog/markdown-contents-as-default)
- Firecrawl exposes Markdown output plus `onlyMainContent`: [Firecrawl scrape docs](https://docs.firecrawl.dev/features/scrape)
- Browserbase Agent Browser exposes accessibility-tree snapshots with element refs: [Browserbase docs](https://docs.browserbase.com/integrations/agent-browser/introduction)

`touch-browser` exists for the next layer:

- `read-view` for readable review
- `compact-view` for lower-token agent loops
- `extract` for block-level evidence and citations
- `session-synthesize`, policy, replay, and MCP for multi-page research workflows

That comparison boundary is documented in [doc/EXTERNAL_BASELINE_AND_POSITIONING.md](doc/EXTERNAL_BASELINE_AND_POSITIONING.md).
The reproducible comparison numbers live in [doc/TOOL_COMPARISON_BENCHMARK_SPEC.md](doc/TOOL_COMPARISON_BENCHMARK_SPEC.md).

## What The Output Looks Like

Read a real page as Markdown:

```bash
cargo run -q -p touch-browser-cli -- read-view https://www.iana.org/help/example-domains
```

```md
# Example Domains

As described in RFC 2606 and RFC 6761, a number of domains such as example.com
and example.org are maintained for documentation purposes.

- [RFC 2606](https://www.rfc-editor.org/rfc/rfc2606.html)
- [RFC 6761](https://www.rfc-editor.org/rfc/rfc6761.html)
```

Use `--main-only` when you want the Markdown view to stay tightly scoped to the main content region on navigation-heavy pages.

Compact the same page for an agent loop:

```bash
cargo run -q -p touch-browser-cli -- compact-view https://www.iana.org/help/example-domains
```

```json
{
  "approxTokens": 171,
  "lineCount": 38,
  "compactText": "h1 Example Domains ... a RFC 2606 ... a RFC 6761 ...",
  "navigationRefIndex": [
    { "id": "b9", "kind": "link", "ref": "rmain:link:go-rfc2606" },
    { "id": "b10", "kind": "link", "ref": "rmain:link:go-rfc6761" }
  ]
}
```

Extract citable evidence from the live page:

```bash
cargo run -q -p touch-browser-cli -- extract https://www.iana.org/help/example-domains \
  --claim "As described in RFC 2606 and RFC 6761, a number of domains such as example.com and example.org are maintained for documentation purposes."
```

```json
{
  "extract": {
    "output": {
      "evidenceSupportedClaims": [
        {
          "claimId": "c1",
          "statement": "As described in RFC 2606 and RFC 6761, a number of domains such as example.com and example.org are maintained for documentation purposes.",
          "supportScore": 0.95,
          "support": ["b8"],
          "citation": {
            "url": "https://www.iana.org/help/example-domains",
            "retrievedAt": "2026-03-14T00:01:30+09:00",
            "sourceLabel": "Example Domains"
          }
        }
      ]
    }
  }
}
```

`supportScore` here is the current evidence-match score for retrieved support, not a final truth guarantee.
Final claim outcomes are intentionally conservative: `evidence-supported`, `contradicted`, `insufficient-evidence`, or `needs-more-browsing`.
If you want a second-pass judge, add `--verifier-command <shell-command>` and let it adjudicate the final verdict without replacing the core evidence collector.

## 30-Second Quick Start

Prerequisites: [rustup](https://rustup.rs), Node.js 18+, `pnpm`.

```bash
bash scripts/bootstrap-local.sh

# Read a real public page
cargo run -q -p touch-browser-cli -- read-view https://www.iana.org/help/example-domains

# Produce the low-token agent view
cargo run -q -p touch-browser-cli -- compact-view https://www.iana.org/help/example-domains

# Extract source-linked evidence from the same page
cargo run -q -p touch-browser-cli -- extract https://www.iana.org/help/example-domains \
  --claim "As described in RFC 2606 and RFC 6761, a number of domains such as example.com and example.org are maintained for documentation purposes."

# Attach the conservative example verifier as a second pass
cargo run -q -p touch-browser-cli -- extract https://www.iana.org/help/example-domains \
  --claim "As described in RFC 2606 and RFC 6761, a number of domains such as example.com and example.org are maintained for documentation purposes." \
  --verifier-command 'node scripts/example-verifier.mjs'
```

If you already built the binary, replace `cargo run -q -p touch-browser-cli --` with `touch-browser`.

## Architecture

```text
URL / fixture / browser tab
  -> Acquisition
  -> Observation compiler (semantic snapshot + stable refs)
  -> read-view / compact-view
  -> extract (evidence + citations + final verdicts + optional verifier adjudication)
  -> policy
  -> session synthesis / replay
  -> CLI / JSON-RPC serve / MCP
```

## Use Cases

- research agents that need citations instead of raw HTML
  Works well for agent frameworks that want a low-token page view plus traceable evidence.
- evidence-linked RAG ingestion pipelines
  Use `read-view` for readable source text and `extract` for claim-level provenance.
- policy-gated web research in self-hosted environments
  Keep domain allowlists, supervised actions, and replay inside your own boundary.
- multi-tab source collection for reports, audits, and replay
  Use persisted sessions or daemon tabs when the task spans many pages and sources.

## CLI At A Glance

Stable research surface:

| Command | What it does |
| --- | --- |
| `open` | open a target and compile a structured snapshot |
| `read-view` | emit readable Markdown for direct review or a higher-level verifier |
| `compact-view` | emit the compact semantic view optimized for AI consumption |
| `extract` | extract claim outcomes with citations: evidence-supported, contradicted, insufficient-evidence, or needs-more-browsing |
| `policy` | return allow/review/block signals for the current target |
| `session-read` | emit readable Markdown from the latest persisted browser snapshot |
| `session-synthesize` | combine multi-page session evidence into JSON or Markdown |
| `serve` | expose the runtime over stdio JSON-RPC for integrations |

Experimental supervised surface:

| Command | What it does |
| --- | --- |
| `checkpoint` | inspect risky state and recommended supervised next steps |
| `approve` | record risk acknowledgements for a session |
| `click` | click an interactive target inside a supervised session |
| `type` | type into a field inside a supervised session |
| `submit` | submit a form or control inside a supervised session |
| `refresh` | refresh a supervised browser session |

The full command table lives in [doc/CLI_SURFACE_SPEC.md](doc/CLI_SURFACE_SPEC.md).

The example second-pass verifier lives at [scripts/example-verifier.mjs](scripts/example-verifier.mjs).

## Examples, Benchmarks, And Integrations

- examples: [examples/README.md](examples/README.md)
- benchmarks: [benchmarks/README.md](benchmarks/README.md)
- demo assets: [demo/README.md](demo/README.md)
- integrations: [integrations/README.md](integrations/README.md)

## MCP Example

Minimal MCP bridge setup from the repository root:

```json
{
  "mcpServers": {
    "touch-browser": {
      "command": "node",
      "args": ["scripts/touch-browser-mcp-bridge.mjs"]
    }
  }
}
```

The bridge starts `touch-browser serve` underneath and exposes tools like `tb_open`, `tb_read_view`, `tb_extract`, `tb_tab_open`, and `tb_session_synthesize`.
`tb_read_view` accepts `mainOnly`, and `tb_extract` accepts `verifierCommand`.

## Design Principles

- evidence-first: return traceable support, not just prose
- read-first by default: the stable surface is for research, not destructive automation
- policy-gated interaction: risky auth/write flows stay supervised
- self-hostable control: CLI, JSON-RPC daemon, and MCP bridge can run inside your boundary

This project is not a consumer browser, not a stealth automation stack, and not a final truth oracle.

## Documentation

- getting started and operations: [doc/INSTALL_AND_OPERATIONS.md](doc/INSTALL_AND_OPERATIONS.md)
- command surface: [doc/CLI_SURFACE_SPEC.md](doc/CLI_SURFACE_SPEC.md)
- MCP bridge: [doc/MCP_BRIDGE_SPEC.md](doc/MCP_BRIDGE_SPEC.md)
- license policy: [LICENSE-POLICY.md](LICENSE-POLICY.md)
- pilot package: [doc/PILOT_PACKAGE_SPEC.md](doc/PILOT_PACKAGE_SPEC.md)
- operations and security package: [doc/OPERATIONS_SECURITY_PACKAGE_SPEC.md](doc/OPERATIONS_SECURITY_PACKAGE_SPEC.md)
- public workflow examples: [doc/PUBLIC_REFERENCE_WORKFLOW_SPEC.md](doc/PUBLIC_REFERENCE_WORKFLOW_SPEC.md)
- real-user benchmark: [doc/REAL_USER_RESEARCH_BENCHMARK_SPEC.md](doc/REAL_USER_RESEARCH_BENCHMARK_SPEC.md)
- external baseline and positioning: [doc/EXTERNAL_BASELINE_AND_POSITIONING.md](doc/EXTERNAL_BASELINE_AND_POSITIONING.md)
- documentation link integrity: [doc/DOC_LINK_INTEGRITY_SPEC.md](doc/DOC_LINK_INTEGRITY_SPEC.md)
- tool comparison benchmark: [doc/TOOL_COMPARISON_BENCHMARK_SPEC.md](doc/TOOL_COMPARISON_BENCHMARK_SPEC.md)
- adversarial benchmark: [doc/ADVERSARIAL_BENCHMARK_SPEC.md](doc/ADVERSARIAL_BENCHMARK_SPEC.md)
- doc index: [doc/README.md](doc/README.md)

## License

This repository now uses `BUSL-1.1`.

- allowed without a commercial license: self-hosted evaluation, development, testing
- not allowed without a commercial license: production, hosted, or commercial operation
- full legal text: [LICENSE](LICENSE)
- plain-language policy: [LICENSE-POLICY.md](LICENSE-POLICY.md)
