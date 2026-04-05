# Operations Security Package Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `self-hosted pilot operations and security package`

## 1. 목적

이 문서는 `touch-browser`를 self-hosted pilot 형태로 운영할 때 필요한 최소 운영·보안 패키지를 고정합니다.

현재 범위:

- container build artifact
- container compose artifact
- env example
- runtime healthcheck
- secret lifecycle runbook
- telemetry retention and audit runbook
- upgrade and rollback runbook

## 2. Container Runtime

제공 아티팩트:

- [deploy/Dockerfile](../deploy/Dockerfile)
- [deploy/docker-compose.pilot.yml](../deploy/docker-compose.pilot.yml)
- [deploy/touch-browser.env.example](../deploy/touch-browser.env.example)
- [scripts/pilot-healthcheck.mjs](../scripts/pilot-healthcheck.mjs)

기본 build:

```bash
docker build -f deploy/Dockerfile -t touch-browser:pilot .
```

기본 run:

```bash
docker run --rm -i \
  -e TOUCH_BROWSER_TELEMETRY_DB=/data/telemetry.sqlite \
  -e TOUCH_BROWSER_TELEMETRY_SURFACE=serve \
  -v "$(pwd)/output/pilot:/data" \
  touch-browser:pilot \
  target/release/touch-browser serve
```

compose 예시:

```bash
docker compose -f deploy/docker-compose.pilot.yml up --build
```

healthcheck:

- container healthcheck는 `node scripts/pilot-healthcheck.mjs`를 사용합니다.
- healthcheck는 `runtime.status` round-trip이 되는지만 확인합니다.

## 3. Secret Lifecycle

- direct CLI의 sensitive 값은 `--session-file` 옆 `secret sidecar`에만 저장됩니다.
- `session-close`는 browser context와 함께 secret sidecar 정리까지 포함해야 합니다.
- serve daemon의 민감값은 `daemon secret store` 메모리에만 저장되며 `runtime.session.secret.store` / `runtime.session.typeSecret` 경로로만 사용합니다.
- 운영자는 production-like pilot에서 plaintext CLI 인수 대신 daemon secret store를 우선 사용해야 합니다.
- 로그, telemetry, MCP 응답에는 raw secret을 남기지 않는 것을 기본 원칙으로 둡니다.

## 4. Telemetry Retention And Audit

- 기본 telemetry 경로는 `telemetry.sqlite`이며 `TOUCH_BROWSER_TELEMETRY_DB`로 override할 수 있습니다.
- pilot retention 기본 원칙은 짧게 유지하고 필요 시 외부 보관소로 export한 뒤 rotate하는 것입니다.
- 최소 audit 확인 경로:
  - `touch-browser telemetry-summary`
  - `touch-browser telemetry-recent --limit <count>`
  - serve `runtime.telemetry.summary`
  - MCP `tb_telemetry_summary`
- backup 전에는 `telemetry.sqlite` 파일을 정지 상태에서 복사합니다.
- retention 또는 export 정책은 팀 환경에 맞게 운영 문서에서 명시적으로 결정해야 합니다.

## 5. Upgrade And Rollback

- upgrade 전 단계:
  - current image or binary tag 기록
  - `telemetry.sqlite` backup
  - 기존 pilot env 파일 보관
  - `pnpm test` 또는 최소 smoke gate 확인
- upgrade:
  - 새 image build 또는 새 binary 배포
  - `scripts/pilot-healthcheck.mjs`로 runtime status 확인
  - reference workflow / staged workflow smoke 재실행
- rollback:
  - 직전 image or binary로 즉시 복귀
  - backup한 telemetry.sqlite 재연결 또는 기존 파일 유지
  - `touch-browser serve` healthcheck 재확인

## 6. Baseline Hardening

- 외부 live browsing은 allowlist를 기본값으로 사용합니다.
- auth, MFA, challenge, high-risk write는 `checkpoint -> approve` 흐름 없이 진행하지 않습니다.
- pilot container는 telemetry volume만 쓰기 가능하게 두고 나머지 파일 시스템은 가능한 한 불변으로 운영합니다.
- 공개 웹 벤치마크와 실제 pilot를 같은 telemetry DB에 섞지 않는 것을 권장합니다.
- session-file과 persistent browser context는 공유 스토리지보다는 단일 operator 경계 안에서 운영합니다.

## 7. 현재 한계

- managed control plane, RBAC, quota, tenant isolation은 아직 제품 범위 밖입니다.
- 자동 retention enforcement와 audit export 전용 API는 아직 없습니다.
- compose artifact는 stdio pilot 운영 예시이며 다중 운영자 환경 orchestration까지 보장하지 않습니다.
