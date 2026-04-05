# touch-browser

`touch-browser` is a self-hostable AI web runtime for research agents.

It is not a consumer browser and it is not a general-purpose stealth automation stack.
The core product shape is:

- compile web pages into AI-friendly semantic snapshots
- extract evidence-backed claims with citations
- keep replayable multi-page sessions with bounded memory
- enforce policy, allowlists, and supervised review at risky boundaries
- expose the runtime over CLI, JSON-RPC daemon, and a thin MCP bridge

## Product Boundary

Stable research surface:

- `open`
- `compact-view`
- `extract`
- `policy`
- `session-snapshot`
- `session-compact`
- `session-extract`
- `session-synthesize`
- `browser-replay`
- `serve`
- MCP bridge workflows

Experimental supervised surface:

- `checkpoint`
- `approve`
- `session-profile`
- `set-profile`
- `click`
- `type`
- `submit`
- `refresh`

The stable surface is intended for research-agent runtime teams.
The supervised surface exists to validate policy-gated auth/write boundaries and is not the core product promise.

## What It Can Do

- browse fixture, live, and browser-backed targets
- compile compact semantic state for AI consumption
- extract supported and unsupported claims with citation metadata
- synthesize multi-tab sessions into replayable reports
- run stateful daemon sessions behind stdio JSON-RPC
- expose a thin MCP tool surface for external agents
- demonstrate staged public/trusted-source research workflows

## What It Does Not Promise

- consumer GUI browsing
- fully autonomous login, payment, or destructive write flows
- CAPTCHA bypass, stealth browsing, or anti-bot evasion
- a generic search engine

## Quick Start

```bash
bash scripts/bootstrap-local.sh
cargo run -q -p touch-browser-cli -- compact-view fixture://research/static-docs/getting-started
```

More detail lives in:

- [doc/README.md](doc/README.md)
- [doc/INSTALL_AND_OPERATIONS.md](doc/INSTALL_AND_OPERATIONS.md)
- [doc/CLI_SURFACE_SPEC.md](doc/CLI_SURFACE_SPEC.md)
- [doc/PILOT_PACKAGE_SPEC.md](doc/PILOT_PACKAGE_SPEC.md)
- [doc/MCP_BRIDGE_SPEC.md](doc/MCP_BRIDGE_SPEC.md)
- [doc/OPERATIONS_SECURITY_PACKAGE_SPEC.md](doc/OPERATIONS_SECURITY_PACKAGE_SPEC.md)
- [doc/REAL_USER_RESEARCH_BENCHMARK_SPEC.md](doc/REAL_USER_RESEARCH_BENCHMARK_SPEC.md)
