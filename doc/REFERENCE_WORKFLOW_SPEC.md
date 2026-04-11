# Reference Workflow Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `sample MCP-backed research workflow`

## 1. Overview

This document defines a reference workflow that shows how an external agent can integrate with `touch-browser`.

Runner:

- [run-reference-research-workflow.mjs](../scripts/run-reference-research-workflow.mjs)

Run:

- `pnpm run pilot:reference-workflow`

## 2. Flow

1. initialize the MCP bridge
2. call `tb_session_create`
3. use `tb_open` to open the pricing fixture
4. use `tb_extract` to validate the pricing claim
5. use `tb_tab_open` to open the docs fixture
6. use `tb_extract` to validate the docs claim
7. use `tb_session_synthesize`
8. call `tb_session_close`

## 3. Artifact

Generated report:

- [report.json](../fixtures/scenarios/reference-research-workflow/report.json)

## 4. Validation

- [reference-workflow-smoke.test.ts](../evals/tests/runtime/gate/reference-workflow-smoke.test.ts)
- MCP-backed reference workflow artifact generation
- core claim extraction validation on the generated artifact

## 5. Notes

- this is a reference integration workflow, not a production customer-agent integration
- it is fixture-backed by design, so it does not measure live-web variability
