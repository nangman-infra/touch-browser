# MCP Bridge Spec

- Status: `Experimental`
- Version: `v1`
- Last Updated: `2026-04-14`
- Scope: `stdio MCP bridge on top of touch-browser serve`

## 1. Overview

This document defines the thin MCP bridge that sits on top of `touch-browser serve` so external agents can use `touch-browser` as an MCP tool server.

Provided file:

- [integrations/mcp/bridge/index.mjs](../integrations/mcp/bridge/index.mjs)
- compatibility launcher: [touch-browser-mcp-bridge.mjs](../scripts/touch-browser-mcp-bridge.mjs)

Run:

- recommended local host path: `npx -y @nangman-infra/touch-browser-mcp`
- global npm package path: `touch-browser-mcp`
- installed standalone command: `touch-browser mcp`
- repo checkout: `pnpm run mcp:bridge`

Recommended local-host setup:

```json
{
  "mcpServers": {
    "touch-browser": {
      "command": "npx",
      "args": ["-y", "@nangman-infra/touch-browser-mcp"]
    }
  }
}
```

Minimal installed standalone setup:

```json
{
  "mcpServers": {
    "touch-browser": {
      "command": "touch-browser",
      "args": ["mcp"]
    }
  }
}
```

Repository checkout setup:

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

- product scope is public docs and research web
- `tb_search` is the discovery surface; engine selection is automatic
- `tb_read_view` is the readable scope-checking surface; inspect `mainContentQuality` and `mainContentReason` before extracting
- `tb_extract` is the evidence retrieval surface for four-state claim outcomes and citations
- `tb_session_synthesize` combines multi-page session traces into a single report
- supervised tools remain available for allowlisted, review-gated browser sessions, but MCP itself stays headless and does not accept `headed`

Recommended AI loop:

1. `tb_search`
2. `tb_search_open_top`
3. `tb_read_view`
4. `tb_extract`

Stop and hand off to a human when:

- `status` is `challenge`
- `nextActionHints` says recovery is human-owned
- the page indicates auth, MFA, or other supervised recovery
- `mainContentReason` shows the current tab is still too broad or low-confidence

Relevant tool inputs:

- `tb_search`: `sessionId`, `tabId`, `query`, `budget`
- `tb_search_open_result`: `sessionId`, `tabId`, `rank`
- `tb_search_open_top`: `sessionId`, `tabId`, `limit`
- `tb_read_view`: `target`, `mainOnly`, `browser`, `budget`, `sessionFile`, `allowDomains`
- `tb_extract`: `target`, `claims`, `verifierCommand`, `browser`, `budget`, `sessionFile`, `allowDomains`

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
- the npm package [packages/mcp/server.json](../packages/mcp/server.json) is the primary stdio distribution metadata for local MCP hosts
- the tool set is intentionally smaller than the full serve method surface
- search works browser-first inside touch-browser; the bridge forwards ranked result items and next-action hints rather than pretending the search phase is already resolved
- search responses carry `status`, `statusDetail`, and structured `nextActionHints.actor/canAutoRun/headedRequired`, but MCP clients should treat headed-required and challenge/auth/MFA states as supervised recovery handoff signals, not as permission to retry with headed settings
- interactive tools only make sense inside allowlisted daemon sessions and still require risk acknowledgement when challenge, MFA, auth, or high-risk-write signals appear
- the bridge starts `touch-browser serve` as an internal child process and injects `TOUCH_BROWSER_TELEMETRY_SURFACE=mcp`
- the standalone bundle ships both `touch-browser mcp` and `touch-browser serve`; the checked-in Node launcher itself remains a repository integration asset
- the npm package `@nangman-infra/touch-browser-mcp` is the preferred local-host install path; it downloads the matching standalone runtime and then launches `touch-browser mcp`
- child-process resolution order is `TOUCH_BROWSER_SERVE_COMMAND` -> `TOUCH_BROWSER_SERVE_BINARY` -> installed `touch-browser` on `PATH` -> packaged binaries under `bin/`, `dist/standalone/*/bin`, or repo-local `target/{release,debug}`
- if no binary can be resolved, the bridge fails fast and tells the operator to install a standalone bundle or build the repo once
- set `TOUCH_BROWSER_SERVE_COMMAND` to force a specific built binary or wrapper command
- use `verifierCommand` to let a second-pass judge adjudicate the final verdict without replacing the base evidence collector
