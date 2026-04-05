# Release Readiness Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `pilot-readiness gate for touch-browser`

## 1. 목적

이 문서는 현재 저장소 상태를 `pilot-ready / alpha-ready / incomplete`로 판정하는 내부 readiness artifact를 고정합니다.

## 2. 산출물

- [report.json](../fixtures/scenarios/release-readiness/report.json)

생성 경로:

- `pnpm run fixtures:release-readiness`

## 3. 판정 요소

- customer-fit baseline
- customer proxy task suite
- safety metrics
- 100-step memory stability
- staged public/trusted-source workflow
- observation G1 readiness
- operations/security package readiness
- latency-cost baseline
- public proof artifact 존재 여부
- real-user public research benchmark
- docs/scripts 존재 여부

## 4. 현재 해석

- `pilot-ready`: 내부 품질, 안전성, 장기 세션, mixed-source workflow, observation baseline, ops/security package, 운영 문서, public proof, real-user public benchmark가 모두 일정 기준 이상
- `alpha-ready`: public proof 일부, real-user benchmark, observation baseline, 또는 운영 패키지 보강이 남아도 pilot 전 단계로는 사용 가능
- `incomplete`: 핵심 내부 게이트가 아직 부족

## 5. 한계

- 이 readiness는 GA 판정이 아닙니다.
- real customer production telemetry와 운영 지원 체계는 별도입니다.
- `operationsPackageReady`는 self-hosted pilot 패키지 범위만 의미하며 managed cloud control plane까지 포함하지 않습니다.
