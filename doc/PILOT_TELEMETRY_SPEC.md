# Pilot Telemetry Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-03-18`
- Scope: `pilot telemetry storage and query surface`

## 1. 목적

이 문서는 `touch-browser`가 파일럿 운영 중 남기는 최소 telemetry 경계를 고정합니다.

현재 목적:

- CLI/serve/MCP 사용 흔적 기록
- 세션/탭/정책 프로필/승인 리스크 추적
- 최근 이벤트 조회
- aggregate summary 조회

## 2. 저장소

- crate: [storage-sqlite](../core/crates/storage-sqlite/src/lib.rs)
- 기본 경로: `output/pilot/telemetry.sqlite`
- override:
  - `TOUCH_BROWSER_TELEMETRY_DB`
  - `TOUCH_BROWSER_TELEMETRY_SURFACE`

기록 필드:

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

## 3. 현재 표면

CLI:

- `touch-browser telemetry-summary`
- `touch-browser telemetry-recent [--limit <count>]`

serve:

- `runtime.telemetry.summary`
- `runtime.telemetry.recent`

MCP:

- `tb_telemetry_summary`
- `tb_telemetry_recent`

## 4. 현재 검증

- [telemetry-smoke.test.ts](../evals/src/runtime/telemetry-smoke.test.ts)
- [serve-daemon.test.ts](../evals/src/runtime/serve-daemon.test.ts)
- CLI direct path에서 telemetry summary/recent 조회 검증 완료
- serve daemon 경로에서 telemetry summary 조회 검증 완료
- MCP bridge는 telemetry tools를 공개하고 serve surface를 그대로 위임

## 5. 현재 한계

- production-grade metrics pipeline이 아니라 local pilot SQLite입니다.
- payload는 compact summary만 저장하며 full DOM/HTML은 저장하지 않습니다.
- real customer telemetry consent, redaction policy, retention policy는 아직 별도 제품 기능이 아닙니다.
