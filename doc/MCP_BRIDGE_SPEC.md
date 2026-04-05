# MCP Bridge Spec

- Status: `Experimental`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `stdio MCP bridge on top of touch-browser serve`

## 1. Overview

This document defines the thin MCP bridge that sits on top of `touch-browser serve` so external agents can use `touch-browser` as an MCP tool server.

Provided file:

- [touch-browser-mcp-bridge.mjs](../scripts/touch-browser-mcp-bridge.mjs)

Run:

- `pnpm run mcp:bridge`

Minimal setup:

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

## 2. Tool Coverage

The bridge implements a thin subset of MCP stdio JSON-RPC and proxies into `touch-browser serve`.

Supported methods:

- `initialize`
- `ping`
- `tools/list`
- `tools/call`

Current tool set:

- `tb_status`
- `tb_session_create`
- `tb_open`
- `tb_read_view`
- `tb_extract`
- `tb_policy`
- `tb_tab_open`
- `tb_tab_list`
- `tb_tab_select`
- `tb_tab_close`
- `tb_click`
- `tb_type`
- `tb_type_secret`
- `tb_submit`
- `tb_refresh`
- `tb_checkpoint`
- `tb_profile`
- `tb_profile_set`
- `tb_approve`
- `tb_secret_store`
- `tb_secret_clear`
- `tb_telemetry_summary`
- `tb_telemetry_recent`
- `tb_session_synthesize`
- `tb_session_close`

## 3. Intended Use

- `tb_read_view` is the readable surface for higher-level review or verifier models
- `tb_extract` is the evidence retrieval surface for structured claims and citations
- `tb_session_synthesize` combines multi-page session traces into a single report
- supervised tools remain available for allowlisted, review-gated browser sessions

Relevant tool inputs:

- `tb_read_view`: `target`, `mainOnly`, `browser`, `headed`, `budget`, `sessionFile`, `allowDomains`
- `tb_extract`: `target`, `claims`, `verifierCommand`, `browser`, `headed`, `budget`, `sessionFile`, `allowDomains`

## 4. Validation

- [mcp-bridge-smoke.test.ts](../evals/src/runtime/mcp-bridge-smoke.test.ts)
- [interface-compatibility.test.ts](../evals/src/runtime/interface-compatibility.test.ts)
- [serve-daemon.test.ts](../evals/src/runtime/serve-daemon.test.ts)
- round-trip validation for `initialize -> tools/list -> tools/call(tb_status)`
- daemon-path validation for read, extract, session synthesis, and supervised interaction flows

## 5. Notes

- this is a thin MCP proxy, not a full MCP resource or prompt server
- the tool set is intentionally smaller than the full serve method surface
- interactive tools only make sense inside allowlisted daemon sessions and still require risk acknowledgement when challenge, MFA, auth, or high-risk-write signals appear
- the bridge starts `touch-browser serve` as an internal child process and injects `TOUCH_BROWSER_TELEMETRY_SURFACE=mcp`
- use `verifierCommand` to attach a second-pass judge without replacing the base evidence collector
