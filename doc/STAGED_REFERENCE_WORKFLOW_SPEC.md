# Staged Reference Workflow Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-03-27`
- Scope: `sample MCP-backed public/trusted staged research workflow`

## 1. 목적

이 문서는 외부 agent가 `touch-browser`와 MCP bridge를 사용해 public source에서 trusted source로 단계를 넘겨 가며 research task를 수행하는 기준 workflow를 고정합니다.

실행 파일:

- [run-staged-reference-workflow.mjs](/Volumes/WD/Developments/touch-browser/scripts/run-staged-reference-workflow.mjs)

실행:

- `pnpm run pilot:staged-reference-workflow`

## 2. 현재 흐름

1. MCP bridge initialize
2. `tb_session_create` with public allowlist
3. `tb_open`으로 local live public pricing page open
4. `tb_extract`로 public claim 검증
5. `tb_tab_open`으로 trusted fixture open
6. `tb_extract`로 trusted-source claim 검증
7. `tb_tab_list` / `tb_tab_select`로 tab orchestration 검증
8. `tb_session_synthesize`
9. `tb_tab_close`
10. `tb_session_close`

## 3. 산출물

generated report:

- [report.json](/Volumes/WD/Developments/touch-browser/fixtures/scenarios/staged-reference-workflow/report.json)

## 4. 현재 검증

- [staged-reference-workflow-smoke.test.ts](/Volumes/WD/Developments/touch-browser/evals/src/runtime/staged-reference-workflow-smoke.test.ts)
- mixed public/trusted-source workflow artifact 생성 검증 완료
- MCP bridge tab list/select/close 표면을 실제 artifact 생성 경로에서 검증

## 5. 현재 한계

- public stage는 deterministic local live server로 고정되어 있어 실제 인터넷 변동성은 포함하지 않습니다.
- trusted stage는 fixture source를 사용하므로 real internal connector variability는 포함하지 않습니다.
- sample workflow이며 실제 고객 agent framework와의 production integration은 아닙니다.
