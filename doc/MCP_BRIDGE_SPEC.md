# MCP Bridge Spec

- Status: `Experimental`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `stdio MCP bridge on top of touch-browser serve`

## 1. Overview

This document defines the thin MCP bridge that sits on top of `touch-browser serve` so external agents can use touch-browser as an MCP tool server.

Provided file:

- [touch-browser-mcp-bridge.mjs](../scripts/touch-browser-mcp-bridge.mjs)

Run:

- `pnpm run mcp:bridge`

Quick setup example:

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

## 2. Protocol Coverage

The bridge implements a minimal subset of MCP stdio JSON-RPC.

Supported methods:

- `initialize`
- `ping`
- `tools/list`
- `tools/call`

Current tool set:

- `tb_status`
- `tb_session_create`
- `tb_open`
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

## 3. Validation

- [mcp-bridge-smoke.test.ts](../evals/src/runtime/mcp-bridge-smoke.test.ts)
- [interface-compatibility.test.ts](../evals/src/runtime/interface-compatibility.test.ts)
- [serve-daemon.test.ts](../evals/src/runtime/serve-daemon.test.ts)
- round-trip validation for `initialize -> tools/list -> tools/call(tb_status)`
- daemon-path validation for `runtime.session.click`, `runtime.session.type`, and `runtime.session.submit`
- daemon-path validation for `runtime.session.typeSecret`, `runtime.session.secret.store`, and `runtime.session.refresh`
- daemon-path validation for `runtime.session.checkpoint` and `runtime.session.approve`
- daemon-path validation for `runtime.session.profile.get` and `runtime.session.profile.set`
- daemon-path validation for `runtime.telemetry.summary` and `runtime.telemetry.recent`
- reference workflow artifact generation
- staged public/trusted workflow artifact generation
- public-web workflow artifact generation

## 4. Notes

- this is a thin MCP proxy, not a full MCP resource or prompt server
- the tool set is intentionally smaller than the full serve method surface
- interactive tools only make sense inside allowlisted daemon sessions and still require risk acknowledgement when challenge, MFA, auth, or high-risk-write signals appear
- `tb_checkpoint` can return provider hints, required acknowledgement risks, approval panels, recommended profiles, and provider playbooks
- `tb_profile` and `tb_profile_set` directly inspect and control the supervised policy profile
- `tb_approve` stores approval state in daemon session memory
- `tb_telemetry_summary` and `tb_telemetry_recent` expose serve telemetry without exposing raw secrets
- the bridge starts `touch-browser serve` as an internal child process and injects `TOUCH_BROWSER_TELEMETRY_SURFACE=mcp`
