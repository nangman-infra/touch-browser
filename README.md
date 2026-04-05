# touch-browser

Turn any web page into structured, citable evidence for AI agents.

`touch-browser` opens live pages, compacts them into AI-friendly semantic snapshots, extracts source-linked evidence, and keeps replayable session traces that another AI system can inspect.

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

- public web benchmark: `5/5` live public-doc samples succeeded, `11.58x` average cleaned DOM token reduction
- real-user research benchmark: `3/3` MCP-driven public research scenarios passed, `8/8` claims returned with citations, `4` public domains covered

## What The Output Looks Like

Compact a real page:

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

Extract citable evidence from the same live page:

```bash
cargo run -q -p touch-browser-cli -- extract https://www.iana.org/help/example-domains \
  --claim "As described in RFC 2606 and RFC 6761, a number of domains such as example.com and example.org are maintained for documentation purposes."
```

```json
{
  "extract": {
    "output": {
      "supportedClaims": [
        {
          "claimId": "c1",
          "statement": "As described in RFC 2606 and RFC 6761, a number of domains such as example.com and example.org are maintained for documentation purposes.",
          "confidence": 0.95,
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

`confidence` here is the current evidence-match score for retrieved support, not a final truth guarantee.

## 30-Second Quick Start

```bash
bash scripts/bootstrap-local.sh

# Compact a real public page
cargo run -q -p touch-browser-cli -- compact-view https://www.iana.org/help/example-domains

# Extract source-linked evidence from the same page
cargo run -q -p touch-browser-cli -- extract https://www.iana.org/help/example-domains \
  --claim "As described in RFC 2606 and RFC 6761, a number of domains such as example.com and example.org are maintained for documentation purposes."
```

If you already built the binary, replace `cargo run -q -p touch-browser-cli --` with `touch-browser`.

## Architecture

```text
URL / fixture / browser tab
           |
      Acquisition
           |
 Observation compiler
 (semantic snapshot + stable refs)
           |
        Evidence
  (claims + citations)
           |
         Policy
  (allow / review / block)
           |
 Memory + session synthesis
           |
 CLI / JSON-RPC serve / MCP
```

## Use Cases

- research agents that need citations instead of raw HTML
- evidence-linked RAG ingestion pipelines
- policy-gated web research in self-hosted environments
- multi-tab source collection for reports, audits, and replay

## CLI At A Glance

Stable research surface:

| Command | What it does |
| --- | --- |
| `open` | open a target and compile a structured snapshot |
| `compact-view` | emit the compact semantic view optimized for AI consumption |
| `extract` | extract supported and unsupported claims with citations |
| `policy` | return allow/review/block signals for the current target |
| `session-synthesize` | combine multi-page session evidence into structured notes and claims |
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

The bridge starts `touch-browser serve` underneath and exposes tools like `tb_open`, `tb_extract`, `tb_tab_open`, and `tb_session_synthesize`.

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
- pilot package: [doc/PILOT_PACKAGE_SPEC.md](doc/PILOT_PACKAGE_SPEC.md)
- operations and security package: [doc/OPERATIONS_SECURITY_PACKAGE_SPEC.md](doc/OPERATIONS_SECURITY_PACKAGE_SPEC.md)
- public workflow examples: [doc/PUBLIC_REFERENCE_WORKFLOW_SPEC.md](doc/PUBLIC_REFERENCE_WORKFLOW_SPEC.md)
- real-user benchmark: [doc/REAL_USER_RESEARCH_BENCHMARK_SPEC.md](doc/REAL_USER_RESEARCH_BENCHMARK_SPEC.md)
- doc index: [doc/README.md](doc/README.md)

## License

Workspace metadata currently marks this repository as `UNLICENSED` in [Cargo.toml](Cargo.toml).
