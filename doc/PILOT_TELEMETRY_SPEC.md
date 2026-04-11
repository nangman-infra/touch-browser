# Pilot Telemetry Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `pilot telemetry storage and query surface`

## 1. Overview

This document defines the minimum telemetry boundary recorded by `touch-browser` during pilot operation.

Goals:

- record CLI, serve, and MCP usage traces
- track session, tab, policy profile, and approval risks
- expose recent events
- expose aggregate summaries

## 2. Storage

- crate: [storage-sqlite](../core/crates/storage-sqlite/src/lib.rs)
- default path:
  - installed bundle: `~/.touch-browser/pilot/telemetry.sqlite`
  - repo checkout: `output/pilot/telemetry.sqlite`
- overrides:
  - `TOUCH_BROWSER_TELEMETRY_DB`
  - `TOUCH_BROWSER_TELEMETRY_SURFACE`

Recorded fields:

- `recordedAtMs`
- `surface`
- `operation`
- `status`
- `sessionId`
- `tabId`
- `currentUrl`
- `policyProfile`
- `policyDecision`
- `riskClass`
- `providerHints`
- `approvedRisks`
- `note`
- compact `payload`

## 3. Query Surface

CLI:

- `touch-browser telemetry-summary`
- `touch-browser telemetry-recent [--limit <count>]`

Serve:

- `runtime.telemetry.summary`
- `runtime.telemetry.recent`

MCP:

- `tb_telemetry_summary`
- `tb_telemetry_recent`

## 4. Validation

- [telemetry-smoke.test.ts](../evals/tests/runtime/gate/telemetry-smoke.test.ts)
- [serve-daemon.test.ts](../evals/tests/runtime/gate/serve-daemon.test.ts)
- direct CLI validation for telemetry summary and recent queries
- serve daemon validation for telemetry summary queries
- MCP bridge exposes telemetry tools and delegates to the serve surface

## 5. Notes

- this is a local pilot SQLite store, not a production-grade metrics pipeline
- payload storage is intentionally compact and does not store full DOM or raw HTML
- customer-consent workflows, redaction policy administration, and retention policy administration are not exposed as separate product features here
