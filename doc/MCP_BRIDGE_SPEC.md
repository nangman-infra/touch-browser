# MCP Bridge Spec

- Status: `Experimental`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `stdio MCP bridge on top of touch-browser serve`

## 1. Overview

This document defines the thin MCP bridge that sits on top of `touch-browser serve` so external agents can use `touch-browser` as an MCP tool server.

Provided file:

- [integrations/mcp/bridge/index.mjs](../integrations/mcp/bridge/index.mjs)
- compatibility launcher: [touch-browser-mcp-bridge.mjs](../scripts/touch-browser-mcp-bridge.mjs)

Run:

- repo checkout: `pnpm run mcp:bridge`

Minimal setup:

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
- `tb_search`
- `tb_search_open_result`
- `tb_search_open_top`
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
- `tb_search` is the discovery surface that structures Google or Brave result pages into ranked candidates and next-action hints, or returns `challenge` / `no-results` when the provider does not yield a normal result list
- `tb_extract` is the evidence retrieval surface for four-state claim outcomes and citations
- `tb_session_synthesize` combines multi-page session traces into a single report
- supervised tools remain available for allowlisted, review-gated browser sessions

Relevant tool inputs:

- `tb_search`: `sessionId`, `tabId`, `query`, `engine`, `headed`, `budget`
- `tb_search_open_result`: `sessionId`, `tabId`, `rank`, `headed`
- `tb_search_open_top`: `sessionId`, `tabId`, `limit`, `headed`
- `tb_read_view`: `target`, `mainOnly`, `browser`, `headed`, `budget`, `sessionFile`, `allowDomains`
- `tb_extract`: `target`, `claims`, `verifierCommand`, `browser`, `headed`, `budget`, `sessionFile`, `allowDomains`

Serve-to-MCP mapping:

| serve JSON-RPC method | MCP tool |
|---|---|
| `runtime.open` | `tb_open` |
| `runtime.readView` | `tb_read_view` |
| `runtime.extract` | `tb_extract` |
| `runtime.search` | `tb_search` |
| `runtime.search.openTop` | `tb_search_open_top` |
| `runtime.session.open` | `tb_tab_open` |
| `runtime.session.synthesize` | `tb_session_synthesize` |

## 4. Validation

- [mcp-bridge-smoke.test.ts](../evals/tests/runtime/gate/mcp-bridge-smoke.test.ts)
- [interface-compatibility.test.ts](../evals/tests/runtime/gate/interface-compatibility.test.ts)
- [serve-daemon.test.ts](../evals/tests/runtime/gate/serve-daemon.test.ts)
- round-trip validation for `initialize -> tools/list -> tools/call(tb_status)`
- daemon-path validation for search tool presence, read, extract, session synthesis, and supervised interaction flows

## 5. Notes

- this is a thin MCP proxy, not a full MCP resource or prompt server
- the tool set is intentionally smaller than the full serve method surface
- search works browser-first inside touch-browser; the bridge forwards ranked result items and next-action hints rather than pretending the search phase is already resolved
- search responses also carry `status`, `statusDetail`, and structured `nextActionHints.actor/canAutoRun/headedRequired`, so an MCP client can decide whether to open ranked tabs, re-run headed for a CAPTCHA, or hand the step back to a human
- interactive tools only make sense inside allowlisted daemon sessions and still require risk acknowledgement when challenge, MFA, auth, or high-risk-write signals appear
- the bridge starts `touch-browser serve` as an internal child process and injects `TOUCH_BROWSER_TELEMETRY_SURFACE=mcp`
- the standalone bundle ships `touch-browser serve`; the checked-in bridge launcher itself remains a repository integration asset
- child-process resolution order is `TOUCH_BROWSER_SERVE_COMMAND` -> `TOUCH_BROWSER_SERVE_BINARY` -> installed `touch-browser` on `PATH` -> packaged binaries under `bin/`, `dist/standalone/*/bin`, or repo-local `target/{release,debug}`
- if no binary can be resolved, the bridge fails fast and tells the operator to install a standalone bundle or build the repo once
- set `TOUCH_BROWSER_SERVE_COMMAND` to force a specific built binary or wrapper command
- use `verifierCommand` to let a second-pass judge adjudicate the final verdict without replacing the base evidence collector
