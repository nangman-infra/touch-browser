# Reference Workflow Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-03-16`
- Scope: `sample MCP-backed research workflow`

## 1. 목적

이 문서는 외부 agent가 `touch-browser`를 어떻게 붙여 쓸 수 있는지 보여주는 reference workflow를 고정합니다.

실행 파일:

- [run-reference-research-workflow.mjs](/Volumes/WD/Developments/touch-browser/scripts/run-reference-research-workflow.mjs)

실행:

- `pnpm run pilot:reference-workflow`

## 2. 현재 흐름

1. MCP bridge initialize
2. `tb_session_create`
3. `tb_open`으로 pricing fixture open
4. `tb_extract`로 가격 claim 검증
5. `tb_tab_open`으로 docs fixture open
6. `tb_extract`로 docs claim 검증
7. `tb_session_synthesize`
8. `tb_session_close`

## 3. 산출물

generated report:

- [report.json](/Volumes/WD/Developments/touch-browser/fixtures/scenarios/reference-research-workflow/report.json)

## 4. 현재 검증

- [reference-workflow-smoke.test.ts](/Volumes/WD/Developments/touch-browser/evals/src/runtime/reference-workflow-smoke.test.ts)
- MCP-backed reference workflow artifact 생성과 핵심 claim extraction 검증 완료

## 5. 현재 한계

- 실제 고객 에이전트 프레임워크와의 통합이 아니라 sample workflow입니다.
- fixture 기반으로 고정되어 있어 실전 live-web variability는 포함하지 않습니다.
