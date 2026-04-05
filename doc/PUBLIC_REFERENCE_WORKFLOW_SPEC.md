# Public Reference Workflow Spec

- Status: `Pilot-validated`
- Version: `v1`
- Last Updated: `2026-03-27`
- Scope: `sample MCP-backed public-web research workflow`

## 1. 목적

이 문서는 외부 agent가 `touch-browser`와 MCP bridge를 사용해 공개 웹 research task를 실제로 수행하는 reference workflow를 고정합니다.

실행 파일:

- [run-public-reference-workflow.mjs](../scripts/run-public-reference-workflow.mjs)

실행:

- `pnpm run pilot:public-reference-workflow`

## 2. 현재 흐름

1. MCP bridge initialize
2. `tb_session_create` with allowlist
3. `tb_open` / `tb_tab_open`으로 public documentation pages open
4. `tb_extract`로 public claims 검증
5. `tb_session_synthesize`
6. `tb_session_close`

## 3. 산출물

generated report:

- [report.json](../fixtures/scenarios/public-reference-workflow/report.json)

## 4. 현재 검증

- explicit script execution으로 artifact 생성 검증
- eval smoke test로 generated artifact contract 검증
- release-readiness / eval-harness gate에서 public proof 경로를 요구
- public web benchmark와 동일한 claim/task 계열을 MCP bridge 경유로 재수행
- 현재 workflow는 `5`개 public tabs와 `4`개 extracted claims를 포함합니다.
- 최신 artifact 기준 supported claim rate는 `1.00`입니다.

## 5. 현재 한계

- 네트워크 변동성은 남아 있지만, 현재는 pilot-quality proof gate 일부로 자동 실행합니다.
- sample workflow이며, 실제 고객 에이전트 프레임워크와의 production integration은 아닙니다.
