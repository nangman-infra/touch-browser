# Real User Research Benchmark Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `public multi-task proof for user-like AI research workflows`

## 1. 목적

이 문서는 `sample workflow` 수준을 넘어서, 실제 사용자가 AI research agent에 던질 법한 공개 문서형 질문 세트를 MCP 기반 multi-tab workflow로 반복 검증합니다.

핵심은 `검색엔진 랭킹`이 아니라 아래를 증명하는 것입니다.

- 실제 public URL 대상
- 질문 기반 multi-source research
- MCP 경유 외부 agent surface
- multi-tab control
- extract + synthesize + close까지 이어지는 end-to-end task proof

## 2. 산출물

generated report:

- [report.json](/Volumes/WD/Developments/touch-browser/fixtures/scenarios/real-user-research-benchmark/report.json)

실행 파일:

- [run-real-user-research-benchmark.mjs](/Volumes/WD/Developments/touch-browser/scripts/run-real-user-research-benchmark.mjs)

실행:

- `pnpm run fixtures:real-user-research`
- `pnpm run pilot:real-user-research`

## 3. 현재 기준선

이 benchmark는 현재 아래 질문군을 포함합니다.

- public standards research
- public web API docs research
- public Node.js runtime docs research

2026-04-05 generated baseline:

- scenario count: `3`
- passed scenario count: `3`
- total extracted claims: `8`
- total supported claims: `8`
- average supported claim rate: `1.00`
- average listed tab count: `3.00`
- unique public domains: `4`

기준 통과 조건:

- scenario count `3+`
- passed scenario count == scenario count
- average supported claim rate `1.00`
- average listed tab count `2+`
- unique public domains `4+`
- 각 scenario가 session close까지 완료

## 4. 해석

- 이 artifact는 로컬 sample app이 아니라 실제 public documentation sources를 대상으로 합니다.
- 따라서 `실사용 AI research 환경`에 더 가까운 public browsing proof 역할을 합니다.
- 현재 기준선은 IANA / RFC Editor / MDN / Node.js docs까지 포함하므로 단일 도메인 sample보다 source diversity가 높습니다.
- 다만 여전히 curated task suite이며, uncontrolled consumer traffic이나 arbitrary search ranking 자체를 증명하는 것은 아닙니다.

## 5. 현재 한계

- 검색 쿼리 discovery 레이어 자체는 제품 범위가 아니므로 URL/domain-curated research benchmark입니다.
- authenticated app, anti-bot page, private enterprise system은 포함하지 않습니다.
- public documentation source의 내용 변경이나 네트워크 변동성은 남아 있습니다.
