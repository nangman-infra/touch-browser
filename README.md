# touch-browser

[![License: BUSL-1.1](https://img.shields.io/badge/license-BUSL--1.1-blue.svg)](LICENSE)
[![Status: pilot-ready](https://img.shields.io/badge/status-pilot--ready-2d7d46.svg)](doc/RELEASE_READINESS_SPEC.md)

Turn any web page into structured, citable evidence for AI agents.

`touch-browser` turns a page into:

- readable Markdown for higher-level review with `read-view`
- compact semantic state for agent loops with `compact-view`
- traceable evidence with citations and optional verifier output
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
If you want a second-pass judge, add `--verifier-command <shell-command>` and attach verifier outcomes without changing the core evidence collector.

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
  -> extract (evidence + citations + optional verifier hook)
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
| `extract` | extract evidence-supported and insufficient-evidence claims with citations |
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
- doc index: [doc/README.md](doc/README.md)

## License

This repository now uses `BUSL-1.1`.

- allowed without a commercial license: self-hosted evaluation, development, testing
- not allowed without a commercial license: production, hosted, or commercial operation
- full legal text: [LICENSE](LICENSE)
- plain-language policy: [LICENSE-POLICY.md](LICENSE-POLICY.md)
