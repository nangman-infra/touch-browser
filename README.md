# touch-browser

![Terminal demo](demo/terminal-demo.gif)

[![License: MPL-2.0](https://img.shields.io/badge/license-MPL--2.0-brightgreen.svg)](LICENSE)
[![Status: pilot-ready](https://img.shields.io/badge/status-pilot--ready-2d7d46.svg)](doc/RELEASE_READINESS_SPEC.md)

Ask a claim. Get page-grounded evidence, verdicts, and citations.

`touch-browser` is an evidence verification layer for AI agents. It does more than fetch a page or convert HTML to Markdown. It opens a page, compiles a structured snapshot, and tells you whether the current page supports a claim, contradicts it, or still needs more browsing.

Use it when you need:

- source-linked evidence instead of raw HTML dumps
- support snippets and verdict explanations that an agent can inspect before answering
- a safe unresolved path for borderline claims instead of bluffing
- policy-gated browsing instead of blind automation
- replayable, auditable multi-page research sessions

Evidence-first, not fact-final:

- `touch-browser` helps an AI collect page-local evidence and trace where it came from
- a higher-level model or human still decides what is true across pages or across the wider world

## What `extract` Returns

Abbreviated `claimOutcome` shape from the current extractor:

```json
{
  "statement": "The Starter plan costs $29 per month.",
  "verdict": "evidence-supported",
  "confidenceBand": "high",
  "reviewRecommended": false,
  "supportSnippets": [
    {
      "blockId": "b4",
      "stableRef": "rmain:table:plan-monthly-price-snapshots-starter-29-10-000-t",
      "snippet": "Starter | $29 | 10,000"
    }
  ],
  "verdictExplanation": "Matched direct support in 3 page block(s). Review the attached snippets before reusing the claim."
}
```

The extractor returns four verdicts:

- `evidence-supported`: the current page surfaced usable support
- `contradicted`: the current page surfaced conflicting evidence
- `insufficient-evidence`: the current page did not provide enough direct support
- `needs-more-browsing`: the current page is not specific enough yet, so the next step is another page

`confidenceBand`, `reviewRecommended`, `supportSnippets`, `verdictExplanation`, and `matchSignals` are there so an agent can decide what to do next without blindly trusting the first match.

## Standalone Bundle

Tagged `v*` pushes now build standalone macOS and Linux bundles in the `Standalone Release` workflow. Each bundle includes:

- `bin/touch-browser`
- the optimized Rust binary under `runtime/touch-browser-bin`
- a bundled Node runtime and Playwright adapter
- the default semantic runner scripts and model cache

When a tagged release is published, download the matching tarball from [GitHub Releases](https://github.com/nangman-infra/touch-browser/releases), unpack it, and run:

```bash
./touch-browser-<version>-<platform>-<arch>/bin/touch-browser telemetry-summary
```

To build the same portable bundle locally:

```bash
pnpm install --frozen-lockfile
pnpm run build:standalone-bundle -- v0.1.0-rc1

# Then run the bundled command from dist/standalone/<bundle-name>/bin
./dist/standalone/touch-browser-v0.1.0-rc1-<platform>-<arch>/bin/touch-browser telemetry-summary
```

## Build From Source

Prerequisites: [rustup](https://rustup.rs), Node.js 18+, `pnpm`.

```bash
bash scripts/bootstrap-local.sh
cargo build --release -p touch-browser-cli

# Verify a claim against a public page
./target/release/touch-browser extract https://www.iana.org/help/example-domains \
  --claim "As described in RFC 2606 and RFC 6761, a number of domains such as example.com and example.org are maintained for documentation purposes."

# Read the same page as Markdown
./target/release/touch-browser read-view https://www.iana.org/help/example-domains

# Produce the low-token agent view
./target/release/touch-browser compact-view https://www.iana.org/help/example-domains
```

For the inner development loop, `cargo run -q -p touch-browser-cli -- ...` still works, but the user-facing command is `touch-browser`.

`bootstrap-local.sh` installs the default semantic models under:

- `~/.touch-browser/models/evidence/embedding`
- `~/.touch-browser/models/evidence/nli`

Use `TOUCH_BROWSER_EVIDENCE_EMBEDDING_MODEL_PATH` or `TOUCH_BROWSER_EVIDENCE_NLI_MODEL_PATH` only when you need to override those default locations.

## Why Not Markdown Alone?

`touch-browser` is not trying to replace every crawler or browser tool. Its job starts after page acquisition.

| Need | Markdown-only fetch | touch-browser |
| --- | --- | --- |
| Read the page | yes | yes |
| Keep source-linked block refs | partial | yes |
| Judge whether the page supports a claim | no | yes |
| Return contradiction and unresolved states | no | yes |
| Give support snippets and verdict explanations | no | yes |
| Tell the agent to escalate instead of answering | no | yes |

## Product Surface

Primary surface:

- `extract`: verify claims against the current page and return structured claim outcomes

Supporting read surfaces:

- `read-view`: readable Markdown for a human reviewer or verifier model
- `compact-view`: low-token semantic state for agent loops
- `search`: structured discovery before opening candidate pages

Safety and audit surfaces:

- `policy`: classify pages and actions as allow, review, or block
- `session-synthesize`: turn a multi-page session into JSON or Markdown with citations
- `serve`: expose the runtime over stdio JSON-RPC for MCP or agent integration

## What touch-browser Is

- an evidence-first extractor
- a selective prediction surface
- a verifier-friendly routing layer

## What touch-browser Is Not

- a universal truth oracle
- a generic crawler replacement
- a guarantee that every unsupported claim is false

## MCP Example

Minimal MCP bridge setup from the repository root:

```json
{
  "mcpServers": {
    "touch-browser": {
      "command": "node",
      "args": ["integrations/mcp/bridge/index.mjs"]
    }
  }
}
```

The bridge starts `touch-browser serve` underneath and exposes tools like `tb_search`, `tb_search_open_top`, `tb_open`, `tb_read_view`, `tb_extract`, `tb_tab_open`, and `tb_session_synthesize`.

By default the bridge prefers an explicit `TOUCH_BROWSER_SERVE_COMMAND`, then a packaged or installed `touch-browser` binary, and only falls back to `cargo run -q -p touch-browser-cli -- serve` when no binary is available.

Use `TOUCH_BROWSER_SERVE_COMMAND` if you want to force a specific built binary or wrapper command.

## Architecture

```text
Query / URL / fixture / browser tab
  -> browser-first search result parsing
  -> Acquisition
  -> Observation compiler
  -> read-view / compact-view
  -> extract (evidence + citations + optional verifier)
  -> policy
  -> session synthesis / replay
  -> CLI / JSON-RPC serve / MCP
```

## Docs And Proof

- quick start and operations: [doc/INSTALL_AND_OPERATIONS.md](doc/INSTALL_AND_OPERATIONS.md)
- command surface: [doc/CLI_SURFACE_SPEC.md](doc/CLI_SURFACE_SPEC.md)
- evidence operating model: [doc/EVIDENCE_OPERATING_MODEL.md](doc/EVIDENCE_OPERATING_MODEL.md)
- examples: [examples/README.md](examples/README.md)
- integrations: [integrations/README.md](integrations/README.md)
- benchmarks and positioning: [doc/README.md](doc/README.md)
- pilot and operations package: [doc/PILOT_PACKAGE_SPEC.md](doc/PILOT_PACKAGE_SPEC.md), [doc/OPERATIONS_SECURITY_PACKAGE_SPEC.md](doc/OPERATIONS_SECURITY_PACKAGE_SPEC.md)

## License

This repository now uses `MPL-2.0`.

- commercial and non-commercial use are allowed
- if you distribute modified MPL-covered files, those covered files stay under `MPL-2.0`
- separate files in a larger work can use different terms
- full legal text: [LICENSE](LICENSE)
- plain-language policy: [LICENSE-POLICY.md](LICENSE-POLICY.md)
