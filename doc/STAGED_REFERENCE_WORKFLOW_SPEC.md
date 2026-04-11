# Staged Reference Workflow Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `sample MCP-backed staged public/trusted research workflow`

## 1. Overview

This document defines a staged workflow where an external agent uses `touch-browser` through the MCP bridge to move from a public source to a trusted source during the same research task.

Runner:

- [run-staged-reference-workflow.mjs](../scripts/run-staged-reference-workflow.mjs)

Run:

- `pnpm run pilot:staged-reference-workflow`

## 2. Flow

1. initialize the MCP bridge
2. call `tb_session_create` with the public allowlist
3. use `tb_open` to open the local live public pricing page
4. use `tb_extract` to validate the public claim
5. use `tb_tab_open` to open the trusted fixture
6. use `tb_extract` to validate the trusted-source claim
7. use `tb_tab_list` and `tb_tab_select` to validate tab orchestration
8. use `tb_session_synthesize`
9. call `tb_tab_close`
10. call `tb_session_close`

## 3. Artifact

Generated report:

- [report.json](../fixtures/scenarios/staged-reference-workflow/report.json)

## 4. Validation

- [staged-reference-workflow-smoke.test.ts](../evals/tests/runtime/proof/staged-reference-workflow-smoke.test.ts)
- mixed public/trusted workflow artifact generation
- validation of `tb_tab_list`, `tb_tab_select`, and `tb_tab_close` on the artifact path itself

## 5. Notes

- the public stage uses a deterministic local live server, so it does not try to measure open internet variability
- the trusted stage uses fixtures rather than a live internal connector
- this is a staged integration reference, not a full production customer-agent integration package
