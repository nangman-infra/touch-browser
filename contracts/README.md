# Contracts

이 디렉터리는 프로세스 간 계약의 기준 위치입니다.

원칙:

- canonical source는 `contracts/schemas/` 아래의 JSON Schema입니다.
- TypeScript와 Rust는 이 계약을 소비하는 구현입니다.
- 생성 산출물은 `contracts/generated/`에 둡니다.

현재 단계에서는 아래 흐름을 고정합니다.

1. JSON Schema 작성
2. `contracts:check`로 레이아웃 검증
3. `contracts:manifest`로 schema manifest 생성
4. 이후 Rust/TS 타입 생성기 연결

향후 예상 산출물:

- `contracts/generated/manifest.json`
- `contracts/generated/ts/*`
- `contracts/generated/rust/*`
