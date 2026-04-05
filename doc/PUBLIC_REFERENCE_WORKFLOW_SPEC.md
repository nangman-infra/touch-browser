# Public Reference Workflow Spec

- Status: `Pilot-validated`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `sample MCP-backed public-web research workflow`

## 1. Overview

This document defines a reference workflow where an external agent uses `touch-browser` through the MCP bridge to execute a public-web research task.

Runner:

- [run-public-reference-workflow.mjs](../scripts/run-public-reference-workflow.mjs)

Run:

- `pnpm run pilot:public-reference-workflow`

## 2. Flow

1. initialize the MCP bridge
2. call `tb_session_create` with an allowlist
3. use `tb_open` and `tb_tab_open` to open public documentation pages
4. use `tb_extract` to retrieve public claims with citations
5. use `tb_session_synthesize`
6. call `tb_session_close`

## 3. Artifact

Generated report:

- [report.json](../fixtures/scenarios/public-reference-workflow/report.json)

## 4. Validation

- explicit script execution generates the artifact
- the eval smoke test validates the generated artifact contract
- the release-readiness and eval-harness gates require this public proof path
- the workflow reruns the same claim family used in the public web benchmark through the MCP bridge
- the current workflow includes `5` public tabs and `4` extracted claims
- the latest generated artifact reports a supported-claim rate of `1.00`

## 5. Notes

- network variability still exists, but this workflow now participates in the pilot-quality proof path
- this is a reference workflow for integration, not a full production customer-agent integration package
