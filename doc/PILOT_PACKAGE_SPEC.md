# Pilot Package Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `external agent integration surface`

## 1. Overview

This document fixes the current pilot package surface for first external-agent integrations.

## 2. Included Surface

Official user-facing runtime surface:

- `touch-browser serve`
- transport: `stdio JSON-RPC`
- methods:
  - `runtime.status`
  - `runtime.open`
  - `runtime.extract`
  - `runtime.policy`
  - `runtime.compactView`
  - `runtime.session.create`
  - `runtime.session.open`
  - `runtime.session.snapshot`
  - `runtime.session.compactView`
  - `runtime.session.extract`
  - `runtime.session.policy`
  - `runtime.session.synthesize`
  - `runtime.session.follow`
  - `runtime.session.paginate`
  - `runtime.session.expand`
  - `runtime.session.replay`
  - `runtime.session.close`
  - `runtime.tab.open`
  - `runtime.tab.list`
  - `runtime.tab.select`
  - `runtime.tab.close`

Repository validation and integration assets:

- `pnpm run mcp:bridge`
- `pnpm run pilot:reference-workflow`
- `pnpm run pilot:staged-reference-workflow`
- `pnpm run pilot:public-reference-workflow`
- `pnpm run pilot:healthcheck`
- thin stdio MCP bridge
- sample MCP-backed research workflow artifact
- sample MCP-backed staged public/trusted workflow artifact
- sample MCP-backed public-web workflow artifact
- self-hosted pilot container package

## 3. Validation

- [serve-smoke.test.ts](../evals/tests/runtime/gate/serve-smoke.test.ts)
- [serve-daemon.test.ts](../evals/tests/runtime/gate/serve-daemon.test.ts)
- [mcp-bridge-smoke.test.ts](../evals/tests/runtime/gate/mcp-bridge-smoke.test.ts)
- [reference-workflow-smoke.test.ts](../evals/tests/runtime/gate/reference-workflow-smoke.test.ts)
- [interface-compatibility.test.ts](../evals/tests/runtime/gate/interface-compatibility.test.ts)
- round-trip validation for `runtime.status` and `runtime.open`
- daemon round-trip validation for `session.create -> session.open -> tab.open -> session.synthesize -> session.close`
- MCP round-trip validation for `initialize -> tools/list -> tools/call(tb_status)`
- artifact generation for the sample MCP-backed research workflow
- artifact generation for the staged public/trusted workflow
- artifact generation for the sample MCP-backed public-web workflow
- standalone install runbook and install script included
- pilot Dockerfile, compose file, env example, and healthcheck included

## 4. Notes

- native multi-tab that keeps a shared browser context is not part of the pilot package
- the MCP bridge exposes a minimal tool subset by design
- current sample integrations focus on the reference workflow and explicit public-web workflows
- the package covers self-hosted pilot delivery, not hosted packaging or a managed control plane
- the standalone installed command is the primary user path; `pnpm run ...` commands are repo-checkout validation paths
