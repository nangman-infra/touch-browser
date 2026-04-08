# CLI Architecture

## Goal

`touch-browser-cli`는 CLI/serve transport, application use case, infrastructure adapter를 분리해서
브라우저 자동화 세부 구현이 유스케이스를 오염시키지 않도록 유지한다.

## Layers

### interface

- 위치: `core/crates/cli/src/interface`
- 책임:
  - CLI 인자 파싱
  - serve JSON-RPC params 파싱
  - application 결과를 JSON/markdown/stdout 형태로 직렬화
- 금지:
  - Playwright 호출 직접 수행
  - 세션 파일 포맷 직접 해석
  - 유스케이스 정책 판단 구현

### application

- 위치: `core/crates/cli/src/application`
- 책임:
  - 검색, 세션, 브라우저 상호작용 유스케이스 실행
  - 정책 검사와 runtime orchestration
  - typed command/result 모델 유지
- 금지:
  - raw JSON shape 조립
  - Playwright request/response 타입 의존
  - 파일 시스템 세부 구현 의존
  - shell/process spawn
  - infrastructure default wiring

### infrastructure

- 위치: `core/crates/cli/src/infrastructure`
- 책임:
  - 세션 저장/복원
  - Playwright adapter 호출
  - fixture/telemetry/acquisition 어댑터 구현
- 금지:
  - CLI params 검증
  - serve JSON-RPC envelope 조립
  - 정책 판단 규칙 보유

## Dependency Direction

의존성 방향은 아래처럼 유지한다.

```text
interface -> application -> ports <- infrastructure
```

- `interface`는 `application` command/result만 안다.
- `application`은 포트만 알고, adapter 구현은 모른다.
- `infrastructure`는 포트를 구현하지만 `interface`를 참조하지 않는다.

## Port Rules

- 포트 이름은 구현이 아니라 역할을 드러내야 한다.
- `Playwright*Params/Result` 같은 adapter 스키마는 `infrastructure` 안에만 둔다.
- `application`은 `BrowserFollowRequest`, `BrowserSnapshotCaptureRequest` 같은 유스케이스 계약만 사용한다.

## Transport Rules

- JSON 직렬화는 `interface/serve_runtime/presenters.rs` 말단에서만 수행한다.
- handler는 params를 typed option으로 바꾸고 presenter를 호출한다.
- daemon state는 typed summary를 반환하고 `serde_json::Value`를 직접 조립하지 않는다.

## Review Checklist

- `application`에 `use crate::*;`가 없는가
- `application`에 `crate::infrastructure::` 또는 `default_cli_ports(`가 없는가
- `application`에서 `json!` 또는 `serde_json::Value`를 직접 만들지 않는가
- `application`에서 `Command::new` 또는 `Stdio::`를 직접 호출하지 않는가
- `ports.rs`가 adapter 스키마 대신 역할 중심 계약을 노출하는가
- `serve` 응답이 presenter에서만 직렬화되는가
- 새 기능 추가 시 test가 CLI/serve 회귀를 잡는가
- `pnpm run architecture:check`가 위 조건을 자동으로 막는가
