# Pilot Package Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `external agent integration surface`

## 1. 목적

이 문서는 첫 외부 에이전트 연동을 위해 현재 제공되는 pilot package 표면을 고정합니다.

## 2. 현재 제공 범위

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

추가 표면:

- `pnpm run mcp:bridge`
- `pnpm run pilot:reference-workflow`
- `pnpm run pilot:staged-reference-workflow`
- `pnpm run pilot:public-reference-workflow`
- `pnpm run pilot:healthcheck`
- thin stdio MCP bridge
- sample MCP-backed research workflow artifact
- sample MCP-backed staged public/trusted-source workflow artifact
- sample MCP-backed public-web research workflow artifact
- self-hosted pilot container package

## 3. 현재 검증

- [serve-smoke.test.ts](../evals/src/runtime/serve-smoke.test.ts)
- [serve-daemon.test.ts](../evals/src/runtime/serve-daemon.test.ts)
- [mcp-bridge-smoke.test.ts](../evals/src/runtime/mcp-bridge-smoke.test.ts)
- [reference-workflow-smoke.test.ts](../evals/src/runtime/reference-workflow-smoke.test.ts)
- [interface-compatibility.test.ts](../evals/src/runtime/interface-compatibility.test.ts)
- `runtime.status`와 `runtime.open` round-trip 검증 완료
- daemon `session.create -> session.open -> tab.open -> session.synthesize -> session.close` round-trip 검증 완료
- MCP `initialize -> tools/list -> tools/call(tb_status)` round-trip 검증 완료
- sample MCP-backed research workflow artifact 생성 검증 완료
- staged public/trusted-source workflow artifact 생성 검증 완료
- sample MCP-backed public-web research workflow artifact 생성 검증 완료
- install/runbook과 bootstrap script 추가 완료
- pilot Dockerfile / compose / env example / healthcheck 추가 완료

## 4. 현재 한계

- shared browser context를 유지하는 native multi-tab은 아직 없습니다.
- MCP bridge는 minimal tools subset만 제공합니다.
- sample agent integration은 fixture-backed reference workflow와 explicit public-web workflow 수준입니다.
- self-hosted pilot container packaging까지는 포함하지만 managed control plane이나 hosted packaging은 아직 없습니다.
